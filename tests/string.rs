use jzap::{
    Error, TryClone, TryCloneIn, TryIterator,
    budget::{Budget, MemoryBudget, Unmetered},
    error::InitError,
    string::{FormatError, String as S, Utf8Lossy},
    vec::{LengthLimit, Unlimited, Vec as V},
};
use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};
mod common;
#[global_allocator]
static ALLOC: common::FaultAllocator = common::FaultAllocator;

#[test]
fn constructors_limits_and_layout() {
    for ch in ['a', 'é', '한', '😀'] {
        let width = ch.len_utf8();
        let budget = MemoryBudget::new(width);
        let mut exact =
            S::try_with_capacity_and_limit_in(width, width, Budget::Limited(&budget)).unwrap();
        let (result, stats) = common::track(1, || exact.try_push(ch));
        result.unwrap();
        assert_eq!(stats.calls, 0);
        assert_eq!(exact.capacity(), width);
        assert_eq!(exact.pop(), Some(ch));
        drop(exact);
        assert_eq!(budget.occupied(), 0);
    }
    assert_eq!(std::mem::size_of::<S>(), std::mem::size_of::<String>());
    assert_eq!(
        std::mem::size_of::<S<Unmetered, LengthLimit>>(),
        std::mem::size_of::<S>() + std::mem::size_of::<usize>()
    );
    let (mut s, stats) = common::track(1, || S::with_limit(7));
    assert_eq!(stats.calls, 0);
    s.try_push_str("é😀").unwrap();
    assert_eq!(s.len(), 6);
    assert!(s.capacity() <= 7);
    assert_eq!(s.try_push('é'), Err(Error::Capacity));
    s.try_push('a').unwrap();
    assert_eq!(s.pop(), Some('a'));
    assert_eq!(s.pop(), Some('😀'));
    s.try_push_str("12345").unwrap();
    assert_eq!(s.as_str(), "é12345");
    s.clear();
    s.try_push('😀').unwrap();
    assert_eq!(s.try_reserve(4), Err(Error::Capacity));
    assert_eq!(s.try_reserve_exact(usize::MAX), Err(Error::Capacity));
    assert_eq!(
        S::try_with_capacity_and_limit(8, 7).unwrap_err(),
        Error::Capacity
    );
}
#[test]
fn utf8_utf16_and_lossy_match_std() {
    for bytes in [
        &b""[..],
        &b"hello"[..],
        &[0xff, 0xe2, 0x82],
        &[0xe0, 0x80, 0x80, b'a'],
        "한😀".as_bytes(),
    ] {
        let result = S::try_from_utf8_lossy(bytes).unwrap();
        assert_eq!(result.as_str(), String::from_utf8_lossy(bytes));
        let input = V::try_from_slice(bytes).unwrap();
        match S::try_from_utf8(input) {
            Ok(s) => assert_eq!(s.as_str(), std::str::from_utf8(bytes).unwrap()),
            Err(e) => {
                assert_eq!(e.value.as_slice(), bytes);
                assert_eq!(e.error, std::str::from_utf8(bytes).unwrap_err());
            }
        }
    }
    let (lossy, stats) = common::track(1, || S::try_from_utf8_lossy(b"valid"));
    assert!(matches!(lossy.unwrap(), Utf8Lossy::Borrowed("valid")));
    assert_eq!(stats.calls, 0);
    for units in [
        &[][..],
        &[0, 0xd800, 0xdc00, 0xffff],
        &[0xd800],
        &[0xdc00, 0xd800, 65],
    ] {
        assert_eq!(
            S::try_from_utf16_lossy(units).unwrap().as_str(),
            String::from_utf16_lossy(units)
        );
        assert_eq!(
            S::try_from_utf16(units).is_ok(),
            String::from_utf16(units).is_ok()
        );
    }
}
#[test]
fn edits_and_ranges_match_std() {
    let mut s = S::try_from_str("aé한😀z").unwrap();
    let original = s.as_str().to_owned();
    for at in [2, 4, 7, 9] {
        assert_eq!(s.get(..at), Err(Error::CharBoundary));
        assert_eq!(s.get_mut(..at), Err(Error::CharBoundary));
        assert_eq!(s.try_insert_str(at, "x"), Err(Error::CharBoundary));
        assert_eq!(s.remove(at), Err(Error::CharBoundary));
        assert_eq!(s.truncate(at), Err(Error::CharBoundary));
        assert_eq!(s.try_split_off(at).unwrap_err(), Error::CharBoundary);
        assert_eq!(s.try_drain(at..at).unwrap_err(), Error::CharBoundary);
        assert_eq!(s.as_str(), original);
    }
    assert_eq!(s.try_replace_range(0..=usize::MAX, "x"), Err(Error::Bounds));
    assert_eq!(s.try_insert_str(usize::MAX, "x"), Err(Error::Bounds));
    assert_eq!(s.remove(s.len()), Err(Error::Bounds));
    s.truncate(usize::MAX).unwrap();
    s.try_replace_range(1..6, "12345678").unwrap();
    let mut model = original;
    model.replace_range(1..6, "12345678");
    assert_eq!(s.as_str(), model);
    s.try_replace_range(1..9, "é").unwrap();
    model.replace_range(1..9, "é");
    s.try_insert(3, '한').unwrap();
    model.insert(3, '한');
    assert_eq!(s.remove(1).unwrap(), model.remove(1));
    s.try_extend_from_within(1..4).unwrap();
    model.extend_from_within(1..4);
    assert_eq!(s.as_str(), model);
    let tail = s.try_split_off(4).unwrap();
    let mtail = model.split_off(4);
    assert_eq!(tail.as_str(), mtail);
    assert_eq!(s.as_str(), model);
}
#[test]
fn drain_partial_reverse_and_forget() {
    let mut s = S::try_from_str("aé한😀z").unwrap();
    {
        let mut drain = s.try_drain(1..10).unwrap();
        assert_eq!(drain.try_next().unwrap(), Some('é'));
        assert_eq!(drain.next_back(), Some('😀'));
        assert_eq!(drain.as_str(), "한");
    }
    assert_eq!(s.as_str(), "az");
    let mut s = S::try_from_str("aé한😀z").unwrap();
    let mut drain = s.try_drain(1..10).unwrap();
    assert_eq!(drain.next(), Some('é'));
    std::mem::forget(drain);
    assert_eq!(s.as_str(), "aé한😀z");
}
#[test]
fn retain_error_and_unwind_restore_utf8() {
    let mut s = S::try_from_str("aé한😀z").unwrap();
    assert_eq!(
        s.try_retain(|ch| if ch == '😀' { Err(9) } else { Ok(ch != 'é') }),
        Err(9)
    );
    assert_eq!(s.as_str(), "a한😀z");
    let result = catch_unwind(AssertUnwindSafe(|| {
        s.retain(|ch| {
            if ch == '😀' {
                panic!("callback");
            }
            ch != '한'
        })
    }));
    assert!(result.is_err());
    assert_eq!(s.as_str(), "a😀z");
    s.retain(|_| false);
    assert!(s.is_empty());
}
#[test]
fn all_allocation_refusals_preserve_or_clean() {
    // Each edit performs exactly one fallible growth allocation; rejected
    // operations leave pointer, capacity and contents unchanged.
    let edits: &[fn(&mut S) -> Result<(), Error>] = &[
        |s| s.try_push_str("longer UTF8 한글😀"),
        |s| s.try_insert_str(1, "longer UTF8 한글😀"),
        |s| s.try_replace_range(1..3, "longer UTF8 한글😀"),
        |s| s.try_extend_from_within(..),
        |s| s.try_reserve(80),
        |s| s.try_reserve_exact(80),
    ];
    for edit in edits {
        let mut s = S::try_from_str("aé").unwrap();
        let pointer = s.as_ptr();
        let capacity = s.capacity();
        let (result, stats) = common::track(1, || edit(&mut s));
        assert!(matches!(result, Err(Error::Allocation { .. })));
        assert_eq!(stats.calls, 1);
        assert_eq!(
            (s.as_str(), s.as_ptr(), s.capacity()),
            ("aé", pointer, capacity)
        );
    }
    for fail in 1..=8 {
        let (result, stats) = common::track(fail, || {
            S::try_from_chars("aé한😀aé한😀aé한😀".chars()).map(drop)
        });
        if result.is_err() {
            assert_eq!(stats.calls, fail);
        }
        assert_eq!(stats.live, 0);
    }
    let mut s = S::try_with_capacity(80).unwrap();
    s.try_push_str("aé").unwrap();
    let pointer = s.as_ptr();
    assert!(common::track(1, || s.try_shrink_to_fit()).0.is_err());
    assert_eq!(s.as_ptr(), pointer);
    assert_eq!(s.as_str(), "aé");
    assert!(common::track(1, || s.try_split_off(1)).0.is_err());
    assert_eq!(s.as_str(), "aé");
}
#[test]
fn budget_peak_target_clone_and_transfers() {
    let budget = MemoryBudget::new(32);
    let target = MemoryBudget::new(32);
    let p = Budget::Limited(&budget);
    let mut s = S::try_from_str_in("aé", p, Unlimited).unwrap();
    assert_eq!(budget.occupied(), 3);
    let copy = s.try_clone_in(Budget::Limited(&target)).unwrap();
    assert_eq!(copy.as_str(), "aé");
    assert_eq!(target.occupied(), 3);
    assert!(s.try_reserve(31).is_err());
    assert_eq!(s.as_str(), "aé");
    assert_eq!(budget.reserved(), 0);
    let (ptr, len, cap, p, l) = s.into_raw_parts();
    let s = unsafe { S::from_raw_parts(ptr, len, cap, p, l) };
    let boxed = s.try_into_boxed_str().unwrap();
    assert_eq!(boxed.as_str(), "aé");
    let mut s = S::from_boxed_str(boxed);
    s.as_mut_str().make_ascii_uppercase();
    assert_eq!(s.as_str(), "Aé");
    let standard = s.into_std();
    assert_eq!(budget.occupied(), 0);
    assert_eq!(standard, "Aé");
    drop(copy);
    assert_eq!(target.occupied(), 0);
    let mut standard = String::with_capacity(20);
    standard.push('a');
    let pointer = standard.as_ptr();
    let rejected = S::try_from_std_in(standard, Unmetered, LengthLimit::new(2)).unwrap_err();
    assert_eq!(rejected.value.as_ptr(), pointer);
    let s = S::try_from_std_in(rejected.value, Unmetered, Unlimited).unwrap();
    assert_eq!(s.as_ptr(), pointer);
}
struct Ignore;
impl fmt::Display for Ignore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = f.write_str("123456789");
        let _ = f.write_str("x");
        Ok(())
    }
}
struct Fails;
impl fmt::Display for Fails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("é")?;
        Err(fmt::Error)
    }
}
#[test]
fn formatting_rollback_sticky_error_and_reuse() {
    let mut s = S::with_limit(8);
    s.try_push_str("hi").unwrap();
    assert_eq!(
        s.try_write_fmt(format_args!("{Ignore}")),
        Err(FormatError::Storage(Error::Capacity))
    );
    assert_eq!(s.as_str(), "hi");
    assert_eq!(
        s.try_write_fmt(format_args!("{Fails}")),
        Err(FormatError::Formatter)
    );
    assert_eq!(s.as_str(), "hi");
    s.try_write_fmt(format_args!("{}{}", '한', 42)).unwrap();
    assert_eq!(s.as_str(), "hi한42");
    let mut s = S::try_from_str("a").unwrap();
    let result = common::track(1, || s.try_write_fmt(format_args!("{Ignore}"))).0;
    assert!(matches!(
        result,
        Err(FormatError::Storage(Error::Allocation { .. }))
    ));
    assert_eq!(s.as_str(), "a");
    assert_eq!(
        S::try_format(format_args!("{}:{}", "한", 42))
            .unwrap()
            .as_str(),
        "한:42"
    );
}
#[test]
fn iterator_failure_rolls_back_and_preserves_utf8() {
    let mut s = S::try_from_str("a").unwrap();
    let input = jzap::iter::from_results([Ok('한'), Ok('😀'), Err(7)]);
    assert_eq!(
        s.try_extend_fallible_chars(input),
        Err(InitError::Initialize(7))
    );
    assert_eq!(s.as_str(), "a");
    let input = jzap::iter::from_results([Ok("é"), Err(3)]);
    assert_eq!(
        s.try_extend_fallible_texts(input),
        Err(InitError::Initialize(3))
    );
    assert_eq!(s.as_str(), "a");
    let mut s = S::with_limit(4);
    s.try_push('a').unwrap();
    assert_eq!(s.try_extend_chars(['é', '😀']), Err(Error::Capacity));
    assert_eq!(s.as_str(), "a");
    s.try_extend_texts(["한"]).unwrap();
    assert_eq!(s.as_str(), "a한");
}
#[test]
fn clone_replacement_and_append_preserve_destination_policy() {
    let mut a = S::with_limit(8);
    a.try_push_str("a").unwrap();
    let mut b = S::with_limit(20);
    b.try_push_str("123456789").unwrap();
    assert_eq!(a.try_clone_from(&b), Err(Error::Capacity));
    assert_eq!(a.as_str(), "a");
    assert_eq!(a.try_append(&mut b), Err(Error::Capacity));
    assert_eq!(b.as_str(), "123456789");
    b.truncate(3).unwrap();
    a.try_append(&mut b).unwrap();
    assert_eq!(a.as_str(), "a123");
    assert!(b.is_empty());
    let copy = a.try_clone().unwrap();
    assert_eq!(copy.max_len(), 8);
    a.clear();
    let pointer = a.as_ptr();
    a.try_clone_from(&copy).unwrap();
    assert_eq!(a.as_ptr(), pointer);
}
#[test]
fn formatting_callback_unwind_rolls_back() {
    struct Panics;
    impl fmt::Display for Panics {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("한")?;
            panic!("callback")
        }
    }
    let mut s = S::try_from_str("a").unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || s.try_write_fmt(format_args!("{Panics}"))
        ))
        .is_err()
    );
    assert_eq!(s.as_str(), "a");
}
#[test]
fn leak_recovery_and_safe_borrows() {
    let mut s = S::try_from_str("abcé").unwrap();
    s.get_mut(..3).unwrap().make_ascii_uppercase();
    assert_eq!(s.as_str(), "ABCé");
    let cap = s.capacity();
    let leaked = s.leak();
    let len = leaked.len();
    let ptr = std::ptr::NonNull::new(leaked.as_mut_ptr()).unwrap();
    let recovered = unsafe { S::from_raw_parts(ptr, len, cap, Unmetered, Unlimited) };
    assert_eq!(recovered.as_str(), "ABCé");
    let (_, count) = common::deallocations_during(ptr.as_ptr().addr(), || drop(recovered));
    assert_eq!(count, 1);
}

