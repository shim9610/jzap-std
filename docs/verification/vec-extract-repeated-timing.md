# Repeated ExtractIf timing in the cloud

The 17 November 2026 follow-up measures the verified selected-first candidate
against `6d17299` and Rust 1.95.0 std. Product and proof inputs remain unchanged
from the completed [source/link verification](vec-extract-selected-first.md).
The fixed twelve-process campaign is complete. All-selected workloads improve
clearly, while retained-prefix workloads regress substantially.

## Results

All twelve processes complete: 144 paired groups and 576 retained intervals,
with no short-group retries or outlier removal. Each interval lasts
1.801–4.903 seconds; retained wall time totals 1,449.749 seconds. The four
implementations together execute 44,932,658,432 calls. The statistical sample
size remains twelve paired process blocks per condition.

A/A mean ratios differ from 1 by at most 2.29%; every condition's 95% A/A
interval lies within the planned 0.95–1.05 band. Across conditions those
interval endpoints range from 0.9574 to 1.0485. Aggregate thread CPU/wall time
is 99.93%, with a minimum per-interval ratio of 99.31%. Thus off-CPU waiting is
small in these observations, while frequency/cache/host effects remain possible.

The candidate reduces all-selected elapsed time by 28.5% for small inputs
and 43.0% for mixed sizes. These reductions remain clear under the family
bounds: candidate/previous 0.6855–0.7459 and 0.5491–0.5898. The retained-prefix
conditions increase time by 55.9–101.4%; all four are above std +15% even under
the adjusted bounds. This candidate therefore does not satisfy the general
performance criterion. It remains a verified Draft PR candidate, not an accepted
default optimization. Product code is unchanged by this timing work.

| Condition | std ns | Previous ns | Candidate ns | Change vs previous, 95% interval | Change vs std | Family classification vs previous |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| Constant false / small | 11.207 | 11.747 | 11.358 | -3.3% (-5.3% to -1.2%) | +1.3% | lower (borderline) |
| Constant false / mixed | 217.695 | 220.848 | 220.151 | -0.3% (-3.0% to +2.7%) | +1.1% | unresolved |
| All selected / small | 16.149 | 23.374 | 16.706 | -28.5% (-30.5% to -26.5%) | +3.5% | lower |
| All selected / mixed | 821.828 | 1427.085 | 814.038 | -43.0% (-44.4% to -41.6%) | -0.9% | lower |
| Parity / small | 16.685 | 18.277 | 18.133 | -0.8% (-3.6% to +2.2%) | +8.7% | unresolved |
| Parity / mixed | 8671.295 | 8045.100 | 8263.270 | +2.7% (+0.6% to +5.1%) | -4.7% | unresolved |
| Early drop / small | 14.590 | 14.805 | 14.701 | -0.7% (-3.4% to +1.8%) | +0.8% | unresolved |
| Early drop / mixed | 272.287 | 270.235 | 276.316 | +2.3% (+1.1% to +3.4%) | +1.5% | higher |
| Retained prefix, none / small | 16.039 | 14.955 | 24.082 | +61.0% (+57.4% to +65.4%) | +50.1% | higher |
| Retained prefix, none / mixed | 803.253 | 770.054 | 1550.516 | +101.4% (+95.5% to +107.1%) | +93.0% | higher |
| Retained prefix, last selected / small | 16.502 | 16.288 | 25.398 | +55.9% (+53.0% to +59.3%) | +53.9% | higher |
| Retained prefix, last selected / mixed | 803.910 | 788.438 | 1386.331 | +75.8% (+69.5% to +82.3%) | +72.4% | higher |

Ratios below 1 indicate less elapsed time. The displayed intervals are
pointwise 95%; classifications use the stricter family bounds in the
[compact data](vec-extract-repeated-timing.csv). Those bounds classify three
conditions as lower, five as higher and four as unresolved versus previous.
The small constant-false result is borderline: its upper family bound is
0.9999, so it should not be treated as strong evidence from twelve blocks.
The small early-drop and both half-selection changes are unresolved; mixed
early drop increases time by 2.3%. Eight conditions are within the observed
std +15% bound and the four retained-prefix conditions are above it.

