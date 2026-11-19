use core::ptr::NonNull;
use crucible::{Symbolic, crucible_assert};
#[crux::test]
fn zst_offset() {
    let count = usize::symbolic("count");
    let p = NonNull::<()>::dangling().as_ptr();
    let q = unsafe { p.add(count) };
    crucible_assert!(q == p);
}
