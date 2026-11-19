// Deliberately invalid: this must fail under Kani's value-validity checker.
#[kani::proof]
fn invalid_bool() {
    let byte = 2u8;
    let invalid = unsafe { *(&byte as *const u8).cast::<bool>() };
    std::hint::black_box(invalid);
}
