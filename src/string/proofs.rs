use super::*;

#[kani::proof]
fn direct_encoder_matches_core_for_every_scalar() {
    let ch: char = kani::any();
    let initial: [u8; 4] = kani::any();
    let mut expected = initial;
    let mut actual = initial;
    let width = ch.encode_utf8(&mut expected).len();
    let written = unsafe { encoding::write_scalar(ch, actual.as_mut_ptr()) };
    assert_eq!(written, width);
    assert_eq!(actual, expected);
}

#[kani::proof]
fn edit_geometry_all_machine_words() {
    let old: usize = kani::any();
    let start: usize = kani::any();
    let end: usize = kani::any();
    let inserted: usize = kani::any();
    let limit: usize = kani::any();
    let result = edit::Edit::new(old, start, end, inserted, limit);
    let valid_range = start <= end && end <= old;
    let mathematical = if valid_range {
        Some(start as u128 + inserted as u128 + (old - end) as u128)
    } else {
        None
    };
    let fits = mathematical.is_some_and(|n| n <= limit as u128);
    assert_eq!(result.is_ok(), valid_range && fits);
    if let Ok(p) = result {
        assert_eq!(p.start, start);
        assert_eq!(p.end, end);
        assert_eq!(p.tail, old - end);
        assert_eq!(p.destination as u128, start as u128 + inserted as u128);
        assert_eq!(p.len as u128, mathematical.unwrap());
        assert!(p.start <= p.destination && p.destination <= p.len);
        assert!(p.len <= limit);
        let required = old.checked_add(p.additional).unwrap();
        assert!(p.len <= required);
        assert!(p.end.checked_add(p.tail).unwrap() <= required);
        assert!(p.destination.checked_add(p.tail).unwrap() <= required);
    }
}
#[kani::proof]
fn deletion_requires_no_growth_for_every_valid_range() {
    let old: usize = kani::any();
    let start: usize = kani::any();
    let end: usize = kani::any();
    kani::assume(start <= end && end <= old);
    let p = edit::Edit::new(old, start, end, 0, old).unwrap();
    assert_eq!(p.additional, 0);
    assert!(p.len <= old);
}
#[kani::proof]
fn scalar_encoding_fits_four_bytes_and_is_utf8() {
    let ch: char = kani::any();
    let mut buffer = [0u8; 4];
    let text = ch.encode_utf8(&mut buffer);
    assert_eq!(text.len(), ch.len_utf8());
    assert!(text.len() >= 1 && text.len() <= 4);
    assert_eq!(text.chars().next(), Some(ch));
    assert_eq!(text.chars().next_back(), Some(ch));
    let mut cursor = text.chars();
    assert_eq!(cursor.next_back(), Some(ch));
    assert!(cursor.as_str().is_empty());
    if ch.is_ascii() {
        assert_eq!(text.as_bytes().first().copied(), Some(ch as u8));
    }
}
#[kani::proof]
#[kani::unwind(6)]
fn utf16_pair_decoder_returns_only_scalars_or_errors() {
    let units: [u16; 2] = kani::any();
    for ch in core::char::decode_utf16(units) {
        if let Ok(ch) = ch {
            let value = ch as u32;
            assert!(value <= 0x10ffff && !(0xd800..=0xdfff).contains(&value));
        }
    }
}
// Split constant-width heap witnesses so CBMC does not combine allocation and
// four Unicode decoder paths in a single SAT instance. Edit geometry and scalar
// encoding above retain their unrestricted machine-word/Unicode inputs.
fn scalar_roundtrip(ch: char) {
    let mut text = String::try_with_capacity(8).unwrap();
    text.try_push(ch).unwrap();
    assert_eq!(text.pop(), Some(ch));
    text.try_push_str("aé").unwrap();
    text.try_insert(1, ch).unwrap();
    assert_eq!(text.as_str().chars().nth(1), Some(ch));
    assert_eq!(text.remove(1).unwrap(), ch);
    assert_eq!(text.as_str(), "aé");
}
#[kani::proof]
#[kani::unwind(12)]
fn actual_ascii_roundtrip() {
    scalar_roundtrip('a');
}
#[kani::proof]
#[kani::unwind(12)]
fn actual_two_byte_roundtrip() {
    scalar_roundtrip('é');
}
#[kani::proof]
#[kani::unwind(12)]
fn actual_three_byte_roundtrip() {
    scalar_roundtrip('한');
}
#[kani::proof]
#[kani::unwind(12)]
fn actual_four_byte_roundtrip() {
    scalar_roundtrip('😀');
}
