# Owning memory budget verification

`OwnedBudget` and `OwnedConcurrentBudget` resolve stored budget lifetime
propagation using the existing fallible Rc/Arc implementation. The product
implementation is checkpoint `660c448`; the guide and expanded shared fuzzer
are recorded at `8a9c883`. The [budget guide](../budget.md) defines usage,
bootstrap accounting and scoped borrowing.

## Executed checks

| Check | Result |
| --- | --- |
| Normal and overflow-checked release | 253 cases each: 227 runtime tests and 26 doctests |
| Real owning-ledger lifetime | 10 scenarios each under native tests, x86_64 Miri, i686 Miri and strict Tree Borrows |
| AddressSanitizer | All 227 runtime cases pass |
| Thread and lifetime boundaries | 108 compiler cases, including 39 positive cases; 12 weakened-bound controls detected |
| Actual-source Loom | 12 scenarios pass; the two new threaded owning-policy scenarios use preemption bound two |
| Shared source obligations | 154 bindings pass, including 18 added query/delegation bodies; copied-domain and wrong-ledger mutations rejected |
| Combined source-to-link certificate | 864 obligations; zero open obligations and zero unproved linked edges |
| Final links | Release, LTO release, overflow-checked release and debug; 62 originating guards and five forwarders closed; eight exit controls detected |
| LLVM coverage of `src/budget/owned.rs` | 46/46 emitted lines, 18/18 functions and 50/52 regions |
| Shared state-machine fuzzing | 10,000 runs; maximum input length 512, exercising borrowed and owning policies |
| Build boundaries | Core-only/all-features Clippy, rustdoc with warnings denied, wasm32/thumbv7em and no-global-OOM builds pass |

The combined input SHA-256 is
`5c2cabf887db443fe15d14f862728d7412755d53f5d1ca23abf8b2cc29ef8665`.
The source was unchanged during collection. Its local receipt is
`target/verification/owned-budgets/certificate/summary.json`.
Tools were Rust 1.95.0, nightly 2026-10-30, Kani 0.68.0 and the repository's
pinned Verus installation.

The new policy module contains no unsafe blocks. The product census remains
415 unsafe blocks, 85 ordinary and two macro unsafe signatures, and 28 unsafe
implementations. Its owned fields are private; actual construction, duplication
and final destruction delegate to the previously checked shared-owner kernels.
The source checks bind these delegates and reject domain substitution. They do
not turn a text-to-model projection into a direct proof of arbitrary Rust code.

## Failure, lifetime and concurrency paths

`tests/owned_budget.rs` keeps real heap ledgers alive after their creator is
dropped. It covers Box, Vec, String and BTreeMap; clone/split and raw transfers;
Box-to-shared conversion; shared and Weak-detaching COW; final Weak release;
failed initialization and cyclic initialization with an escaped Weak; reservation
cancel/commit/refund; zero-byte budgets; and bootstrap allocation refusal.
Allocator observations check final release and charge refunds before the last
ledger owner is destroyed.

A concrete application state containing `Vec<u8, OwnedConcurrentBudget>` crosses
a thread boundary and satisfies `Send + Sync + 'static`. An owning future also
satisfies `Send + 'static`. Local owning policy thread sharing is rejected.
Borrowed policy APIs keep their explicit lifetime contracts.

The added Loom paths check actual owning-policy count saturation and COW,
competing Vec admissions after the creator is dropped, and Weak upgrade against
final release when the shared header owns the last policy. Count saturation
uses a reduced test maximum; threaded paths explore the stated bounded schedules.

Numeric and internal-guard checks retain the established native/32-bit models,
actual Rust Kani witnesses, source correspondence checks and deliberate defects.
Final-link roots now exercise owning policies and allocation-ordinal refusal.
Retained panic edges require their source/caller proofs; the intermediate link
check alone intentionally reports those edges. The combined result closes them.
Typed pointer provenance, initialization, aliasing and destruction use the
compiler, Miri and sanitizer division of verification work. Valid allocator,
public unsafe and returning user-callback contracts remain explicit.

## Memory accounting

| Requested storage on the checked 64-bit target | Local | Concurrent |
| --- | ---: | ---: |
| Owning policy handle | 8 bytes | 8 bytes |
| Bootstrap ledger/control allocation, once per domain | 40 bytes | 32 bytes |
| Sized Box handle with owning policy | 16 bytes | 16 bytes |

Bootstrap storage is allocated with `Unmetered`, outside the payload domain's
requested-byte quota. It is not hidden payload usage: an owning zero-byte budget
still needs this allocation and can fail with `Allocation`. Policy duplication
does not allocate another ledger. The size regression also passes on i686,
where policy handles are four bytes and bootstrap requests are 20/16 bytes.
Allocator usable sizes and RSS are separate quantities.

Reallocation retains old-plus-new admission. The separate
[review and moving-allocator regression](../budget-review.md) explain why delta
admission cannot bound all permitted allocators' temporary requested storage.

## Reproduction

```sh
cargo test --locked --all-features
cargo test --locked --all-features --release --config 'profile.release.overflow-checks=true'
MIRIFLAGS='-Zmiri-strict-provenance' cargo +nightly-2026-10-30 miri test --locked --all-features --test owned_budget
MIRIFLAGS='-Zmiri-tree-borrows -Zmiri-strict-provenance' cargo +nightly-2026-10-30 miri test --locked --all-features --test owned_budget
MIRIFLAGS='-Zmiri-strict-provenance' cargo +nightly-2026-10-30 miri test --locked --all-features --target i686-unknown-linux-gnu --test owned_budget
python3 scripts/verify_thread_bounds.py --mutations
python3 scripts/verify_shared.py --kani --mutations --loom
python3 scripts/verify_panic_paths.py --output target/verification/owned-budgets/certificate
```

