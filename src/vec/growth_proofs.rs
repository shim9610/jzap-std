//! Original storage-growth publication, with unrestricted numeric unit storage.
use super::*;

fn publication<L: LengthPolicy>(
    len: usize,
    capacity: usize,
    needed: usize,
    policy: L,
    exact: bool,
) {
    let mut owner = super::query_proofs::numeric_unit_owner(len, capacity, policy);
    let pointer = owner.as_slice().as_ptr();
    let limit = owner.max_len();
    let expected = geometry::growth_for_layout(capacity, needed, limit, 0, exact).unwrap();
    assert!(owner.grow_to(needed, exact) == Ok(()));
    assert!(owner.len() == len && owner.capacity() == expected && owner.max_len() == limit);
    assert!(needed <= owner.capacity() && owner.capacity() <= limit);
    assert!(owner.as_slice().as_ptr() == pointer && owner.layout().size() == 0);
}

#[kani::proof]
fn storage_growth_publishes_only_the_admitted_capacity_for_every_numeric_owner() {
    let len: usize = kani::any();
    let capacity: usize = kani::any();
    let needed: usize = kani::any();
    let limit: usize = kani::any();
    let exact: bool = kani::any();
    kani::assume(len <= capacity && capacity < needed && needed <= limit);
    publication(len, capacity, needed, LengthLimit::new(limit), exact);
    publication(len, capacity, needed, Unlimited, exact);
}

/// A full owner's push grows exactly as `grow(1, false)` proposes, or refuses
/// with the same error and leaves length and capacity unchanged.
fn push_growth<L: LengthPolicy>(len: usize, policy: L) {
    let mut owner = super::query_proofs::numeric_unit_owner(len, len, policy);
    let limit = owner.max_len();
    let expected = required_len(len, 1, limit)
        .and_then(|needed| geometry::growth_for_layout(len, needed, limit, 0, false));
    match owner.try_push(()) {
        Ok(()) => {
            assert!(Ok(owner.capacity()) == expected);
            assert!(owner.len() == len + 1 && owner.capacity() <= limit);
        }
        Err(rejected) => {
            assert!(expected.err() == Some(rejected.error));
            assert!(owner.len() == len && owner.capacity() == len);
        }
    }
}

#[kani::proof]
fn push_growth_matches_grow_one_for_every_full_numeric_owner() {
    let len: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(len <= limit);
    push_growth(len, LengthLimit::new(limit));
    push_growth(len, Unlimited);
}
