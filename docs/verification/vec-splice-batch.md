# Batch staging for Vec Splice

After filling the original hole and admitted owner space, Splice collects any
remaining replacement items into a temporary Vec. The previous controller
checked staging capacity after each item. The new controller makes one owning
push, fills the admitted spare span using the existing initialization cursor,
then checks the new length and capacity before continuing. It keeps allocation
growth, source order, the retained tail and the error payload. The temporary
Vec drops the completed staged prefix if a source or allocation error interrupts
the operation.

This ports the batching strategy from `468477a` to the current source with
direct Verus contracts, actual Rust checks and source routing for the new fill
call. The verified loop covers both native word widths and arbitrary callback
replies. The caller proof composes reservation, the first write and the bounded
fill. The existing initialization guard publishes the prefix on return or unwind.

## Verification

- All 434 native tests pass, along with format, Clippy and the unsafe census. The
  census routes 582 unsafe blocks, one more than the scalar version.
- Vec release, overflow-checked release, strict-provenance Miri and ASan with
  leak detection each pass 58 tests. Four focused Splice scenarios also pass
  on i686 Miri and Tree Borrows. The new source-error scenario checks exact
  source consumption, original tail and prefix retention, and one drop per item.
- The staging group passes direct Verus and three actual Rust Kani harnesses.
  The owner composition has 533 Verus obligations with zero errors and detects
  all 53 controls, including a changed batch fill extent. The audited inventory
  contains 87 state writes, 132 constructor fields and 179 unsafe callers.
- The fresh global source/link certificate passes at `42afdab` without model
  reuse. Four build profiles account for 89 originating guards and 12
  forwarding calls, with zero open source obligations and zero unproved link
  edges. All eight injected exit controls are detected. The certificate and
  focused logs are under `target/verification/splice-batch/`.

## Repeated timing

Compared with scalar baseline `d9ac1a2` and std in one consumer executable,
unmetered unlimited `Vec<u64>`, with the original input clone, removal
consumption, replacement, result observation and drop inside each call.
Rust 1.95.0 and LLVM 22.1.2 use default alignment, `-O`, `panic=abort` and
one codegen unit. The cloud host reports AMD EPYC 9V45, pinned logical CPU 0.
Small input lengths are 0–32 and mixed input lengths are 256–4096.

Three seeds and four rounds produce 144 matched groups and 576 retained
intervals lasting 1.669–4.102 seconds, with 1,441.236 timed seconds and
23,886,028,800 calls. All 12 same-code controls pass. Means weight each
observation by calls. The [compact results](vec-splice-batch-timing.csv)
include wall time, thread CPU time and the adjusted paired bounds for the
24 primary wall-time comparisons.

| Condition | std ns | Scalar ns | Batch ns | vs scalar | vs std | std +15% verdict |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Delete, small | 16.961 | 21.492 | 21.617 | +0.58% | +27.45% | Above |
| Delete, mixed | 576.698 | 562.010 | 576.930 | +2.65% | +0.04% | Within |
| Shorter, small | 17.853 | 22.465 | 23.154 | +3.06% | +29.69% | Above |
| Shorter, mixed | 660.512 | 678.516 | 660.369 | -2.67% | -0.02% | Within |
| Exact, small | 17.698 | 22.256 | 22.757 | +2.25% | +28.58% | Above |
| Exact, mixed | 707.308 | 722.949 | 704.171 | -2.60% | -0.44% | Within |
| Longer with hint, small | 51.320 | 55.884 | 58.629 | +4.91% | +14.24% | Unresolved |
| Longer with hint, mixed | 807.724 | 819.638 | 816.258 | -0.41% | +1.06% | Within |
| Longer with zero hint, small | 84.381 | 59.898 | 59.144 | -1.26% | -29.91% | Within |
| Longer with zero hint, mixed | 1,476.716 | 906.857 | 885.091 | -2.40% | -40.06% | Within |
| Large excess with zero hint, small | 202.374 | 211.199 | 188.515 | -10.74% | -6.85% | Within |
| Large excess with zero hint, mixed | 11,825.796 | 15,085.254 | 11,171.216 | -25.95% | -5.54% | Within |

Mixed large excess improves 25.95% from scalar, with adjusted bounds
−28.30% to −23.87%. It is 5.54% faster than std, with bounds −9.44% to
−1.48%. Small large excess improves 10.74% from scalar and is 6.85%
faster than std. The generated batch fill processes four `u64` values per
vector iteration using two stores. The fixed staging-allocation schedule
matches scalar in all 36 observed pairs, including calls, held/peak requested
and usable bytes, and final release. The separate instrumented entry check
confirms only the large zero-hint cases with positive input length enter staging
and compares complete output contents in all 72 observations.

Small delete, shorter and exact replacement still exceed the std margin by
about 5 ns per call. Small longer replacement with a known hint takes 2.75 ns
more than scalar, with adjusted bounds +1.43% to +8.65%. Its std margin remains
unresolved. This case skips staging in the entry check. The linked wrappers
have the same size and the generated instruction sequences match after
normalizing labels and call symbol names. The hardware cause of its elapsed
difference was not isolated.

## Reproduction

Use the current checkout and the documented [tool requirements](README.md#tool-requirements).
The [snapshot timing commands](../../tools/throughput-analysis/README.md#snapshot-vec-timing)
measure current Splice behavior against std and establish a baseline for later
changes. The table above records the earlier batch-versus-scalar experiment.

```sh
bash scripts/verify.sh --area vec --check tests --check release --check overflow
python3 scripts/verify_source.py --group vec-staging --mutations
python3 scripts/verify_owner_protocol.py --mutations
bash scripts/verify.sh --area all --profile full --check certificate
```
