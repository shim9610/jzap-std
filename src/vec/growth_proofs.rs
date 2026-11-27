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

// Closure harnesses (issue #16, unit 1 step 5): the capacity writes of
// `resize_storage` and `grow_for_push`, driven through every unconditional
// entry point from an arbitrary owner that satisfies only `closure_invariant`.
use super::query_proofs::{closure_exit, closure_owner};

fn closure_reserve_case<L: LengthPolicy>(limit: L, exact: bool) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let additional: usize = kani::any();
    let result = if exact {
        owner.try_reserve_exact(additional)
    } else {
        owner.try_reserve(additional)
    };
    closure_exit(&owner, max);
    assert!(owner.len() == len);
    match result {
        Ok(()) => assert!(cap <= owner.capacity() && owner.capacity() - len >= additional),
        Err(_) => assert!(owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_reserve_preserves_the_invariant() {
    closure_reserve_case(LengthLimit::new(kani::any()), false);
    closure_reserve_case(Unlimited, false);
}
#[kani::proof]
fn closure_reserve_exact_preserves_the_invariant() {
    closure_reserve_case(LengthLimit::new(kani::any()), true);
    closure_reserve_case(Unlimited, true);
}

fn closure_grow_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let additional: usize = kani::any();
    let result = owner.grow(additional, kani::any());
    closure_exit(&owner, max);
    assert!(owner.len() == len);
    match result {
        Ok(()) => assert!(owner.capacity() - len >= additional),
        Err(_) => assert!(owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_grow_preserves_the_invariant() {
    closure_grow_case(LengthLimit::new(kani::any()));
    closure_grow_case(Unlimited);
}

fn closure_reserve_length_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let additional: usize = kani::any();
    let result = owner.reserve_length(additional);
    closure_exit(&owner, max);
    assert!(owner.len() == len);
    match result {
        // The value the bulk writers publish as the new length.
        Ok(needed) => {
            assert!(len.checked_add(additional) == Some(needed) && needed <= owner.capacity())
        }
        Err(_) => assert!(owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_reserve_length_preserves_the_invariant() {
    closure_reserve_length_case(LengthLimit::new(kani::any()));
    closure_reserve_length_case(Unlimited);
}

fn closure_push_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let result = owner.try_push(());
    closure_exit(&owner, max);
    match result {
        Ok(()) => assert!(owner.len() == len + 1 && cap <= owner.capacity()),
        Err(_) => assert!(owner.len() == len && owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_push_preserves_the_invariant() {
    closure_push_case(LengthLimit::new(kani::any()));
    closure_push_case(Unlimited);
}

fn closure_shrink_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    // A zero-sized element returns before the capacity write; the sized
    // instantiations are in the stubbed harnesses.
    assert!(owner.try_shrink_to(kani::any()) == Ok(()));
    assert!(owner.try_shrink_to_fit() == Ok(()));
    closure_exit(&owner, max);
    assert!(owner.len() == len && owner.capacity() == cap);
}
#[kani::proof]
fn closure_shrink_preserves_the_invariant_of_a_unit_owner() {
    closure_shrink_case(LengthLimit::new(kani::any()));
    closure_shrink_case(Unlimited);
}