#[test]
fn remaining_conversions_format_helpers_and_refusals() {
    use jzap::fmt::TryToString;
    use std::borrow::Cow;
    assert_eq!(S::try_from_char('😀').unwrap().as_str(), "😀");
    assert_eq!(42.try_to_string().unwrap().as_str(), "42");
    assert_eq!(
        jzap::fmt::try_format_in(format_args!("{}", "é"), Unmetered, LengthLimit::new(1)),
        Err(FormatError::Storage(Error::Capacity))
    );
    let owned = String::from("한");
    let ptr = owned.as_ptr();
    let adopted = S::try_from_cow(Cow::Owned(owned)).unwrap();
    assert_eq!(adopted.as_ptr(), ptr);
    assert_eq!(S::try_from_cow(Cow::Borrowed("é")).unwrap().as_str(), "é");
    for fail in 1..=6 {
        let (result, stats) = common::track(fail, || {
            S::try_from_utf16_lossy(&[0xd800, 65, 0xdfff, 0xd83d, 0xde00]).map(drop)
        });
        if result.is_err() {
            assert_eq!(stats.calls, fail);
        }
        assert_eq!(stats.live, 0);
        let (result, stats) = common::track(fail, || {
            S::try_from_utf8_lossy(&[0xff, 0xff, 0xff, 0xff]).map(drop)
        });
        if result.is_err() {
            assert_eq!(stats.calls, fail);
        }
        assert_eq!(stats.live, 0);
    }
}

