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

// Closure harnesses (issue #16, unit 1 step 5): one actual writer each, from an
// arbitrary owner that satisfies only `closure_invariant`, under both sealed
// length policies. The fixture is in `query_proofs.rs`.
use super::query_proofs::{closure_bound, closure_exit, closure_owner, closure_units};

fn closure_constructors<T>(limit: usize) {
    let zero_sized = mem::size_of::<T>() == 0;
    let expected_capacity = if zero_sized { limit } else { 0 };
    let owner = Vec::<T>::new();
    assert!(owner.len() == 0 && owner.capacity() == if zero_sized { usize::MAX } else { 0 });
    closure_exit(&owner, usize::MAX);
    drop(owner);
    let owner = Vec::<T>::new_in(Unmetered);
    assert!(owner.len() == 0);
    closure_exit(&owner, usize::MAX);
    drop(owner);
    let owner = Vec::<T>::with_limit(limit);
    assert!(owner.len() == 0 && owner.capacity() == expected_capacity);
    closure_exit(&owner, limit);
    drop(owner);
    let owner = Vec::<T, _>::with_limit_in(limit, Unmetered);
    closure_exit(&owner, limit);
    drop(owner);
    let owner = Vec::<T, Unmetered, LengthLimit>::empty(Unmetered, LengthLimit::new(limit));
    closure_exit(&owner, limit);
    drop(owner);
    let owner = Vec::<T, Unmetered, Unlimited>::empty(Unmetered, Unlimited);
    closure_exit(&owner, usize::MAX);
    drop(owner);
}

#[kani::proof]
fn closure_empty_constructors_establish_the_invariant() {
    #[repr(align(64))]
    struct Aligned([u8; 64]);
    let limit: usize = kani::any();
    closure_constructors::<()>(limit);
    closure_constructors::<[(); 3]>(limit);
    closure_constructors::<u8>(limit);
    closure_constructors::<u64>(limit);
    closure_constructors::<[u64; 200]>(limit);
    closure_constructors::<Aligned>(limit);
}

#[kani::proof]
fn closure_capacity_constructors_establish_the_invariant() {
    let capacity: usize = kani::any();
    let limit: usize = kani::any();
    let Ok(owner) = Vec::<()>::try_with_capacity(capacity) else {
        unreachable!()
    };
    closure_exit(&owner, usize::MAX);
    drop(owner);
    let Ok(owner) = Vec::<()>::try_with_capacity_in(capacity, Unmetered) else {
        unreachable!()
    };
    closure_exit(&owner, usize::MAX);
    drop(owner);
    match Vec::<()>::try_with_capacity_and_limit(capacity, limit) {
        Ok(owner) => {
            assert!(capacity <= limit);
            closure_exit(&owner, limit);
        }
        Err(error) => assert!(capacity > limit && error == Error::Capacity),
    }
    match Vec::<()>::try_with_capacity_and_limit_in(capacity, limit, Unmetered) {
        Ok(owner) => {
            assert!(capacity <= limit);
            closure_exit(&owner, limit);
        }
        Err(error) => assert!(capacity > limit && error == Error::Capacity),
    }
}

fn closure_from_array_case<const N: usize, L: LengthPolicy>(limit: L) {
    let max = limit.max_len();
    match Vec::try_from_array_in([(); N], Unmetered, limit) {
        Ok(owner) => {
            assert!(N <= max && owner.len() == N);
            closure_exit(&owner, max);
        }
        Err(rejected) => assert!(N > max && rejected.error == Error::Capacity),
    }
}
#[kani::proof]
fn closure_array_constructor_establishes_the_invariant() {
    closure_from_array_case::<0, _>(LengthLimit::new(kani::any()));
    closure_from_array_case::<1, _>(LengthLimit::new(kani::any()));
    closure_from_array_case::<3, _>(LengthLimit::new(kani::any()));
    closure_from_array_case::<{ usize::MAX }, _>(LengthLimit::new(kani::any()));
    closure_from_array_case::<0, _>(Unlimited);
    closure_from_array_case::<3, _>(Unlimited);
    closure_from_array_case::<{ usize::MAX }, _>(Unlimited);
}

fn closure_from_slice_case<L: LengthPolicy>(limit: L) {
    let max = limit.max_len();
    let source = closure_units();
    match Vec::try_from_slice_in(source, Unmetered, limit) {
        Ok(owner) => {
            assert!(owner.len() == source.len());
            closure_exit(&owner, max);
        }
        Err(error) => assert!(source.len() > max && error == Error::Capacity),
    }
}
#[kani::proof]
fn closure_slice_constructor_establishes_the_invariant() {
    closure_from_slice_case(LengthLimit::new(kani::any()));
    closure_from_slice_case(Unlimited);
}

