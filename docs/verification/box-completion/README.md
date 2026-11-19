# Box API and verification checkpoint

**Status: provisionally closed implementation milestone. Performance remains
provisional pending repeated measurements under more conditions.** The reference
API is Rust 1.95 stable Box. Equivalent behavior is provided through fallible
methods and explicit adapters; identical compiler-special syntax is not required.

The library has no external runtime dependencies. Default builds use `no_std +
alloc`; OS strings and paths require `std`. Core-only foundations build without
default features. Usage is described in the [Box guide](../../box.md).

## Functional mapping

| Standard functionality | jzap interface | Checkpoint status |
| --- | --- | --- |
| `new`, `pin` | `try_new`, `try_new_in`, `try_pin`, `try_pin_in` | Implemented; allocation refusal returns the input |
| `new_uninit`, `new_zeroed` | Fallible counterparts and `_in` variants | Implemented; initialization, zeroing, and refusal tested |
| Uninitialized/zeroed slices | `try_new_uninit_slice`, `try_new_zeroed_slice` and `_in` | Implemented; length, layout, budget, and failure tested |
| Both `assume_init` forms and `write` | Explicit same-name operations | Implemented; unsafe obligations remain explicit |
| Raw ownership and leaking | `into_raw`, `from_raw`, `leak`; pointer/policy pairs for budgets | Implemented; ownership, address, budget, and lifetime tests |
| `into_pin` | `Box::into_pin` returning `Pinned<T, P>` | Implemented; address preservation and !Unpin extraction rejection |
| Any/Error downcast variants | Typed `downcast` methods | Implemented; wrong-type rejection retains ownership |
| Value extraction and unsizing | `into_inner`, `into_slice`, `box_unsize!`, `pin_box_unsize!` | Implemented; compiler-checked pointer coercion |
| Clone and clone_from | `TryClone`, `try_clone_from`, `TryCloneIn` | Implemented; partial cleanup and destination preservation tested |
| Default | `TryDefault`, `try_default_in`; infallible empty slice/str defaults | Implemented; arrays can initialize in destination slots |
| Values and borrowed slices/str | `try_new`, copy/clone-from-slice constructors, `try_from_str` | Implemented; Copy uses a bulk copy |
| Vec/String/Cow input | `try_from_std_vec_in`, `try_from_std_string_in`, `try_from_cow_in` | Implemented; failed shrink retains original pointer, length, and capacity |
| C strings | CStr/CString and validated boxed-byte constructors | Implemented; encoding, layout, address, and metadata checked |
| OS strings and paths | Native borrowed/owned conversion methods | Implemented under std; Linux execution tested, Windows not executed |
| Iterator/character/text collection | Fallible iterator, character, and text-fragment constructors | Implemented; growth, shrink, input errors, and dishonest hints tested |
| Owned and borrowed slice iteration | IntoIterator and double-ended IntoIter | Implemented; only remaining elements are dropped |
| Borrowing and checked access | Sized Deref; explicit AsRef/AsMut/Borrow; dynamic get/get_mut | Implemented; no implicit slice/str indexing |
| Callable and async-callable behavior | Borrowed calls and `box_call_once!` | Implemented; capture consumption, argument panic, and async calls tested |
| Future | `Pinned<F>` and Box of Unpin futures | Implemented; polling preserves the pinned address |
| Iterator traits | Direct sized-iterator forwarding; explicit borrow for dyn Iterator | Implemented; double-ended, length, and skip behavior tested |
| Formatting, comparison, hashing, Error | Forwarding trait implementations | Implemented; typed Hasher overrides are preserved |
| Send/Sync/Unpin | Conditions on payload and policy | Implemented; Cell budgets cannot be shared across threads |
| Payload-specific standard extensions | Explicit `.as_ref()` / `.as_mut()` interoperability | Available under the borrowed payload's own contracts |
| Transfer to standard owners | `into_std` and dedicated Vec/string/path conversions | Explicit ownership transfer; subsequent std operations use std contracts |

Historical std method inventory records 19 stable public
method declarations, including type-specific overloads. Nightly-only custom
allocators and unstable traits are reference material, not required stable syntax.
Additional operations such as `take` have their own ownership contracts. This
matrix does not claim support for unrelated containers such as Arc or Vec.

## Failure and execution contracts

