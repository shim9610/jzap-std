# Box and Vec performance recheck

The [long-interval randomized follow-up](throughput-long.md) records the user's
requested equal-work comparisons in total seconds. The per-call measurements
below describe the preceding fixed-input campaign and its code-generation analysis.

Measured on 2026-11-04 against Rust 1.95.0 / LLVM 22.1.2 std, on an AMD EPYC
9V74 Linux x86-64 KVM guest, CPU 0. Baseline commit `535b3583c1d84cb4d0814cbed7665dce107b0a86`
plus the working tree. **Box passes the measured workloads after the collection
fix below. Vec still does not pass the complete performance gate.**

## Decisions

Product comparisons exclude identical std controls and reference-only cases.
Pass requires the upper bootstrap 95% time ratio to be at most 1.15; fail means
the lower bound exceeds 1.15. All other intervals are inconclusive. The identical
std control must also pass. No threshold was changed after measurement.

| Build / workload | Pass | Fail | Inconclusive | Identical std controls |
| --- | ---: | ---: | ---: | --- |
| Vec, default loop alignment | 198 | 30 | 12 | 60/60 pass |
| Vec, 32-byte loop alignment | 207 | 26 | 7 | 60/60 pass |
| Vec, 64-byte loop alignment | 214 | 13 | 13 | 60/60 pass |
| Dynamic Box, default and budgeted | 96 | 0 | 0 | 48/48 pass |
| u64 Box, default and budgeted | 12 | 0 | 0 | 6/6 pass |

With 64-byte alignment, all 160 Vec comparisons at 4,096 and 131,072 elements
pass. The 13 failures and 13 inconclusive comparisons are at 32 elements.
Failed small cases add 3.23–9.44 ns per measured lifecycle. This is not a claim
that every operation has a universal fixed overhead: in particular, retain
still measures about 4% slower at 4,096 elements in this build.

## Box collection fix

The first recheck failed collection of 32 bytes. Generated code kept a scalar
tail of 1–32 bytes, including 32 byte-at-a-time iterations for a 32-byte source.
The private `Take` counter and source-end check gave LLVM a trip-count expression
based on `length - 1`; the vector path required at least 33 bytes. At 4 KiB the
same default-policy collection added 13.25 ns, rather than doubling total time.

Collection now checks its private reserved end directly before calling
`try_next`, writing and publishing the checked successor. Growth stays outside
that span. Initial and later spans remain independent of dishonest size hints
and overridden user folds. The actual 32-byte creation paths now call `memcpy`
with length 32 for std, default and budgeted ownership; their scalar byte loops
are absent.

| 32-byte collection, per owner | std ns | jzap ns | Ratio, bootstrap 95% interval |
| --- | ---: | ---: | --- |
| Initial default policy | 14.073 | 27.643 | 1.9643 [1.9336, 2.0497] |
| Initial budgeted policy | 14.073 | 28.669 | 2.0372 [1.9966, 2.1076] |
| Corrected default policy | 13.178 | 13.543 | 1.0277 [0.9894, 1.0483] |
| Corrected budgeted policy | 13.178 | 13.580 | 1.0305 [0.9995, 1.0465] |

Both campaigns retain every sample and use 30 processes per condition. After
the fix, all 144 dynamic time comparisons, including controls, pass. The largest
product point estimate is 1.0933 [1.0909, 1.0989], for equal-length replacement
of 32-byte zeroed slices. The u64 timing executable and all Vec executables are
byte-identical after rebuilding the final source; their initial samples remain
applicable. The changed dynamic Box images were measured again in full.

## Vec: placement and short lifecycles

These are complete clone/edit/drop lifecycles. `extract_all` removes the middle
half of the input; `pop` clones its source and pops every value. They are not
isolated single-method latencies. Both implementations receive the same flags.

| Workload, policy, N | Default: std / jzap ns | Align32: std / jzap ns | Align64: std / jzap ns |
| --- | ---: | ---: | ---: |
| retain, default, 4,096 | 1,639.93 / 2,823.53 | 1,642.20 / 1,710.03 | 1,641.30 / 1,709.25 |
| extract_all, budgeted, 4,096 | 1,731.94 / 2,899.41 | 1,729.91 / 2,367.57 | 2,285.23 / 2,334.16 |
| clone, limit + budget, 32 | 11.00 / 17.55 | 11.10 / 17.35 | 11.37 / 16.94 |
| pop all, limit + budget, 32 | 29.75 / 39.05 | 30.86 / 39.12 | 30.71 / 29.12 |

The default retain compaction path occupies addresses `0x40c50` through
`0x40c81`. Its final `test sil,1` occupies 64-byte offsets 60–63 and the taken
`jne` starts at offset 0 of the next line. The corresponding std path starts
at `0x343c0`, with its selected path fitting in one line. Both process two
input elements per iteration; the product loop has no per-element budget or
limit check. Register allocation uses `sil` rather than std's `dl`, adding a
REX prefix to byte tests. Here the test and branch are on opposite sides of the line.

