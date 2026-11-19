# Fallible BTreeSet

`jzap::collections::BTreeSet<T, P = Unmetered>` provides ordered unique ownership
on the existing `BTreeMap<T, (), P>` nodes. Allocation and callback failures are
explicit. The product uses stable Rust and adds no runtime dependency.

This report consolidates the API, implementation decisions, verification and
std comparisons. The pinned stable functionality checkpoint is complete.
Functional, memory and renewed source/link checks pass within the stated
boundaries; measured performance differences remain visible below.

## Pinned reference and API mapping

| Source | Identity | SHA-256 of `alloc/src/collections/btree/set.rs` |
| --- | --- | --- |
| Stable Rust | 1.95.0, `59807616e` | `e4994c6c821b738f9ce7bcbea0426f9d0ff14cffeaf0887dd29c0c7ce98f26e1` |
| Nightly Rust | nightly-2026-10-30, `92b5b8c87` | `068502799e10dd29e9c89f3f7c6350df7ca39a2d579835855cd3c4af191e4df2` |

The pinned stable inventory contains 27 inherent names. The maintained API check
reads the installed reference and rejects missing or newly introduced names.
Declarations are checked separately from behavior. Entry, get-or-insert and
cursor APIs remain unstable in this reference and are outside this stable
checkpoint. The [B-tree reference review](btree-design.md) covers node algorithm
references; those libraries are not product dependencies.

| std surface | jzap counterpart |
| --- | --- |
| `new`, `clear`, `len`, `is_empty` | Same names; empty construction does not allocate |
| `contains`, `get`, `first`, `last` | Same names; heterogeneous lookup through `Borrow<Q>` |
| `insert`, `replace` | `try_insert`, `try_replace` |
| `remove`, `take`, `pop_first`, `pop_last` | Same names; allocation-free |
| `range`, `retain`, `extract_if` | `try_range`, `try_retain`, `try_extract_if` |
| `split_off`, `append` | `try_split_off`, `try_append` |
| `iter` | Shared double-ended exact iterator; owning iteration is also supported |
| `difference`, `intersection`, `union`, `symmetric_difference` | Allocation-free sorted borrowing iterators |
| `is_disjoint`, `is_subset`, `is_superset` | Same names |
| `Clone`, allocating algebra operators | `TryClone`, `try_union`, `try_intersection`, `try_difference`, `try_symmetric_difference` |
| `FromIterator`, `Extend`, array conversion | `try_from_iter[_in]`, `try_extend`, `try_from_array` |
| `Default`, comparison, hashing, formatting | Empty `Default`/`TryDefault`, comparison/`Hash`/`Debug` |

Ordinary allocating `Clone`, collection, extension and algebra operators are not
implemented: their signatures cannot return allocation refusal. Explicit
counterparts provide that behavior. Borrowed/owning iterators implement the
applicable standard iterator traits and jzap's fallible mirror traits. Sources
for construction/extension use `TryIntoIterator`; source errors and rejected
insertions have separate `ExtendError` variants. Consumers that run to exhaustion
require finite source sequences, as in the project's loop/termination contract.

```rust
use jzap::{Error, TryClone, collections::BTreeSet};

fn example() -> Result<(), Error> {
    let mut left = BTreeSet::new();
    left.try_insert(3u64).map_err(|e| e.error)?;
    left.try_insert(7).map_err(|e| e.error)?;
    let mut right = BTreeSet::new();
    right.try_insert(7).map_err(|e| e.error)?;
    right.try_insert(11).map_err(|e| e.error)?;
    let shared = left.intersection(&right).next();
    assert_eq!(shared, Some(&7)); // Caller assertion, not a library requirement.
    let owned = left.try_union(&right)?;
    let copy = owned.try_clone()?;
    assert_eq!(copy.len(), 3);
    Ok(())
}
```

## Failure and ownership contracts

