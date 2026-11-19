#[kani::proof]
fn unwrap_formatting() {
    let ok: bool = kani::any();
    let result: Result<u8, u8> = if ok { Ok(7) } else { Err(9) };
    kani::assume(ok);
    assert!(result.unwrap() == 7);
}