A diagnostic copy moves the 59-byte compaction block two bytes earlier inside
existing padding and repairs its relative branches. Every instruction, register
and branch destination under relocation is preserved; other code stays in
place. In a separate rotating-order 30-process comparison, all three identical
std controls pass:

| N | std ns | Original jzap ns | Shifted diagnostic ns | Shift / original, 95% interval |
| ---: | ---: | ---: | ---: | --- |
| 32 | 21.959 | 27.907 | 21.145 | 0.7577 [0.7236, 0.7815] |
| 4,096 | 1,640.906 | 2,828.723 | 1,719.027 | 0.6077 [0.6044, 0.6100] |
| 131,072 | 58,258.875 | 99,193.250 | 60,085.938 | 0.6057 [0.5942, 0.6248] |

This establishes a placement-dependent accumulating loss with the current
instruction sequence. It does not isolate a particular CPU front-end mechanism.
The patched executable is a diagnostic control; product acceptance above uses
the three unmodified compiler outputs.

The pop bodies are four instructions, 15 bytes, in all five owner variants:
load, output-barrier store, decrement, backedge. Surrounding clone code differs:
std inlines allocation and copying, while several product paths call
`TryClone::try_clone`, check the returned tag and reload the owner from its
result storage. Capacity checks and optional budget bookkeeping also occur
outside the pop loop. These observations identify extra work without assigning
an independently measured latency to each instruction.

Alignment remains opt-in through `.cargo/loop-align32.toml` and
`.cargo/loop-align64.toml`. The table shows that alignment can also slow std.
No workload-frequency estimate or universal fastest setting is inferred.
The Vec executable's text section is 490,435 / 498,195 / 513,235 bytes for
default / align32 / align64, respectively: the latter adds about 4.65% text.

## Allocation and verification

Vec passes 21,060 assertions across 1,620 cases: 18 operations, nine sizes,
five owner representations and native/moving reallocators. Requested and glibc
usable live/peak payload bytes never exceed std in this matrix. Final tracked
requested and usable balances are zero. Handles are 24 bytes for std/default,
32 for a limit or budget, and 40 for both; handle costs are separate.

Dynamic Box passes 360 byte/handle comparisons, 72 final-zero checks and
72 allocation-call comparisons. Budgeted ownership includes the accepted
additional 8 bytes per handle. These are allocator measurements, not RSS.

The collection proof now binds both actual private span guards, checked
successors, writes and publications. Two additional compiled-source controls
change `<` to `<=` in the initial and growth spans; both are rejected.
The fresh [Plan A certificate](unsafe-proof-plan.md) covers the final source.

| Final-source regression check | Result |
| --- | --- |
| Combined source/guard certificate | Complete; 455 connections, zero open; 400 native and 190 32-bit Verus results, zero errors; all 14 source controls detected |
| Unit, property and documentation tests | 154 pass normally; 154 pass in overflow-checked release |
| Miri / Tree Borrows with strict provenance / AddressSanitizer | 119 distinct tests pass in each mode |
| Formatting, Clippy and whitespace checks | Pass |

## Reproduction and evidence

Each Vec and dynamic Box condition uses 30 independent processes, rotating and
reversing execution order, two warmups and 30 retained batches. u64 Box uses the
existing 15-process protocol. Bootstrap uses 5,000 resamplings, seed 617.
The initially inconclusive large-retain std controls for align32/align64 were
extended to 90 processes for all six labels, retaining the initial 30 samples.
Their combined results appear above. Intervals apply per comparison.

After building the default timing and memory images, reproduce its matrix with:

```sh
python3 scripts/compare_vec.py --trials 30 --sizes 32,4096,131072 \
  --ops grow,reserved,copy,clone,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all,insert_remove,resize,split_off,append,shrink,pop \
  --memory-ops grow,copy,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all,insert_remove,resize,split_off,append,shrink,pop \
  --output target/verification/vec-recheck
```

For each alignment setting, build with `cargo --config .cargo/loop-align32.toml`
or `cargo --config .cargo/loop-align64.toml`, using a separate target directory;
select that executable using `--timing-binary`. This applies the option equally
to std and jzap. Run `scripts/compare_box_dynamic.py --trials 30` and
`scripts/compare_box_analysis.py` after building their timing/memory images.
The analysis-only pinned Varlit fixture is described in
[the Box methodology](../benchmarks.md). Compilation, verification and timing
are sequential.

Local protocols, binary hashes, raw samples and receipts are under ignored
`target/verification/performance-recheck/`. Final product source digest,
excluding proof-only Rust files:
`06481ca65645f07bf59570d8e60af09760cf037037da9ec0f4619e6a34cdcbe2`.