Generated models, binaries, mutation sources, raw timing records and logs remain
under ignored `target/verification/`. Git retains maintained inputs and compact
result summaries.

## Paired performance renewal

These API measurements use one thread. The subsequent
[application-workflow evaluation](arc-scenarios.md) covers 19 conditions and
475 long intervals with up to four workers. It finds substantial overhead for
short contended handle operations and high-rate allocation through one budget,
while larger shared-state reads and page COW have smaller costs. The earlier
near-equal isolated handle clone results do not cover those contended paths.

The eight tasks contain **240 valid intervals** and [64 aggregate rows](owned-budget-benchmarks.csv). Every selected interval is at least **1.007 seconds**. Three seeds (617, 6203, 96011) use equal inputs, epochs and operation counts across borrowed/owning binaries and all five policy/control labels. The driver pins CPU 0. Compilation, proofs and fuzzing finished before timing. Each mean is summed valid elapsed time divided by summed calls.

The maintained shared-owner executable selects owning policies with the `owned-budgets` feature. Both binaries prepare borrowed and owning domains outside the timed interval. Domain bootstrap construction is excluded; new payload allocation, COW duplication and final payload release are included for the stated lifecycle tasks. Hot handle clones reuse prepared owners. Value payloads are 64 bytes; slice inputs include zero/one, logarithmic random lengths and 4,096 elements.

All times below are ns/call. The std column is the owning binary's matched std control. Borrowed figures come from the paired borrowed binary; its separate std measurements, totals, call counts and seed ranges remain in the CSV.

| Operation | std ns/call | Borrowed local | Owning local | Borrowed concurrent | Owning concurrent | Owning concurrent vs std |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rc_construct | 10.488 | 12.644 | 12.608 | 12.983 | 13.824 | +31.8% |
| arc_construct | 11.521 | 11.424 | 13.828 | 12.494 | 16.186 | +40.5% |
| rc_clone | 0.927 | 0.959 | 0.916 | 1.002 | 0.927 | +0.1% |
| arc_clone | 5.009 | 4.963 | 4.967 | 4.939 | 5.050 | +0.8% |
| rc_cow_shared | 16.677 | 12.340 | 12.443 | 12.789 | 14.542 | -12.8% |
| arc_cow_shared | 12.956 | 14.412 | 15.432 | 16.958 | 21.445 | +65.5% |
| rc_slice | 131.309 | 133.487 | 139.062 | 137.590 | 137.230 | +4.5% |
| arc_slice | 131.924 | 138.396 | 144.647 | 140.705 | 143.623 | +8.9% |

Unmetered means range from −33.5% to +8.9% against their matched std, within the observed +15% range. Optional budgeted modes have misses: owning local Rc/Arc construction is approximately +20%, and owning concurrent construction/COW reaches +65.5%. These results do not establish a universal +15% result for budgeted ownership. Repeated-std controls range from −7.4% to +2.7%; seed ranges include both workload and execution variation. No confidence-interval acceptance or hardware-counter diagnosis is inferred.

### Identified additional work

Borrowed policy duplication returns a ledger pointer. Owning local duplication loads the Rc count, checks its limit, increments it and stores it. Owning concurrent duplication checks the Arc count and uses `lock cmpxchg` with a retry edge. In an optimized typed wrapper, it includes `cmpq`, `leaq 1(...)` and `lock cmpxchgq`; a saturated count returns `RefCount`. Owning release uses `decq` locally or `lock decq` concurrently, with final-ledger destruction only at zero. These are distinct from the budget reservation CAS and atomic charge refund already required by concurrent accounting.

The actual timed binary also shows the order at final payload release: free the payload allocation, `lock sub` the charge from the ledger, then `lock decq` its owner count. The retained instruction sequence refunds storage before the last owning policy can destroy its ledger.

For owning concurrent Arc construction/destruction the measured difference from std is **4.665 ns/call**; the difference from the borrowed concurrent fixture is **3.692 ns/call**. Shared Arc COW differs from std by **8.489 ns/call**, including the borrowed mode's existing accounting/COW cost; the difference from the borrowed concurrent fixture is **4.487 ns/call**. These paired differences include code-layout and control variation and are not isolated instruction-cycle measurements.

Each new policy owner requires a count credit. Construction and shared COW acquire one for the new allocation and release it with that allocation. Buffer growth retains its policy; cloning a shared handle increments the object count and reuses its header policy. Policy-owner operations are outside payload copy loops, so their count follows allocation/owner lifecycles rather than the number of copied elements. The hot-handle measurements are consistent with that source distinction: owning concurrent Rc clone is 0.927 ns versus std 0.927 ns, and Arc clone is 5.050 ns versus std 5.009 ns. A scoped borrowed policy remains available when the owner already outlives the operation.

Build both executables first, then run without other build/solver work:

```sh
cargo build --locked --release --manifest-path tools/throughput-analysis/Cargo.toml --bin shared --target-dir target/verification/owned-budgets/borrowed
cargo build --locked --release --manifest-path tools/throughput-analysis/Cargo.toml --bin shared --features owned-budgets --target-dir target/verification/owned-budgets/owned
python3 tools/throughput-analysis/compare.py --binary borrowed="$PWD/target/verification/owned-budgets/borrowed/release/shared" --binary owned="$PWD/target/verification/owned-budgets/owned/release/shared" --tasks rc_construct,arc_construct,rc_clone,arc_clone,rc_cow_shared,arc_cow_shared,rc_slice,arc_slice --small-tasks '' --small-variants '' --output target/verification/owned-budgets/performance
python3 tools/throughput-analysis/report.py --input target/verification/owned-budgets/performance
```
