# Typed ExtractIf scan experiment

The 17 November 2026 experiment applies const generics and `NonZeroUsize` to
the real public extraction path in disposable builds of `2c278c2`. Product
source and compiler defaults stay unchanged. The candidate kernel is maintained
in `tools/throughput-analysis/experiments/extraction_phase.rs`; source copies,
assembly and receipts are ignored under `target/verification/extract-phase-20261117/`.
The maintained kernel and builder are preserved at
`6059f0a`
in PR 5.

The selected-first candidate reduces the repeated all-selected path from twelve
to ten instructions, but grows retained-prefix work from five to seven. The
completed twelve-process comparison observes mixed all-selected time 34.7%
lower, with its statistical verdict held by control drift. Mixed retained-prefix
time increases 6.2–9.2% with stable controls and adjusted bounds excluding no
change. Native and memory checks pass. This remains an experiment with no formal
source/link certificate or product adoption.

The subsequent [normal-inlining follow-up](vec-extract-inline.md) instead
changes one private attribute on the unchanged deletion-count driver and
recovers a five-instruction selected loop. Its validation and timing status
are reported separately; the measurements below remain this typed experiment.

## Implementation and generated code

The scan is instantiated with `HAS_GAP = false` and zero-sized `Prefix`, or
`HAS_GAP = true` and `Gapped(NonZeroUsize)`. The retained destination is
`read - removed`. The four-word Extraction state and its existing methods stay
unchanged. Within one scan, retention changes only read; selection updates the
deletion count and returns immediately. The cached removed count therefore
stays constant during retained scanning. Errors do not advance the cursor, and
movement still precedes retirement of a retained element.

The initial version selects its phase before scanning. Its const-only and
NonZero consumers have identical function assembly: the local scan's gap test
is removed, but caller dispatch remains after every returned element. The
selected-first follow-up admits and tests one element first, returning a
selected element before constructing a phase witness. Only after retention
does it enter the typed scan. Predicate order and call count are preserved.

The phase witness is transient within one `next()` call; it does not replace
the public iterator's stored state. Monomorphization removes the scan-local
gap branch, but does not make the phase statically known across subsequent
`next()` calls. The experiment adds no unsafe code. Its derived destination
arithmetic still needs source-connected proof before product adoption.

Rust 1.95.0 / LLVM 22.1.2 uses the existing dynamic-mode `u64`
clone/edit/observe/drop consumer with `-O -C panic=abort -C codegen-units=1`.
Experiment switches are materialized into ordinary source before compilation;
there are no LLVM cost overrides or application build requirements.

Counts cover successful repeated paths, including conditional branches and
unconditional jumps, but excluding entry and cleanup. Std's all-selected path
remains five instructions / one conditional branch in a 539-byte wrapper.

| Variant | All-selected instructions / conditional branches | Retained-prefix instructions | Wrapper bytes |
| --- | ---: | ---: | ---: |
| Deletion-count baseline | 12 / 3 | 5 | 724 |
| Const generic, ordinary count, forced inline | 13 / 3 | 5 | 899 |
| Const generic, NonZero witness, forced inline | 13 / 3 | 5 | 899 |
| NonZero, inline hint or automatic inline | 17 / 4 | 6 | 636 |
| NonZero, no inline | Per-result scan call | In called scan | 591 + 213 in two scan functions |
| NonZero, selected-first entry, forced inline | 11 / 3 | 6 | 995 |
| NonZero, selected-first entry, inline hint: timing candidate | 10 / 3 | 7 | 737 |
| Ordinary count, selected-first entry, inline hint: control | 10 / 3 | 7 | 721 |

The candidate's selected path has no gap test, but still checks the fixed mode
twice and advances read/deleted separately. Its retained prefix has seven
instructions; an earlier six-instruction progress report confused two inline
variants and was corrected. Existing-hole parity retention also grows from
seven to eleven instructions. Smaller function size alone does not establish
a speedup. The ordinary-count selected-first control also reaches ten selected
instructions, so that reduction cannot be attributed uniquely to NonZero.

## Completed validation

