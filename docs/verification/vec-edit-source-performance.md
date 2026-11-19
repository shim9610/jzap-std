# ExtractIf and splice-fill source migration

Current assembly-based optimization: [2026-11-16 follow-up](vec-edit-optimization.md).
The timing tables below retain their original source and measurement dates.

This comparison covers the executable ExtractIf control helper and Splice's
reuse of the shared initialization cursor. The product source is `970d96c`;
later documentation and measurement-tool commits do not change these bodies.
ExtractIf's previous implementation is `d28d8ec`; splice-fill's is `1509feb`.
Proof domains, caller checks and memory execution are recorded in the
[migration guide](../direct-source-migration.md#vec-extractif-direct-source-checkpoint).

## Work and aggregation

Rust 1.95.0 / LLVM 22.1.2 compiles std, previous and current consumers with
optimization, one codegen unit and panic=abort. The host is an Intel Xeon
Platinum 8573C with a four-CPU container quota. Default and optional loop-aligned
results are separate builds. Compilers, provers and memory-check processes finish
before timing starts.

Each call uses the default unmetered policy, clones an exact-capacity u64 input,
edits it, observes its final length and endpoints, and drops it. Each condition
has 64 pregenerated inputs, with
lengths 0–32 (small) or 256–4096 (mixed), and three independent process seeds.
Implementation order is randomized. The same opaque runtime workload selector,
inputs and call counts reach all three implementations. Excluded calibration
targets two seconds for the fastest implementation; every accepted interval must
be at least one second. A table entry is `sum(seconds) / sum(calls)`, expressed in
ns/call. CSVs retain each seed's seconds, call count and checksum.

ExtractIf conditions select none, all, approximately half, or the first selected
value before early iterator drop. Two additional conditions use the same parity
predicate with all-odd input, or all-odd input except for a final even value.
These distinguish predicate-independent elimination from data-dependent scans
and long retained prefixes. Splice replaces the middle half of the input with
zero, half-hole, exact-hole or excess replacement values. Replacement values
depend on their index.

## Default-build time

| Condition | Size | std ns | Previous ns | Current ns | Current/std | Current/previous |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| None | small | 19.567 | 20.167 | 19.709 | +0.72% | -2.27% |
| None | mixed | 428.404 | 424.135 | 430.634 | +0.52% | +1.53% |
| All | small | 31.196 | 59.311 | 40.512 | +29.86% | -31.70% |
| All | mixed | 1,892.875 | 5,550.352 | 3,027.031 | +59.92% | -45.46% |
| Half | small | 47.276 | 52.922 | 52.773 | +11.63% | -0.28% |
| Half | mixed | 14,754.459 | 14,472.193 | 14,085.951 | -4.53% | -2.67% |
| Early drop | small | 33.575 | 31.136 | 32.674 | -2.68% | +4.94% |
| Early drop | mixed | 613.866 | 581.554 | 658.701 | +7.30% | +13.27% |
| Data-dependent none | small | 34.134 | 48.370 | 30.740 | -9.94% | -36.45% |
| Data-dependent none | mixed | 1,425.236 | 2,439.376 | 1,420.234 | -0.35% | -41.78% |
| Long retained prefix | small | 32.246 | 39.357 | 32.909 | +2.06% | -16.38% |
| Long retained prefix | mixed | 1,699.313 | 2,632.062 | 1,397.244 | -17.78% | -46.91% |

108 intervals; minimum 1.227 seconds. [Per-seed seconds and calls](vec-extraction-source-timing.csv).

| Condition | Size | std ns | Previous ns | Current ns | Current/std | Current/previous |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Empty replacement | small | 33.392 | 38.490 | 36.456 | +9.18% | -5.28% |
| Empty replacement | mixed | 1,040.429 | 1,028.902 | 1,103.217 | +6.03% | +7.22% |
| Short replacement | small | 31.843 | 37.015 | 36.491 | +14.60% | -1.42% |
| Short replacement | mixed | 1,040.656 | 1,167.616 | 1,284.587 | +23.44% | +10.02% |
| Exact replacement | small | 32.128 | 38.360 | 39.338 | +22.44% | +2.55% |
| Exact replacement | mixed | 1,252.804 | 1,274.324 | 1,387.651 | +10.76% | +8.89% |
| Excess replacement | small | 86.327 | 101.098 | 97.049 | +12.42% | -4.00% |
| Excess replacement | mixed | 1,514.551 | 1,697.752 | 1,781.769 | +17.64% | +4.95% |

72 intervals; minimum 1.071 seconds. [Per-seed seconds and calls](vec-splice-fill-source-timing.csv).

## Instructions and scaling

The ExtractIf wrapper is 776 bytes, compared with 824 previously and 539 for
std in the default comparison. For all-selected input, std's repeated path is
five instructions in 16 bytes. The current two-phase helper retains three
conditional branches per selected element after the first: prefix/gap state,
range end and runtime workload selection. The difference repeats with selection
count; it is not a fixed call-entry cost. The retained-prefix loop is five
instructions in 15 bytes, compared with six in 17 bytes previously. The retained
gapped path is eight instructions in 24 bytes, compared with std's seven in 22.
Consumer specialization can change these paths: a separate constant-all code
generation control gives current six instructions/20 bytes and std five/17.

Splice's current comparison wrapper is 1,716 bytes versus 1,521 previously; the
previous executable also has a separate 425-byte fill function. Both current
fills inline, whereas the previous post-growth fill calls that function. The
wrapper stack frame, including saved registers, changes from 200 to 216 bytes;
the previous out-of-line fill needs an additional 32 bytes on its call path.
Growth, reservation, tail movement and excess-staging functions are merged to
the same linked implementations in this comparison.

Both first replacement vector loops execute 13 instructions for four u64 values.
The previous loop occupies 55 bytes at `0x18080`; the current loop occupies 54
bytes at `0x17990`. The former fits one 64-byte interval; the latter crosses its
boundary. Instruction counts establish equivalent vector arithmetic, while
the larger wrapper and changed placement are separate code-generation changes.
Elapsed times alone do not identify a particular hardware stall or cycle count.

## Matched 32-byte loop option

The optional consumer build uses `-C llvm-args=-align-loops=32` for std and both
product versions. Final linked first-vector-loop addresses are `0x18720` for
std, `0x181e0` for previous and `0x17ac0` for current; each is divisible by 32.
Their positions within a 64-byte interval still differ. Library and application
default flags are unchanged.

| Condition | Size | std ns | Previous ns | Current ns | Current/std | Current/previous |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Empty replacement | small | 39.905 | 45.102 | 41.074 | +2.93% | -8.93% |
| Empty replacement | mixed | 843.526 | 919.600 | 919.300 | +8.98% | -0.03% |
| Short replacement | small | 37.985 | 43.596 | 39.912 | +5.07% | -8.45% |
| Short replacement | mixed | 1,094.680 | 1,211.936 | 1,225.284 | +11.93% | +1.10% |
| Exact replacement | small | 42.682 | 46.472 | 45.382 | +6.33% | -2.35% |
| Exact replacement | mixed | 1,473.031 | 1,324.402 | 1,576.320 | +7.01% | +19.02% |
| Excess replacement | small | 106.439 | 130.175 | 128.041 | +20.30% | -1.64% |
| Excess replacement | mixed | 1,583.943 | 1,456.949 | 1,528.491 | -3.50% | +4.91% |

72 intervals; minimum 1.512 seconds. [Per-seed seconds and calls](vec-splice-fill-source-loops32-timing.csv).

Alignment changes the observed gaps without improving every condition. Mixed
short replacement moves from current/previous +10.0% to +1.1%; mixed exact
replacement moves from +8.9% to +19.0%. The option is not selected as a universal
performance improvement. The code and timing observations do not assign the
whole default-build difference to one cache-line crossing.

## Alternatives evaluated

A single ExtractIf loop with the existing read/write state produced a 16-byte,
five-instruction all-selected loop. Its separate 108-interval comparison made
the mixed half-selection condition 99.6% slower than std and the mixed long-prefix
condition 136.7% slower. It was not adopted.

A single-loop deletion-count prototype kept four-word cursor storage and passed
40 native Vec scenarios. In another 108-interval comparison it remained 70.1%
slower than std for mixed all-selection, despite improving 5.8% against the
current two-phase implementation. Its long-prefix conditions regressed 7.5%
(small) and 4.1% (mixed) against current. It was not adopted or included in the
direct-proof claims.

Splice alternatives retained a local initialization cursor, changed the shared
loop to a range iterator, or restored an out-of-line post-growth fill. The first
two did not reduce the compiled replacement-loop work. A 72-interval comparison
of the function-boundary alternative improved mixed short replacement 24.5%
against current but regressed mixed excess replacement 22.8% and small short
replacement 60.9%. None was adopted. Product-source snapshots and generated
assembly for these experiments remain outside Git.

An earlier default no-selection comparison had one slow seed. Repeating that
same immutable binary with elapsed/thread-CPU observation did not reproduce the
gap: current was 4.7% faster than std in that seed, with more than 99.9% of the
interval accounted for by thread CPU time. The earlier seed has no matching CPU
observation; it is not attributed to scheduling or a named hardware event.

## Allocation observations

All 108 ExtractIf observations and 72 independent splice-fill observations match
std, previous and current for allocation calls, requested held/peak bytes,
usable held/peak bytes and owner handle size. Handles are 24 bytes. Every owner
releases all observed bytes after drop. The tested lengths are 0, 1, 8, 32, 512
and 4,096, with every workload pattern. Splice's default timing and independent
memory-only phases are separate receipts; the optional alignment build uses
its own timing binary.

[ExtractIf observations](vec-extraction-source-memory.csv) and
[splice-fill observations](vec-splice-fill-source-memory.csv) retain each sample.

The observer wraps the System allocator. Requested live/peak bytes and
`malloc_usable_size` live/peak bytes describe its allocation-call boundary;
internal transient storage during System realloc and OS resident memory are not
measured. Timing and allocation observation use separate binaries.

## Reproduction

The tables above use the fixture at `687db7d`; use that checkout for their four
splice patterns and iterator type. The current driver additionally exercises
zero-lower-hint sources and large excess that reaches staging. Exact-hint excess
rows above measure the fill/growth path, not the private staging collector.
The [staging coverage section](../direct-source-migration.md#staging-benchmark-path-coverage)
records the separate instrumented entry check.

The maintained [comparison driver](../../tools/throughput-analysis/README.md#extractif-and-splice-fill-source-comparison)
supports separate build, run and memory phases. Use a fresh ignored output path:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_range.py --benchmark extraction --baseline d28d8ec --profile default --memory --output target/extraction --build-only
# After all compilation, proof and memory-check jobs finish:
python3 tools/throughput-analysis/prepare_range.py --benchmark extraction --baseline d28d8ec --profile default --memory --output target/extraction --run-only
```

Select `--benchmark splice-fill --baseline 1509feb` for Splice. Select
`--profile loops32` for its separate alignment comparison. `--memory-only`
rebuilds and runs only allocation observations. Run-only mode checks source,
compiler, fixture, profile and binary identities before reusing a build.
Compact CSVs are kept with this report. Generated archives, disassembly,
diagnostic hooks and full build logs stay under ignored `target/` storage.

## Path-covered staging checkpoint

The following run compares the pre-staging-migration source `687db7d` with the
validated scalar controller at `736d479`. Both libraries and std receive the
same `Hinted` iterator and randomized workload. The fixture has six patterns;
only positive-length large zero-lower-hint excess enters `stage_excess`.
[Entry coverage](vec-staging-entry-coverage.csv) is a separate instrumented run
with full final-content comparisons. Timings below use uninstrumented libraries.

This is a newly measured collector path, not a 21–23% regression of Vec as a
whole or of previously compared fill/compaction loops. In the large zero-hint
case, current/previous is +13.05% for small inputs and −18.24% for mixed lengths.
The scalar controller retains per-element returned-capacity checks. This path
has different instructions and allocation behavior from the earlier fill loop.

| Condition | Size | std ns | Previous ns | Current/candidate ns | vs std | vs previous |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Exact hint: empty | small | 35.995 | 38.565 | 36.431 | +1.21% | -5.53% |
| Exact hint: empty | mixed | 989.590 | 1,486.178 | 969.197 | -2.06% | -34.79% |
| Exact hint: short | small | 44.165 | 45.531 | 42.747 | -3.21% | -6.12% |
| Exact hint: short | mixed | 1,161.685 | 1,918.130 | 1,254.616 | +8.00% | -34.59% |
| Exact hint: exact | small | 48.657 | 57.478 | 53.911 | +10.80% | -6.20% |
| Exact hint: exact | mixed | 1,311.350 | 1,638.675 | 1,239.480 | -5.48% | -24.36% |
| Exact hint: excess | small | 91.682 | 123.281 | 129.017 | +40.72% | +4.65% |
| Exact hint: excess | mixed | 1,691.545 | 1,922.463 | 1,542.155 | -8.83% | -19.78% |
| Zero lower hint: ordinary excess | small | 169.142 | 109.213 | 105.220 | -37.79% | -3.66% |
| Zero lower hint: ordinary excess | mixed | 3,065.552 | 2,086.570 | 1,661.531 | -45.80% | -20.37% |
| Zero lower hint: large excess | small | 427.346 | 456.929 | 516.558 | +20.88% | +13.05% |
| Zero lower hint: large excess | mixed | 33,000.931 | 49,741.510 | 40,671.008 | +23.24% | -18.24% |

108 intervals; minimum 1.295 seconds. [Seconds/calls](vec-staging-source-timing.csv), [allocation observations](vec-staging-source-memory.csv).

All 108 current/previous allocation observations match for calls, requested and
usable held/peak bytes, owner handles and final release. They do not all match
std. At zero-lower-hint ordinary excess and length 4,096, std makes 12 allocation
calls with 81,920 requested peak bytes; both product versions make two calls
with 65,536 requested peak bytes. At large excess and length 4,096, std makes
15 calls and each product version makes 16; all three hold 147,464 requested
bytes and peak at 278,536 requested bytes. Handles remain 24 bytes and all observed
bytes are released.

The latter default run reports usable held bytes of 151,536 for std and 147,464
for both product versions. With `MALLOC_MMAP_THRESHOLD_=131072` applied to the
same memory binary, usable held bytes become 151,536 for all three, while their
requested bytes stay unchanged. Usable peaks also change; the
[small allocator-control CSV](vec-staging-allocator-control.csv) retains both
observations. These are allocator observations, not different requested payload
sizes. The observer still does not see internal realloc high-water storage.

## Historical batch prototype

The 8 November experiment used prototype `468477a`. PR #5 later merged a
verified implementation on the same staging path. See the
[Splice batch reproduction guide](vec-splice-batch.md#reproduction) for the
merged implementation's source, scalar baseline and commands.

The experiment compared the prototype with scalar `736d479`. After one owning
push, the prototype fills spare capacity through the existing initialization
cursor. Metadata admission
runs per batch. Linked assembly contains packed replacement-generation operations
in that span loop; the staged function grows from 1,010 to 1,520 bytes.

The candidate passes 40 optimized native Vec scenarios and the following timing
and allocation comparison. Its direct contracts, Kani harnesses, caller binding,
additional initialization-call boundary, Miri/ASan and final link attachment have
not been ported or checked. It is not adopted in the PR branch.

| Condition | Size | std ns | Previous ns | Current/candidate ns | vs std | vs previous |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Exact hint: empty | small | 38.631 | 42.411 | 55.884 | +44.66% | +31.77% |
| Exact hint: empty | mixed | 1,783.717 | 1,410.285 | 1,271.506 | -28.72% | -9.84% |
| Exact hint: short | small | 41.260 | 52.529 | 58.175 | +41.00% | +10.75% |
| Exact hint: short | mixed | 1,092.005 | 1,411.370 | 1,459.350 | +33.64% | +3.40% |
| Exact hint: exact | small | 37.094 | 50.382 | 59.585 | +60.63% | +18.27% |
| Exact hint: exact | mixed | 1,730.527 | 1,644.106 | 1,760.263 | +1.72% | +7.07% |
| Exact hint: excess | small | 106.380 | 118.794 | 116.545 | +9.56% | -1.89% |
| Exact hint: excess | mixed | 1,713.732 | 1,730.797 | 2,009.589 | +17.26% | +16.11% |
| Zero lower hint: ordinary excess | small | 192.667 | 113.982 | 123.744 | -35.77% | +8.56% |
| Zero lower hint: ordinary excess | mixed | 3,548.583 | 2,018.958 | 2,278.205 | -35.80% | +12.84% |
| Zero lower hint: large excess | small | 456.181 | 689.776 | 592.888 | +29.97% | -14.05% |
| Zero lower hint: large excess | mixed | 35,752.542 | 44,497.823 | 36,520.969 | +2.15% | -17.93% |

108 intervals; minimum 1.141 seconds. [Seconds/calls](vec-staging-batch-candidate-timing.csv), [allocation observations](vec-staging-batch-candidate-memory.csv).

All 108 previous/candidate allocation observations match, including usable byte
observations and final release. Mixed large zero-hint excess reaches std +2.15%;
small large zero-hint excess remains +29.97%. The scalar small-case mean also
changes between the two linked comparisons (516.558 versus 689.776 ns). Keep
these runs separate; they do not establish a universal small-case improvement.
