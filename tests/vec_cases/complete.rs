use super::*;
use std::ops::Bound;

#[test]
fn drain_every_range_and_consumption_matches_std() {
    for len in 0..9 {
        for start in 0..=len {
            for end in start..=len {
                for taken in 0..=end - start {
                    let mut a = V::try_from_iter(0..len).unwrap();
                    let mut b: Vec<_> = (0..len).collect();
                    let ptr = a.as_ptr();
                    {
                        let mut x = a.try_drain(start..end).unwrap();
                        let mut y = b.drain(start..end);
                        for i in 0..taken {
                            assert_eq!(x.len(), y.len());
                            assert_eq!(
                                if i % 2 == 0 { x.next() } else { x.next_back() },
                                if i % 2 == 0 { y.next() } else { y.next_back() }
                            );
                        }
                        assert_eq!(x.as_slice(), y.as_slice());
                    }
                    assert_eq!(a.as_slice(), b);
                    assert_eq!(a.as_ptr(), ptr);
                }
            }
        }
    }
    let mut v = V::try_from_array([1, 2, 3]).unwrap();
    for range in [
        (Bound::Included(2), Bound::Excluded(1)),
        (Bound::Excluded(usize::MAX), Bound::Unbounded),
        (Bound::Unbounded, Bound::Included(usize::MAX)),
        (Bound::Included(0), Bound::Excluded(4)),
    ] {
        assert!(matches!(v.try_drain(range), Err(Error::Bounds)));
        assert_eq!(v.as_slice(), [1, 2, 3]);
    }
}
#[test]
fn retain_dedup_and_extraction_match_std_and_callback_order() {
    for len in 0..25 {
        let input: Vec<_> = (0..len).map(|x| x / 3).collect();
        let mut a = V::try_from_slice(&input).unwrap();
        let mut b = input.clone();
        a.retain_mut(|x| {
            *x += 1;
            *x % 2 == 0
        });
        b.retain_mut(|x| {
            *x += 1;
            *x % 2 == 0
        });
        assert_eq!(a.as_slice(), b);
        let mut calls_a = Vec::new();
        let mut calls_b = Vec::new();
        a.dedup_by(|x, y| {
            calls_a.push((*x, *y));
            *x == *y
        });
        b.dedup_by(|x, y| {
            calls_b.push((*x, *y));
            *x == *y
        });
        assert_eq!(a.as_slice(), b);
        assert_eq!(calls_a, calls_b);
        for taken in 0..=len {
            let mut a = V::try_from_slice(&input).unwrap();
            let mut b = input.clone();
            let start = len / 4;
            let end = len - len / 4;
            let x: Vec<_> = a
                .try_extract_if(start..end, |x| {
                    *x += 1;
                    Ok::<_, ()>(*x % 2 == 0)
                })
                .unwrap()
                .take(taken)
                .map(Result::unwrap)
                .collect();
            let y: Vec<_> = b
                .extract_if(start..end, |x| {
                    *x += 1;
                    *x % 2 == 0
                })
                .take(taken)
                .collect();
            assert_eq!(x, y);
            assert_eq!(a.as_slice(), b);
        }
    }
}
#[test]
fn extraction_long_retained_prefix_preserves_progress_and_ownership() {
    struct Item<'a> {
        id: usize,
        value: usize,
        drops: &'a [Cell<usize>; 9],
    }
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.drops[self.id].set(self.drops[self.id].get() + 1);
        }
    }
    for selected in 0..=9 {
        for fail in 0..=9 {
            for consume_all in [false, true] {
                let drops = std::array::from_fn(|_| Cell::new(0));
                let mut v = V::try_from_iter((0..9).map(|id| Item {
                    id,
                    value: id,
                    drops: &drops,
                }))
                .unwrap();
                let mut calls = Vec::new();
                let mut extracted = Vec::new();
                let mut saw_error = false;
                let mut e = v
                    .try_extract_if(.., |x| {
                        calls.push(x.id);
                        x.value += 100;
                        if x.id == fail {
                            Err(7)
                        } else {
                            Ok(x.id == selected)
                        }
                    })
                    .unwrap();
                loop {
                    match e.next() {
                        Some(Ok(x)) => extracted.push(x),
                        Some(Err(7)) => {
                            saw_error = true;
                            assert!(e.next().is_none());
                            assert_eq!(e.size_hint(), (0, Some(0)));
                            break;
                        }
                        Some(Err(_)) => unreachable!(),
                        None => break,
                    }
                    if !consume_all {
                        break;
                    }
                }
                drop(e);
                let visited = if consume_all {
                    (fail + 1).min(9)
                } else {
                    (selected.min(fail) + 1).min(9)
                };
                assert_eq!(calls, (0..visited).collect::<Vec<_>>());
                assert_eq!(saw_error, fail < 9 && (consume_all || fail <= selected));
                let removed = selected < 9 && selected < fail;
                assert_eq!(
                    extracted.iter().map(|x| x.id).collect::<Vec<_>>(),
                    if removed { vec![selected] } else { vec![] }
                );
                assert_eq!(
                    v.as_slice().iter().map(|x| x.id).collect::<Vec<_>>(),
                    (0..9)
                        .filter(|&i| !removed || i != selected)
                        .collect::<Vec<_>>()
                );
                for item in v.as_slice().iter().chain(extracted.iter()) {
                    assert_eq!(
                        item.value,
                        item.id + if item.id < visited { 100 } else { 0 }
                    );
                }
                drop(extracted);
                drop(v);
                assert!(drops.iter().all(|x| x.get() == 1));
            }
        }
    }
    for fail in 0..9 {
        let drops = std::array::from_fn(|_| Cell::new(0));
        let mut v = V::try_from_iter((0..9).map(|id| Item {
            id,
            value: id,
            drops: &drops,
        }))
        .unwrap();
        let mut calls = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut e = v
                .try_extract_if(.., |x| {
                    assert_eq!(x.id, calls);
                    calls += 1;
                    x.value += 100;
                    if x.id == fail {
                        panic!("retained prefix callback");
                    }
                    Ok::<_, ()>(false)
                })
                .unwrap();
            assert!(e.next().is_none());
        }));
        assert!(result.is_err());
        assert_eq!(calls, fail + 1);
        assert_eq!(v.len(), 9);
        for (id, item) in v.as_slice().iter().enumerate() {
            assert_eq!(item.id, id);
            assert_eq!(item.value, id + if id <= fail { 100 } else { 0 });
        }
        drop(v);
        assert!(drops.iter().all(|x| x.get() == 1));
    }
}

