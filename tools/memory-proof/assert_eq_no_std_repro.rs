#![no_std]
mod proofs {
#[kani::proof]
fn equality_with_formatting() {
    let x: u8 = kani::any();
    let y: u8 = kani::any();
    kani::assume(x == y);
    assert_eq!(x, y);
}
}
