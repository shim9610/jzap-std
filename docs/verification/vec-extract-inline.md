# ExtractIf normal-inlining follow-up

The runtime change is one attribute on `vec::compaction_driver::extract_next`:
`#[inline(always)]` becomes `#[inline]`. The driver body, numeric state,
contracts, physical copies, callback/error order and public API are unchanged.
The compiler may still inline the driver; the measured consumer has no new
driver call. No application LLVM options are required.
The runtime checkpoint is `656c3306bb561781a6eb910fa0674c5f10757a6a`.

The selected loop now has five instructions, matching std, versus twelve in
the deletion-count baseline at `2c278c2` and ten in the
[typed-scan experiment](vec-extract-phase-types.md). The retained prefix has
six, versus five in the baseline and seven in that experiment. The completed
twelve-process comparison reduces all-selected time 23.0% for small sizes and
41.5% for mixed sizes, with stable controls and adjusted intervals excluding
no change. Mixed retained prefixes change -0.2% and +0.6%, neither resolved;
mixed parity is 5.0% slower than baseline. All twelve measured conditions meet
the adjusted std +15% criterion. Executed and focused formal checks pass.

## What changed in generated code

Rust 1.95.0 / LLVM 22.1.2 compiles the existing public unmetered, unlimited
`u64` clone/edit/observe/drop consumer. The statistical build uses
`-O -C panic=abort -C codegen-units=1`, with no unswitch cost override.
Its candidate wrapper is 610 bytes, versus baseline 724 and std 539.
Counts cover repeated successful paths, including jumps and excluding
entry/cleanup; the prefix figures describe parity-based retained scanning.
The timing callback returns `Result<bool, Infallible>`. Returning-error tests
are separate; these timings do not quantify a nonzero error payload or error
frequency.

| Path | Forced-inline baseline | Normal-inline candidate | std |
| --- | ---: | ---: | ---: |
| All selected | 12 instructions / 3 conditional branches | 5 / 1 | 5 / 1 |
| Retain before the first hole | 5 / 2 | 6 / 2 | 8 / 2 |
| Retain with an existing hole | 7 / 2 | 7 / 2 | 7 / 2 |

The candidate's all-selected loop loads a value, increments one cursor,
stores the observed result, compares with the end and branches back. Fixed
`kind == 0` / `kind == 1` selector checks move outside that loop. Since this
specialized path never retains an element, read and deleted are coalesced
into one induction variable. The arbitrary public predicate still runs once
per visited element; this is not permission to skip a stateful predicate.

The retained prefix gains a separate value load/test instead of the baseline's
memory-operand test. It does not gain a self-store, and existing-hole retention
keeps its seven-instruction path. Instruction counts alone do not establish the
elapsed effect.

The ordinary `rustc -O` control also produces the five-instruction selected and
six-instruction prefix loops, with no explicit panic/codegen-unit overrides. Its
candidate wrapper is 622 bytes, versus baseline 724 and std 539. This control
is code generation only, not a second timing campaign or other-CPU result.
A separate default `rustc -O` link-only build, without assembly emission or
panic/codegen-unit overrides, also has five selected instructions in a 593-byte
candidate wrapper. This checks that the result is not specific to requesting
assembly output; it is not an elapsed-time comparison.

## Controls and compiler diagnosis

Changing only `ExtractIf::next` to a normal inline hint, only the driver to a
hint, both to hints, or removing those hints produces the same 610-byte
consumer in the controlled profile. The retained change is the private driver
hint; `next` keeps its existing annotation.

Before/after `SimpleLoopUnswitchPass` dumps show the forced-inline baseline's
outer extraction loop unchanged and the normal-inline candidate's outer loops
transformed. The attribute changes the intermediate representation (IR)
presented to ordinary optimization;
no particular numeric cost reduction or universal inliner rule is asserted.
Stable `--emit=mir` output for the local consumer does not expose enough of
the external generic driver to identify an earlier transformation. That
diagnostic is inconclusive, not a proof of the inlining stage responsible.

Other disposable controls were investigated before selecting the attribute:

| Control | Observed generated code; no elapsed verdict |
| --- | --- |
| Fuse the index/result conversion into `next` | While/loop variants retain the baseline's 724-byte consumer |
| Outline only retained scanning | 632-byte caller plus 183-byte scan; iterator state escapes to the stack and selector/state loads remain in the selected path |
| Store written/deleted and derive read | Fewer counter writes, but derived-index and dispatch work remains |
| Unconditional overlap-safe retained copy | Reaches a five-instruction selected loop, but adds prefix self-stores; rejected in favor of the attribute-only change |
| Copy a zero/one count to suppress self-copy | Reaches five selected instructions, but retains a memcpy call inside retained scanning |
| Pair equal read/write prefix updates | Improves the prefix expression, but does not recover a comparably simple selected loop |