#[test]
fn callback_errors_preserve_unvisited_suffix_and_mutations() {
    for fail in 0..6 {
        let mut v = V::try_from_array([0, 1, 2, 3, 4, 5]).unwrap();
        let mut count = 0;
        let r = v.try_retain_mut(|x| {
            let i = count;
            count += 1;
            *x += 10;
            if i == fail { Err(7) } else { Ok(*x % 2 == 0) }
        });
        assert_eq!(r, Err(7));
        let expected: Vec<_> = (0..fail)
            .filter(|x| x % 2 == 0)
            .map(|x| x + 10)
            .chain([fail + 10])
            .chain(fail + 1..6)
            .collect();
        assert_eq!(v.as_slice(), expected);
        let mut v = V::try_from_array([0, 1, 2, 3, 4, 5]).unwrap();
        let mut count = 0;
        let mut e = v
            .try_extract_if(.., |x| {
                let i = count;
                count += 1;
                *x += 10;
                if i == fail { Err(7) } else { Ok(*x % 2 != 0) }
            })
            .unwrap();
        let mut saw_error = false;
        for item in e.by_ref() {
            if item.is_err() {
                saw_error = true;
            }
        }
        assert!(saw_error);
        assert_eq!(e.next(), None);
        assert_eq!(e.size_hint(), (0, Some(0)));
        drop(e);
        assert_eq!(v.as_slice(), expected);
    }
    let mut v = V::try_from_array([1, 1, 2, 2, 3]).unwrap();
    let mut calls = 0;
    assert_eq!(
        v.try_dedup_by(|a, b| {
            calls += 1;
            if calls == 3 { Err(9) } else { Ok(*a == *b) }
        }),
        Err(9)
    );
    assert_eq!(v.as_slice(), [1, 2, 2, 3]);
    v.try_dedup_by_key(|x| Ok::<_, ()>(*x)).unwrap();
    assert_eq!(v.as_slice(), [1, 2, 3]);
}
#[test]
fn splice_every_range_length_and_early_drop_matches_explicit_contract() {
    for len in 0..7 {
        for start in 0..=len {
            for end in start..=len {
                for n in 0..7 {
                    let mut a = V::try_from_iter(0..len).unwrap();
                    let mut b: Vec<_> = (0..len).collect();
                    let mut x = a.try_splice(start..end, 100..100 + n).unwrap();
                    let y: Vec<_> = b.splice(start..end, 100..100 + n).collect();
                    assert_eq!(x.by_ref().collect::<Vec<_>>(), y);
                    x.finish().unwrap();
                    assert_eq!(a.as_slice(), b);
                }
            }
        }
    }
    let mut v = V::try_from_array([1, 2, 3, 4]).unwrap();
    let mut calls = 0;
    drop(
        v.try_splice(
            1..3,
            std::iter::from_fn(|| {
                calls += 1;
                Some(9)
            }),
        )
        .unwrap(),
    );
    assert_eq!(calls, 0);
    assert_eq!(v.as_slice(), [1, 4]);
    // Unknown length stages excess only once, exact final output matches std.
    let mut a = V::try_from_array([0, 1, 2, 3]).unwrap();
    a.try_splice(1..2, (10..30).filter(|_| true))
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(
        a.as_slice(),
        std::iter::once(0)
            .chain(10..30)
            .chain([2, 3])
            .collect::<Vec<_>>()
    );
}
#[test]
fn splice_fail_each_allocation_and_limit_preserves_retained_tail_and_budget() {
    let mut observed = 0;
    for fail in 1..9 {
        let ((), stats) = common::track(usize::MAX, || {
            let budget = MemoryBudget::new(4096);
            let mut v = V::try_from_array_in(
                [1u64, 2, 3, 4],
                Budget::Limited(&budget),
                LengthLimit::new(32),
            )
            .unwrap();
            refuse_after(fail);
            let r = v
                .try_splice(1..2, (10..25).filter(|_| true))
                .unwrap()
                .finish();
            allow();
            if r.is_err() {
                observed += 1;
                assert_eq!(v.first(), Some(&1));
                assert_eq!(&v.as_slice()[v.len() - 2..], &[3, 4]);
            } else {
                assert_eq!(v.len(), 18);
            }
            assert!(v.capacity() <= 32);
            assert_eq!(budget.occupied(), v.capacity() * 8);
            v.try_push(99).unwrap();
            drop(v);
            assert_eq!(budget.occupied(), 0);
        });
        assert_eq!(stats.live, 0);
    }
    assert!(observed >= 3);
    let mut v = V::try_from_array_in([0, 1, 2, 3], Unmetered, LengthLimit::new(4)).unwrap();
    assert_eq!(
        v.try_splice(1..2, [8, 9]).unwrap().finish(),
        Err(Error::Capacity)
    );
    assert_eq!(v.as_slice(), [0, 8, 2, 3]);
}
#[test]
fn spare_storage_checked_length_and_reference_returning_edits() {
    let mut v = V::<u32>::try_with_capacity_and_limit(4, 4).unwrap();
    v.spare_capacity_mut()[0].write(11);
    unsafe {
        v.try_set_len(1).unwrap();
    }
    let (prefix, spare) = v.split_at_spare_mut();
    prefix[0] += 1;
    spare[0].write(22);
    unsafe {
        v.try_set_len(2).unwrap();
    }
    assert_eq!(unsafe { v.try_set_len(5) }, Err(Error::Capacity));
    *v.try_push_mut(33).unwrap() += 1;
    *v.try_insert_mut(1, 44).unwrap() += 1;
    assert_eq!(v.as_slice(), [12, 45, 22, 34]);
    assert_eq!(v.push_within_capacity(99).unwrap_err().value, 99);
    assert_eq!(
        v.try_pop_if(|x| {
            *x += 1;
            Err::<bool, _>(7)
        }),
        Err(7)
    );
    assert_eq!(v.pop_if(|x| *x == 35), Some(35));
    *v.push_within_capacity(66).unwrap() = 77;
    assert_eq!(v.as_slice(), [12, 45, 22, 77]);
    let _: &Unmetered = v.allocation_policy();
}
#[test]
fn owned_resize_clone_failures_and_internal_extensions_roll_back() {
    #[derive(Debug)]
    struct Item<'a> {
        id: usize,
        calls: &'a Cell<usize>,
        live: &'a Cell<usize>,
        fail: usize,
    }
    impl TryClone for Item<'_> {
        fn try_clone(&self) -> Result<Self, Error> {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n == self.fail {
                return Err(Error::Capacity);
            }
            self.live.set(self.live.get() + 1);
            Ok(Self {
                id: self.id,
                calls: self.calls,
                live: self.live,
                fail: self.fail,
            })
        }
    }
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.live.set(self.live.get() - 1);
        }
    }
    for fail in 1..5 {
        let calls = Cell::new(0);
        let live = Cell::new(1);
        let item = Item {
            id: 7,
            calls: &calls,
            live: &live,
            fail,
        };
        let mut v = V::with_limit(5);
        match v.try_resize_owned(4, item) {
            Err(e) => {
                assert!(v.is_empty());
                assert_eq!(live.get(), 1);
                assert_eq!(e.value.id, 7);
                drop(e);
            }
            Ok(()) => {
                assert_eq!(calls.get(), 3);
                assert_eq!(live.get(), 4);
                drop(v);
            }
        }
        assert_eq!(live.get(), 0);
    }
    for fail in 1..4 {
        let calls = Cell::new(0);
        let live = Cell::new(2);
        let mut v = V::try_from_array([
            Item {
                id: 1,
                calls: &calls,
                live: &live,
                fail,
            },
            Item {
                id: 2,
                calls: &calls,
                live: &live,
                fail,
            },
        ])
        .unwrap();
        let r = v.try_extend_from_within(..);
        if fail <= 2 {
            assert!(r.is_err());
            assert_eq!(v.len(), 2);
            assert_eq!(live.get(), 2);
        } else {
            assert!(r.is_ok());
            assert_eq!(v.len(), 4);
        }
        drop(v);
        assert_eq!(live.get(), 0);
    }
    let mut a = V::try_from_array([1, 2, 3, 4]).unwrap();
    a.try_extend_copy_from_within(1..=2).unwrap();
    assert_eq!(a.as_slice(), [1, 2, 3, 4, 2, 3]);
    assert_eq!(a.try_extend_from_within(5..3), Err(Error::Bounds));
}
#[test]
fn conversions_flatten_limits_zst_overflow_and_iterator_clone() {
    let budget = MemoryBudget::new(512);
    let arrays =
        V::try_from_array_in([[1u32, 2], [3, 4]], Budget::Limited(&budget), Unlimited).unwrap();
    let ptr = arrays.as_ptr().cast::<u32>();
    let charge = budget.occupied();
    let flat = arrays.try_into_flattened().unwrap();
    assert_eq!(flat.as_ptr(), ptr);
    assert_eq!(flat.as_slice(), [1, 2, 3, 4]);
    assert_eq!(budget.occupied(), charge);
    assert_eq!(flat.try_into_array::<4>().unwrap(), [1, 2, 3, 4]);
    assert_eq!(budget.occupied(), 0);
    let arrays = V::try_from_array_in([[1, 2], [3, 4]], Unmetered, LengthLimit::new(2)).unwrap();
    let e = arrays.try_into_flattened().unwrap_err();
    assert_eq!(e.error, Error::Capacity);
    assert_eq!(e.value.len(), 2);
    let empty = V::<[u64; 0]>::try_from_array([[], [], []])
        .unwrap()
        .try_into_flattened()
        .unwrap();
    assert_eq!((empty.len(), empty.capacity()), (0, 0));
    let mut z = V::<[(); 2]>::new();
    unsafe {
        z.try_set_len(usize::MAX).unwrap();
    }
    let z = z.try_into_flattened().unwrap_err();
    assert_eq!(z.error, Error::Capacity);
    let mut i = V::try_from_array([1, 2, 3, 4]).unwrap().into_iter();
    assert_eq!(i.next(), Some(1));
    assert_eq!(i.next_back(), Some(4));
    i.as_mut_slice()[0] = 9;
    let copy = i.try_clone().unwrap();
    assert_eq!(copy.as_slice(), [9, 3]);
    assert_eq!(i.as_slice(), [9, 3]);
    assert_eq!(
        V::try_from_array([1, 2])
            .unwrap()
            .try_into_array::<3>()
            .unwrap_err()
            .value
            .as_slice(),
        [1, 2]
    );
    assert_eq!(
        V::try_from_cow(std::borrow::Cow::Borrowed(&[1, 2][..]))
            .unwrap()
            .as_slice(),
        [1, 2]
    );
    assert_eq!(
        V::try_from_cow(std::borrow::Cow::Owned(vec![1, 2]))
            .unwrap()
            .as_slice(),
        [1, 2]
    );
    assert_eq!(V::try_from_str("aé").unwrap().as_slice(), "aé".as_bytes());
    assert_eq!(V::try_from_elem(9, 3).unwrap().as_slice(), [9, 9, 9]);
    assert!(V::try_from_fallible_iter(jzap::iter::from_results([Ok(1), Err(7)])).is_err());
}
#[test]
fn leak_can_be_reclaimed_with_original_capacity_and_charge() {
    let budget = MemoryBudget::new(64);
    let v = V::try_from_array_in([1u32, 2], Budget::Limited(&budget), Unlimited).unwrap();
    let cap = v.capacity();
    let leaked = v.leak();
    leaked[0] = 9;
    assert_eq!(budget.occupied(), 8);
    let len = leaked.len();
    let ptr = std::ptr::NonNull::new(leaked.as_mut_ptr()).unwrap();
    let recovered =
        unsafe { V::from_raw_parts(ptr, len, cap, Budget::Limited(&budget), Unlimited) };
    assert_eq!(recovered.as_slice(), [9, 2]);
    drop(recovered);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn byte_writes_are_transactional_and_formatter_cannot_hide_failure() {
    struct Ignores;
    impl std::fmt::Display for Ignores {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let _ = f.write_str("too long to fit");
            Ok(())
        }
    }
    struct Rejects;
    impl std::fmt::Display for Rejects {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    let mut v = V::with_limit(8);
    v.try_write_all(b"a").unwrap();
    assert_eq!(v.try_write_vectored(&[b"bc", b"de"]), Ok(4));
    assert_eq!(v.try_write_vectored(&[b"fg", b"hi"]), Err(Error::Capacity));
    assert_eq!(v.as_slice(), b"abcde");
    assert_eq!(
        v.try_write_fmt(format_args!("{Ignores}")),
        Err(Error::Capacity)
    );
    assert_eq!(v.as_slice(), b"abcde");
    assert_eq!(
        v.try_write_fmt(format_args!("x{Rejects}")),
        Err(Error::InvalidStructure)
    );
    assert_eq!(v.as_slice(), b"abcde");
    v.try_write_fmt(format_args!("{}", 42)).unwrap();
    assert_eq!(v.as_slice(), b"abcde42");
    v.flush();
}
#[test]
fn zst_drop_counts_and_large_alignment_on_every_edit_path() {
    thread_local! {static DROPS:Cell<usize>=const{Cell::new(0)};}
    #[derive(Debug)]
    struct Z;
    impl Drop for Z {
        fn drop(&mut self) {
            DROPS.with(|n| n.set(n.get() + 1));
        }
    }
    DROPS.with(|n| n.set(0));
    {
        let mut v = V::with_limit(20);
        for _ in 0..10 {
            v.try_push(Z).unwrap();
        }
        drop(v.try_drain(2..5).unwrap());
        v.dedup_by(|_, _| true);
        v.try_splice(.., [Z, Z, Z]).unwrap().finish().unwrap();
        drop(
            v.try_extract_if(.., |_| Ok::<_, ()>(true))
                .unwrap()
                .next()
                .unwrap()
                .unwrap(),
        );
    }
    assert_eq!(DROPS.with(Cell::get), 13);
    #[repr(align(4096))]
    #[derive(Debug, PartialEq)]
    struct Aligned(u8);
    let mut v = V::try_from_array([Aligned(1), Aligned(2), Aligned(3)]).unwrap();
    v.try_splice(1..2, [Aligned(4), Aligned(5)])
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(v.as_ptr().addr() % 4096, 0);
    v.retain(|x| x.0 != 4);
    assert_eq!(v.as_slice(), [Aligned(1), Aligned(5), Aligned(3)]);
    let removed: Vec<_> = v
        .try_extract_if(.., |x| Ok::<_, ()>(x.0 == 5))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(removed, [Aligned(5)]);
    assert_eq!(v.as_slice(), [Aligned(1), Aligned(3)]);
}
#[test]
fn destructor_unwind_restores_tail_and_never_double_drops() {
    #[derive(Debug)]
    struct Item<'a> {
        id: usize,
        counts: &'a [Cell<usize>; 6],
        panic_id: usize,
    }
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.counts[self.id].set(self.counts[self.id].get() + 1);
            if self.id == self.panic_id {
                panic!("drop once");
            }
        }
    }
    for operation in 0..4 {
        let counts = std::array::from_fn(|_| Cell::new(0));
        let mut v = V::try_from_iter((0..6).map(|id| Item {
            id,
            counts: &counts,
            panic_id: 2,
        }))
        .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| match operation {
            0 => drop(v.try_drain(1..4).unwrap()),
            1 => {
                let _ = v.try_splice(1..4, std::iter::empty()).unwrap().finish();
            }
            2 => v.retain(|x| x.id == 0),
            _ => v.dedup_by(|_, _| true),
        }));
        assert!(result.is_err());
        let ids: Vec<_> = v.iter().map(|x| x.id).collect();
        assert_eq!(
            ids,
            if operation < 2 {
                vec![0, 4, 5]
            } else {
                vec![0, 3, 4, 5]
            }
        );
        drop(v);
        assert!(counts.iter().all(|x| x.get() == 1));
    }
}
#[test]
fn predicate_and_iterator_unwind_preserve_all_remaining_ownership() {
    for operation in 0..4 {
        let mut v = V::try_from_array([0, 1, 2, 3, 4]).unwrap();
        let r = catch_unwind(AssertUnwindSafe(|| match operation {
            0 => v.retain_mut(|x| {
                if *x == 3 {
                    panic!("callback");
                }
                *x % 2 == 0
            }),
            1 => v.dedup_by(|a, _| {
                if *a == 3 {
                    panic!("callback");
                }
                true
            }),
            2 => {
                let mut e = v
                    .try_extract_if(.., |x| {
                        if *x == 3 {
                            panic!("callback");
                        }
                        Ok::<_, ()>(*x % 2 == 1)
                    })
                    .unwrap();
                while e.next().is_some() {}
            }
            _ => {
                let mut i = 0;
                let it = std::iter::from_fn(|| {
                    i += 1;
                    if i == 3 {
                        panic!("iterator");
                    }
                    Some(10 + i)
                });
                let _ = v.try_splice(1..2, it).unwrap().finish();
            }
        }));
        assert!(r.is_err());
        assert_eq!(
            v.as_slice(),
            match operation {
                0 | 2 => &[0, 2, 3, 4][..],
                1 => &[0, 3, 4][..],
                _ => &[0, 11, 12, 2, 3, 4][..],
            }
        );
    }
}
#[test]
fn all_new_allocating_constructors_refuse_and_return_owned_inputs() {
    let ((), stats) = common::track(usize::MAX, || {
        refuse_after(1);
        let rejected = V::try_from_array([1u64, 2]).unwrap_err();
        allow();
        assert_eq!(rejected.value, [1, 2]);
        refuse_after(1);
        assert!(V::try_from_slice(&[1u64, 2]).is_err());
        allow();
        refuse_after(1);
        let e = V::try_from_elem(7u64, 5).unwrap_err();
        allow();
        assert_eq!(e.value, 7);
        let mut v = V::try_from_array([1u64, 2]).unwrap();
        let ptr = v.as_ptr();
        refuse_after(1);
        assert!(v.try_extend_from_within(..).is_err());
        allow();
        assert_eq!(v.as_slice(), [1, 2]);
        assert_eq!(v.as_ptr(), ptr);
        let i = v.into_iter();
        refuse_after(1);
        assert!(i.try_clone().is_err());
        allow();
        assert_eq!(i.as_slice(), [1, 2]);
    });
    assert_eq!(stats.live, 0);
}

