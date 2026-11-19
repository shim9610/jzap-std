# Vec extraction: fixed work, scaling and machine-code placement

This report records the pre-batching source and diagnostic experiments. See
[the source fix and current measurements](extraction-fix.md) for the subsequent
implementation and current acceptance status.

The original 32-element slowdown is **not a size-independent 20% tax**. Its
additional instruction count is fixed per lifecycle, and the measured time gap
largely disappears at larger sizes. It is also **not universally +4–5 ns**:
a separate no-selection kernel exhibits a growing penalty. Moving that kernel's
unchanged loop five bytes earlier removes the penalty in native measurements.
A subsequent 64-position sweep identifies the triggering placement: the entire
17-byte hot loop straddles an aligned 64-byte region. LLVM aligns its start to
16 bytes, which does not prevent that crossing. Those diagnostic binary controls
did not modify product source.

## Scope and method

Product source: `4c1f55b8f4d54636046718d03d54e21569c39eec` (unchanged during
these experiments). Date: 2026-11-03. Rust 1.95.0 / LLVM 22.1.2,
`x86_64-unknown-linux-gnu`, AMD EPYC 9V74 under KVM, pinned guest CPU 0.
Release uses optimization level 3, one codegen unit and `panic=abort`.
The analyzed case uses default unlimited/unmetered `Vec<u64>` and an inlined
predicate that always returns `Ok`; claims about its erased error handling do
not extend to arbitrary callback types or implementations.

Each condition has 30 independent process observations, with implementation
order rotated/reversed, two warmup cycles and 30 retained cycles per process.
Tables use the median of per-process per-operation medians. Ratios are
jzap/std. Intervals are bootstrap 95% intervals from 5,000 resamples (seed 617),
per comparison. Lifecycle scaling uses independent resampling; diagnostic
contrasts resample trial pairs. Compilation and proofs were not run concurrently
with timing. Virtualized timing and code placement are part of these conditions;
these measurements are not cross-platform bounds.

The original lifecycle executable was frozen before adding the phase benchmark:

- Lifecycle SHA-256: `bb05c7b3114f82837cf0b376ceb506f7aa04cca7f39b15a57b3afe114116abad`.
- Phase executable SHA-256: `7bc88152dcf8cd08140f452d0db41903ca860d3d58b5328e51502f81172f5625`.

## Original lifecycle: size scaling

One operation clones N values, extracts **all values in the middle half**
(`N/4..3N/4`), consumes each yielded value through `black_box`, and frees the
remaining vector. These times include allocation, cloning and destruction.

| N | std ns | jzap ns | Difference ns | Ratio | Ratio 95% interval |
| ---: | ---: | ---: | ---: | ---: | --- |
| 8 | 15.146 | 16.250 | +1.104 | 1.0729 | 1.0403–1.1083 |
| 16 | 16.636 | 18.181 | +1.545 | 1.0929 | 1.0629–1.1429 |
| 32 | 21.539 | 25.564 | +4.025 | 1.1868 | 1.1343–1.2507 |
| 64 | 29.876 | 32.158 | +2.282 | 1.0764 | 1.0707–1.1063 |
| 128 | 49.246 | 50.803 | +1.556 | 1.0316 | 1.0273–1.0374 |
| 256 | 101.326 | 101.572 | +0.245 | 1.0024 | 0.9988–1.0061 |
| 512 | 205.886 | 206.057 | +0.171 | 1.0008 | 0.9974–1.0030 |
| 1,024 | 393.023 | 392.475 | -0.549 | 0.9986 | 0.9967–1.0044 |
| 4,096 | 1728.105 | 1726.582 | -1.523 | 0.9991 | 0.9986–0.9998 |
| 16,384 | 7308.000 | 7309.109 | +1.109 | 1.0002 | 0.9990–1.0006 |
| 65,536 | 30807.562 | 30778.688 | -28.875 | 0.9991 | 0.9940–1.0031 |
| 131,072 | 65498.875 | 64787.250 | -711.625 | 0.9891 | 0.9357–1.0712 |

At N=32, cloning alone measured 11.272 ns/std versus 11.301 ns/jzap
(ratio 1.0025, interval 0.9468–1.0191). This is a separate workload, not a
quantity to subtract mechanically from the lifecycle measurement.

