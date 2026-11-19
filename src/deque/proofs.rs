//! Kani executes original product kernels; direct Verus separately covers every-word geometry.
use super::*;
// Avoid monomorphizing formatting of unreachable error branches in the verifier.
// Refusal still fails the query: this is the same success assertion as unwrap.
trait Success<T> {
    fn verified(self) -> T;
}
impl<T, E> Success<T> for Result<T, E> {
    fn verified(self) -> T {
        match self {
            Ok(value) => value,
            Err(_) => panic!("unexpected proof refusal"),
        }
    }
}
trait Rejection<E> {
    fn verified_err(self) -> E;
}
impl<T, E> Rejection<E> for Result<T, E> {
    fn verified_err(self) -> E {
        match self {
            Err(error) => error,
            Ok(_) => panic!("unexpected proof success"),
        }
    }
}

#[kani::proof]
fn geometry_all_words() {
    let cap: usize = kani::any();
    let head: usize = kani::any();
    let len: usize = kani::any();
    kani::assume(cap > 0 && head < cap && len <= cap);
    if len > 0 {
        assert!(geometry::after_pop(len, cap) < cap)
    }
    let p = geometry::physical(head, len, cap);
    if cap <= isize::MAX as usize {
        assert!(geometry::allocated_physical(head, len, cap) == p)
    }
    assert!(p < cap);
    assert!(
        head.checked_add(len) == Some(p)
            || head.checked_add(len).and_then(|v| v.checked_sub(cap)) == Some(p)
            || head.checked_add(len).is_none() && len - (cap - head) == p
    );
    assert!((geometry::retreat(p, len, cap)) == (head));
    let (a, b) = geometry::split(head, len, cap);
    assert!(a <= cap - head && b <= head);
    assert!((a.checked_add(b)) == (Some(len)));
    let new: usize = kani::any();
    if new > cap {
        let h = geometry::grown_head(head, len, cap, new);
        assert!(h < new);
        if b != 0 {
            if h == head {
                assert!(cap.checked_add(b).is_some_and(|end| end <= new))
            } else {
                assert!((h.checked_add(a)) == (Some(new)));
                assert!(b <= h)
            }
        }
    }
}
fn owner(head: usize, len: usize) -> VecDeque<u8> {
    let mut d = VecDeque::try_from_array([0u8, 1, 2, 3]).verified();
    d.head = Checked::new(head);
    d.len = Checked::new(len);
    for i in 0..4 {
        if i < len {
            unsafe { d.slot(i).write(i as u8) }
        }
    }
    d
}
fn order(d: &VecDeque<u8>, expected: &[u8]) {
    assert!((d.len()) == (expected.len()));
    for (i, v) in expected.iter().enumerate() {
        assert!((d.get(i)) == (Ok(v)))
    }
}
// Concrete ownership kernels leave integer universality to the direct-source geometry proofs.
fn moves_case<const HEAD: usize, const LEN: usize>() {
    let n = LEN / 2;
    let mut d = owner(HEAD, LEN);
    d.rotate_left(n).verified();
    d.rotate_right(n).verified();
    for i in 0..LEN {
        assert!((d.get(i)) == (Ok(&(i as u8))))
    }
    drop(d);
    let mut d = owner(HEAD, LEN);
    assert!((d.make_contiguous().len()) == (LEN));
    for i in 0..LEN {
        assert!((d.get(i)) == (Ok(&(i as u8))))
    }
    drop(d);
    let mut d = owner(HEAD, LEN);
    d.try_reserve_exact(5).verified();
    for i in 0..LEN {
        assert!((d.get(i)) == (Ok(&(i as u8))))
    }
    d.try_shrink_to_fit().verified();
    for i in 0..LEN {
        assert!((d.get(i)) == (Ok(&(i as u8))))
    }
}
#[kani::proof]
#[kani::unwind(6)]
fn actual_ring_moves_and_contiguous_growth() {
    match kani::any::<u8>() % 5 {
        0 => moves_case::<0, 4>(),
        1 => moves_case::<3, 3>(),
        2 => moves_case::<2, 3>(),
        3 => moves_case::<1, 4>(),
        _ => moves_case::<2, 4>(),
    }
}
fn edit_case<const AT: usize, const END: usize>() {
    let mut d = owner(3, 4);
    d.try_insert(AT, 99).verified();
    assert!((d.remove(AT)) == (Some(99)));
    for i in 0..4 {
        assert!((d.get(i)) == (Ok(&(i as u8))))
    }
    {
        let mut drain = d.drain(AT..END).verified();
        if let Some(v) = drain.next() {
            assert!((v) == (AT as u8))
        }
        let _ = drain.next_back();
    }
    assert!((d.len()) == (4 - (END - AT)));
    for i in 0..4 - (END - AT) {
        assert!((d.get(i)) == (Ok(&(if i < AT { i } else { i + END - AT } as u8))))
    }
}
#[kani::proof]
#[kani::unwind(6)]
fn actual_insert_remove_and_drain() {
    match kani::any::<u8>() % 3 {
        0 => edit_case::<1, 2>(),
        1 => edit_case::<1, 3>(),
        _ => edit_case::<3, 4>(),
    }
}
fn retention_case<const STOP: usize>() {
    let mut d = owner(3, 4);
    let mut visited = 0;
    let _ = d.try_retain(|v| {
        if visited == STOP {
            Err(())
        } else {
            visited += 1;
            Ok(*v % 2 == 0)
        }
    });
    let mut expected = [0; 4];
    let mut n = 0;
    for i in 0..4 {
        if i >= STOP || i % 2 == 0 {
            expected[n] = i as u8;
            n += 1
        }
    }
    order(&d, &expected[..n]);
}
fn split_case<const AT: usize>() {
    let mut d = owner(3, 4);
    let mut tail = d.try_split_off(AT).verified();
    d.try_append(&mut tail).verified();
    assert!(tail.is_empty());
    order(&d, &[0, 1, 2, 3]);
}
#[kani::proof]
#[kani::unwind(6)]
fn actual_retention_error_and_split_append() {
    match kani::any::<u8>() % 6 {
        0 => retention_case::<0>(),
        1 => retention_case::<3>(),
        2 => retention_case::<4>(),
        3 => split_case::<0>(),
        4 => split_case::<2>(),
        _ => split_case::<4>(),
    }
}
fn shrink_case<const HEAD: usize, const LEN: usize>() {
    let budget = crate::budget::MemoryBudget::new(4);
    let mut d = VecDeque::try_from_array_in([0u8, 1, 2, 3], Budget::Limited(&budget)).verified();
    d.head = Checked::new(HEAD);
    d.len = Checked::new(LEN);
    let p = d.ptr;
    assert!(matches!(d.try_shrink_to_fit(), Err(Error::Budget { .. })));
    assert!((d.ptr) == (p));
    assert!((d.head.get()) == (HEAD));
    assert!((d.len()) == (LEN));
    assert!((d.capacity()) == (4));
    assert!((budget.occupied()) == (4));
    assert!((budget.reserved()) == (0));
    for i in 0..LEN {
        assert!((d.get(i)) == (Ok(&(geometry::physical(HEAD, i, 4) as u8))))
    }
    drop(d);
    assert!((budget.occupied()) == (0));
}
#[kani::proof]
#[kani::unwind(6)]
fn actual_shrink_refusal_restores_ring_and_accounting() {
    match kani::any::<u8>() % 12 {
        0 => shrink_case::<0, 1>(),
        1 => shrink_case::<0, 2>(),
        2 => shrink_case::<0, 3>(),
        3 => shrink_case::<1, 1>(),
        4 => shrink_case::<1, 2>(),
        5 => shrink_case::<1, 3>(),
        6 => shrink_case::<2, 1>(),
        7 => shrink_case::<2, 2>(),
        8 => shrink_case::<2, 3>(),
        9 => shrink_case::<3, 1>(),
        10 => shrink_case::<3, 2>(),
        _ => shrink_case::<3, 3>(),
    }
}