#[test]
fn comparison_borrowing_and_target_iterator_clone_preserve_policies() {
    use std::borrow::{Borrow, BorrowMut};
    let mut a = V::try_from_array([1u64, 2, 3]).unwrap();
    let b = V::try_from_array_in([1u64, 2, 3], Unmetered, LengthLimit::new(3)).unwrap();
    assert_eq!(a, b);
    assert_eq!(a, [1, 2, 3]);
    assert_eq!([1, 2, 3], a);
    let slice: &[u64] = a.borrow();
    assert_eq!(slice, [1, 2, 3]);
    let slice: &mut [u64] = a.borrow_mut();
    slice[0] = 0;
    assert!(a < V::try_from_array([1u64, 2, 3]).unwrap());
    let budget = MemoryBudget::new(128);
    let mut i = a.into_iter();
    i.next().unwrap();
    let c = i.try_clone_in(Budget::Limited(&budget)).unwrap();
    assert_eq!(c.as_slice(), [2, 3]);
    assert_eq!(budget.occupied(), 16);
    drop(c);
    assert_eq!(budget.occupied(), 0);
    assert_eq!(i.as_slice(), [2, 3]);
    assert!(jzap::vec::IntoIter::<u8>::default().next().is_none());
}
#[test]
fn dishonest_splice_hints_are_bounded_and_never_authorize_uninitialized_values() {
    struct Hint {
        next: u8,
        hint: usize,
    }
    impl Iterator for Hint {
        type Item = u8;
        fn next(&mut self) -> Option<u8> {
            if self.next == 0 {
                None
            } else {
                self.next -= 1;
                Some(9)
            }
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.hint, Some(self.hint))
        }
    }
    for hint in [0, 1, 20, usize::MAX] {
        let mut v = V::try_from_array_in([0, 1, 2, 3], Unmetered, LengthLimit::new(12)).unwrap();
        v.try_splice(1..2, Hint { next: 3, hint })
            .unwrap()
            .finish()
            .unwrap();
        assert_eq!(v.as_slice(), [0, 9, 9, 9, 2, 3]);
        assert!(v.capacity() <= 12);
    }
    let mut v = V::try_from_array([0, 1, 2]).unwrap();
    assert!(matches!(
        v.try_extract_if(4.., |_| Ok::<_, ()>(true)),
        Err(Error::Bounds)
    ));
    assert!(matches!(v.try_splice(4.., [5]), Err(Error::Bounds)));
    assert_eq!(v.as_slice(), [0, 1, 2]);
}

