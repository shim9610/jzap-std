# Randomized collection throughput: total elapsed time

The [per-operation average report](throughput-averages.md) applies the confirmed
reporting standard: repeated intervals of at least one second, followed by
total-time/total-call normalization. The original totals below are retained.

The 2026-11-04 campaign uses **equal randomized work and total elapsed seconds**.
Every valid recording is at least one second. This is the primary timing format
requested by the user; earlier fixed-input per-call reports remain diagnostic
history. Product source is unchanged from the [preceding recheck](performance-recheck.md).

## Completed campaign

| Item | Recorded result |
| --- | --- |
| Workloads | 20 Vec and 11 Box tasks, plus three focused small-input Vec tasks |
| Compiler settings | Default, 32-byte and 64-byte loop alignment, equally applied to std and jzap |
| Inputs | 64 pre-generated shuffled cases per task/seed; three seeds: 617, 6203, 96011 |
| Main recording count | 1,584, in 102 fully checked seed groups |
| Valid recording duration | Minimum 1.022408 s; maximum 3.891593 s |
| Measured time / campaign wall time | 2,210.105 s / 2,420.163 s |
| Calibration / repeated short attempts | 3,055 calibration processes; zero short or incomplete measured attempts |
| Comparison output | 330 product conditions and 99 identical-std controls, each with three seed recordings |

Setup, RNG, source creation, two warmup epochs and final source teardown are
outside the timer. Each timed region runs a fixed number of complete epochs;
there is no clock read or RNG inside its operation loop. Within every seed,
input digest, call order, epochs and operation count match across every policy
and compiler setting. The same flags are used for std and jzap in each image.
The driver verifies these conditions before committing a result group.

Mixed Vec lengths span 0–131,072 u64 elements. Dynamic Box inputs span
0–1,048,576 bytes; strings contain random ASCII. A capped binary-exponent size
sampler includes explicit empty, one-element and maximum cases. Values, valid
edit ranges and replacement/resize parameters vary. Dedup inputs contain random
runs. These are declared synthetic distributions, not application-frequency
estimates. Operations are reported separately.

The tables sum the three matched seed recordings. The bracketed values are the
minimum and maximum seed ratios, **not confidence intervals**. These observations
do not replace the earlier bootstrap upper-95%-bound acceptance gate. Across
conditions, identical-std total ratios range from 0.855 to 1.130; this environment
still has meaningful timing variation despite long recordings and fixed affinity.

## Vec: default compiler setting

Each edit includes cloning the prepared source, editing and dropping the result.
`reserved` clears and refills reserved owners. `pop` consumes every value.
`extract_all` extracts every value in a randomly chosen valid subrange.
The default policy has neither an element limit nor a byte budget. The budgeted
column has an ample byte budget and no element limit. The CSV includes limit-only
and combined policies too; their limits permit the same successful work.

| Operation | Calls | std seconds | Default policy seconds | Budgeted seconds | Default ratio / seed range |
| --- | ---: | ---: | ---: | ---: | --- |
| grow | 204,288 | 3.796 | 3.769 | 3.827 | 0.993 [0.978, 1.013] |
| reserved | 209,536 | 4.148 | 4.064 | 3.645 | 0.980 [0.961, 0.990] |
| copy | 710,912 | 4.029 | 3.962 | 3.754 | 0.983 [0.891, 1.139] |
| clone | 703,744 | 4.149 | 4.180 | 3.944 | 1.007 [0.923, 1.164] |
| retain | 68,672 | 4.239 | 4.300 | 4.018 | 1.014 [0.949, 1.106] |
| dedup | 132,672 | 4.119 | 4.609 | 4.753 | 1.119 [1.021, 1.173] |
| drain | 911,936 | 4.257 | 4.100 | 4.319 | 0.963 [0.941, 0.997] |
| extract | 164,288 | 3.892 | 4.125 | 3.924 | 1.060 [0.990, 1.110] |
| splice | 314,560 | 3.983 | 4.188 | 4.669 | 1.051 [0.845, 1.349] |
| within | 849,344 | 3.829 | 3.934 | 3.655 | 1.028 [0.930, 1.125] |
| splice_unknown | 124,096 | 7.474 | 3.845 | 4.014 | 0.515 [0.286, 0.945] |
| splice_no_hint | 103,616 | 6.806 | 4.100 | 4.118 | 0.602 [0.393, 0.840] |
| extract_none | 832,384 | 4.211 | 4.070 | 5.862 | 0.966 [0.892, 1.015] |
| extract_all | 273,536 | 4.018 | 3.728 | 4.072 | 0.928 [0.875, 0.976] |
| insert_remove | 433,728 | 4.138 | 3.830 | 4.136 | 0.926 [0.892, 0.947] |
| resize | 541,504 | 4.211 | 4.693 | 4.618 | 1.114 [0.862, 1.466] |
| split_off | 337,216 | 4.169 | 4.276 | 4.253 | 1.026 [0.889, 1.129] |
| append | 52,416 | 4.171 | 4.387 | 4.204 | 1.052 [1.006, 1.115] |
| shrink | 176,384 | 3.925 | 4.304 | 3.881 | 1.096 [1.018, 1.153] |
| pop | 425,728 | 3.953 | 4.129 | 4.227 | 1.045 [0.994, 1.112] |

