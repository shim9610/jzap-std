// Kani half of the VeriFast proof of `layout::array` (tools/verifast).
//
// tools/verifast/check.py appends this file to the `cfg(kani)` module
// `src/layout/proofs.rs` of an assembled copy of the product crate under
// target/verification/verifast/; the product source is not changed. The
// harnesses check the VeriFast postcondition of `array` on the original
// product function (with `map_err` and its closure), for every `count` and
// four element types:
//
// VeriFast: req thread_token(?t)  (ghost; no Kani form)
//           ens thread_token(t) &*& match result {
//                 Ok(l) => is_valid_layout(count * size_of::<T>(), align_of::<T>()) &*&
//                          l.size() == count * size_of::<T>() &*& l.align() == align_of::<T>(),
//                 Err(e) => <Error>.own(t, e) }
//           on_unwind_ens false   (Kani: a reachable panic fails the harness by default)
// is_valid_layout(size, align) = is_power_of_2(align) && size <= isize::MAX - isize::MAX % align
fn vf_is_valid_layout(size: u128, align: u128) -> bool {
    align != 0 && (align & (align - 1)) == 0 && size <= (isize::MAX as u128) - (isize::MAX as u128) % align
}
macro_rules! vf_array_spec {
    ($name:ident, $ty:ty) => {
        #[kani::proof]
        fn $name() {
            let count: usize = kani::any();
            let result = array::<$ty>(count);
            let size = core::mem::size_of::<$ty>() as u128;
            let align = core::mem::align_of::<$ty>() as u128;
            let bytes = (count as u128) * size; // VeriFast `*` is mathematical
            match result {
                Ok(l) => {
                    assert!(vf_is_valid_layout(bytes, align));
                    assert!(l.size() as u128 == bytes);
                    assert!(l.align() as u128 == align);
                }
                Err(_e) => {} // <Error>.own(t, e): a plain enum, no Kani assertion form
            }
        }
    };
}
#[repr(align(64))]
struct VfAligned64(u8);
vf_array_spec!(vf_spec_array_u8, u8);
vf_array_spec!(vf_spec_array_u64, u64);
vf_array_spec!(vf_spec_array_1600, [u8; 1600]);
vf_array_spec!(vf_spec_array_align64, VfAligned64);
// Negative control: a deliberately wrong postcondition (size == count) must FAIL for u64.
#[kani::proof]
fn vf_negative_array_size_is_count_u64() {
    let count: usize = kani::any();
    if let Ok(l) = array::<u64>(count) {
        assert!(l.size() == count);
    }
}
