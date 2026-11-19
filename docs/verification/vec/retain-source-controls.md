# Retain source-form controls

At the recorded checkpoint, the product had a proportional limited-retain
regression. These controls identify source and code-layout effects; no candidate
was adopted in this experiment.
Product checkpoint: `37beeb3598e8ac2d94756fcd77644b1adf29d3a9`. The experiment
starts from tool checkpoint `4917a32`. Its historical source-form runner is
preserved in Git history.
Current performance measurement uses the [long-interval protocol](../../benchmarks.md);
the tables below describe the earlier 20-workload executable.

## Method and current difference

Rust 1.95 / LLVM 22.1.2, Linux x86-64 AMD EPYC 9V74 KVM guest, CPU 0;
opt-level=3, one codegen unit, panic=abort, default alignment, no LTO.
Each form/size/policy has 30 independent processes, two warmups and 30 retained
cycles. Implementation/form order rotates and reverses. Bootstrap 95% intervals
use 5,000 paired trial resamples, seed 617. Compilation/tests finish before timing.
The workloads clone the source, retain alternating u64 values, then drop.
The three sizes are 32, 4,096 and 131,072, under all four policies plus std controls.

Current limited-retain differences from the same executable's std control:

| N | std ns | jzap ns | Difference ns | Ratio (95% interval) |
| ---: | ---: | ---: | ---: | --- |
| 32 | 21.659 | 27.747 | +6.088 | 1.2811 (1.2434–1.3365) |
| 4,096 | 1641.766 | 2748.719 | +1106.953 | 1.6742 (1.6734–1.6756) |
| 131,072 | 77479.750 | 100328.000 | +22848.250 | 1.2949 (1.2318–1.4059) |

These current controls pass. Some other forms have noisy/inconclusive large-size
std controls (including advance_once at 131,072); their results are not promoted
to accepted comparisons. The 4,096-element controls shown next all pass.

## One-source-change and factorial controls

| Source form, N=4,096 limited | std ns | jzap ns | Ratio (95% interval) |
| --- | ---: | ---: | --- |
| Current, 20 workloads | 1641.766 | 2748.719 | 1.6742 (1.6734–1.6756) |
| Advance read once | 1642.195 | 2748.641 | 1.6738 (1.6717–1.6757) |
| Invert keep/remove branches | 1641.652 | 2748.480 | 1.6742 (1.6721–1.6753) |
| 20 workloads, remove max(1) | 2168.137 | 2169.039 | 1.0004 (0.9998–1.0008) |
| 14 workloads, no max(1) | 2721.648 | 1615.594 | 0.5936 (0.5935–0.5939) |
| 14 workloads, keep max(1) | 2721.609 | 1616.258 | 0.5939 (0.5934–0.5942) |
| Use std-style prefix loop | 1615.711 | 1615.246 | 0.9997 (0.9985–1.0004) |

The current, advance_once and invert_branch executables have the identical full
SHA-256: `26b8b1874b7049ce970de2beb267f390367b23c47d66238fb48c1b53535db39a`.
Moving the cursor increment out of the two branches, or reversing their source
order, therefore changes neither final code nor layout. Timing variation among
these identical executables is not a source optimization.

Removing max(1) from benchmark construction changes register allocation and
addresses despite leaving actual setup values identical at all measured nonzero
sizes. This form is a diagnostic, not a replacement for empty-input insertion
benchmarks. The crossed 14/20-workload controls show that the regression depends
on the combination of code context and construction expression; max(1) is not a
universal per-element cost. Returning to 14 workloads also moves std into a slow
layout. Its approximately 0.594 jzap/std ratio is not universal jzap superiority.

Matching std's initial scan structure changes basic-block placement in the later
compaction loop. The candidate fixes limited retain at all three tested sizes,
but **introduces a metered-only N=4,096 failure**: std 1,615.711 ns, jzap
2,169.703 ns, ratio 1.3429 (1.3412–1.3443). It is not adopted as a product fix.

## Actual std source and final machine code

The pinned Rust 1.95 retain_mut implementation already uses a read/write
PanicGuard, as jzap does. A previous conversational description using a
processed/deleted counter was incorrect for this pinned version. Relevant current
differences include:

- std explicitly returns for empty input, then uses loop with its end check
  after advancing read. jzap uses a while read < original prefix scan.
- std marks the first removal as unlikely internally. The stable-source
  std_prefix control does not add that internal intrinsic.
- std lists removal before retention in compaction; jzap lists retention first.
  The inversion control proves that this source ordering is erased here.
- jzap handles callback Result errors and carries optional limit/budget state.
  The known-Ok predicate erases callback-error handling in this timing case;
  limit/budget checks remain outside the per-element compaction path.

