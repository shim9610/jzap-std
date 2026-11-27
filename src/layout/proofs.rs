//! Actual Layout construction must accept exactly core's full count domain.
use super::*;
#[kani::proof]
fn original_layout_matches_core_for_every_count() {
    let count: usize = kani::any();
    macro_rules! one {
        ($ty:ty) => {
            match (Layout::array::<$ty>(count), array::<$ty>(count)) {
                (Ok(expected), Ok(actual)) => assert!(actual == expected),
                (Err(_), Err(Error::Layout)) => {}
                _ => assert!(false),
            }
        };
    }
    one!(u8);
    one!(u64);
    one!([u8; 3]);
    one!([u64; 3]);
    one!(());
    #[repr(align(128))]
    struct Aligned(u8);
    #[repr(align(128))]
    struct AlignedUnit;
    one!(Aligned);
    one!(AlignedUnit);
}

// Second instantiation (issue #16, unit 1 step 6): `Capacity::for_slice`.
// A zero-sized element takes every slice length; a sized element takes the
// slices of a real three-element array, because a longer slice needs storage.
#[kani::proof]
fn closure_slice_capacity_matches_the_slice_layout() {
    use crate::capacity::Capacity;
    let limit: usize = kani::any();
    let length: usize = kani::any();
    // SAFETY: zero-sized values need no storage.
    let units = unsafe {
        core::slice::from_raw_parts(core::ptr::NonNull::<()>::dangling().as_ptr(), length)
    };
    let capacity = Capacity::for_slice(units, limit);
    assert!(capacity.get() == limit && capacity.layout().size() == 0);
    let arrays = unsafe {
        core::slice::from_raw_parts(core::ptr::NonNull::<[(); 3]>::dangling().as_ptr(), length)
    };
    let capacity = Capacity::for_slice(arrays, limit);
    assert!(capacity.get() == limit && capacity.layout().size() == 0);
    kani::assume(length <= 3);
    macro_rules! one {
        ($ty:ty, $values:expr) => {{
            let values: [$ty; 3] = $values;
            let capacity = Capacity::for_slice(&values[..length], limit);
            assert!(capacity.get() == length);
            assert!(Ok(capacity.layout()) == array::<$ty>(length));
            assert!(capacity.layout() == Layout::for_value(&values[..length]));
        }};
    }
    one!(u8, [1, 2, 3]);
    one!(u64, [1, 2, 3]);
    one!([u64; 200], [[0; 200]; 3]);
}
