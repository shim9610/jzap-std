# Rust 1.95 Vec functional mapping

All 46 stable inherent methods are mapped below. This is a functional mapping,
not identical syntax or an unconditional proof over all caller implementations.
`scripts/check_vec_api.py` checks the pinned rust-src inventory against these
maintained adapter names and the product source.

| std Vec method | jzap counterpart |
| --- | --- |
| `new` | `new` |
| `with_capacity` | `try_with_capacity` |
| `from_raw_parts` | `from_raw_parts` |
| `into_raw_parts` | `into_raw_parts` |
| `push` | `try_push` |
| `push_mut` | `try_push_mut` |
| `capacity` | `capacity` |
| `reserve` | `try_reserve` |
| `reserve_exact` | `try_reserve_exact` |
| `try_reserve` | `try_reserve` |
| `try_reserve_exact` | `try_reserve_exact` |
| `shrink_to_fit` | `try_shrink_to_fit` |
| `shrink_to` | `try_shrink_to` |
| `into_boxed_slice` | `try_into_boxed_slice` |
| `truncate` | `truncate` |
| `as_slice` | `as_slice` |
| `as_mut_slice` | `as_mut_slice` |
| `as_ptr` | `as_ptr` |
| `as_mut_ptr` | `as_mut_ptr` |
| `set_len` | `try_set_len` |
| `swap_remove` | `swap_remove` |
| `insert` | `try_insert` |
| `insert_mut` | `try_insert_mut` |
| `remove` | `remove` |
| `retain` | `try_retain` |
| `retain_mut` | `try_retain_mut` |
| `dedup_by_key` | `try_dedup_by_key` |
| `dedup_by` | `try_dedup_by` |
| `pop` | `pop` |
| `pop_if` | `try_pop_if` |
| `append` | `try_append` |
| `drain` | `try_drain` |
| `clear` | `clear` |
| `len` | `len` |
| `is_empty` | `is_empty` |
| `split_off` | `try_split_off` |
| `resize_with` | `try_resize_with` |
| `leak` | `leak` |
| `spare_capacity_mut` | `spare_capacity_mut` |
| `resize` | `try_resize_owned` |
| `extend_from_slice` | `try_extend_cloned` |
| `extend_from_within` | `try_extend_from_within` |
| `into_flattened` | `try_into_flattened` |
| `dedup` | `dedup` |
| `splice` | `try_splice` |
| `extract_if` | `try_extract_if` |

## Related trait families

| Standard interface | Explicit counterpart / boundary |
| --- | --- |
| Clone, clone_from | TryClone, TryClone::try_clone_from; allocating Clone intentionally absent |
| FromIterator, Extend | try_from_iter, try_from_fallible_iter, try_extend, try_extend_fallible |
| From array/slice/Cow/str | try_from_array, try_from_slice, try_from_cow, try_from_str; `_in` constructors select policy/limit |
| Vec ↔ Box slice | from_boxed_slice, try_into_boxed_slice; explicit standard-owner transfers also available |
| Vec → array | try_into_array, returning the original owner on length mismatch |
| Default / Drop | Non-allocating empty owner / guarded element and allocation cleanup |
| IntoIterator | Owned and shared/mutable borrowed iteration; iterator cloning is fallible |
| PartialEq / Eq / PartialOrd / Ord / Hash | Implemented; comparison/hash callbacks retain their caller-code contracts |
| AsRef / AsMut / Borrow / BorrowMut | Explicit slice borrows, with the standard slice boundary visible |
| Debug | Formatter result; arbitrary element formatting is caller code |
| Index / IndexMut / Deref / DerefMut | Checked get/get_mut and explicit slice borrows, no implicit panic-indexing path |
| std::io::Write for Vec<u8> | try_write/all/vectored/fmt and flush; fixed-size errors, no hidden I/O error allocation |
| Send / Sync / Unpin / unwind auto-traits | Follow element ownership and policy constraints; Cell-based budget prevents cross-thread sharing |

## Deliberate API differences

- `Splice::finish` is required for replacement; its result reports failures that std hides in Drop.
- Fallible extraction yields one callback error, then ends while retaining the unvisited suffix.
- `try_set_len` checks capacity/limit but remains unsafe because only the caller can prove initialization.
- `try_resize_owned` preserves std’s final move; the earlier borrowed `try_resize` remains available.
- Raw parts include the allocation and length policies; flattening preserves the numeric limit.
- General allocator plugins and nightly-only Vec methods are not stable Vec requirements.
- Conversions into other allocating collections (Arc/Rc/String/etc.) belong to those destination types. Explicit std transfer is an interoperability boundary, not proof that their allocation paths are fallible.

The [guide](vec.md) specifies failure state and [verification report](verification/vec/README.md) records tests and performance.