## Box: default compiler setting

Creation and clone rows include immediate destruction of each new owner.
`clone_from` overwrites prepared targets. This exercises a different allocation
lifecycle from the earlier batches that held many owners simultaneously.

| Operation | Calls | std seconds | Default policy seconds | Budgeted seconds | Default ratio / seed range |
| --- | ---: | ---: | ---: | ---: | --- |
| box_u64_create_drop | 396,816,448 | 3.877 | 3.463 | 5.022 | 0.893 [0.858, 0.923] |
| box_u64_clone_drop | 429,732,352 | 4.214 | 3.956 | 5.215 | 0.939 [0.883, 1.022] |
| box_u64_clone_from | 2,497,062,272 | 3.827 | 3.801 | 3.856 | 0.993 [0.944, 1.023] |
| box_slice_create_drop | 1,315,520 | 3.936 | 3.841 | 4.064 | 0.976 [0.934, 1.063] |
| box_slice_clone_drop | 952,448 | 4.247 | 4.043 | 3.872 | 0.952 [0.882, 1.033] |
| box_slice_clone_from | 1,279,680 | 4.234 | 4.125 | 3.944 | 0.974 [0.961, 0.992] |
| box_str_create_drop | 963,520 | 4.233 | 4.179 | 4.432 | 0.987 [0.816, 1.142] |
| box_str_clone_drop | 863,744 | 3.917 | 3.613 | 3.846 | 0.923 [0.832, 0.998] |
| box_str_clone_from | 875,968 | 7.894 | 3.806 | 3.905 | 0.482 [0.422, 0.532] |
| box_zeroed_create_drop | 2,104,896 | 3.932 | 3.901 | 3.851 | 0.992 [0.896, 1.059] |
| box_collect_create_drop | 1,098,112 | 4.171 | 4.225 | 4.061 | 1.013 [0.971, 1.089] |

## Focused small Vec inputs

These cases use sizes 0–64 so large copies cannot hide short-lifecycle costs.
All rows below use the default compiler setting and the same three seeds.

| Operation | Calls | std seconds | Default policy seconds | Budgeted seconds | Limit + budget seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| clone | 287,354,816 | 4.000 | 3.849 | 4.842 | 5.145 |
| extract_all | 152,864,640 | 3.686 | 4.744 | 4.447 | 4.904 |
| pop | 121,140,928 | 3.791 | 4.170 | 4.071 | 3.989 |

## Alignment and remaining overhead

The following counts use the observed sum-of-times ratio of 1.15 for triage;
they are not replacements for confidence-interval acceptance decisions.

| Input family | Compiler setting | Product conditions | Observed totals above 1.15 |
| --- | --- | ---: | ---: |
| mixed Vec | default | 80 | 4 |
| mixed Vec | align32 | 80 | 6 |
| mixed Vec | align64 | 80 | 6 |
| mixed Box | default | 22 | 2 |
| mixed Box | align32 | 22 | 2 |
| mixed Box | align64 | 22 | 2 |
| small Vec | default | 12 | 6 |
| small Vec | align64 | 12 | 9 |

Every compiler setting receives identical work, so its absolute totals can also
be compared with the other settings. A single setting does not win every task.
The library default remains unchanged; both alignment choices remain opt-in.

The disassembly below belongs to the actual measured images. Diagnostic copies
and follow-ups remain separate from the main 1,584 recordings.

### Dedup: additional work per input element

