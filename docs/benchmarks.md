# Box benchmark methodology and remaining work

The latest Vec range-edit measurements are in the [ExtractIf report](verification/vec-extract-inline.md)
and the [Splice batch report](verification/vec-splice-batch.md). The
[November 4 Vec checkpoint](verification/vec-optimization.md) records the
broader operation matrix and optional alignment comparisons.

The [long-interval randomized comparison](verification/throughput-long.md) is the
November 4 total-seconds report. It includes immediate create/drop workloads and
their budget bookkeeping costs.

The [preceding recheck](verification/performance-recheck.md) records the fixed-input
Box/Vec measurements and the corrected small-collection regression. The results
below describe the earlier Box checkpoint.

**Performance status: provisional.** Recorded comparisons passed their chosen
thresholds on one Linux x86_64 environment. They do not establish identical
performance across workloads, allocators, machines, or operating systems.
The Box implementation milestone is paused at this checkpoint while a broader
measurement campaign remains open.

## What was measured

| Workload | Inputs | Operations | Recorded sample structure |
| --- | --- | --- | --- |
| Sized Box | 10,000 / 100,000 u64 owners | Create, clone, drop | 15 processes per implementation, two warm-ups and 30 retained cycles |
| Dynamic Box | 32 / 4,096 / 1,048,576 bytes; slices, str, zeroed slices, exact-length collection | Create, clone, equal-length replacement, drop | 30 processes per implementation, including the initial 15; two warm-ups and 30 retained cycles |
| Memory | Same dynamic inputs; default and budgeted ownership | Retained, peak, cumulative, final balance, handle size, allocation calls | Separate instrumentation for requested and glibc usable bytes |
| Focused clone diagnosis | Four 1MiB strings | Clone | Additional 30 processes using the identical acceptance binary |

The dynamic owner count is `max(4, 1,048,576 / length)`. Source and destination
handle Vec allocations are included. Creation/clone/drop and replacement are
measured in separate process modes, because replacement changed the allocation
lifecycle of later cycles in the original combined mode. The original combined
and inconclusive runs are identified in the checkpoint history, not relabeled as successful.

Processes alternate order and use a fixed CPU. Timing uses the default allocator
without memory instrumentation. Output barriers occur before a phase's timer ends;
input barriers remain in place. Memory instrumentation runs separately and
includes handle storage. Final tracked balance means memory owned by the measured
workload, not the allocator's whole-process resident memory.

## Results and interpretation

The recorded final dynamic run passed 144 time comparisons, including identical
std controls, and 360 memory checks, including std baselines. Unmetered and
metered u64 Box workloads also passed their time checks. The engineering threshold
was a bootstrap 95% interval upper bound of 1.15 relative to std. It is a local
acceptance rule, not a published guarantee.

On the measured 64-bit target, the budget policy adds 8 bytes per handle. Reports
show raw memory and the adjustment for that accepted fixed cost. Additional
payload allocation, leaks, and timing are not exempted.

The largest reported unmetered point estimate was **+6.38% for cloning four 1MiB
strings**. Its 95% interval was −0.80% to +12.24%. Repeating the exact binary gave
+2.65%, with an interval of −3.74% to +12.67%. Both intervals include no difference.
Actual call-site tracing found the same allocation function and one memcpy per
payload, with no extra UTF-8 traversal or budget check in the unmetered path.
No fixed 6.4% instruction cost was established. See the
[case analysis](verification/box-str-clone-cause/README.md).

