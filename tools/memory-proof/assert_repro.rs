#[kani::proof]
fn equality_without_formatting() {
    let x: u8 = kani::any();
    let y: u8 = kani::any();
    kani::assume(x == y);
    assert!(x == y);
}