#[kani::proof]
fn closure_adopting_constructors_establish_the_invariant_for_unit_storage() {
    let length: usize = kani::any();
    let limit: usize = kani::any();
    let mut source = alloc::vec::Vec::<()>::new();
    // SAFETY: every unit is valid without bytes; std's unit capacity is usize::MAX.
    unsafe {
        source.set_len(length);
    }
    match Vec::try_from_std_in(source, Unmetered, LengthLimit::new(limit)) {
        Ok(owner) => {
            assert!(owner.len() == length);
            closure_exit(&owner, limit);
        }
        Err(rejected) => assert!(length > limit && rejected.value.len() == length),
    }
    // SAFETY: aligned zero-byte global storage, valid unit values, zero charge.
    let source = unsafe {
        crate::boxed::Box::from_raw_parts(
            NonNull::slice_from_raw_parts(NonNull::<()>::dangling(), length),
            Unmetered,
        )
    };
    let owner = Vec::from_boxed_slice(source);
    assert!(owner.len() == length);
    closure_exit(&owner, usize::MAX);
}

fn closure_insert_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let result = owner.try_insert(kani::any(), ());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { len + 1 } else { len });
}
#[kani::proof]
fn closure_insert_preserves_the_invariant() {
    closure_insert_case(LengthLimit::new(kani::any()));
    closure_insert_case(Unlimited);
}

fn closure_pop_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let result = owner.pop();
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_some() { len - 1 } else { len });
}
#[kani::proof]
fn closure_pop_preserves_the_invariant() {
    closure_pop_case(LengthLimit::new(kani::any()));
    closure_pop_case(Unlimited);
}

fn closure_remove_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let result = owner.remove(kani::any());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { len - 1 } else { len });
}
#[kani::proof]
fn closure_remove_preserves_the_invariant() {
    closure_remove_case(LengthLimit::new(kani::any()));
    closure_remove_case(Unlimited);
}

fn closure_swap_remove_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let result = owner.swap_remove(kani::any());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { len - 1 } else { len });
}
#[kani::proof]
fn closure_swap_remove_preserves_the_invariant() {
    closure_swap_remove_case(LengthLimit::new(kani::any()));
    closure_swap_remove_case(Unlimited);
}

fn closure_truncate_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let requested: usize = kani::any();
    owner.truncate(requested);
    closure_exit(&owner, max);
    assert!(owner.len() == len.min(requested));
}
#[kani::proof]
fn closure_truncate_preserves_the_invariant() {
    closure_truncate_case(LengthLimit::new(kani::any()));
    closure_truncate_case(Unlimited);
}

fn closure_push_within_capacity_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let pushed = owner.push_within_capacity(()).is_ok();
    closure_exit(&owner, max);
    assert!(owner.capacity() == cap);
    assert!(owner.len() == if pushed { len + 1 } else { len });
}
#[kani::proof]
fn closure_push_within_capacity_preserves_the_invariant() {
    closure_push_within_capacity_case(LengthLimit::new(kani::any()));
    closure_push_within_capacity_case(Unlimited);
}

fn closure_set_len_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let requested: usize = kani::any();
    // SAFETY: units require no bytes, resources or per-element initialization.
    let result = unsafe { owner.try_set_len(requested) };
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { requested } else { len });
}
#[kani::proof]
fn closure_set_len_preserves_the_invariant() {
    closure_set_len_case(LengthLimit::new(kani::any()));
    closure_set_len_case(Unlimited);
}

fn closure_extend_copy_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let source = closure_units();
    let result = owner.try_extend_copy(source);
    closure_exit(&owner, max);
    match result {
        Ok(()) => assert!(len.checked_add(source.len()) == Some(owner.len())),
        Err(_) => assert!(owner.len() == len),
    }
}
#[kani::proof]
fn closure_extend_copy_preserves_the_invariant() {
    closure_extend_copy_case(LengthLimit::new(kani::any()));
    closure_extend_copy_case(Unlimited);
}

fn closure_append_case<L: LengthPolicy, M: LengthPolicy>(limit: L, other_limit: M) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let mut other = closure_owner::<(), _, _>(Unmetered, other_limit);
    let (len, max) = (owner.len(), owner.max_len());
    let (other_len, other_max) = (other.len(), other.max_len());
    let result = owner.try_append(&mut other);
    closure_exit(&owner, max);
    closure_exit(&other, other_max);
    match result {
        Ok(()) => assert!(len.checked_add(other_len) == Some(owner.len()) && other.len() == 0),
        Err(_) => assert!(owner.len() == len && other.len() == other_len),
    }
}
#[kani::proof]
fn closure_append_preserves_the_invariant_of_both_owners() {
    closure_append_case(LengthLimit::new(kani::any()), LengthLimit::new(kani::any()));
    closure_append_case(LengthLimit::new(kani::any()), Unlimited);
    closure_append_case(Unlimited, LengthLimit::new(kani::any()));
    closure_append_case(Unlimited, Unlimited);
}