The original acceptance run's +4.148 ns / 1.1998 ratio and this run's +4.025 ns /
1.1868 ratio describe the same small case. They do not imply an enduring 20%
penalty as work grows. The interval at N=131,072 is wide; its negative point
estimate is not evidence of a reliable speedup.

The broader run also covered all four policies. It found failures at N=8
metered extraction (1.1902) and N=128 limited/metered cloning (1.2074), plus
inconclusive cases. All identical-std controls passed. This targeted explanation
of the default extraction path does not close those other policy/size cases.

## Actual executed instructions

The measured executable was disassembled with Capstone 5.0.7 / pyelftools 0.32
and one lifecycle was executed under Unicorn 2.1.4 to count instructions.
Allocator-shim, allocation, memcpy, memmove and free calls were intercepted;
their arguments and retained contents were checked. Caller-saved argument and
scratch registers were poisoned after calls. Instructions *inside* those common
callees are excluded. This is a path count, not a cycle simulation or safety proof.

For N=32 (16 selected values):

| Wrapper phase | std instructions | jzap instructions | Difference |
| --- | ---: | ---: | ---: |
| Clone through memcpy call | 19 | 22 | +3 |
| Extraction through memmove call | 214 | 218 | +4 |
| Finalize, free call and next-iteration control | 15 | 13 | −2 |
| Total | 248 | 253 | **+5** |

For K=N/2 selected values, checked from N=8 through 4,096, totals are
`12*K + 56` for std and `12*K + 61` for jzap. Both steady selected paths execute
12 instructions, including one alignment NOP, per value. The +5 is fixed
wrapper work, not +5 instructions for every element.

The three extra clone instructions are a layout bound check:

```asm
movabs rcx, 0x0fffffffffffffff ; isize::MAX / sizeof(u64)
cmp    r14, rcx              ; source length
ja     layout_error
```

The remaining net +2 comes from different scan entry/tail restoration and final
length/iteration bookkeeping (+4 before memmove, −2 afterward). The source uses
read/write cursors; std uses a deletion count. There is no additional per-value
Result-discriminant check in this concrete always-Ok predicate. Both paths call
one 256-byte allocation, memcpy 256 bytes, memmove the 64-byte tail, and free
256 bytes. Extraction adds no allocation. Both emit one 8-byte stack store per
yielded value for the benchmark's `black_box` consumer.

Instruction count alone misses placement: the steady executed path occupies
44 encoded bytes for std and 53 for jzap. Both contain 38 non-NOP bytes; the
padding NOP is 6 versus 15 bytes. Equal instruction counts therefore do not
establish equal execution time.

## Native controls that isolate the cause

Diagnostic copies of the measured ELF executable were modified locally, keeping
unrelated code addresses fixed. The original product executable was retained.

1. **Layout check removed with equal-length padding.** At N=32 the paired change
   in the jzap-minus-std gap was −1.028 ns, interval −2.330 to +0.562 ns. This
   does not resolve its time contribution. Assigning the observed +4 ns to
   the three bound-check instructions would be unsupported.
2. **Yield consumer stores replaced with equal-length padding in both libraries.**
   At N=32 the gap changed by −2.735 ns (−3.137 to −1.746); at N=128 it changed
   by +5.408 ns (+2.522 to +7.421). The consumer affects the comparison even
   though both emit a store. Removing it is not a library optimization.
3. **Selected hot block packed into its existing padding.** The same 10-byte
   load/compare/branch block moved from `0x39a00` to `0x399f1`; incoming and
   outgoing relative branches were retargeted. The selected cycle fits within
   one 64-byte region and skips its former executed padding. This changes both
   placement and executed NOP work, so it does not separate their contributions.

| N | Original ratio | Packed ratio | Paired change in gap, ns | Change 95% interval, ns |
| ---: | ---: | ---: | ---: | --- |
| 8 | 1.0181 | 0.9818 | -0.539 | -0.544–-0.271 |
| 32 | 1.1869 | 0.9089 | -5.808 | -6.010–-5.224 |
| 128 | 1.0412 | 0.9029 | -6.591 | -7.266–-6.280 |
| 4,096 | 0.9990 | 0.8454 | -266.684 | -268.559–-264.379 |

