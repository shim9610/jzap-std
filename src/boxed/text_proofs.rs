use super::*;

#[kani::proof]
#[kani::solver(cadical)]
fn character_byte_cursor_preserves_every_valid_state() {
    let position: usize = kani::any();
    let len: usize = kani::any();
    let next: Option<char> = kani::any();
    kani::assume(position <= len && len <= 4);
    let mut bytes = CharBytes {
        iter: crate::iter::from_std(next),
        bytes: kani::any(),
        position,
        len,
    };
    let _ = bytes.try_next();
    assert!(bytes.position <= bytes.len && bytes.len <= 4);
    let (low, high) = bytes.try_size_hint().unwrap();
    assert!(low >= bytes.len - bytes.position);
    assert!(high.is_some());
    assert!(high.unwrap() >= low);
}