| Check | Result and scope |
| --- | --- |
| Native Vec | 41 pass in both debug and release for const-only, const-plus-NonZero, selected-first NonZero and selected-first ordinary-count controls |
| Allocation observation | Each of those four controls has 108 observations / 36 triples matching baseline/std for calls, requested/usable held and peak bytes, cleanup and handle size |
| Candidate Miri | All 41 pass in native strict provenance, i686 strict provenance and Tree Borrows |
| Candidate ASan | All 41 pass with `detect_leaks=0`; no LeakSanitizer pass is claimed |
| Formal source/link | Not run for the replacement; historical certificates do not cover these experimental drivers |
| Other inline variants | Assembly comparisons only; no native/memory pass is claimed |
| Tooling checks | Timing statistics tests 3/3; verification-runner tests 30/30 after activating the Rust environment; kernel rustfmt, builder Python syntax and diff whitespace checks pass |

The initial consumer build failed because both libraries had the same stable
crate identity. The final runner names them `jzap_old` and `jzap_new`; the failed
attempt is not counted as validation. The timing source copy reads Git archives
from the repository root; that generated runner changes only the archive working
directory. The measured source and binary are fingerprinted by the existing tool.
The first final runner-test invocation omitted the environment activation:
four tests errored because `cargo` was absent from PATH. The activated rerun
passes all thirty tests; the failed invocation is not counted as a pass.

## Repeated timing

The comparison uses the deletion-count runtime at `2c278c2` as its baseline,
not the earlier read/write representation. The selected-first NonZero variant
with an inline hint is the only variant timed. Ordinary-count and other inline
variants above are code-generation controls, not elapsed-time comparisons.

The fixed protocol uses three input seeds and four rounds, giving twelve
separate process invocations and twelve paired blocks per condition. Each
group shares 64 prepared inputs, operation counts and checksums across std,
baseline, candidate and an identical baseline control. The public unmetered,
unlimited `u64` clone/edit/observe/drop path is measured at lengths 0–32 and
256–4096. Compilation and memory checks finish before timing, which is pinned
to one allowed CPU. Consumer source and executable fingerprints are checked.

The prescribed 50,000 paired bootstrap resamples stay within the three fixed
seed strata. Tables display pointwise 95% intervals; effect classifications
require Bonferroni-adjusted bounds for 24 comparisons to exclude one and the
identical-code A/A 95% interval to lie wholly within 0.95–1.05. The std +15%
gate also uses adjusted bounds and that control gate. No observations are
discarded or extra rounds added to obtain significance. These shared-host
results describe this generated program and input corpus; ratios from prior
campaigns are not multiplied into this comparison.

All twelve processes complete: 144 paired groups, 576 retained intervals of
1.806–4.502 seconds, 1,382.354 seconds total and 47,033,990,144 calls across
the four implementations. No short-group retry or outlier removal occurs.
CPU 0 on the host reports AMD EPYC 9V45. The environment connection was
reestablished during measurement; the original process continued with the same
records and protocol, without a restart or a second campaign.

Ten conditions meet the A/A gate. Mixed all-selected has an identical-code
control ratio of 0.98156, with a 95% interval of 0.94858–1.01492. Small
prefix/no-removal has a ratio of 0.96850 and interval 0.94232–0.99272.
Both miss the required lower bound of 0.95; their effects and std +15% verdicts
remain unresolved. The maximum mean A/A deviation is 3.15%.

The table reports operation-count-weighted means and pointwise intervals.
The [compact data](vec-extract-phase-types.csv) preserves all 48 wall/CPU
comparisons, pointwise/adjusted bounds, control diagnostics and classifications.

