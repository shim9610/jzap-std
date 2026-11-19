# Vec edit assembly optimization

The 2026-11-16 follow-up to issue 4
keeps the shared collection-fill cursor in a local variable and publishes it
when the fill call exits. This removes a length store from the first scalar
Splice fill loop. [Repeated cloud timing](#repeated-splice-timing) finds four of
twelve conditions above the issue's std +15% criterion, including large
zero-hint replacement at mixed sizes.

The comparison baseline is `44ecab7b93ebe25317db036f508e4cd372473f79`, the latest
working branch before this change. The pre-migration reference is
`fd738fb0657c06466c41b0b5aef8ca2bf876ae33`. Historical elapsed-time results remain
in [the source-migration report](vec-edit-source-performance.md).
The collection-fill runtime is committed as `49e82f6`; the later ExtractIf
candidate is verified at `f92e94f`. Verification bindings and results are tracked in
PR 5.

## Publication and cleanup

`init::collection::fill` borrows the caller's initialized length exclusively.
It copies that length into a local cursor, runs the existing admitted
initialization driver, and uses a stack guard to publish the cursor before
returning or unwinding. The driver still advances only after a successful
element write. Source exhaustion, source error and callback panic therefore
publish exactly the initialized prefix before the outer owner or Splice tail
guard performs cleanup.

The publication guard borrows the local cursor as well as the caller's output.
Keeping the cursor in a separate stack object also lets Kani abstract the loop's
modified cursor without abstracting the guard's output reference. The original
aggregate-field version failed the loop-contract proof and was replaced.
No proof checks or input ranges were removed.

This shared path also serves boxed collection buffers. The change adds no
heap allocation, unsafe block, public API, platform branch or compiler option.
The collection-fill change leaves ExtractIf and staging control unchanged;
the later ExtractIf follow-up is recorded below.

## Generated instructions and modeled cycles

Rust 1.95.0 / LLVM 22.1.2 builds the existing `u64` clone/edit/observe/drop
consumer with `-O -C panic=abort -C codegen-units=1` and the default unmetered,
unlimited Vec. The runtime replacement-shape selector is preserved. First fill
means filling the original hole; second fill follows tail movement for excess
replacement. Counts cover successful repeated iterations, excluding entry,
exit, allocation and cleanup.

| Repeated path | Baseline instructions | Current instructions | std instructions | Conditional branches, all three |
| --- | ---: | ---: | ---: | ---: |
| First scalar fill, one value | 13 | 10 | 10 | 2 |
| Second scalar fill, one value | 13 | 12 | 11 | 2 |
| Each SSE2 vector fill, four values | 13 | 13 | 13 | 1 |

The first scalar loop loses its per-element owner-length store and two
instructions used to maintain the previous endpoint expression. The second
scalar loop loses one endpoint instruction. Each still writes the initialized
length at its exit. The first scalar loop now has the same instruction count
and modeled steady-state cost as std in this fixture.

LLVM Machine Code Analyzer (`llvm-mca` 19.1.7) repeats each actual assembly path
10,000 times with known branch outcomes. Values below are simulated total
cycles divided by iterations, **not measured time or application speedups**.

| Path | Skylake baseline → current | Alder Lake baseline → current | Zen 4 baseline → current |
| --- | ---: | ---: | ---: |
| First scalar fill | 2.7505 → 2.2505 | 1.6683 → 1.2017 | 2.1672 → 1.6672 |
| Second scalar fill | 3.0005 → 2.7505 | 1.3350 → 1.3350 | 2.1672 → 2.0006 |
| First vector fill, four values | 2.7507 → 2.7509 | 2.0020 → 2.0020 | 2.1673 → 2.1673 |

Zen 3 and Zen 5 are also included in the compact [model results](vec-edit-optimization-model.csv).
The vector paths retain the same arithmetic and two stores per four values;
register allocation and endpoint control differ. Alder Lake's first vector
block has 12 modeled micro-operations instead of 11 and a block-throughput
estimate of 2.0 instead of 1.8, although its simulated cycle count is unchanged.
The consumer grows from 1,772 to 1,777 bytes; std is 2,379 bytes in this fixture.

The pre-migration reference's first scalar fill has 13 instructions and three
conditional branches. Its modeled Skylake/Alder Lake/Zen 4 costs are
3.0004/1.5017/2.2505 cycles per value. Its first vector fill retains 13
instructions and approximately 2.7509/2.0020/2.1673 cycles per four values.
That older consumer is 1,562 bytes; the remaining growth since that reference
is outside the five-byte increase introduced here. Only the first fill is
modeled for the older consumer because its excess path has a different loop
structure.

The model assumes overlapping out-of-order execution and optimistic L1 data
access. It does not predict branch misses, instruction-cache placement,
allocation time or host scheduling. Instruction latencies cannot be summed to
obtain elapsed time. Instruction information: [LLVM MCA documentation](https://llvm.org/docs/CommandGuide/llvm-mca.html)
and [uops.info instruction tables](https://uops.info/table.html).

## ExtractIf candidate

A first-selected fast path with counted pairs in the retained scans reduced the
dynamic-selector all-selected path from 11 to 10 instructions. Its modeled
cycles changed from 2.2505 to 2.0007 on Skylake and from 1.8340 to 1.6675 on Zen 4,
but increased from 1.5018 to 2.0018 on Alder Lake. It also enlarged the wrapper.
The candidate was rejected based on the static comparison, without elapsed-time
confirmation. The Splice-only checkpoint retained the two-phase ExtractIf
implementation and its proof domains. That checkpoint's consumer retains the
baseline's 776-byte ExtractIf wrapper
and 11-instruction, three-conditional-branch repeated all-selected path; std's
wrapper is 539 bytes and its corresponding path is five instructions.

A later [selected-first experiment](vec-extract-selected-first.md) reaches the
five-instruction path under default settings and passes focused safety checks.
It also adds instructions to retained paths and remains a timing candidate.
The Draft PR reapplies it at `f92e94f`, and its fresh global verification now
passes with no model reuse, open source obligations or unproved link edges.
The linked report records that follow-up; the results below describe the
earlier Splice checkpoint.
The later [repeated cloud measurements](vec-extract-repeated-timing.md) confirm
all-selected improvements and retained-prefix regressions, so this ExtractIf
candidate is not accepted as a general default optimization.

The subsequent [deletion-count experiment](vec-extract-deleted-count.md) tests
std's state representation as well as its decision order. It retains only the
read-index update on the prefix path. Focused proofs and memory/compiler
checks pass. Twelve-process timing reduces retained-prefix time 29.9–34.7%
versus the read/write candidate but increases mixed all-selected time 48.5%
(std +47.4%). It remains an experiment; no fresh global certificate has run
for that changed representation.

## Verification status

The focused direct check passes the original initialization driver in both
Verus word domains and three actual-Rust Kani harnesses: arbitrary-word cursor
publication, every admitted native-word fill prefix, and every admitted Splice
hole/tail coordinate with arbitrary source success/end/error. A new source
mutation omits cursor publication and is rejected by a solver counterexample;
the existing fill-end and error controls remain enabled.

All 433 native/doc test cases pass in debug, release and overflow-checked
release. The 41 Vec, 13 fallible-iterator and 31 Box scenarios each pass Miri with native strict
provenance, i686 strict provenance and Tree Borrows, plus AddressSanitizer
with leak detection disabled as described below. Whole-library lint, rustdoc,
format, verification-runner tests, unsafe routing, core-only, wasm32, thumb and
no-global-OOM builds pass. Receipts record unchanged source inputs.
The Vec libFuzzer target completes 10,000 runs, including its std differential
Splice operations. Compiler ownership/thread boundary checks pass. LLVM coverage
for the Vec and Box scenarios executes all five functions and 32 of 33 lines in
`init/collection.rs`; this is executed-path coverage, not an all-input proof.
With `panic=abort`, four selected Splice tests and all 13 fallible-iterator
tests pass in isolated test processes, including allocation/limit refusal and
source errors at every insertion stage with exact and unknown hints.

The final fresh global campaign passes without model or prerequisite reuse:
20 direct foundation/Vec groups with 55 source mutations, 30 standalone Verus
targets with 120 source controls, and the shared/String/collection, owner,
UTF-8 and arithmetic-policy replays. Both independent glue audits pin the
local-cursor routing and `Commit::drop`, with a compiled publication-omission
control. The exhaustive owner audit binds
84 state writes, 130 constructor fields and 178 unsafe callers; publication and
the guard's two fields use the actual all-word publication proof. The wrapping
memory suite retains its previous harnesses and adds this proof, for 19 actual
Rust/state harnesses.

The final source/link certificate is complete with unchanged source inputs,
zero open source obligations and zero unproved link edges across release,
release with link-time optimization, overflow-checked release and debug.
All eight intentional panic/bounds/allocation-failure/abort controls are
detected. The certificate attaches 89 originating guard calls and 12 forwarding
calls to source contracts; it does not require every guard to disappear from
the binary. Direct Rust/Verus kernels and reviewed source projections remain
distinct evidence. The scope is numeric overflow-to-memory and internal guard
unreachability under the recorded source contracts in Linux x86_64 fixtures;
allocator, public unsafe and returning-callback contracts remain explicit.
Alias, initialization and drop scenarios are checked separately by the memory
and compiler suites above.

The isolated Linux allocation observer executes six replacement shapes at lengths
0, 1, 2, 3, 8 and 32: 108 old/new/std observations. Old and current allocation
calls, held/peak requested and usable bytes, and handle sizes match in all
36 paired observations. Every observation ends with zero held bytes. Some
excess-replacement shapes have different staging allocations from std; those
differences already exist in the baseline.

## Repeated Splice timing

The Linux x86-64 run compares the current product at `98b8b71`, the pre-fill
baseline `44ecab7` and std using the same unmetered, unlimited `u64` workload.
Each call clones the prepared input, consumes the removed items, finishes the
replacement, observes the result, and drops the owner. The six replacement
shapes each use lengths 0–32 and 256–4096. The runner uses Rust 1.95.0,
LLVM 22.1.2, `-O -C panic=abort -C codegen-units=1` and default alignment.
It pins the process to logical CPU 0 on the cloud host reporting AMD EPYC 9V45.

Three seeds and four rounds yielded 144 matched groups and 576 valid intervals
of 1.322–4.401 seconds. These total 1,435.736 seconds and 23,300,324,608 calls.
All twelve identical-code controls passed their 0.95–1.05 gate. Calibration is
excluded. The displayed means divide total wall time by total calls. The
+15% verdict uses Bonferroni-adjusted bounds from 50,000 paired bootstrap
resamples stratified by seed. Raw observations stay under
`target/verification/splice-timing/`. The [compact timing table](vec-splice-repeated-timing.csv)
includes wall and thread CPU time, pointwise and adjusted bounds, and A/A
results.

| Replacement | Input | std ns | Current ns | vs std | Adjusted change interval | +15% gate |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Delete | Small | 16.799 | 21.962 | +30.7% | +24.9% to +35.1% | Above |
| Delete | Mixed | 555.329 | 533.733 | −3.9% | −7.2% to −0.8% | Within |
| Shorter | Small | 17.715 | 22.770 | +28.5% | +25.6% to +31.6% | Above |
| Shorter | Mixed | 676.119 | 639.830 | −5.4% | −9.7% to −1.1% | Within |
| Exact | Small | 17.737 | 22.787 | +28.5% | +24.5% to +32.1% | Above |
| Exact | Mixed | 702.556 | 677.236 | −3.6% | −8.0% to +0.3% | Within |
| Longer, known hint | Small | 51.934 | 57.370 | +10.5% | +7.0% to +14.3% | Within |
| Longer, known hint | Mixed | 811.346 | 789.804 | −2.7% | −6.4% to +0.7% | Within |
| Longer, zero lower hint | Small | 86.371 | 60.122 | −30.4% | −32.6% to −28.0% | Within |
| Longer, zero lower hint | Mixed | 1,474.840 | 862.975 | −41.5% | −45.2% to −38.5% | Within |
| Large excess, zero lower hint | Small | 207.815 | 214.507 | +3.2% | −0.2% to +6.6% | Within |
| Large excess, zero lower hint | Mixed | 11,961.144 | 17,007.926 | +42.2% | +36.2% to +48.2% | Above |

The three small-operation gaps are about 5 ns each. Mixed large excess costs
5.047 µs more than std. Its change from the pre-fill baseline is +0.19%
(adjusted −4.52% to +5.20%). The small shorter replacement improves by 3.52%
from baseline (adjusted −7.18% to −0.95%). The other eleven baseline comparisons
are unresolved under the adjusted bounds.

Both product wrappers in the timed assembly call the same `stage_excess`
specialization. Its repeat path checks returned capacity against the next slot
and limit and branches on the replacement iterator's progress. The std staging
loop checks capacity for growth and advances its iterator. The isolated
allocation observer confirms that old and current match in all 36 input-shape
pairs. At mixed-workload size 4,096, the large-excess case makes 16 allocation
calls versus std's 15 and both peak at 278,536 requested bytes. These are
observed path differences. Their individual contributions to the elapsed gap
remain unmeasured.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/measure_vec_edit.py --benchmark splice-fill \
  --baseline 44ecab7 --output target/verification/splice-replay
```

## Reproduction

All generated assembly, binaries, proof output and raw observations belong in
ignored `target/verification/` directories. Run from the repository with the
pinned toolchains activated. No timing executable is run by the assembly tool.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/analyze_vec_edit.py \
  --baseline 44ecab7b93ebe25317db036f508e4cd372473f79 \
  --mca /path/to/llvm-mca-19 \
  --output target/verification/vec-edit-assembly
python3 tools/throughput-analysis/analyze_vec_edit.py \
  --baseline fd738fb0657c06466c41b0b5aef8ca2bf876ae33 \
  --first-fill-only \
  --mca /path/to/llvm-mca-19 \
  --output target/verification/vec-edit-assembly-pre
python3 tools/throughput-analysis/prepare_range.py \
  --benchmark splice-fill --baseline 44ecab7 --profile default --memory-only \
  --output target/verification/vec-edit-memory
python3 scripts/verify_source.py --group vec-splice-fill --mutations \
  --output target/verification/vec-edit-source
bash scripts/verify.sh --area all --check tests --check release --check overflow \
  --check clippy --check builds --check docs --check format --check runner --check unsafe
ASAN_OPTIONS=detect_leaks=0 bash scripts/verify.sh --area vec --check fuzz
bash scripts/verify.sh --area all --test-target vec --test-target box_complete \
  --check coverage --check bounds
XDG_CACHE_HOME=/workspace/.cache ASAN_OPTIONS=detect_leaks=0 \
  bash scripts/verify.sh --area vec --test-target vec --test-target iteration --profile memory
XDG_CACHE_HOME=/workspace/.cache ASAN_OPTIONS=detect_leaks=0 \
  bash scripts/verify.sh --area box --test-target box_complete --profile memory
RUSTFLAGS='-C panic=abort -Zpanic_abort_tests' \
  cargo +nightly-2026-10-30 test --locked --offline --all-features \
  -Z panic-abort-tests --test vec splice -- --test-threads=1
RUSTFLAGS='-C panic=abort -Zpanic_abort_tests' \
  cargo +nightly-2026-10-30 test --locked --offline --all-features \
  -Z panic-abort-tests --test iteration
python3 scripts/source_certificate.py \
  --output target/verification/vec-edit-certificate
```

That run could not execute LeakSanitizer inside the restricted execution-tool
sandbox. `detect_leaks=0` retained AddressSanitizer's memory-access checks;
Miri and explicit allocation observations checked cleanup separately. The
[later LeakSanitizer follow-up](vec-extract-leaks.md) succeeds outside the
sandbox on the current runtime, without changing system ptrace settings. It
does not retroactively change this historical run's scope. Platforms outside
the executed targets require their own runtime validation.