#[test]
fn trait_parity_owned_inputs_and_rejected_box_transfer() {
    use std::{
        borrow::{Borrow, BorrowMut},
        hash::{Hash, Hasher},
    };
    let mut text = S::try_from_str("abcé").unwrap();
    let borrowed: &str = text.borrow();
    assert_eq!(borrowed, "abcé");
    let borrowed: &mut str = text.borrow_mut();
    borrowed.make_ascii_uppercase();
    assert_eq!(text, "ABCé");
    assert_eq!("ABCé", text);
    assert_eq!(String::from("ABCé"), text);
    assert_eq!(text, String::from("ABCé"));
    assert!(text < S::try_from_str("Z").unwrap());
    let mut a = std::collections::hash_map::DefaultHasher::new();
    let mut b = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut a);
    "ABCé".hash(&mut b);
    assert_eq!(a.finish(), b.finish());
    assert_eq!(format!("{text:?}/{text}"), "\"ABCé\"/ABCé");
    let copy = S::try_from_fallible_chars(jzap::iter::from_results([Ok::<_, ()>('한')])).unwrap();
    assert_eq!(copy.as_str(), "한");
    assert_eq!(S::try_from_chars(['é']).unwrap().as_str(), "é");
    assert_eq!(S::try_from_texts(["é", "한"]).unwrap().as_str(), "é한");
    let repaired = S::try_from_utf8_lossy(&[0xff]).unwrap();
    assert_eq!(
        repaired
            .try_to_owned_in(Unmetered, Unlimited)
            .unwrap()
            .as_str(),
        "�"
    );
    let bytes = V::try_from_slice(b"abc").unwrap();
    let mut text = unsafe { S::from_utf8_unchecked(bytes) };
    unsafe {
        text.as_mut_vec().as_mut_slice().make_ascii_uppercase();
    }
    assert_eq!(text.into_bytes().as_slice(), b"ABC");
    let mut text = S::try_with_capacity(80).unwrap();
    text.try_push_str("é").unwrap();
    let pointer = text.as_ptr();
    let rejected = common::track(1, || text.try_into_boxed_str())
        .0
        .unwrap_err();
    assert_eq!(rejected.value.as_str(), "é");
    assert_eq!(rejected.value.as_ptr(), pointer);
    assert_eq!(FormatError::Formatter.to_string(), "formatter failed");
    assert_eq!(
        jzap::string::Utf16Error::UnpairedSurrogate(0xd800).to_string(),
        "unpaired UTF-16 surrogate"
    );
}