#[test]
fn iterator_clone_replacement_preserves_destination_limit_budget_and_failure_state() {
    let mut small = V::try_from_array_in([9u64], Unmetered, LengthLimit::new(1))
        .unwrap()
        .into_iter();
    let large = V::try_from_array_in([1u64, 2, 3], Unmetered, LengthLimit::new(3))
        .unwrap()
        .into_iter();
    let ptr = small.as_slice().as_ptr();
    assert_eq!(small.try_clone_from(&large), Err(Error::Capacity));
    assert_eq!(small.as_slice(), [9]);
    assert_eq!(small.as_slice().as_ptr(), ptr);
    let ((), stats) = common::track(usize::MAX, || {
        let a = MemoryBudget::new(64);
        let b = MemoryBudget::new(64);
        let mut destination =
            V::try_from_array_in([9u64], Budget::Limited(&a), LengthLimit::new(4))
                .unwrap()
                .into_iter();
        let source = V::try_from_array_in([1u64, 2, 3], Budget::Limited(&b), LengthLimit::new(4))
            .unwrap()
            .into_iter();
        let ptr = destination.as_slice().as_ptr();
        refuse_after(1);
        assert!(destination.try_clone_from(&source).is_err());
        allow();
        assert_eq!(destination.as_slice(), [9]);
        assert_eq!(destination.as_slice().as_ptr(), ptr);
        assert_eq!((a.occupied(), b.occupied()), (8, 24));
        destination.try_clone_from(&source).unwrap();
        assert_eq!(destination.as_slice(), source.as_slice());
        assert_eq!((a.occupied(), b.occupied()), (24, 24));
        drop(destination);
        drop(source);
        assert_eq!((a.occupied(), b.occupied()), (0, 0));
    });
    assert_eq!(stats.live, 0);
}

