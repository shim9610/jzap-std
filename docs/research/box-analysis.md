# Box implementation references and code-generation study

This study informed jzap's allocation, cloning, and cleanup contracts. Reference
implementations were inspected and tested as research fixtures, not adopted as
product runtime dependencies. The current implementation status is the
[provisional Box checkpoint](../verification/box-completion/README.md).

## Pinned references

| Reference | Inspected version | Relevant finding |
| --- | --- | --- |
| Rust stable std/alloc | 1.95.0; LLVM 22.1.2 | Standard Box allocation, clone, layout, and unwind cleanup |
| Rust nightly std/alloc | nightly-2026-10-30, rustc 92b5b8c87 | Fallible Box experiments require unstable features; this revision uses allocator_ext |
| Varlit | cd7b0446da3e731fba867802bef5f4b7b3fc328d | Allocates before cloning, but its inspected constructor drops input on refusal |
| fallible_collections | 0.5.2 | Additional fallible collection implementation reference |

The [reference inventory](../../reference/README.md) identifies the retained
sources. Compilation probes checked the actual feature gates on both toolchains.

jzap allocates before invoking a clone and returns rejected constructor input.
This prevents unnecessary clone work when the outer allocation is refused while
preserving ownership. The behavior comparison contains three tests against the
actual reference implementation. An unwind test exposed missing deallocation in
an early jzap destructor: std deallocated once and the old jzap path zero times.
An allocation guard corrected cleanup; both then deallocated once. `catch_unwind`
was a test mechanism, not a conversion of user panics into library errors.

## Assembly observations

The original unmetered implementation retained out-of-line budget machinery.
Making the small policy operations available for ordinary inlining removed that
cost without `inline(always)` or LTO. Counts below describe inspected wrappers,
including error paths, rather than CPU cycles or universal operation costs.

| Wrapper | std instructions / maximum CFA | Varlit | jzap before | jzap after |
| --- | --- | --- | --- | --- |
| New scalar | 14 / 16B | 25 / 32B | 59 / 80B | 25 / 32B |
| Clone scalar | 15 / 16B | 26 / 32B | 59 / 80B | 25 / 32B |
| Drop scalar | 3 / 8B | See raw assembly | 3 / 8B | 3 / 8B |

ABI sizes in that experiment were 8 bytes for std Box, 56 bytes for the Varlit
constructor Result, 32 bytes for jzap's constructor Result, and 24 bytes for its
clone Result. Normalizing errors to Option was a separate diagnostic, not the
public API benchmark. Nightly fallible Box with a zero-sized error normalized to
the same observed shape apart from labels.

For a 4KiB constructor, std and Varlit used a 4096-byte memset; the inspected jzap
path stored the first 15 bytes and used a 4081-byte memset. This release-only
observation did not establish small-stack behavior in debug or for arbitrary T;
subsequent destination-initialization tests cover selected large values explicitly.
Bulk Copy paths used a length check and memcpy, with a semantic difference on
mismatch: std panics and jzap returns Bounds without changing the destination.

The observations above come from the original Box checkpoint's assembly and
execution probes. Use the [maintained Box checks](../verification/box-completion/README.md)
when validating the current implementation.

## Performance scope and reproducibility

Early timing results used per-item barriers and preceded acceptance of the fixed
budget-handle memory cost. They are historical evidence, not the current product
performance verdict. The [barrier study](../verification/box-performance-cause/README.md)
explains the harness correction; the [1MiB string study](../verification/box-str-clone-cause/README.md)
uses the actual benchmark executable to separate instruction-path observations
from unproven timing causes.

The analysis package requires a separately available pinned Varlit checkout.
The root library does not. See [benchmark prerequisites](../benchmarks.md) before
rebuilding reference comparisons. Broader repeated measurements remain pending;
this study does not establish equivalence across allocators, machines, or targets.
