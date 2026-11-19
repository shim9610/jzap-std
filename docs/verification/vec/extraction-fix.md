# Batched untouched-prefix extraction

This report describes the first post-fix benchmark layout. See the
[expanded benchmark report](expanded-benchmarks.md) for the current acceptance
decision after adding other workloads to the executable.

Source: `37beeb3598e8ac2d94756fcd77644b1adf29d3a9`. The source change removes the long scalar scan responsible
for the measured no-selection regression. It uses ordinary Rust and the default
compiler options; it adds no runtime dependency, alignment flag, assembly,
iterator field or allocation. Indices remain full-width `usize`.

## Implementation

The untouched prefix shares a range check across four sequential predicate
calls. Each completed call immediately commits its own cursor update; selection
or error returns immediately. No later predicate is evaluated after a yielded
item or an error. A scalar loop handles the remaining zero to three values.
Thus the former 17-byte scalar path cannot accumulate its penalty over an
arbitrarily long untouched prefix.

The invariant is `write <= read <= end <= original`. Testing `read <= write`
expresses the untouched state using that ordering; under the invariant it is
equivalent to equality. This changes the optimizer's handling of the scan and
compaction paths without adding state. Four-way batching with the former equality
test, and outlining the scan into a separate function, were rejected because
other selection paths regressed. The final implementation keeps compaction
inlined and makes the cursor ordering explicit.

The shared check `end - read >= 4` guarantees that each of the next four visits
is in bounds and that incrementing `read` cannot overflow. Callback order,
callback mutations, partial consumption, error fusion and Drop restoration are
unchanged. This is not a guarantee that every compiled loop is at most 16 bytes:
it reduces repeated range-check work and bounds the residual scalar scan.

In the measured phase executable, LLVM emits 18 instructions for four retained
values in the steady-state batch loop, compared with six instructions per value
in the original scalar loop (24 per four values). The batch has four predicate
comparisons, one range comparison and one loop backedge per four values. The
original had a range comparison and a loop backedge per value. These counts
exclude entry/exit and apply to the inlined known-Ok benchmark predicate; arbitrary
callbacks may generate different code. The change improves the instruction work
as well as changing placement, without claiming immunity to future layout effects.

## Measurements

Rust 1.95.0 / LLVM 22.1.2, x86-64 AMD EPYC 9V74 KVM guest, CPU 0; release
optimization level 3, one codegen unit, panic=abort, no extra alignment options.
Each condition uses 30 independent processes, two warmups and 30 retained cycles.
These are extraction-only pool timings for default `Vec<u64>`, a known-Ok
predicate, and the middle half of the vector. Selection 50% is a contiguous
prefix. Setup and validation are outside the timer.

| N | Selected | Previous jzap ns | Current jzap ns | Current std ns | Current jzap/std (95% interval) |
| ---: | ---: | ---: | ---: | ---: | --- |
| 32 | 0% | 10.724 | 5.939 | 7.765 | 0.7648 (0.7619–0.7770) |
| 32 | 50% | 10.459 | 10.464 | 10.762 | 0.9723 (0.9635–0.9763) |
| 32 | 100% | 10.048 | 11.542 | 11.822 | 0.9763 (0.9748–0.9783) |
| 4,096 | 0% | 1126.586 | 414.062 | 601.273 | 0.6886 (0.6869–0.6928) |
| 4,096 | 50% | 1102.336 | 1094.594 | 1043.562 | 1.0489 (1.0474–1.0498) |
| 4,096 | 100% | 959.242 | 1217.586 | 1219.078 | 0.9988 (0.9970–0.9999) |
| 16,384 | 0% | 4477.562 | 1557.594 | 2306.812 | 0.6752 (0.6720–0.6775) |
| 16,384 | 50% | 4614.938 | 4625.594 | 4422.812 | 1.0458 (1.0439–1.0485) |
| 16,384 | 100% | 4036.625 | 5122.000 | 5133.875 | 0.9977 (0.9965–0.9992) |

Both implementations use the same options within each executable. Adding code
can move std's machine-code addresses too, even though its source is unchanged.
The previous all-selected jzap kernel was faster in absolute time; that advantage
is not preserved in these new layouts. The current default kernel stays within
5.1% of its std control for the displayed mixed-selection cases and is approximately
equal for full selection. Full-vector and tail-range controls also passed the
15% bound; their largest upper interval bound was 1.0565. Phase times must not be
added to reconstruct the separate clone/edit/drop lifecycle.