#[test]
fn unhinted_splice_capacity_ownership_and_zst_match_contracts() {
    for capacity in [1, 3, 4, 7, 8] {
        for len in [1, capacity / 2, capacity] {
            for count in [0, 1, 2, 5, 9, 17] {
                let mut a = V::<usize>::try_with_capacity(capacity).unwrap();
                a.try_extend(0..len).unwrap();
                let mut b = Vec::with_capacity(capacity);
                b.extend(0..len);
                let mut left = 100..100 + count;
                let mut right = left.clone();
                a.try_splice(len / 3..len * 2 / 3, std::iter::from_fn(|| left.next()))
                    .unwrap()
                    .finish()
                    .unwrap();
                drop(b.splice(len / 3..len * 2 / 3, std::iter::from_fn(|| right.next())));
                assert_eq!(a.as_slice(), b);
                assert!(a.capacity() <= b.capacity());
            }
        }
    }
    let mut z = V::<()>::new();
    z.try_extend([(); 3]).unwrap();
    let mut extra = [(); 5].into_iter();
    z.try_splice(1..2, std::iter::from_fn(|| extra.next()))
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(z.len(), 7);
    for fail in 1..6 {
        let ((), stats) = common::track(usize::MAX, || {
            let mut a = V::try_from_array([
                B::try_new(1u8).unwrap(),
                B::try_new(2u8).unwrap(),
                B::try_new(3u8).unwrap(),
            ])
            .unwrap();
            let mut replacement = V::new();
            for i in 10..30 {
                replacement.try_push(B::try_new(i).unwrap()).unwrap();
            }
            let mut replacement = replacement.into_iter();
            refuse_after(fail);
            let r = a
                .try_splice(1..2, std::iter::from_fn(|| replacement.next()))
                .unwrap()
                .finish();
            allow();
            assert_eq!(**a.first().unwrap(), 1);
            assert_eq!(**a.last().unwrap(), 3);
            if r.is_ok() {
                assert_eq!(a.len(), 22);
            }
        });
        assert_eq!(stats.live, 0);
    }
}