Thread CPU ratios give the same classification counts and reproduce the large
changes: all-selected small/mixed 0.7149/0.5704; retained-prefix ratios
1.5596–2.0143. The large gains and regressions are therefore not explained by
an observed scheduler wait alone. The retained-path increases are consistent
with the previously identified extra cursor maintenance and gap tests
(5→10 and 8→11 repeated instructions); hardware stall attribution is not
established. These whole-call measurements do not calibrate LLVM MCA's isolated
cycle predictions or establish performance on other hosts.

## Fixed protocol

The benchmark uses the existing public-API clone/edit/observe/drop consumers,
default unmetered/unlimited policy and `-O -C panic=abort -C codegen-units=1`.
Each call includes allocation, cloning, iteration, observation and cleanup.
All compilation and verification finish before timing starts. The host reports
AMD EPYC 9V45; all measurements inherit affinity to one allowed CPU.

Three fixed input seeds each run in four separate processes, for twelve process
blocks. Each block contains twelve conditions: no selection, all selection,
half selection, early iterator drop, a retained prefix with no selected value,
and a retained prefix with only its final value selected, each at small and
mixed sizes. Each condition has 64 prepared inputs; small lengths are 0–32 and
mixed lengths 256–4096. Preparation is outside timing. Per-case input streams
and digests keep the corpus identical when condition order changes.

Each paired group measures std, baseline, candidate and a second call to the
exact same baseline function. The latter is the A/A control. Implementations
share corpus, call count and checksum. Their order is counterbalanced across
twelve blocks; condition and seed order also vary deterministically.
Calibration targets two seconds for the fastest implementation with a 10% work
margin. Every retained interval must last at least one second. A short group
is kept as a rejected attempt and all four intervals repeat with twice the work.

The stopping rule is fixed before measurement: four rounds of three seeds.
There is no significance-based stopping, trimming or removal of slow outliers.
Wall time, thread CPU time and surrounding context-switch counters are recorded
for each interval. CPU time diagnoses scheduling; it does not correct frequency,
cache or virtual-machine effects.

## Statistical interpretation

The mean is total elapsed time divided by total calls. Ratios compare matched
work; the calls within a long interval are not independent samples. The sampling
unit is a paired process block. Bootstrap resampling retains the implementation
pair and samples four blocks within each of the three fixed input-seed strata,
using 50,000 resamples and seed 617.

The report includes pointwise 95% intervals and Bonferroni bootstrap bounds for
the 24 primary wall-time comparisons: candidate/baseline and candidate/std
across twelve conditions. An effect is classified only if the family bounds
exclude 1 and the A/A control's 95% interval lies entirely inside 0.95–1.05.
The std +15% comparison uses the same family bounds. Thread CPU results are
supporting diagnostics. These approximate intervals are conditional on the fixed
corpora and process-block resampling model; more repetitions do not remove
shared-host temporal correlation or systematic bias.

## Reproduction

Activate the repository toolchains and stop other build/proof jobs before
measurement. Raw observations, calibration/rejected attempts, build provenance,
assembly and binary hashes remain under ignored `target/`. The run phase refuses
changed source, protocol or executable inputs.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
python3 -m unittest discover -s tools/throughput-analysis \
  -p test_measure_vec_edit.py -v
python3 tools/throughput-analysis/measure_vec_edit.py \
  --baseline 6d172996618879cdece75a182d0c8d22fda82819 \
  --rounds 4 --seeds 1,2,3 --target-seconds 2 --build-only \
  --output target/verification/extract-statistics-replay
python3 tools/throughput-analysis/measure_vec_edit.py \
  --baseline 6d172996618879cdece75a182d0c8d22fda82819 \
  --rounds 4 --seeds 1,2,3 --target-seconds 2 --run-only \
  --output target/verification/extract-statistics-replay
```

Validation tests reject incomplete/duplicated pairs, short intervals and
changed work/results. Statistical tests verify that shared block noise cancels
for a constant paired effect and that large A/A variation remains visible.
