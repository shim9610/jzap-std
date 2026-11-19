use super::*;
#[kani::proof]
fn endpoints_all_words() {
    let (start, count, capacity): (usize, usize, usize) = kani::any();
    let got = geometry::admitted_end(start, count, capacity);
    let expected = start
        .checked_add(count)
        .filter(|end| start <= capacity && *end <= capacity);
    assert!(got == expected);
    if let Some(end) = got {
        assert!(count <= capacity && start <= capacity - count);
        assert!(end - start == count);
    }
    if start < capacity {
        let next = geometry::initialized_next(start, capacity);
        assert!(next > start && next <= capacity && next - start == 1);
    }
    let end: usize = kani::any();
    let got = geometry::range(start, end, capacity);
    assert!(got.is_ok() == (start <= end && end <= capacity));
    if let Ok((a, b)) = got {
        assert!(a == start && b == end);
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn actual_copy_swap_and_disjoint_partition_guards() {
    let original: [u8; 4] = kani::any();
    let source: [u8; 4] = kani::any();
    let (count, middle): (usize, usize) = kani::any();
    kani::assume(count <= 4);
    let mut values = original;
    let result = SliceMut::new(&mut values).copy_from_slice(&source[..count]);
    if count == 4 {
        assert!(result == Ok(()) && values == source);
    } else {
        assert!(result == Err(Error::Bounds) && values == original);
    }
    let mut values = original;
    let mut other = source;
    let result = SliceMut::new(&mut values).swap_with_slice(&mut other[..count]);
    if count == 4 {
        assert!(result == Ok(()) && values == source && other == original);
    } else {
        assert!(result == Err(Error::Bounds) && values == original && other == source);
    }
    let mut view = SliceMut::new(&mut values);
    let result = view.split_at_mut(middle);
    assert!(result.is_ok() == (middle <= 4));
    if let Ok((mut a, mut b)) = result {
        assert!(a.len() == middle && b.len() == 4 - middle);
        if let Ok(v) = a.get_mut(0) {
            *v = 7;
        }
        if let Ok(v) = b.get_mut(0) {
            *v = 9;
        }
    }
}
#[kani::proof]
#[kani::unwind(8)]
fn actual_swap_and_overlap_preserve_checked_state() {
    let original: [u8; 4] = kani::any();
    let (a, b, destination): (usize, usize, usize) = kani::any();
    let mut values = original;
    let result = SliceMut::new(&mut values).swap(a, b);
    let mut expected = original;
    if a < 4 && b < 4 {
        expected.swap(a, b);
        assert!(result == Ok(()));
    } else {
        assert!(result == Err(Error::Bounds));
    }
    assert!(values == expected);
    let mut values = original;
    let result = SliceMut::new(&mut values).copy_within(a..b, destination);
    let mut expected = original;
    if a <= b && b <= 4 && destination <= 4 && b - a <= 4 - destination {
        expected.copy_within(a..b, destination);
        assert!(result == Ok(()));
    } else {
        assert!(result == Err(Error::Bounds));
    }
    assert!(values == expected);
}
#[kani::proof]
#[kani::unwind(4)]
fn actual_bound_normalization_all_words() {
    let (a, b, length): (usize, usize, usize) = kani::any();
    let s = match kani::any::<u8>() % 3 {
        0 => Bound::Unbounded,
        1 => Bound::Included(a),
        _ => Bound::Excluded(a),
    };
    let e = match kani::any::<u8>() % 3 {
        0 => Bound::Unbounded,
        1 => Bound::Included(b),
        _ => Bound::Excluded(b),
    };
    let start = match s {
        Bound::Unbounded => Some(0),
        Bound::Included(a) => Some(a),
        Bound::Excluded(a) => a.checked_add(1),
    };
    let end = match e {
        Bound::Unbounded => Some(length),
        Bound::Excluded(b) => Some(b),
        Bound::Included(b) => b.checked_add(1),
    };
    let expected = match (start, end) {
        (Some(a), Some(b)) if a <= b && b <= length => Some((a, b)),
        _ => None,
    };
    assert!(checked_range((s, e), length).ok() == expected);
}
