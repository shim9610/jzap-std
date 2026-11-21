# Linux binary-level bottleneck follow-up

This report covers a full Linux benchmark of public `main` (`d1a04e7`), the
binary-level causes of the slower operations, and the source changes made on
`work/perf-bottlenecks`. It records which gaps were changed, which were
examined and left unchanged, and why. It is not a whole-library performance or
MVP completion claim.

## Conditions

- Intel Core i7-13700KF, Linux 7.0, Rust 1.95.0, release profile of
  `tools/throughput-analysis` (opt-level 3, one codegen unit, panic=abort).
- Timing on CPU 6 with its hyperthread sibling offline, turbo disabled and a
  fixed 3.4 GHz frequency. No compiler, prover or other benchmark ran during a
  timed interval.
- Collection timings use `tools/throughput-analysis/compare.py` (intervals of at
  least one second, three seeds, call-weighted means). B+ timings use the
  `bplus` and `bplus_range` binaries against `brie-tree` 0.1.2 in the same
  process.
- Hardware performance counters were unavailable (`perf_event_paranoid=4`).
  Causes below come from disassembly of the measured executables, ptrace PC
  sampling, single-step instruction traces, LLVM IR, and `LD_PRELOAD` call
  counters. Without counters, the share of a gap caused by code placement
  (64-byte line offsets) is not quantified.

## Changes

### BTree leaf insertion skips empty moves

`node::insert` shifted the pair suffix with two `ptr::copy` calls even when
the insertion point was the end of the node. With a length of zero these are
real `memmove` calls. std's `slice_insert` skips the move in that case.
The suffix and edge moves now run only for a nonzero tail.

`LD_PRELOAD` counts over 20 epochs of the small workloads:

| Workload | Zero-length `memmove` before | After | std |
| --- | ---: | ---: | ---: |
| BTreeMap small `btree_remove_cycle` | 11,547 | 6,325 | 6,325 |
| BTreeSet small `btree_set_build` | 3,820 | 2 | — |

Single-step traces of the small BTreeSet build show 355.8 instead of 365.7
instructions per build. Timing of the guard alone, in otherwise identical
binaries (three seeds, ratio to the std control in the same binary):

| Small workload | Guard | No guard | std control in each binary (ns) |
| --- | ---: | ---: | --- |
| BTreeSet `build` | +20.7% | +8.3% | 61.69 / 65.03 |
| BTreeMap `build` | +8.6% | +23.6% | 77.94 / 74.60 |
| BTreeMap `remove_cycle` | +2.3% | +5.5% | 23.97 / 24.25 |

The identical std code moved by about 5% between the two binaries, and the
guard moved the two builds in opposite directions. At this input size code
placement outweighs the guard. The guard stays because it removes the calls
and instructions and matches std's condition. The small BTreeSet build gap
remains open.

### Vec `retain` keeps the predicate state in a register

In the measured consumer, jzap reloaded the retain threshold from memory for
every element (`cmp 0x68(%r12),%rax`). std kept it in a register. LLVM IR
showed why: std's `Vec::retain` receives the user closure by value, so the
closure's captured reference becomes a `noalias` argument whose scope proves
that stores into the vector cannot change it. jzap's `retain`, `retain_mut` and
`try_retain` forwarded the closure through a `&mut` capture, and inlining left
no such scope. The wrappers now move the closure. The resulting loop matches
std's instruction sequence, with the threshold in `%rax`.

### Vec `dedup` uses the unpaired sweep

The paired compaction sweep produced a loop whose duplicate case took a branch
in both halves: 1.78 taken branches per pair against std's 1.22 for the
benchmark's duplicate share. This difference was already recorded, unresolved,
in `docs/direct-source-migration.md`. `try_dedup_by` now calls the
compaction driver with `PAIRED = false`. Its loop is instruction-for-instruction
the same as std's. The source bindings (`scripts/caller_bindings.py`,
`scripts/verify_owner_protocol.py`) and the maintained memory model
(`tools/memory-proof/edit-loops.verus.rs`) follow the new routing.

### BTree borrowed iteration keeps the in-leaf step as the loop body

For BTreeSet/BTreeMap borrowed iteration, the in-leaf advance jumped to a
shared tail block: 11 instructions and 2 taken branches per element, against
std's 9 and 1. Reordering the source did not change LLVM's layout. A
`core::hint::cold_path()` (stable since 1.95) on the leaf-boundary path makes
the in-leaf step a fall-through body of 10 instructions and 1 taken branch.

