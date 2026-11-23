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

Unbudgeted (`unlimited`) ratios against std; `d1a04e7` and the branch
executable before the strided B+ merge (`4b5439a`) in the same run. After the
merge, the same stack-controlled run against `d1a04e7` (CPU 6, three seeds)
showed no regression against this table, and strided lookups matched the
strided-page branch head within about 2%; its rows are not republished here.
The changes after the merge are reported in their own sections:

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

### `FixedTree` point lookup reads the validated pools unchecked

After the conditional-selection change, each width-eight branch level of
`FixedTree::get` still executed about 24 instructions: a node-ID subtraction
with a borrow check and a branch-pool bound check, a first-key fence, the
routing result stored to the stack as `Option<usize>` and reloaded, and a
child-slot bound check; the leaf added a leaf-ID and a value-slot check. All
of these are guaranteed by `try_new`: leaves have height zero, a branch has
the checked successor of its children's common height, every child ID is
below its parent's ID, and the root is the last record.

`get` now reads the branch, child, leaf and value records with
`get_unchecked`; the branch offset is the library's bounded
`wrapping_sub(id, leaves.len())`. `floor_slot` keeps the conditional binary
search but starts at slot zero and returns a plain `usize`, so its result is
below `N` for every order. The first-key fence runs at the root only: with a
stable order every lower node's minimum equals its separator, so the removed
fences could not reject a query that the root admitted. The routing `Option`
came from that per-level fence, which is why the two earlier source forms that
removed only the `Option` compiled to the same code. `range` keeps its checks.

The public `try_new` and `get` are thin wrappers. `try_new` allocates the
validation summaries and hands their spare slots to `assemble`; `get` builds
the key comparisons as closures and calls `find`. `assemble`, `summarize`,
`summarize_branch`, `find`, `floor_slot`, `leaf_slot` and `leaf_rank` carry
Verus specifications on the product source (next subsection). Leaves of at
most eight keys use `leaf_slot`, the iteration count and conditional selection
of `slice::binary_search_by` in Rust 1.95; wider leaves use `leaf_rank`, the
`slice::partition_point` algorithm of the same release followed by an equality
check. `summarize` writes one summary per record into the vector's spare
capacity after a single length check, `summarize_branch` reads the written
prefix and writes the next slot, and the summaries are never dropped because
they hold only references and integers. The public signature is unchanged and
the methods contain no public `unsafe`.

In the release build of a width-eight `u64` lookup, a branch level is 14
instructions with no stack access and no branch other than the loop, and the
leaf search is branch-free; in the benchmark executable the three `fixed`
lookup closures went from 106, 103 and 103 to 74, 68 and 68 instructions. The
lookup closures of the current source have the same machine code, function by
function after address normalization, as the `as_ptr().add()` form measured
below; `leaf_slot` compiles to the same code as `binary_search_by`.

Paired A/B of that lookup code against the previous branch build (`1c1a689`)
in one run, on a cloud Intel Xeon at 2.30 GHz pinned with `taskset` to one CPU,
without frequency control. Seeds 1, 2, 3 and 10, two rounds with alternating
build order, batches 60000/20000/17000, every interval at least 1.53 s,
call-weighted ns/lookup:

| Width / height | `FixedTree`, previous | `FixedTree`, unchecked | Ratio | Seed ratios |
| --- | ---: | ---: | ---: | --- |
| 8 / 2 | 14.95 | 12.48 | 0.835 | 0.721–0.905 |
| 32 / 2 | 24.45 | 21.76 | 0.890 | 0.763–0.997 |
| 16 / 3 | 39.66 | 24.69 | 0.622 | 0.607–0.648 |

The other lookup closures, including `brie-tree` and `packed`, have the same
instructions in both executables at shifted addresses and still moved
(`brie-tree` 11.61 → 15.71 ns at width eight). These cloud numbers show the
direction of the change only and are not comparable with the pinned-PC
figures. Timings on the pinned Linux PC have not been run.

`try_new` was measured on its own: the pools are prepared outside the interval
and the interval repeats `FixedTree::try_new` only. Against `e25b03b` (the
summaries pushed into a vector), the current form executes, per Cachegrind:

| Width / height | Instructions | Loads | Stores | L1/LL misses |
| --- | ---: | ---: | ---: | --- |
| 8 / 2 | −4.4% | −4.7% | +4.0% | same |
| 32 / 2 | −12.0% | −2.0% | −1.1% | same |
| 16 / 3 | −17.4% | −3.7% | −3.4% | same |

The width-eight stores are the register saves of the outlined
`summarize_branch` call; inlining it removes them but adds loads and stores in
the child loop at every width. Paired three-way timing on the same cloud CPU,
six seeds by four rounds with rotating order, every interval at least
1.147 s:

