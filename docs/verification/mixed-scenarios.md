# Mixed-collection scenarios (2026-11-10, local)

`tools/throughput-analysis/src/bin/scenarios.rs` writes each scenario once
against std and once against jzap (generic over the allocation policy) with the
same inputs, call order and checksum. Every collection is created and dropped
inside the timed scenario. The inputs are synthetic; they are not an
application's call frequencies.

Environment and protocol: i7-13700KF, CPU 6 at 3.4 GHz (turbo off, sibling
offline), Rust 1.95.0, default release build; seeds 617/6203/96011, every
interval at least one second, means are `sum(seconds) / sum(calls)`; one call
is one scenario over one of 64 input cases (sizes zero/one/maximum and
log-uniform up to 4096, "small" up to 32). Product source `1b53d12`; benchmark
source `00c381b`. Raw records: `target/verification/local-2026-11-10/scenario-*`.

| Scenario | Collections | Input | std ns | Unmetered | Local budget | Concurrent budget | Unmetered seed ratios |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| wordcount | String, HashMap, BinaryHeap, Vec | mixed | 30,123 | +2.8% | +3.0% | +5.8% | 1.022–1.032 |
| wordcount | | small | 261.2 | +7.6% | +11.1% | +20.8% | 1.063–1.086 |
| scheduler | Box, HashMap, BinaryHeap, VecDeque | mixed | 141,313 | -2.2% | -2.2% | -1.9% | 0.970–0.984 |
| scheduler | | small | 825.5 | +3.7% | +4.6% | +7.9% | 1.024–1.062 |
| index | BTreeMap, BTreeSet, Vec | mixed | 110,984 | +0.6% | +0.6% | -2.4% | 1.004–1.010 |
| index | | small | 368.3 | +1.1% | +8.9% | +13.8% | 1.000–1.021 |
| graph | Vec of Vec, VecDeque, HashSet | mixed | 60,867 | -2.8% | -1.5% | +0.8% | 0.967–0.976 |
| graph | | small | 646.6 | -5.2% | -0.8% | +11.2% | 0.911–0.986 |
| request | all of the above | mixed | 44,290 | +0.1% | +1.0% | +6.4% | 0.999–1.002 |
| request | | small | 414.4 | +3.7% | +3.6% | +20.9% | 1.021–1.064 |

Repeated-std controls range from 0.989 to 1.005. The budget columns are the
total cost of an optional feature std does not have, not a comparison between
equivalent implementations.

Counters (`perf stat` differences between two epoch counts, seed 617,
instructions per 64-case epoch, std / jzap unmetered): scheduler 67.1M / 62.2M
and 728K / 685K (small); index 34.1M / 31.6M and 326K / 328K; graph 38.6M /
35.7M and 567K / 528K; request 28.9M / 29.7M and 391K / 404K; wordcount 17.5M /
18.7M and 249K / 266K.

The wordcount rows are after the group-mask retirement change; before it the
small input was +18.6% and the mixed input +3.7% (cause and counters in
[hash-map-design.md](../hash-map-design.md#group-mask-retirement-on-clear-and-drop-2026-11-10)).
The remaining small-input wordcount difference is mostly instruction count and
is not attributed further.

Reproduce one scenario:

```bash
cd tools/throughput-analysis && cargo build --release --features hash-map --bin scenarios && cd ../..
python3 tools/throughput-analysis/compare.py --binary default="$PWD/tools/throughput-analysis/target/release/scenarios" --tasks scenario_wordcount --small-tasks scenario_wordcount --small-variants default --output target/verification/scenario-wordcount
python3 tools/throughput-analysis/report.py --input target/verification/scenario-wordcount
```
