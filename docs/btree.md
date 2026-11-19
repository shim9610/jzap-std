# Fallible ordered maps

`jzap::collections::BTreeMap<K, V, P = Unmetered>` owns a degree-six B-tree.
It provides ordered lookup, editing, double-ended traversal, Entry operations,
fallible cloning and explicit iterator construction. Nodes use the existing
allocation and byte-budget policies. Product code adds no runtime dependency.

The pinned stable API checkpoint is implemented and verified. Executed checks,
measured workloads and build conditions are in the [verification report](verification/btree.md).
The [reference review](btree-design.md) identifies the fixed upstream sources.

```rust
use jzap::{Error, TryClone, collections::BTreeMap};

fn example() -> Result<(), Error> {
    let mut map = BTreeMap::new();
    map.try_insert(7u64, 19u64).map_err(|rejected| rejected.error)?;
    *map.entry(11).try_or_insert(23).map_err(|rejected| rejected.error)? = 29;
    assert_eq!(map.get(&11), Some(&29));
    assert_eq!(map.try_range(7..=11)?.count(), 2);
    let copy = map.try_clone()?;
    assert_eq!(map, copy);
    Ok(())
}
```

## Standard-library counterparts

The reference is Rust 1.95.0's stable API. Compiler-gated nightly methods are
outside this inventory. Methods that cannot fail keep their usual return shape.

| std operation | jzap counterpart | Failure behavior |
| --- | --- | --- |
| `new`, `Default`, `clear`, `len`, `is_empty` | Same names | Empty construction allocates nothing; clear releases all nodes |
| `get`, `get_mut`, `get_key_value`, `contains_key` | Same names; heterogeneous `Borrow<Q>` lookup | Absence is `Option` or `false`; no allocation |
| `insert` | `try_insert` | Returns `Rejected<(K,V)>`; preserves map and input on refusal |
| `entry`, `first_entry`, `last_entry` | Same names | Entry search allocates nothing |
| `first_key_value`, `last_key_value` | Same names | Shared borrowed pair or absence |
| `remove`, `remove_entry`, `pop_first`, `pop_last` | Same names | Move ownership out; allocation-free rebalancing |
| `iter`, `iter_mut`, `keys`, `values`, `values_mut` | Same names | Borrowed double-ended traversal |
| `range`, `range_mut` | `try_range`, `try_range_mut` | Reversed bounds and equal excluded bounds return `Error::Bounds` |
| `retain` | `try_retain` | Predicate error keeps prior removals/value edits and the current pair |
| `extract_if` | `try_extract_if` and `TryIterator::try_next` | Range validation and predicate failures have separate return paths |
| `split_off` | `try_split_off` | Reserve the new border before moving pairs; refusal preserves source |
| `append` | `try_append` | All allocation/accounting admission precedes mutation; refusal preserves both maps |
| `into_iter`, `into_keys`, `into_values` | Same names; fallible iterator mirrors also implemented | Own remaining pairs; early drop releases the remaining forest |
| `Clone` | `TryClone`, `TryCloneIn`, `TryClone::try_clone_from` | Partial destination is cleaned; source and existing destination preserved on error |
| `FromIterator`, `Extend`, array conversion | `try_from_iter`, `try_from_iter_in`, `try_extend`, `try_from_array` | Iterator/storage error is explicit; extension retains its successful prefix and rejected pair |
| Comparison, hashing, formatting | `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`, `Debug` | Ordered contents; user trait implementations retain their execution contracts |
| Indexing | `get` / `get_mut` | Explicit absence instead of indexing panic |

Occupied Entry has `key`, `get`, `get_mut`, `into_mut`, `insert`, `remove` and
`remove_entry`. Vacant Entry has `key`, `into_key`, `try_insert`,
`try_insert_entry` and `try_insert_with`. Entry provides `try_or_insert`,
`try_or_insert_with`, `try_or_insert_with_key`, `try_or_default`,
`try_insert_entry` and `try_and_modify`. Initializers return `Result`; storage
admission happens before an initializer runs. Occupied alternatives do not run
initializers or allocate nodes.