| Width / height | `e25b03b` | Current | Ratio | Current faster |
| --- | ---: | ---: | ---: | ---: |
| 8 / 2 | 252.9 ns | 229.9 ns | 0.909 | 23/24 |
| 32 / 2 | 8,116.0 ns | 7,621.4 ns | 0.939 | 20/24 |
| 16 / 3 | 24,998.5 ns | 22,701.6 ns | 0.908 | 17/24 |

Rejected `try_new` forms: checked slot access (instructions +9.3%, 3.0–7.0%
slower), unchecked access with two stack reloads per child, and index-driven
leaf and parent loops that recompute each record address (+8.6% instructions
at width eight); slice iterators and taking the leaf's first and last keys in
the loop restored the advantage.

#### Verification of the unchecked reads

- Verus, directly on the product source (`verify_direct_verus.py`, target
  `fixed-bplus`, `--no-cheating`; whole crate 279 verified, 0 errors). The
  type invariant of `FixedTree` states that the root is the last record and
  every branch names only earlier records exactly one level lower. `assemble`
  establishes it from the summaries; `find` uses it to prove that every branch
  read has `leaves.len() <= id`, every record index stays inside its pool and
  the descent ends at a leaf, for every pool content, child ID and key order.
  `summarize` and `summarize_branch` write slot `written` only after
  `written` slots and read only below it. Key comparisons reach the verified
  code as closures whose results are not assumed. `core::hint::select_unpredictable`
  is the one core function vstd does not specify; its documented equivalence
  to `if c { a } else { b }` is an `assume_specification` in the separate
  crate `tools/direct-verus/std-specs.rs`, checked against the installed core
  source by `scripts/verus_std_specs.py` and imported by every product-crate
  Verus run. Seven source defects (reading a child at the written prefix,
  accepting mixed child heights, a branch height of +2, a leaf height of one,
  a root past the last record, a route past the middle slot, a value past the
  match) each leave a solver error.
- The ten wrapping calls in `fixed.rs` are covered as `direct:fixed-bplus`;
  the coverage check requires the enclosing function to carry a `verus_spec`.
- Kani, actual product source (`fixed::proofs`, 11 harnesses,
  `tools/consumer-audit/check_bplus_fixed_get.py`). A key whose every
  comparison returns an arbitrary result models every order. Ten harnesses
  verify: width two up to height two and width three up to height one with
  arbitrary child IDs, `floor_slot` for widths 1–5, 7–9, 16, 17, 32, 33 and
  64, the leaf searches for widths 7, 8, 9 and 16, `u8` equivalence with a
  linear search and the previous checked lookup at heights one and two, and
  the three earlier fixed harnesses. Five source defect controls are rejected.
