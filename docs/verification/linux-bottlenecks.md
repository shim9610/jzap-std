# Linux binary-level bottleneck follow-up

This report covers the slower operations found by a full Linux benchmark of
public `main` (`d1a04e7`), their binary-level causes, and the changes made on
`work/perf-bottlenecks`. It states what changed, what was examined and left
unchanged, and why. It is not a whole-library performance or MVP completion
claim.

## Conditions

- Intel Core i7-13700KF (P-core CPU 6, hyperthread sibling offline, turbo off,
  fixed 3.4 GHz), Linux 7.0, Rust 1.95.0, release profile of
  `tools/throughput-analysis` (opt-level 3, one codegen unit, panic=abort). No
  compiler, prover or other benchmark ran during a timed interval.
- Collection timings use `compare.py --stack-offsets 0,16,32,48`: every
  measurement runs once per 16-byte stack offset with address randomization
  disabled, and each seed sums the four offsets ([stack-alignment
  control](../benchmarks.md#stack-alignment-control)). Three seeds, intervals of
  at least one second, call-weighted means against the std control of the same
  executable.
- B+ timings use the `bplus` and `bplus_range` binaries against `brie-tree`
  0.1.2 in the same process.
- Causes come from disassembly of the measured executables, ptrace PC
  sampling, single-step instruction traces, LLVM IR, `LD_PRELOAD` call counters
  and `perf stat` counters (`cpu_core` events on CPU 6).

## Results

Unbudgeted (`unlimited`) ratios against std; `d1a04e7` and the final branch
executable in the same run:

| Operation | Input | `d1a04e7` | Branch | Branch seed ratios |
| --- | --- | ---: | ---: | --- |
| Vec `grow` | mixed | +16.7% | +0.3% | 0.998–1.006 |
| Vec `grow` | small | +15.0% | −4.4% | 0.951–0.963 |
| Vec `retain` | mixed | +4.2% | +1.5% | 1.008–1.021 |
| Vec `retain` | small | +6.3% | +3.3% | 1.026–1.043 |
| Vec `dedup` | mixed | +9.2% | +0.8% | 1.008–1.009 |
| Vec `dedup` | small | +21.4% | −11.5% | 0.883–0.887 |
| Vec `extract_all` | mixed | −3.0% | −0.5% | 0.989–1.001 |
| Vec `extract_all` | small | +8.0% | −1.6% | 0.960–1.000 |
| BTreeMap `iter` | mixed | +2.4% | −16.9% | 0.827–0.833 |
| BTreeMap `iter` | small | −41.3% | −48.4% | 0.507–0.522 |
| BTreeMap `remove_cycle` | mixed | −2.7% | −1.5% | 0.983–0.988 |
| BTreeMap `remove_cycle` | small | +12.2% | +1.3% | 0.997–1.022 |
| BTreeMap `build` | small | +16.7% | +9.3% | 1.078–1.109 |
| BTreeSet `iter` | mixed | +34.3% | +1.5% | 1.009–1.020 |
| BTreeSet `iter` | small | +11.0% | −34.2% | 0.648–0.665 |
| BTreeSet `build` | mixed | −4.2% | −6.3% | 0.936–0.939 |
| BTreeSet `build` | small | +16.2% | +20.8% | 1.185–1.221 |
| VecDeque `push_back_256` | mixed | −2.3% | −2.5% | 0.969–0.983 |
| VecDeque `push_back_8` | small | +9.1% | +4.2% | 1.033–1.048 |
| HashSet `build` | small | +8.7% | +4.5% | 1.021–1.062 |
| HashSet `is_disjoint_true` | mixed | +7.8% | +4.2% | 1.036–1.047 |
| String `utf16` | mixed | +6.5% | +3.2% | 1.032–1.033 |

String, HashSet and small Vec `dedup` sources are unchanged between the two
executables (small `dedup` runs the same loop as std); their differences
between the columns are code placement, see [placement](#code-placement).

B+ lookups against `brie-tree` (ns per lookup or range; fixed-only mode, four
seeds; ranges at width 8, three seeds):

| Condition | `d1a04e7` | Branch | `brie-tree` |
| --- | ---: | ---: | ---: |
| `FixedTree::get`, 8 / 2 | 32.54 | 19.73 | 12.02 |
| `FixedTree::get`, 32 / 2 | 24.88 | 24.64 | 29.90 |
| `FixedTree::get`, 16 / 3 | 46.97 | 43.82 | 46.00 |
| Streaming `Lookup`, 8 / 2 | 43.47 | 43.45 | 12.02 |
| Range, 0 items | 13.17 | 12.86 | 8.67 |
| Range, 1 item | 14.59 | 14.72 | 11.96 |
| Range, 32 items | 62.82 | 55.67 | 63.51 |
| Range, 256 items | 286.26 | 270.29 | 241.41 |

Width 16 runs the same code in both executables, so the 16 / 3 difference is
placement.

## Changes

### Vec push growth works on copies of the storage words

`try_push`'s cold growth took `&mut self`. The owner's address then escaped
into an out-of-line call, so the vector stayed in memory and every push stored
`len` to the stack. In the benchmark the push loop also stores its `black_box`
temporary, and the two slots shared a 64-byte line at only one of four
16-byte stack alignments. At the other three, store-buffer-full stalls added
0.67e9 cycles to an otherwise identical 7.69e9-instruction run, so mixed `grow`
took 22% longer depending on the process's stack address.

`try_push` now copies the pointer and capacity into locals and passes those,
with length, limit and accounting, to a cold `unsafe fn grow_for_push`. It
performs the computation of `grow(1, false)`: the same length admission,
growth proposal, layout and allocation or reallocation. Only success writes
the new words into the locals, and `try_push` then stores them in the owner;
a refusal leaves both unchanged. The owner's address no longer escapes, so the
push loop keeps `len` in a register; its only stack store is the benchmark's
temporary.

A first form returned the words as `Result<(NonNull<T>, Capacity<T>), Error>`.
Its push loop is the same, but CBMC did not finish
`proofs::vec_bounded_push_pop_preserves_values_and_budget` with it: the
program grew from 273,210 to 293,713 steps and the solver exceeded a 24 GB
limit (an unlimited local run was killed at 57 GB; the CI runner was stopped
twice). With the out-parameters the harness takes 18.6 s and 1.78 GB, as
before the change (19.7 s, 1.74 GB). Why the returned tuple is hard for the
solver was not determined.

Ten-target assembly of the push loop (instructions / stack stores / stack
loads per element):

| Target | std | Before | After | Limited before | Limited after |
| --- | --- | --- | --- | --- | --- |
| x86-64 Linux, FreeBSD, Windows, macOS | 10/2/2 | 13/2/4 | 10/1/1 | 13/2/4 | 10/1/1 |
| AArch64 Linux, Android, Windows | 11/2/2 | 12/2/3 | 10/1/1 | 12/2/3 | 10/1/1 |
| AArch64 macOS | 11/2/2 | 12/2/3 | 9/1/1 | 12/2/3 | 9/1/1 |
| ARMv7 Android | 16/2/3 | 15/2/1 | 12/1/1 | 15/2/3 | 12/1/1 |
| i686 Linux | 14/3/4 | 18/3/6 | 17/3/4 | 18/3/6 | 15/2/4 |

On i686 the eight general registers spill the two halves of each `u64`, so the
store count stays; no target got worse. Each growth costs more at its call
site: on x86-64 Linux the path from the growth branch back into the loop is 12
instructions with `&mut self`, 19 with the returned tuple and 21 with the
out-parameters.

Paired timing of the two forms (same run, four stack offsets, three seeds;
ratio to the std control of the same executable):

| Input, policy | Returned tuple | Out-parameters |
| --- | ---: | ---: |
| mixed, unbudgeted | +1.8% | +1.7% |
| mixed, `limited` | +1.7% | +1.7% |
| mixed, `metered` | +1.8% | +1.5% |
| small, unbudgeted | −0.0% | +0.9% |
| small, `limited` | +13.9% | +5.4% |
| small, `metered` | −1.9% | −0.8% |

For small `limited` the out-parameter form executes 1.9% more instructions
(23.03e9 against 22.60e9) and takes 8% fewer cycles (4.31e9 against 4.70e9);
the difference is front-end bound slots (15.7% against 25.1%). The returned
tuple form measured −7.6% for the same case in the executable of the
[results](#results) table, so these small-input differences follow code
placement. The [results](#results) table was measured with the returned-tuple
form and was not repeated with the out-parameters.

A new Kani harness, `push_growth_matches_grow_one_for_every_full_numeric_owner`,
checks for every full numeric owner under both length policies that `try_push`
publishes the capacity `grow(1, false)` proposes, or refuses with the same
error and unchanged words. Changing the current-capacity argument to zero or
the growth to exact is detected; changing the additional count to zero is an
equivalent mutant (doubling or the later capacity refusal gives the same
result) and is not used as a control.

### Growth paths inline `storage::reallocate`

An out-of-line `storage::reallocate` kept the budget branches, a stack round
trip of the reservation result and a memory-passed `Result` in every Vec and
VecDeque growth of unmetered owners: 116 instructions per growth against std's
80. It is now `#[inline(always)]`; a growth takes about 90. Its body and the
accounting order are unchanged. Small VecDeque `push_back_8` moved from +9.1% to
+4.2% of std.

### BTree leaf insertion skips empty moves

`node::insert` shifted the pair suffix with two `ptr::copy` calls even at the
end of a node, where both are zero-length `memmove` calls. std's
`slice_insert` skips the move. The moves now run only for a nonzero tail.
`LD_PRELOAD` counts over 20 epochs:

| Workload | Zero-length `memmove` before | After | std |
| --- | ---: | ---: | ---: |
| BTreeMap small `btree_remove_cycle` | 11,547 | 6,325 | 6,325 |
| BTreeSet small `btree_set_build` | 3,820 | 2 | — |

The guard's effect on the small BTreeSet build is described in [BTreeSet small
build](#btreeset-small-build).

### Vec `retain` keeps the predicate state in a register

jzap reloaded the retain threshold from memory for every element (`cmp
0x68(%r12),%rax`); std kept it in a register. LLVM IR shows that std's
`Vec::retain` receives the closure by value, so its captured reference is a
`noalias` argument whose scope proves stores into the vector cannot change it.
jzap's `retain`, `retain_mut` and `try_retain` forwarded the closure through a
`&mut` capture. The wrappers now move the closure, and on all ten targets the
inner loop no longer loads the threshold.

### Vec `dedup` uses the unpaired sweep

The paired compaction sweep took a branch in both halves for duplicates (1.78
taken branches per pair against std's 1.22). `try_dedup_by` now calls the
compaction driver with `PAIRED = false`; the loop is std's instruction
sequence. The source bindings and the maintained memory model
(`tools/memory-proof/edit-loops.verus.rs`) follow the routing.

### BTree borrowed iteration keeps the in-leaf step as the loop body

The in-leaf advance jumped to a shared tail block: 11 instructions and 2 taken
branches per element against std's 9 and 1. Source reordering did not change
the layout; `core::hint::cold_path()` on the leaf-boundary path makes the
in-leaf step a fall-through body of 10 instructions and 1 taken branch.
AArch64 already used that shape; i686 and ARMv7 gained a dedicated in-leaf
loop.

### Width-eight `FixedTree` point lookup uses conditional selection

`FixedTree::get` used branch search for nodes of at most eight keys (the
`SMALL_NODE_KEYS` rule from `5313656`, measured on AMD EPYC for streaming
lookup). On this CPU it compiled to two data-dependent branches per level and a
stack round trip of the routing result. One A/B campaign (four seeds,
ns/lookup): branch search 32.50, conditional selection 19.64, fixed-offset
count 24.02 (`brie-tree` 12.23). `get` now uses conditional selection at every
width. On AArch64, ARMv7 and i686 the machine code is unchanged (already
`csel`). Streaming `Lookup` with conditional selection took 50.24 against
43.41 ns in all four seeds and is unchanged.

### Width-eight `FixedTree` range leaf bounds use fixed offsets

For width eight and keys of at most eight bytes on 64-bit non-Windows targets,
both leaf bounds now count the keys below the bound at fixed offsets, as the
branch seek does; the out-of-line `partition_point` call is gone. On 32-bit
targets a `u64` comparison takes two instructions and the count grew the range
code (i686 311 to 408, ARMv7 363 to 424 instructions); without a timing there,
32-bit targets keep the binary search and their code equals `d1a04e7`. The Kani
harness `width_eight_leaf_prefix_matches_binary_search` checks the count against
`prefix_len` for every sorted `[u8; 8]`, query and inclusivity. Against the
previous branch (three seeds, ns/range): 0 items 13.17 to 12.83, 1 item 14.67
to 14.71, 32 items 62.96 to 56.23, 256 items 285.75 to 274.10. Counting only the
lower bound was slower for empty and single-item ranges.

## Code placement

Some gaps follow where code sits, not what it executes. Counters on CPU 6:

| Case | Counter evidence | Conclusion |
| --- | --- | --- |
| Small Vec `dedup` | jzap loop equals std's; jzap default 1.90e9 cycles, 3.66e6 uop-cache misses, front-end bound 51.8%; std 1.61e9, 0.61e6, 44.3%; jzap with `align-loops=64` 1.64e9, 1.01e6, 42.7% | The hottest branch crossed a 64-byte line in that build |
| HashSet `extract` | Same instructions in both executables (2.99e9 for std); std mispredictions 3.49e7 in `d1a04e7` against 4.58e7 on the branch; jzap 4.4–4.6e7 in both | The std control's branch prediction depends on its address |

Each placement-sensitive workload in one stack-controlled run with three
executables (ratio to std):

| Workload | `d1a04e7` | Branch | Branch, `align-loops=64` |
| --- | ---: | ---: | ---: |
| Vec `dedup`, small | +21.4% | −11.3% | −0.0% |
| BTreeSet `build`, small | +16.2% | +20.2% | +7.9% |
| HashSet `extract`, mixed | +20.2% | +1.1% | −7.2% |
| BinaryHeap `peek_mut_8`, mixed | +5.6% | +4.4% | +2.8% |

The branch small `dedup` result is below std because std's control is slower
in that executable (28.69 against 24.83–25.04 ns elsewhere). Default builds do
not force loop alignment; the opt-in setting is in [build
tuning](../build-tuning.md).

### BTreeSet small build

The small set build is the largest remaining gap. Single-step traces per
inserted key (same executable):

| Part | std | jzap |
| --- | ---: | ---: |
| Search and loop (std also drops here) | 81.3 | 62.2 |
| Insertion with splits and planning | 53.1 | 63.8 |
| Drop guard (`NodeGuard::drain`) | — | 12.1 |
| Allocator and libc | 26.7 | 26.4 |
| Total | 161.5 | 165.2 |

Both allocate 0.23 nodes per key. The failure-preserving design (allocating
every split node before the first move, and the progress-recording drop guard)
adds about four instructions per key (2.2%). The cycle gap is front end:

| Counter | std | jzap | jzap, `align-loops=64` (std 2.87e9 cycles) |
| --- | ---: | ---: | ---: |
| Cycles | 2.83e9 | 3.44e9 | 3.12e9 |
| Instructions | 1.34e10 | 1.37e10 | 1.39e10 |
| Front-end bound | 19.2% | 32.9% | 23.1% |
| uop-cache misses | 0.99e6 | 5.22e6 | 3.96e6 |

`try_insert_at` is 2,336 bytes and holds the in-place insert together with the
split planning and commit. The common in-place path executes in 64-byte lines
0, 1, 5, 6, 34 and 35 of the function; std's `insert_entry` (2,010 bytes) runs
its common path in lines 0, 1 and 26. The scattered hot path explains the
uop-cache misses. Separating the split path into a cold function is the
indicated next experiment; it has not been tried.

## Examined and left unchanged

Gaps measured in the first full run of `d1a04e7` unless stated:

| Operation | Gap | Cause found | Reason left unchanged |
| --- | ---: | --- | --- |
| String `from_utf16` | +3.2% (branch) | Per code point: checked length add, in-capacity path as a taken branch, length re-check after the write with a materialized `Result` | Outlining growth made the default build slower; a std-shaped push changes the proved String edit envelope |
| HashSet `build`, small | +4.5% (branch) | Per insert +11 instructions; per resize +85 (copy-then-publish wrapper and guard calls) | Ownership-preserving resize and bounded probing; estimated gain about 1% |
| HashSet `extract` | see placement | 19 vs 17 instructions per element: index-based bucket addressing, constant rematerialized, bounded group scan | The measured gap follows the std control's placement |
| Rc `clone` | +6.7% | Saturating count: load/compare/store instead of `incq` | Part of the shared count protocol |
| Arc `cow_weak`, `unique4096` | +5.8%, +9.4% | Uniqueness pre-check | Part of the shared count protocol |
| `scenario_wordcount` | +5.2% | Default `Hasher::write_str` out of line | `write_str` cannot be overridden on stable Rust 1.95 |
| Scratch `get_8` | +23.6% | One extra move for the `Result` error value, loop crosses a line | The checked view returns `Result` by contract |
| HashMap/HashSet `default_new`, HashSet `owned_*` | — | Recorded in `docs/random-state.md` | Source variants were tried and reverted there |
| HashSet `iter`, BinaryHeap `iter_8`/`drain_8`, arithmetic `call` | — | Identical or shorter code at a different line offset | Default builds do not force alignment |
| Benchmark-wrapper inlining (BTreeSet disjoint/skew, HashMap `get`) | — | Outlined benchmark adapter | Measurement tool, not product code |

Rejected variants: outlining String growth (slower default build); two source
forms removing the `FixedTree` routing `Option` (identical machine code);
conditional binary search for the width-eight range seek (slower at every range
size: 0 items 16.55 against 12.85 ns, 256 items 282.68 against 272.20 ns).

Empty B+ ranges execute about 160 instructions against `brie-tree`'s 79: per
level 27 against 16 (seven fixed-offset comparisons, node-ID and slice checks,
checked leaf rank against an unchecked arena search), leaf 38 against 17, and
about 25 for the end slot and returned cursor. Moving the node-ID and rank
checks to `try_new` with unchecked indexing was bounded by an unproved
experiment (ranges about 1 ns faster when small, 1–2% slower when large;
`brie-tree` unchanged in its gap) and is deferred, see [recorded
decisions](../decisions.md).

## Verification

Run locally on the changed areas (`vec`, `deque`, `btree`, `btree-set`):
formatting, tests, release and overflow-checked tests, Clippy (all features
and core only), API pins, actual Rust Kani harnesses (including the B+
lookup, fixed lookup and range harnesses and the two Vec growth harnesses),
direct product-source Verus, direct-source groups with their source mutants
(`vec-layout-growth` detects all five growth mutants), projected models
(`verify_btree`), ASan scenarios, fixed B+ Miri on x86-64 and i686, and the
certificate stages `bplus`, `owner-protocol` (with mutations) and `link`.

The owner protocol binds the push growth as two more unsafe callers (the call
in `try_push` and the reallocation in `grow_for_push`) and four more state
writes (`*ptr` and `*cap` in `grow_for_push`, `self.ptr` and `self.cap` in
`try_push`), all to the resized-owner contract of `resize_storage`: 181
callers and 91 writes, pinned in the certificate assembly. The unsafe
inventory routes the two new push-growth blocks to the storage group; its
expected totals still differ from `d1a04e7`'s actual counts for
G16 (116 against 117) and G17 (63 against 72), as on `d1a04e7` itself. The
retained panic edges are 6/9/6/81 (release/LTO/checked/debug) and match
`d1a04e7` by function, source location and target.

Source pins updated with the changes: BTree review seals and pinned bodies
(`scripts/btree_bindings.py`, `scripts/btree_callers.py`), B+ source seals and
audit pins (`scripts/bplus_bindings.py`, `tools/consumer-audit/`), the B+ range
harness count (`scripts/source_certificate.py`), the dedup routing, the Vec
push-growth bindings and bodies (`scripts/caller_bindings.py`,
`scripts/callsite_projection.py`) and the unsafe inventory. The maintained
`btree_actual_insert` model proves the copy indices for every tail length,
including zero.

Not run locally: Miri in all three configurations, fuzzing, coverage and the
global certificate, which runs in pull-request CI. Targets other than x86-64
Linux were compared in assembly only, not timed.

## Reproduction

```sh
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --target-dir target/verification/bottlenecks/bin \
  --features hash-map,hash-set,owned-budgets,bplus-pool \
  --bin jzap-throughput-analysis --bin btree --bin btree_set --bin deque --bin hash_set --bin bplus
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --target-dir target/verification/bottlenecks/bench --features bplus-bench --bin bplus_range
bin=$PWD/target/verification/bottlenecks/bin/release
python3 tools/throughput-analysis/compare.py --binary new=$bin/jzap-throughput-analysis \
  --cpus 6 --seeds 1,2,3 --stack-offsets 0,16,32,48 --tasks grow,dedup \
  --small-tasks grow,dedup --small-variants new --output target/verification/bottlenecks/vec
python3 tools/throughput-analysis/report.py --input target/verification/bottlenecks/vec
BPLUS_FIXED_ONLY=1 BPLUS_BATCHES_8_2=40000 BPLUS_BATCHES_32_2=13000 BPLUS_BATCHES_16_3=11200 \
  taskset -c 6 $bin/bplus 1
taskset -c 6 target/verification/bottlenecks/bench/release/bplus_range 1 8
```

Pass a second `--binary` built from `d1a04e7` to repeat the paired comparison.
Raw records stay under `target/verification/`.
