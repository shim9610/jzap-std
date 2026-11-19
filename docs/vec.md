# Vec with an optional element limit

Vec implementation work is provisionally closed at the
[current checkpoint](verification/vec-optimization.md). Functional coverage and
the source-connected safety checks are complete within their documented scope;
default-build performance regressions remain recorded separately.

`jzap::vec::Vec` provides fallible contiguous storage. Select a maximum element
count at construction when growth must stop at a known bound:

```rust
use jzap::{vec::Vec, Error};

let mut values = Vec::<u64>::with_limit(3);
assert_eq!(values.capacity(), 0); // Selecting a limit does not allocate.
for value in [10, 20, 30] {
    values.try_push(value).unwrap();
}
let rejected = values.try_push(40).unwrap_err();
assert_eq!(rejected.error, Error::Capacity);
assert_eq!(rejected.value, 40);
assert_eq!(values.as_slice(), &[10, 20, 30]);

assert_eq!(values.pop(), Some(30));
values.try_push(rejected.value).unwrap();
assert_eq!(values.as_slice(), &[10, 20, 40]);
```

The example unwraps expected successes for brevity. Application code can handle
allocation, layout, budget, and capacity errors explicitly. `with_limit` creates
an empty owner; it does not promise that later allocations will succeed.

## Construction and storage

| Interface | Meaning |
| --- | --- |
| `Vec::new()` | No configured element ceiling; empty and allocation-free |
| `Vec::with_limit(max_len)` | Empty owner with a fixed maximum element count |
| `try_with_capacity(capacity)` | Fallibly reserve initial capacity without a configured limit |
| `try_with_capacity_and_limit(capacity, max_len)` | Initial capacity and an independent fixed ceiling |
| Corresponding `_in` constructors | Select the outer allocation's budget policy |
| `len`, `capacity`, `max_len`, `remaining_limit` | Initialized count, reserved storage, ceiling, and remaining allowance |

An element limit counts values, including zero-sized values. Byte budgets account
for requested allocations, including spare capacity; they also reserve the entire
new allocation while old storage remains live during growth. They do not
implicitly reassign budgets inside values moved into the vector.

For nonzero-sized values, `len <= capacity <= max_len`. Geometric growth clamps
to the ceiling instead of rejecting a fitting insertion because a doubling would
overshoot. Removing values and clearing do not change the ceiling. Zero-sized
values need no allocation and expose the ceiling as their logical capacity.

`Vec<T>` uses zero-sized unlimited/unmetered policies. On the measured 64-bit
target its handle is 24 bytes, matching std. Selecting an element limit adds
8 bytes; selecting a budget adds another 8 bytes. These costs are explicit and
are not hidden by a payload-only memory comparison.