## Ownership and failure contracts

| Operation | Preserved state / committed progress | Allocation and budget |
| --- | --- | --- |
| Single insertion | A refusal preserves shape, order, length, old values and both input objects. Equal-key replacement retains the stored key and returns the old value. | Checked total length; reserve the complete split chain before committing |
| Entry initialization | A returned initialization error preserves the key and map; external callback side effects remain. | Refusal does not invoke the initializer; unused reservations are released |
| Entry modification | `try_and_modify` propagates the callback error and retains value edits already made by that callback. | No allocation |
| Remove / pop / clear | Returned pairs move to the caller; clear drops all pairs. | No new allocation; refund follows actual deallocation |
| Range creation | Returned bounds error preserves the map. Mutable traversal checks physical cursor order independently of `Ord`. | No allocation |
| Retain / extract | Earlier edits/removals remain. A predicate error consumes that visit, retains the current pair and its value edits, and leaves extraction ready for the next candidate. Dropping extraction leaves unvisited pairs. | No allocation |
| Split | Returned storage error preserves the original tree; success moves the suffix. | New border uses the source policy; both outputs retain that policy |
| Append | Returned admission refusal preserves both trees. Success empties the source; duplicate keys retain the destination key and take the source value. | Recycles input nodes by allocation kind; reserves bounded spare-node credits before commit |
| Clone / clone-from | Error cleans initialized prefixes; old destination remains until a complete replacement exists. | `TryCloneIn` propagates the target local budget into newly owned nested allocations |
| Extend / construction | Successful prefix remains; iterator error and rejected owned pair are distinguished by `ExtendError`. Failed construction drops its partial map. | Each input pair uses single-insertion admission |

`new_in` selects `Unmetered`, `Budget`, or `ConcurrentBudget`. The budget counts
requested node bytes, with no duplicate node charge. Appending between different
budget objects reserves the transferred node bytes in the destination while the
source charge remains held, then transfers accounting at commit. Nested payload
allocations retain their own policies. A source using the same policy type but a
different budget object is supported.

Safe user callbacks, `Ord`, `TryClone`, formatting, hashing, and destructors must
return according to their contracts. Ordering changes are logic errors; they
must not create out-of-bounds access or overlapping mutable yields. Cleanup
guards also exercise a single destructor unwind. A callback's arbitrary abort or
nontermination cannot be converted into a library error. Public unsafe caller
contracts and allocator behavior follow the [project support boundary](support.md).

Container operations use bounded tree height and parent links. Insertion/search
and deletion are logarithmic in tree size. Traversal is linear in visited pairs;
append consumes both input streams and split additionally counts one result
subtree to establish exact lengths. Node moves do not call user code.

## Borrowed node slices

`root_view` exposes the owned tree's initialized key/value slices and checked
child navigation. Its lifetime freezes the map while the view is in use.

`view::NodeSlice` borrows caller-owned typed key/value slices and optional child
identifiers. Construction checks equal key/value lengths, strictly increasing
keys, and exactly one more child than keys for an internal node. External node
size is independent of the owned tree's eleven-pair capacity.

```rust
use jzap::{Error, collections::btree_map::view::{LookupStep, NodeSlice}};

fn external_lookup_step() -> Result<(), Error> {
    let root_keys = [10u64];
    let root_values = [100u64];
    let children = [1usize, 2];
    let root = NodeSlice::new(&root_keys, &root_values, Some(&children))?;
    assert_eq!(root.lookup(&17)?, LookupStep::NeedNode(2));

    let leaf_keys = [17u64];
    let leaf_values = [170u64];
    let leaf = NodeSlice::<_, _, usize>::new(&leaf_keys, &leaf_values, None)?;
    assert_eq!(leaf.lookup(&17)?, LookupStep::Found(&17, &170));
    Ok(())
}
```

`NodeSlice::lookup` performs one local step. `NeedNode` carries a copied
identifier, so the caller can release or reuse the current buffer before the
next step. Only a `Found` result borrows its node data.

