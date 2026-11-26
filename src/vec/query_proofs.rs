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
    // SAFETY: every unit-array count has a valid zero-byte, alignment-one layout.
    let capacity = unsafe { crate::capacity::Capacity::from_validated_raw(capacity) };
    // The checked constructor admits exactly the states asserted above.
    let Ok(extent) = crate::extent::Extent::try_new(length, capacity, policy) else {
        unreachable!()
    };
    Vec::<(), _, _> {
        ptr: NonNull::dangling(),
        extent,
        policy: Unmetered,
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
