//! Real non-ZST allocations: every incoming word includes every wrapping result.
//! Kani's object-bounds checks execute the product's actual loads, stores, moves,
//! reallocations and cleanup. Unbounded numeric extents have separate source-linked
//! proofs; these harnesses do not assume a pointer-access result is safe.
//! Range rejection is proved for every machine word by the shared real validator.
//! Physical range probes use fixed admitted endpoints for concrete memory
//! transitions; the rejection and byte-span theorems cover all machine words.
use crate::{
    Error,
    budget::Unmetered,
    error::InitError,
    vec::{LengthLimit, Vec},
};
use core::ops::Bound;

const DATA: [u8; 2] = [11, 17];
fn owner(len: usize, limit: usize) -> Vec<u8, Unmetered, LengthLimit> {
    let mut owner = Vec::try_from_array_in(DATA, Unmetered, LengthLimit::new(limit)).unwrap();
    owner.truncate(len);
    owner
}
fn arbitrary_len() -> usize {
    let len = kani::any::<u8>() as usize;
    kani::assume(len <= 2);
    len
}
fn bound(word: usize, kind: u8) -> Bound<usize> {
    match kind {
        0 => Bound::Included(word),
        1 => Bound::Excluded(word),
        _ => Bound::Unbounded,
    }
}
fn expected_range(start: usize, end: usize, first: u8, last: u8, len: usize) -> bool {
    // Independent wider arithmetic: inclusive/excluded endpoints cannot wrap.
    let start = match first {
        0 => start as u128,
        1 => start as u128 + 1,
        _ => 0,
    };
    let end = match last {
        0 => end as u128 + 1,
        1 => end as u128,
        _ => len as u128,
    };
    start <= end && end <= len as u128
}
fn unchanged(owner: &Vec<u8, Unmetered, LengthLimit>, len: usize) {
    assert_eq!(owner.len(), len);
    for i in 0..len {
        assert_eq!(*owner.get(i).unwrap(), DATA[i]);
    }
}

