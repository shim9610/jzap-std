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