| Operation | Success and failure state |
| --- | --- |
| Insertion | A duplicate retains the stored object. Refusal returns the input in `Rejected<T>` and preserves the set/accounting. |
| Replacement | An equal key replaces exactly its initialized slot and returns the original object, without allocation. A missing key uses transactional insertion. |
| Removal / take / pop / clear | Objects move out or drop exactly once; no allocation is required. |
| Ranges | Reversed bounds and equal excluded endpoints return `Bounds`. Bounds are captured once by the underlying map. |
| Retain / extraction | Predicates return `Result`. Earlier removals remain on error; the current object is retained and that visit is consumed. Dropping extraction retains unvisited objects. |
| Split / append | Storage, policy cloning and accounting admission precede mutation. Refusal preserves both inputs. |
| Borrowed algebra | No allocation or payload cloning. Hint arithmetic checks overflow; an unrepresentable upper bound is `None`. |
| Owned algebra / clone | Cloning or storage refusal destroys the private partial result and preserves the inputs. |
| Extension | Successful insertions remain on source/insert error; the rejected current value is returned. Failed fresh construction cleans its private prefix. |

`new_in` accepts unmetered, borrowed local/concurrent and owned local/concurrent
allocation policies. Owned policies allow the set to outlive the creator's stack
frame. Nodes keep their existing layout rather than storing a policy per node.
Payload cloning and destination-policy cloning follow the existing `TryClone`
and `TryCloneIn` contracts. No mutable reference to a stored key is exposed.

