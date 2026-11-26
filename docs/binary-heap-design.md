# Fallible BinaryHeap

The Vec-backed implementation provides explicit failure paths for the pinned
stable API. Same-ledger metered append reuses the larger heap allocation while
preserving policies and admission. Runtime, allocation and fresh combined
source/link checks pass. Timing remains conditional on workload and build settings.
There is no new runtime dependency.

## References

The primary reference is Rust 1.95.0 (`59807616e`),
`library/alloc/src/collections/binary_heap/mod.rs`, compared with
nightly-2026-10-30 (`92b5b8c87`).
Both expose the same 23 distinct stable inherent names, including PeekMut::pop.
Allocator APIs, pop_if, sorted drain/iteration and PeekMut::refresh remain unstable;
nightly also adds an unstable mutable-slice view. They are outside stable support.

The maintained [heapless reference](https://github.com/rust-embedded/heapless/blob/47dfb021aaa14041a57a79058fc3864ce52a83f1/src/binary_heap.rs)
was also reviewed: its fixed-capacity push returns the rejected item and its hole
guard restores ownership during sifting. It is not archived, and its pinned
commit is dated 2026-11-03. This is an algorithm reference, not a dependency or
an allocator/budget implementation to adopt. Rust and heapless reference licenses
remain upstream; no source snapshot is tracked here.

## Use

```rust
use jzap::{Rejected, collections::BinaryHeap};

fn sort_values(values: [u64; 4]) -> Result<jzap::vec::Vec<u64>, Rejected<[u64; 4]>> {
    let heap = BinaryHeap::try_from_array(values)?;
    Ok(heap.into_sorted_vec())
}
```

`try_push` likewise returns `Rejected<T>` with the incoming value on refusal.
`peek` and `pop` return Option without allocation. Use `core::cmp::Reverse<T>`
for a min heap; its fallible clone delegates to the wrapped value. A Vec carrying
an element limit can be moved into `from_vec`; the heap keeps that limit.
Construct with `new_in` or `try_with_capacity_in` for borrowed/owning local or
concurrent allocation policies. A move back through `into_vec` keeps the policy.
Clone-from retains the destination's domain and limit, and can reuse equal-length
Copy storage; `try_clone_in` selects a target budget and heapifies the output type.

## Storage and contracts

`BinaryHeap<T, P, L>` owns the existing `Vec<T, P, L>`; normal construction uses
the existing unmetered/unlimited defaults. Moving an existing vector preserves
its allocation policy and element limit. Sorted/unsorted vector recovery transfers
the same allocation without allocation or policy cloning. Mutable item access is
restricted to PeekMut. Raw slice views retain the existing interoperability boundary.
The [budget guide](budget.md) covers policy ownership and reallocation admission.

| Operation | Implemented behavior |
| --- | --- |
| Construction | Empty construction does not allocate; capacity and array construction return storage errors and retain rejected array ownership |
| Push | Reserve through Vec before changing the heap; allocation/budget/limit refusal returns the incoming item and preserves the heap |
| Pop | Remove the maximum without allocation; empty heaps return None |
| Append | Reserve before transfer; refusal preserves both heaps. Unmetered heaps keep the larger-owner shortcut. Metered reuse requires the identical ledger and equal limits, grows exactly before exchange when necessary, and preserves both policy handles |
| Retain | Fallible predicate; earlier removals remain, the current/unvisited values survive refusal, and a guard restores heap order before return |
| Extend | Open TryIntoIterator source; source/storage errors retain the successful prefix as a valid heap and return a rejected item on push refusal. Hints are not bounds or termination proofs |
| Clone | TryClone/TryCloneIn; private partial construction is cleaned on refusal. Clone-from is transactional. Target-type clones are heapified |
| Mutable peek | Defer repair until mutable access; restore on drop. Match std leak amplification if the guard is forgotten, and provide infallible PeekMut::pop for its proven nonempty heap |
| Drain and iteration | Unsorted borrowed/consuming/draining exact, double-ended, fused iterators and fallible mirrors; dropping a drain destroys its remaining prefix and keeps capacity |

The 23 distinct stable inherent names map as follows. `pop` occurs on both the
heap and PeekMut; both counterparts are implemented.

| Standard names | Explicit counterparts |
| --- | --- |
| new, as_slice, capacity, clear, drain, into_sorted_vec, into_vec, is_empty, iter, len, peek, peek_mut, pop | Same names and functional behavior |
| append, push, retain, shrink_to, shrink_to_fit, with_capacity | try_append, try_push, try_retain, try_shrink_to, try_shrink_to_fit, try_with_capacity |
| reserve / try_reserve, reserve_exact / try_reserve_exact | try_reserve, try_reserve_exact; storage errors always return |

Default/TryDefault, Debug and Vec ownership conversions are implemented.
TryClone/TryCloneIn replace allocating Clone; fallible array/iterator construction
and `try_extend`/`try_extend_copy` provide the From/FromIterator/Extend functions.
Borrowed, consuming and draining iteration provide the standard traversal traits
and the public fallible mirrors. Stable Rust allocator support remains the existing
allocation-policy API; compiler-only unstable traits are not claimed.

Ord and destructor callbacks must return normally; Ord must obey its ordering
contract. A hole guard also restores initialized ownership if a comparison unwinds;
heap order after arbitrary comparator failure is not used as a memory-safety premise.
Successful same-type element cloning must preserve ordering, as with std Clone;
target-type cloning is explicitly heapified.

## Invariants and validation scope

The bounded sift frontier proves `0 <= start <= pos < end <= len`. Nonzero-sized
Rust slices have at most isize::MAX elements, so their `2*pos+1` child candidate
cannot overflow even when it is a leaf sentinel. Zero-sized heaps use the guarded
`pos < end / 2` calculation, or the out-of-range `end` sentinel. Two-child access
requires `child < end-1`; equality permits only the single left child. A candidate
is never converted to a pointer before this check. Parent steps require pos > start
and strictly decrease. Downward positions strictly increase below end. The direct
product geometry contracts cover both candidate calculations and bool-based child
selection over machine words; the valid nonzero slice layout fact remains a
reviewed caller precondition.

A hole owns one removed value and one missing logical slot. Each move copies
one initialized, distinct slot into that hole and transfers the hole position;
drop fills it exactly once. Six actual Rust Kani harnesses check geometry,
append lengths and growth capacity bounds over machine words. The ownership
kernels exercise sifting, tail repair, sorting, append success and storage refusal
on arbitrary arrays of up to four u8 values. The admission kernels check ledger
balance, policy-handle identity, both-owner preservation on refusal and cleanup.
Miri/AddressSanitizer, differential properties and drop/failure injection exercise
real storage/provenance. The global check combines these with reviewed callers
and the existing Vec contracts.

The refusal kernel checks the actual Vec storage-reuse unit with arbitrary
initialized payloads. It checks exact contents, pointers, capacities, policy handles,
the Budget error payload and cleanup. The Heap caller's immediate `?` propagation
is a reviewed composition boundary exercised by native/Miri tests and a compiled
error-propagation defect. This avoids expanding heap repair again in a refusal
query; the successful whole-Heap append and existing repair kernels remain separate.

The internal length restoration request is always within the unchanged Vec
capacity. Its existing fallible setter is used under that explicit precondition;
the actual-code kernel checks acceptance in the owner protocol.
No unwrap, assertion, unreachable hint or panic fallback is permitted in product
restoration/drop code.

Hole movement derives both addresses from one mutable raw pointer. The Miri
control reintroduces shared-then-mutable pointer derivation and is rejected under
Stacked Borrows. Positive tests exercise repeated sifts, comparisons that unwind,
drain and mutable-peek lifetime transitions under both borrow models.

## Measurement protocol

The time comparisons use random corpora, at least one second per accepted
interval, three independent seeds, operation-weighted means and identical-std
condition means within 0.9–1.1, comparing genuine std BinaryHeap directly.
The unmetered +15% observation criterion remains unchanged; optional budgets
are reported separately. Workloads cover push, worst-direction insertion, repeated pop,
heap construction, sorting, append with both size relations, retain, mutable
peek, clone and unordered iteration, with small and large values where moves
matter. Requested and allocator-usable held/peak bytes are separate criteria;
handle size and observer scope are stated alongside them. Compilation/proofs and
timing do not overlap. Code generation and controlled repeats accompany the gaps.

Only maintained tests/tools, compact results and reproduction commands enter
Git. Raw logs, generated inputs, binaries and assembly remain in ignored target
storage.

## Verification checkpoint

Runtime product `7688418` retains the existing allocation and length policies.
Proof/checker `6389ac4` separates successful Heap append from storage-admission
refusal. Rebuilding default and aligned time executables after these proof-only
changes produced byte-identical files. Native tests and documentation examples
total 338 passing cases. Fresh global source/link composition passes in all four
profiles without model reuse or source changes during verification.

| Evidence | Checked scope | Result |
| --- | --- | --- |
| Stable API inventory | Rust 1.95's 23 distinct inherent names and explicit trait counterparts | Pass |
| Runtime and properties | 18 heap tests, including 256 fixed-seed generated operation sequences; allocation refusal, both-owner rollback, limits/domains, policy-handle identity, callback/source refusal, partial cloning and exact drop ownership | Pass |
| Actual Rust Kani | Geometry, append lengths and growth bounds over machine words; arbitrary small-array sifts, permutation/order, allocated growth, pop, mutable peek, drain, repair, sorting, successful Heap append and actual Vec storage-refusal state | Pass: six kernels; reviewed caller composition as described above |
| Direct Verus | Unchanged product parent/child/selection bodies, with arithmetic preconditions | Pass: four heap functions and four solver-rejected source defects |
| Defect controls | Reversed sift, lost hidden tail, double-owned drain, pointer provenance, ledger identity, policy-handle exchange, excess growth, ignored original spare capacity and swallowed admission errors | Pass: eight compiled runtime regressions and one Miri control detected |
| Executed memory | Strict-provenance native/i686 Miri, native Tree Borrows and AddressSanitizer; large aligned non-Copy elements, owning ledgers and unwind/drop paths | Pass: 17 heap scenarios in each Miri configuration |
| Fuzz and coverage | 10,000 heap fuzz runs; LLVM heap-module source coverage | 364/374 lines (97.33%), 63/72 branches (87.50%) |
| Builds and tooling | Core/alloc, wasm/thumb, no-global-OOM, strict Clippy/rustdoc, audits, licenses, runner tests and software bill of materials | Pass; 19 runner tests and seven reporting tests |
| Final source/link composition | Regenerated common prerequisites; release, release with link-time optimization (LTO), overflow-checked release and debug consumer roots, including all budget policies and eight exit controls | Pass: no model reuse, unchanged source, zero unresolved source conditions as counted before the premise rule and zero unproved linked edges; all eight exit controls detected. That count included rows with stated premises; all 50 current heap rows have [stated premises not proved](direct-source-migration.md#certificate-dispositions-and-premise-status-2026-11-26) |

The checked Linux x86-64 consumer fixtures retain the core guard calls below.
Verified source/caller conditions make the originating guards unreachable; the
combined check closes forwarding calls after accounting for all their origins.

| Profile | Originating guard calls | Forwarding calls | Unproved edges |
| --- | ---: | ---: | ---: |
| Release | 3 | 0 | 0 |
| Release with LTO | 6 | 0 | 0 |
| Release with overflow checks | 3 | 0 | 0 |
| Debug | 58 | 5 | 0 |

Kani checks the real product calls, with the input and unwind bounds stated in
the maintained harnesses; it is not an unbounded Rust ownership theorem.
Direct Verus includes the original geometry source without extracting
or substituting bodies. The combined check also retains older reviewed projections
for other domains. Their translation boundary, compiler/core, valid allocator,
public-unsafe and returning-callback contracts remain explicit. Source seals
trigger renewed review when callers change; they do not establish semantics.

## Timing results

Product `7688418` was measured with Rust 1.95 / LLVM 22.1.2 on one pinned CPU
of an AMD EPYC 9V74 Linux x86-64 guest. The matrix has 60 operation/item-width/corpus
conditions, default and opt-in 32-byte loop alignment, three seeds (617, 6203,
96011), std, an identical std control and three allocation policies. Every selected
interval lasts at least one second; calibration, preparation and two warmup epochs
are excluded. Compilation and proofs did not overlap timing.

The complete run produced 1,800 valid intervals. Two condition pairs were repeated
in both builds for all three seeds because an identical-std condition mean was
outside 0.9–1.1: mixed drain_256 and small push_ascending_256. These repeats add
60 intervals. The selected 1,800 intervals use the complete repeats in place of
those initial pairs, with identical-std means from 0.9076 to 1.0833.
[Current matched groups](verification/binary-heap-timing.csv) contain the selected
360 variant/seed groups. The [initial control screen](verification/binary-heap-control-screen.csv)
retains the 12 replaced groups, including all five implementations, so the selection
and initial drift remain independently inspectable. Performance values did not
select repeats.

Times below are **nanoseconds per complete heap workload for one corpus case**:
sum of valid elapsed seconds divided by sum of workload calls. Each epoch visits
the same 64 cases in the same order for every implementation. Mixed cases include
empty, one-element, 4,096-element and randomized intermediate inputs; small cases
use at most 32 elements. Seed variation includes corpus variation; these are
operation-weighted observed means, not confidence-interval acceptance or
application call-frequency weights.

Destructive workloads include source cloning, checksum and cleanup. from_vec
includes input copying; sort clones and sorts in place; peek_mut changes the root
once per input element. iter scans prepared heaps. clone and into_iter both clone
and consume a heap in separate call contexts. extend deliberately receives an
unknown-length source on both sides. Optional local/concurrent ledgers are reported
separately; their single-threaded measurements do not describe shared-ledger contention.

### Mixed inputs, 8-byte items

| Operation | Build | std ns | Unmetered ns | Change | Local budget ns | Concurrent budget ns |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| push | default | 2,158.46 | 1,926.65 | -10.7% | 2,541.31 | 2,502.79 |
| push | align32 | 2,574.48 | 2,308.84 | -10.3% | 2,518.86 | 2,308.83 |
| push_reserved | default | 1,939.70 | 2,456.02 | +26.6% | 2,449.28 | 2,357.24 |
| push_reserved | align32 | 1,918.97 | 1,968.86 | +2.6% | 2,434.54 | 2,530.32 |
| push_ascending | default | 2,493.06 | 3,154.96 | +26.5% | 2,594.92 | 3,141.24 |
| push_ascending | align32 | 2,450.45 | 2,507.37 | +2.3% | 2,493.21 | 2,442.77 |
| pop_all | default | 12,909.61 | 13,931.16 | +7.9% | 13,994.34 | 14,026.64 |
| pop_all | align32 | 12,871.66 | 13,691.01 | +6.4% | 14,297.88 | 13,655.27 |
| from_vec | default | 613.68 | 646.79 | +5.4% | 589.43 | 644.65 |
| from_vec | align32 | 626.00 | 577.68 | -7.7% | 591.06 | 585.70 |
| sort | default | 12,751.25 | 13,798.55 | +8.2% | 13,309.28 | 13,822.51 |
| sort | align32 | 13,059.34 | 12,944.77 | -0.9% | 13,542.33 | 13,613.74 |
| append_small | default | 166.86 | 205.59 | +23.2% | 179.89 | 175.07 |
| append_small | align32 | 170.93 | 172.17 | +0.7% | 178.17 | 173.22 |
| append_large | default | 175.33 | 175.97 | +0.4% | 185.91 | 179.49 |
| append_large | align32 | 168.72 | 174.97 | +3.7% | 184.95 | 181.91 |
| retain | default | 654.84 | 584.60 | -10.7% | 575.69 | 524.68 |
| retain | align32 | 544.70 | 571.96 | +5.0% | 573.98 | 620.83 |
| peek_mut | default | 4,473.03 | 4,731.85 | +5.8% | 4,761.29 | 5,363.28 |
| peek_mut | align32 | 4,706.15 | 4,860.71 | +3.3% | 4,968.20 | 4,868.24 |
| clone | default | 131.42 | 125.72 | -4.3% | 130.26 | 162.26 |
| clone | align32 | 120.54 | 128.56 | +6.7% | 128.27 | 124.35 |
| iter | default | 42.17 | 79.45 | +88.4% | 43.34 | 77.21 |
| iter | align32 | 45.67 | 42.34 | -7.3% | 44.68 | 44.83 |
| into_iter | default | 123.32 | 156.77 | +27.1% | 129.08 | 127.54 |
| into_iter | align32 | 126.00 | 131.62 | +4.5% | 130.73 | 126.91 |
| drain | default | 155.96 | 155.68 | -0.2% | 154.65 | 159.97 |
| drain | align32 | 120.27 | 124.64 | +3.6% | 123.24 | 121.19 |
| extend | default | 935.20 | 890.89 | -4.7% | 897.01 | 879.90 |
| extend | align32 | 989.85 | 895.71 | -9.5% | 888.22 | 881.26 |

### Mixed inputs, 256-byte items

| Operation | Build | std ns | Unmetered ns | Change | Local budget ns | Concurrent budget ns |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| push | default | 17,771.07 | 17,786.47 | +0.1% | 18,392.84 | 17,983.27 |
| push | align32 | 17,670.58 | 18,171.28 | +2.8% | 18,129.74 | 17,669.52 |
| push_reserved | default | 17,788.64 | 17,332.23 | -2.6% | 16,517.19 | 19,518.05 |
| push_reserved | align32 | 18,894.98 | 18,052.68 | -4.5% | 16,997.93 | 17,065.56 |
| push_ascending | default | 30,015.42 | 29,872.54 | -0.5% | 26,082.92 | 25,702.14 |
| push_ascending | align32 | 33,095.54 | 32,347.60 | -2.3% | 30,048.22 | 26,424.87 |
| pop_all | default | 54,817.92 | 52,185.97 | -4.8% | 52,234.80 | 53,036.85 |
| pop_all | align32 | 55,441.22 | 55,127.05 | -0.6% | 52,254.34 | 53,823.29 |
| from_vec | default | 12,747.73 | 12,542.78 | -1.6% | 12,955.69 | 13,147.22 |
| from_vec | align32 | 12,582.23 | 12,820.46 | +1.9% | 12,856.63 | 12,201.01 |
| sort | default | 57,888.85 | 57,055.96 | -1.4% | 54,711.87 | 56,654.07 |
| sort | align32 | 56,573.12 | 52,835.24 | -6.6% | 56,110.19 | 53,297.89 |
| append_small | default | 7,019.24 | 6,970.26 | -0.7% | 6,905.58 | 6,785.69 |
| append_small | align32 | 8,270.27 | 7,078.97 | -14.4% | 7,385.19 | 7,424.32 |
| append_large | default | 4,013.15 | 4,291.61 | +6.9% | 4,070.48 | 3,962.68 |
| append_large | align32 | 4,054.44 | 3,917.77 | -3.4% | 3,941.23 | 4,220.33 |
| retain | default | 6,479.22 | 6,986.38 | +7.8% | 6,701.51 | 6,567.99 |
| retain | align32 | 6,384.38 | 6,355.09 | -0.5% | 6,465.21 | 6,374.96 |
| peek_mut | default | 37,509.29 | 38,453.63 | +2.5% | 43,052.91 | 37,668.85 |
| peek_mut | align32 | 35,601.51 | 40,536.94 | +13.9% | 43,880.43 | 37,641.02 |
| clone | default | 3,689.45 | 3,756.66 | +1.8% | 3,701.78 | 3,726.98 |
| clone | align32 | 3,622.40 | 3,732.50 | +3.0% | 3,614.79 | 3,610.34 |
| iter | default | 452.08 | 442.80 | -2.1% | 441.30 | 436.12 |
| iter | align32 | 456.29 | 446.06 | -2.2% | 462.45 | 458.00 |
| into_iter | default | 3,725.47 | 3,682.90 | -1.1% | 3,746.13 | 3,656.10 |
| into_iter | align32 | 3,920.11 | 3,693.74 | -5.8% | 3,682.67 | 3,717.41 |
| drain | default | 3,676.47 | 3,668.37 | -0.2% | 3,705.41 | 3,704.87 |
| drain | align32 | 3,684.15 | 3,696.20 | +0.3% | 3,715.42 | 3,727.83 |
| extend | default | 12,944.69 | 12,358.00 | -4.5% | 12,994.45 | 12,781.37 |
| extend | align32 | 12,642.39 | 12,638.87 | -0.0% | 12,900.58 | 12,887.44 |

### Small inputs, 8-byte items

| Operation | Build | std ns | Unmetered ns | Change | Local budget ns | Concurrent budget ns |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| push | default | 43.93 | 44.70 | +1.8% | 46.32 | 53.45 |
| push | align32 | 41.25 | 42.89 | +4.0% | 46.13 | 52.05 |
| push_reserved | default | 18.97 | 19.62 | +3.4% | 20.53 | 21.59 |
| push_reserved | align32 | 19.41 | 20.27 | +4.4% | 22.01 | 22.32 |
| push_ascending | default | 46.01 | 48.13 | +4.6% | 48.08 | 54.95 |
| push_ascending | align32 | 47.99 | 46.65 | -2.8% | 50.71 | 56.10 |
| pop_all | default | 62.22 | 65.75 | +5.7% | 66.55 | 67.52 |
| pop_all | align32 | 63.61 | 65.43 | +2.9% | 67.46 | 66.68 |
| from_vec | default | 24.70 | 23.51 | -4.8% | 24.89 | 26.68 |
| from_vec | align32 | 26.52 | 24.91 | -6.1% | 25.55 | 26.38 |
| sort | default | 61.09 | 62.58 | +2.4% | 63.87 | 63.66 |
| sort | align32 | 61.34 | 60.97 | -0.6% | 62.78 | 64.50 |
| append_small | default | 57.39 | 62.59 | +9.1% | 69.56 | 59.22 |
| append_small | align32 | 58.46 | 60.38 | +3.3% | 65.60 | 60.44 |
| append_large | default | 58.48 | 58.54 | +0.1% | 65.93 | 65.41 |
| append_large | align32 | 58.90 | 60.33 | +2.4% | 67.28 | 67.85 |
| retain | default | 18.20 | 16.48 | -9.4% | 18.50 | 17.69 |
| retain | align32 | 16.04 | 17.04 | +6.2% | 18.62 | 18.16 |
| peek_mut | default | 41.44 | 40.15 | -3.1% | 40.84 | 41.86 |
| peek_mut | align32 | 41.31 | 40.42 | -2.2% | 40.31 | 42.86 |
| clone | default | 12.45 | 12.79 | +2.7% | 13.07 | 13.50 |
| clone | align32 | 12.38 | 12.58 | +1.6% | 13.07 | 13.75 |
| iter | default | 1.80 | 2.25 | +24.8% | 1.84 | 2.28 |
| iter | align32 | 1.82 | 1.75 | -3.9% | 1.87 | 1.73 |
| into_iter | default | 12.53 | 12.71 | +1.4% | 13.22 | 13.06 |
| into_iter | align32 | 12.65 | 12.85 | +1.6% | 13.29 | 14.12 |
| drain | default | 13.44 | 13.23 | -1.6% | 13.74 | 14.24 |
| drain | align32 | 13.44 | 13.30 | -1.0% | 13.71 | 14.58 |
| extend | default | 41.91 | 50.88 | +21.4% | 50.86 | 55.13 |
| extend | align32 | 40.80 | 48.95 | +20.0% | 52.93 | 55.63 |

### Small inputs, 256-byte items

| Operation | Build | std ns | Unmetered ns | Change | Local budget ns | Concurrent budget ns |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| push | default | 163.77 | 173.46 | +5.9% | 173.35 | 186.85 |
| push | align32 | 173.18 | 175.90 | +1.6% | 169.04 | 176.66 |
| push_reserved | default | 173.71 | 164.54 | -5.3% | 170.96 | 159.23 |
| push_reserved | align32 | 165.60 | 168.11 | +1.5% | 157.17 | 170.17 |
| push_ascending | default | 189.79 | 207.81 | +9.5% | 206.67 | 211.73 |
| push_ascending | align32 | 194.50 | 206.06 | +5.9% | 201.62 | 210.90 |
| pop_all | default | 251.14 | 200.77 | -20.1% | 224.02 | 238.16 |
| pop_all | align32 | 243.21 | 237.07 | -2.5% | 236.24 | 241.56 |
| from_vec | default | 102.60 | 103.17 | +0.6% | 106.31 | 105.07 |
| from_vec | align32 | 97.56 | 101.28 | +3.8% | 109.06 | 101.60 |
| sort | default | 236.27 | 246.10 | +4.2% | 252.07 | 277.38 |
| sort | align32 | 245.36 | 254.61 | +3.8% | 247.65 | 266.32 |
| append_small | default | 190.90 | 169.75 | -11.1% | 193.65 | 185.55 |
| append_small | align32 | 180.31 | 172.40 | -4.4% | 191.01 | 179.63 |
| append_large | default | 154.27 | 166.05 | +7.6% | 168.69 | 173.67 |
| append_large | align32 | 150.72 | 165.39 | +9.7% | 173.93 | 173.68 |
| retain | default | 51.70 | 50.77 | -1.8% | 55.63 | 57.87 |
| retain | align32 | 49.84 | 49.77 | -0.1% | 58.05 | 61.21 |
| peek_mut | default | 235.19 | 265.85 | +13.0% | 249.81 | 250.97 |
| peek_mut | align32 | 231.99 | 259.30 | +11.8% | 253.89 | 256.22 |
| clone | default | 32.53 | 31.99 | -1.7% | 33.96 | 34.17 |
| clone | align32 | 32.10 | 32.76 | +2.1% | 34.62 | 35.12 |
| iter | default | 2.88 | 2.68 | -7.0% | 2.62 | 2.70 |
| iter | align32 | 2.81 | 2.70 | -4.0% | 2.67 | 2.58 |
| into_iter | default | 31.69 | 31.47 | -0.7% | 33.46 | 34.11 |
| into_iter | align32 | 32.87 | 33.73 | +2.6% | 33.86 | 35.37 |
| drain | default | 32.70 | 34.08 | +4.2% | 34.81 | 33.35 |
| drain | align32 | 32.66 | 32.09 | -1.8% | 35.23 | 34.90 |
| extend | default | 105.70 | 115.41 | +9.2% | 115.81 | 118.35 |
| extend | align32 | 101.52 | 113.70 | +12.0% | 116.37 | 115.50 |

### Final-source follow-up and optional alignment

Unmetered means meet the unchanged observed +15% criterion in 53/60 default
conditions and 59/60 aligned conditions. The excesses below remain visible;
optional budgets are not counted as unmetered performance.

| Corpus | Operation / item bytes | Build | Additional ns | Change |
| --- | --- | --- | ---: | ---: |
| mixed | append_small_8 | default | +38.73 | +23.2% |
| mixed | into_iter_8 | default | +33.45 | +27.1% |
| mixed | iter_8 | default | +37.28 | +88.4% |
| mixed | push_ascending_8 | default | +661.90 | +26.5% |
| mixed | push_reserved_8 | default | +516.32 | +26.6% |
| small | extend_8 | align32 | +8.15 | +20.0% |
| small | extend_8 | default | +8.97 | +21.4% |
| small | iter_8 | default | +0.45 | +24.8% |

The small extend_u64 gap is 8.97 ns by default and 8.15 ns aligned. Mixed
extend_u64 is faster than std in both builds. Growth-boundary checks and
repair were inspected and alternatives measured; the retained implementation does
not establish a universal fixed overhead or per-element percentage.

The aligned executable's `size` text total is 704,467 bytes versus 693,019 default
(+1.65%). The [opt-in configurations](build-tuning.md) select alignment for the
application's compiler, CPU and workload. Compiler defaults remain the library default.

### Copy-witnessed array cloning (2026-11-10)

The 2026-11-09 local campaign measured `append_large_256` at +31.5% unmetered
(4,914.99 ns std, 6,462.08 ns). The time was not in append: the workload clones
its source heap first, and `[u64; 32]` elements were cloned through one
initializer slot per element and per `u64` (perf: 82% of cycles in
`<[u64; 32] as TryClone>::try_clone_into`, 3.43G instructions against std's
0.42G for the same run; std copies with `memmove`). Vec `try_extend_cloned`
now copies copy-witnessed elements in one bulk copy (see [Vec](vec.md));
the same run then executes 0.46G instructions.

Same protocol (CPU 6 at 3.4 GHz, seeds 617/6203/96011, every interval at least
one second, `sum(seconds) / sum(calls)`), mixed corpus, 256-byte items:

| Operation | std ns | Unmetered ns | Change | Local | Concurrent | 2026-11-09 unmetered |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| append_large | 4,972.00 | 5,108.58 | +2.7% | -0.0% | -0.0% | +31.5% |
| append_small | 9,459.29 | 9,370.37 | -0.9% | -0.3% | +0.1% | +1.0% |
| clone | 4,798.34 | 4,757.49 | -0.9% | -0.4% | +0.5% | +4.2% |
| into_iter | 4,721.73 | 4,745.89 | +0.5% | +0.5% | +1.0% | +3.6% |
| drain | 4,813.36 | 4,803.38 | -0.2% | -0.3% | -0.2% | +5.1% |
| retain | 7,960.30 | 7,786.44 | -2.2% | -0.7% | -1.0% | +1.4% |
| sort | 42,522.77 | 42,036.03 | -1.1% | +0.5% | +0.3% | +4.0% |
| from_vec | 11,125.20 | 11,278.54 | +1.4% | +0.6% | +2.1% | — |
| push | 17,156.37 | 16,123.52 | -6.0% | -6.0% | -6.1% | — |
| pop_all | 46,078.44 | 42,721.52 | -7.3% | -6.7% | -6.1% | -4.4% |
| peek_mut | 29,916.16 | 30,084.81 | +0.6% | +2.2% | +0.8% | +5.0% |

Raw records: `target/verification/local-2026-11-10/heap-witness/` (ignored).
The 8-byte conditions were not re-measured in this campaign.

### Metered append comparison

A separate paired campaign compares the original `64f233b` implementation with
the same-ledger storage-reuse algorithm first recorded at `b929f78`: 120 valid
intervals, three seeds and four corpus/item-width conditions. Identical-std means
range from 0.9805 to 1.0526. The final `7688418` implementation exchanges only the
three storage fields and is measured against std in the complete matrix above.
[Matched comparison groups](verification/binary-heap-append-comparison.csv) retain
both binaries' std baselines and exact counts.

| Corpus | Item bytes | Policy | Before ns | Storage reuse ns | Reduction |
| --- | ---: | --- | ---: | ---: | ---: |
| mixed | 8 | local | 548.21 | 221.53 | 59.6% |
| mixed | 8 | concurrent | 546.98 | 193.58 | 64.6% |
| mixed | 256 | local | 42,687.89 | 4,006.96 | 90.6% |
| mixed | 256 | concurrent | 41,893.38 | 3,970.58 | 90.5% |
| small | 8 | local | 76.41 | 68.14 | 10.8% |
| small | 8 | concurrent | 74.23 | 69.33 | 6.6% |
| small | 256 | local | 220.72 | 172.01 | 22.1% |
| small | 256 | concurrent | 219.08 | 179.58 | 18.0% |

## Code generation and remaining costs

### Controlled placement follow-up

The placement investigation separates a fixed instruction sequence from source
and caller changes. It follows the causal-analysis and setup-randomization
approaches in [Mytkowicz et al., ASPLOS 2009](https://www2.ccs.neu.edu/racket/Performance/mytkowicz.pdf)
and [Curtsinger and Berger, ASPLOS 2013](https://people.cs.umass.edu/~emery/pubs/stabilizer-asplos13.pdf).
Each executable has a fixed layout.

The first experiment retains the two observed 30-byte SSE2 scan loops, including
their original register allocations, at offsets 0/16/32/48 within a 64-byte line.
There are two copies per form and offset. The preparer checks identical loop
bytes within each register form. A fixed-width entry jump skips all padding.
Each process shares one driver, call boundary, shuffled corpus and data addresses
across all 16 kernels. Three input seeds and three lengths give 144 accepted
intervals, each at least one second. Correctness is checked against a Rust sum;
an intentionally changed add opcode is rejected by both the byte check and sum.

The table gives operation-weighted ns per complete scan, including the shared
call boundary, from the two copies and three seeds:

| Elements | Register form | Offset 0 | Offset 16 | Offset 32 | Offset 48 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | std | 2.98 | 3.18 | 3.24 | 3.56 |
| 16 | jzap | 2.98 | 3.17 | 3.14 | 3.55 |
| 256 | std | 26.43 | 25.32 | 26.15 | 39.31 |
| 256 | jzap | 26.24 | 25.45 | 25.98 | 39.09 |
| 4,096 | std | 444.53 | 450.96 | 450.19 | 613.95 |
| 4,096 | jzap | 451.47 | 453.16 | 452.92 | 615.14 |

Both register forms show the same offset-sensitive slowdown. The 48-versus-16
difference grows with scan length. Neither loop contains Result handling.
[Compact timings](verification/binary-heap-layout-kernels.csv) retain all 16
interval totals per seed/length. Recompute a cell from its two kernel columns
across three seeds: sum those six elapsed times, multiply by 1e9, and divide by
twice the sum of the three operation counts.

The AMD [Zen 4 optimization guide](https://www.numberworld.org/blogs/2024_8_7_zen5_avx512_teardown/57647_zen4_sog.pdf),
sections 2.8.3 and 2.9.1, describes hot-loop and branch placement and Op Cache
entries spanning two adjacent 64-byte lines. The VM exposes no CPU
performance-monitoring device; hardware-event counters were not recorded.

The collection benchmark comparison uses eight linker seeds, the archive baseline, and
equal std/jzap caller-inline/caller-outline forms. Each mean uses three input
seeds with matched work and interval lengths of at least one second. The mapped
scan and sift-up loop bytes match the recorded baseline in every linker variant.
The optional instruction-form comparison matches all 665 functions after masking
relocated addresses and Rust symbol hashes. Internal branch offsets, register
operands and function sizes are retained. Linker shuffling changes function
placement; internal block order is unchanged.

| Setting | iter std ns | iter jzap ns | Change | Ascending push std ns | Ascending push jzap ns | Change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Archive baseline | 42.90 | 76.15 | +77.5% | 2,470.57 | 3,133.28 | +26.8% |
| Link seed 617 | 41.88 | 41.64 | -0.6% | 2,999.48 | 2,452.71 | -18.2% |
| Link seed 6203 | 41.85 | 75.34 | +80.0% | 2,483.20 | 3,033.21 | +22.1% |
| Link seed 96011 | 44.32 | 45.02 | +1.6% | 2,472.17 | 2,454.68 | -0.7% |
| Link seed 104729 | 42.75 | 43.91 | +2.7% | 2,570.57 | 2,486.42 | -3.3% |
| Link seed 130363 | 42.04 | 80.57 | +91.6% | 2,589.55 | 3,242.03 | +25.2% |
| Link seed 155921 | 42.27 | 75.04 | +77.5% | 2,529.79 | 3,131.54 | +23.8% |
| Link seed 196613 | 43.61 | 75.96 | +74.2% | 2,514.75 | 3,110.84 | +23.7% |
| Link seed 262147 | 75.99 | 43.83 | -42.3% | 2,501.85 | 2,544.08 | +1.7% |
| Caller inline(always), both sides | 42.11 | 76.31 | +81.2% | 2,497.18 | 2,548.49 | +2.1% |
| Caller inline(never), both sides | 77.25 | 42.26 | -45.3% | 2,544.92 | 2,468.35 | -3.0% |

These two tasks have identical-std control means between 0.9693 and 1.0549.
The eight link seeds were fixed before timing; the table retains every seed.
For example, scan offsets (std/jzap) are 16/48 in the archive baseline, 0/32
with link seed 617, 32/0 with seed 96011, and 48/16 with seed 262147. Ascending
sift-up offsets are respectively 0/48, 48/32, 16/0, and 32/16. The sift-up body
remains 28 bytes and nine instructions. The outlined caller also changes register
allocation: its two scan loops are 28 bytes, at offsets 48/32. Its jzap scan
function is shared among policy specializations; the measured Unmetered caller
was confirmed through the source-line information and call target.

For both tasks, the eight link settings have this joint loop-placement distribution:

| Placement within a 64-byte line | Settings | Sample fraction |
| --- | ---: | ---: |
| Only jzap's loop crosses the boundary | 4 | 50.0% |
| Both loops fit within one line | 3 | 37.5% |
| Only std's loop crosses the boundary | 1 | 12.5% |

Giving each of the eight link settings equal weight produces these means:

| Task | std ns | jzap ns | Change |
| --- | ---: | ---: | ---: |
| iter | 46.84 | 60.16 | +28.4% |
| Ascending push | 2,582.67 | 2,806.94 | +8.7% |

The averaging uses absolute times, with per-setting means computed from the three
matched input seeds. These fractions describe the eight recorded settings. The
distribution of layouts across application builds was not measured.

Reserved insertion's 165 lifecycle intervals pass equal-work and duration checks.
Identical-std mean ratios
range from 0.8208 to 1.2417 across settings. A separate 30-interval original/archive
pair, targeting 2.5 seconds, has an original std-control mean of 1.1025.
Five of eleven settings in the first campaign and one of two in the paired
campaign failed the predeclared 0.9–1.1 control gate. These runs are stored in the
[compact control-screen table](verification/binary-heap-layout-control-screen.csv);
the separate pair is named pair_original/pair_base. The original and archive
executables have identical normalized instruction forms but different placement
of the outlined reserved-insertion function, including its exact 94-byte hot
region. The unmetered inner-loop offsets are 0 in the original and 32 in the
archive executable; both 28-byte loops fit within one 64-byte line.

A shared-buffer probe transfers each
of 64 preallocated output buffers between std and jzap Heaps using safe
ownership conversions outside timing. Input and output addresses are identical
for each implementation; pointer/capacity identity is asserted after every
interval. Timed work is clear/refill plus a root read. It excludes allocation,
destruction and the final full scan. The complete jzap heap array is checked
against repeated std pushes. Three shuffled labels include two identical std
intervals; three input seeds and four sizes give 36 intervals of at least one
second. Both sift-up loops are 28 bytes/nine instructions at offset 48.

| Elements | std ns | jzap ns | Difference ns | Change | std-control change |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 1.43 | 1.99 | +0.56 | +39.5% | +1.0% |
| 16 | 27.10 | 27.50 | +0.40 | +1.5% | +0.3% |
| 256 | 499.53 | 530.60 | +31.07 | +6.2% | +3.1% |
| 4,096 | 38,976.75 | 40,195.31 | +1,218.56 | +3.1% | -0.8% |

An additional nine intervals reuse the lifecycle benchmark's original mixed
corpus, including its case order. Corpus digests and element/call counts match
the original benchmark for all three seeds. Output storage is still shared and
reused. Its std/jzap means are 1,913.32/1,746.15 ns (-8.7%); the identical-std
control mean is -3.8%. Both 28-byte sift-up loops start at offset 0 in this
executable. This preserves the original inputs while removing timed allocation,
destruction and the final full scan.

[Compact shared-buffer timings](verification/binary-heap-layout-reserved.csv)
retain all three totals per seed/size; size -1 denotes the original mixed corpus.
A cell is sum(seconds) times 1e9 divided by sum(operations) for its three seeds.
These controls range from 0.9617 to 1.0312. The individual contributions of
allocation, destruction, the final scan and placement were not measured separately.
For empty input, the jzap wrapper
still saves registers and tests its length; std clears the length and returns
before creating a stack frame. The observed gap is 0.56 ns. Full wrapper sizes
are 167/302 bytes (std/jzap, including the
benchmark's unwrap error handling), while their sift-up work matches.

Small unknown-length extension:

| Setting | std ns | jzap ns | Change |
| --- | ---: | ---: | ---: |
| Archive baseline | 44.08 | 50.38 | +14.3% |
| Growth helper without cold hint | 39.73 | 48.90 | +23.1% |
| Caller inline(always), both sides | 42.05 | 55.74 | +32.5% |
| Caller inline(never), both sides | 52.12 | 47.91 | -8.1% |

All four identical-std control means are within 0.9–1.1. The Result-tag test,
length/capacity reloads and admission check after growth are present in both
growth-helper variants. Production source, default compiler flags and verification
contracts are unchanged. The 14 prepared executables pass 6,720 functional workload
processes covering both widths, both profiles, every operation, std and the three
benchmarked jzap policies.

Build and partial-run commands are in the
[throughput tool guide](../tools/throughput-analysis/README.md). Recompute the
lifecycle/extension means and the withheld controls without Rust or local logs:

```sh
python3 tools/throughput-analysis/report.py --from-compact docs/verification/binary-heap-layout-timing.csv
python3 tools/throughput-analysis/report.py --from-compact docs/verification/binary-heap-layout-control-screen.csv
```

For a partial placement reproduction, prepare only two linker seeds and compare
the collection operation:

```sh
OUT=target/verification/heap-layout-reproduction
python3 tools/throughput-analysis/prepare_heap_layout.py prepare --output "$OUT" --forms base --link-seeds 617,262147
python3 tools/throughput-analysis/prepare_heap_layout.py inspect --output "$OUT" --link-seeds 617,262147
python3 tools/throughput-analysis/prepare_heap_layout.py kernel --output "$OUT" --sizes 256 --seeds 617,6203,96011
python3 tools/throughput-analysis/compare.py --binary base="$OUT/base" --binary shuffled="$OUT/link_262147" --tasks binary_heap_iter_8 --small-tasks '' --output "$OUT/scan"
python3 tools/throughput-analysis/prepare_heap_layout.py prepare-reserved --output "$OUT"
python3 tools/throughput-analysis/prepare_heap_layout.py reserved --output "$OUT" --sizes 16,256,4096
python3 tools/throughput-analysis/prepare_heap_layout.py prepare-mixed --output "$OUT"
python3 tools/throughput-analysis/prepare_heap_layout.py mixed --output "$OUT"
```

Actual addresses depend on the checkout path, compiler and link inputs.

### Existing code generation analysis

Insertion derives its index from the published length after successful Vec growth.
This exposes one insertion position to LLVM, avoiding independent pre/post-growth
indices and the large-value temporary copies seen in the earlier analysis.
Paired-child sifts express selection as a bool increment, enabling `cmp`/`sbb`
instead of a value-dependent branch. The frontier checks remain in place.
The retain guard skips unnecessary repair and clamping; its destructor can still
be outlined. The cost estimate is deferred when a full rebuild is already selected.

The product `7688418` timed executables give this layout for the measured
drain_u64 loop; std and jzap both cross a line in the default build:

| Build | Implementation | Loop address | Bytes | Offset in 64-byte line | Crosses line |
| --- | --- | --- | ---: | ---: | --- |
| Default | std | 0x2b370 | 30 | 48 | Yes |
| Default | Unmetered | 0x2a6f0 | 30 | 48 | Yes |
| Aligned | std | 0x2c1e0 | 30 | 32 | No |
| Aligned | Unmetered | 0x2b4a0 | 30 | 32 | No |

Both loops have seven instructions. The byte encodings below are unchanged
between the default and aligned build for each implementation:

| Instruction | std bytes | Unmetered bytes | Length |
| --- | --- | --- | ---: |
| First 16-byte load | f3 41 0f 6f 14 f6 | f3 41 0f 6f 14 16 | 6 |
| Packed u64 add | 66 0f d4 ca | 66 0f d4 ca | 4 |
| Second 16-byte load | f3 41 0f 6f 54 f6 10 | f3 41 0f 6f 54 16 10 | 7 |
| Packed u64 add | 66 0f d4 c2 | 66 0f d4 c2 | 4 |
| Counter advance | 48 83 c6 04 | 48 83 c2 20 | 4 |
| Compare | 48 39 f2 | 48 39 d1 | 3 |
| Back edge | 75 e2 | 75 e2 | 2 |

std advances an element counter by four with scale-eight addressing; jzap advances
a byte counter by 32. Both load and add four u64 values per iteration, with no
additional per-element failure check. Source locations were mapped through a
line-labeled companion after verifying identical loop bytes and function-relative
offsets. Addresses above identify the recorded timed executables.

The full timing matrix compares both build settings.

The measured iter_u64 loop is also seven instructions and 30 bytes: two
16-byte loads, two packed adds, an element-counter advance by four, a comparison
and the back edge. std uses r10/r11 where jzap uses r9/r8; the instruction lengths
and work per iteration match. Its placement differs:

| Build | Implementation | Loop address | Offset in 64-byte line | Crosses line |
| --- | --- | --- | ---: | --- |
| Default | std | 0x27dd0 | 16 | No |
| Default | Unmetered | 0x27c30 | 48 | Yes |
| Aligned | std | 0x287e0 | 32 | No |
| Aligned | Unmetered | 0x28620 | 32 | No |

The loop bytes and function-relative offsets were checked against the matching
source-line companions. Neither scan has an additional per-element failure
branch. Loop alignment is an optional application build setting.

Unknown-length extension has additional work at growth boundaries. In the
u64 success path, `grow_for_push` is followed by a load of the returned Result tag,
`cmp $0xb`/`jne`, then length/capacity reloads and `cmp`/`jae` before publication.
The measured unmetered u64 success path inlines the guard's
rebuild_tail/rebuild/sift_down chain (the repair region begins at 0x27690 in the
default executable). Matching source-line companion bytes confirm the inlined
repair chain.
std's grow-and-handle path has no Result value to decode after the call. That
decode and the post-growth reloads occur only at capacity growth, which is
logarithmic in the final length under geometric growth. Both implementations
still test available capacity while inserting; heap-tail repair runs once after
extension. jzap preserves the rejected item and repairs the successful prefix
on refusal. Measurements cover small and mixed corpora with default and
32-byte loop alignment.

The earlier metered append path copied the large suffix into the small receiver
and could rebuild the entire heap. The follow-up checks ledger identity and equal
limits, then reuses the larger heap's allocation. It grows that allocation to the exact
combined length when both owners need growth. This request is no larger than the
original receiver's geometric request. If the original receiver already has room
and the source does not, append retains the original no-allocation path. Storage
exchange moves only pointer, length and capacity; policy handles remain in place.
Different domains or limits keep the original path. This addresses copying and
heap repair work in addition to accounting instructions.

For identical ledgers and limits, admission follows the existing requested-byte
contract:

| Available storage | Follow-up action | New reservation |
| --- | --- | --- |
| Larger heap already holds the combined length | Exchange storage, append the smaller suffix | None |
| Only the original receiver holds the combined length | Keep the original receiver | None |
| Both need growth | Grow the larger heap exactly before exchange | Combined length times item size, no larger than the original geometric request |

For example, one u64 slot plus eight source slots occupy 72 requested bytes. With
a 144-byte ledger, the original receiver can reserve 72 new bytes. Doubling the
larger buffer would require 128 new bytes and fail; exact growth reserves the same
72 bytes and succeeds. Final held storage is then 80 bytes, including the emptied
source's retained slot. Allocator-usable sizes are checked separately.

### Growth and repair experiments

Three growth experiments were discarded: a cold writer, a returned slot/length
pair, and a separate hot path with a returned growth slot. They did not improve
the small 8-byte extension target; the original push path is retained. In the
inspected specializations, the slot/length helper replaced a tail jump with a
call frame and additional return-buffer copies. Both slot experiments were also
measured with matching default/32-byte settings. An inline hint on the
storage-reuse helper produced byte-identical executables and was discarded.
An additional rebuild-tail inline hint removed the separate u64 specialization,
but matching default/32-byte trials did not show a consistent std-relative gain;
the original repair annotations are retained.

## Allocation observations

The final `7688418` observer run covers 23,040 observations: four implementations,
three seeds, 64 corpus cases, two item widths and 15 operations. The
[compact allocation table](verification/binary-heap-memory.csv) has 90 policy/task
rows; each compares all 192 seed/case pairs with std.

| Metric | Largest case ratio to std |
| --- | ---: |
| Requested held bytes | 1.0000 |
| Requested peak bytes | 1.0000 |
| Allocator-usable held bytes | 1.0000 |
| Allocator-usable peak bytes | 1.0000 |
| Allocation/reallocation calls | 1.0000 |

For append_large_256, exact metered growth reduces mean held and peak requested
storage to 134,294.67 bytes versus std's 264,362.67 bytes across the 192 seed/case
pairs (ratio 0.5080). Local and concurrent policies match; unmetered storage
matches std. Allocator-usable means are 134,309.67 and 264,377.67 bytes respectively.

With the default Unlimited length policy, heap handles are 24 bytes for
std/unmetered and 32 bytes for borrowed local or concurrent budgets on this
64-bit target. Held snapshots include all live
workload owners: both append heaps, including the emptied source's retained
capacity, and both source and clone where applicable. Tracked balance returns
to zero after cleanup in every case. Ledgers are constructed outside observation;
this is not a new measurement of owning-ledger bootstrap costs.

Each implementation/seed/case/width runs in a fresh process, so allocator reuse
from a preceding implementation cannot bias usable-size comparisons. The observer
tracks System allocator boundaries and `malloc_usable_size`; it does not measure
stack storage, resident pages or temporary overlap internal to libc realloc.

## Reproduction

Use proof/checker `6389ac4` or a later documentation-only revision, activate the
documented toolchain, then select checks independently. The shared
runner writes command lines, outcomes and logs beneath an ignored output directory:

```sh
python3 scripts/verify.py --area binary-heap --check api --check tests --check release --check overflow
python3 scripts/verify.py --area binary-heap --check miri --check miri-tree --check miri-32 --check asan --check fuzz
python3 scripts/verify.py --area binary-heap --check verus
python3 scripts/verify.py --area binary-heap --check kani
python3 scripts/verify.py --area binary-heap --check models
python3 scripts/verify.py --area all --check certificate
```

`models` for this area invokes actual Rust Kani kernels and compiled defect checks.
`verus` includes the unchanged product geometry module. See the verification
checkpoint for the distinction from inherited projections and execution contracts.
A partial selection is not a complete certificate.

`kani` selects the actual Rust kernels without defect controls. To inspect only
the append-success and storage-refusal kernels, invoke Kani directly:

```sh
cargo kani --lib --harness collections::binary_heap::proofs::actual_metered_ -Z unstable-options --cbmc-args --object-bits 12
```

The direct product geometry proof can also run without Python when Verus is on
PATH or JZAP_VERUS names its executable:

```sh
"${JZAP_VERUS:-verus}" tools/direct-verus/binary-heap.rs --crate-type lib --crate-name direct_product --edition=2024 --cfg 'feature="alloc"' --no-cheating --output-json
```

Timing uses `tools/throughput-analysis/src/bin/binary_heap.rs` and the common
`compare.py`/`report.py` drivers. The allocator observer is compiled separately
with `btree-memory`; the time binary excludes it. Known-length input-copy
construction reserves exactly on both sides. No comparator or source failure is
timed as success, and std is not charged for optional jzap budget accounting.

Timing and allocation use runtime `7688418`; the current proof/checker revision
builds byte-identical time executables on the measured toolchain. Finish both
builds before timing, and run compilation/proofs separately from the campaign:

```sh
cargo build --release --offline --manifest-path tools/throughput-analysis/Cargo.toml --bin binary_heap --target-dir target/verification/binary-heap/default-build
RUSTFLAGS='-C llvm-args=-align-loops=32' cargo build --release --offline --manifest-path tools/throughput-analysis/Cargo.toml --bin binary_heap --target-dir target/verification/binary-heap/align32-build
BIN=target/verification/binary-heap/default-build/release/binary_heap
TASKS="$($BIN tasks)"
python3 tools/throughput-analysis/compare.py --binary default="$BIN" --binary align32=target/verification/binary-heap/align32-build/release/binary_heap --tasks "$TASKS" --small-tasks "$TASKS" --small-variants default,align32 --output target/verification/binary-heap/performance/current
python3 tools/throughput-analysis/report.py --input target/verification/binary-heap/performance/current --compact target/verification/binary-heap/performance/matched.csv
```

For a partial timing campaign, replace TASKS with a comma-separated list from
`$BIN tasks`; choose mixed and small task lists independently. If an identical-std
condition mean falls outside 0.9–1.1, repeat that complete condition in both builds
for all three seeds in a fresh output directory. The recorded campaign repeated
mixed `binary_heap_drain_256` and small `binary_heap_push_ascending_256`. Select
repeats using a new campaign's own controls, before assessing product performance.

The compact file retains every matched seed's counts, input/result digests and
five elapsed times. Published means can be recomputed without Rust or local logs:

```sh
python3 tools/throughput-analysis/report.py --from-compact docs/verification/binary-heap-timing.csv
python3 tools/throughput-analysis/report.py --from-compact docs/verification/binary-heap-control-screen.csv
python3 tools/throughput-analysis/report.py --from-compact docs/verification/binary-heap-append-comparison.csv
```

The first file directly reproduces the current tables; the second retains the
initial rejected controls. For the paired append experiment, build `64f233b` and
`b929f78` from Git in disposable directories, then pass both default-release
binaries to compare.py with append_large_8 and append_large_256 in both corpora.
No earlier source snapshot is required in the current tree.

For assembly inspection, use `objdump -d -C` on each recorded executable. A
source-line companion can be built with `cargo rustc` and
`-- -C debuginfo=1 -C strip=none`; verify the corresponding loop bytes and
function-relative offsets before mapping its source locations to a timed build.
Keep those temporary binaries and dumps under ignored `target/` storage.

For allocator observations, use a separate build. The first command below replays
every label/seed/case/width in a fresh process, keeping allocator history matched:

```sh
cargo build --release --offline --manifest-path tools/throughput-analysis/Cargo.toml --bin binary_heap --features btree-memory --target-dir target/verification/binary-heap/memory-build
python3 tools/throughput-analysis/memory_report.py --heap-binary target/verification/binary-heap/memory-build/release/binary_heap --format binary-heap --output target/verification/binary-heap/memory.csv --max-ratio 1.15
# Select one process, seed, corpus case and width:
target/verification/binary-heap/memory-build/release/binary_heap memory unlimited 6203 10 8
```
