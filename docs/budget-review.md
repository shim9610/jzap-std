# Allocation budget architecture review

Updated on 2026-11-05 using Rust 1.95.0 and Kani 0.68.0. The original probes
used checkpoint `33b4fa6`; the maintained reallocation regression now uses the
standard allocator trait's default implementation. Fallible policy ownership is
implemented and certified through the iterator optimization at `c4c16e2`.
Owning local and concurrent budgets are now implemented; their
[usage guide](budget.md) and [verification report](verification/owned-budgets.md)
record the subsequent lifetime fix. Retained-size reallocation remains a
separate proposed contract.

| Concern | Verified current behavior | Required change |
| --- | --- | --- |
| Source-to-Verus correspondence | The Python bridge proves extracted projections; an added panic branch can escape that standalone check. | The default certificate now requires direct Rust budget checks and an unmodeled-branch control. Other projections retain their correspondence obligations. |
| Thread sharing | `MemoryBudget` uses `Cell` and cannot be shared across threads. `ConcurrentMemoryBudget` already uses atomics and supports thread sharing. | Retain both policies; use the concurrent implementation for shared state. |
| Dynamic ownership | Borrowed policies retain their borrow contracts. `OwnedBudget` and `OwnedConcurrentBudget` keep the ledger alive without a policy lifetime parameter. | Implemented using fallible Rc/Arc; use the owning concurrent form for dynamic `Send + 'static` state. |
| Reallocation admission | A 60-byte allocation growing to 100 bytes under a 150-byte limit is refused before calling the allocator. | Keep the conservative contract explicit; provide a separate retained-size accounting option if delta admission is added. |

## Source verification

The review compiled a mutation that inserts a panic after the local budget's
availability update and before reservation construction. The former `verify_verus.py`
returned success: its selected expressions were unchanged. This demonstrates a
gap in that standalone projection check, rather than proving that every check
in the combined pipeline accepts the defect.

The existing actual-Rust harnesses in `src/budget_proofs.rs` both passed:

- `reservation_and_commit_preserve_every_valid_state`
- `release_preserves_every_valid_owned_charge_partition`

The first harness rejected the compiled mutation at the inserted panic in
`MemoryBudget::reserve`. These loop-free harnesses quantify valid machine-word
accounting states and execute the real reservation, commitment and destruction
code. The release harness assumes ownership of a charge no larger than occupied
bytes. Both are now mandatory in the default certificate through
`scripts/verify_budget_native.py --mutations`, which also requires the inserted
panic to fail at its actual source location. This closes the demonstrated budget
check omission. Initialization and other projected models retain their separate
correspondence obligations; switching text extraction to an abstract syntax tree
alone does not prove equivalence of a translation.

## Thread and lifetime probes

Compiler probes confirmed that local `MemoryBudget` in a shared static is
rejected for lacking `Sync`, and that a Vec borrowing a local concurrent budget
is rejected by a `Send + 'static` future boundary. This existing configuration
passes both checks without leaking a budget:

```rust
use jzap::{budget::{ConcurrentBudget, ConcurrentMemoryBudget}, vec::Vec};

static BUDGET: ConcurrentMemoryBudget = ConcurrentMemoryBudget::new(150);
type AppBytes = Vec<u8, ConcurrentBudget<'static>>;

fn require_state<T: Send + Sync + 'static>() {}
fn check() { require_state::<AppBytes>(); }
```

Dynamic per-request or per-application budgets can use `OwnedConcurrentBudget`,
which holds the existing fallible `Arc<ConcurrentMemoryBudget>`.
The allocation-policy bound now uses `TryClone`, and Box, Vec, String, shared
owners and BTreeMap distinguish borrows, moves and fallible owner duplication.
Handle-count saturation remains an error before mutation. The
[policy contract](policy-duplication.md) and
[verification report](verification/policy-duplication.md) record this prerequisite.

Shared headers now hold their policy in `ManuallyDrop`. Final weak release takes
it exactly once, frees storage and refunds its charge before dropping the policy.
Weak owners retain this policy through payload retirement. Non-Copy owner and
refund oracles cover this order, including initializer failure and unwind.
The owning policies reuse this protocol: an internal borrow is scoped to the
operation while the stored policy owns its ledger. The creator can be dropped
before its containers, including containers surviving through a last Weak handle.

## Reallocation accounting

Both local and concurrent policies rejected 60-to-100 growth at limit 150 with
`Budget { requested: 100, available: 90 }`. Data, capacity and charges remained
unchanged; the local probe observed zero allocator realloc calls. Dropping the
owners refunded their charges.