- Kani 0.68.0 with CBMC 6.11.0 reports a counterexample for the width-one
  harness (single-key chains up to height four): two `get_unchecked`
  precondition assumptions and the `wrapping_sub` assertion. Every concrete
  playback of it diverges natively ("Not enough det vals found"): the real
  execution needs more comparison results than the trace holds, so the trace
  is not an execution of the program. Building the pools as literals, or
  widening the key to two bytes, still fails; reading the same records through
  `as_ptr().add()` verified. This is the class of the open
  [model-checking/kani#5104](https://github.com/model-checking/kani/issues/5104)
  (false counterexamples for pointer reads), whose stated trigger does not
  match ours, so the cause inside the verifier is not identified. The check
  requires exactly these failed checks and native divergence of every
  playback, never counts the harness as verified, and stops for review if the
  failure changes, a playback reproduces or the harness passes.
- The width-one shapes are covered by the Verus proof and by
  `fixed_bplus_width_one_chains_read_only_their_pools`: every width-one tree
  of the harness shapes, every child ID below the record count and every
  comparison outcome sequence (5,641 runs, 292 accepted trees) return `Ok`
  and only values from the pool, natively with debug precondition checks and
  in x86-64 and i686 Miri.
- Miri: the fixed B+ tests, including a width-two, height-three tree whose
  order becomes pseudo-random after validation and the width-one test, pass
  in x86-64 and i686 Miri (8 tests each).
- The debug consumer audit binds the new `get_unchecked`, `get_unchecked_mut`,
  range `get_unchecked` and `split_at_mut_unchecked` precondition checks from
  `try_new`, `summarize_branch` and the inlined lookup to their callers (22
  resolved panic edges, 17 guard kinds).
- `tools/consumer-audit/bplus_fixed.rs` keeps the unique-parent and range
  lemmas bound to the `try_new`, `get`, `summarize` and `summarize_branch`
  items by digest (15 verified); the lookup's descent and address facts are
  now the direct proof above rather than lemmas over a model.

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
uop-cache misses.

`1c1a689` keeps only the in-place insert in `try_insert_at` and moves
empty-root and split insertion into `#[cold] #[inline(never)] fn
try_insert_split`. Paired A/B on CPU 6 against the merged build:

| Workload | Before | After | Against std, before → after |
| --- | ---: | ---: | --- |
| BTreeSet `build`, small | 75.2 ns | 60.1 ns | +20.4% → −8.5% |
| BTreeMap `build`, small | 86.6 ns | 74.2 ns | — |
| BTreeMap `build`, mixed | 35.90 µs | 36.95 µs | −2.4% after |

The mixed BTreeMap build is 2.9% slower in absolute time and still 2.4%
faster than std. It does not execute more work. In Callgrind on the same
mixed corpus (20 epochs, 722,744 insertions), the insertion code went from
261.6 to 232.2 instructions per insertion, the split path runs for 11.9% of
insertions (85,734 calls) and the std control executes identical instruction
counts in both executables. A paired run of the two executables on a cloud
Intel Xeon at 2.30 GHz (one CPU, three seeds, two rounds, every interval at
least 1.13 s) measured the split build at 0.904 (mixed) and 0.863 (small) of
the merged build, while the unchanged std control moved by +4.9% and −9.2%
between the executables. The pinned-PC slowdown is within that placement
range; it was not traced with counters on that PC, so its physical cause
there is not established.

## Examined and left unchanged

Gaps measured in the first full run of `d1a04e7` unless stated:

| Operation | Gap | Cause found | Reason left unchanged |
| --- | ---: | --- | --- |
| String `from_utf16` | +3.2% (branch) | Per code point: checked length add, in-capacity path as a taken branch, length re-check after the write with a materialized `Result` | Outlining growth made the default build slower; a std-shaped push changes the proved String edit envelope |
| HashSet `build`, small | about +6% (merged build) | About 35 more instructions per insert: 11.5 in insert and about 19 in resize; `try_reserve` stays out of line | Ownership-preserving resize and bounded probing. Inlining the storage free (`Free::drop`) into resize in `1c1a689` measured 170.8 → 168.9 ns, with the ratio still about +6% |
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

### Follow-up verification (`06d8b7f`)

The cold split, the inlined storage free and the `FixedTree` direct proof
were checked as follows. Commits are those the results come from; the
product changes after `584a08a` touch only `fixed.rs`, `bplus.rs` (one Verus
attribute) and `lib.rs` (the verification-only import).

| Check | Commit | Result |
| --- | --- | --- |
| Quick verify, areas `btree`, `btree-set`, `hash-map`, `hash-set`, `vec` | `1c1a689` plus the first lookup form | pass |
| Miri x86-64, i686 and Tree Borrows, the same five areas | `2835f63`, `bad541a`, `584a08a` | 15 of 15 pass |
| Fixed B+ Miri x86-64 and i686 (8 tests each) | `06d8b7f` | pass (certificate stage `bplus`) |
| Fixed B+ Miri, Tree Borrows (8 tests) | `06d8b7f` | pass (`-Zmiri-strict-provenance -Zmiri-tree-borrows`) |
| Direct Verus `fixed-bplus` with 7 source defects | `06d8b7f` | 279 verified, 0 errors; 7 of 7 detected |
| Kani `fixed::proofs` with 5 source defects | `06d8b7f` | 10 verified, the width-one harness recorded as a false counterexample (5 playbacks diverge), 5 of 5 rejected |
| Debug consumer audit | `06d8b7f` | 22 resolved panic edges, 17 guard kinds, caller mutations rejected |
| Clippy, all features | `06d8b7f` | no warnings |
| Global certificate, pull-request CI ([run 45127296343](https://github.com/shim9610/jzap-std/actions/runs/45127296343)) | merge of `06d8b7f` into `main` (`2a703f3`) | 17 stages and the composition pass; `complete`, no reused models, 0 open source obligations, 0 unproved link edges |
| Certificate stages run locally one at a time | `06d8b7f` | the first 15 stages pass; stopped during `link` once the CI composition had passed |

The CI composition checks the pull-request merge commit, not `06d8b7f` alone.
Its source obligations do not include the B+ files: their unchecked reads are
bound by the direct proof, Kani, Miri and the debug audit above, not by the
obligation count ([shim9610/jzap-std#16](https://github.com/shim9610/jzap-std/issues/16)
tracks how that count is reported).

Pins updated: the `btree_map.rs` review seal and the split insertion bodies
(`scripts/btree_bindings.py`, `scripts/btree_callers.py`), the HashMap
`raw.rs` seal, the B+ source seals, the fixed item seals and the debug-audit
caller paths. The unsafe inventory counts G16 at 131 (117 before; the 14 unsafe
blocks of `fixed.rs`) and G17 at 72; G17 and the `unsafe fn` count (117 at
`d1a04e7`, 118 with `grow_for_push`) had been above their recorded values
since before `d1a04e7`, the HashMap increase coming from the ARM64 control-group
commits. The expected values now record 608 blocks (G16 131, G17 72), 118
`unsafe fn`s, 2 macro `unsafe fn`s and 42 `unsafe impl`s, and
`check_unsafe_inventory.py` passes.

Not run: fuzzing, coverage, timings on the pinned Linux PC, and targets other
than x86-64 Linux.

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