fn closure_exchange_case<L: LengthPolicy>(limit: L, other_limit: L) {
    // One metered domain: the only policies under which the owners exchange.
    let budget = crate::budget::MemoryBudget::new(kani::any());
    let mut owner = closure_owner::<(), _, _>(Budget::Limited(&budget), limit);
    let mut other = closure_owner::<(), _, _>(Budget::Limited(&budget), other_limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let (other_len, other_cap, other_max) = (other.len(), other.capacity(), other.max_len());
    let result = owner.try_reuse_larger_buffer(&mut other);
    closure_exit(&owner, max);
    closure_exit(&other, other_max);
    match result {
        Ok(true) => {
            assert!(owner.len() == other_len && other.len() == len);
            assert!(other.capacity() == cap && owner.capacity() >= other_cap);
        }
        _ => {
            assert!(owner.len() == len && other.len() == other_len);
            assert!(owner.capacity() == cap && other.capacity() == other_cap);
        }
    }
    kani::cover!(result == Ok(true));
    kani::cover!(result == Ok(true) && owner.capacity() != other_cap);
}
#[kani::proof]
fn closure_buffer_exchange_preserves_the_invariant_of_both_owners() {
    closure_exchange_case(LengthLimit::new(kani::any()), LengthLimit::new(kani::any()));
}
#[kani::proof]
fn closure_buffer_exchange_preserves_the_invariant_of_both_unlimited_owners() {
    closure_exchange_case(Unlimited, Unlimited);
}

fn closure_split_off_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let at: usize = kani::any();
    match owner.try_split_off(at) {
        Ok(tail) => {
            assert!(at <= len && owner.len() == at && tail.len() == len - at);
            closure_exit(&tail, max);
        }
        Err(error) => assert!(at > len && error == Error::Bounds && owner.len() == len),
    }
    closure_exit(&owner, max);
}
#[kani::proof]
fn closure_split_off_preserves_the_invariant_of_both_owners() {
    closure_split_off_case(LengthLimit::new(kani::any()));
    closure_split_off_case(Unlimited);
}

fn closure_clone_case<L: LengthPolicy>(limit: L) {
    let owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let Ok(copy) = owner.try_clone() else {
        unreachable!()
    };
    assert!(copy.len() == len && owner.len() == len);
    closure_exit(&copy, max);
    closure_exit(&owner, max);
}
#[kani::proof]
fn closure_clone_establishes_the_invariant_of_the_copy() {
    closure_clone_case(LengthLimit::new(kani::any()));
    closure_clone_case(Unlimited);
}

fn closure_extend_copy_from_within_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let result = owner.try_extend_copy_from_within((closure_bound(), closure_bound()));
    closure_exit(&owner, max);
    match result {
        Ok(()) => assert!(len <= owner.len() && owner.len() - len <= len),
        Err(_) => assert!(owner.len() == len),
    }
}
#[kani::proof]
fn closure_extend_copy_from_within_preserves_the_invariant() {
    closure_extend_copy_from_within_case(LengthLimit::new(kani::any()));
    closure_extend_copy_from_within_case(Unlimited);
}

fn closure_into_array_case<const N: usize, L: LengthPolicy>(limit: L) {
    let owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    match owner.try_into_array::<N>() {
        // The owner was emptied and released inside the call.
        Ok(_) => assert!(len == N),
        Err(rejected) => {
            assert!(len != N && rejected.value.len() == len);
            closure_exit(&rejected.value, max);
        }
    }
}
#[kani::proof]
fn closure_into_array_preserves_the_invariant_of_a_returned_owner() {
    closure_into_array_case::<0, _>(LengthLimit::new(kani::any()));
    closure_into_array_case::<1, _>(LengthLimit::new(kani::any()));
    closure_into_array_case::<3, _>(LengthLimit::new(kani::any()));
    closure_into_array_case::<{ usize::MAX }, _>(LengthLimit::new(kani::any()));
    closure_into_array_case::<0, _>(Unlimited);
    closure_into_array_case::<3, _>(Unlimited);
    closure_into_array_case::<{ usize::MAX }, _>(Unlimited);
}

fn closure_flatten_case<const N: usize, L: LengthPolicy>(limit: L) {
    let owner = closure_owner::<[(); N], _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    match owner.try_into_flattened() {
        Ok(elements) => {
            assert!(elements.len() as u128 == len as u128 * N as u128);
            closure_exit(&elements, max);
        }
        Err(rejected) => {
            assert!(rejected.value.len() == len);
            closure_exit(&rejected.value, max);
        }
    }
}
#[kani::proof]
fn closure_flatten_establishes_the_invariant_of_the_element_owner() {
    closure_flatten_case::<0, _>(LengthLimit::new(kani::any()));
    closure_flatten_case::<1, _>(LengthLimit::new(kani::any()));
    closure_flatten_case::<3, _>(LengthLimit::new(kani::any()));
    closure_flatten_case::<{ usize::MAX }, _>(LengthLimit::new(kani::any()));
    closure_flatten_case::<0, _>(Unlimited);
    closure_flatten_case::<3, _>(Unlimited);
    closure_flatten_case::<{ usize::MAX }, _>(Unlimited);
}
