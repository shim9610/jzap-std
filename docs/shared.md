# Fallible Rc, Arc and Weak

`jzap::rc::{Rc, Weak}` provides local shared ownership.
`jzap::sync::{Arc, Weak}` provides atomic shared ownership on targets with
pointer-width atomics. Both use the same allocation, initialization and ownership
implementation. Allocation policies are sealed; the default is `Unmetered`.

```rust
use jzap::{Error, budget::{ConcurrentBudget, ConcurrentMemoryBudget}, sync::Arc};

fn example() -> Result<(), Error> {
    let budget = ConcurrentMemoryBudget::new(4096);
    let first = Arc::try_new_in(42_u64, ConcurrentBudget(&budget))
        .map_err(|rejected| rejected.error)?;
    let second = Arc::try_clone(&first)?;
    let weak = Arc::try_downgrade(&first)?;
    drop((first, second));
    assert!(weak.try_upgrade()?.is_none());
    assert!(budget.used() > 0); // Weak still owns the allocation.
    drop(weak);
    assert_eq!(budget.used(), 0);
    Ok(())
}
```

## Counts and failures

`try_clone`, `try_downgrade` and `Weak::try_clone` return `Error::RefCount`
when another admitted credit would exceed the limit. `try_upgrade` returns `Ok(None)` for a dead payload,
`Ok(Some(owner))` for a successful admission, and an error for a saturated live
counter. The strong/local-weak limit is `isize::MAX`; atomic weak admission
reserves one count below that limit to distinguish retired metadata from the
uniqueness lock. The strong limit is reduced to seven in dedicated
boundary/model tests. There is no ordinary `Clone` implementation that can hide
this failure path.

Native Linux x86-64 strong cloning uses an increment and guard-bit check,
subtracting only the refused credit on failure. Other native targets retain
CAS admission. The [integrated protocol report](verification/arc-headroom-integration.md)
documents the trusted native execution bound, retirement representation and
verification scope. Concurrent count queries remain observational snapshots;
temporary live saturation is reported at the admitted maximum, never as zero.

Construction returns `Rejected<T>` with the original input on storage refusal.
`try_init_in` initializes final storage through `TryInit` and `InitSlot`.
`try_new_cyclic` publishes the payload only after successful initialization;
upgrading its construction weak returns absence. An escaped construction weak
keeps rejected storage alive until its own final release. Uninitialized/zeroed
constructors return `MaybeUninit` payloads; `assume_init` is explicitly unsafe.

`try_unwrap` returns the unchanged owner when another strong owner exists.
`into_inner` consumes its owner and returns an optional payload; if every
concurrent Arc owner calls it, exactly one retrieves the value. Last strong
release drops the payload once. Last weak release frees the allocation and
refunds its budget charge. Cleanup guards also run during a returning unwind.

## Mutation and cloning

`get_mut` requires one strong owner and no explicit weak owners.
`try_make_mut` implements copy-on-write: another strong owner requires a
fallible payload clone; weak-only aliases require a move into fresh storage and
lose their ability to upgrade. Replacement storage is reserved before changing
ownership. Allocation or clone failure preserves the original owner.

`TryClone` and `TryCloneIn` clone a handle, retaining the allocation's original
budget. They do not re-account shared storage against another budget.
`try_clone_payload_with` selects accounting for the new control block; nested
allocations follow the payload’s TryClone policy. `try_deep_clone_in` propagates
a target local budget into supported payload clones. Payload operations use `TryClone`, rather than a blanket bridge
to arbitrary standard `Clone` implementations.

Slices and strings expose `as_slice`/`as_str` and checked access explicitly.
Sized values implement `Deref`. Formatting, comparison, hashing and borrowed
user operations retain the called implementation's execution contract.

## Conversion, coercion and pinning

Explicit fallible conversions cover owned arrays, slices, jzap/std Box and Vec,
String, borrowed values and Cow. Box conversion supports arbitrary dynamically
sized payloads, including trait objects; allocation refusal returns the source.
Slice-to-array conversion checks length, and array-to-slice conversion uses
compiler-checked unsizing. `into_bytes` shares a string allocation as bytes while
existing strong/weak aliases continue to exclude mutable access.
`CStr` is supported with `alloc`; `OsStr` and `Path` require `std`.