Allocation refusal, budget, layout, bounds, and initialization errors use
allocation-free error values. Recorded writes determine partial cleanup.
`try_init_struct!` checks field completeness and alignment. General fallible
replacement preserves the destination on error; it may need temporary storage.
Small sized values and supported Copy arrays can reuse their allocation. Iterator
consumption and callback side effects are not rolled back.

Under `no_global_oom_handling`, standard CString and slice/str Cow interoperability
are unavailable because the corresponding std facilities are removed. Core CStr,
slice, string allocation, cloning, and collection remain available; the recorded
core/alloc rebuild passed.

The library cannot turn arbitrary termination or nontermination in a user
callback, destructor, or allocator into an error return. Unwind cleanup tests do
not convert user panics into Result errors. Unsafe callers retain validity,
provenance, layout, and ownership obligations.

## Executed verification

| Check | Recorded result and scope |
| --- | --- |
| Deterministic tests | 21 foundation + 30 Box tests passed; allocator failure positions, ownership, addresses, cleanup, budgets, UTF-8, and dynamic types |
| Property/differential tests | Five properties × 2,048 cases; recorded replay seed 123456789 |
| Compile-fail checks | 12 passed; lifetimes, sharing, initialization, indexing, Pin, and macro unsafe boundaries |
| Miri | Stacked Borrows and Tree Borrows/strict provenance passed the 51-test scope; added overflow regression also ran in both models |
| AddressSanitizer | 21 foundation + 30 Box tests passed |
| Basic Kani | 12 implementation harnesses passed within their recorded bounds |
| Enhanced Kani | 3 passed; 3 initialization-check limitations; 4 compiler errors; 2 compiler timeouts at 60 seconds; not counted as successful proofs |
| Verus | 19 source-linked verification results passed; allocator and arbitrary user code remain outside the proof boundary |
| Mutation checks | Four runtime defects and six Verus defects were detected |
| Fuzzing | 10,000 runs over extended slice/string/collection/budget paths |
| Small stack | 64KiB stack with 256KiB arrays/structs, defaults, cloning/replacement, and dynamic slices; debug and release passed |
| panic=abort | Real allocation refusal returned errors for individual values, slices, zeroed storage, and collection |
| Configurations | core-only, alloc, std, wasm32, thumbv7em; Clippy and no-global-OOM rebuild passed |
| Dependencies | Zero runtime dependencies; audit, deny, and software bill of materials generation executed |
| Loom | Not applicable to an existing shared atomic protocol; Cell budget sharing is forbidden |

See the original source commit in [summary.json](summary.json) and the
[proof boundaries](../memory-proof/README.md). In that checkpoint, the
allocator/reallocator module had 98.3% line and
94.4% branch coverage; the collection buffer had 97.3% and 100%. Not all defensive
branches are reached, including incompatible native layouts absent on this host.

Linux x86_64 was executed. Wasm and thumbv7em were compile-checked, not executed.
Windows native-string and handle behavior was not executed in this environment.

## Performance checkpoint

| Workload | Recorded result | Interpretation |
| --- | --- | --- |
| Dynamic slice/str/zeroed/collection | 144/144 time comparisons passed, including std controls | Limited to recorded inputs and environment |
| Dynamic memory | 360/360 checks passed, including std baselines | Requested/usable bytes and accepted handle cost are separated |
| u64, 10,000 / 100,000 owners | Unmetered and metered create/clone/drop passed | A selected workload, not universal parity |
| Four 1MiB string clones | Original point estimate +6.38%, interval −0.80% to +12.24%; exact-binary repeat +2.65% | No fixed 6.4% overhead established |

The engineering time threshold was an upper 95% interval bound of 1.15. The
accepted extra budget pointer is a memory adjustment only. Final datasets are
[dynamic time](performance-30/summary.json), [dynamic memory](performance-30/memory-summary.json),
and [sized time](sized-performance/timing-summary.json). Earlier failed and
inconclusive results remain available in the checkpoint commit history.

Diagnosed issues included per-element collection growth checks, an out-of-line
allocation guard destructor, an out-of-line replacement fast path, and benchmark
lifecycle interference. Assembly and controls can be regenerated locally; the original record is in
Git history. The [1MiB clone case](../box-str-clone-cause/README.md) distinguishes
observed instruction paths from unproven timing causes.

The [benchmark methodology](../../benchmarks.md) defines reproduction prerequisites
and pending conditions. This provisional close does not complete that campaign.
