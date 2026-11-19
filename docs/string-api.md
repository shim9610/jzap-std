# String API mapping

Reference: Rust 1.95 stable (`59807616e`), 38 inherent methods.
`python3 scripts/check_string_api.py` checks the pinned source and includes
combined `const unsafe fn` declarations. Nightly-only APIs do not define this
stable completion scope.

| std String method | jzap counterpart |
| --- | --- |
| `new` | `new` |
| `with_capacity` | `try_with_capacity` |
| `from_utf8` | `try_from_utf8` |
| `from_utf8_lossy` | `try_from_utf8_lossy` |
| `from_utf16` | `try_from_utf16` |
| `from_utf16_lossy` | `try_from_utf16_lossy` |
| `into_raw_parts` | `into_raw_parts` |
| `from_raw_parts` | `from_raw_parts` |
| `from_utf8_unchecked` | `from_utf8_unchecked` |
| `into_bytes` | `into_bytes` |
| `as_str` | `as_str` |
| `as_mut_str` | `as_mut_str` |
| `push_str` | `try_push_str` |
| `extend_from_within` | `try_extend_from_within` |
| `capacity` | `capacity` |
| `reserve` | `try_reserve` |
| `reserve_exact` | `try_reserve_exact` |
| `try_reserve` | `try_reserve` |
| `try_reserve_exact` | `try_reserve_exact` |
| `shrink_to_fit` | `try_shrink_to_fit` |
| `shrink_to` | `try_shrink_to` |
| `push` | `try_push` |
| `as_bytes` | `as_bytes` |
| `truncate` | `truncate` |
| `pop` | `pop` |
| `remove` | `remove` |
| `retain` | `try_retain` |
| `insert` | `try_insert` |
| `insert_str` | `try_insert_str` |
| `as_mut_vec` | `as_mut_vec` |
| `len` | `len` |
| `is_empty` | `is_empty` |
| `split_off` | `try_split_off` |
| `clear` | `clear` |
| `drain` | `try_drain` |
| `replace_range` | `try_replace_range` |
| `into_boxed_str` | `try_into_boxed_str` |
| `leak` | `leak` |

Related trait functionality uses explicit APIs: `TryClone`/`TryCloneIn`,
`TryDefault`, `try_from_str`/`try_from_char`/`try_from_cow`, `try_from_chars`/
`try_from_texts` and native fallible iterator extension, `try_push_str` for Add/
AddAssign, `try_write_fmt` and `fmt::TryToString`. Allocation-free Default,
Display, Debug, Hash, Eq/Ord and str borrowing/conversion traits are implemented.
Ordinary allocating Clone/Extend/FromIterator/Add and implicit Deref/Index are
intentionally replaced by these explicit interfaces.

`from_utf8_lossy` returns a jzap `Utf8Lossy` borrowed/owned enum; valid input
borrows without allocation. The owned alternative is jzap String. Encoding
failure carries the original byte owner for UTF-8 adoption, and an unpaired
surrogate value for UTF-16. Range errors distinguish invalid bounds from UTF-8
scalar-boundary errors. Unsafe APIs retain their explicit caller contracts.

String's implicit str method catalogue is available through explicit `as_str`
and `as_mut_str` borrows. It does not inherit the library's failure guarantee.
