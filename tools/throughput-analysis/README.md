# Long-interval collection comparison

This analysis-only workspace compares std and jzap using homogeneous randomized
workloads. Every accepted recording lasts at least one second. Keep the total
elapsed seconds and call counts, then report **means over repeated long intervals**.
The operation-weighted mean is `sum(elapsed seconds) / sum(executed calls)`.
Per-operation ns or microseconds are derived afterward, without timing one call.
It has no external reference checkout or additional runtime dependency.

`measure_vec_edit.py` adds fixed-count, paired ExtractIf process repetitions,
a same-function A/A control, per-interval thread CPU time and stratified paired
bootstrap intervals. It reuses the existing extraction consumers and does not
change product code. Its thread-clock diagnostics require 64-bit Linux.
The [repeated timing report](../../docs/verification/vec-extract-repeated-timing.md)
records its protocol and reproduction commands. Build with `--build-only`, then
run the frozen executable with `--run-only` after other build/proof jobs finish.

The `deque` executable uses the same protocol for VecDeque. Build with
`--bin deque`; add `--features btree-memory` only for the allocation observer.
The [VecDeque guide](../../docs/vec-deque-design.md) describes its 18 workload
types, element widths, selective reproduction and published results.
`deque-borrowed-consume` changes both implementations' final scan to borrowed
iteration; `deque-outline-action` gives both action functions an explicit call
boundary for assembly inspection. Default timing uses neither diagnostic feature.

The `hash_set` executable (`--features hash-set --bin hash_set`) uses this
protocol for set operations. Its operation units, input cases, allocation
observer and selective commands are in the [HashSet guide](../../docs/hash-set-design.md).
`prepare_disjoint.py` prepares the guide's caller-context and fixed-size
diagnostics from a pinned Git revision without modifying production code.
Use an explicit empty `--tasks ''` to disable mixed-size cases when repeating
only a small-input condition.

`prepare_heap_layout.py` prepares Heap placement diagnostics from a pinned Git
archive. Its fixed-instruction kernel retains both observed SSE2 register forms,
two copies of each, and loop offsets 0/16/32/48 within a 64-byte line. It verifies
the 30-byte, seven-instruction loops before timing. An entry jump skips padding;
all kernels share data addresses, an indirect call boundary, and the timing driver
within each process. Fixed-instruction kernels and collection workloads use
separate executables.

