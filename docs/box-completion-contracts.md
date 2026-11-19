# Box implementation and measurement contracts

**Milestone status: provisionally closed.** The target is functional coverage of
Rust 1.95 stable Box through explicit fallible operations. Runtime dependencies
remain zero, and core, alloc, and std configurations remain separated.

| Area | Contract |
| --- | --- |
| Dynamically sized ownership | `Box<T: ?Sized>` obtains the actual Layout before destruction; payload destruction precedes storage release and budget refund |
| Slice initialization | Validate length, Layout, and budget before allocation; record each write; clean only the current written value and completed prefix on failure |
| Cloning | Preserve the source; `TryCloneIn` charges newly owned storage to the target and uses the output element's Layout; Copy paths use bulk copying |
| Strings | Preserve UTF-8; byte ownership conversion does not reallocate; validation rejection returns the input |
| Ownership transfer | Explicit value, raw-pointer/policy, and compatible type conversions; unsafe recovery requires valid storage, metadata, ownership, and accounting |
| Borrowing | Sized Deref is supported; slice/str indexing requires an explicit standard borrow or checked access |
| Uninitialized storage | Single-value and slice MaybeUninit constructors, including zeroed storage; safe write or explicit unsafe completion |
| Pin and type erasure | Explicit adapters preserve address, lifetime, and allocation policy |
| Standard interoperability | Owner transfers are explicit boundaries; no implicit allocating Clone/From/collection trait hides failure |

The source policy is retained by `TryClone`. Constructor `_in` policies select the
outer storage; existing inner owners are not silently moved to another budget.
Use `TryCloneIn` for deep target-budget cloning. A denied allocation runs no
initializer; returned-error cleanup releases all internally completed values.
Iterator consumption and callback side effects are not rolled back.

Sized `try_clone_from` bulk-copies supported primitive arrays and reuses storage
for other values up to 256 bytes through their failure-preserving element method.
Larger user values use destination initialization in a staged allocation. This
bounds library-selected by-value staging, not arbitrary stack use inside a user
implementation. General slice replacement may also need temporary storage.

## Recorded acceptance protocol

The engineering threshold used for these runs was a bootstrap 95% interval upper
bound of 1.15 relative to std. It is not a universal performance guarantee.
Measurements include unmetered and metered policies and an identical std control.

- Sized workload: 10,000 and 100,000 u64 Boxes, 15 independent processes per label.
- Dynamic workload: 32, 4,096, and 1,048,576 bytes; Copy slices, str, zeroed slices,
  and exact-length byte iterator collection. Keep `max(4, 1,048,576 / length)`
  source owners and the same number of clones.
- Each process runs two warm-up cycles and retains 30 measured cycles. Processes
  use one CPU and alternate implementation order. Compilation and verification
  do not run concurrently with timing.
- Creation/clone/drop use one process mode; replacement uses a separate process
  mode. Adding std string replacement to the first mode changed later clone
  timings because the allocation lifecycle changed. Earlier results are retained.
- The first dynamic 15-process run left three intervals inconclusive. A further
  15 processes were added with the same binary and all 30 were pooled; no earlier
  samples were discarded. The threshold remained 1.15.
- Memory measurements run separately. Requested bytes, glibc usable bytes,
  retained bytes, peak, total requested bytes, and final tracked balance are
  distinguished. The budget pointer's extra handle storage is reported separately.
- On the measured 64-bit target, a budget adds 8 bytes per handle. That fixed
  storage cost is accepted by design; it does not waive the speed threshold or
  allow unrelated allocations or leaks.

Copy replacement of equal-length primitive slices and str requires no additional
allocation. No-allocation owner conversions are checked for pointer preservation
and zero allocator calls.

Three-process diagnostics are cause investigations, not acceptance evidence.
Results and protocol changes are linked from the [benchmark methodology](benchmarks.md).
Broader conditions and repeated independent sessions remain outstanding, so this
milestone does not close performance validation.