| Condition | std ns | Baseline ns | Candidate ns | Change vs baseline, 95% interval | Change vs std | Classification vs baseline |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| Constant false / small | 10.685 | 11.096 | 11.385 | +2.6% (+0.2% to +5.4%) | +6.6% | unresolved |
| Constant false / mixed | 213.560 | 212.911 | 215.935 | +1.4% (-0.8% to +3.8%) | +1.1% | unresolved |
| All selected / small | 15.727 | 20.282 | 19.529 | -3.7% (-5.5% to -1.8%) | +24.2% | lower |
| All selected / mixed | 780.730 | 1379.841 | 901.488 | -34.7% (-36.3% to -33.1%) | +15.5% | control drift: unresolved |
| Parity / small | 16.405 | 17.807 | 18.042 | +1.3% (-0.6% to +3.3%) | +10.0% | unresolved |
| Parity / mixed | 7731.689 | 7981.519 | 7811.108 | -2.1% (-3.9% to -0.2%) | +1.0% | unresolved |
| Early drop / small | 13.757 | 13.817 | 14.003 | +1.3% (-0.6% to +3.2%) | +1.8% | unresolved |
| Early drop / mixed | 267.585 | 263.968 | 262.289 | -0.6% (-1.8% to +0.5%) | -2.0% | unresolved |
| Retained prefix, none / small | 15.739 | 14.665 | 15.938 | +8.7% (+5.4% to +11.5%) | +1.3% | control drift: unresolved |
| Retained prefix, none / mixed | 800.953 | 755.656 | 825.452 | +9.2% (+6.2% to +12.8%) | +3.1% | higher |
| Retained prefix, last selected / small | 16.166 | 15.402 | 16.704 | +8.5% (+6.3% to +10.9%) | +3.3% | higher |
| Retained prefix, last selected / mixed | 810.110 | 753.637 | 800.550 | +6.2% (+3.4% to +8.9%) | -1.2% | higher |

The mixed all-selected observation is 901.488 ns versus baseline 1,379.841 ns
and std 780.730 ns. Its candidate/baseline adjusted interval is
0.62791–0.67543, and candidate/std is 1.10506–1.20383. The control gate still
holds both verdicts unresolved; the std interval also spans the +15% boundary.
Small all-selected improves 3.7% with a stable control, but remains 24.2% above
std, with adjusted candidate/std bounds 1.20857–1.29408. It fails that criterion.

The mixed prefix regressions are 69.796 ns with no removal and 46.913 ns with a
final removal. Their candidate/baseline adjusted bounds are 1.04903–1.15018
and 1.01994–1.10178, respectively. Both classify as higher with stable controls.
Small prefix/final-removal is also higher by 8.5%. Against std, nine conditions
are within the +15% bound, one is above and two have unresolved control drift.

Thread CPU/wall time is 99.90% in aggregate, with a minimum interval ratio of
98.46%. CPU-time candidate/baseline ratios are 0.65258 for mixed all-selected,
1.09226 for mixed prefix/no-removal and 1.06201 for mixed prefix/final-removal.
The wall-time changes also appear in thread CPU time. Instruction and branch
differences are established above; a hardware-counter attribution of their
individual elapsed cost is not established.

The experiment therefore demonstrates local branch removal and a tradeoff,
not a general performance pass. The pure type change alone did not recover
std's five-instruction selected loop; selected-first entry and inline policy
improve it partially while increasing retention work. The product keeps the
deletion-count runtime. No whole-library proof or performance certificate is
claimed for this replacement driver.

## Reproduction

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
. /workspace/.devtools/activate.sh
python3 tools/throughput-analysis/prepare_extraction_phase.py \
  --baseline 2c278c2 --checks --timing-workspace \
  --output target/verification/extract-phase-replay
```

From its generated `timing-work` directory:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/measure_vec_edit.py --baseline 2c278c2 \
  --output target/statistics --rounds 4 --seeds 1,2,3 --target-seconds 2 --build-only
# Finish all compilation and other checks before timing.
python3 tools/throughput-analysis/measure_vec_edit.py --baseline 2c278c2 \
  --output target/statistics --rounds 4 --seeds 1,2,3 --target-seconds 2 --run-only
```

The [previous fixed protocol](vec-extract-deleted-count.md#repeated-timing)
supplies counterbalanced order, an identical-code A/A control, thread CPU
diagnostics and paired stratified bootstrap with adjusted bounds. Instruction
counts above are not an elapsed-time verdict.