### Width-eight `FixedTree` point lookup uses conditional selection

`FixedTree::get` chose branch-based routing for nodes of at most eight keys
(the `SMALL_NODE_KEYS` rule from `5313656`, measured on an AMD EPYC 9V74 for
streaming lookup). On this CPU the width-eight branch search compiled to two
data-dependent branches per level plus a stack round trip of the routing
result. Three variants were timed in one A/B campaign (four seeds, fixed-only
mode, ns/lookup):

| 8 / 2 | Branch search (before) | Conditional selection | Fixed-offset count |
| --- | ---: | ---: | ---: |
| `fixed` | 32.50 | 19.64 | 24.02 |
| `brie-tree` in the same binary | 12.23 | 12.26 | 16.81 |

Conditional selection was fastest in every seed (19.1–20.5 ns). The
fixed-offset binary also slowed the unchanged `brie-tree` code, which marks a
placement effect of that size in this binary. `FixedTree::get` now uses
conditional selection at every width. Widths 16 and 32 run the same code as
before.

Streaming `Lookup` was re-evaluated on the same CPU. Replacing its width-eight
fixed-offset count with conditional binary search increased the streaming time
from 43.41 to 50.24 ns/lookup in all four seeds. Streaming lookup is unchanged.

### Width-eight `FixedTree` range leaf bounds use fixed offsets