User `Ord`, `Drop`, cloning, formatting and iterator implementations must obey
the documented returning-callback contracts. Allocators must obey their allocation
contract, including returning null on refusal rather than terminating. Public
unsafe callers retain their stated obligations. These boundaries do not exclude
internal arithmetic, allocation, cleanup or aliasing defects from library
responsibility. See the [project contract](jzap_std_specification_final.md#23-보장-계약).

## Implementation and generated code

The safe set wrapper delegates balancing, transactional edits and destruction to
the map. Equal-key replacement adds one internal initialized-slot primitive and
is checked by actual Rust Kani and the memory suite. Borrowed algebra chooses
search for highly unequal sizes and joint traversal for similarly sized sets,
with empty, disjoint and touching-endpoint shortcuts.

The first timing pass found per-item costs in three places:

- Borrowed traversal decoded the owning cursor's packed 16-bit index/height on
  every item. Borrowed frontiers now keep native-word fields. Canonical map
  iteration constructs leaf edges directly; partially consumed owning views
  retain their separate pending internal-pair conversion.
- Two independently peekable merge inputs carried duplicated state. Union and
  symmetric difference now share one pending reference and its source identity.
  Inlined folds expose leaf-construction invariants to the caller and reduce
  pending-owning-cursor processing and associated register spills.
- Owned algebra inserted every sorted output key through the general search
  path. A private packed sorted builder now uses the existing packing kernels.
  Length admission precedes mutation; failures clean its private initialized
  prefix. The packed push is inlined, removing a per-output call whose prologue
  saved six registers and maintained a large stack frame.

The append pool remains preallocated-only after commit. The sorted builder uses
on-demand fallible allocation only while its result is private. Deliberately
changing either admission rule is rejected by the maintained controls. The
initial hypothesis of an extra leaf `memmove` was disproved by disassembly and
is not used to explain the final improvement.

Compiler inlining and layout vary with the consumer. No global loop-alignment
flag is imposed. The existing optional 32/64-byte Cargo configurations remain
consumer choices. Measured source and executable identities are recorded by the
benchmark protocol; assembly and earlier source snapshots stay outside Git.

## Verification

The accepted verification split distinguishes proofs from executed memory tests:

| Property | Maintained check and scope |
| --- | --- |
| Stable behavior and refusal state | `tests/btree_set.rs`: std differential edits, algebra, ranges, identity, allocation-ordinal retry, callback/clone refusal, partial cleanup, over-alignment and zero-sized values |
| Shared map regression | `tests/btree.rs`: balancing, split borders, append recycling, mutable aliases and owning frontiers |
| Numeric bounds / tree geometry | Source-connected Verus models over machine words and 32-bit values; Python translation remains a trust boundary |
| Actual replacement and node kernels | Native Rust Kani, including every initialized replacement slot and non-Copy object identity |
| Borrowed leaf state | Actual Rust Kani establishes constructor bounds and one arbitrary front/back step for every valid leaf subinterval; preservation composes inductively |
| Lazy merge | Actual generic product merge over arbitrary sorted prefixes of two byte triples; ordering, membership, progress, fused termination and hints |
| Hint overflow | Actual Rust Kani over unrestricted machine-word lengths |
| Typed memory / cleanup | Miri strict provenance on native/i686 and native Tree Borrows; AddressSanitizer; differential fuzzing and LLVM coverage |
| Type boundaries | Compiler accept/reject cases and weakened Send/Sync negative controls |
| Internal exits | Fresh global source/model/link composition in release, LTO, overflow-checked release and debug consumer fixtures, with deliberate exit controls |

The leaf proof checks one arbitrary transition rather than enumerating all
histories. Its two frontiers delimit an initialized live prefix and its remaining
count equals their distance. Either next direction preserves that invariant and
returns the expected slot; an empty interval stays empty. Taller tree paths and
partially retired owning frontiers also use geometry contracts and actual memory
scenarios. The harness does not claim a universal heap-graph proof.

The merge harness compiles the product's generic merge directly with slice
iterators; it does not translate the algorithm into Python or a separate model.
Changing pending-source identity must yield a Kani counterexample. Changing a
borrowed constructor to start at the wrong end must also yield a counterexample.
Review fingerprints stop changed source from inheriting earlier correspondence;
a fingerprint is not semantic proof. The [verification guide](verification/README.md)
explains remaining projected models, direct product-source Verus targets and the
source/link composition boundary.

Recorded execution results for product implementation `deac281`:

| Check | Result |
| --- | --- |
| Stable inventory | All 27 inherent names mapped |
| Native / release / overflow-checked release | 276 tests and doctests passed per profile |
| Miri, strict provenance | 18 set and 20 map scenarios passed in each of native, i686 and Tree Borrows configurations |
| AddressSanitizer | All 38 set/map scenarios passed |
| Differential fuzzing | 10,000 runs per target; map corpus includes deep-tree seeds |
| Benchmark preflight | 432 std/product processes agree on work and checksums |
| Actual Rust Kani | Ten shared map/edge kernels and two set kernels passed; wrong constructor, pending identity and hint arithmetic were rejected |
| Source/compiler boundaries | 39 accept/reject cases passed; all four weakened trait-bound mutants detected |
| Emitted set coverage | 416/420 lines and 17/18 branch outcomes; this excludes uninstantiated generic code |
| Verification tools | 16 runner tests, parser regression tests, normal configurations, rustdoc, audits, licenses and SBOM checks passed |
| Final source/link composition | Fresh complete run in release, LTO, overflow-checked release and debug; zero open source conditions and zero unproved linked edges; all eight deliberate exit controls detected |

The final run used verification tooling `a4df267` with unchanged product source
from `deac281`. No generated prerequisite or owner model was reused. The common
runner's `summary.json` and its `certificate/summary.json` distinguish the
successful composition from the raw link step: all four binaries retain guarded
core checks, whose conditions are attached to the source/caller proofs. A raw
`--check link` can therefore report retained edges; `--check certificate` checks
their unreachability under the stated contracts. This is not a proof of arbitrary
Rust provenance or unrestricted user callbacks.

The source/link renewal also verifies the concrete `ptr::replace` guard used by
key replacement. It reads the pinned core argument/predicate expressions,
proves alignment/non-null acceptance for a valid typed slot, and rejects a
reversed null condition. Slot initialization/exclusivity are separate caller
obligations, covered by the replacement kernel, source contracts and memory
scenarios. Ordinary runtime statements remain visible to the AST census; only
an exact Verus-only `proof!` statement is erased from that runtime inventory.

The common owner checker routes the exact set file manifest to its independent
set checks. Reference variables named `left`/`right` do not acquire numeric-owner
contracts merely because their names match. Unknown files cannot inherit this
routing, and the routing helper participates in dependent input fingerprints.

## Performance and allocation

The comparison uses Rust 1.95.0 / LLVM 22.1.2, Linux x86_64, CPU 0, optimization
level 3, one codegen unit, panic=abort and default loop alignment. Each seed
prepares 64 shuffled `u64` cases outside timing. Mixed sizes include empty/single
sets and reach 4,096 elements; small sizes reach 32. Seeds are 617, 6203 and
96011. Normal algebra partially overlaps; skew algebra uses at most eight keys
against the full other set; disjoint algebra shifts the other keys by 1,000,000.

Every retained interval lasts at least one second. Implementations receive the
same seed, corpus, order and call count. The reported mean is summed elapsed
seconds divided by summed calls. Calibration, incomplete attempts and short
intervals are excluded. A second identical std process measures runtime drift;
conditions with control drift above 10% are repeated as whole conditions, using
fixed independent seeds rather than selecting favorable samples.

Lookup counts two queries per source key; replacement counts one key operation;
remove-cycle counts one take/reinsert pair. Other rows count a complete set
workload. Reads and algebra use prepared inputs. Destructive operations include
source cloning; owned algebra includes result construction and destruction.
These operation units are recorded by the executable's `describe` command.

Unmetered results compare the same set functionality. Local and concurrent
budget rows also include byte-ledger admission/refund, which std does not
provide; their ratios describe that optional feature's total cost. Each timed
context creates its ledger outside the interval. Owned-policy domain bootstrap
and reference-count costs remain documented in [owned budgets](verification/owned-budgets.md).

Measured on 2026-11-06 (KST), implementation checkpoint `deac281`.
The main campaign has **480 valid intervals**, with a minimum duration of
**1.0904 seconds**, across 27 mixed and five small-input conditions. No
aggregate identical-std control exceeded 10% drift (range: -7.8% to +6.3%).
The [compact timing aggregates](verification/btree-set-benchmarks.csv) retain
elapsed totals, call counts, seed spread and all optional-policy/control rows.

Thirty of the 32 unmetered means are within +15% of std, ranging from -37.8% to
+12.6%. Two short intersection conditions remain above that relative threshold:
small skewed inputs add **5.784 ns/call** (19.747 to 25.531 ns), and disjoint
inputs add **3.085 ns/call** (9.235 to 12.320 ns). Mixed skewed inputs add
4.026 ns/call. These observations separate the few-nanosecond short-path cost
from the removed per-element traversal regression; they do not prove a constant
penalty for every possible input. The checkpoint does not claim identical speed
for every operation or a universal confidence-interval performance pass.

The frozen comparison's shared `Owner::algebra` wrapper contains all lazy
operations. An inspected product instantiation is 10,305 code bytes versus
5,338 for std, while its stack reservation is 216 versus 280 bytes; both save
six registers. The product retains packed-index masks in endpoint lookup.
Constructor/fold placement, branches and code layout therefore still differ on
short calls. These are disassembly observations, not hardware-counter proof of
a cache stall or a complete cycle-by-cycle attribution of the residual ns.

Follow-up (2026-11-10, local, CPU 6 at 3.4 GHz, `perf stat` differences between
two epoch counts, seed 617). The 2026-11-09 campaign measured disjoint
intersection at +24.4% (6.393 to 7.953 ns). Per 64-call epoch, std executes
6,269 instructions and 1,251 cycles; jzap 8,838 and 1,773 (about 40
instructions per call). LLVM's inline remarks give the cause: the benchmark's
`Owner::algebra` wrapper is inlined into the timing loop for std (cost 115–120,
threshold 525) and not for jzap (cost 845–1,555 after constant-folding the
operation kind), so each jzap call pays the call, a six-register prologue and
the kind dispatch. The size comes from the always-inlined `intersection()`
constructor and fold, which `deac281` keeps in the caller so the canonical
cursor construction removes pending-cursor cases from the per-element loop.
Making the intersection fold `#[inline]` instead left the disjoint epoch at
8,837 instructions and was reverted. The same counters show the per-element
side of that trade: partial-overlap intersection 1.604M vs std 1.788M
instructions per epoch, subset 2.341M vs 2.882M. Moving the Search/Stitch
construction into a separate `#[inline]` helper (endpoint checks and `Answer`
kept always-inlined) gave 8,634 instructions but 1,812 cycles per disjoint
epoch, and skewed intersection went from 10,974 to 13,262 cycles per epoch;
adding the `#[inline]` fold as well left the wrapper's inline cost at 845–1,495
(remarks) with the same counts. Both were reverted.

The shared map regression adds 60 valid long intervals. Unmetered mixed
iteration/range/append changes are -18.9%/-17.6%/+1.1%; small iteration is -20.3%.
Its repeated-std controls range from -3.0% to +5.1%.

### Mixed inputs

| Operation | Calls | std ns/op | Unmetered ns/op | Local budget ns/op | Concurrent budget ns/op | Unmetered change | Concurrent budget change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| build | 128,128 | 32,930.886 | 32,467.201 | 32,791.719 | 31,729.993 | -1.4% | -3.6% |
| get | 79,660,708 | 52.122 | 50.127 | 49.982 | 51.396 | -3.8% | -1.4% |
| replace | 76,967,644 | 51.934 | 54.192 | 52.036 | 53.193 | +4.3% | +2.4% |
| iter | 5,377,024 | 846.990 | 805.524 | 970.224 | 920.806 | -4.9% | +8.7% |
| range | 12,840,640 | 459.633 | 337.313 | 379.756 | 329.678 | -26.6% | -28.3% |
| clone | 1,796,160 | 3,259.135 | 2,163.834 | 2,238.499 | 2,338.140 | -33.6% | -28.3% |
| remove | 108,992 | 36,640.137 | 36,191.476 | 36,248.872 | 35,575.525 | -1.2% | -2.9% |
| retain | 562,752 | 7,351.583 | 7,070.274 | 6,923.602 | 7,616.211 | -3.8% | +3.6% |
| extract | 583,168 | 6,926.207 | 7,064.570 | 7,242.923 | 7,563.529 | +2.0% | +9.2% |
| split | 1,662,912 | 3,558.916 | 2,212.485 | 2,686.559 | 2,605.801 | -37.8% | -26.8% |
| append | 417,216 | 9,498.446 | 9,683.732 | 10,273.565 | 10,870.738 | +2.0% | +14.4% |
| pop_all | 450,624 | 10,011.104 | 9,031.017 | 9,261.010 | 9,422.037 | -9.8% | -5.9% |
| into_iter | 1,513,536 | 2,893.672 | 2,666.981 | 2,893.081 | 2,835.502 | -7.8% | -2.0% |
| remove_cycle | 34,097,563 | 122.316 | 118.965 | 121.816 | 123.409 | -2.7% | +0.9% |
| union | 1,441,920 | 2,856.833 | 2,833.976 | 2,776.647 | 2,999.621 | -0.8% | +5.0% |
| intersection | 1,489,664 | 2,542.681 | 2,722.961 | 2,865.046 | 2,612.800 | +7.1% | +2.8% |
| difference | 1,330,560 | 2,990.334 | 2,909.504 | 2,886.592 | 2,977.699 | -2.7% | -0.4% |
| symmetric_difference | 1,495,744 | 2,930.427 | 2,728.774 | 2,816.578 | 2,633.687 | -6.9% | -10.1% |
| subset | 941,824 | 5,092.570 | 4,475.226 | 4,234.328 | 4,245.735 | -12.1% | -16.6% |
| intersection_skew | 64,747,136 | 62.510 | 66.536 | 66.988 | 58.800 | +6.4% | -5.9% |
| difference_skew | 61,254,144 | 68.204 | 64.107 | 67.061 | 68.827 | -6.0% | +0.9% |
| intersection_disjoint | 419,176,704 | 9.235 | 12.320 | 12.388 | 12.155 | +33.4% | +31.6% |
| union_disjoint | 1,126,144 | 4,563.606 | 3,840.775 | 4,014.925 | 3,462.730 | -15.8% | -24.1% |
| owned_union | 649,856 | 6,317.936 | 6,383.021 | 6,160.929 | 6,112.970 | +1.0% | -3.2% |
| owned_intersection | 935,104 | 4,349.323 | 4,273.730 | 4,297.929 | 4,395.270 | -1.7% | +1.1% |
| owned_difference | 1,025,216 | 3,651.072 | 3,933.442 | 3,868.149 | 3,933.994 | +7.7% | +7.7% |
| owned_symmetric_difference | 901,184 | 4,513.196 | 4,421.185 | 4,391.445 | 4,486.664 | -2.0% | -0.6% |


### Small inputs

| Operation | Calls | std ns/op | Unmetered ns/op | Local budget ns/op | Concurrent budget ns/op | Unmetered change | Concurrent budget change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| build | 51,320,064 | 73.728 | 82.984 | 82.805 | 84.571 | +12.6% | +14.7% |
| get | 912,751,940 | 4.793 | 4.903 | 4.950 | 4.311 | +2.3% | -10.1% |
| iter | 877,558,464 | 4.536 | 5.107 | 7.494 | 7.243 | +12.6% | +59.7% |
| intersection_skew | 191,346,944 | 19.747 | 25.531 | 28.654 | 24.259 | +29.3% | +22.8% |
| owned_union | 58,373,952 | 67.418 | 73.716 | 73.489 | 70.925 | +9.3% | +5.2% |

### Allocated memory and handles

A separate instrumented final-source executable produced **13,824 observations**:
18 operations, 64 cases, three seeds and std plus three product policies.
The [54 aggregate rows](verification/btree-set-memory.csv) distinguish requested
bytes from allocator-usable bytes. These are live-allocation metrics, not RSS.

| Metric | Worst individual product/std ratio |
| --- | ---: |
| Requested held bytes | 1.0000 |
| Allocator-usable held bytes | 1.0026 |
| Requested peak bytes | 1.1024 |
| Allocator-usable peak bytes | 1.1024 |

Every observed held/peak case is within +15%. All four owned algebra operations
match std exactly for requested/usable held and peak bytes in this corpus.
Read-only lookup, iteration, range and borrowed algebra allocate nothing.

| Handle on this 64-bit target | std bytes | jzap bytes |
| --- | ---: | ---: |
| Unmetered set | 24 | 24 |
| Set with a borrowed or owned budget policy | — | 32 |
| Shared iterator | 72 | 56 |
| Lazy union | 160 | 128 |
| Owning iterator | 72 | 40 |

The optional policy's eight-byte handle cost is separate from node allocations.
Owned ledger bootstrap costs are shared per domain and remain in the owned-budget
report; they are not included in these per-node ratios.


## Reproduce selected checks

The [tool and evidence guide](verification/README.md) describes installation
requirements and the distinction between direct Verus, models, Kani and memory
execution. No command below starts GitHub CI.

```sh
bash scripts/verify.sh --list
bash scripts/verify.sh --area btree-set --check api --check tests
bash scripts/verify.sh --area btree-set --check kani
bash scripts/verify.sh --area btree --check models
bash scripts/verify.sh --area btree-set --check miri --check miri-32 --check miri-tree --check asan
bash scripts/verify.sh --area btree-set --check coverage --check fuzz --check bounds
bash scripts/verify.sh --check unsafe
bash scripts/verify.sh --check certificate
bash scripts/verify.sh --profile full --plan
```

`--area btree --check kani` includes the shared node and borrowed-frontier
harnesses. For proof negative controls as well as positive kernels, run
`python3 scripts/verify_btree.py --kani --mutations` and
`python3 scripts/verify_btree_set.py --kani --mutations`. Receipts record exact
commands, tool output, source identity and failure/unrun status. Selected passes
do not substitute for the global certificate.

Build and finish checks before running timing; do not run compilers or solvers
concurrently with the following comparison:

```sh
cargo build --locked --release --manifest-path tools/throughput-analysis/Cargo.toml --bin btree_set
python3 tools/throughput-analysis/compare.py \
  --binary default=tools/throughput-analysis/target/release/btree_set \
  --small-tasks btree_set_build,btree_set_get,btree_set_iter,btree_set_intersection_skew,btree_set_owned_union \
  --small-variants default --target-seconds 1.3 \
  --output target/verification/btree-set/reproduction
python3 tools/throughput-analysis/report.py --input target/verification/btree-set/reproduction
```

Memory instrumentation uses a separate executable and is excluded from timing:

```sh
cargo build --locked --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --bin btree_set --features btree-memory --target-dir target/verification/set-memory-build
target/verification/set-memory-build/release/btree_set memory > target/verification/set-memory.jsonl
python3 tools/throughput-analysis/memory_report.py \
  --format btree-set --input target/verification/set-memory.jsonl \
  --output target/verification/set-memory.csv --max-ratio 1.15
```

Git keeps the current compact summary and reproduction commands. Raw JSON,
solver inputs, assembly, coverage, fuzz corpus and logs remain ignored under
`target/verification/`. Prior implementations are recovered from Git history.