The std compaction loop processes two incoming elements per iteration. The
product compaction loop processes one and retains a `test write,write` guard
before its backedge. This comes from `Compaction::previous_kept()` returning
`None` at zero, although the dedup state invariant establishes a retained first
value. In the default image the product test/backedge is at `0x34cfb/0x34cfe`.
The std two-element duplicate path at `0x375e0` has nine executed instructions
for two duplicates; the product duplicate path has eight per duplicate.

The extra loop control is paid with input size. It is not one fixed charge for
an entire vector. Default unmetered dedup totals are 4.119 s versus 4.609 s;
align32 totals are 3.811 s versus 4.541 s. Source specialization and placement
also vary; the measurements do not assign an independent latency to each guard.
This remains an optimization target.

### Extract-none: placement-dependent accumulating cost

Default budgeted extraction has an 18-byte, six-instruction prefix loop starting
at `0x23ab0`, offset 48 within a 64-byte line. Its final compare occupies offsets
61–63, followed by the backedge on the next line. Align32 moves the same six
operations to offset 32. Align64 uses offset 0, with a longer selected-exit
branch encoding. Neither loop has a per-element budget check.

A diagnostic copy shifts only that prefix two bytes earlier within existing
padding and repairs relative branches, preserving instructions and registers.
For the same 832,384 calls, three matched seed totals are:

| Diagnostic condition | Total seconds |
| --- | ---: |
| std | 4.221 |
| Identical std control | 3.985 |
| Original product | 5.088 |
| Shifted product | 3.927 |

Every diagnostic interval also exceeds one second. This confirms a placement
penalty for this image; it does not isolate a particular CPU front-end mechanism
or turn a modified diagnostic into a product fix. The main campaign's original
budgeted total, 5.862 s, is retained separately.

### Pop: clone strategy differs while the pop body is equivalent

In all three compiler settings, std and the unmetered product use four-instruction,
15-byte pop bodies: load, output-barrier store, decrement and backedge. The
align32 bodies both start at offset 0 within a 64-byte line. A split pop-loop
boundary therefore does not explain its 1.257 lifecycle ratio.

The preceding clone differs. Std calls `memcpy`; the product emits a
seven-instruction SSE copy loop transferring 32 bytes per iteration, with a
scalar tail. `Vec::try_extend_cloned` initializes through individual `TryClone`
slots, which LLVM vectorizes, rather than taking std's bulk-copy specialization.
The measured workload includes that payload-dependent copy and allocation;
its whole elapsed difference must not be labeled an added per-pop check.
The separate effects of copy throughput and surrounding placement are not
individually timed in this campaign.

### Targeted controls: resize, dedup, full extraction and pop

A follow-up uses the original default executable, the same three seeds, corpora
and per-seed call counts. Each diagnostic copies the executable and changes only
the specified machine-code region; all later function and loop addresses stay
fixed. Rotating four-process groups contain std, identical std control, original
product and edited product. All 72 recordings exceed one second (minimum
1.229656 s); no compilation or verification runs concurrently. These are
successful `u64` diagnostic paths, not product changes or safety certificates.

| Diagnostic | Profile | std seconds | std control seconds | Original seconds | Edited seconds |
| --- | --- | ---: | ---: | ---: | ---: |
| Remove dedup's zero-written guard | Mixed | 4.484 | 4.311 | 4.782 | 4.904 |
| Remove extraction's post-yield untouched-state test | 0–64 | 4.444 | 4.111 | 5.165 | 5.324 |
| Replace clone copy with memcpy before resize | Mixed | 4.363 | 4.425 | 4.434 | 4.350 |
| Replace clone copy with memcpy before pop | Mixed | 4.601 | 4.320 | 4.474 | 4.392 |
| Replace outlined clone copy with memcpy before extraction | 0–64 | 4.142 | 4.285 | 5.384 | 5.403 |
| Replace clone copy with memcpy before pop | 0–64 | 4.185 | 4.295 | 4.614 | 4.180 |

Guard removal alone shows no improvement. Dedup still has a one-element scan
versus std's two-element scan; a private nonempty compaction state and two-element
unrolling are candidates to test together while preserving comparison, error and
destruction order. The guard's independent share of the original gap is unproven.