#[test]
fn extraction_owns_small_and_large_noncopy_values_exactly_once() {
    #[derive(Debug)]
    struct Item<'a, const N: usize> {
        id: Box<usize>,
        counts: &'a [Cell<usize>; 6],
        payload: [u64; N],
    }
    impl<const N: usize> Drop for Item<'_, N> {
        fn drop(&mut self) {
            let id = *self.id;
            assert!(self.payload.iter().all(|x| *x == id as u64));
            self.counts[id].set(self.counts[id].get() + 1);
        }
    }
    fn check<const N: usize>() {
        for stop in 0..7 {
            for fail_at in 0..7 {
                let counts = std::array::from_fn(|_| Cell::new(0));
                let mut v = V::try_from_iter((0..6).map(|id| Item::<N> {
                    id: Box::new(id),
                    counts: &counts,
                    payload: [id as u64; N],
                }))
                .unwrap();
                let mut e = v
                    .try_extract_if(.., |x| {
                        if *x.id == fail_at {
                            Err(())
                        } else {
                            Ok(*x.id % 2 == 0)
                        }
                    })
                    .unwrap();
                for _ in 0..stop {
                    drop(e.next());
                }
                drop(e);
                drop(v);
                assert!(counts.iter().all(|n| n.get() == 1));
            }
        }
    }
    check::<0>();
    check::<4>();
}