At N=32 the same campaign measured std 20.578 / jzap 24.424 ns originally and
std 20.575 / jzap 18.701 ns with the packed block. Packing removes the original
short-case deficit without changing the bounds check, ownership algorithm,
allocation calls or Result contract. These copied executables diagnose the
cause; they are not a source-level fix or replacement acceptance result.

## Selection and call context expose a separate growing penalty

The maintained phase benchmark calls explicit non-inlined extraction kernels.
Extraction-only timing builds a pool outside the timer (approximately 1 MiB of
payload, bounded to 4–4,096 vectors), then edits it. Clone-only, drop-only and
complete-lifecycle controls are also available. Pool/cache/allocator state and
inlining differ from the original lifecycle, so phase medians are not additive.

All cases below scan the middle half. Selection 50% means a contiguous prefix of
that range, not alternating values.

| N | Selected | std ns | jzap ns | Difference ns | Ratio |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 32 | 0% | 7.440 | 10.704 | +3.265 | 1.439 |
| 32 | 50% | 13.769 | 10.461 | -3.307 | 0.760 |
| 32 | 100% | 15.937 | 10.017 | -5.921 | 0.629 |
| 4,096 | 0% | 609.344 | 1131.992 | +522.648 | 1.858 |
| 4,096 | 50% | 1507.234 | 1109.391 | -397.844 | 0.736 |
| 4,096 | 100% | 1770.203 | 958.297 | -811.906 | 0.541 |

The no-selection path really does have a growing cost in this executable.
Its six-instruction, 17-byte loop is:

```asm
; original start 0x184b0 (48 bytes into a 64-byte region)
cmp rdi, rdx
jae exit
mov r10, [r15+rdi*8]
inc rdi
cmp r10, rcx
jae loop_start ; at 0x184bf, spans the 64-byte boundary
```

Moving the unchanged 17 bytes to `0x184ab`, five bytes earlier into existing
padding, places the final branch wholly inside that region. Relative jumps are
adjusted and vacated bytes padded. **No per-element instruction is removed**;
entry/exit padding changes location. No other function moves. The following
independent size sweep compares original and packed copies, each with its std
control:

| N | Original std ns | Original jzap ns | Original difference ns | Original ratio | Packed ratio (95% interval) |
| ---: | ---: | ---: | ---: | ---: | --- |
| 8 | 3.316 | 4.071 | +0.755 | 1.228 | 0.994 (0.990–0.998) |
| 16 | 4.454 | 6.286 | +1.833 | 1.411 | 0.989 (0.986–0.992) |
| 32 | 7.434 | 10.740 | +3.306 | 1.445 | 1.010 (1.005–1.016) |
| 64 | 13.415 | 19.453 | +6.038 | 1.450 | 0.897 (0.894–0.900) |
| 128 | 31.057 | 37.698 | +6.641 | 1.214 | 0.952 (0.941–0.962) |
| 256 | 56.759 | 73.125 | +16.366 | 1.288 | 0.948 (0.942–0.957) |
| 512 | 100.256 | 149.392 | +49.136 | 1.490 | 1.004 (0.999–1.012) |
| 1,024 | 175.787 | 292.135 | +116.348 | 1.662 | 1.005 (0.997–1.013) |
| 4,096 | 607.469 | 1129.250 | +521.781 | 1.859 | 0.992 (0.986–0.998) |
| 16,384 | 2406.938 | 4483.531 | +2076.594 | 1.863 | 0.958 (0.952–0.963) |

From N=4,096 to 16,384, the original penalty grows from 521.781 to 2,076.594 ns:
about 0.25 ns per visited value (N/2 values visited). This approximate slope
applies to that large-size region, not every size. Moving the identical loop
removes the positive slope. This establishes local code placement as a cause;
it does not identify a unique decoder, cache or branch-predictor event.
User-space CPU-cycle counters were unavailable (`perf_event_open`: `ENOENT`).

Patched lifecycle paths were checked for all/none/alternating selection at
N=8,32,4,096, including contents, length, call arguments and source preservation.
Native packed phase controls checked N=8,32,4,096 × three selection rates ×
full/middle/tail ranges. Timed extraction pools check resulting contents outside
the timer. These checks validate the diagnostic paths; they do not substitute
for product memory-safety verification.

