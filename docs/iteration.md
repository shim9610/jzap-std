# Fallible iteration

Use the open `jzap::TryIterator` and `jzap::TryIntoIterator` traits for input that
can fail. User implementations are trusted to follow their documented failure
contract. They need neither a private marker nor a standard `Iterator` impl.

| Trait | Operation | Meaning |
| --- | --- | --- |
| `TryIterator` | `try_next() -> Result<Option<Item>, Error>` | Item, end, or source failure |
| `TryIntoIterator` | `try_into_iter() -> Result<IntoIter, Error>` | Conversion may fail before traversal |
| `TryDoubleEndedIterator` | `try_next_back()` | Failure channel from the other end |
| `TryExactSizeIterator` | `try_len() -> Result<usize, Error>` | Exact remaining item count |
| `TryFusedIterator` | Marker contract | After a successful end, later calls remain ended |

`TryIterator` also provides `try_size_hint`, `try_nth`, `try_fold`,
`try_for_each`, `try_by_ref`, and `try_take`. The other traits provide
`try_nth_back` and `try_is_empty`. This is the initial trait foundation, not the
complete standard iterator method catalogue: native fallible `map`, `filter`,
and `zip` adapters are not yet provided.

An iterator is its own `TryIntoIterator`. Mutable references and
`Box<dyn TryIterator<Item = T, Error = E>>` forward iteration. Existing Vec/Box
borrowed and owning iterators, drains, splices' removed-item traversal, and
extraction have fallible counterparts.

Types that also implement standard `Iterator` share its `try_fold` and
`try_for_each` method names. Select the intended trait explicitly on those types,
for example `TryIterator::try_fold(&mut iter, initial, callback)`.

## Implementation contract

Return supported arithmetic, indexing, allocation, and callback failures through
the associated error type. Prefer `Checked`/`TryArithmetic` for arithmetic and
checked accessors for indices. Compute the next state before committing it when
failure should preserve state. Describe consumption and cleanup on failure,
including the behavior of invoked callbacks and destructors.

```rust
use jzap::{Checked, Error, TryIterator};
struct Numbers { next: Checked<usize>, remaining: Checked<usize> }
impl TryIterator for Numbers {
    type Item = usize;
    type Error = Error;
    fn try_next(&mut self) -> Result<Option<usize>, Error> {
        if self.remaining.get() == 0 { return Ok(None); }
        let remaining = (self.remaining - Checked::new(1)).map_err(|_| Error::Capacity)?;
        let next = (self.next + Checked::new(1)).map_err(|_| Error::Capacity)?;
        let item = self.next.get();
        self.remaining = remaining;
        self.next = next;
        Ok(Some(item))
    }
}
```

Consumers stop at the first error. An error may be resumable; neither `Err` nor
`Ok(None)` promises permanent exhaustion without an additional contract.
`try_take(n)` limits attempts, so errors also use one attempt. A zero limit does
not query or advance the source. `TryFoldError::Source` retains a traversal error;
`Operation` retains the processing callback's error.

## Container boundaries

| API | Error and state contract |
| --- | --- |
| `Vec::try_extend_fallible` | Accepts `TryIntoIterator<Item = T, Error = E>`; returns `InitError<E>`; error rolls back the appended suffix, preserving original elements |
| `Vec::try_from_fallible_iter[_in]` | Same input/error contract; partial new storage is cleaned up |
| `Box<[T]>::try_from_fallible_iter_in` | Same input/error contract; partial new storage is cleaned up |
| `Box<str>::try_from_fallible_chars_in` / `try_from_fallible_texts_in` | Propagates conversion, traversal, and hint failures; cleans up partial UTF-8 storage |
| `Vec::try_splice_fallible` | Conversion failure precedes removal; `Splice::try_finish` returns traversal/storage errors; already inserted values and the retained tail remain |

`InitError::Initialize(e)` retains a conversion, hint, or traversal error.
`InitError::Storage(e)` identifies a container failure. Consumption and external
effects are not rolled back. Vec suffix rollback may retain enlarged capacity.
Splice Drop never advances replacement input. For an infallible source,
`Splice::finish` retains its storage-only error result.

`iter::from_std(source)` explicitly bridges ordinary items with error type
`Infallible`. `iter::from_results(source)` promotes result-valued items into the
iteration failure channel. These bridges retain the standard implementation's
execution behavior. Existing ordinary `try_extend`, `try_from_iter`,
`try_splice`, and text-input APIs use explicit bridges internally. At the
fallible-input APIs, wrap old result-valued standard inputs explicitly:

```rust
use jzap::{iter::from_results, vec::Vec};
let input = from_results([Ok(1), Err("source failed"), Ok(3)]);
let result = Vec::try_from_fallible_iter(input);
assert!(matches!(result, Err(jzap::error::InitError::Initialize("source failed"))));
```

There is no blanket conversion of every standard iterator into the open traits.
A fallible signature alone does not prove that an implementation avoids panic.

## Checking a consumer implementation

```sh
python3 scripts/check_consumer_link.py
python3 scripts/check_consumer_link.py --source /absolute/path/my_iterator_audit.rs
```

The supplied module exports `pub fn run(seed: usize) -> Result<(), jzap::Error>`.
It can use `core`, `jzap`, and its own trait implementations. Exercise the concrete
methods, callbacks, payloads, early cleanup, and error paths used by the consumer.
Start from the [checked example](../tools/consumer-audit/cases/checked.rs).
Additional application dependencies require adapting the fixture and recording
those dependencies; this tool does not import an arbitrary application graph.

The tool requires Linux x86_64, Cargo, a C compiler, and a GNU-compatible linker.
It tests release with overflow checks and `panic=abort`, both with and without fat
link-time optimization (LTO). Its test-only panic handler references an undefined
symbol: a retained panic edge fails the final link. Direct process-exit calls
are also wrapped with undefined symbols. These restrictions are in the test
runtime, not the shipped library, and do not restrict the application's `unwrap`.

Every invocation runs a known-good implementation and negative controls for raw
addition overflow, ordinary indexing, and direct `abort`. The first two contain
no explicit panic or `unwrap`. Build errors and missing control detections fail
the check. Linked roots then execute with boundary seeds and refusal at each
allocation position observed in their initial run. The driver accepts returned
errors, checks live fixture allocations, and applies a runtime timeout. It does
not check whether every returned error is semantically correct; functional tests
must do that.

Logs, link maps, source hashes, tool versions, settings, and results remain under
`target/verification/consumer-iterator/`. Retained unreachable checks require
removal or a source-connected unreachability proof; there is no unresolved-edge
allowlist. The result covers retained direct edges in these concrete roots. It
does not prove all generic instantiations, undefined behavior, termination,
indirect/foreign exits, stack sufficiency, or dynamically loaded code. Combine it
with functional/error-injection tests, Miri/sanitizers, and formal invariant
checks. The library fixture does not certify a different consumer's iterator.