The full default-build lifecycle matrix has **167/168 product passes and 42/42
std-control passes**. All 12,870 memory assertions across 990 cases pass.
The original unlimited/unmetered N=32 full-extraction case now measures std
20.584 ns versus jzap 19.547 ns: ratio 0.9496 (0.9318–0.9632).

One condition remains above the unchanged bound: **N=32, limited and metered,
clone → extract the middle half → drop**, std 20.584 ns versus jzap 24.702 ns,
ratio 1.2001 (1.1677–1.2253), difference 4.118 ns. An additional size sweep found
N=16 at +2.626 ns / 1.1620, and inconclusive N=128 limited/metered clone and
extraction comparisons. The latter extraction difference was +7.354 ns, so the
remaining cost is not reported as universally bounded by 4–5 ns.

Assembly places the extra length-limit test, budget reservation/occupancy
updates and release bookkeeping outside the extraction scan. The limited/metered
extraction difference is +2.605 ns at N=256, +3.961 at 512, +2.465 at 1,024,
+4.453 at 4,096 and +3.156 at 16,384. This does not reproduce the previous
approximately 86% growing scan penalty. The precise share of time due to each
check versus code placement remains unisolated; the whole 4.118 ns is not
assigned to budget checks alone. **Overall Vec performance acceptance remains
open**, rather than treating this source improvement as a pass for every policy.

A prior symmetric alignment control on the previous source used
`-C llvm-args=-align-loops=32` for both implementations. All 549 loop-alignment
directives in 70 lifecycle specializations were `.p2align 5`; both phase kernels
also used `.p2align 5`. The original N=32 default extraction lifecycle passed
(0.9291, interval 0.9040–0.9898), but limited/metered splice remained above the
bound after 90 observations (1.1793, 1.1585–1.2182). A paired default/aligned
splice control could not resolve the alignment flag's contribution to that gap
(change +0.594 ns, interval −2.029 to +2.446). This control established that
symmetric padding is possible, not that a global flag solves every operation.

## Verification

The 38 Vec tests, three Vec properties, full repository tests, both Miri borrow
models, ASan, panic=abort failure returns, small-stack cases, 10,000 fuzz inputs,
portability builds, no-global-OOM build, Verus and mutation checks passed.
The new non-Copy regression covers all first-selection/error positions in nine
values, partial consumption, mutations, fused errors and exactly-once dropping.
Its added unwind cases cover every position of an entirely retained prefix;
the final test was rerun normally, in both Miri modes and with ASan.

Basic Kani now has ten passing harnesses. The previous three-element exhaustive
extraction harness remains, and a new nine-element harness proves the first
extraction, remaining length and retained order across two batches and a remainder.
A single nine-element complete-consumption attempt exceeded 180 seconds; it was
replaced by these separate properties, not counted as a successful proof.
Runtime differential, mutation and memory-safety checks cover repeated consumption
and callback failure. Earlier enhanced-Kani compiler limitations remain historical
non-passes; no new enhanced proof is claimed.

## Reproduction

```sh
python3 scripts/verify_vec.py
cargo build --locked --manifest-path tools/vec-analysis/Cargo.toml --release
cargo build --locked --manifest-path tools/vec-analysis/Cargo.toml --release --bin jzap-vec-analysis --features memory --target-dir tools/vec-analysis/target/memory
python3 scripts/compare_vec.py --trials 30 --ops grow,reserved,copy,clone,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all --memory-ops grow,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all
```

These commands describe the historical fixed-input lifecycle comparison. The
retired phase runner is linked from the [earlier analysis](extraction-analysis.md).
Use the [long-interval protocol](../../benchmarks.md) for current measurements.
Compile/verify before timing. The lifecycle command can return nonzero for the
remaining comparison. Current hashes and compact results are in
[summary.json](summary.json); the [earlier instruction analysis](extraction-analysis.md)
describes the pre-change executable. Raw outputs and rejected diagnostic builds
remain in ignored `target/verification/`, with no source snapshots added to Git.