#[kani::proof]
#[kani::unwind(6)]
fn actual_clone_limits_and_consuming_fold() {
    let mut d = VecDeque::try_from_array([0u8, 1, 2, 3]).verified();
    d.rotate_left(3).verified();
    let copy = d.try_clone().verified();
    assert!(copy.into_iter().fold(0u16, |sum, v| sum + u16::from(v)) == 6);
    assert!(d.pop_front() == Some(3));
    d.try_push_back(3).verified();
    d.try_resize(5, &9).verified();
    assert!(d.len() == 5);
    assert!(d.back() == Some(&9));
    let mut bounded = VecDeque::with_limit(0);
    assert!(bounded.try_push_front(9u8).verified_err().value == 9);
    assert!(bounded.try_reserve(usize::MAX) == Err(Error::Capacity));
}

#[kani::proof]
#[kani::unwind(6)]
fn actual_copy_clone_from_reuses_admitted_storage() {
    let budget = crate::budget::MemoryBudget::new(3);
    let mut target = VecDeque::try_from_array_in([0u8; 3], Budget::Limited(&budget)).verified();
    target.rotate_left(2).verified();
    let pointer = target.ptr;
    let values: [u8; 3] = kani::any();
    let mut source = VecDeque::try_from_array_in(values, Budget::Unmetered).verified();
    source.rotate_left(1).verified();
    target.try_clone_from(&source).verified();
    assert!(target.ptr == pointer && target.len() == 3 && target.head.get() == 0);
    assert!(target.get(0) == Ok(&values[1]));
    assert!(target.get(1) == Ok(&values[2]));
    assert!(target.get(2) == Ok(&values[0]));
    assert!(budget.occupied() == 3 && budget.reserved() == 0);
    let larger = VecDeque::try_from_array_in([1u8, 2, 3, 4], Budget::Unmetered).verified();
    assert!(matches!(
        target.try_clone_from(&larger),
        Err(Error::Budget { .. })
    ));
    assert!(target.ptr == pointer && target.len() == 3 && target.capacity() == 3);
    assert!(target.get(0) == Ok(&values[1]));
    assert!(target.get(1) == Ok(&values[2]));
    assert!(target.get(2) == Ok(&values[0]));
    drop(target);
    assert!(budget.occupied() == 0 && budget.reserved() == 0);
}