#[test]
fn exact_clone_preserves_source_and_cleans_each_failure_position() {
    for failure in 1..=5 {
        let ((), stats) = common::track(usize::MAX, || {
            let bytes = 4 * size_of::<B<u64>>();
            let budget = MemoryBudget::new(2 * bytes);
            let mut source = V::<B<u64>, _>::with_limit_in(6, Budget::Limited(&budget));
            for i in 0..4 {
                source.try_push(B::try_new(i).unwrap()).unwrap();
            }
            let pointer = source.as_ptr();
            refuse_after(failure);
            assert!(source.try_clone().is_err());
            assert_eq!(budget.occupied(), bytes);
            assert_eq!(budget.reserved(), 0);
            assert_eq!(source.as_ptr(), pointer);
            assert_eq!(
                source.as_slice().iter().map(|x| **x).collect::<Vec<_>>(),
                [0, 1, 2, 3]
            );
            allow();
            let copy = source.try_clone().unwrap();
            assert_eq!(copy.max_len(), 6);
            assert_eq!(copy.capacity(), 4);
            assert_eq!(budget.occupied(), 2 * bytes);
            drop(copy);
            assert_eq!(budget.occupied(), bytes);
        });
        assert_eq!(stats.live, 0);
    }
    #[derive(Debug)]
    struct Item<'a> {
        id: usize,
        fail: usize,
        mode: usize,
        drops: &'a Cell<usize>,
    }
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    impl TryClone for Item<'_> {
        fn try_clone(&self) -> Result<Self, Error> {
            panic!("must use recorded initialization")
        }
        fn try_clone_into(&self, slot: &mut jzap::init::InitSlot<'_, Self>) -> Result<(), Error> {
            if self.id == self.fail && self.mode == 0 {
                return Err(Error::Bounds);
            }
            if self.id == self.fail && self.mode == 2 {
                return Ok(());
            }
            slot.write(Self {
                id: self.id,
                fail: self.fail,
                mode: self.mode,
                drops: self.drops,
            })
            .unwrap();
            if self.id == self.fail {
                if self.mode == 3 {
                    panic!("clone callback");
                }
                return Err(Error::Bounds);
            }
            Ok(())
        }
    }
    for fail in 0..4 {
        for mode in 0..4 {
            let drops = Cell::new(0);
            let source = V::try_from_iter((0..4).map(|id| Item {
                id,
                fail,
                mode,
                drops: &drops,
            }))
            .unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| source.try_clone()));
            if mode == 3 {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap().unwrap_err(),
                    if mode == 2 {
                        Error::Uninitialized
                    } else {
                        Error::Bounds
                    }
                );
            }
            assert_eq!(source.len(), 4);
            assert_eq!(drops.get(), fail + usize::from(mode == 1 || mode == 3));
            drop(source);
            assert_eq!(drops.get(), fail + usize::from(mode == 1 || mode == 3) + 4);
        }
    }
    let empty = V::<u64>::with_limit(0).try_clone().unwrap();
    assert_eq!((empty.len(), empty.capacity(), empty.max_len()), (0, 0, 0));
    let mut z = V::<()>::with_limit(7);
    z.try_resize(3, &()).unwrap();
    let c = z.try_clone().unwrap();
    assert_eq!((c.len(), c.capacity(), c.max_len()), (3, 7, 7));
}

