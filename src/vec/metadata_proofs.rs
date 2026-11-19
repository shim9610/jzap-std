//! Direct original-owner checks; unit storage permits unrestricted numeric lengths.
use super::*;

fn length_update<L: LengthPolicy>(length: usize, capacity: usize, policy: L, requested: usize) {
    let mut owner = super::query_proofs::numeric_unit_owner(length, capacity, policy);
    let pointer = owner.as_slice().as_ptr();
    let limit = owner.max_len();
    // SAFETY: units require no bytes, resources or per-element initialization.
    let result = unsafe { owner.try_set_len(requested) };
    assert!(result.is_ok() == (requested <= capacity));
    if requested > capacity {
        assert!(result == Err(Error::Capacity));
        assert!(owner.len() == length);
    } else {
        assert!(owner.len() == requested);
    }
    assert!(owner.capacity() == capacity && owner.max_len() == limit);
    assert!(owner.as_slice().as_ptr() == pointer);
}
#[kani::proof]
fn every_numeric_length_update_and_refusal_preserves_other_metadata() {
    let length: usize = kani::any();
    let capacity: usize = kani::any();
    let limit: usize = kani::any();
    let requested: usize = kani::any();
    kani::assume(length <= capacity && capacity <= limit);
    length_update(length, capacity, LengthLimit::new(limit), requested);
    length_update(length, capacity, Unlimited, requested);
}

#[kani::proof]
fn original_empty_constructors_and_standard_adoption_cover_every_unit_length_limit() {
    let length: usize = kani::any();
    let limit: usize = kani::any();
    let empty = Vec::<()>::with_limit(limit);
    assert!(empty.len() == 0 && empty.capacity() == limit && empty.max_len() == limit);
    drop(empty);
    let empty = Vec::<u64>::with_limit(limit);
    assert!(empty.len() == 0 && empty.capacity() == 0 && empty.max_len() == limit);
    drop(empty);
    let empty = Vec::<()>::new();
    assert!(empty.len() == 0 && empty.capacity() == usize::MAX);
    drop(empty);
    let mut source = alloc::vec::Vec::<()>::new();
    // SAFETY: every unit is valid without bytes; std's unit capacity is usize::MAX.
    unsafe {
        source.set_len(length);
    }
    let pointer = source.as_ptr();
    match Vec::try_from_std_in(source, Unmetered, LengthLimit::new(limit)) {
        Ok(owner) => {
            assert!(length <= limit);
            assert!(owner.len() == length && owner.capacity() == limit && owner.max_len() == limit);
            assert!(owner.as_slice().as_ptr() == pointer);
            let source = owner.into_std();
            assert!(
                source.len() == length
                    && source.capacity() == usize::MAX
                    && source.as_ptr() == pointer
            );
        }
        Err(rejected) => {
            assert!(length > limit && rejected.error == Error::Capacity);
            assert!(rejected.value.len() == length && rejected.value.capacity() == usize::MAX);
            assert!(rejected.value.as_ptr() == pointer);
        }
    }
}

#[kani::proof]
fn boxed_and_vector_unit_owners_transfer_every_word_length_without_moving_storage() {
    let length: usize = kani::any();
    let pointer = NonNull::<()>::dangling();
    // SAFETY: aligned zero-byte global storage, valid unit values, zero charge.
    let source = unsafe {
        crate::boxed::Box::from_raw_parts(NonNull::slice_from_raw_parts(pointer, length), Unmetered)
    };
    let owner = Vec::from_boxed_slice(source);
    assert!(
        owner.len() == length && owner.capacity() == usize::MAX && owner.max_len() == usize::MAX
    );
    assert!(owner.as_slice().as_ptr() == pointer.as_ptr());
    let source = owner.try_into_boxed_slice().unwrap();
    assert!(source.len() == length && source.as_slice().as_ptr() == pointer.as_ptr());
    drop(source);
}