A width-eight range opened its first leaf with a binary search for the lower
bound and, when the first two candidates did not settle the upper bound, an
out-of-line `partition_point` call. For width eight and keys of at most eight
bytes (the existing `short_keys` condition, not on Windows), both leaf bounds
now count the keys below the bound at fixed offsets on 64-bit targets, as
the branch seek already does. Other widths, key sizes and 32-bit targets keep
the binary search (see [cross-target assembly](#cross-target-assembly)). A new Kani
harness, `width_eight_leaf_prefix_matches_binary_search`, checks the count
against `prefix_len` for every sorted `[u8; 8]`, query and inclusivity.

Two variants were timed against the unchanged branch (three seeds, width 8,
height 2, ns/range):

| Items per range | Before | Lower bound only | Both bounds (adopted) | `brie-tree` |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 13.17 | 13.43 | 12.83 | 8.67 |
| 1 | 14.67 | 15.24 | 14.71 | 11.93 |
| 32 | 62.96 | 57.20 | 56.23 | 63.11 |
| 256 | 285.75 | 280.82 | 274.10 | 240.94 |

The lower-bound-only variant was slower for empty and single-item ranges and
was not adopted.

Empty ranges were then traced per instruction (seed 1, width 8, height 2).
jzap executes about 160 instructions per range and `brie-tree` about 79,
excluding the shared 15-instruction measurement loop:

| Part of an empty range | jzap | `brie-tree` |
| --- | ---: | ---: |
| Closure entry and exit | 15 | 8 |
| Branch descent, per level | 27: seven fixed-offset comparisons, node ID and slice checks, checked leaf rank | 16: conditional binary search over arena offsets |
| Leaf | 38: eight comparisons, three dead stack stores | 17: conditional binary search, one end comparison, early return |
| End slot and cursor | about 25 | none for an empty range |
| Next leaf (one query in eight starts at a leaf end) | about 3 on average | — |

Replacing the width-eight branch seek's fixed-offset count with conditional
binary search (`child_index::<true>`) was timed against the current branch
(three seeds, ns/range). It was slower at every size and was not adopted:

| Items per range | Current | Conditional seek | `brie-tree` |
| ---: | ---: | ---: | ---: |
| 0 | 12.85 | 16.55 | 8.67 |
| 1 | 14.70 | 18.53 | 11.92 |
| 32 | 55.89 | 64.86 | 63.48 |
| 256 | 272.20 | 282.68 | 241.48 |

The remaining empty and single-item gap comes from checked node IDs and leaf
ranks at each level and from building the returned cursor. `brie-tree`
addresses its arena without these checks.

`try_new` already validates every child ID, height and parent, and the pools
are immutably borrowed, so the per-lookup node-ID and rank checks cannot fail
on a constructed snapshot. To bound the benefit of moving them to
construction, an unproved experiment removed them with unchecked indexing
(leaf slice checks kept, because an inconsistent `Ord` must still return
`InvalidStructure`). Against the current branch (ns):

| Condition | Current | Node-ID and rank checks removed | `brie-tree` in each binary |
| --- | ---: | ---: | --- |
| Range, 0 items | 12.85 | 11.74 | 8.68 / 8.68 |
| Range, 1 item | 14.71 | 13.77 | 11.95 / 11.93 |
| Range, 32 items | 55.66 | 56.76 | 63.39 / 63.04 |
| Range, 256 items | 270.78 | 274.46 | 241.71 / 241.58 |
| `get`, 8 / 2 | 19.50 | 17.11 | 11.56 / 24.10 |
| `get`, 32 / 2 | 24.93 | 25.19 | 30.34 / 42.04 |
| `get`, 16 / 3 | 44.07 | 42.25 | 45.99 / 51.65 |

The unchanged `brie-tree` code doubled in the experimental `get` binary, so
those `get` differences are not attributable to the checks. Ranges gained
about 1 ns when small and lost 1–2% when large. With that bound, the change
was deferred: it would add internal `unsafe` that needs a new proof of the
construction-time height and ID invariant, for a gain that leaves the
`brie-tree` gap largely in place. The approval terms and status are in
[recorded decisions](../decisions.md).

## Remeasurement against public `main`

Each run holds the `d1a04e7` executable, the branch executable and their std
controls. Values are unbudgeted (`unlimited`) means against the std control
of the same executable; every interval lasted at least 1.18 seconds.

| Operation | Input | `d1a04e7` vs std | Branch vs std | Branch seed range |
| --- | --- | ---: | ---: | --- |
| Vec `retain` | mixed | +6.6% | −0.7% | 0.973–1.015 |
| Vec `retain` | small | +6.8% | −9.9% | 0.895–0.911 |
| Vec `dedup` | mixed | +9.4% | −0.6% | 0.991–0.997 |
| Vec `dedup` | small | +22.3% | +14.6% | 1.139–1.155 |
| BTreeMap `iter` | mixed | +2.3% | −17.6% | 0.809–0.836 |
| BTreeMap `iter` | small | −41.8% | −48.7% | 0.511–0.516 |
| BTreeMap `remove_cycle` | mixed | −2.9% | −1.6% | 0.982–0.985 |
| BTreeMap `remove_cycle` | small | +12.8% | +2.1% | 1.015–1.032 |
| BTreeSet `iter` | mixed | +36.0% | +2.6% | 1.009–1.055 |
| BTreeSet `iter` | small | +10.6% | −33.6% | 0.661–0.669 |
| BTreeSet `build` | mixed | −4.3% | −6.6% | 0.931–0.936 |
| BTreeSet `build` | small | +15.7% | +19.2% | 1.177–1.209 |

B+ lookups against `brie-tree` 0.1.2 in the same process (fixed-only mode,
four seeds; ranges at width 8, three seeds; ns per lookup or range):

| Condition | `d1a04e7` | Branch | `brie-tree` (branch run) |
| --- | ---: | ---: | ---: |
| `FixedTree::get`, 8 / 2 | 32.54 | 19.73 | 12.02 |
| `FixedTree::get`, 32 / 2 | 24.88 | 24.64 | 29.90 |
| `FixedTree::get`, 16 / 3 | 46.97 | 43.82 | 46.00 |
| Streaming `Lookup`, 8 / 2 | 43.47 | 43.45 | 12.02 |
| Range, 0 items | 13.17 | 12.86 | 8.67 |
| Range, 1 item | 14.59 | 14.72 | 11.96 |
| Range, 32 items | 62.82 | 55.67 | 63.51 |
| Range, 256 items | 286.26 | 270.29 | 241.41 |

The 16 / 3 `FixedTree` change has no source cause (width 16 runs the same
code) and is attributed to placement. Small BTreeSet `build` remains above
+15%. Streaming lookup and empty/single-item ranges remain slower than
`brie-tree`.

Small Vec `dedup` (+14.6%) was traced per instruction. Its element loop is
instruction-for-instruction std's and runs the same per-call counts (15.55,
12.19, 3.36 and 3.44 executions per call of its four blocks). The remaining
nine instructions per call belong to the benchmark's `try_clone` (a capacity
overflow test) and register moves; small `retain` uses the same clone and is
9.9% faster than std. In this build the hottest backward branch of the
element loop straddles a 64-byte line, while std's block sits in one line. A
diagnostic build with `-C llvm-args=-align-loops=64` (three seeds, same
protocol) gives:

| Small workload | Default build | `align-loops=64` |
| --- | ---: | ---: |
| Vec `dedup` | +14.0% (1.133–1.155) | +0.9% (1.006–1.014) |
| Vec `retain` | −9.0% (0.906–0.914) | +3.8% (1.034–1.047) |