These are source copies under ignored `target/verification/extract-next-control/`,
not additional product changes or formal verification passes. An initial
outlined prototype incorrectly put a helper inside the Iterator trait impl
and failed compilation; the corrected inherent-method build is the one above.
One diagnostic launch omitted Rust environment activation and did not run;
the activated diagnostic completed. Neither failure is counted as validation.

## Completed validation

| Check | Fresh result and scope |
| --- | --- |
| Native tests | 433 pass in debug, release and overflow-checked release |
| Quality/build | Formatting; all/core Clippy; core, wasm32, thumbv7em and no-global-OOM compile checks pass |
| Direct Verus | Original driver and its contracts pass; three actual-source scan/retirement defects detected |
| Actual Rust Kani | Extraction cursor/error and bounded three-element content/order harnesses pass; three compiled source defects detected |
| Vec memory | 41 each pass in native strict-provenance Miri, i686 Miri, Tree Borrows and AddressSanitizer (ASan) |
| LeakSanitizer follow-up | All 41 pass with leak detection enabled outside the restricted execution-tool sandbox; normal and intentional product-owner leak controls behave as expected |
| Allocation | 108 observations / 36 std-baseline-candidate triples match, including held/peak requested/usable bytes, handle size and zero held bytes after cleanup |
| Whole-library source/link | Not rerun; no historical global certificate is applied to this current deletion-count runtime |

The original ASan run uses `detect_leaks=0`. The subsequent
[LeakSanitizer follow-up](vec-extract-leaks.md) reruns all 41 with
`detect_leaks=1` and confirms the exit-time scan and intentional-leak detection.
The unchanged
deletion-count numeric-state proof is historical evidence from the
[state-representation report](vec-extract-deleted-count.md), separate from
the fresh driver/caller checks above.

## Repeated timing