#[kani::proof]
#[kani::solver(cadical)]
fn all_word_range_bounds_fail_before_any_memory_effects() {
    let len: usize = kani::any();
    let start: usize = kani::any();
    let end: usize = kani::any();
    let first: u8 = kani::any();
    let last: u8 = kani::any();
    let result = crate::vec::edit::checked_range((bound(start, first), bound(end, last)), len);
    assert_eq!(result.is_ok(), expected_range(start, end, first, last, len));
    if let Ok((start, end)) = result {
        assert!(start <= end && end <= len);
    }
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn every_wrapping_result_is_checked_before_slice_range_access() {
    let len = arbitrary_len();
    let start: usize = kani::any();
    let end: usize = kani::any();
    let mut value = owner(len, 2);
    let result = value.get_mut(start..end);
    assert_eq!(result.is_ok(), start <= end && end <= len);
    if let Ok(range) = result {
        if let Some(first) = range.first_mut() {
            *first = 97;
        }
    }
    let result = value.get_mut(start..=end);
    // A freshly constructed inclusive range with start == end+1 is an empty
    // slice, matching core's conversion to start..end+1. Compute in wider math.
    assert_eq!(
        result.is_ok(),
        start as u128 <= end as u128 + 1 && end as u128 + 1 <= len as u128
    );
    if let Ok(range) = result {
        if let Some(first) = range.first_mut() {
            *first = 97;
        }
    }
}

#[kani::proof]
#[kani::solver(cadical)]
fn every_wrapping_result_is_checked_before_wide_element_access() {
    let index: usize = kani::any();
    let mut value = Vec::try_from_array([11u64, 17]).unwrap();
    let result = value.get_mut(index);
    assert_eq!(result.is_ok(), index < 2);
    if let Ok(place) = result {
        *place = 97;
    }
    assert_eq!(value.len(), 2);
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn every_wrapping_result_is_checked_before_indexed_memory_access() {
    let len = arbitrary_len();
    let index: usize = kani::any();
    let mut value = owner(len, 2);
    assert_eq!(value.get(index).is_ok(), index < len);
    let result = value.get_mut(index);
    assert_eq!(result.is_ok(), index < len);
    if let Ok(place) = result {
        *place = 97;
    }
    assert_eq!(value.len(), len);
    for i in 0..len {
        assert_eq!(
            *value.get(i).unwrap(),
            if i == index { 97 } else { DATA[i] }
        );
    }
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn admitted_indices_are_bounded_before_single_element_moves() {
    match kani::any::<u8>() {
        0 => {
            let mut value = owner(1, 2);
            assert!(value.try_insert(0, 97).is_ok());
            assert_eq!(value.as_slice(), &[97, 11]);
        }
        1 => {
            let mut value = owner(2, 2);
            assert_eq!(value.remove(0), Ok(11));
            assert_eq!(value.as_slice(), &[17]);
        }
        2 => {
            let mut value = owner(2, 2);
            assert_eq!(value.swap_remove(0), Ok(11));
            assert_eq!(value.as_slice(), &[17]);
        }
        _ => {
            let mut value = owner(1, 2);
            assert!(value.push_within_capacity(97).is_ok());
            assert_eq!(value.as_slice(), &[11, 97]);
        }
    }
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn every_wrapping_result_is_checked_before_reservation_or_reallocation() {
    let len = arbitrary_len();
    let additional: usize = kani::any();
    let exact: bool = kani::any();
    let mut value = owner(len, 4);
    let result = if exact {
        value.try_reserve_exact(additional)
    } else {
        value.try_reserve(additional)
    };
    if additional > 4 - len {
        assert_eq!(result, Err(Error::Capacity));
    }
    if result.is_ok() {
        assert!(value.capacity() - len >= additional);
    }
    unchanged(&value, len);
    assert!(len <= value.capacity() && value.capacity() <= 4);
    assert!(value.capacity() * core::mem::size_of::<u8>() <= 4);
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(8)]
fn admitted_range_is_bounded_before_drain_and_tail_restoration() {
    let len = 2;
    let start = 0;
    let end = 1;
    let first: u8 = kani::any();
    let last: u8 = kani::any();
    let valid = expected_range(start, end, first, last, len);
    let mut value = owner(len, 2);
    {
        let result = value.try_drain((bound(start, first), bound(end, last)));
        assert_eq!(result.is_ok(), valid);
        match result {
            Ok(mut drain) => {
                let _ = drain.next();
                let _ = drain.next_back();
            }
            Err(error) => assert_eq!(error, Error::Bounds),
        }
    }
    if !valid {
        unchanged(&value, len);
    }
    assert!(value.len() <= len);
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn admitted_range_is_bounded_before_extraction_and_error_cleanup() {
    let len = 2;
    let start = 0;
    let end = 2;
    let mut value = owner(len, 2);
    {
        let result = value.try_extract_if(start..end, |_| {
            if kani::any() {
                Err(7u8)
            } else {
                Ok(kani::any::<bool>())
            }
        });
        assert_eq!(result.is_ok(), start <= end && end <= len);
        if let Ok(mut extract) = result {
            let _ = extract.next();
            let _ = extract.next();
        }
    }
    if !(start <= end && end <= len) {
        unchanged(&value, len);
    }
    assert!(value.len() <= len);
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn admitted_range_is_bounded_before_splice_and_source_failure_cleanup() {
    let len = 2;
    let start = 0;
    let end = 1;
    let mut value = owner(len, 2);
    let replacement = crate::iter::from_results([Ok(97), Err(7u8)]);
    {
        let result = value.try_splice_fallible(start..end, replacement);
        assert_eq!(result.is_ok(), start <= end && end <= len);
        match result {
            Ok(splice) => {
                let result = splice.try_finish();
                assert!(result.is_err());
            }
            Err(InitError::Storage(error)) => assert_eq!(error, Error::Bounds),
            Err(InitError::Initialize(_)) => unreachable!(),
        }
    }
    if !(start <= end && end <= len) {
        unchanged(&value, len);
    }
    assert!(value.len() <= value.capacity() && value.capacity() <= 2);
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(8)]
fn admitted_range_is_bounded_before_internal_range_copy() {
    let len = 1;
    let start = 0;
    let end = 1;
    let mut value = owner(len, 2);
    let copied: bool = kani::any();
    let result = if copied {
        value.try_extend_copy_from_within(start..end)
    } else {
        value.try_extend_from_within(start..end)
    };
    let fits = start <= end && end <= len && end - start <= 2 - len;
    assert_eq!(result.is_ok(), fits);
    if !fits {
        unchanged(&value, len);
    }
    assert!(value.len() <= 2);
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn every_wrapping_result_is_checked_before_box_index_and_owned_iteration() {
    let len = arbitrary_len();
    let index: usize = kani::any();
    let mut value = owner(len, 2)
        .try_into_boxed_slice()
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(value.get(index).is_ok(), index < len);
    let result = value.get_mut(index);
    assert_eq!(result.is_ok(), index < len);
    if let Ok(place) = result {
        *place = 97;
    }
    let mut iter = value.into_iter();
    let _ = iter.next();
    let _ = iter.next_back();
    assert!(iter.len() <= len);
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(7)]
fn every_wrapping_result_is_checked_before_split_and_spare_views() {
    let len = arbitrary_len();
    let index: usize = kani::any();
    let mut value = owner(len, 2);
    assert_eq!(value.remaining_limit(), 2 - len);
    let (initialized, spare) = value.split_at_spare_mut();
    assert_eq!(initialized.len(), len);
    assert_eq!(spare.len(), 2 - len);
    let result = value.try_split_off(index);
    assert_eq!(result.is_ok(), index <= len);
    if let Ok(tail) = result {
        assert_eq!(value.len() + tail.len(), len);
    } else {
        unchanged(&value, len);
    }
}
