# Expanded Vec benchmarks and placement controls

Tool checkpoint: `153784073b0fdbd22f015c1cf5d9e8a32a22926a`. Product implementation remains
`37beeb3598e8ac2d94756fcd77644b1adf29d3a9`; this experiment changes benchmark
coverage, not product source. Rust 1.95 / LLVM 22.1.2, Linux x86-64 AMD EPYC 9V74
KVM guest, CPU 0; release opt-level=3, one codegen unit, panic=abort, no LTO or
extra alignment flags. The benchmark executable changed, including code placement.

## Current decision

Across 20 workloads, three sizes and four policies, **233/240 product comparisons
pass, six fail and one is inconclusive**. All 60 identical std controls pass.
The 17-workload memory matrix passes all 19,890 assertions across 1,530 cases.
Overall Vec performance acceptance remains open.

Each condition starts with 30 independent processes, alternating implementation
order, two warmups and 30 retained cycles. Retain at all three sizes and split_off
at N=32 were extended to 90 processes without discarding the initial observations.
The original large-retain std control was inconclusive; its 90-observation result
passes. The upper bootstrap 95% ratio threshold remains 1.15; intervals are
per comparison, not a simultaneous or cross-platform guarantee.

The table shows the largest median ratio for each workload across sizes/policies.
All rows are complete ownership lifecycles; edits include source cloning and
destruction. In particular, pop is **clone → pop every value → drop**, not one pop.

| Workload | N | Policy | std ns | jzap ns | Ratio (95% interval) | Decision |
| --- | ---: | --- | ---: | ---: | --- | --- |
| grow | 131,072 | metered | 116987.062 | 122998.500 | 1.0514 (0.9943–1.0962) | pass |
| reserved | 4,096 | limited_metered | 3451.953 | 3689.371 | 1.0688 (1.0373–1.0864) | pass |
| copy | 32 | metered | 14.669 | 16.289 | 1.1105 (1.0898–1.1145) | pass |
| clone | 32 | limited_metered | 11.154 | 12.159 | 1.0901 (1.0697–1.0963) | pass |
| retain | 4,096 | limited | 1641.453 | 2753.961 | 1.6778 (1.6746–1.7141) | fail |
| dedup | 32 | limited_metered | 44.071 | 45.375 | 1.0296 (0.9868–1.0762) | pass |
| drain | 32 | limited_metered | 15.736 | 16.325 | 1.0375 (1.0139–1.0612) | pass |
| extract | 32 | limited_metered | 20.361 | 21.427 | 1.0524 (1.0307–1.0772) | pass |
| splice | 32 | limited_metered | 44.943 | 48.878 | 1.0876 (1.0672–1.1329) | pass |
| within | 32 | limited_metered | 35.720 | 38.190 | 1.0691 (1.0361–1.0880) | pass |
| splice_unknown | 131,072 | unlimited | 85856.562 | 53292.125 | 0.6207 (0.6016–0.6322) | pass |
| splice_no_hint | 131,072 | limited_metered | 87747.375 | 53378.938 | 0.6083 (0.5929–0.6197) | pass |
| extract_none | 32 | limited_metered | 20.723 | 15.174 | 0.7322 (0.7280–0.7378) | pass |
| extract_all | 32 | unlimited | 20.693 | 24.983 | 1.2073 (1.1827–1.2261) | fail |
| insert_remove | 32 | limited_metered | 37.462 | 41.500 | 1.1078 (1.0667–1.1298) | pass |
| resize | 131,072 | limited | 39718.250 | 39706.250 | 0.9997 (0.9985–1.0004) | pass |
| split_off | 32 | limited_metered | 22.446 | 26.072 | 1.1615 (1.0934–1.2000) | inconclusive |
| append | 32 | limited_metered | 58.283 | 62.564 | 1.0735 (1.0363–1.1097) | pass |
| shrink | 32 | limited_metered | 48.479 | 50.521 | 1.0421 (1.0143–1.0917) | pass |
| pop | 32 | limited_metered | 27.610 | 36.284 | 1.3142 (1.2888–1.3506) | fail |

The six failures are limited retain at all three sizes, unlimited and limited
extract_all at N=32, and limited/metered pop at N=32. Limited/metered split_off at
N=32 remains inconclusive after 90 observations. Newly added workloads contribute
70/72 product passes, one failure and one inconclusive result.

## Added work and memory scope

Insertion/removal inserts one value in the middle then removes it. Resize doubles
the length using a scalar value. Split-off moves the second half to a new owner,
then drops that owner. Append clones a second source inside the timer and appends
it. Shrink truncates to half length and shrinks storage to fit. Pop consumes every
value through black_box. A maintained differential test checks the six edits at
six sizes, including zero, under all four policies (144 cases).