The fixed comparison uses the deletion-count runtime at `2c278c2`, not the
previous typed or read/write experiment. Three input seeds and four rounds
give twelve separate process blocks per condition. Std, baseline, candidate
and an identical baseline control receive matched prepared inputs and work.
The [existing protocol](vec-extract-repeated-timing.md#fixed-protocol) retains
counterbalanced order, intervals of at least one second, fixed stopping,
50,000 stratified paired bootstrap resamples, adjusted bounds and the A/A gate.
Compilation and verification finish before measurement. All twelve process
blocks complete, retaining 144 paired groups and 576 intervals of
1.797–4.285 seconds. These total 1,368.040 seconds and 48,083,814,144 calls
across the four implementations. Calibration records are excluded; no short
group retry or outlier removal occurs. Measurement is pinned to CPU 0 on the
host reporting AMD EPYC 9V45. Input lengths are 0–32 and 256–4096, with 64
prepared inputs per condition. Source and executable fingerprints stay unchanged.

All twelve identical-code (A/A) controls meet the fixed stability gate: their
pointwise 95% intervals lie wholly within 0.95–1.05. The maximum mean control
deviation is 1.97%; the overall interval extrema are 0.96199 and 1.04343.
The table reports operation-count-weighted wall-time means and pointwise 95% intervals.
Classifications require the stricter Bonferroni bounds for 24 comparisons to
exclude one as well as the A/A gate. The [compact data](vec-extract-inline.csv)
preserves all 48 wall/CPU comparisons and both sets of bounds.

| Condition | std ns | Baseline ns | Normal inline ns | Change vs baseline, 95% interval | Change vs std | Classification vs baseline |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| Constant false / small | 10.547 | 10.871 | 10.690 | -1.7% (-3.9% to +0.6%) | +1.4% | unresolved |
| Constant false / mixed | 209.696 | 209.237 | 211.550 | +1.1% (-0.3% to +2.6%) | +0.9% | unresolved |
| All selected / small | 15.809 | 20.383 | 15.703 | -23.0% (-24.2% to -21.6%) | -0.7% | lower |
| All selected / mixed | 773.704 | 1324.453 | 775.111 | -41.5% (-43.4% to -39.6%) | +0.2% | lower |
| Parity / small | 15.873 | 17.029 | 17.035 | +0.0% (-2.1% to +2.4%) | +7.3% | unresolved |
| Parity / mixed | 7641.783 | 7875.490 | 8269.695 | +5.0% (+3.6% to +6.5%) | +8.2% | higher |
| Early drop / small | 13.890 | 13.356 | 13.709 | +2.6% (+0.2% to +5.3%) | -1.3% | unresolved |
| Early drop / mixed | 260.683 | 266.954 | 261.967 | -1.9% (-4.0% to +0.4%) | +0.5% | unresolved |
| Retained prefix, none / small | 15.295 | 13.676 | 14.296 | +4.5% (+1.0% to +8.8%) | -6.5% | unresolved |
| Retained prefix, none / mixed | 793.132 | 737.928 | 736.165 | -0.2% (-2.1% to +1.7%) | -7.2% | unresolved |
| Retained prefix, last selected / small | 16.297 | 15.305 | 15.431 | +0.8% (-1.5% to +3.0%) | -5.3% | unresolved |
| Retained prefix, last selected / mixed | 790.481 | 748.163 | 752.959 | +0.6% (-1.3% to +2.4%) | -4.7% | unresolved |

All-selected candidate/baseline adjusted intervals are 0.75497–0.79096
(small) and 0.55673–0.61424 (mixed). Their std ratios are 0.99331 and
1.00182, with adjusted intervals 0.96150–1.02067 and 0.95164–1.05737.
Both improvements over baseline are clear; neither std difference is resolved.
All twelve std comparison upper bounds are within 1.15; the largest is 1.10659
for mixed parity. This passes the prescribed std criterion for this corpus
and generated program, not for every type, callback, compiler or application.

The mixed-prefix candidate/baseline adjusted intervals are 0.96897–1.02652
with no removal and 0.97725–1.03206 with a final removal. No change is resolved,
and those upper bounds exclude the typed experiment's 6.2–9.2% regressions in
these conditions. Their direct std ratios are 0.92817 and 0.95253, both lower
after adjustment. Ratios from different generated programs are not multiplied
into this comparison.

Mixed parity increases from 7,875.490 ns to 8,269.695 ns: +394.205 ns / +5.0%,
with adjusted candidate/baseline bounds 1.02833–1.07227. It remains within the
std criterion at +8.2%, with adjusted candidate/std bounds 1.06194–1.10659.
This regression is retained, not hidden by the selected-path improvement.

Its assembly has the same seven-instruction retained run and the same retained
load/store operations. After the first hole, selected transitions and branch layout differ: the
baseline's parity branch jumps backward to a shared selected block that reloads
the value and repeats fixed-mode dispatch; the candidate branches forward to a
selected block, reuses the loaded value and omits that repeated mode dispatch.
The candidate therefore does not introduce extra element copies or allocation
to explain the +5.0%. The exact hardware cause of this elapsed regression is
not established by instruction counts; branch/stall counters were not collected.

Thread CPU/wall time is 99.91% in aggregate, with a minimum interval ratio of
98.89%. CPU candidate/baseline ratios are 0.77169 for small all-selected,
0.58527 for mixed all-selected and 1.04998 for mixed parity. The large changes
also appear in thread CPU time. Shared-host uncertainty is assessed by the
fixed paired protocol, not removed by treating loop iterations as independent
statistical samples.

## Current checks and historical reproduction

The [snapshot timing commands](../../tools/throughput-analysis/README.md#snapshot-vec-timing)
measure the current ExtractIf implementation against std. With the
[required tools](README.md#tool-requirements), run these checks on the checkout.

```sh
bash scripts/verify.sh --area all --check format --check tests \
  --check release --check overflow --check clippy --check builds \
  --output target/verification/extract-inline-quality
python3 scripts/verify_source.py --group vec-extraction \
  --group vec-extraction-values --mutations \
  --output target/verification/extract-inline-source
ASAN_OPTIONS=detect_leaks=0 \
  bash scripts/verify.sh --area vec --test-target vec --profile memory \
  --output target/verification/extract-inline-memory
```

The original timing and allocation replay requires baseline `2c278c2`.

```sh
python3 tools/throughput-analysis/prepare_range.py --benchmark extraction \
  --baseline 2c278c2 --profile default --memory-only \
  --output target/verification/extract-inline-allocation
python3 tools/throughput-analysis/measure_vec_edit.py --baseline 2c278c2 \
  --output target/verification/extract-inline-statistics \
  --rounds 4 --seeds 1,2,3 --target-seconds 2 --build-only
# Finish all compilation and checks before timing.
python3 tools/throughput-analysis/measure_vec_edit.py --baseline 2c278c2 \
  --output target/verification/extract-inline-statistics \
  --rounds 4 --seeds 1,2,3 --target-seconds 2 --run-only
```

Raw assembly, diagnostics, receipts and timing records are ignored under
`target/verification/extract-inline-followup/`.
