# Box guide

Collection's fallible input APIs use open `TryIntoIterator`/`TryIterator`
contracts; see [iteration guidance and consumer checks](iteration.md).

**Status: provisionally closed implementation milestone.** This guide describes
Rust 1.95 stable functionality provided through explicit fallible APIs. It does
not promise identical compiler-special syntax or universal performance parity.
See the [API matrix](verification/box-completion/README.md) and
[benchmark methodology](benchmarks.md) for the measured scope.

## Create a value and retain rejected input

`Box::try_new(value)` returns `Result<Box<T>, Rejected<T>>`. Allocation failure
returns the original input; constructing the error does not allocate.

```rust
use jzap::boxed::Box;

let result = Box::try_new(42_u64);
match result {
    Ok(value) => assert_eq!(*value, 42),
    Err(rejected) => assert_eq!(rejected.value, 42),
}
```

`try_new_in(value, policy)` selects the outer allocation's policy. Moving an
already-owned value into a Box does not reassign budgets of allocations inside it.
`Box<T>` uses `Unmetered`; `Box<T, Budget<'a>>` retains a reference to a caller-owned
budget. Its lifetime prevents the Box from outliving that budget. The current
`Cell`-based budget is not shared across threads.

## Clone with an explicit failure contract

`TryClone::try_clone` retains the source policy. `TryCloneIn::try_clone_in`
propagates the target budget through newly owned storage and can change the output
type's lifetime or element layout. There is no blanket delegation to arbitrary
standard `Clone` implementations.

```rust
use jzap::{boxed::Box, budget::{Budget, MemoryBudget}, TryCloneIn};

fn clone_into_budget() -> Result<(), jzap::Error> {
    let source = Box::try_new(7_u64).map_err(|e| e.error)?;
    let target = MemoryBudget::new(8);
    let copy = source.try_clone_in(Budget::Limited(&target))?;
    assert_eq!(target.occupied(), 8);
    drop(copy);
    assert_eq!(target.occupied(), 0);
    Ok(())
}
```

`try_clone_from` preserves the destination on a returned error. Known primitive
and reference slices, nested primitive arrays, and equal-length strings can reuse
storage. Other sized values up to 256 bytes delegate to the element's
failure-preserving replacement. Larger user values and general slices use a
staged owner and may need additional storage. User implementations must honor the
trait's preservation contract.

For values whose `TryClone` provides a copy witness (primitives, references and
arrays of them), `try_clone` allocates and copies the value directly; other
values use the initializer slot, whose completion check is retained. The direct
path removed 8 instructions per `u64` clone/drop pair: before it, LLVM merged
the slot's error result with the allocation result and re-tested a constant
`Result` tag after a successful allocation. With loops aligned to 64 bytes
(`.cargo/loop-align64.toml`) the unbudgeted pair went from 30.42 to 28.33
cycles against std's 28.06–28.47, and the loop is instruction-for-instruction
the same as std's. The default build's one-second comparison moved the other
way (+7.5% to +14.6% vs std) because the identical loop sits at another address;
the budgeted pair improved from +12.5% to +8.6%. Measured 2026-11-09 on the
local i7-13700KF, CPU 6 at 3.4 GHz, Rust 1.95.0. The direct path is exercised
by `tests/foundation.rs` (`Box<u64>` clone, nested Box array clone); the
foundation area passed Miri on `a49a8d8` in the cloud run reported 2026-11-10:
x86-64 strict provenance, i686 and Tree Borrows, 52 tests each, no UB report.

## Initialize large values in their destination

Passing `T` by value to `try_new` can require a caller-side stack temporary.
`try_init_in`, `try_init_array_in`, `try_init_slice_in`, and `try_init_struct!`
provide destination initialization instead.

```rust
use jzap::boxed::Box;

let values = Box::<[u64]>::try_init_slice(4, |index, slot| {
    slot.write(index as u64).map_err(|e| e.error)
});
assert!(values.is_ok());
```

The slot records whether a value was actually written. Returning `Ok(())` without
a write is rejected. On error, the current written element and completed prefix
are dropped exactly once. Named-field initialization checks field completeness at
compile time and field alignment before creating references. `MaybeUninit`
constructors and unsafe `assume_init` remain explicit lower-level alternatives;
unsafe callers are responsible for validity and ownership.

## Slices, strings, and collection

Slice and string Boxes expose checked `get` / `get_mut` and explicit borrowing
through `as_slice`, `as_str`, `AsRef`, and `AsMut`. They do not implicitly expose
indexing through `Deref`. Once callers obtain a raw standard borrow, ordinary
standard-library operations retain their standard contracts.

Copy slice construction performs a bulk copy after layout and allocation checks.
General slice cloning invokes `TryClone`; its constructor policy covers the outer
allocation, while `TryCloneIn` is the explicit deep target-budget operation.
UTF-8 conversion validates bytes and returns the original owner on rejection.

Fallible iterator, character, and string-fragment collection checks growth and
shrink requests. Failure destroys the internal partial result. Already consumed
input and external callback side effects are not rolled back. Size hints are
allocation hints, never trusted as memory-safety bounds.

## Ownership, dynamic types, and calls

| Operation | Explicit interface |
| --- | --- |
| Move out a sized value | `Box::into_inner` |
| Array to slice | `into_slice` or `box_unsize!` |
| Concrete type to trait object | `box_unsize!(owner => dyn Trait)` |
| Pinned dynamic conversion | `pin_box_unsize!` |
| Borrow a callable | `owner.as_ref()(...)` / `owner.as_mut()(...)` |
| Consume a callable, including `dyn FnOnce` | `box_call_once!(owner; arguments...)` |
| Pin ownership and poll a future | `Box::into_pin`, `try_pin`, `Pinned` |
| Checked type recovery | `downcast` for supported Any/Error objects |
| Transfer a standard global Box | `from_std`, `try_from_std_in`, `into_std` |
| Raw ownership with accounting | `into_raw_parts` / unsafe `from_raw_parts` |

Unsizing uses compiler-checked pointer coercion without reallocating. Raw recovery
requires unique initialized storage with the correct global allocation layout and
existing budget charge. `leak` deliberately retains both storage and its charge.

Conversions from Vec, String, CString, OS strings, and paths have dedicated APIs.
OS string and path conversions require the `std` feature.
A failed fallible Vec shrink returns the original owner, including its pointer,
length, and capacity. CStr/OS/path conversions validate the necessary borrowed
layout and metadata rather than assuming a private fat-pointer representation.
After an explicit transfer to a standard owner, subsequent standard operations
follow that owner's failure behavior.

## Failure and execution boundaries

The [contracts](box-completion-contracts.md) define library-controlled failure and
cleanup. Rust's `unsafe` obligations, user callbacks, custom destructors, and
allocator behavior remain explicit boundaries. Unwind cleanup tests do not mean
user panics are converted into `Err`, and successful finite tests do not establish
termination for arbitrary external code.