Rust's pinned [`GlobalAlloc::realloc` default implementation](https://github.com/rust-lang/rust/blob/59807616e1fa2540724bfbac14d7976d7e4a3860/library/core/src/alloc/global.rs#L286)
allocates the replacement, copies the preserved prefix, then deallocates the old
block. The maintained test in `tests/reallocation_budget.rs` inherits that method
and observes both allocations. For the 60-to-100 growth, its peak requested bytes
are 160 and its retained bytes are 100. Reserving only the additional 40 therefore
cannot preserve old-plus-new admission for every permitted global allocator.
Allocator rounding, internal scratch and resident memory remain separate from
these requested-layout counts.

A retained-size mode is feasible: reserve positive growth before realloc,
cancel it on refusal, and release a shrink difference only after success. This
keeps the accounting ledger within its limit, while allowing transient allocator
storage above that limit. It needs a separate explicit contract.

### Admission conditions

Let `b` be unrelated occupied/reserved bytes, `o` the old layout size, `n` the new
layout size and `L` the limit. For a nonzero resize with a changed size, with
the original block already charged to this budget:

| Contract | Admission condition | What it bounds |
| --- | --- | --- |
| Current conservative policy | `b + o + n <= L` | Old and replacement requested layouts can coexist within the admitted total |
| Proposed retained-size policy | `b + max(o, n) <= L` | The accounting ledger and successful final requested layouts |

The formulas use mathematical integers. The implementation uses checked
availability and reservation transitions. When a moving allocator holds both
blocks, the second contract can undercount the temporary requested storage by
`min(o, n)`. Inspecting the returned pointer after realloc cannot prevent a peak
that already occurred. `GlobalAlloc` provides no preflight guarantee that a
resize will reuse the original block.

The regression runs each of these cases with both local and concurrent budgets:

| Limit | Resize | Result | Allocation attempts | Peak requested bytes | Retained bytes |
| ---: | --- | --- | ---: | ---: | ---: |
| 150 | 60 → 100 | Budget refusal before realloc | 1 | 60 | 60 |
| 160 | 60 → 100 | Success with movement | 2 | 160 | 100 |
| 160 | 60 → 100, replacement refused | Allocation error; reservation cancelled | 2 attempts | 60 | 60 |
| 100 | 100 → 60 | Budget refusal before realloc | 1 | 100 | 100 |
| 160 | 100 → 60 | Success with movement | 2 | 160 | 60 |

Every refusal preserves the original pointer, capacity, initialized bytes and
charge. Every final allocation is freed once and its budget returns to zero.
Native and strict Tree Borrows Miri runs pass. The shrink cases matter because
the standard default can allocate before freeing on shrink too.

### Available choices

| Choice | Benefit | Cost or prerequisite |
| --- | --- | --- |
| Preserve current admission and document growth/shrink clearly | Retains the established requested-peak contract | A resize can be refused although its final capacity fits |
| Reserve planned capacity from an empty Vec | Existing API avoids subsequent reallocation within that capacity | Unused capacity consumes budget from construction |
| Add an explicit retained-size mode | Allows 60 → 100 under a 150-byte retained-size limit | Moving realloc can temporarily exceed the limit; requires an opt-in contract |
| Require guaranteed in-place resizing | Could admit a resize without a second live layout | Requires an allocator guarantee absent from portable `GlobalAlloc` |

The current decision is to preserve conservative admission and document its
practical consequence for both growth and shrink. `try_with_capacity_in` already
supports planned capacity: allocating 100 bytes from an empty budget and then
growing the initialized prefix from 60 to 100 consumes no second allocation.
This applies when capacity can be planned in advance.

A retained-size mode is feasible as a distinct API. It would reserve positive
growth before realloc, cancel on refusal, and refund a shrink difference after
successful realloc. It is proposed here without changing the existing policy.
Budget lifetime ownership is independent of this accounting choice and can
proceed with conservative admission intact.

Reproduce the allocator cases with:

```sh
cargo test --locked --all-features --test reallocation_budget
MIRIFLAGS='-Zmiri-tree-borrows -Zmiri-strict-provenance' cargo +nightly-2026-10-30 miri test --locked --all-features --test reallocation_budget
```

## Implementation order

The agreed sequence is:

1. Finish BTreeMap implementation, verification, performance and documentation.
2. Replace `Copy`-dependent duplication with an explicit fallible API and verify it.
3. Evaluate reallocation-budget solutions, including documentation and alternative
   accounting contracts.
4. Review the resulting status, then resolve budget lifetime propagation.

Runtime policy changes need renewed failure-injection, thread/lifetime, Miri,
concurrency, source/guard and performance checks. Generated probes and raw
results remain outside Git.
