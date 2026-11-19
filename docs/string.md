# String with an optional byte limit

`jzap::string::String` owns UTF-8 text using the existing fallible Vec storage.
`String::with_limit` selects a maximum **byte** length independently of allocated
capacity and the byte budget. `BoundedString<P>` names the limited policy type.

```rust
use jzap::{Error, string::String};

fn example() -> Result<(), Error> {
    let mut text = String::with_limit(7);
    text.try_push_str("é😀")?; // Six UTF-8 bytes.
    assert_eq!(text.try_push('é'), Err(Error::Capacity));
    assert_eq!(text.as_str(), "é😀");
    text.try_push('a')?;
    assert_eq!(text.pop(), Some('a'));
    Ok(())
}
```

All safe edits preserve UTF-8. Byte positions inside a scalar return
`Error::CharBoundary`; positions outside the text return `Error::Bounds`.
Truncation at or beyond the current byte length is a no-op, matching std.

| Family | Interface | Failure state |
| --- | --- | --- |
| Construction | `new`, `with_limit`, `try_with_capacity`, `try_with_capacity_and_limit`; `_in` variants select the allocation policy | Empty construction does not allocate; refusal creates no owner |
| Text and character append | `try_push_str`, `try_push`, `try_append` | Original text and a source owner survive refusal |
| Insert/replace | `try_insert`, `try_insert_str`, `try_replace_range` | Validate boundaries and final length, then reserve before moving bytes |
| Removal | `truncate`, `pop`, `remove`, `clear` | No allocation; invalid byte positions are explicit errors |
| Retain | `retain`, `try_retain` | Earlier removals persist on callback error; current/unvisited scalars survive |
| Range editing | `try_split_off`, `try_drain`, `try_extend_from_within` | Invalid boundaries preserve text; split inherits policy and ceiling |
| Storage | `try_reserve`, `try_reserve_exact`, `try_shrink_to`, `try_shrink_to_fit` | Refusal preserves pointer, text, length and capacity |
| Checked access | `get`, `get_mut`, `is_char_boundary` | Bounds and scalar-boundary errors are distinct |
| Explicit borrows | `as_str`, `as_mut_str`, `as_bytes`, `chars`, `char_indices`, `bytes` | Standard borrowed operations retain core contracts |
| Clone | `TryClone`, `TryCloneIn`, `try_clone_from` | Source is preserved; replacement keeps destination policy/ceiling and reuses capacity |
| Iteration input | `try_from_chars`, `try_from_texts`, `try_extend_chars`, `try_extend_texts`; native fallible variants | Appended suffix rolls back on source/storage error; consumed inputs and callback effects remain consumed |
| Formatting | `try_write_fmt`, `String::try_format`, `fmt::try_format[_in]`, `fmt::TryToString` | Text rolls back on sink/formatter error, including ignored sink errors |

Returned errors from append/formatting rollback can retain capacity acquired by
successful earlier reservations. Retain's unwind guard closes any compaction
gap before the owner becomes accessible again. A user's callback remains
responsible for its own execution and side effects.

String drain follows std String's behavior: Drop removes the entire selected
range, including unyielded characters; forgetting the drain retains the original
text. It supports double-ended standard and fallible iteration. It does not
allocate, and its character borrow ends before bytes are compacted.

## Encoding and ownership conversion

`try_from_utf8` adopts a jzap byte Vec without copying after validation. Invalid
UTF-8 returns `Rejected` containing the unchanged byte owner and `Utf8Error`.
`try_from_utf8_lossy` returns `Utf8Lossy::Borrowed` for valid input, without
allocation; invalid input produces an owned repaired String. A borrowed result
has no owner ceiling or budget charge. `try_to_owned_in` checks these policies
when copying it into owned storage.

`try_from_utf16[_in]` returns an unpaired-surrogate value or a storage error.
`try_from_utf16_lossy[_in]` inserts replacement characters and returns storage
failures explicitly. UTF-16 unit counts are distinct from UTF-8 byte limits.

`into_bytes`, `try_into_boxed_str` and `from_boxed_str` transfer ownership and
budget charges. Box conversion can fail when shrinking; it returns the original
String. `try_from_std_in`, `try_from_cow[_in]` and `into_std` are explicit
standard-owner boundaries. Adoption rejects an allocated capacity beyond the
chosen ceiling, even if the current text would fit. `into_std` ends jzap's budget
and ceiling tracking without copying.

Raw parts, `from_utf8_unchecked`, and `as_mut_vec` retain explicit unsafe contracts.
Callers must preserve allocation identity, initialized extent, budget charge and
valid UTF-8. `leak` intentionally keeps the allocation and its budget charge alive.

## Representation and verification

On the measured 64-bit target the default handle is 24 bytes, matching std.
A byte limit adds 8 bytes; a budget reference adds another 8 bytes. These words
are reported separately from payload allocations.

The [stable API mapping](string-api.md) enumerates all 38 String inherent methods.
The [contracts](string-contracts.md) describe the source references and failure
semantics. Reproduce local checks with `bash scripts/verify.sh --area string --profile memory`. Generated
proofs, code generation and raw timings stay under ignored `target/verification/`.