#[test]
fn drain_thread_bounds_and_backwards_fallible_iteration() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<jzap::string::Drain<'static>>();
    let mut text = S::try_from_str("aé한😀z").unwrap();
    let mut drain = text.try_drain(1..10).unwrap();
    assert_eq!(
        jzap::TryDoubleEndedIterator::try_next_back(&mut drain).unwrap(),
        Some('😀')
    );
    assert_eq!(drain.size_hint(), "é한".chars().size_hint());
    drop(drain);
    assert_eq!(text.as_str(), "az");
}

// Kani uses its bundled core; this exhaustively checks the public writer against
// the installed stable compiler's encoder too. Miri covers the separate pointer
// scenarios; repeating over a million scalars there adds no new memory shape.
#[cfg(not(miri))]
#[test]
fn every_unicode_scalar_matches_stable_std_with_both_policies() {
    let budget = MemoryBudget::new(4);
    let mut plain = S::try_with_capacity(4).unwrap();
    let mut limited = S::try_with_capacity_and_limit_in(4, 4, Budget::Limited(&budget)).unwrap();
    let mut scalars = 0;
    for value in 0..=0x10ffff {
        let Some(ch) = char::from_u32(value) else {
            continue;
        };
        let mut bytes = [0; 4];
        let reference = ch.encode_utf8(&mut bytes);
        plain.clear();
        limited.clear();
        plain.try_push(ch).unwrap();
        limited.try_push(ch).unwrap();
        assert_eq!(plain.as_str(), reference, "U+{value:04X}");
        assert_eq!(limited.as_str(), reference, "U+{value:04X}");
        scalars += 1;
    }
    assert_eq!(scalars, 1_112_064);
    assert_eq!(budget.occupied(), 4);
    drop(limited);
    assert_eq!(budget.occupied(), 0);
}