`view::Lookup` additionally validates visited node kind, expected height,
ancestor identifiers and inherited separator bounds. `view::Traversal` supports
ordered full/range traversal and returns `Visit::NeedNode`, `Visit::Item`, or
`Visit::Done`. A rejected supplied node does not advance the continuation. These
continuations retain separator/path borrows, so supplied slices must remain valid
for their use; the query's lifetime is independent of the returned node data.
Validation covers visited nodes. Locally valid unseen nodes are not certified.

The caller supplies slices and chooses when to fetch another node. The library
does not define file formats, I/O, mappings, caches, persistence or recovery.

### B+ tree point lookup

`view::bplus::Node` and `view::bplus::Lookup` support complete keys and values
stored in leaves. Each branch separator is the minimum of its child, and the
branch has one child identifier per separator. The lookup follows the last
separator at or below the query, including an exact match. It checks the
expected node kind, child minimum, inherited exclusive ceiling, and ancestor
identifiers on the visited path.

```rust
use jzap::{Error, collections::btree_map::view::{LookupStep, bplus::{Lookup, Node}}};

fn bplus_lookup() -> Result<(), Error> {
    let query = 20u64;
    let mut keys = [10, 20];
    let mut lookup = Lookup::new(0usize, 1, &query)?;
    let root = Node::<_, u64>::branch(&keys, &[1, 2])?;
    assert_eq!(lookup.provide(0, &root)?, LookupStep::NeedNode(2));

    keys = [20, 25];
    let values = [200, 250];
    let leaf = Node::leaf(&keys, &values)?;
    assert_eq!(lookup.provide(2, &leaf)?, LookupStep::Found(&20, &200));
    Ok(())
}
```

Keys copied into the lookup can outlive the supplied node borrow. A key that
contains a reference still borrows its referent. A `Found` result borrows the
supplied leaf and a rejected node can be replaced without restarting the
lookup. Supply a consistent tree snapshot for the lifetime of the lookup.

