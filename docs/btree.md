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