fn empty_copy_case<const ADMIT: bool>() {
    let budget = crate::budget::MemoryBudget::new(4);
    let mut d = VecDeque::try_from_array_in([0u8; 4], Budget::Limited(&budget)).verified();
    // An empty nonzero-head state is also reached by the native/Miri public pop sequence.
    d.len = Checked::new(0);
    d.head = Checked::new(3);
    let pointer = d.ptr;
    if ADMIT {
        let values: [u8; 4] = kani::any();
        d.try_extend_copy(&values).verified();
        assert!(d.head.get() == 0 && d.ptr == pointer && d.len() == 4);
        for i in 0..4 {
            assert!(d.get(i) == Ok(&values[i]));
        }
    } else {
        assert!(matches!(
            d.try_extend_copy(&[0; 5]),
            Err(Error::Budget { .. })
        ));
        assert!(d.ptr == pointer && d.head.get() == 3 && d.len() == 0 && d.capacity() == 4);
    }
    assert!(budget.occupied() == 4 && budget.reserved() == 0);
}
fn wrapped_copy_case<const GROW: bool>() {
    if GROW {
        let mut d = VecDeque::try_from_array([1u8, 2]).verified();
        d.head = Checked::new(1);
        d.try_extend_copy(&[3]).verified();
        order(&d, &[2, 1, 3]);
    } else {
        let values: [u8; 4] = kani::any();
        let mut d = VecDeque::try_from_array(values).verified();
        d.head = Checked::new(3);
        d.len = Checked::new(2);
        d.try_extend_copy(&[12, 13]).verified();
        assert!(d.get(0) == Ok(&values[3]));
        assert!(d.get(1) == Ok(&values[0]));
        assert!(d.get(2) == Ok(&12));
        assert!(d.get(3) == Ok(&13));
    }
}
#[kani::proof]
#[kani::unwind(6)]
fn actual_empty_copy_extension_admission_and_wrap() {
    match kani::any::<u8>() % 4 {
        0 => empty_copy_case::<false>(),
        1 => empty_copy_case::<true>(),
        2 => wrapped_copy_case::<false>(),
        _ => wrapped_copy_case::<true>(),
    }
}

#[kani::proof]
#[kani::unwind(4)]
fn actual_no_drop_truncation_all_word_lengths() {
    let initial: usize = kani::any();
    let keep: usize = kani::any();
    let mut d = VecDeque::<()>::new();
    d.try_resize(initial, &()).verified();
    if initial == usize::MAX {
        assert!(matches!(
            d.try_push_front(()),
            Err(Rejected {
                error: Error::Capacity,
                ..
            })
        ));
        assert!(matches!(
            d.try_push_back(()),
            Err(Rejected {
                error: Error::Capacity,
                ..
            })
        ));
    }
    d.truncate(keep);
    assert!(d.len() == if keep < initial { keep } else { initial });
    d.clear();
    assert!(d.is_empty());
    d.try_resize(initial, &()).verified();
    let start: usize = kani::any();
    let end: usize = kani::any();
    if start <= end && end <= initial {
        let mut drain = d.drain(start..end).verified();
        let take_front: bool = kani::any();
        let take_back: bool = kani::any();
        if take_front {
            let _ = drain.next();
        }
        if take_back {
            let _ = drain.next_back();
        }
        drop(drain);
        assert!(d.len() == initial - (end - start));
    }
}