#[cfg(not(miri))]
mod properties {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]
        #[test]
        fn unicode_edit_state_machine(ops in prop::collection::vec((0u8..9, any::<u16>(), any::<char>()), 0..100)) {
            let mut model = String::new(); let mut ours = S::new();
            for (op, raw, ch) in ops {
                let bounds: std::vec::Vec<_> = model.char_indices().map(|(i,_)|i).chain([model.len()]).collect();
                let at = bounds[raw as usize % bounds.len()];
                match op {
                    0 => { model.push(ch); ours.try_push(ch).unwrap(); }
                    1 => { model.insert(at,ch); ours.try_insert(at,ch).unwrap(); }
                    2 if at < model.len() => { prop_assert_eq!(model.remove(at), ours.remove(at).unwrap()); }
                    3 => { model.truncate(at); ours.truncate(at).unwrap(); }
                    4 => { prop_assert_eq!(model.pop(), ours.pop()); }
                    5 => { let text = ch.to_string(); model.replace_range(at.., &text); ours.try_replace_range(at..,&text).unwrap(); }
                    6 => { model.retain(|c| c != ch); ours.retain(|c| c != ch); }
                    7 => { model.extend_from_within(at..); ours.try_extend_from_within(at..).unwrap(); }
                    _ => { let tail = model.split_off(at); let mut otail = ours.try_split_off(at).unwrap(); prop_assert_eq!(tail.as_str(),otail.as_str()); model.push_str(&tail); ours.try_append(&mut otail).unwrap(); }
                }
                prop_assert_eq!(model.as_str(),ours.as_str());
                prop_assert_eq!(std::str::from_utf8(ours.as_bytes()).unwrap(),ours.as_str());
            }
        }
        #[test]
        fn lossy_bytes_match_std(bytes in prop::collection::vec(any::<u8>(),0..200)) {
            let repaired = S::try_from_utf8_lossy(&bytes).unwrap();
            prop_assert_eq!(repaired.as_str(), String::from_utf8_lossy(&bytes));
        }
    }
}
