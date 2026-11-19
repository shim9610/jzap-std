# ExtractIf deletion-count experiment

This follow-up implements and tests the requested std-style state representation
against the selected-first read/write candidate at `47a4f3f`. The experimental implementation
stores `read`, `deleted`, `end` and `total`. Retaining an element advances only
`read`; selecting one advances both `read` and `deleted`. The destination cursor
is derived as `read - deleted`, and cleanup restores `total - deleted` elements.
The private state remains four machine words. The public API, callback order,
returned-error stop, tail restoration and allocation policy are unchanged.

The experimental runtime is preserved at
`047bcda`
in PR 5.
Repeated timing is complete: retained-prefix time falls 29.9–34.7% versus the
read/write candidate, but mixed all-selected time rises 48.5% and exceeds std
by 47.4%. The changed representation remains an experiment, not an accepted
general default optimization. Focused source and executed memory checks pass.
A fresh whole-library source/link certificate has **not** run for this
representation; the historical `f92e94f` certificate does not certify it.

## Repeated timing

The fixed campaign completes twelve separate process invocations: three
input seeds, four rounds, 144 paired groups and 576 retained intervals. All
intervals last 1.706–3.868 seconds, totaling 1,431.637 seconds and
45,166,365,952 calls across the four implementations. No short-group retry or
outlier removal occurs. The statistical sample size is twelve paired blocks
per condition, rather than the number of loop iterations.

The unmetered, unlimited `u64` consumers process 64 prepared inputs per condition:
small lengths 0–32 and mixed lengths 256–4096. Each paired group shares inputs,
call count and checksum across std, read/write, deletion-count and the identical
read/write control. Compilation and proof execution finish before timing; the
benchmark is pinned to CPU 0 on the host reporting AMD EPYC 9V45. Paired bootstrap
uses 50,000 resamples within the three fixed seed strata.

Eleven conditions meet the prescribed A/A stability gate. All-selected/small
does not: its identical-code control ratio is 0.9836 with a 95% interval of
0.94925–1.01571, just outside the required 0.95–1.05 band. Its observed +20.6%
change versus read/write and +23.0% versus std remain reported, but the effect
and std +15% verdict are unresolved under the fixed protocol. No extra rounds
are added to change that verdict. The maximum mean A/A deviation is 1.64%.