The storage resize helper is always inlined (`69daa38`). With a runtime-selected
budget policy LLVM otherwise kept it out of line, adding about 16 cycles per
small budgeted clone. The change left the machine code of the other benchmark
binaries byte-identical; measurements are in the
[budget cost breakdown](budget.md#measured-cost-breakdown-2026-11-09-local).

`try_extend_cloned`, and therefore `try_clone` of a Vec or BinaryHeap, copies
copy-witnessed elements without their own bulk hook (arrays of primitives,
nested arrays) with one `copy_nonoverlapping` after the checked reservation
(2026-11-10). Before, such elements went through one initializer slot per
element and one per array item: cloning a 256-element heap of `[u64; 32]`
executed about seven times std's instructions. The branch uses the
`try_extend_copy` admission and publication, and is bound to the same
reserved-suffix contract in the source projection.

## Implemented operations and failure state

| Family | Implemented interface | Returned-error behavior |
| --- | --- | --- |
| Reservation | `try_reserve`, `try_reserve_exact` | Pointer, length, contents, capacity, and budget unchanged |
| Shrinking | `try_shrink_to`, `try_shrink_to_fit` | Same preservation on refusal |
| Single-value editing | `try_push`, `try_insert`, `pop`, `remove`, `swap_remove` | Rejected owned input is returned; invalid indexes are explicit errors |
| Removal | `truncate`, `clear` | No allocation; ceiling and capacity retained |
| Copy and clone extension | `try_extend_copy`, `try_extend_cloned` | Copy refusal changes nothing; clone failure rolls back the suffix, retaining any successful reservation |
| Initialization/resizing | `try_resize`, `try_resize_with`, `try_resize_with_init` | Original contents/length survive error; initialized suffix is cleaned; capacity may have grown |
| Iterator extension | `try_extend`, `try_extend_fallible` | Original contents/length survive error; yielded inputs, including a rejected extra item, are consumed |
| Owner-to-owner editing | `try_append`, `try_split_off` | Refusal preserves both existing owners; split keeps the source policy and ceiling |
| Clone | `TryClone`, `TryCloneIn` | Source preserved; target-budget cloning uses the output element layout |
| Clone replacement | `try_clone_from` | Destination policy and ceiling retained; error preserves contents; supported equal-length Copy paths reuse storage |
| Access | Checked `get`/`get_mut`, first/last, explicit slices, borrowed iteration | Invalid checked ranges return Bounds; explicit standard borrows retain standard contracts |
| Owning iteration | Double-ended, exact-size, fused `IntoIter` | Remaining values are dropped exactly once; no cloning iterator adapter hides allocation |
| Ownership transfer | Raw parts, standard Vec adoption/transfer, Box slice transfer | Explicit accounting/limit boundary; rejected adoption or shrinking returns the original owner |

`try_resize(new_len, &value)` borrows the repeated value. The initialization form
writes directly through `InitSlot`, permitting large arrays without a whole-value
stack temporary. Extension iterator hints are not trusted as bounds or used to reject an
otherwise fitting input. A single iterator callback's termination remains its
own contract.

Standard-owner adoption rejects capacity above the selected limit even if the
current length would fit. `into_std` explicitly ends jzap's accounting and limit
contract. Unsafe raw recovery requires the exact allocation, initialized prefix,
capacity, policy charge, and limit invariant. Safe owner transfers do not expose
a mutable standard owning Vec inside the jzap wrapper.

## Range edits and explicit completion

`try_drain(range)` checks bounds before borrowing the vector for removal. Its
iterator is double-ended and exact-size. Drop removes the remaining selected
values and restores the retained tail, without allocating. Forgetting the
iterator can leak removed values and the tail, as with std's draining iterator.

`try_extract_if(range, predicate)` lazily yields `Result<T, E>`. The predicate
returns `Result<bool, E>`; true extracts the value. On an error, the iterator
yields that error once and becomes exhausted. The failing value and unvisited
suffix are retained, including mutations already made by the predicate. Early
Drop retains the unvisited values and does not call the predicate again.

`try_splice(range, replacement)` returns a double-ended iterator over removed
values. **Call `finish()` to insert replacement values and receive allocation,
budget, layout, or element-limit errors.** Drop alone removes the selected range
and discards the unconsumed replacement iterator. It cannot report a replacement
failure, so it never performs replacement. This explicit completion is the
functional alternative to std's allocation during `Splice::drop`.

```rust
use jzap::vec::Vec;
let mut values = Vec::try_from_array([1, 2, 3, 4]).unwrap();
let mut edit = values.try_splice(1..3, [8, 9, 10]).unwrap();
assert_eq!(edit.next(), Some(2));
edit.finish().unwrap();
assert_eq!(values.as_slice(), &[1, 8, 9, 10, 4]);
```

If completion fails, inserted values stay inserted and the original retained
tail survives. Removed values and consumed replacement items are not restored.
Exact-length hints enable one tail movement. With no useful lower bound, the
first yielded excess item justifies a checked reservation; the iterator then
fills the resulting spare space. Only overflow beyond that gap is staged. The
final growth avoids doubling capacity twice for one replacement.
Hints are clamped to the logical limit and never establish a capacity rejection
without a yielded item. A dishonest overestimate can cause an unnecessary
reservation and therefore an allocation/budget error; it cannot cause unsafe
access. Iterator termination and side effects remain caller-code boundaries.

## Additional adapters

| Standard function family | Explicit interface and behavior |
| --- | --- |
| Retain | `try_retain`, `try_retain_mut`; error retains the current and unvisited suffix after earlier removals; infallible-callback `retain` / `retain_mut` also available |
| Deduplication | `try_dedup_by`, `try_dedup_by_key`, `dedup`, `dedup_by`, `dedup_by_key`; compare later value first, as std does |
| Mutable insertion result | `try_push_mut`, `try_insert_mut`; refusal returns the owned input |
| Conditional pop | `try_pop_if`, `pop_if`; predicate mutations remain on refusal |
| Internal extension | `try_extend_from_within`, bulk `try_extend_copy_from_within`; checked range and transactional appended suffix |
| Owned resize | `try_resize_owned`; clone n−1 times and move the input once, returning it on failure |
| Spare storage | `spare_capacity_mut`, `split_at_spare_mut`, unsafe `try_set_len`; length bounds are checked, initialization remains an unsafe caller obligation |
| Conversions | Fallible array/slice/iterator/Cow/string-byte construction, `try_into_array`, `try_into_flattened`, explicit Box/std/raw transfers |
| Leaking | `leak`; deliberately retains the allocation and budget charge; raw recovery needs original capacity and policy |
| Iterator ownership | `as_mut_slice`, `TryClone`, `TryCloneIn`, Debug, Default, AsRef/AsMut, double-ended exact-size/fused iteration; clone replacement retains destination policy/limit |
| Comparisons and borrowing | Slice/array/cross-policy equality, ordering, hashing, Borrow/BorrowMut, AsRef/AsMut |
| Byte writer | `try_write`, `try_write_all`, `try_write_vectored`, `try_write_fmt`, allocation-free `flush`; formatting rolls back appended bytes and remembers ignored write failures |

Flattening preserves the **numeric element limit**. A limited Vec of arrays can
therefore reject flattening if the resulting element length or capacity exceeds
that limit; rejection returns the original owner. Unlimited vectors retain the
std behavior, with explicit overflow errors for zero-sized elements. Empty arrays
produce an empty vector without allocating.

Array inputs/outputs are by-value APIs and have the caller's ordinary stack-size
constraints. For large values use the recorded initialization APIs; the tests
exercise 256KiB array elements on a 64KiB thread stack.

## Coverage boundary

The [stable API mapping](vec-api.md) enumerates all 46 inherent public methods in
the installed Rust 1.95 source and their explicit counterparts. Related trait
families have explicit adapters where implicit allocation or indexing is forbidden.
Slice methods remain accessible through the explicit `as_slice` / `as_mut_slice`
interoperability boundary; this does not make arbitrary standard slice operations
part of jzap's checked failure contract. Nightly-only Vec features are listed
separately by the inventory checker and do not define the stable completion scope.
General field-level `TryClone` derive support remains separate.

See the [implementation contract](vec-contracts.md) and
[verification report](verification/vec/README.md) for checks and measurement limits.

## Optional build tuning

The application can select compiler-default, 32-byte or 64-byte loop alignment.
See [optional build tuning](build-tuning.md) for opt-in Cargo configuration files,
equivalent RUSTFLAGS, and configuration precedence. These options are not Cargo
features and do not alter element limits, budgets or failure behavior.