#[test]
fn bulk_clone_uses_checked_storage_and_preserves_failure_state() {
    let ((), stats) = common::track(usize::MAX, || {
        let budget = MemoryBudget::new(128);
        let mut value = V::<u64, _>::with_limit_in(8, Budget::Limited(&budget));
        value.try_extend_cloned(&[1, 2, 3, 4]).unwrap();
        let pointer = value.as_ptr();
        let occupied = budget.occupied();
        refuse_after(1);
        assert!(value.try_extend_cloned(&[5, 6, 7, 8]).is_err());
        assert_eq!(value.as_slice(), [1, 2, 3, 4]);
        assert_eq!(value.as_ptr(), pointer);
        assert_eq!((budget.occupied(), budget.reserved()), (occupied, 0));
        refuse_after(1);
        assert!(value.try_clone().is_err());
        assert_eq!((budget.occupied(), budget.reserved()), (occupied, 0));
        allow();
        value.try_extend_cloned(&[5, 6, 7, 8]).unwrap();
        assert_eq!(value.as_slice(), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(value.try_extend_cloned(&[9]), Err(Error::Capacity));
        drop(value);
        assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
    });
    assert_eq!(stats.live, 0);
    let booleans = V::try_from_slice(&[true, false, true]).unwrap();
    assert_eq!(
        booleans.try_clone().unwrap().as_slice(),
        booleans.as_slice()
    );
    let characters = V::try_from_slice(&['\0', '한', '\u{10ffff}']).unwrap();
    assert_eq!(
        characters.try_clone().unwrap().as_slice(),
        characters.as_slice()
    );
    let values = [13, 17, 19];
    let references = V::try_from_slice(&[&values[0], &values[1], &values[2]]).unwrap();
    let copy = references.try_clone().unwrap();
    for (left, right) in references.iter().zip(copy.iter()) {
        assert!(std::ptr::eq(*left, *right));
    }
}

#[test]
fn batched_dedup_errors_preserve_order_and_drop_each_value_once() {
    struct Item<'a> {
        id: usize,
        key: usize,
        drops: &'a [Cell<usize>],
    }
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.drops[self.id].set(self.drops[self.id].get() + 1);
        }
    }
    let keys = [0, 0, 1, 2, 2, 2, 3, 4, 4, 5, 5, 6, 6, 6];
    for len in 2..=keys.len() {
        for failure in 0..len {
            let drops = std::array::from_fn::<_, 14, _>(|_| Cell::new(0));
            let mut value = V::try_from_iter((0..len).map(|id| Item {
                id,
                key: keys[id],
                drops: &drops,
            }))
            .unwrap();
            let mut expected = vec![0];
            let mut expected_calls = Vec::new();
            for read in 1..len {
                let previous = *expected.last().unwrap();
                expected_calls.push((read, previous));
                if read - 1 == failure {
                    expected.extend(read..len);
                    break;
                }
                if keys[read] != keys[previous] {
                    expected.push(read);
                }
            }
            let mut calls = Vec::new();
            let result = value.try_dedup_by(|later, previous| {
                calls.push((later.id, previous.id));
                if calls.len() - 1 == failure {
                    Err(7)
                } else {
                    Ok(later.key == previous.key)
                }
            });
            assert_eq!(result, if failure < len - 1 { Err(7) } else { Ok(()) });
            assert_eq!(calls, expected_calls);
            assert_eq!(value.iter().map(|v| v.id).collect::<Vec<_>>(), expected);
            drop(value);
            for count in &drops[..len] {
                assert_eq!(count.get(), 1);
            }
        }
    }
}
