# Shared ownership implementation and verification

The implementation provides fallible counterparts of the pinned Rust 1.95 stable
Rc/Weak and Arc/Weak APIs, with local and concurrent byte budgets. The
[API mapping](../shared-api.md) and [guide](../shared.md) describe conversions,
copy-on-write (COW), pinning and failure ownership. Product code adds no external
runtime dependency. Loom is isolated in a verification-only workspace.

The measurements and verification counts below describe the original shared-owner
campaign. The [current integrated report](arc-headroom-integration.md) records
the renewed source/link certificate and verification, and supersedes the Arc CAS
performance/code-generation baseline on native Linux x86-64. Rc still uses local
count updates. [Owning-budget results](owned-budgets.md) add the lifetime-owning
policies. Std has no byte-budget accounting: metered/std ratios show the total
cost of the additional feature, as defined in the current report's
[comparison scope](arc-headroom-integration.md#comparison-scope).

## Source and verification

The source baseline is `fea2479ed7b6ee8b464c8a4a7c4a94bf96ee1ae8` plus the
String/shared implementation. The local certificate hashes product sources,
projection checkers, proof kernels, parser sources and Loom inputs. New source
phases, unsafe operations, changed actual arguments and unproved final-link
guards invalidate completion. Generated inputs and logs remain under ignored
`target/verification/shared`.

| Check | Executed result |
| --- | --- |
| Stable API inventory | 24 distinct stable names in each Rc/Weak and Arc/Weak reference mapped |
| Normal / overflow-checked release | 208 cases each, including 32 shared tests and 26 doctests |
| Reduced count limit | 33 shared tests with a live count ceiling of seven |
| Miri | 31 scenarios each on x86_64, i686, and x86_64 Tree Borrows with strict provenance |
| AddressSanitizer | 32 shared tests |
| State-machine fuzzing | 10,000 shared runs, maximum input length 128 |
| Kani | 14 actual shared implementation harnesses and two concurrent-budget harnesses |
| Verus | 26 native-word and 26 32-bit contracts |
| Loom | Nine scenarios over actual atomic counter/budget code; two compiled semantic defects detected |
| Thread/lifetime bounds | 76 compiler cases; 12 weakened-bound controls rejected |
| Shared LLVM coverage | 900/953 lines and 69/90 branch outcomes, including concurrent-budget code |
| Existing Box/Vec/String memory paths | 136 additional Miri scenarios passed after accounting integration |
| Dependency checks | Root and Loom lockfiles audited; root license/source checks and software bill of materials generated |
| Builds | Stable default/std/core-only; wasm32 and thumbv7em; nightly no-global-OOM-handling |
| Unsafe census | 97 shared blocks; complete reviewed phase/argument bindings, including macro signatures |

The completed combined certificate has **624 source obligations, zero open
obligations and zero unproved linked edges**. Its exact input fingerprint is:

```text
a407fb1ff6ca7cc33c472d0819f0be5b69fc3753fc9bc956bf8d0704aae3c0fb
```

Release, fat link-time optimization (LTO) and overflow-checked release fixtures
have no retained termination edges. Debug's 47 originating guard calls and four
forwarding calls are attached to source-connected unreachability contracts.
The new atomic load/store/compare-exchange/fence guards are discharged by the
pinned core predicates, actual constant order arguments and Kani order checks.
Deliberate panic, bounds, allocation-termination and abort controls are detected.
Each profile executes 13,347 concrete cases, including 13,087 explicit refusals.

Kani's numeric harnesses cover unrestricted count admission, valid decrements,
layout extension and retired charge encoding. Layout checks instantiate local,
unmetered and concurrent policy headers. Sequential atomic Kani witnesses verify
actual operations; interleavings and publication ordering use Loom. Other Kani
heap witnesses cover actual last release, both COW branches, retained weak
refunds, owned cyclic publication/refusal and a three-byte Box-slice metadata
move. The latter executes 650 safety checks; it is not a proof for every arbitrary dynamically sized type.

Verus checks numeric bounds, prefix extents, exclusive ghost credits, retirement,
cyclic publication, atomic-order admissibility and reservation/commit algebra.
Reviewed complete source bodies bind actual constructors, actual pointer/count
arguments, private fields and all transitions to these contracts. This is a
fail-closed source projection rather than a general Rust separation-logic proof.

The compiler and runtime memory tools cover typed allocation, pointer provenance,
aliasing, destruction and wide-pointer metadata. Miri scenarios include trait
objects, nested dynamically sized structures, zero-sized and over-aligned values,
partial initialization and destructor unwind. Generic Box transfer relies on the
pinned compiler's wide-pointer data-first application binary interface (ABI),
following portable-atomic-util's
stable implementation pattern. The pointer component is copied as a pointer to
preserve allocation provenance. Numeric size lemmas alone do not establish ABI or
aliasing correctness.

Loom directly exercises final release against upgrade, uniqueness against
downgrade, copy-on-write against upgrade, publication, concurrent into_inner,
strong/weak cleanup, budget cancellation/commit and retained weak charges.
A last-release acquire witness uses Loom's checked UnsafeCell. Removing release
ordering produces a causality violation; removing a refund produces an assertion
failure. Mutant builds use isolated target directories and verify their source
paths. Additional compiled source changes to admission, payload offset, implicit
credit and budget compare-exchange are rejected by source bindings. Compiled
changes to the header representation, counter seal, pinned-owner privacy and
exclusive reservation cloning are also rejected. A prefix overrun fails the solver.

Unsafe caller contracts and user-defined allocator, callback, iterator and payload
operations remain explicit boundaries. The final-link fixture checks concrete
roots and cannot certify arbitrary consumer code. Consumers can run the same
[link checks](../iteration.md) against their own concrete call graph.

## Performance results

Measured on 2026-11-04 with Rust 1.95.0 / LLVM 22.1.2, Linux 6.18.57 and an
AMD EPYC 9V74 guest pinned to CPU 0. The build uses opt-level 3, one codegen unit,
panic=abort and default loop alignment. Builds and proofs finish before timing.

The final campaign contains **540 valid recordings**, all at least one second,
across 108 matched seed groups. The [benchmark aggregates](shared-benchmarks.csv)
contain 144 policy/control means, total elapsed seconds, calls and seed ranges.
All 36 default-policy means fall within the observed +15% range: **−74.5% to
+11.7%** versus std. These three-seed observations are distinct from an
independent-process bootstrap confidence-interval gate.

Values below are nanoseconds per complete workload call, derived from summed
long-interval time divided by summed calls. A negative delta is faster.

| Workload | std Rc ns | jzap Rc ns | Delta | std Arc ns | jzap Arc ns | Delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| construct | 10.93 | 10.15 | -7.1% | 11.60 | 11.00 | -5.2% |
| clone | 1.02 | 1.10 | +7.9% | 5.02 | 4.86 | -3.3% |
| weak_clone | 1.33 | 1.34 | +0.5% | 5.31 | 5.02 | -5.4% |
| downgrade | 1.03 | 1.15 | +11.7% | 4.81 | 4.78 | -0.7% |
| upgrade | 1.33 | 1.34 | +0.6% | 5.03 | 4.95 | -1.5% |
| get_mut | 0.65 | 0.69 | +7.1% | 2.44 | 2.47 | +1.2% |
| unwrap | 15.11 | 13.64 | -9.8% | 17.89 | 17.04 | -4.7% |
| cow_shared | 15.37 | 10.79 | -29.8% | 12.65 | 13.77 | +8.8% |
| cow_weak | 26.57 | 20.39 | -23.3% | 26.63 | 24.51 | -8.0% |
| cyclic | 10.76 | 9.51 | -11.6% | 11.01 | 11.43 | +3.8% |
| cyclic_infallible | 10.92 | 9.94 | -9.0% | 11.88 | 10.59 | -10.9% |
| cow_unique | 2.81 | 0.72 | -74.5% | 2.45 | 2.48 | +1.3% |
| slice | 138.63 | 129.66 | -6.5% | 147.18 | 154.81 | +5.2% |
| slice_clone | 136.89 | 131.75 | -3.7% | 134.55 | 142.76 | +6.1% |
| slice_cow | 273.81 | 258.43 | -5.6% | 258.73 | 251.46 | -2.8% |
| slice (max 32) | 17.68 | 18.31 | +3.5% | 18.85 | 18.18 | -3.6% |
| slice_clone (max 32) | 16.90 | 17.22 | +1.9% | 16.79 | 17.07 | +1.6% |
| slice_cow (max 32) | 40.51 | 27.90 | -31.1% | 39.42 | 33.13 | -15.9% |

| Policy | Means within observed +15% | Largest observed slowdown | Extra ns in conditions above +15% |
| --- | ---: | ---: | ---: |
| Default Unmetered | 36/36 | +11.7% | — |
| Local budget | 31/36 | +20.6% | 2.11–3.46 |
| Concurrent budget | 26/36 | +40.1% | 2.44–7.02 |

The default and budget policies have the same handle width. Budgeted lifecycles
add admission/commit/refund work and a header word, which can change payload
alignment and allocator classes. Concurrent accounting requires a successful
atomic reservation before allocation and an atomic refund after deallocation.
Commit does not add another atomic operation. These updates occur per allocation
lifecycle, not per payload element; hot handle-only operations do not re-charge
the budget. Final release must refund after free, which also prevents a direct
tail call to free on metered paths. Policy rows above the observed +15% threshold
remain above it; the additional functionality is reported rather than folded
into the default-policy result.

The main std control has one large outlier: rc_cyclic's aggregate is +58.6%
versus the identical std command, driven by seed 6203 (3.657s versus 1.481s).
Its process CPU time also rose, so the journal does not identify guest-process
descheduling as the cause. The raw recording and aggregate are retained. A
separate three-seed repeat has std_control/std = 0.957 and the default product
within +15%; the [repeat aggregates](shared-rechecks.csv) preserve this result.
This control anomaly limits interpretation of that original row;
it does not represent additional library instructions.

## Memory

The separate Linux System-allocator executable records requested layouts,
`malloc_usable_size` observations, allocation calls, held storage, peak storage
and owner-handle size. All measured lifecycles finish with zero tracked live
bytes and zero budget charge. Allocation hooks are absent from the timing build.
The [allocation aggregates](shared-memory.csv) summarize 7,728 rows into 126
operation/policy groups, including six layout shapes. All 1,932 unmetered rows
match std in calls, requested/observed held/peak bytes and handle size. Raw case
rows remain local.

On x86_64 a sized strong/weak handle is eight bytes, and a dynamically sized
handle sixteen bytes, for std and every product policy. The default Unmetered-policy header has
two counters. Budget/ConcurrentBudget headers add one reference word (including
the optional Budget::Unmetered value); the retired strong word
holds the allocation charge until the last weak release. This removes the need
for a separate size word. Alignment and allocator classes determine the physical
increment:

| Payload | std/default requested | Metered requested | std/default allocator bytes | Metered allocator bytes |
| --- | ---: | ---: | ---: | ---: |
| Unit | 16 | 24 | 24 | 24 |
| u8 / u64 | 24 | 32 | 24 | 40 |
| u128 | 32 | 48 | 40 | 56 |
| 64 bytes | 80 | 88 | 88 | 88 |
| Alignment-256 zero-sized value | 256 | 256 | 264 | 264 |

Both Rc and Arc, and both budget policies, share these layout results. The
caller-owned budget object adds 24 bytes locally or 16 bytes concurrently on
this target, shared across its allocations; heap instrumentation excludes this
object and reports handle bytes separately. Budget accounting tracks requested
bytes, not allocator rounding or resident memory.
The additional policy field and padding are explicit costs; small-object memory
ratios can exceed 15%.

## Integration rechecks

The [integration aggregates](shared-integration-benchmarks.csv) contain 96 further
valid long intervals over Vec growth/clone, Box scalar creation/slice clone, and
String creation/reserved reuse. All six default-policy means remain within the
observed +15% range. The metered Box scalar lifecycle adds 2.48ns (+26.6%).

The first reserved limited-String result is +18.79ns (+17.6%). A controlled
reversion of the accounting adapter produces the same 137 normalized assembly
instructions/labels for that instantiation, including registers, branch targets
and calls. No allocation or budget operation executes on its reserved hot path.
A separate matched repeat reports default limited String at +5.2%, and 32-byte
loop alignment at −3.5%; every std/product variant receives the same selected
compiler setting. Both the first result and the [repeat](shared-rechecks.csv)
remain recorded.

## Code generation

Strong and weak admission check the live-count ceiling before changing it.
Local counters use loads/stores; atomic counters use compare-exchange. Standard
Arc clone instead increments atomically and terminates on saturation. These
extra tests and retry instructions operate on counter words, independent of
payload length. Contention can cause additional attempts.

COW first observes obvious strong/weak aliases before taking the uniqueness
lock. This removes a redundant locked operation on replacement paths while the
final uniqueness protocol still rechecks before exposing mutable data.
Explicit inlining avoids an out-of-line Result transfer on the local-budget
instantiation: the earlier caller made a function call and reloaded the result
tag and returned pointer on every unique access.

Owned cyclic construction moves a returned initialized value directly; the
separate final-storage initializer still requires recorded InitSlot completion.
Returning callback errors directly avoids an intermediate mapped Result. For
the default policy with a 64-byte payload, the earlier copy used two scalar transfers and
three 16-byte transfers, with vector stores at offsets 24, 40 and 56. The revised
copy uses four 16-byte transfers at offsets 16, 32, 48 and 64, matching std's
payload copy shape. Final strong destruction is kept out of the hot loop in a
separate drop_slow path, following std. Unmetered final release also skips the
unused atomic load of the retired byte charge.

Borrowed u64 slice cloning lowers to memcpy in both implementations. Slice COW
can retain a vectorized clone loop in jzap while std calls memcpy; the variable
length workload measures that complete lifecycle. This is distinct from the
counter-only operations. Generated assembly/LLVM output remains local; maintained
entry points are in tools/throughput-analysis/src/lib.rs.

## Reproduction

Use the pinned tools listed in the [verification index](README.md):

```sh
bash scripts/verify.sh --area shared --check api --check tests --check small-counts --check loom
bash scripts/verify.sh --area shared --profile memory
cargo rustc --release --locked --manifest-path tools/throughput-analysis/Cargo.toml --lib -- --emit=asm,llvm-ir
cargo build --release --locked --manifest-path tools/throughput-analysis/Cargo.toml --bin shared --features shared-memory
tools/throughput-analysis/target/release/shared memory > target/verification/shared/memory.jsonl
python3 tools/throughput-analysis/memory_report.py --input target/verification/shared/memory.jsonl --output target/verification/shared/memory.csv
cargo build --release --locked --manifest-path tools/throughput-analysis/Cargo.toml --bin shared
python3 tools/throughput-analysis/compare.py --binary default=tools/throughput-analysis/target/release/shared --small-tasks rc_slice,arc_slice,rc_slice_clone,arc_slice_clone,rc_slice_cow,arc_slice_cow --small-variants default --output target/verification/shared/performance
python3 tools/throughput-analysis/report.py --input target/verification/shared/performance
```

Complete build/proof jobs before timing. The benchmark prepares 64 shuffled cases
per seed, checks semantics outside the timer, then executes a fixed count between
two clock reads. Every accepted interval lasts at least one second. std,
identical std control, unmetered, local-budget and concurrent-budget variants
receive matching corpora, epochs and call counts. Three seeds are 617, 6203 and
96011. The fixed payload is 64 bytes; slice cases include zero, one, 4096 and
random logarithmic sizes, plus a separate maximum-32 profile.

Hot clone/drop, weak clone/drop, downgrade/drop, upgrade/drop, get_mut and unique
copy-on-write reuse prepared allocations. Other tasks include the stated
allocation/destruction lifecycle. Slice clone exercises the generic TryClone
constructor. Slice COW includes creation of the source and the shared replacement,
so each call copies its input twice. Cyclic construction is measured with both
a unit error and an uninhabited Infallible callback error type. Shared COW
retains another strong owner;
weak-only COW retains the old weak allocation. Reported per-call means divide
the sum of matched elapsed seconds by the sum of executed calls. Seed ranges and
an identical std control expose repeat variability. Optional global alignment
flags remain a consumer choice as documented in [build tuning](../build-tuning.md).