## Follow-up: the whole hot loop crosses a 64-byte boundary

The generated assembly has `.p2align 4` immediately before the untouched-prefix
loop. This aligns the start to 16 bytes. Its six instructions occupy 17 bytes,
so a start at byte 48 of an aligned 64-byte region spills into the next region:
`48 + 17 = 65`. LLVM's emitted alignment satisfies its 16-byte directive but
leaves this performance-sensitive loop crossing intact. This is a performance
placement issue, not a failure of Rust ownership or Result handling.

A native minimal reproducer copied the same six instructions into executable
memory at **every start offset from 0 through 63**. Entry and exit code stayed at
fixed offsets within separate page-aligned slots. The data, register setup and
visited count were identical. Every call checked its returned visit count.
There were 30 processes per variant, with shuffled offset order, two warmup
batches and ten retained batches of 64 calls, each visiting 4,096 values. Timing
uses `CLOCK_MONOTONIC_RAW` on guest CPU 0. No compilation ran during measurement.

| Start offset within 64 bytes | Encoded loop span | Median ns / visited value | Ratio to offset 0 (95% interval) |
| ---: | --- | ---: | --- |
| 0 | 0–16, contained | 0.2727 | 1.000 |
| 47 | 47–63, contained | 0.2727 | 1.000 (0.981–1.000) |
| 48 | 48–64, crosses | 0.5799 | 2.127 (1.962–2.259) |
| 49 | 49–65, crosses | 0.5584 | 2.048 (1.983–2.238) |
| 52 | 52–68, crosses | 0.5576 | 2.045 (1.976–2.270) |
| 63 | 63–79, crosses | 0.5763 | 2.114 (1.993–2.391) |

All 48 contained placements measured 0.2726–0.2731 ns/value; all 16 crossing
placements measured 0.5432–0.5799 ns/value (ranges of per-offset medians).
Intervals use 2,000 paired bootstrap resamples across process medians, seed 617.
The start offset changes code addresses but not data addresses within a process.

The sweep also disproves the narrower explanation that only a split branch
instruction is responsible. At offset 52, neither compare/branch pair straddles
the boundary, but the loop as a whole still does, and it is still slow. Crossing
a 16- or 32-byte boundary inside a single 64-byte region does not produce this
step in the measured loop.

Four controls distinguish encoded span from data work and conditional prediction:

| Control | Loop bytes | First slow start offset | Observation |
| --- | ---: | ---: | --- |
| Original six instructions | 17 | 48 | Entire loop first crosses 64-byte boundary |
| `inc edi` instead of `inc rdi` | 16 | 49 | Same bounded indices; one byte shorter moves the threshold one byte |
| Redundant `66` prefix before REX.W `inc rdi` | 18 | 47 | Same six instructions and index semantics; one byte longer moves the threshold one byte |
| Load replaced with equal-size register move | 17 | 48 | Removing the data read preserves the step |
| Conditional backedge replaced with equal-size unconditional jump | 17 | 48 | Conditional backedge prediction is not necessary for the step; baseline throughput also changes |

The 32-bit increment is valid only for the bounded microbenchmark's 4,096
iterations; it is not a proposed change to the library's usize index. The register
move copies the zero threshold, preserving the always-taken backedge without
accessing data. These controls diagnose instruction placement, not Vec semantics.

The actual phase executable was then checked at all six available placements
within its existing padding. Only the 17-byte loop and relative branch targets
moved; unrelated addresses stayed fixed. Entry/exit padding uses single-byte
NOPs for every variant, including the zero-shift control. Each condition had
30 processes and the usual two warmups plus 30 retained cycles.

| Loop shift | std ns | jzap ns | Ratio (95% interval), N=4,096, 0% selection |
| ---: | ---: | ---: | --- |
| -5 | 604.648 | 602.609 | 0.9966 (0.9907–0.9999) |
| -4 | 607.383 | 606.055 | 0.9978 (0.9912–1.0031) |
| -3 | 604.953 | 601.828 | 0.9948 (0.9895–1.0021) |
| -2 | 607.609 | 603.555 | 0.9933 (0.9860–1.0041) |
| -1 | 608.555 | 604.094 | 0.9927 (0.9854–0.9987) |
| 0 | 605.273 | 1130.891 | 1.8684 (1.8579–1.8723) |

