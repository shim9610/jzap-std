# Historical checkpoint: budget accounting and array initialization

This directory preserves an earlier optimization and verification stage. Dynamic
Box support was subsequently implemented; current status is in the
[Box checkpoint](../box-completion/README.md).

## Accounting change

The budget caches available bytes and derives reserved bytes as
`limit - available - occupied`. Committing a reservation updates occupied bytes
without a second availability store. Reservation refusal and rollback preserve
the invariant `occupied + reserved <= limit`; deallocation completes before the
charge is returned. The earlier and intermediate records are available in Git history.

## Destination initialization

Debug and release tests initialized 256KiB arrays on a 64KiB stack. In the inspected
release code for a 4KiB payload, initialization used `memset`, cloning used
`memcpy`, and the maximum recorded call-frame offset was 32 bytes. This is evidence
for those instantiated paths, not a stack-size guarantee for arbitrary user
initializers or by-value inputs.

## Memory and timing interpretation

For 100,000 scalar owners plus clones, requested peak bytes were 3,200,000 for
std/unmetered and 4,800,000 for metered Box. The 1,600,000-byte difference is
200,000 handles × 8 bytes. Recorded glibc usable peaks were 6,405,600 and 8,003,040
bytes, respectively; both made 200,002 allocations and ended with zero live
requested bytes. Requested bytes and allocator usable bytes are different metrics.

Timing from this stage included a per-element result barrier. At 10,000 values,
creation ratios included 1.177 for a 16-byte std wrapper and 1.189 for metered Box.
The later [barrier diagnosis](../box-performance-cause/README.md) isolated that
measurement artifact. These historical ratios must not replace the current
phase-barrier dataset or be treated as a fixed budget-bookkeeping penalty.

Raw logs and source copies were removed from this directory; the original
checkpoint is retained in Git history.
See [benchmark methodology](../../benchmarks.md) for the current protocol and
pending repeated measurements under additional conditions.