The small `dedup` gap is a code placement effect. Default builds do not force
loop alignment; the opt-in setting is described in
[build tuning](../build-tuning.md). No source change was made for it.

## Cross-target assembly

The changed paths were compiled for ten targets with the same Rust 1.95.0
compiler, for `d1a04e7` and for the branch. `core` and `alloc` were built from
the toolchain's `rust-src` with `rustc`, and an empty `compiler_builtins`
stand-in satisfied `alloc` (it supplies link-time intrinsics only). A
`no_std` harness reproduces the benchmark loop shapes; on x86-64 Linux it
reproduces the measured `retain` reload. These are static comparisons. No
target other than x86-64 Linux was timed.

| Change | x86-64 Linux, Windows, macOS, FreeBSD | AArch64 Linux, Android, macOS, Windows | i686 Linux | ARMv7 Android |
| --- | --- | --- | --- | --- |
| `retain` threshold loads in inner loops | 2 → 0 | 1 → 0 | 1 → 0 | 1 → 0 |
| `dedup` sweep | paired loop → std's loop | paired loop → std's loop | paired 22-instruction loop → single-element loop; one stack bound load versus std's memory compare | paired loop → std's loop |
| BTreeSet in-leaf iteration | 10 instructions with jumps → 9 without | already 10 instructions with one taken branch; rotation only | shared tail → own 10-instruction loop | shared tail → own 12-instruction loop |
| `FixedTree::get` width 8 | selects 4 → 6, conditional branches 13 → 11 | identical machine code (already `csel`) | identical | identical |
| Range width 8 | calls 4 → 1 (Windows unchanged by design) | calls 4 → 1 (Windows unchanged) | identical | identical |

With the fixed-offset leaf count enabled on 32-bit targets, the range function
grew from 311 to 408 instructions on i686 and from 363 to 424 on ARMv7, because
each `u64` comparison takes two instructions there. Without a timing on those
targets, the count is limited to 64-bit pointer widths and the 32-bit code is
identical to `d1a04e7`. The existing width-eight branch seek and streaming
lookup still count on all non-Windows targets; their 32-bit timing is likewise
unverified.

## Examined and left unchanged

| Operation | Gap | Cause found | Reason left unchanged |
| --- | ---: | --- | --- |
| B+ `FixedTree` routing result | — | Width-eight branch search kept `Option<usize>` on the stack | Two source forms compiled to identical machine code; the selected search above removes the case |
| String `from_utf16` | +6.6% | Per code point: checked length add, in-capacity path as a taken branch, length re-check after the write with a materialized `Result` | Moving growth out of line made the default build slower (+6.3% to +7.7%) and was reverted. A std-shaped push changes the proved String edit envelope |
| HashSet small build | +7.4% | Per insert +11 instructions; per resize +85 (copy-then-publish wrapper and guard calls) | Ownership-preserving resize and bounded probing; estimated inlining gain about 1% |
| HashSet `extract` | +19.6% in the full run | 19 vs 17 instructions per element: index-based bucket addressing, constant rematerialized, bounded group scan | Gap follows the std control's placement; see below |
| VecDeque small `push_back_8` | +8.3% | 117 vs 80 instructions per growth | Spread across budget/layout checks and ring relocation |
| Rc `clone` | +6.7% | Saturating count: load/compare/store instead of `incq` | Part of the shared count protocol |
| Arc `cow_weak`, `unique4096` | +5.8%, +9.4% | Uniqueness pre-check | Part of the shared count protocol |
| `scenario_wordcount` | +5.2% | Default `Hasher::write_str` out of line | `write_str` is unstable to override on Rust 1.95 |
| Scratch `get_8` | +23.6% | One extra move for the `Result` error value, loop crosses a line | Checked view returns `Result` by contract |
| HashMap/HashSet `default_new`, HashSet `owned_*` | — | Recorded in `docs/random-state.md` | Source variants were already tried and reverted there |
| HashSet `iter`, BinaryHeap `iter_8`/`drain_8`, arithmetic `call` | — | Identical or shorter code at a different line offset | Default builds do not force alignment |
| Benchmark-wrapper inlining (BTreeSet disjoint/skew, HashMap `get`) | — | Outlined benchmark adapter | Measurement tool, not product code |

### HashSet `extract` follows the std control

The hash table sources are identical in `d1a04e7` and on the branch. Timed in
one run (three seeds, ns per call):

