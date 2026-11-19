//! Actual Buffer reservation, including layout and metadata publication at all word states.
use super::*;
#[kani::proof]
fn every_valid_unit_owner_reservation_matches_the_numeric_result() {
    let len: usize = kani::any();
    let cap: usize = kani::any();
    kani::assume(len <= cap);
    let additional: usize = kani::any();
    let hint: usize = kani::any();
    let ptr = NonNull::dangling();
    let layout = Layout::new::<()>();
    let mut owner = Buffer::<(), crate::budget::Unmetered> {
        ptr,
        len: Checked::new(len),
        cap: Checked::new(cap),
        layout,
        policy: crate::budget::Unmetered,
    };
    let expected = crate::boxed::buffer_numeric::reservation(len, cap, additional, hint);
    let actual = owner.reserve(additional, hint);
    assert!(owner.ptr == ptr && owner.len.get() == len && owner.layout == layout);
    match expected {
        Ok((needed, proposed)) => {
            assert!(actual == Ok(needed));
            assert!(owner.cap.get() == proposed);
        }
        Err(error) => {
            assert!(actual == Err(error));
            assert!(owner.cap.get() == cap);
        }
    }
    drop(owner);
}