A **one-byte move is sufficient** to remove the deficit in the actual kernel;
five bytes were not intrinsically necessary. All six variants passed 162 native
content checks across N=8,32,4,096, selection 0/50/100%, and full/middle/tail ranges.

The measured cause is a sustained throughput penalty when this tiny hot loop
spans aligned 64-byte code regions. That explains why extra time accumulates per
visited value instead of once per API call. Removing data reads preserves the
penalty, pointing to instruction delivery/control flow rather than payload
memory access. The experiments do **not** distinguish the precise internal
micro-op-cache delivery rule from a loopback/branch-target mechanism. Hardware
performance counters are not exposed here; `/sys/bus/event_source/devices`
contains breakpoint, msr and software sources, with no CPU PMU source. In
particular, this is not evidence of an instruction-cache *miss* on each iteration.

For architectural context, [Agner Fog's microarchitecture guide](https://www.agner.org/optimize/microarchitecture.pdf)
(version 2026-05-23), sections 24.4, 24.5 and 24.15, describes Zen 4 compare/branch
fusion, decoded-instruction caching and branch throughput. Its explicit tiny-loop
64-byte-boundary rule in section 22.15 concerns earlier Zen processors, so it is
not treated as a Zen 4 specification. The boundary classification above comes
from measurements on this host. CPU cycles were not measured; nanoseconds must
not be relabeled as a proven one-cycle versus two-cycle transition.

To reproduce the boundary control, use the six-instruction sequence above,
fix threshold to zero, place its start at every offset in a 64-byte region, and
preserve the target of both branches. Repeat with the four width/operation
controls and with actual-function shifts −5 through 0, checking retained
contents before comparing timings. One-experiment source, downloads and raw
outputs stay under ignored `target/verification/vec-extract-analysis/boundaries/`.

## Engineering conclusion and remaining work

| Question | Evidence-based answer |
| --- | --- |
| Fixed additional wrapper instructions? | Yes: +5 for the analyzed all-selected lifecycle; same per-selected-value instruction count. |
| Always +4–5 ns? | No: time depends on size, selection, placement and consumer context. |
| General 20% cost of fallibility? | No such pattern in this size sweep; known-Ok Result handling is erased in the inspected loop. |
| Any proportional regression? | Yes: the original no-selection phase kernel has an approximately linear penalty at larger sizes. Moving the same instructions removes it. |
| Is a Vec-wide algorithm rewrite justified? | LLVM’s 16-byte loop alignment permits a 17-byte hot loop to cross a 64-byte region; a one-byte relocation removes this kernel’s regression. This does not establish a Vec-wide algorithmic penalty. |
| Product performance closed? | No. Source-level improvement and repeated default-build/context checks remain; the binary controls are diagnostic. |

For N stored values, M visited values and L tail values moved, the successful
lifecycle includes O(N) cloning plus O(M+L) extraction and predicate work.
The fixed +5 instructions must be distinguished from an alignment-dependent
per-iteration time penalty. The unresolved optimization target is reliable hot
loop generation across caller contexts, without removing failure checks or
selecting one favorable binary. The existing acceptance failure remains recorded.

## Reproduction and retention

The phase measurement runner used for this investigation is preserved in
Git history.
Replaying that historical experiment requires its matching source and toolchain.
The current [long-interval benchmark protocol](../../benchmarks.md) uses
`tools/throughput-analysis/compare.py`; compile and verify before timing.

For the historical binary controls, inspect the symbols and instructions above
against the recorded hashes before relocating blocks; absolute offsets are not
portable across builds. Original lifecycle symbols end in
`run17h0b262cd817465a7cE` (std) and `run17hf30bb65e5204c58dE` (jzap).
The original phase symbol is `jzap_extract_phase`.
Source and compiler changes require redisassembly and branch retargeting, not
blind offset patching. These one-executable patch/count tools, raw samples,
assembly and binary copies remain temporarily under ignored
`target/verification/vec-extract-analysis/`; they are not maintained product
tools or committed evidence. Git retains this report and the phase benchmark;
the retired diagnostic runner and its matching source are available in commit
history.
