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
