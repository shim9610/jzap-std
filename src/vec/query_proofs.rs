//! Full-word metadata queries on actual owners; no allocator or pointer model copy.
use super::*;
pub(super) fn numeric_unit_owner<L: LengthPolicy>(
    length: usize,
    capacity: usize,
    policy: L,
) -> Vec<(), Unmetered, L> {
    assert!(length <= capacity && capacity <= policy.max_len());
    // This in-crate representation fixture exercises the full numeric envelope,
    // including capacities below the limit. Public ZST raw interoperability has
    // the stricter capacity==limit contract, so it is not used for this fixture.
    Vec::<(), _, _> {
        ptr: NonNull::dangling(),
        len: Checked::new(length),
        // SAFETY: every unit-array count has a valid zero-byte, alignment-one layout.
        cap: unsafe { crate::capacity::Capacity::from_validated_raw(capacity) },
        policy: Unmetered,
        limit: policy,
        marker: PhantomData,
    }
}
fn queries<L: LengthPolicy>(length: usize, capacity: usize, policy: L, expected_limit: usize) {
    let mut owner = numeric_unit_owner(length, capacity, policy);
    assert!(owner.max_len() == expected_limit);
    assert!(owner.remaining_limit() == expected_limit - length);
    assert!(owner.spare_capacity_mut().len() == capacity - length);
    let (initialized, spare) = owner.split_at_spare_mut();
    assert!(initialized.len() == length && spare.len() == capacity - length);
    assert!(owner.len() == length && owner.capacity() == capacity);
    assert!(owner.max_len() == expected_limit);
}
#[kani::proof]
fn every_valid_zst_owner_query_and_spare_partition() {
    let length: usize = kani::any();
    let capacity: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(length <= capacity && capacity <= limit);
    queries(length, capacity, LengthLimit::new(limit), limit);
    queries(length, capacity, Unlimited, usize::MAX);
}

// Closure fixture (issue #16, unit 1 step 5). A closure harness builds each
// owner with `closure_owner`, whose only assumption is `closure_invariant`,
// runs actual owner methods and asserts `closure_invariant` on every return.

/// `len <= cap <= limit`, read from the owner's own words, and the pointer
/// pairing of issue #16 rule B at an empty or zero-sized owner: the pointer is
/// `NonNull::dangling()` (address `align_of::<T>()`), read through `as_ptr`.
/// The pairing of a nonempty sized owner with its allocation is checked by the
/// stubbed sized harnesses (`growth_proofs::sized`).
pub(super) fn closure_invariant<T, P: AllocationPolicy, L: LengthPolicy>(
    owner: &Vec<T, P, L>,
) -> bool {
    owner.len.get() <= owner.cap.get()
        && owner.cap.get() <= owner.limit.max_len()
        && (mem::size_of::<T>() != 0 && owner.cap.get() != 0
            || owner.as_ptr().addr() == mem::align_of::<T>())
}

/// An arbitrary owner of zero-sized elements: every length and capacity word
/// that satisfies `closure_invariant` under the given limit, and nothing else.
/// A zero-sized element needs no bytes, so every count has a valid layout and
/// the aligned dangling pointer is valid storage for it.
pub(super) fn closure_owner<T, P: AllocationPolicy, L: LengthPolicy>(
    policy: P,
    limit: L,
) -> Vec<T, P, L> {
    assert!(mem::size_of::<T>() == 0);
    let owner = Vec {
        ptr: NonNull::dangling(),
        len: Checked::new(kani::any()),
        // SAFETY: every count of a zero-sized element has a valid zero-byte layout.
        cap: unsafe { crate::capacity::Capacity::from_validated_raw(kani::any()) },
        policy,
        limit,
        marker: PhantomData,
    };
    kani::assume(closure_invariant(&owner));
    owner
}

/// The exit check of a closure harness: the invariant, under the same limit.
pub(super) fn closure_exit<T, P: AllocationPolicy, L: LengthPolicy>(
    owner: &Vec<T, P, L>,
    limit: usize,
) {
    assert!(closure_invariant(owner));
    assert!(owner.max_len() == limit);
}

/// A unit slice of any length, at an aligned non-null address.
pub(super) fn closure_units<'a>() -> &'a [()] {
    // SAFETY: zero-sized values need no storage.
    unsafe { core::slice::from_raw_parts(NonNull::<()>::dangling().as_ptr(), kani::any()) }
}

/// Any range endpoint.
pub(super) fn closure_bound() -> core::ops::Bound<usize> {
    match kani::any::<u8>() {
        0 => core::ops::Bound::Included(kani::any()),
        1 => core::ops::Bound::Excluded(kani::any()),
        _ => core::ops::Bound::Unbounded,
    }
}