Resize's actual fill loop already matches std's five instructions per four u64
values. Its preceding clone uses the element-wise path described above. Changing
that copy to memcpy reduces pooled time by 1.9%, but individual seed changes run
from 17.9% faster to 19.8% slower. Mixed-size pop similarly changes by only 1.8%,
with inconsistent seed direction. Neither result establishes a repeatable gain
or explains the original whole-workload gap.

Small pop is the positive copy control: 121,140,928 calls take 4.613832 s before
and 4.180225 s after, or **38.086 → 34.507 ns per complete clone/pop/drop call**.
All three seeds improve (5.8%, 14.6%, 7.7%); the pooled reduction is 9.4%. The pop
body stays unchanged. This supports improving the primitive bulk-clone path for
this input family. A production path must retain fallible allocation and tracked
initialization, with the existing fallible per-element fallback for other types.

Small extraction retains a gap despite both isolated controls. Its product
clone is an outlined fallible function while std's preparation is inlined.
Clone call/result setup and extraction-state construction are candidates for
further isolation; their independent costs have not been established. A single
scan that tests for holes only when retaining a value is another candidate, but
must preserve the fast no-selection prefix, callback errors and suffix cleanup.
Mixed-size full extraction was already faster in the original unmetered sample.

The follow-up therefore identifies a small-pop copy improvement and concrete
code-generation differences, not a universal cause or an accepted product fix.
Local region bytes, binary hashes, per-seed totals and qualification checks are
under ignored `target/verification/vec-optimization/`; raw dumps stay out of Git.

### Scalar budgeted Box: fixed bookkeeping per owner

The successful budgeted create/drop path has nine explicit accounting/control
instructions beyond the allocator/payload/free work: load available, compare,
reject branch, decrement and store available, increment occupied, then a vector
load/add/store to release occupied bytes and restore available bytes. These
instructions are visible at `0x210f8`–`0x2110b`, `0x2112f`, and
`0x21150`–`0x2115f` in the default image. The heap allocator and deallocator calls
remain present in both std and product kernels.

This is a cost per allocated owner. It is visible when creating/dropping
396,816,448 scalar owners: 3.877 s std versus 5.022 s budgeted. Nonallocating
scalar replacement totals 3.827 s std versus 3.856 s budgeted across
2,497,062,272 calls. It is not a universal proportional slowdown of all Box work.
The accepted extra handle bytes do not exempt this timing result.

### Unreproduced small-clone recording

The align64 unmetered small-clone result includes one seed-6203 recording of
3.282472 s versus std 1.310213 s. The other two seed ratios are near parity.
A follow-up repeats the exact image, seed and 98,186,688-call count three times,
rotating/reversing all six labels. Unmetered durations are 1.245–1.298 s;
std durations are 1.312–1.368 s. Child CPU usage closely follows wall time in
these replays, with no major page faults. The original recording had no CPU-time
receipt, so its root cause remains unclassified. It is retained in the original
total and CSV, not silently replaced with the replays.

## Verification and reproduction

The tool's Rust tests pass three differential/replay tests, including all four
Vec policies, removed-element order and initialized Box values. Python tests
pass three comparison-contract tests, including changed-input/count, missing,
duplicate and subsecond negative controls. All 133 native CLI smoke cases use
matching corpus digests and work counts and complete source/budget cleanup.
Formatting and Clippy pass. The unchanged product inputs still match the
completed [Plan A certificate](unsafe-proof-plan.md).

Run commands and the full workload definition are in
[the reusable runner](../../tools/throughput-analysis/README.md). Generated data
are under ignored `target/verification/throughput-long/`: `protocol.json`,
`records.jsonl`, `completed.json`, `summary.json`, `summary.csv`, `integrity.json`,
and separate diagnostic/replay receipts. The CSV contains all 429 policy,
setting and workload summaries, including std controls. Allocation measurements
remain those of the separate earlier memory matrix; this timing campaign adds
no memory-instrumented random cases.

Host: AMD EPYC 9V74 KVM guest, Linux x86-64, CPU 0; Rust 1.95.0 / LLVM 22.1.2,
opt-level 3, one codegen unit, panic=abort, no LTO. Product baseline is commit
`535b3583c1d84cb4d0814cbed7665dce107b0a86` plus the working tree. Benchmark input
fingerprint (including the runner) is
`bc81887f5abad136a7b603a125eb6b64e76719a3ce3fea3ce9b5c64f4fb5601b`.
No new product dependency or runtime source change was introduced.