The table displays pointwise 95% intervals. Classifications use the stricter
Bonferroni bounds for 24 comparisons and the A/A gate, following
[the fixed protocol](vec-extract-repeated-timing.md#fixed-protocol). The complete
wall/CPU ratios and bounds are in the [compact data](vec-extract-deleted-count.csv).

| Condition | std ns | Read/write ns | Deletion-count ns | Change vs read/write, 95% interval | Change vs std | Classification vs read/write |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| Constant false / small | 11.108 | 11.258 | 11.492 | +2.1% (+0.8% to +3.2%) | +3.5% | unresolved |
| Constant false / mixed | 215.760 | 218.662 | 218.460 | -0.1% (-2.6% to +2.1%) | +1.3% | unresolved |
| All selected / small | 16.815 | 17.153 | 20.684 | +20.6% (+18.5% to +22.8%) | +23.0% | control drift: unresolved |
| All selected / mixed | 816.792 | 810.981 | 1204.281 | +48.5% (+44.9% to +52.0%) | +47.4% | higher |
| Parity / small | 16.702 | 18.229 | 18.535 | +1.7% (-0.1% to +3.6%) | +11.0% | unresolved |
| Parity / mixed | 8272.111 | 8479.401 | 8126.897 | -4.2% (-6.9% to -1.3%) | -1.8% | unresolved |
| Early drop / small | 14.440 | 15.003 | 14.350 | -4.4% (-7.1% to -1.8%) | -0.6% | lower |
| Early drop / mixed | 271.927 | 276.254 | 270.810 | -2.0% (-4.3% to +0.2%) | -0.4% | unresolved |
| Retained prefix, none / small | 15.648 | 21.582 | 15.127 | -29.9% (-31.9% to -28.0%) | -3.3% | lower |
| Retained prefix, none / mixed | 819.302 | 1200.207 | 783.862 | -34.7% (-35.5% to -33.8%) | -4.3% | lower |
| Retained prefix, last selected / small | 16.153 | 23.006 | 15.921 | -30.8% (-32.4% to -29.0%) | -1.4% | lower |
| Retained prefix, last selected / mixed | 822.896 | 1159.626 | 768.478 | -33.7% (-34.8% to -32.7%) | -6.6% | lower |

All four retained-prefix improvements remain clear after adjustment. For mixed
sizes, deletion-count/read-write family bounds are 0.6417–0.6679 with no removal
and 0.6472–0.6786 with a final removal. Their std ratios are 0.9567 and 0.9339.
The previous retained-prefix regression is therefore improved in these measured
conditions. Mixed all-selected moves the opposite way: the adjusted ratio is
1.4299–1.5371 versus read/write and 1.4297–1.5272 versus std. This alone fails
the general std +15% criterion. Ten conditions are within that bound, one is
above, and all-selected/small is unresolved because of the control gate.

Thread CPU/wall time is 99.78% in aggregate, with a minimum interval ratio of
98.32%. CPU-time new/read-write ratios are 1.4868 for mixed all-selected,
0.6523 for mixed prefix/no-removal and 0.6640 for mixed prefix/final-removal;
the large changes therefore also appear in thread CPU time. Hardware-counter
stall attribution is not established. These shared-host observations are
conditional on the fixed inputs and generated program. They do not establish
universal performance, and should not be multiplied by ratios from the preceding
campaign to infer a new comparison against its different baseline.

## Generated code

Rust 1.95.0 / LLVM 22.1.2 compiles the existing dynamic-selector `u64`
clone/edit/observe/drop consumer with `-O -C panic=abort -C codegen-units=1`.
The callback returns `Result<bool, Infallible>`. Counts cover the successful
repeated path, including conditional branches, and exclude entry and cleanup.

| Repeated path | Read/write candidate | Deletion-count candidate | std |
| --- | ---: | ---: | ---: |
| All selected, after the first result | 5 instructions / 1 conditional branch | 12 / 3 | 5 / 1 |
| Parity: retain before the first hole | 10 / 3 | 5 / 2 | 8 / 2 |
| Parity: retain with an existing hole | 11 / 3 | 7 / 2 | 7 / 2 |

The deletion-count consumer is 724 bytes, versus 520 for the read/write candidate
and 539 for std. The retained-prefix loop tests the predicate, advances the read
index and tests the range end. No deletion count is changed there. In contrast,
the all-selected loop now maintains the deletion count, a negated offset and a
derived destination pointer, and rechecks the gap and workload selector. These
are observed assembly differences, not a cycle estimate or timing verdict.
In std's full-range all-selected consumer, the workload selector is tested before
the loop, and read/deleted collapse into one counter. Its repeated path is only
load, increment, observation store, end comparison and backedge. The product
consumer retains the future compaction state even on this no-retention path.

The pinned std source (`alloc/src/vec/extract_if.rs`, Rust 1.95.0) increments
its read index after a successful predicate call, increments `del` only when
removing, and copies to `i - del` when retaining with a hole. This experiment
matches that deletion-count representation; its existing driver still retires
the read index within the selected/retained state methods. Disposable assembly
comparisons also tried a common read-index advance after the predicate, a flat
iterator body and raw private counters. None recovered the five-instruction
all-selected loop. Those copies have no product proof or acceptance claim;
the compiler controls below subsequently isolate a loop-unswitch cost decision.

## Why the checks remain: compiler control

The follow-up keeps the product source and the comparison consumer unchanged
and varies one LLVM loop-unswitch cost option at a time. Rust 1.95.0 / LLVM
22.1.2 emits the following repeated all-selected paths:

| Consumer compilation | Instructions / conditional branches | Gap/mode checks and future compaction maintenance |
| --- | ---: | --- |
| Default options | 12 / 3 | Remain in the repeated path |
| `-C llvm-args=-unswitch-threshold=1000` | 5 / 1 | Removed from that path |
| `-C llvm-args=-enable-unswitch-cost-multiplier=false` | 5 / 1 | Removed from that path |

The five-instruction path is load, read-index increment, observation store,
end comparison and backedge. The fixed mode test is before this loop, and
read/deleted collapse into one counter. No product branch, arithmetic guard,
callback contract or error path is deleted to obtain this result. The options
are analysis controls; product compiler defaults remain unchanged. These builds
have no new elapsed-time measurement or general performance acceptance.

LLVM's `SimpleLoopUnswitchPass` dumps provide the corresponding intermediate-code
evidence. With default options, the outer loop `%bb2.i27.lr.ph` is unchanged by
that pass after inner-loop versions have been created. With the cost multiplier
disabled, the pass transforms that outer loop, and final assembly has the
five-instruction selected path. The multiplier limits code growth from loop
cloning. These controls show that the repeated tests are not required by the
program's semantics; a cost policy prevents the default compiler from performing
this specialization. The precise instruction-by-instruction cost calculation
has not been reconstructed.

Here **mode** means the benchmark's fixed `kind == 1` dispatch, not the user's
per-element predicate call. The general API must still call a stateful/fallible
predicate once for every visited element. In the original driver the selected
branch returns before the gap test, so the source does not require a gap test
on a selected element either. The repeated selected-path gap test appears in
the transformed control flow.

Seven disposable source-expression controls try hoisting the gap Boolean,
explicit prefix/gap loops, admitted state transitions, a range loop, an explicit
loop exit, a direct decision callback and an untouched-state cleanup test. None
recovers this five-instruction path under default options. They are assembly-only
copies, not adopted or formally verified product changes. Those controls did
not recover the default-options selected loop.

The [typed scan follow-up](vec-extract-phase-types.md) applies const generics,
NonZero witnesses and selected-first entry in disposable library builds. Its
ten-instruction selected path trades against a seven-instruction retained prefix.
Native and executed memory checks pass. Twelve-process timing observes mixed
all-selected time 34.7% lower with a control-drift verdict, while mixed retained
prefixes are 6.2–9.2% slower with stable controls and adjusted bounds excluding
no change. Small all-selected remains 24.2% above std. The typed driver is not
adopted; product source remains this report's deletion-count baseline.

The subsequent [normal-inlining follow-up](vec-extract-inline.md) changes only
the private driver attribute and recovers the five-instruction selected loop
without LLVM cost overrides. Its twelve-process comparison confirms all-selected
time reductions of 23.0% and 41.5% versus this baseline, with no resolved mixed
retained-prefix change. Mixed parity is 5.0% slower than this baseline; all
twelve adjusted std +15% gates pass. Full scope and fresh checks are reported
separately.

A separate instrumented consumer checks complete retained contents and returned
consumer observations against std for 2,140 cases under each of default, higher
threshold and disabled-multiplier options. It exhausts parity patterns through
length eight and adds six input patterns at lengths 32, 256, 512 and 4096.
All three runs pass. This is a bounded smoke check in a different instrumented
program, not a codegen-equivalence or whole-library verification claim.

Raw pass dumps, assembly and smoke receipts are under ignored
`target/verification/extract-unswitch-20261117/`. To reproduce the compiler
control with the unchanged experimental runtime, first build the extraction
libraries. Then compile the existing consumer, adding one cost option for each
separate output directory:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
. /workspace/.devtools/activate.sh
python3 tools/throughput-analysis/prepare_range.py \
  --baseline 47a4f3f --benchmark extraction --profile default --build-only \
  --output target/verification/extract-unswitch-libs
mkdir -p target/verification/extract-unswitch-control
rustc --edition=2024 -O -C panic=abort -C codegen-units=1 \
  --crate-name extract_unswitch --emit=asm,llvm-ir \
  --out-dir target/verification/extract-unswitch-control \
  --extern jzap_old=target/verification/extract-unswitch-libs/libjzap_old.rlib \
  --extern jzap_new=target/verification/extract-unswitch-libs/libjzap_new.rlib \
  -C llvm-args=-enable-unswitch-cost-multiplier=false \
  tools/throughput-analysis/experiments/extraction.rs
```

Omit the cost option for the default control, or replace it with
`-C llvm-args=-unswitch-threshold=1000` for the threshold control. For pass dumps,
use `-C llvm-args=-print-before=simple-loop-unswitch` and the corresponding
`-print-after` option, filtering to the current consumer's LLVM symbol.

## Verification changes

All 31 existing closed-state methods retain direct contracts. The logical view
continues to expose the read/write cursor relation, with the new invariant
`deleted <= read <= end <= total`. Two additional solver controls omit deletion
retirement or incorrectly increment it on retention. The state runner now
requires eleven solver controls and the same five compiled scope controls.
The derived `written()` query and the cursor-equivalence contract of `untouched()`
now explicitly require the valid closed-state invariant; their direct caller
adapters require it too. Constructors and transitions still establish it, and
the arbitrary admitted cursor domain is retained.

The Kani loop-progress observer uses checked subtraction because a loop-invariant
expression must be side-effect free. Product `written()` retains its bounded
subtraction assertion. The first attempt produced a Kani loop-invariant diagnostic
and was interrupted; it is not counted as a pass. An earlier Verus attempt also
failed on an opaque-field reference in an open invariant; the invariant now
uses the closed logical view. The corrected focused source proofs pass.

After timing, the cleanup-length mutation anchor is updated from the old nested
cursor expression to `total - deleted`; the legacy native stop anchor now
targets `end = read`. A fresh four-group state/cursor/extraction replay passes,
including all seven native source mutations and three extraction Verus controls.
The targeted compiled stop mutant also fails the callback-error test as required.
That targeted execution is not a pass claim for the full legacy mutation runner.
Product runtime source stays identical to `047bcda` throughout timing and this
verification-tool maintenance.

The routing-only preflight observes 87 state writes, 132 constructor fields and
178 unsafe callers. Its use of previous theorem names is only a namespace and
source-shape check, not fresh solver or global certificate evidence. The previous
84/130 census changes because Extraction now publishes its own fields instead
of delegating through nested Compaction. Exact constructor and write expressions
remain checked separately for the two state types.

| Completed focused check | Result |
| --- | --- |
| Whole-library native/doctests | 433 pass in each of debug, release and overflow-checked release |
| Compiler and repository checks | Runner, formatting, Clippy, Rustdoc, feature builds, unsafe routing and compiler boundaries pass |
| Direct state methods | All 31 methods verify in both Verus word domains; actual Rust all-word state harnesses pass; eleven solver and five compiled scope controls are detected |
| Direct extraction controller | Both Verus word domains and actual Rust arbitrary admitted word states pass; three Verus and three native source controls are detected |
| Cleanup and cursor controls | Four native cursor source defects and the targeted compiled error-stop defect are detected |
| Actual iterator values | Every byte input, range, Boolean reply and error ordinal through length three passes against std and the returned-error contract |
| Vec executed memory checks | 41 pass in native strict-provenance Miri, i686 strict-provenance Miri, Tree Borrows and ASan |
| Vec fuzzing | 10,000 runs complete |
| Panic-abort compatible tests | Callback error, extraction ownership and std/order comparison: three pass |
| Allocation observation | 108 observations / 36 std-baseline-candidate triples match; no bytes held after cleanup |

Those ASan runs use `detect_leaks=0` after LeakSanitizer fails inside the
restricted execution-tool sandbox. The [current-runtime leak follow-up](vec-extract-leaks.md)
passes outside that sandbox without changing system ptrace settings; it does
not change these earlier runs' scope.
The first abort selection included the retained-prefix test's intentional
`panic!`/`catch_unwind` section and therefore failed under `panic=abort`. That
attempt remains recorded as failed. The test passes in the unwind and memory
suites; the three abort-compatible tests check returning-error behavior without
an intentional unwind.

## Reproduction

Raw assembly, logs and receipts remain ignored under
`target/verification/extract-deleted-count-20261117/`. On the experimental source:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
. /workspace/.devtools/activate.sh
python3 scripts/verify_source.py --group state --group vec-extraction \
  --group vec-extraction-values --group vec-cursors --mutations \
  --output target/verification/extract-deleted-source
XDG_CACHE_HOME=/workspace/.cache ASAN_OPTIONS=detect_leaks=0 \
  bash scripts/verify.sh --area vec --test-target vec --profile memory \
  --output target/verification/extract-deleted-memory
python3 tools/throughput-analysis/measure_vec_edit.py --baseline 47a4f3f \
  --output target/verification/extract-deleted-statistics \
  --rounds 4 --seeds 1,2,3 --target-seconds 2 --build-only
# Finish all verification and compilation before timing.
python3 tools/throughput-analysis/measure_vec_edit.py --baseline 47a4f3f \
  --output target/verification/extract-deleted-statistics \
  --rounds 4 --seeds 1,2,3 --target-seconds 2 --run-only
```

The statistical protocol, same-code control, minimum interval, fixed stopping
rule and interpretation follow [the preceding timing report](vec-extract-repeated-timing.md).
The baseline here is the selected-first candidate, rather than that report's
two-phase baseline.
