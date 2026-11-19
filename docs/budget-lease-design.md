# Budget lease (chunked credit) design

Status: **on hold (user decision, 2026-11-09); not implemented.** The lease is
an optional feature, and its risk and API cost (thread confinement, explicit
charge hand-off before moving a container, early refusal) were judged too large
for now. Shared concurrent ledgers keep their measured cost. This document fixes the
contract, lifetimes, failure behavior, verification and measurement criteria
before any code, as required by the project instructions. Items marked
**Decision** are open until the user confirms them.

## Motivation and measured cause

A `ConcurrentMemoryBudget` is one `AtomicUsize` (`used`). Every budgeted
allocation performs one `lock cmpxchg` reservation and every release one
`lock sub`. With four threads allocating against one ledger, those operations
move one cache line between cores on every allocation. Measured on the local
i7-13700KF (Rust 1.95.0, one-second protocol, `arc_scenarios`; ns per
operation, percentages relative to std; CPUs 8, 10 and 12 used for the
four-thread runs have the `performance` governor with turbo off but no fixed
minimum frequency):

| Scenario | jzap unbudgeted | Borrowed concurrent | Owned concurrent |
| --- | ---: | ---: | ---: |
| `request64`, 1 thread | 75.7 ns | 93.6 ns | 102.3 ns |
| `request64`, 4 threads | 19.2 ns | 53.7 ns (+180%) | 97.7 ns (+410%) |
| `request4096`, 4 threads | 34.9 ns | 61.7 ns (+88%) | 102.1 ns (+212%) |

Owned concurrent policies add a policy-count `lock inc`/`lock dec` per
container, and the policy count shares the ledger's allocation. In a short
`perf stat` run of `request4096` with four threads (setup included, so only the
ratios are used), the owned policy shows about 17 times the cross-core
forwarded loads (`mem_load_l3_hit_retired.xsnp_fwd`) and about 130 times the
memory-ordering machine clears of the borrowed policy. An experiment replacing the
reservation CAS with `fetch_add` plus rollback changed one-thread results by
less than 2% and four-thread results by 3–14%. The number of shared atomic
operations, not their form, is the cost.

A lease takes credit from the shared ledger in chunks and serves allocations
from a private ledger, so the shared ledger is touched once per chunk instead
of once per allocation.

## Scope

In scope: one borrowed, thread-confined lease type over an existing
`ConcurrentMemoryBudget` (directly or through `OwnedConcurrentBudget::ledger()`),
its policy handle, and the accounting dispatch needed by existing containers.

Out of scope unless separately approved: an owning (lifetime-free) lease, a
thread-shareable lease, leases over the local `MemoryBudget`, automatic
per-thread leases (TLS), and changes to existing policies' behavior.

## Types (proposed)

```rust
/// Thread-confined credit taken from a shared concurrent ledger in chunks.
pub struct BudgetLease<'p> { /* parent: &'p ConcurrentMemoryBudget, chunk,
                               granted, occupied, available: Cell<usize> */ }

impl<'p> BudgetLease<'p> {
    /// No ledger operation and no allocation; `chunk` may be zero.
    pub const fn new(parent: &'p ConcurrentMemoryBudget, chunk: usize) -> Self;
    pub fn granted(&self) -> usize;   // credit currently held from the parent
    pub fn occupied(&self) -> usize;  // committed charges of this lease
    pub fn reserved(&self) -> usize;  // pending reservations of this lease
    pub fn available(&self) -> usize; // unused credit held by this lease
    /// Return unused credit above `keep` to the parent.
    pub fn release_unused(&self, keep: usize);
}

/// Policy handle stored by containers; Copy, borrow duplication never fails.
#[derive(Clone, Copy)]
pub struct LeaseBudget<'l>(pub &'l BudgetLease<'l>);
```

`BudgetLease` uses `Cell` and is therefore `!Sync`. `LeaseBudget<'l>` holds a
shared reference, so it and every container storing it are `!Send`. The
compiler, not documentation, prevents a charge from being released on another
thread. `BudgetLease` itself is `Send` (it can be moved to a worker before use)
because it only borrows a `Sync` parent.

`LeaseBudget` implements the sealed `AllocationPolicy`, and `budget()` returns
a new `Accounting::Lease(&BudgetLease)` variant. `TryClone` returns a copy, like
`Budget<'a>`.

## Ledger state and invariants

Lease fields: `granted` G, `occupied` U, `available` V; pending reservations are
`R = G - U - V`.

1. Lease: `U + V + R == G`, all fields non-negative (no wrapping in valid states).
2. Parent: its `used` counts G for every live lease plus all direct charges, so
   the existing parent invariant `used <= limit` is unchanged. A lease never
   causes the parent limit to be exceeded.
3. Domain identity: `same_domain` is true only for the identical lease
   (`ptr::eq`). Two leases of one parent are different domains, because a
   charge must be refunded to the lease that holds it. A lease and its parent
   used directly are different domains.

## Transitions

All arithmetic uses the existing checked/bounded numeric helpers. No transition
calls user code, allocates, panics or blocks.