The B+ API covers point lookups with complete keys. The design issue tracks
partial-key comparison and range traversal. [B+ tree lookup design](https://github.com/shim9610/jzap-std/issues/4)

The point-lookup benchmark compares the same resident node slices against a
checked Rust reference, with and without local node validation in the timed
path. It also includes `brie-tree` 0.1.2 and the standard `BTreeMap` as whole-index
references. The executable checks every query against the standard map before
timing. Build first, then run each seed on the same idle CPU.

```sh
cargo build --locked --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --features bplus-bench --bin bplus
for seed in 1 2 3; do
  tools/throughput-analysis/target/release/bplus "$seed"
done
```

On Linux, build with `bplus-memory` to record lookup allocation counts and bytes
using the instrumented system allocator. Use the uninstrumented `bplus-bench`
build for timing. Both builds report the size of the lookup continuation.

#### Complete-key prototype timing

The Linux x86-64 run used Rust 1.95.0, the release profile, and CPU affinity 2
on an AMD EPYC 9V74. Each build ran 54 intervals across three seeds, with matching
query counts and checksums before and after inlining. The shortest interval was
1.066 seconds. All table values are call-weighted ns/lookup. The range in
parentheses gives the fastest and slowest seed means for the jzap lookup.

| Fanout / height | jzap before → after inlining | Checked reference | jzap with validation | Checked reference with validation | brie-tree | std BTreeMap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 8 / 2 | 69.11 → 45.84 (41.62–52.13) | 48.40 | 72.58 | 62.61 | 13.30 | 11.51 |
| 32 / 2 | 108.85 → 82.42 (77.85–85.93) | 81.40 | 146.67 | 119.48 | 31.77 | 29.69 |
| 16 / 3 | 140.62 → 120.67 (115.96–123.29) | 104.46 | 160.16 | 131.19 | 48.57 | 42.02 |

The generated lookup loop after inlining omits the per-node `provide` and
`descend` calls. The prevalidated jzap means range from 5.3% below to 15.5% above
the same-layout reference. The two whole-index references use their own node
layouts and trust their internal structure. The memory run observed zero heap
allocations, zero peak and retained allocation bytes during the lookup intervals,
and a 584-byte `Lookup<u64, usize>` on this target. Node and index construction
occur outside those intervals.

#### Branch routing follow-up

Internal nodes select the last admitted separator with a single ordering test
per iteration. In the measured `u64` build, the branch search uses a comparison
and one direction branch in place of the three-way decision used for exact-key
search. Leaves still use exact-key search. Node construction uses the slice's
strict ordering predicate, and descent checks and splits the active ancestor
prefix once before recording the current node.

The paired rerun used the same machine, CPU affinity, and release settings.
It reran `f563596` and the candidate with identical query counts and checksums,
alternating execution order by seed. All 108 valid intervals lasted at least
one second. Values are call-weighted ns/lookup.

| Fanout / height | Prevalidated jzap before → after | Checked reference | jzap with validation before → after | Checked reference with validation | brie-tree | std BTreeMap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 8 / 2 | 43.83 → 38.05 | 49.71 | 71.47 → 71.55 | 63.20 | 13.54 | 12.50 |
| 32 / 2 | 88.70 → 86.48 | 83.17 | 145.30 → 143.46 | 122.68 | 31.93 | 32.98 |
| 16 / 3 | 122.49 → 114.05 | 103.37 | 156.87 → 154.15 | 136.63 | 46.63 | 45.64 |

The final prevalidated seed ranges were 33.81–43.39, 79.37–93.53 and
108.26–128.15 ns/lookup, respectively. At height three the final mean was
10.3% above the same-layout reference. Repeated prevalidated reference means
shifted by −3.7% to +5.3%, and the standard map shifted by up to +21.2%.
The validation-inclusive path still
costs 12.8–16.9% more than the checked reference, and the specialized whole
index remains faster. The allocation run again observed zero lookup allocations,
zero peak and retained allocation bytes, and a 584-byte continuation.

#### Node ID width

The benchmark uses the same prevalidated lookup for `jzap` (`usize` node IDs)
and `jzap_u32` (`u32` IDs). Both fixtures use identical keys, values, numeric
node IDs, and topology. Fixture construction and node validation run before
timing. Each method must match the standard map on every query before timing.
The `u32` measurement includes the checked conversion of the root index. Its
child-link storage uses 4 bytes per link, compared with 8 bytes for `usize`
on this target.

The release build used Rust 1.95.0 on an AMD EPYC 9V74, pinned to Linux CPU 2.
The three seeds provided 63 intervals. The shortest lasted 1.054 seconds.
Each case and seed used matching operation counts and
checksums across all seven methods. Both relative orders of the ID variants
occurred for each case. Times are call-weighted nanoseconds per lookup.
The target gap is the measured jzap time minus the `brie-tree` time from
the same run.

| Fanout / height | `brie-tree` target | jzap `usize` | `usize` target gap | jzap `u32` | `u32` target gap | `u32` − `usize` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 8 / 2 | 14.11 | 42.92 | +28.81 | 39.05 | +24.94 | −3.87 |
| 32 / 2 | 32.65 | 80.61 | +47.96 | 79.22 | +46.57 | −1.39 |
| 16 / 3 | 51.60 | 112.75 | +61.15 | 112.48 | +60.88 | −0.27 |

The per-seed differences for `u32` minus `usize` were −6.20 to −1.65 ns
at 8 / 2, −9.30 to +9.24 ns at 32 / 2, and −4.41 to +3.36 ns at 16 / 3.
Only 8 / 2 showed a reduction in every seed.

The x86-64 release binary initializes the 64-slot ancestor array with 36
stores (512 bytes) for `usize` and 16 stores (256 bytes) for `u32`. The
continuation sizes are 584 and 320 bytes, respectively. The child loads
and scalar ancestor comparisons also change from 64-bit to 32-bit.
This experiment measures the full cost of changing ID width, including
child-link layout, conversion, and generated code.
The instrumented run for seed 1 observed zero lookup heap allocations for
both ID widths in all three cases.

```sh
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --locked --release \
  --manifest-path tools/throughput-analysis/Cargo.toml --features bplus-bench --bin bplus
export BPLUS_BATCHES_8_2=28696 BPLUS_BATCHES_32_2=9839 BPLUS_BATCHES_16_3=9480
for seed in 1 2 3; do
  taskset -c 2 tools/throughput-analysis/target/release/bplus "$seed"
done
```

#### Point-lookup bottlenecks

This experiment uses the same resident fixtures and `usize` node IDs as the
standard benchmark. The generated cases validate every query against the
standard map, and each seed times the same jzap implementation twice to expose
timing variation. `brie-tree` provides the latency target in the same binary.

Rust 1.95.0 on Linux x86-64, pinned to CPU 2 of an AMD EPYC 9V74, produced
108 intervals across three seeds. The shortest interval lasted 1.219 seconds.
All methods matched operation counts and checksums within each case and seed.
The gap is jzap latency minus the `brie-tree` latency in this run. The
fastest variant that retains the lookup checks by case was lazy path
initialization at 8 / 2, and lazy initialization with both search changes
at 32 / 2 and 16 / 3.

| Fanout / height | `brie-tree` target (ns) | jzap baseline (ns) | Baseline gap (ns) | Fastest checked variant (ns) | Checked variant gap (ns) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 8 / 2 | 13.24 | 47.31 | +34.07 | 37.17 | +23.93 |
| 32 / 2 | 34.99 | 97.38 | +62.39 | 64.30 | +29.32 |
| 16 / 3 | 47.96 | 126.87 | +78.91 | 84.78 | +36.82 |

Each comparison below subtracts the stated control time from the variant
time in ns/lookup. Cases that reversed direction across seeds are marked
with an asterisk. Changes affect compiler output and the differences cannot
be added as independent costs.

| Variant and control | 8 / 2 | 32 / 2 | 16 / 3 |
| --- | ---: | ---: | ---: |
| Same baseline measured a second time | +4.22 | +2.76* | −2.81* |
| Lazy versus eager path initialization | −22.91 | −2.17* | −9.00 |
| Conditional move for child selection versus baseline | −0.31* | −11.40 | −20.69 |
| Lower-bound leaf search added to conditional-move child selection | +3.11* | −9.13 | −9.65 |
| Omit cycle detection versus lazy initialization | −11.41 | −13.61 | −11.48 |
| Omit key fence checks versus lazy initialization | −9.44 | −0.97* | −4.12 |
| Omit cycle detection and fences versus lazy initialization | −17.38 | −33.31 | −31.15 |
| Contiguous buffers versus baseline | +7.73* | −3.05 | −1.32* |

The initialization comparison uses identical ancestor checks in both variants.
The eager variant fills the full path, and the lazy variant writes a slot as
each node is visited. The eager variant and the library baseline compiled to
the same 337 instructions, with one encoding difference in the relative
address of a call to the same ancestor-check helper. Both variants call that
helper out of line in this binary. At 8 / 2 the eager variant nevertheless
averaged 12.77 ns above the library baseline. The two execute at different
addresses and read separate view arrays. The measured spread limits the
precision of the 8 / 2 initialization estimate.

The `cmov` variant changes child selection. The second search variant also
uses a lower-bound search followed by one equality test in the leaf. Both
search changes retain
the path checks. The packed fixture places keys, values, and child IDs in
three contiguous buffers and uses the same lookup machine code as the baseline.

The cases omitting cycle detection or key fences are benchmark controls
over the valid, immutable fixtures. A key fence is the key range inherited
from the parent. These controls still check node ID, height, and slice bounds.

Run the diagnostic generator against the experiment snapshot `5da0191`.
It derives its variants from that snapshot's eager-path implementation.

```sh
dir=target/verification/bplus-bottlenecks
python3 tools/throughput-analysis/prepare_bplus_bottlenecks.py "$PWD" "$dir/experiment"
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --locked --release \
  --manifest-path "$dir/experiment/Cargo.toml" --features bplus-bench \
  --target-dir "$dir/binary"
export BPLUS_BATCHES_8_2=28000 BPLUS_BATCHES_32_2=9500 BPLUS_BATCHES_16_3=8000
for seed in 1 2 3; do
  taskset -c 2 "$dir/binary/release/jzap-throughput-analysis" "$seed"
done
```

#### Checked point-lookup changes

Each successful descent initializes one ancestor slot. The cycle check reads
the initialized prefix and the candidate child ID. A rejected node leaves
the prefix and pending ID intact for retry. For nodes with more than eight
keys, internal nodes choose the last admitted separator using a conditional
selection and leaves look for the first key at or above the query before
checking it for equality. Nodes with up to eight keys use branch-based
search. The same node ID, height, key fence, cycle, and slice bounds checks
govern lookup.

The release binary omits the full-path initialization in both ID variants.
On x86-64, the larger internal-node search uses `cmov`. The ancestor-array
capacity is still 64 entries on that target, and the instrumented run
recorded zero lookup heap allocations at either ID width.

The paired benchmark builds `5da0191` and the current implementation with
Rust 1.95.0 and `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`. The timed
loop runs only the `jzap`, `jzap_u32`, and `brie` labels. All seven
benchmark methods participate in the per-query comparison with the
standard map. The run uses Linux CPU 2 on an AMD EPYC 9V74 and alternates
the binary order across seeds 1, 2, and 3. The three case-specific batch
counts are `BPLUS_BATCHES_8_2=28000`, `BPLUS_BATCHES_32_2=9800`, and
`BPLUS_BATCHES_16_3=9000`.

All 54 timing intervals had matching query counts and checksums within each
case and seed. The shortest lasted 1.218 seconds. The target gaps subtract
the `brie-tree` mean for the corresponding build before rounding. The table
uses call-weighted nanoseconds per lookup.

| Fanout / height | Current `brie-tree` target | Current `usize` | `usize` gap before → current | Current `u32` | `u32` gap before → current |
| --- | ---: | ---: | ---: | ---: | ---: |
| 8 / 2 | 13.40 | 27.60 | +22.51 → +14.20 | 30.23 | +19.83 → +16.84 |
| 32 / 2 | 31.44 | 62.30 | +55.60 → +30.87 | 63.08 | +52.52 → +31.64 |
| 16 / 3 | 44.14 | 78.32 | +73.46 → +34.18 | 81.03 | +69.38 → +36.89 |

The paired changes in jzap lookup latency, current minus baseline, were
−13.60 to −1.34 ns, −27.02 to −22.37 ns, and −38.26 to −35.47 ns for
`usize`. The `u32` ranges were −10.50 to +2.89 ns, −28.07 to −10.28 ns,
and −40.42 to −23.63 ns. Only 8 / 2 with `u32` changed direction between
seeds. At 16 / 3 the `brie-tree` paired change ranged from −5.15 to +13.48 ns.

An earlier paired run applied both search changes to every node size.
At 8 / 2 that version took 5.81 ns more per lookup for `usize` and
7.87 ns more for `u32` than its baseline. The current version chooses the
search by key count and scans initialized ancestors with a scalar loop.

The all-feature suite passed 437 tests. The three B+ tests passed under native,
i686, and Tree Borrows Miri. The maximum-height test retries after a cycle
using nonzero IDs and borrowed string keys. Two Kani harnesses compared
branch routing and leaf lookup against linear search over up to five keys.
The overflow-checked no-std consumer linked and ran with and without
link-time optimization. Both profiles detected their overflow, bounds, and
abort controls.

#### Fixed-width pool benchmark

The `bplus-pool` benchmark compares prebuilt in-memory point lookups on
512, 32,768, and 65,536 entries. The checked view uses the public jzap
lookup API with prevalidated nodes and retains path checks during lookup.
The resident view validates the entire snapshot during setup. The
`split` and `packed` pools share fixed node widths, numeric IDs, topology,
and search code. The split pool borrows per-node arrays. The packed pool
embeds those arrays in separate contiguous leaf and branch allocations.

Each implementation uses the same `u64` keys and values, 4,096-query sequence
per seed, hit and miss outcomes, and number of lookup calls. The benchmark
builds the trees and validates every query against the standard map before it
starts the clock. Timed lookups use the same dispatcher and checksum.
The `brie-tree` adapter converts the common `u64` query to its
`NonMaxU64` key type during lookup. In the timed binary this adds a
comparison and branch for the reserved `u64::MAX` value.

The run used source `bbea8dc`, Rust 1.95.0, and `brie-tree` 0.1.2 in a single
release binary on Linux x86-64, pinned to virtual CPU 2 on an AMD EPYC 9V74.
Independent processes used seeds 1, 2, 3, and 10. In each fixture, every jzap
path ran before `brie-tree` twice and after it twice. The 60 intervals all
matched call counts and checksums within each fixture and seed. The shortest
lasted 1.308 seconds. Each latency is total elapsed time divided by total
lookup calls. A target gap is the measured latency minus the `brie-tree`
latency from the same run, in ns/lookup.

| Fanout / height | `brie-tree` target (ns) | Checked view gap | Resident gap | Split gap | Packed gap |
| --- | ---: | ---: | ---: | ---: | ---: |
| 8 / 2 | 13.89 | +26.75 | +8.10 | −2.25 | −3.49 |
| 32 / 2 | 32.71 | +29.77 | +18.86 | +0.41 | −2.36 |
| 16 / 3 | 43.44 | +35.92 | +25.88 | −1.49 | −8.02 |

The per-seed gaps show run-to-run variation. SD is the sample standard
deviation of the four paired gaps, in ns/lookup.

| Fanout / height | jzap path | Seed 1 | Seed 2 | Seed 3 | Seed 10 | SD |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 8 / 2 | Checked view | +28.41 | +19.29 | +31.10 | +28.22 | 5.15 |
| 8 / 2 | Resident | +11.13 | +7.98 | +10.39 | +2.89 | 3.72 |
| 8 / 2 | Split | −1.51 | −1.16 | −1.59 | −4.76 | 1.68 |
| 8 / 2 | Packed | −2.96 | −2.38 | −3.05 | −5.55 | 1.41 |
| 32 / 2 | Checked view | +27.84 | +33.14 | +29.63 | +28.46 | 2.37 |
| 32 / 2 | Resident | +22.52 | +24.51 | +16.72 | +11.71 | 5.80 |
| 32 / 2 | Split | −1.93 | +4.00 | +2.54 | −2.99 | 3.39 |
| 32 / 2 | Packed | −6.42 | −2.08 | −0.57 | −0.37 | 2.81 |
| 16 / 3 | Checked view | +42.40 | +36.02 | +33.30 | +31.98 | 4.63 |
| 16 / 3 | Resident | +29.28 | +20.95 | +21.99 | +31.28 | 5.17 |
| 16 / 3 | Split | +6.19 | −4.21 | −2.65 | −5.31 | 5.24 |
| 16 / 3 | Packed | −1.94 | −9.05 | −8.86 | −12.25 | 4.35 |

Height counts edges from the root to a leaf. The jzap fixtures use the fanout
and height shown in the table. The same entries inserted in ascending order
give `brie-tree` 16 key slots per node and root heights of 2, 4, and 5,
respectively. The benchmark compares each library's complete lookup at the
same workload size. The two fixed-width jzap pools also provide a comparison
with matched topology.

The earlier run at `16724a9` recorded zero timed lookup allocations for
both fixed-width layouts across these three fixtures.
See [B+ panic and termination checks](#b-panic-and-termination-checks) for
the verified setup and lookup paths.

```sh
dir=target/verification/bplus-pool-rematch
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --locked --release \
  --manifest-path tools/throughput-analysis/Cargo.toml --features bplus-pool --bin bplus \
  --target-dir "$dir/binary"
export BPLUS_BATCHES_8_2=40000 BPLUS_BATCHES_32_2=13000 BPLUS_BATCHES_16_3=11200
for seed in 1 2 3 10; do
  taskset -c 2 "$dir/binary/release/bplus" "$seed" > "$dir/timing-$seed.csv"
done
```

#### B+ panic and termination checks

The no_std consumer fixture calls the public checked B+ view and the exact
pool source used by the benchmark at widths 8, 16, and 32 with `u64` keys and
`usize` node IDs. The benchmark above uses the pool's verified height loop.

Pool construction publishes one record per accepted page. A branch resolves
its child IDs against the completed prefix, verifies their height and key
bounds, and only then calls `try_push`. Missing or forward child IDs return
`Err`. Splitting validates the partition and fixed array widths before
returning the pools. Verus checked five constructor lemmas, including the
prefix invariant preserved by each successful append and the capacity of
each split pool. The replay checks the reviewed Rust bodies and reuses the
Vec, layout, storage, and budget contracts from `42afdab` after confirming
their sources match. The caller proof relies on those contracts and on the
allocator returning a suitably aligned block or a null pointer on failure.

Both optimized Linux x86-64 `panic=abort` links with overflow checks passed,
with and without link-time optimization. Each ran 228 normal and injected
allocation-refusal cases without leaks. The fixture requires success in each
normal case and `Err` for every allocation refusal. The overflow, bounds,
and abort controls were detected in both profiles.

The strict debug link still retains the unresolved panic sink. The diagnostic
ELF has 17 resolved panic edges, including forwarding edges. The replay
binds all 12 guard kinds to the exact static callers in this fixture,
including the size-two window in node ordering, the three binary-search
caller paths, and the shared Vec and allocator paths. The ordering guard
can only take the suffix at index 1 after obtaining a window of length 2.
The index proofs show that each binary-search access and endpoint is in range.
The replay pins the Rust 1.95.0 slice and iterator sources and rejects an
unreviewed caller with an unchanged guard symbol. The debug diagnostic also
ran all 228 normal and allocation-refusal cases. These results apply to the
resolved edges of this concrete ELF and the reviewed source contracts.
User comparisons and external allocators have their own contracts.

The pool passed six Kani harnesses over the actual Rust implementation.
That includes the constructor with an arbitrary machine-word child ID on
a two-page fixture, branch validation with arbitrary child summaries, the
exact-key lookup oracle, and one-step lookup for all three widths. The
constructor's general prefix claim comes from the separately bound Verus
proof. Allowing a child to point at the page being appended caused the
Verus proof to fail. All three pool tests passed natively and in x86-64
and i686 Miri.

```sh
python3 tools/consumer-audit/check_bplus_debug.py
python3 scripts/check_consumer_link.py \
  --source tools/consumer-audit/cases/bplus_pool.rs \
  --source-dependency tools/throughput-analysis/src/bplus_pool.rs \
  --require-success
cargo kani --manifest-path tools/throughput-analysis/Cargo.toml \
  --features bplus-pool --bin bplus --harness pool::proofs::
for target in x86_64-unknown-linux-gnu i686-unknown-linux-gnu; do
  cargo +nightly-2026-10-30 miri test --locked --target "$target" \
    --manifest-path tools/throughput-analysis/Cargo.toml --features bplus-pool --bin bplus
done
```

## Consumer checks and build tuning

Use the open `TryIterator` / `TryIntoIterator` interfaces when a source may fail.
Their signatures provide error propagation, not proof of user implementations.
Ordinary iterator interoperability remains explicit; callers can still choose
standard collection or unwrap operations in their own code.

The [consumer link fixture](iteration.md) detects retained termination edges in
the concrete compiled call graph. Run it against the actual consumer roots,
payload callbacks, features, target and optimization profile. The fixture does
not restrict what a downstream user can compile. Debug precondition guards need
source-connected unreachability checks even when optimized roots remove them.

Loop alignment remains a consumer build choice under [build tuning](build-tuning.md).
No build script injects compiler flags into downstream applications.