Memory uses a separate hooked allocator binary, nine sizes from zero to 131,072,
and native/moving realloc modes. Requested and glibc-usable live/peak bytes,
cumulative requests, allocation calls and final ownership are compared directly
with std. Every final tracked balance is zero. These are allocator observations,
not RSS. The handles remain 24/32/32/40 bytes for default/limited/metered/both;
those sizes are reported separately from payload memory.

## Retain: confirmed proportional placement penalty

The expanded binary's limited retain case is 2,753.961 ns versus std 1,641.453 ns
at N=4,096 (90 observations). The input alternates retained and removed u64 values.
The limit check is outside the compaction loop; no per-element limit check was added.

The hot compaction path starts at `0x28950`. Its taken backedge at `0x2897f` is
encoded `75 cf`, spanning offsets 63 and 64 across an aligned 64-byte region.
The executed path covers 49 bytes from the loop header through that branch.
A copied executable moves the whole 58-byte compaction block one byte earlier
into existing padding and repairs relative branches. It preserves operations,
registers and all unrelated addresses. The hot path then fits through offset 63.

In a separate paired 30-process control, N=4,096 changes from 2,749.660 ns to
1,626.391 ns; the same std code measures 1,641.223 ns. The ratio changes from
1.6754 (1.6732–1.6808) to 0.9910 (0.9898–0.9919). The paired reduction is
1,123.270 ns (1,120.953–1,132.773), about 0.274 ns per input element.
At N=32 the reduction is 7.393 ns; at N=131,072 it is 38,549.625 ns, with wider
large-input variation. This is a layout-dependent accumulating loss, not a
universal small fixed charge. Moving the block establishes placement causality;
it does not isolate a particular CPU cache/decoder mechanism or prove that the
split branch alone is the cause. Hardware PMU counters remain unavailable.

The diagnostic executable is not a product fix. The current default binary still
fails these retain conditions. The earlier extraction-prefix change does not
remove layout sensitivity from all Vec loops or call contexts.

## Pop and short lifecycle costs

At N=32, limited/metered pop measures std 27.610 ns versus jzap 36.284 ns
(+8.674 ns, ratio 1.3142). At N=4,096 it is 1,714.969 versus 1,717.539 ns
(+2.570 ns, ratio 1.0015). At N=131,072 it is 57,851.312 versus 58,623.000 ns
(ratio 1.0133, upper interval 1.0371). This does not show a 31% per-element penalty.

Both compiled pop loops execute four instructions per value: load, black_box
store, decrement, backedge. The std loop is 15 bytes; the limited/metered loop is
16 bytes at offset 48, ending at offset 63. It does not cross the 64-byte boundary
that triggered the retain control. Additional clone validation, budget reservation,
occupancy/release updates and register spills are outside that loop.

A second copied executable replaces just four budget counter writes with
same-width NOPs, keeping addresses fixed. This deliberately disables accounting
for that successful cloned-owner lifecycle and must never be shipped. At N=32 it
reduces 36.446 ns to 32.618 ns: paired change −3.828 ns (−6.468 to −2.306).
At N=4,096 the reduction is 13.023 ns; the large-input change is inconclusive.
These controls establish a contribution from counter updates and their dependent
execution, not a universal nanosecond cost or an explanation of the entire gap.
The remaining checks/spills and placement are not individually timed.

Both split_off lifecycles include a second allocation; jzap also performs that
owner's budget reservation/release; its N=32 difference is +3.626 ns after 90 observations.
It is not promoted to a pass while its interval straddles the threshold.
The two N=32 extract_all failures are +4.290 ns and +3.665 ns; their larger-size
comparisons pass. The earlier fixed-wrapper analysis and new placement controls
explain why a previous passing binary cannot certify a newly linked one.

## Reproduction

```sh
cargo test --locked --manifest-path tools/vec-analysis/Cargo.toml --bin jzap-vec-analysis
cargo build --locked --manifest-path tools/vec-analysis/Cargo.toml --release
cargo build --locked --manifest-path tools/vec-analysis/Cargo.toml --release --bin jzap-vec-analysis --features memory --target-dir tools/vec-analysis/target/memory
python3 scripts/analyze_vec_codegen.py
python3 scripts/compare_vec.py --trials 30 --ops grow,reserved,copy,clone,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all,insert_remove,resize,split_off,append,shrink,pop --memory-ops grow,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all,insert_remove,resize,split_off,append,shrink,pop
```

Compilation and verification finish before timing. The command returns nonzero
for failures or inconclusive results. Use compare_vec.py's --extend option to
retain previous observations when extending a matching condition/binary.
Current hashes and compact decisions are in [summary.json](summary.json).
The address-specific binary controls, logs and samples remain local under
ignored target/verification/vec-expanded; they are not required fixtures for the
maintained tests. Product-source changes remain in Git history.