`rc_unsize!`, `arc_unsize!`, their weak counterparts, and `pin_rc_unsize!` /
`pin_arc_unsize!` use compiler-checked coercions. Downcast preserves the source
owner when its dynamic type differs. Compiler-only coercion traits are replaced
by these explicit operations; the crate builds on stable Rust.

`try_pin` and `try_pin_init_in` create a `shared::Pinned` owner. For an existing
owner, `into_pin` requires `T: Unpin`; `try_into_pin` accepts a unique allocation
with no ordinary strong or weak aliases. Pinned handle clones preserve pinning.
Unsafe extraction/conversion requires the caller to maintain pinning through
payload destruction.

`into_raw` returns a `NonNull<shared::Control<...>>` retaining header metadata;
`as_ptr` returns the payload pointer. They are different pointer contracts.
`Weak::into_raw` and `Weak::as_ptr` use `Option` to represent an empty weak.
Raw count admission remains fallible. Unsafe reconstruction requires the exact
allocation, metadata, policy and transferred credit described in its rustdoc.

## Budgets and representation

`ConcurrentMemoryBudget` is caller-owned and performs no hidden allocation.
Its atomic `used()` includes reservations and committed bytes. Reserving uses a
compare-exchange loop; committing transfers the token, and cancellation or final
release refunds it once. Separate `used`/`available` calls are observations,
not an atomic snapshot. Borrowed policies retain a budget reference, so the compiler prevents
the budget from being destroyed before a strong or weak owner.

A static concurrent budget supports `Send + Sync + 'static` owners.
`OwnedBudget` and `OwnedConcurrentBudget` keep their ledger alive through Rc
and Arc respectively, so stored policies need no borrowed-budget lifetime.
The local owning policy remains single-threaded; the concurrent owning policy
supports thread transfer when the payload also meets its thread bounds.
Owning-domain bootstrap storage is allocated once per domain, outside the
payload quota. The [budget guide](budget.md) describes both policy choices;
the [architecture review](budget-review.md) records reallocation accounting.

`ConcurrentBudget` also works with Box, Vec and String. A local `Budget` makes an
Arc handle local: atomic counters alone do not make a non-Sync policy Send/Sync.
Atomic owners require both the payload and policy to satisfy their thread bounds;
Rc owners remain local. Budget admission uses a lock-free compare-exchange loop.
Arc downgrade can wait for an internal uniqueness probe to release its weak
lock; the probe calls no user code, and progress requires its holder to resume.
No per-thread completion bound is claimed.

On the measured 64-bit target, sized handles are eight bytes and dynamically
sized handles sixteen bytes, including metered handles. The default header has
two counter words, matching std. Its policy type is Unmetered; the optional
Budget::Unmetered value retains a policy word in its control block.
The caller-owned budget object is shared across
allocations (local 24 bytes, concurrent 16 bytes on this target). A metered header
adds one budget reference,
plus any alignment padding. After payload retirement its strong word stores
the byte charge; surviving weak owners do not require a separate size field.
Allocator rounding is measured separately from requested layouts.

## Performance and implementation status

Rc and its Weak provide the [mapped stable Rust 1.95 capabilities](shared-api.md),
including fallible count admission, cyclic construction, copy-on-write,
conversions and pinning adapters. Rc uses local, non-atomic count updates.
The [current integrated verification](verification/arc-headroom-integration.md#verification-status)
includes the local implementation, owning budgets and whole-library source/link
checks. The [Rc timing tables](verification/shared.md#performance-results) and
[owning-policy renewal](verification/owned-budgets.md#paired-performance-renewal)
record their measurement campaigns and conditions.

Default `Unmetered` ownership and optional budget accounting are separate
performance configurations. Std Rc/Arc have no shared byte-budget reservation
or refund. Their unmetered timings provide the baseline for equivalent ownership
work; budget-enabled timings show the total cost of adding the stated accounting
and ledger-lifetime guarantees. They do not compare two implementations of the
same budget algorithm. Cloning an existing payload handle retains its original
charge and performs no new byte-budget reservation.

The [current Arc report](verification/arc-headroom-integration.md#comparison-scope)
shows these configurations side by side, with absolute times, ratios, uncertainty
and memory costs so callers can select a policy for their workload.

See the [stable API mapping](shared-api.md), [contracts and reference pins](shared-contracts.md)
and [verification/performance report](verification/shared.md). Run
`bash scripts/verify.sh --area shared --profile memory` for local verification. Consumer code can use the
[final-link checking infrastructure](iteration.md) on its own concrete roots.