Use the [snapshot timing instructions](../tools/throughput-analysis/README.md#snapshot-vec-timing)
to measure current Vec behavior and establish the next comparison baseline.

Recorded Box result summaries:

- [Final dynamic time](verification/box-completion/performance-30/summary.json)
  and [memory](verification/box-completion/performance-30/memory-summary.json).
- [Sized time](verification/box-completion/sized-performance/timing-summary.json).
- [Focused repeat](verification/box-str-clone-cause/repeat-summary.json).

## Conditions still to measure

Define and record each experiment before running it. Keep the direct std control
and preserve failed and inconclusive samples.

| Dimension | Planned conditions | Purpose |
| --- | --- | --- |
| Independent sessions and hardware | Repeated sessions; additional CPU/cache configurations | Check whether estimates persist beyond one environment |
| Allocator and page state | Explicitly distinguish fresh-process, warmed reuse, and fresh-page workloads | Separate allocation and page effects from payload copying |
| Size and alignment | More points around allocation boundaries, over-aligned values, small and large payloads | Detect threshold-dependent costs |
| Operation mix | Isolated operations and specified mixed lifecycles; short/unknown/exact iterator hints | Measure realistic behavior without mixing incompatible samples |
| Budget behavior | Unmetered, ample budget, tight budget, and rejection | Separate success overhead from failure latency and cleanup |
| Build and platform | Fixed compiler/optimization settings; additional supported targets with execution access | Test code-generation and platform dependence |
| Timing and memory evidence | Process distributions, confidence intervals, allocator calls/bytes, peak and final ownership; counters where available | Distinguish observations from causal explanations |

These are pending experiments, not completed results. A performance difference
should be attributed to instructions, allocation, cache, or scheduling only after
a corresponding control or measurement supports that attribution.

## Reproduction prerequisites

Runtime code and ordinary Cargo tests need no Varlit checkout. The separate
`tools/box-analysis` workspace compares the actual pinned Varlit implementation and
currently requires this analysis-only path dependency:

```text
<parent of jzap-std>/reference-sources/varlit-cd7b044/
  crates/varlit-core/Cargo.toml
```

The checkout must be commit `cd7b0446da3e731fba867802bef5f4b7b3fc328d` and include its
workspace dependencies. Small source snapshots in `reference/` are provenance
records, not a substitute for the complete analysis fixture. If that checkout is
unavailable, the original reference comparison cannot be reproduced from this
repository alone. This prerequisite must be resolved before claiming a fully
self-contained public benchmark package; do not silently substitute a different
implementation.

With the fixture present, use Rust 1.95, Linux CPU affinity support, Python 3, and
glibc for usable-size measurements:

```sh
cargo build --locked --manifest-path tools/box-analysis/Cargo.toml --release --bins
cargo build --locked --manifest-path tools/box-analysis/Cargo.toml --release --bin dynamic --features memory --target-dir tools/box-analysis/target/memory
python3 scripts/compare_box_dynamic.py --trials 30 --output /tmp/jzap-dynamic
python3 scripts/compare_box_analysis.py --output /tmp/jzap-sized
```

Do not run compilation or verification concurrently with these timings. Scripts
return a nonzero status for failed or inconclusive acceptance comparisons.
Rebuilding with a different compiler or configuration is a new experiment.
Maintained tools write raw outputs under `target/verification/`; copy only the
reviewed result summary into documentation. Historical detailed records can be
retrieved from the source checkpoint in Git history.

## Stack-alignment control

A process's stack address depends on address randomization and on the size of
its environment. Some loops store to more than one stack slot per iteration,
and whether those slots share a 64-byte line changes the store-buffer
throughput. One measured process therefore lands on one of four 16-byte stack
alignments at random, and a small number of seeds can report gaps that differ
by tens of percent between runs (see the [Vec growth
case](verification/linux-bottlenecks.md#vec-push-growth-takes-the-storage-words-by-value)).

`tools/throughput-analysis/compare.py --stack-offsets 0,16,32,48` removes that
sampling. Each measurement then runs once per offset under `setarch -R`, with
the environment cleared except a padding variable of the offset length, so
every executable and label visits all four alignment classes. Each offset is
its own process and must last at least one second; the seed totals sum the
offsets. `report.py` merges the offsets before computing means. Without the
option the original single-process protocol is unchanged.

```sh
python3 tools/throughput-analysis/compare.py --binary new=$PWD/target/release/jzap-throughput-analysis \
  --cpus 6 --stack-offsets 0,16,32,48 --tasks grow --small-tasks grow --small-variants new \
  --output target/verification/stack-offsets
python3 tools/throughput-analysis/report.py --input target/verification/stack-offsets
```

The option controls stack placement only. Code placement (uop-cache lines and
branch-predictor aliasing) is fixed by the build and is examined by comparing
builds, for example with the opt-in loop alignment in [build
tuning](build-tuning.md).