Final ELF instructions were decoded and one alternating keep/remove pair was
executed in a local instruction emulator. The count excludes clone, allocation,
entry, final iteration/exit and cleanup. This is an instruction-path check, not
a CPU timing model or ownership proof. Every compared steady path executes
**11 instructions per two input values**.

“Executed bytes” sums instructions actually visited. “Address span” also includes
skipped instructions/padding between visited blocks. Earlier references to a
49-byte path meant this address span, not 49 bytes of executed instructions.

| Form / implementation | Instructions / pair | Executed bytes | Address span | Header offset mod 64 | Last backedge offset mod 64 |
| --- | ---: | ---: | ---: | ---: | ---: |
| current / limited | 11 | 35 | 49 | 16 | 63 |
| current / std | 11 | 35 | 49 | 32 | 15 |
| no_minimum_capacity / limited | 11 | 35 | 49 | 48 | 31 |
| no_minimum_capacity / std | 11 | 35 | 49 | 48 | 31 |
| old_14_workloads / limited | 11 | 37 | 37 | 48 | 19 |
| old_14_workloads / std | 11 | 35 | 49 | 16 | 63 |
| old_14_with_minimum / limited | 11 | 37 | 37 | 48 | 19 |
| old_14_with_minimum / std | 11 | 35 | 49 | 16 | 63 |
| std_prefix / limited | 11 | 35 | 35 | 32 | 1 |
| std_prefix / std | 11 | 35 | 49 | 32 | 15 |

Current limited/std paths each execute 35 bytes in 11 instructions over a 49-byte
span. Both use rcx for reading and rax for writing in the decoded steady path;
the length registers are rbp and r12 respectively. The limited backedge starts at
0x2897f (offset 63), with bytes 75 cf across the 64-byte boundary. The std
backedge is at offset 15. Both paths' address spans cross a 64-byte boundary;
merely crossing a boundary somewhere is not sufficient to explain this case.

The std_prefix limited path has the same counts of 35 bytes/11 instructions, now
contiguously across a 35-byte span, rather than jumping over 14 bytes of alternate
path code. The first-scan source change therefore affects later block placement,
without reducing the steady pair's instruction count. The source-form table
explains why a simple source rewrite may change one specialization and hurt another.

The earlier one-byte relocation control establishes layout causality for the
original slow block. It does not separately prove that the split backedge alone
causes the loss, or identify a CPU decoder/cache/loopback mechanism. PMU events
remain unavailable in this guest. No statement about exact CPU stalls is claimed.

## Verification, reproduction and remaining work

Every built source form passes the 38 Vec tests and three Vec properties
(2,048 cases each, seed 123456789) before timing. The tested forms exercise callback
errors, mutation, cleanup, Drop unwind, ownership and limit/budget behavior. No
candidate changes are retained in src/. Miri, Kani and full acceptance reruns are
required if a product candidate is subsequently selected; these diagnostic tests
are not a substitute. Internal panic branches must be removed or proven
unreachable against actual implementation before completion.

The archived runner applies only to its matching historical product and tool
checkout. It temporarily edited tracked source to compare rejected candidates;
those experiments are complete. Current measurement uses
`tools/throughput-analysis/compare.py`, with generated outputs under `target/`.
Prior tools and sources remain in Git history without checked-in snapshots.

## Symmetric compiler-alignment controls

Subsequent controls use the unchanged product and expanded 20-workload source at
`716f416`, Rust 1.95 / LLVM 22.1.2 and the same EPYC 9V74 KVM guest/CPU 0.
The default executable reproduces the recorded SHA-256 exactly. Both std and jzap
benchmark monomorphizations receive identical options in each executable.
Loop-alignment builds use -align-loops=32 or -align-loops=64. All 725 loop/block
alignment directives change to .p2align 5 or 6; 132 function alignment directives
remain .p2align 4. Final ELF inspection confirms the measured loop placement.

Thirty independent processes per condition retain two warmups and 30 cycles.
The six operations retain/extract_all/pop/split_off/clone/reserved are measured
first, then the remaining fourteen. Timing and bootstrap methodology otherwise
match compare_vec.py. Compilation finishes before timing. No product source or
allocation policy is changed. The hooked memory executable is the unchanged
unaligned baseline; repeated retain memory checks are not aligned-memory proofs.

For N=4,096 limited retain:

| Configuration | std ns | jzap ns | Ratio (95% interval) |
| --- | ---: | ---: | --- |
| Default | 1641.066 | 2747.941 | 1.6745 (1.6732–1.6752) |
| Loop alignment 32 | 1642.355 | 1643.020 | 1.0004 (0.9985–1.0097) |
| Loop alignment 64 | 1642.945 | 1642.746 | 0.9999 (0.9164–1.0073) |
| Branch-boundary protection 32 | 2195.477 | 1642.391 | 0.7481 (0.7472–0.7486) |

Loop alignment removes the observed large retain deficit in these builds.
It does not demonstrate the fastest placement for every workload. Full 20-workload
results, including identical std controls, remain partially unresolved:

