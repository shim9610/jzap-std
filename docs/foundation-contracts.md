# Clone, initialization, and budget contracts

These foundations support the provisional Box milestone. Collections such as
BTreeMap build on them; no external B-tree implementation is a runtime dependency.

## Clone semantics

`TryClone::try_clone(&self) -> Result<Self, Error>` retains the source policy.
`TryCloneIn<'target>` has an associated `Output` and propagates the target budget
to newly owned allocations. A budgeted Box can therefore produce an output with a
different budget lifetime or element layout. Option, Result, tuples, and arrays
propagate the corresponding operation.

There is no blanket implementation that calls arbitrary standard `Clone`.
Scalars and shared references copy their value or reference. Shared Arc/Rc
ownership is not implemented by delegating to standard clone.

Array construction tracks the initialized prefix and drops only completed values
on failure. Explicit Copy slice copying validates equal lengths and performs a
bulk copy. A Box allocates its outer storage before invoking payload cloning, so
outer allocation refusal runs no clone callback. Returned clone errors preserve
the source and clean internally completed results.

## Recorded initialization

`TryInit<T>` receives an exclusive `InitSlot<T>`. The slot records successful
writes: duplicate writes are rejected, and a callback returning success without a
write produces `Uninitialized`. A callback error after a write drops that value.
Unsafe raw initialization interfaces are available with explicit validity,
alignment, lifetime, and ownership obligations; the safe protocol does not trust
a callback's success assertion alone.

`InitSlot<[T; N]>::try_init_array` lends destination element slots sequentially.
Only a finished element joins the completed prefix. Failure leaves the parent
uninitialized and eligible for retry. An already initialized parent is rejected
without invoking the initializer. Dynamic slices and named-field initialization
follow the corresponding recorded-write contract; see the [Box guide](box.md).

Array implementations of destination clone/default operations recurse into
slots rather than constructing a whole-array temporary. The default implementation
for a user type may still call its by-value implementation. Its stack usage and
external side effects remain part of that implementation's contract.

## Budget transitions and cleanup

The sealed allocation policy selects either no accounting or a caller-owned
`MemoryBudget`. Its invariant is:

```text
occupied + reserved <= limit
reserved = limit - available - occupied
```

Reservation decreases available bytes. Commitment increases occupied bytes;
cancellation restores available bytes. Deallocation releases occupied bytes only
after storage is freed. Reallocation reserves the entire new request while the
old charge remains live, preserving the old allocation on refusal.

The budget is single-threaded and uses Cell. A sized unmetered Box holds one
pointer; a metered Box additionally holds its budget reference. The measured
64-bit difference is 8 bytes. Both the handle and arrays storing handles are
included in memory accounting.

## Measurement history and current contract

Initial foundation and Copy diagnostics used nine independent processes. Later
Box comparisons used 15 processes with two warm-up and 30 retained cycles, and
the final dynamic comparison pooled 30 processes. These protocols are not
interchangeable samples.

The project used a 1.15 upper confidence-bound threshold for time. The metered
handle's extra pointer is an explicitly accepted memory cost, not a speed
exception. Early per-item output barriers forced wider handles through stack
temporaries; later phase-level barriers preserve inputs and allocator operations
without that extra observer cost. Both datasets remain available.

The current [Box contract](box-completion-contracts.md) and
[benchmark methodology](benchmarks.md) take precedence over historical reports.
Performance is still provisional pending more conditions and repeated sessions.

## Provenance

The pinned Varlit source documents fallible cloning and allocation-before-clone
ordering used in this comparison.
Target-budget propagation, recorded slots, and cleanup changes are verified as
jzap code. Snapshots, exclusions, and upstream notices are in
[reference/](../reference/README.md).
