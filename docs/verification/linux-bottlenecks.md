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
now count the keys below the bound at fixed offsets, as the branch seek
already does. Other widths and key sizes keep the binary search. A new Kani
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
code) and is attributed to placement. Small Vec `dedup` and small BTreeSet
`build` remain above +15%. Streaming lookup and empty/single-item ranges
remain slower than `brie-tree`.

## Examined and left unchanged

| Operation | Gap | Cause found | Reason left unchanged |
| --- | ---: | --- | --- |
| B+ `FixedTree` routing result | — | Width-eight branch search kept `Option<usize>` on the stack | Two source forms compiled to identical machine code; the selected search above removes the case |
| String `from_utf16` | +6.6% | Per code point: checked length add, in-capacity path as a taken branch, length re-check after the write with a materialized `Result` | Moving growth out of line made the default build slower (+6.3% to +7.7%) and was reverted. A std-shaped push changes the proved String edit envelope |
| HashSet small build | +7.4% | Per insert +11 instructions; per resize +85 (copy-then-publish wrapper and guard calls) | Ownership-preserving resize and bounded probing; estimated inlining gain about 1% |
| HashSet `extract` | +19.6% | 19 vs 17 instructions per element: index-based bucket addressing, constant rematerialized, bounded group scan | Bounded `Scan` frontier is proved by an actual Rust kernel |
| VecDeque small `push_back_8` | +8.3% | 117 vs 80 instructions per growth | Spread across budget/layout checks and ring relocation |
| Rc `clone` | +6.7% | Saturating count: load/compare/store instead of `incq` | Part of the shared count protocol |
| Arc `cow_weak`, `unique4096` | +5.8%, +9.4% | Uniqueness pre-check | Part of the shared count protocol |
| `scenario_wordcount` | +5.2% | Default `Hasher::write_str` out of line | `write_str` is unstable to override on Rust 1.95 |
| Scratch `get_8` | +23.6% | One extra move for the `Result` error value, loop crosses a line | Checked view returns `Result` by contract |
| HashMap/HashSet `default_new`, HashSet `owned_*` | — | Recorded in `docs/random-state.md` | Source variants were already tried and reverted there |
| HashSet `iter`, BinaryHeap `iter_8`/`drain_8`, arithmetic `call` | — | Identical or shorter code at a different line offset | Default builds do not force alignment |
| Benchmark-wrapper inlining (BTreeSet disjoint/skew, HashMap `get`) | — | Outlined benchmark adapter | Measurement tool, not product code |

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