| Build | Nominal pass / fail / inconclusive, including controls | Valid product passes | Passing std controls |
| --- | --- | --- | --- |
| Loop alignment 32 | 297 / 0 / 3 | 234/240 | 59/60 |
| Loop alignment 64 | 292 / 0 / 8 | 229/240 | 59/60 |

Each build has one inconclusive large-input std control, invalidating four nominal
product passes at that condition. The 32-byte build also has inconclusive N=32
limited/metered splice and shrink. The 64-byte build has seven inconclusive product
comparisons, plus its inconclusive std control. None are discarded or relabeled
as passes. These are separate per-comparison intervals, not universal speed bounds.
The current default-build acceptance decision is not replaced by a favorable flag.

The text section grows from 477,875 bytes to 485,027 (+1.50%) with loop alignment
32 and 498,275 (+4.27%) with alignment 64. Final instruction paths can grow too:
the aligned std retain path has 39 executed bytes across an 81-byte address span,
versus 35 bytes across 49 in the default build; the instruction count remains 11.
Longer branches and padding are real tradeoffs. A loop header aligned to 64 bytes
does not imply that all its basic blocks fit in the same 64-byte region.

### Direct branch-boundary protection and a demonstrated regression

LLVM also supports:

```sh
RUSTFLAGS='-C llvm-args=-x86-align-branch-boundary=32 -C llvm-args=-x86-align-branch=jcc+fused+jmp' cargo build --locked --manifest-path tools/vec-analysis/Cargo.toml --release --bin jzap-vec-analysis --target-dir target/verification/vec-loop-alignment/branch32
```

This option pads selected branches/fused pairs to avoid crossing or ending against
the specified boundary in newly generated code. It does not rebuild precompiled
stdlib/runtime objects automatically. On six workloads at N=32 and 4,096 it has
57 nominal passes, one failure and two inconclusive results, with all 12 std
controls passing. It repairs limited retain but creates a proportional
limited/metered pop regression: N=4,096 std 1,715.742 ns, jzap 2,842.492 ns,
ratio 1.6567 (1.6447–1.7318).

The actual pop loop explains the new work:

```text
default:  load 5 B; black_box store 5 B; decrement 4 B; branch 2 B
protected: load 5 B; black_box store 5 B; NOP 6 B; decrement 4 B; branch 2 B
```

The default loop starts at offset 48 and fits through offset 63. Protection adds
one executed NOP per value, increasing the loop from 4 instructions/16 bytes to
5 instructions/22 bytes and moving the backedge into the next 64-byte region.
It guards the branch boundary while worsening the surrounding loop geometry.

A copied-executable control removes only that six-byte hot-loop padding, repairs
the backedge and places the padding after the loop. All other addresses and
accounting operations remain unchanged. In a paired 30-process experiment,
N=4,096 changes from 2,825.594 ns to 1,704.160 ns. Their std controls measure
1,713.906 and 1,699.906 ns; both identical std controls pass. Product/std intervals
change from 1.6472–1.6494 to 1.0018–1.0029. The N=32 original std control is
inconclusive, so no small-case timing conclusion is drawn from that control.
This establishes that the inserted hot padding causes the new large-case loss;
it does not assign exact hardware stalls or cycles. The copied binary is not
shipped. Its source/flags and patch layout, not an old source snapshot, identify
the diagnostic.

### Practical scope and reproduction

Loop alignment can steer this known bad placement. No tested flag establishes
universally optimal placement or full Vec acceptance. Source-independent compiler
options are application build choices: a library's Cargo profile does not impose
the consumer's RUSTFLAGS. LLVM backend options and the relevant geometry must be
rechecked for a different compiler/CPU. No alignment option is enabled by default
in this repository.

```sh
RUSTFLAGS='-C llvm-args=-align-loops=32' cargo rustc --locked --manifest-path tools/vec-analysis/Cargo.toml --release --bin jzap-vec-analysis --target-dir target/verification/vec-loop-alignment/align32 -- --emit=asm
# For the second build, use -align-loops=64 and a separate align64 target directory.
python3 scripts/compare_vec.py --trials 30 --ops grow,reserved,copy,clone,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all,insert_remove,resize,split_off,append,shrink,pop --memory-ops retain --timing-binary target/verification/vec-loop-alignment/align32/release/jzap-vec-analysis --memory-binary target/verification/vec-loop-alignment/build-memory/release/jzap-vec-analysis --output target/verification/vec-loop-alignment/repeat32
```

Build the ordinary memory-hook executable separately before timing, as in the
expanded benchmark reproduction. Raw observations, compiler help, binary hashes,
assembly, final ELF traces and diagnostic copies remain under ignored
target/verification/vec-loop-alignment. The original source remains unchanged;
these experiments do not complete the pending source-level safety proofs.