**Reserve `b` bytes** (allocation, reallocation's new request):

1. If `b <= V`: `V -= b`, return a lease reservation.
2. Otherwise let `need = b - V`. Request `grant = max(chunk, need)` from the
   parent with the existing `reserve` (CAS). If the parent refuses and
   `grant > need`, request exactly `need` once. This retry keeps chunking from
   refusing a request that the parent could still admit.
3. On parent success: `G += grant`, `V += grant`, then step 1.
4. On parent refusal: return `Error::Budget { requested: b, available }` where
   `available = V + parent.available()` observed at refusal. G, U, V and the
   rejected input are unchanged.

**Commit** (allocator success): `U += b` (R decreases by `b`).

**Cancel** (allocator refusal or unwind before commit): `V += b`. Credit stays
in the lease.

**Release** a committed charge `b` (storage freed): `U -= b`, `V += b`, then
return surplus: if `V > 2 * chunk`, give `V - chunk` back to the parent
(`G -= V - chunk`, `V = chunk`, parent `release`). The two-chunk threshold
avoids returning and re-requesting a chunk on every alternation.
**Decision:** threshold `2 * chunk` and keep `chunk` (proposed) versus
returning everything above zero.

**Drop the lease:** containers and reservations borrow the lease, so U and R
can be non-zero only through a deliberate leak (`mem::forget`). Return V to the
parent. Leaked U stays charged to the parent, which matches existing behavior
for leaked budgeted allocations.

**Reallocation** keeps the existing rule: the full new request is reserved
while the old charge stays live, and the old charge is released after the
allocator succeeds.

**Zero-size requests** follow the existing storage path (a zero-byte
reservation, no parent operation).

## Contract changes users must know

1. **Early refusal.** Unused credit held by other leases is not available to a
   request. A request can be refused while real usage is below the limit, by
   at most the unused credit of the other live leases (at most about
   `2 * chunk` each after any release, plus their unreleased grants).
   The limit is never exceeded.
2. **Usage queries.** `ConcurrentMemoryBudget::used()` includes credit granted
   to leases; per-lease `occupied()` gives real usage.
3. **Error payload.** `Error::Budget::available` for a lease refusal is the
   lease's unused credit plus the parent's available bytes at refusal, a
   snapshot that other threads can change immediately.

## Failure and ownership guarantees (unchanged from existing containers)

Single-input refusal preserves the container and the input; cross-container
moves preserve both; partial construction is cleaned once. A lease adds no new
failure kind: it reports `Error::Budget` and never `RefCount` or `Allocation`
by itself.

## Platform

The lease uses no atomics, OS calls or thread-local storage. It requires only
the parent's existing `target_has_atomic = "ptr"` condition and works on every
platform the concurrent budget supports, with or without `std`.

## Implementation impact (to be reviewed before coding)

- `Accounting` gains a `Lease` variant. Every match (`reserve`, `release`,
  `commit`, `retain`, `same_domain`, `is_metered`) and the guard types change.
- `scripts/credit_bindings.py` pins the shape of `storage::allocate_impl` and
  `reallocate`; the lease path needs its own bindings.
- Budget proof harnesses (`src/budget_proofs.rs`, Verus numeric functions in
  `src/budget_numeric.rs`) gain lease transitions.
- Existing policies keep identical generated code. This is checked by comparing
  the `.text` of the existing benchmark binaries, as done for the Vec change.

## Verification plan

| Property | Tool |
| --- | --- |
| Lease numeric transitions (grant, retry, commit, cancel, release, surplus return) never wrap and keep invariants 1–2 for all machine words | Verus on the product numeric functions; Kani on the actual Rust transitions |
| Parent `used <= limit` and no lost or doubled credit with several threads each holding a lease, including refusal and surplus return races | Loom on the actual code |
| Containers (Box, Vec, String, BTreeMap, HashMap, Arc) with a lease policy: refusal preservation, cleanup, reallocation, drop order | Existing test suites instantiated with `LeaseBudget`; Miri strict, Tree Borrows, i686 |
| Allocator refusal at every allocation index under a lease | Existing failure-injection tests |
| `LeaseBudget` containers are `!Send`; a lease cannot outlive its parent | Compile-fail tests |
| No panic, abort or allocation in the lease path | Link census (`check_link_exits.py`) unchanged from base |
| Existing policies unchanged | Byte comparison of benchmark `.text` and the existing area verification |

## Measurement plan and criteria (fixed before measuring)

Workloads: `arc_scenarios` `request64` and `request4096` with 1 and 4 threads,
and the Vec small `clone`/`pop` tasks, one lease per worker, chunk 64 KiB
(**Decision:** chunk value). One-second protocol, seeds 617/6203/96011, CPU 6
for one thread and CPUs 6, 8, 10, 12 for four threads.

Proposed criteria (**Decision**):

- One thread: lease policy within +5% of the local `Budget<'a>` policy for the
  same workload (the lease's private ledger costs about the same as the local
  ledger).
- Four threads: lease policy per-operation time at most +25% over jzap
  unbudgeted, against +88% to +180% for the borrowed concurrent policy today.
- Memory: requested and peak allocator bytes identical to the borrowed
  concurrent policy; ledger state adds no heap allocation.

## Open decisions

1. Accept the early-refusal contract change for this optional policy.
2. Chunk default, or require the caller to pass it (proposed: caller passes it).
3. Surplus threshold (`2 * chunk`, keep `chunk`).
4. Scope: borrowed thread-confined lease only (proposed) or also an owning form.
5. Performance criteria above.
