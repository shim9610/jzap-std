//! Original nonallocating owner transitions over the complete numeric envelope.
use super::*;

fn push<L: LengthPolicy>(length: usize, capacity: usize, policy: L) {
    let mut owner = super::query_proofs::numeric_unit_owner(length, capacity, policy);
    let pointer = owner.as_slice().as_ptr();
    let limit = owner.max_len();
    match owner.push_within_capacity(()) {
        Ok(place) => {
            assert!(length < capacity);
            assert!(core::ptr::eq(place, pointer));
        }
        Err(rejected) => {
            assert!(length == capacity && rejected.error == Error::Capacity);
            assert!(rejected.value == ());
        }
    }
    assert!(
        owner.len()
            == if length < capacity {
                length + 1
            } else {
                length
            }
    );
    assert!(owner.capacity() == capacity && owner.max_len() == limit);
    assert!(owner.as_slice().as_ptr() == pointer);
}

#[kani::proof]
fn within_capacity_push_preserves_every_numeric_owner() {
    let length: usize = kani::any();
    let capacity: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(length <= capacity && capacity <= limit);
    push(length, capacity, LengthLimit::new(limit));
    push(length, capacity, Unlimited);
}

#[kani::proof]
fn suffix_guard_restores_every_admitted_original_length() {
    let original: usize = kani::any();
    let initialized: usize = kani::any();
    let capacity: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(original <= initialized && initialized <= capacity && capacity <= limit);
    let mut owner =
        super::query_proofs::numeric_unit_owner(initialized, capacity, LengthLimit::new(limit));
    let pointer = owner.as_slice().as_ptr();
    drop(SuffixGuard {
        owner: &mut owner,
        original,
    });
    assert!(owner.len() == original && owner.capacity() == capacity && owner.max_len() == limit);
    assert!(owner.as_slice().as_ptr() == pointer);
}
