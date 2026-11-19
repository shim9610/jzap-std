// ZST storage permits every machine-word length without allocating or assuming
// an unrealistically large live allocation. These exercise the real counter
// updates, not copies of their expressions. Pointer/element-size proofs are
// separate obligations; this is not a proof for arbitrary user callbacks.
#[cfg(feature = "alloc")]
#[kani::proof]
fn box_iterator_counters_cover_every_machine_word_length() {
    use core::ptr::NonNull;
    let len: usize = kani::any();
    let ptr = NonNull::slice_from_raw_parts(NonNull::<()>::dangling(), len);
    // SAFETY: unit values require no bytes, are initialized, and have no charge.
    let owner = unsafe { crate::boxed::Box::from_raw(ptr.as_ptr()) };
    let mut it = owner.into_iter();
    assert!(it.len() == len);
    assert!(it.next().is_some() == (len != 0));
    let remaining = if len == 0 { 0 } else { len - 1 };
    assert!(it.len() == remaining);
    assert!(it.next_back().is_some() == (remaining != 0));
    assert!(it.len() <= len);
    drop(it);
}

#[cfg(feature = "alloc")]
#[cfg(jzap_loop_contracts)]
#[kani::proof]
#[kani::solver(z3)]
fn vec_retain_all_lengths_with_loop_contracts() {
    use crate::{
        budget::Unmetered,
        vec::{Unlimited, Vec},
    };
    let len: usize = kani::any();
    // SAFETY: all unit elements are initialized; no allocation or budget charge.
    let mut v = unsafe {
        Vec::<()>::from_raw_parts(
            core::ptr::NonNull::dangling(),
            len,
            usize::MAX,
            Unmetered,
            Unlimited,
        )
    };
    let _: Result<(), ()> = v.try_retain_mut(|_| {
        if kani::any() {
            Err(())
        } else {
            Ok(kani::any())
        }
    });
    assert!(v.len() <= len);
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::solver(z3)]
fn vec_single_edits_cover_every_machine_word_length() {
    use crate::{
        budget::Unmetered,
        vec::{LengthLimit, Vec},
    };
    let len: usize = kani::any();
    let limit: usize = kani::any();
    let index: usize = kani::any();
    kani::assume(len <= limit);
    // SAFETY: valid ZST storage with the configured capacity/limit.
    let mut v = unsafe {
        Vec::<(), _, _>::from_raw_parts(
            core::ptr::NonNull::dangling(),
            len,
            limit,
            Unmetered,
            LengthLimit::new(limit),
        )
    };
    match kani::any::<u8>() {
        0 => {
            let r = v.try_push(());
            assert!(r.is_ok() == (len < limit));
        }
        1 => {
            let r = v.try_insert(index, ());
            assert!(r.is_ok() == (index <= len && len < limit));
        }
        2 => {
            let r = v.remove(index);
            assert!(r.is_ok() == (index < len));
        }
        3 => {
            let r = v.swap_remove(index);
            assert!(r.is_ok() == (index < len));
        }
        4 => {
            let r = v.pop();
            assert!(r.is_some() == (len != 0));
        }
        5 => {
            v.truncate(index);
            assert!(v.len() == len.min(index));
        }
        _ => {
            let r = v.push_within_capacity(());
            assert!(r.is_ok() == (len < limit));
        }
    }
    assert!(v.len() <= limit && v.capacity() == limit);
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::solver(z3)]
fn vec_bulk_copy_covers_every_machine_word_length() {
    use crate::{
        budget::Unmetered,
        vec::{LengthLimit, Vec},
    };
    let len: usize = kani::any();
    let additional: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(len <= limit);
    // SAFETY: both the owner and borrowed units have valid zero-byte storage.
    let mut v = unsafe {
        Vec::<(), _, _>::from_raw_parts(
            core::ptr::NonNull::dangling(),
            len,
            limit,
            Unmetered,
            LengthLimit::new(limit),
        )
    };
    let source = unsafe {
        core::slice::from_raw_parts(core::ptr::NonNull::<()>::dangling().as_ptr(), additional)
    };
    let expected = len.checked_add(additional).filter(|n| *n <= limit);
    let r = v.try_extend_copy(source);
    assert!(r.is_ok() == expected.is_some());
    assert!(v.len() == expected.unwrap_or(len));
    assert!(v.len() <= limit);
}