The same preparer builds the unchanged Heap benchmark with eight reproducible
link orders and isolated caller-inline, caller-outline, sift-outline, push-inline
and growth-coldness variants. Caller attributes apply equally to std and jzap.
Section shuffling retains block order within each function. Method references:
[Mytkowicz et al., ASPLOS 2009](https://www2.ccs.neu.edu/racket/Performance/mytkowicz.pdf)
and [Curtsinger and Berger, ASPLOS 2013](https://people.cs.umass.edu/~emery/pubs/stabilizer-asplos13.pdf).
Layouts are fixed per executable.

```sh
python3 tools/throughput-analysis/prepare_heap_layout.py prepare
python3 tools/throughput-analysis/prepare_heap_layout.py inspect
python3 tools/throughput-analysis/prepare_heap_layout.py kernel
# Select forms, linker seeds, data sizes and input seeds with --help.
# Use compare.py for the prepared real-collection binaries as documented below.
```

Builds finish before measurements. Kernel records use equal counts, shared input,
shuffled kernel order and intervals of at least one second, with resumable CSV
groups under ignored target storage. Preparation writes only to target/ storage.
Results are in the Heap guide. Resume requires an identical binary, driver and
parameter set; changes require a fresh `--output` directory.
The optional `inspect` pass compares function instruction forms after masking
relocated addresses and legacy Rust symbol hashes. It retains internal branch
offsets, registers and function sizes. RIP-relative data references and external
call specializations are masked.

For insertion isolation, `prepare-reserved` builds the actual pinned Heap APIs
with reused buffers. The `reserved` mode measures clear/refill and root reads,
excluding allocation, storage conversion, destruction and a final full scan.
Safe ownership conversions transfer each output allocation between implementations
outside timing; pointer/capacity identity is asserted after every interval. Input
addresses are also shared, and jzap's complete output is checked against repeated
std pushes. The three shuffled intervals include two identical std controls.

```sh
python3 tools/throughput-analysis/prepare_heap_layout.py prepare-reserved
python3 tools/throughput-analysis/prepare_heap_layout.py reserved --sizes 0,16,256,4096
```

`prepare-mixed`/`mixed` run the same insertion probe with the pinned lifecycle
benchmark's original mixed corpus and case order. Only preparation and metadata
change; both fill functions and the timed body are retained. Its corpus digest
and element/call counts can be checked against `binary_heap_push_reserved_8`.
The recorded size -1 denotes that mixed corpus rather than a fixed length.

The [recorded campaign](../../docs/verification/throughput-long.md) contains the
reviewed totals and analysis; complete generated policy/flag tables are in the
local `target/verification/throughput-long/summary.csv` output.

## Snapshot Vec timing

From a clean Git checkout, build with Rust 1.95.0 on 64-bit Linux. On the
first snapshot, `HEAD` establishes a same-source baseline and the harness
measures against std. Use a dedicated machine for timing, after builds and
proof jobs finish. Each benchmark writes a separate receipt and CSV under
`target/verification/`. Use a fresh output path for each build.

```sh
python3 tools/throughput-analysis/measure_vec_edit.py --benchmark extraction \
  --baseline HEAD --output target/verification/extractif-snapshot --build-only
python3 tools/throughput-analysis/measure_vec_edit.py --benchmark splice-fill \
  --baseline HEAD --output target/verification/splice-snapshot --build-only
# Run on the timing machine after compilation and verification finish.
python3 tools/throughput-analysis/measure_vec_edit.py --benchmark extraction \
  --baseline HEAD --output target/verification/extractif-snapshot --run-only
python3 tools/throughput-analysis/measure_vec_edit.py --benchmark splice-fill \
  --baseline HEAD --output target/verification/splice-snapshot --run-only
```

For subsequent changes, pass the accepted commit from this repository in
place of `HEAD` and use a fresh output path. In `summary.csv`, use the
wall-time rows with stable controls. `new/old` compares with the accepted
implementation. A `higher` verdict for this comparison identifies a measured
regression. The `new/std` comparison reports the std +15% margin. A drift
verdict or an interval crossing the threshold is inconclusive. Retain
receipts from a completed run for comparisons. The [Splice report](../../docs/verification/vec-splice-batch.md#repeated-timing)
records the existing small-input gaps.

## Protocol

- Each seed prepares 64 cases and a shuffled call order before timing. The same
  corpus digest, operation count and epoch count must match across all policies
  and compiler settings. A corpus is replayed in that order during each epoch.
- Mixed Vec lengths cover 0–131,072 u64 elements; dynamic Box payloads cover
  0–1,048,576 bytes. The generator samples a capped binary exponent and a size
  inside that bin, with explicit empty, one-element and maximum cases. Scalar
  Box inputs are randomized u64 values. Strings contain randomized ASCII.
- Values, valid edit ranges, insertion/split indices, resize lengths and splice
  replacement lengths/values vary. Retain/extract thresholds vary; dedup inputs
  contain randomized runs. `extract_none` and `extract_all` retain their stated
  selection behavior while their values, lengths and ranges vary.
- The focused small profile uses sizes 0–64 for clone, full extraction and pop.
  This keeps small-operation costs visible alongside mixed-size results.
- Calibration selects one common epoch count for every compared executable and
  policy in a task/seed group, targeting at least 1.2 seconds for the fastest.
  If any actual recording is shorter than one second, the entire group repeats
  with a larger common count. All short/incomplete attempts remain in the journal.
- Each process prepares sources, runs two warmup epochs, then reads `Instant`
  once before and once after all measured epochs. There is no RNG, clock read
  or time-based stopping condition inside the operation loop. Successful timing
  processes also check retained sources and budget cleanup outside the timer.
- Runs are sequential on one CPU, with rotating/reversed implementation order
  and rotating compiler-setting order. `std_control` executes the identical std
  command in another process. Builds and verification finish before timing.
- Default seeds are 617, 6203 and 96011. Totals sum matched seed recordings;
  mean recording time divides this sum by the repeat count. Mean time per
  operation divides it by the total call count. Because seeds may have different
  counts, the primary per-operation mean weights each executed call equally.
  The reporter also preserves each seed's normalized mean, arithmetic mean,
  sample standard deviation and ratio range. These three-seed observations
  supplement the existing bootstrap acceptance tests. A point ratio below 1.15
  is not relabeled as passing that separate confidence-interval gate.

The synthetic size distribution is a reproducible workload definition, not an
estimate of application call frequencies. Different operations are reported
separately rather than mixed into a global Vec or Box score.

## Timed work

The 20 Vec tasks correspond to the existing analysis driver. Grow/copy build
and destroy a fresh owner. Reserved push clears and refills reserved sources.
Clone includes destruction; edits include source cloning, editing and destruction.
Pop consumes all values. Extract tasks examine a random valid subrange; full
extraction removes every value in that subrange. Splice includes replacement
completion. Limit policies use sufficient headroom for the same successful work.

Box tasks cover scalar, byte-slice and string creation/clone/destruction, equal-size
`clone_from`, zeroed creation/destruction and exact-length byte collection/destruction.
`clone_from` reuses prepared destination owners. Isolated free-only latency is
not a new claim of this lifecycle measurement.

Input construction, source preparation, warmup, corpus verification and final
source teardown are outside the recorded interval. Result consumption and each
temporary owner's destruction are inside it. No allocation hooks are installed
in this timing binary. Allocation results remain those of the separate recorded
memory matrix; these randomized timing cases do not add memory measurements.

## Build, verify and run

From the repository root, using Rust 1.95:

```sh
cargo test --locked --manifest-path tools/throughput-analysis/Cargo.toml
python3 -m unittest discover -s tools/throughput-analysis -p 'test_*.py'
cargo clippy --locked --manifest-path tools/throughput-analysis/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path tools/throughput-analysis/Cargo.toml --release
cargo --config .cargo/loop-align32.toml build --locked --manifest-path tools/throughput-analysis/Cargo.toml --release --target-dir target/verification/throughput-build-align32
cargo --config .cargo/loop-align64.toml build --locked --manifest-path tools/throughput-analysis/Cargo.toml --release --target-dir target/verification/throughput-build-align64
python3 tools/throughput-analysis/compare.py \
  --binary default=tools/throughput-analysis/target/release/jzap-throughput-analysis \
  --binary align32=target/verification/throughput-build-align32/release/jzap-throughput-analysis \
  --binary align64=target/verification/throughput-build-align64/release/jzap-throughput-analysis \
  --output target/verification/throughput-long
python3 tools/throughput-analysis/report.py --input target/verification/throughput-long
```

Both implementations reside in each executable and receive the same flags.
The default campaign contains 1,584 valid recordings across 102 seed groups,
plus calibration and any repeated short attempts. It therefore requires at least
26.4 minutes of measured work. Repeating the exact command resumes completed
groups only when the frozen source, executable and protocol fingerprints match.

`protocol.json` fixes the inputs, binaries and conditions before measurement.
`records.jsonl` preserves every calibration and timed attempt; `completed.json`
selects fully validated attempts. `summary.json` contains seconds, total calls,
ratios and seed ranges. The standard report step revalidates complete equal-work
groups and the one-second minimum, then writes `per-operation.json`,
`per-operation.csv` and `per-operation.md`. It preserves outliers and does not
modify the frozen sampler, protocol, journal or selected attempts. The JSON
includes the source protocol/journal hashes and the reporting code hash.
These generated files belong under ignored `target/`.

Rust tests compare randomized edit contents and removed-item order with std,
check Box values/initialization and verify replayable boundary inputs. Python
negative controls reject unequal corpora/work, duplicate/missing records and
subsecond intervals, and check summation of equal-work totals.
Reporter tests additionally cover unequal-count weighting, repeat means,
outlier retention and exclusion of unselected short/incomplete attempts.

### Controlled stack layout

`prepare_stack_layout.py --binary PATH --output target/verification/layout`
creates two launchers for the same immutable executable. On Linux they disable
ASLR in children with `setarch -R` and shift an unused environment string by
0/16 bytes. Each launcher verifies the executable's SHA-256 before running it.
Pass both launchers to `compare.py` as separate `--binary NAME=PATH` arguments;
`--tasks` and `--seeds` retain their usual selective behavior. The product's
alignment and compiler flags are unchanged. See the [VecDeque report](../../docs/vec-deque-design.md)
for the copy-layout experiment and its separate C-kernel reproduction command.

## Range-normalization source migration

`prepare_range.py` compares the actual private Vec range normalizer from a pinned
Git revision and the current product with Rust 1.95 std. It adds a call-only
visibility shim to disposable source copies. `--build-only` inspects preparation;
`--profile default|aligned|loops32|both` selects the consumer alignment option.
`aligned` requests 64-byte function alignment; `loops32` requests 32-byte loop
alignment. `both` selects only `default` and `aligned`.
All binaries build before timing. Each seed and condition uses equal calls,
pre-generated descriptors and rotated implementation order. `JZAP_RANGE_BATCHES`
changes the common batch count; intervals shorter than one second fail. Results,
compiler identity, source fingerprints and binary hashes remain under `target/`.
The [migration report](../../docs/direct-source-migration.md#vec-range-normalization-follow-up)
lists the conditions and totals. This is a private scalar-helper diagnostic, not
a whole-collection workload or a proof input.

`prepare_range.py --benchmark flatten --profile default` selects the public ZST
array-owner diagnostic at widths 3 and usize::MAX, with unlimited/limited policies.
It uses the same equal-call protocol and stable Rust for every binary; no
visibility shim or unstable std API is needed. The recorded run sets
`JZAP_RANGE_BATCHES=250000`. Details and compact totals are in the
[array-flattening follow-up](../../docs/direct-source-migration.md#vec-array-flattening-follow-up).


## BTree source-migration comparison

`compare_btree_codegen.py` compiles nine actual public/private BTree consumers
against a pinned revision and the current source in release and overflow-checked
release. The two private visibility adapters call the original planner. Its
report pairs every reached local relocation target, retains external targets,
and lists instruction multisets and reordered sections.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](../../docs/verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/compare_btree_codegen.py --baseline bcf8c7a --output target/btree-codegen
python3 tools/throughput-analysis/prepare_btree.py --baseline bcf8c7a --output target/btree-timing --build-only
# Run after other compilation, proof and memory-check processes finish.
python3 tools/throughput-analysis/prepare_btree.py --baseline bcf8c7a --output target/btree-timing --run-only
```

The timing driver uses three independent input seeds and identical corpus/work
counts across std, previous and current implementations. Excluded pilot runs
select work targeting two seconds for the fastest implementation; every reported
interval must be at least one second. `--operation`, `--size` and `--seed` select
subsets. Builds, source archives, disassembly and raw metadata stay in `target/`.

`diagnose_btree_timing.py` reuses the exact recorded binaries. It randomizes
implementation order across repeated processes and records child CPU time,
context switches and page faults alongside each timed interval. `--cpu` applies
the same allowed CPU to all implementations. `--code-only` compares linked
function instruction rows, including addresses and bytes, without timing or
recompilation. Output requires a fresh directory under `target/`.

```sh
python3 tools/throughput-analysis/diagnose_btree_timing.py --binaries target/btree-timing --output target/btree-repeat --operation entry --size mixed
python3 tools/throughput-analysis/diagnose_btree_timing.py --binaries target/btree-timing --output target/btree-linked-code --code-only
```

`prepare_range.py --benchmark compaction` compares retain-all, retain-half,
dedup-distinct and dedup-runs on identical random small/mixed corpora. Each case
includes an exact-capacity input clone, edit and drop, with three independent
seeds, equal calls and excluded calibration targeting two seconds for the fastest
implementation. `--memory` records separate System allocation observations.
`--run-only` validates source, compiler, profile and binary identities before
using an existing build. All intervals must exceed one second.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](../../docs/verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_range.py --benchmark compaction --baseline bc4271d --profile default --memory --output target/compaction --build-only
# After other compilation and checks finish:
python3 tools/throughput-analysis/prepare_range.py --benchmark compaction --baseline bc4271d --profile default --memory --output target/compaction --run-only
```


## ExtractIf and splice-fill source comparison

`prepare_range.py --benchmark extraction` covers no selections, all selections,
half selections, early iterator drop, data-dependent no selections and a long
retained prefix. Every condition uses 64 randomized inputs with either lengths
0–32 or 256–4096. `--benchmark splice-fill` uses the same fixture with empty,
short, exact and excess replacement lengths, then zero-lower-hint ordinary and
large excess conditions. Large excess supplies `4 * input_length + 1` values
and uses positive small lengths 1–32 to reach the private staging collector. Every call clones exact-capacity
input, performs the edit, observes the result and drops the owner. The shared
runtime workload selector remains opaque in both std and product consumers.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](../../docs/verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_range.py --benchmark extraction --baseline d28d8ec --profile default --memory --output target/extraction --build-only
# After compilation, provers and memory checks finish:
python3 tools/throughput-analysis/prepare_range.py --benchmark extraction --baseline d28d8ec --profile default --memory --output target/extraction --run-only
```

Use `--benchmark splice-fill --baseline 1509feb` for the splice-fill change.
`--profile loops32` applies `-C llvm-args=-align-loops=32` to the whole comparison
consumer, including std and both product versions. It does not change Cargo's
default build flags. Separate `--memory-only` runs rebuild both libraries and
the memory observer without timing; their summary records only the memory phase.
Default and loop-aligned results are reported separately. The [edit report](../../docs/verification/vec-edit-source-performance.md)
lists exact revisions, measured totals, allocation observations and code-generation
causes. Generated builds, source archives and disassembly remain in `target/`.

`--entry-only --benchmark splice-fill` is a separate coverage diagnostic. It
compiles disposable library copies with an atomic counter at the original
`stage_excess` entry, checks complete final contents against std, and records
entry counts for both product versions over six conditions and six lengths.
Known-length and ordinary zero-hint excess conditions do not enter staging;
positive-length large zero-hint excess enters once. These instrumented binaries
are not used for timing or proof input.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](../../docs/verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_range.py --benchmark splice-fill --baseline 687db7d --entry-only --output target/staging-entries
```
