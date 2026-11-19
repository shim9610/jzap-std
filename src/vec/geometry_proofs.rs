//! Actual array-owner adapter, with arbitrary valid zero-sized lengths/limits.
use super::Vec;
use crate::Error;

fn adapter<const N: usize>() {
    let limit: usize = kani::any();
    let length: usize = kani::any();
    kani::assume(length <= limit);
    let mut arrays = Vec::<[(); N]>::with_limit(limit);
    // SAFETY: every bit pattern of the zero-sized array is valid and owns no
    // resources. Every logical index is in the admitted ZST capacity.
    assert!(unsafe { arrays.try_set_len(length) }.is_ok());
    let pointer = arrays.as_slice().as_ptr().cast::<()>();
    let mathematical = length as u128 * N as u128;
    match arrays.try_into_flattened() {
        Ok(elements) => {
            assert!(mathematical <= limit as u128);
            assert!(elements.len() as u128 == mathematical);
            assert!(elements.capacity() == limit && elements.max_len() == limit);
            assert!(elements.as_slice().as_ptr() == pointer);
        }
        Err(rejected) => {
            assert!(mathematical > limit as u128);
            assert!(rejected.error == Error::Capacity);
            assert!(rejected.value.len() == length);
            assert!(rejected.value.capacity() == limit && rejected.value.max_len() == limit);
            assert!(rejected.value.as_slice().as_ptr().cast::<()>() == pointer);
        }
    }
}

#[kani::proof]
fn flattened_owner_checks_all_word_lengths_and_preserves_rejection() {
    adapter::<0>();
    adapter::<1>();
    adapter::<3>();
    adapter::<{ usize::MAX }>();
}