#[cfg(all(feature = "alloc", jzap_loop_contracts))]
#[inline(always)]
fn nondeterministic_initialize(
    _: usize,
    slot: &mut crate::init::InitSlot<'_, ()>,
) -> Result<(), ()> {
    if kani::any() {
        return Err(());
    }
    if kani::any() {
        let _ = slot.write(());
    }
    Ok(())
}

#[cfg(all(feature = "alloc", jzap_loop_contracts))]
#[kani::proof]
#[kani::solver(z3)]
fn box_slice_initialization_covers_every_length_and_callback_result() {
    let len: usize = kani::any();
    let r = crate::boxed::Box::<[()]>::try_init_slice(len, nondeterministic_initialize);
    if let Ok(owner) = r {
        assert!(owner.len() == len);
    }
}

#[cfg(all(feature = "alloc", jzap_loop_contracts))]
#[kani::proof]
#[kani::solver(z3)]
fn vec_resize_covers_every_length_and_callback_result() {
    use crate::{
        budget::Unmetered,
        vec::{LengthLimit, Vec},
    };
    let len: usize = kani::any();
    let new_len: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(len <= limit);
    let mut backing = 0u8;
    // SAFETY: all units are initialized in valid zero-byte storage.
    let mut v = unsafe {
        Vec::<(), _, _>::from_raw_parts(
            core::ptr::NonNull::from(&mut backing).cast(),
            len,
            limit,
            Unmetered,
            LengthLimit::new(limit),
        )
    };
    let r = v.try_resize_with_init(new_len, nondeterministic_initialize);
    assert!(v.len() <= limit);
    assert!(v.len() == if r.is_ok() { new_len } else { len });
}

// Selected only by the verification driver, which requires semantic failures.
#[kani::proof]
fn negative_panic_is_detected() {
    if kani::any() {
        panic!("deliberate panic proof control");
    }
}

#[kani::proof]
fn negative_overflow_is_detected() {
    let value: usize = kani::any();
    let _ = value + 1;
}

#[kani::proof]
fn negative_initializer_panic_is_detected() {
    let _ = crate::boxed::Box::<[()]>::try_init_slice::<()>(1, |_, _| {
        panic!("deliberate initializer proof control");
    });
}

#[kani::proof]
fn native_overflow_flag_matches_checked_add() {
    macro_rules! check {
        ($ty:ty) => {{
            let a: $ty = kani::any();
            let b: $ty = kani::any();
            let expected = a.checked_add(b);
            let (value, overflow) = a.overflowing_add(b);
            assert!(overflow == expected.is_none());
            if let Some(sum) = expected {
                assert!(value == sum);
            }
        }};
    }
    check!(u32);
    check!(u64);
    check!(usize);
}

#[kani::proof]
fn native_abs_diff_matches_ordered_subtraction() {
    macro_rules! check {
        ($ty:ty) => {{
            let a: $ty = kani::any();
            let b: $ty = kani::any();
            let expected = if a <= b { b - a } else { a - b };
            assert!(a.abs_diff(b) == expected);
        }};
    }
    check!(u32);
    check!(u64);
    check!(usize);
}

#[kani::proof]
fn native_power_of_two_matches_bit_mask() {
    macro_rules! check {
        ($ty:ty) => {{
            let value: $ty = kani::any();
            assert!(value.is_power_of_two() == (value != 0 && (value & (value - 1)) == 0));
        }};
    }
    check!(u32);
    check!(u64);
    check!(usize);
}

#[kani::proof]
#[kani::solver(z3)]
fn layout_array_matches_std_for_every_count() {
    #[repr(align(64))]
    struct Aligned([u8; 64]);
    #[repr(align(64))]
    struct AlignedUnit;
    let count: usize = kani::any();
    macro_rules! check {
        ($ty:ty) => {
            assert!(
                crate::layout::array::<$ty>(count)
                    == core::alloc::Layout::array::<$ty>(count).map_err(|_| crate::Error::Layout)
            );
        };
    }
    check!(u8);
    check!(u64);
    check!([u8; 3]);
    check!(Aligned);
    check!(());
    check!(AlignedUnit);
}