| Executable | std control | jzap | jzap vs std |
| --- | ---: | ---: | ---: |
| `d1a04e7` | 2,585.4 | 3,161.7 | +22.3% |
| Branch | 3,135.4 | 3,128.8 | −0.2% |
| Branch, `align-loops=64` | 3,160.4 | 2,891.7 | −8.5% |

jzap's time is nearly constant across the three executables; the std control
moved by 18% between them. The full-run gap therefore comes from the
placement of std's loop in that executable, which matches the earlier
−7.3% to +16.2% swings recorded in `docs/random-state.md`. The two-instruction
difference per element remains but does not set the measured gap, so the
shared `Scan` was not changed.

### Placement triage of the remaining gaps

Each remaining candidate was timed in one run with three executables: `d1a04e7`,
the branch, and the branch built with `-C llvm-args=-align-loops=64`, each
against its own std control (three seeds, unbudgeted ratios). The std controls
stayed within 2% across the executables.

| Workload | `d1a04e7` | Branch | Branch, `align-loops=64` | Reading |
| --- | ---: | ---: | ---: | --- |
| BTreeSet `build`, small | +15.3% | +18.4% | +6.3% | Mostly placement; about 6% remains |
| VecDeque `push_back_8`, small | +9.5% | +11.2% | +11.6% | Code difference |
| HashSet `build`, small | +7.5% | +7.4% | +6.4% | Code difference |
| String `utf16`, mixed | +6.3% | +6.5% | +8.8% | Code difference |
| BinaryHeap `peek_mut_8`, mixed | +5.4% | +5.6% | +2.6% | Half placement; about 2.6% remains |

The small BTreeSet build is 3% slower on the branch (73.42 against 71.27 ns
per call) and fastest with aligned loops (66.89 ns). Only the insertion guard
on this branch reaches that path, so its share cannot be separated from
placement without hardware counters.

## Verification

Executed locally on the changed areas (`vec`, `btree`, `btree-set`):
formatting, tests, release and overflow-checked tests, Clippy, API pins,
actual Rust Kani harnesses (including the B+ lookup, fixed lookup and range
harnesses, three each), direct
product-source Verus, direct-source negative controls, projected models
(`verify_btree`), ASan scenarios, the B+ consumer link audit, and the fixed
B+ Miri tests on x86-64 and i686 (6 each) passed.

Source pins updated with the changes: BTree review seals
(`scripts/btree_bindings.py`), the pinned `node::insert` and
`BorrowState::take` bodies (`scripts/btree_callers.py`), the B+ source seals
(`scripts/bplus_bindings.py`, `tools/consumer-audit/check_bplus_fixed.py`),
the B+ range harness count (`scripts/source_certificate.py`), and the dedup
routing described above. The maintained `btree_actual_insert` model proves
the copy indices for every tail length, including zero, so the guard needs
no model change.

Two checks fail identically on unmodified `d1a04e7`:

- `unsafe`: the inventory expects G16 = 116 and G17 = 63; both revisions
  count 117 and 72.
- `link`: the same retained panic edges remain (6/9/6/81 in
  release/LTO/checked/debug). The modified and unmodified sets match by
  function, source location and target.

Not run locally: Miri (all three configurations), fuzzing, coverage, and the
global certificate with Kani and mutation controls. The global certificate
runs in pull-request CI.

## Reproduction

Build the measured executables from the same checkout into ignored
directories, then run the timing drivers on an otherwise idle pinned CPU:

```sh
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --target-dir target/verification/bottlenecks/bin \
  --features hash-map,hash-set,owned-budgets,bplus-pool \
  --bin jzap-throughput-analysis --bin btree --bin btree_set --bin bplus
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --target-dir target/verification/bottlenecks/bench --features bplus-bench --bin bplus_range
bin=target/verification/bottlenecks/bin/release
python3 tools/throughput-analysis/compare.py --binary new=$PWD/$bin/btree_set --cpus 6 \
  --tasks btree_set_iter,btree_set_build --small-tasks btree_set_iter,btree_set_build \
  --small-variants new --output target/verification/bottlenecks/btree_set
python3 tools/throughput-analysis/report.py --input target/verification/bottlenecks/btree_set
BPLUS_FIXED_ONLY=1 BPLUS_BATCHES_8_2=40000 BPLUS_BATCHES_32_2=13000 BPLUS_BATCHES_16_3=11200 \
  taskset -c 6 $bin/bplus 1
taskset -c 6 target/verification/bottlenecks/bench/release/bplus_range 1 8
```

Pass a second `--binary` built from `d1a04e7` to repeat the paired
comparison. Raw records stay under `target/verification/`.
