use jzap::{
    Error, TryClone, TryCloneIn, TryExactSizeIterator, TryIterator,
    budget::{Budget, MemoryBudget},
    collections::{VecDeque, vec_deque},
    vec::Unlimited,
};
use std::{cell::Cell, collections::VecDeque as Std, rc::Rc};
mod common;
#[path = "common/deallocations.rs"]
mod deallocations;
#[global_allocator]
static A: common::FaultAllocator = common::FaultAllocator;
fn values<T: Copy, P: jzap::budget::AllocationPolicy, L: jzap::vec::LengthPolicy>(
    d: &VecDeque<T, P, L>,
) -> Vec<T> {
    d.iter().copied().collect()
}
fn ring(cap: usize, head: usize, len: usize) -> VecDeque<usize> {
    let mut d = VecDeque::try_with_capacity(cap).unwrap();
    for i in 0..head {
        d.try_push_back(i).unwrap();
        assert_eq!(d.pop_front(), Some(i));
    }
    for i in 0..len {
        d.try_push_back(i).unwrap()
    }
    d
}
fn bounds() -> usize {
    if cfg!(miri) { 7 } else { 15 }
}
#[test]
fn exhaustive_ring_moves() {
    for cap in 1..bounds() {
        for head in 0..cap {
            for len in 0..=cap {
                let expected: Vec<_> = (0..len).collect();
                for index in 0..=len {
                    let mut d = ring(cap, head, len);
                    let mut e = expected.clone();
                    d.try_insert(index, 99).unwrap();
                    e.insert(index, 99);
                    assert_eq!(values(&d), e, "insert {cap}/{head}/{len}/{index}");
                    assert_eq!(d.remove(index), Some(99));
                    assert_eq!(values(&d), expected);
                }
                for index in 0..len {
                    let mut d = ring(cap, head, len);
                    let mut e = expected.clone();
                    assert_eq!(d.remove(index), Some(e.remove(index)));
                    assert_eq!(values(&d), e, "remove {cap}/{head}/{len}/{index}");
                }
                for n in 0..=len {
                    let mut d = ring(cap, head, len);
                    let mut e = expected.clone();
                    d.rotate_left(n).unwrap();
                    e.rotate_left(n);
                    assert_eq!(values(&d), e, "left {cap}/{head}/{len}/{n}");
                    d.rotate_right(n).unwrap();
                    assert_eq!(values(&d), expected);
                }
                let mut d = ring(cap, head, len);
                assert_eq!(d.make_contiguous(), expected);
                assert!(d.as_slices().1.is_empty());
                let mut d = ring(cap, head, len);
                d.try_shrink_to_fit().unwrap();
                assert_eq!(values(&d), expected);
                assert_eq!(d.capacity(), len);
                let mut d = ring(cap, head, len);
                d.try_reserve_exact(cap + 3).unwrap();
                assert_eq!(values(&d), expected);
                for new_len in len..=cap {
                    let mut d = ring(cap, head, len);
                    d.try_resize(new_len, &99).unwrap();
                    let mut resized = expected.clone();
                    resized.resize(new_len, 99);
                    assert_eq!(values(&d), resized, "resize {cap}/{head}/{len}/{new_len}");
                }
            }
        }
    }
}
#[test]
fn exhaustive_drain_split_append_ranges() {
    for cap in 1..bounds().min(10) {
        for head in 0..cap {
            for len in 0..=cap {
                for start in 0..=len {
                    for end in start..=len {
                        let mut d = ring(cap, head, len);
                        let mut e: Std<_> = (0..len).collect();
                        assert_eq!(
                            d.range(start..end).unwrap().copied().collect::<Vec<_>>(),
                            (start..end).collect::<Vec<_>>()
                        );
                        let mut a = d.drain(start..end).unwrap();
                        let mut b = e.drain(start..end);
                        assert_eq!(a.next(), b.next());
                        assert_eq!(a.next_back(), b.next_back());
                        drop(a);
                        drop(b);
                        assert_eq!(
                            values(&d),
                            e.into_iter().collect::<Vec<_>>(),
                            "drain {cap}/{head}/{len}/{start}/{end}"
                        );
                    }
                }
                for at in 0..=len {
                    let mut d = ring(cap, head, len);
                    let mut tail = d.try_split_off(at).unwrap();
                    assert_eq!(values(&d), (0..at).collect::<Vec<_>>());
                    assert_eq!(values(&tail), (at..len).collect::<Vec<_>>());
                    d.try_append(&mut tail).unwrap();
                    assert_eq!(values(&d), (0..len).collect::<Vec<_>>());
                    assert!(tail.is_empty());
                }
            }
        }
    }
}
fn random(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
#[test]
fn differential_lifecycle() {
    let mut d = VecDeque::new();
    let mut e = Std::new();
    let mut seed = 9610;
    for _ in 0..if cfg!(miri) { 400 } else { 30000 } {
        let v = random(&mut seed);
        let n = (v as usize) % (d.len() + 1);
        match v % 16 {
            0..=2 => {
                d.try_push_back(v).unwrap();
                e.push_back(v)
            }
            3 => {
                d.try_push_front(v).unwrap();
                e.push_front(v)
            }
            4 => assert_eq!(d.pop_back(), e.pop_back()),
            5 => assert_eq!(d.pop_front(), e.pop_front()),
            6 => {
                d.try_insert(n, v).unwrap();
                e.insert(n, v)
            }
            7 => assert_eq!(d.remove(n), e.remove(n)),
            8 => {
                d.rotate_left(n).unwrap();
                e.rotate_left(n)
            }
            9 => {
                d.rotate_right(n).unwrap();
                e.rotate_right(n)
            }
            10 => {
                assert_eq!(d.make_contiguous(), e.make_contiguous());
            }
            11 => {
                d.try_shrink_to(v as usize % 10).unwrap();
                e.shrink_to(v as usize % 10)
            }
            12 => {
                d.try_reserve(v as usize % 10).unwrap();
                e.reserve(v as usize % 10)
            }
            13 => {
                d.try_retain_mut(|v| {
                    *v = v.wrapping_add(1);
                    Ok::<_, ()>(*v % 3 != 0)
                })
                .unwrap();
                e.retain_mut(|v| {
                    *v = v.wrapping_add(1);
                    *v % 3 != 0
                })
            }
            14 => {
                d.truncate(n);
                e.truncate(n)
            }
            _ => {
                let mut t = d.try_split_off(n).unwrap();
                let mut u = e.split_off(n);
                d.try_append(&mut t).unwrap();
                e.append(&mut u)
            }
        }
        assert_eq!(values(&d), e.iter().copied().collect::<Vec<_>>());
        assert_eq!(d.len(), e.len());
    }
}
#[test]
fn grow_and_shrink_refusal_preserves_exact_split() {
    for cap in 2..bounds() {
        for head in 0..cap {
            for len in 1..=cap {
                let mut d = ring(cap, head, len);
                let (a, b) = d.as_slices();
                let before = (a.to_vec(), b.to_vec(), a.as_ptr(), b.as_ptr(), d.capacity());
                let (result, _) = common::track(1, || d.try_reserve_exact(cap));
                assert!(matches!(result, Err(Error::Allocation { .. })));
                let (a, b) = d.as_slices();
                assert_eq!(
                    (a.to_vec(), b.to_vec(), a.as_ptr(), b.as_ptr(), d.capacity()),
                    before
                );
                if len < cap {
                    let (result, _) = common::track(1, || d.try_shrink_to_fit());
                    assert!(matches!(result, Err(Error::Allocation { .. })));
                    let (a, b) = d.as_slices();
                    assert_eq!(
                        (a.to_vec(), b.to_vec(), a.as_ptr(), b.as_ptr(), d.capacity()),
                        before,
                        "shrink refusal {cap}/{head}/{len}"
                    );
                }
            }
        }
    }
}
#[test]
fn limits_budgets_and_failures() {
    let budget = MemoryBudget::new(256);
    let mut d = VecDeque::try_with_capacity_and_limit_in(4, 6, Budget::Limited(&budget)).unwrap();
    for n in 0..4 {
        d.try_push_front(n).unwrap()
    }
    let old = values(&d);
    let used = budget.occupied();
    let (result, _) = common::track(1, || d.try_push_back(5));
    assert_eq!(result.unwrap_err().value, 5);
    assert_eq!(values(&d), old);
    assert_eq!(budget.occupied(), used);
    d.try_push_back(5).unwrap();
    d.try_push_back(6).unwrap();
    assert_eq!(d.capacity(), 6);
    assert_eq!(d.try_push_front(7).unwrap_err().error, Error::Capacity);
    assert_eq!(d.try_insert(99, 8).unwrap_err().value, 8);
    assert_eq!(d.try_reserve(usize::MAX), Err(Error::Capacity));
    assert!(d.range(..=usize::MAX).is_err());
    assert_eq!(d.rotate_left(7), Err(Error::Bounds));
    let mut clone = d.try_clone().unwrap();
    clone.clear();
    drop(clone);
    drop(d);
    assert_eq!(budget.occupied(), 0);
    let mut d = VecDeque::with_limit(3);
    d.try_extend_copy(&[1, 2, 3]).unwrap();
    assert_eq!(d.try_push_back(4).unwrap_err().value, 4);
    d.pop_front();
    d.try_push_back(4).unwrap();
}
#[derive(Debug)]
struct Token {
    index: usize,
    drops: Rc<Vec<Cell<usize>>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops[self.index].set(self.drops[self.index].get() + 1)
    }
}
fn tokens(n: usize) -> (VecDeque<Token>, Rc<Vec<Cell<usize>>>) {
    let drops: Rc<Vec<Cell<usize>>> = Rc::new((0..n).map(|_| Cell::new(0)).collect());
    let mut d = VecDeque::new();
    for index in 0..n {
        d.try_push_back(Token {
            index,
            drops: drops.clone(),
        })
        .unwrap()
    }
    (d, drops)
}
#[test]
fn retain_resize_errors_retire_once() {
    let (mut d, drops) = tokens(10);
    d.rotate_left(7).unwrap();
    let old: Vec<_> = d.iter().map(|t| t.index).collect();
    let mut visited = 0;
    let r = d.try_retain_mut(|v| {
        visited += 1;
        if visited == 6 {
            Err(())
        } else {
            Ok(v.index % 2 == 0)
        }
    });
    assert_eq!(r, Err(()));
    let expected: Vec<_> = old
        .iter()
        .enumerate()
        .filter(|(i, v)| *i >= 5 || **v % 2 == 0)
        .map(|(_, v)| *v)
        .collect();
    assert_eq!(d.iter().map(|t| t.index).collect::<Vec<_>>(), expected);
    drop(d);
    assert!(drops.iter().all(|n| n.get() == 1));
    let (mut d, drops) = tokens(6);
    d.rotate_left(4).unwrap();
    {
        let mut drain = d.drain(1..5).unwrap();
        drop(drain.next());
        drop(drain.next_back());
    }
    drop(d);
    assert!(drops.iter().all(|n| n.get() == 1));
    let mut d = VecDeque::try_from_array([1, 2, 3]).unwrap();
    let mut calls = 0;
    let r = d.try_resize_with(8, || {
        calls += 1;
        if calls == 3 { Err(()) } else { Ok(4) }
    });
    assert!(r.is_err());
    assert_eq!(values(&d), [1, 2, 3]);
    let r = d.try_pop_front_if(|v| {
        *v = 9;
        Err::<bool, _>(())
    });
    assert_eq!(r, Err(()));
    assert_eq!(d.front(), Some(&9));
}
#[test]
fn iteration_conversions_search_and_clone() {
    assert_eq!(vec_deque::Iter::<u8>::default().len(), 0);
    assert_eq!(vec_deque::IterMut::<u8>::default().len(), 0);
    let mut d = ring(8, 5, 7);
    for v in d.range_mut(2..6).unwrap() {
        *v += 10
    }
    assert_eq!(values(&d), [0, 1, 12, 13, 14, 15, 6]);
    let mut it = d.iter_mut();
    *it.next().unwrap() = 7;
    *it.next_back().unwrap() = 8;
    assert_eq!(it.try_len(), Ok(5));
    assert_eq!(format!("{it:?}"), "[1, 12, 13, 14, 15]");
    drop(it);
    {
        let mut clone = d.try_clone().unwrap();
        let mut drain = clone.drain(1..6).unwrap();
        assert_eq!(drain.next(), Some(1));
        assert_eq!(drain.next_back(), Some(15));
        assert_eq!(format!("{drain:?}"), "[12, 13, 14]");
        assert_eq!(drain.len(), 3);
    }
    let copy = d.try_clone().unwrap();
    let p = copy.as_slices().1.as_ptr();
    let v = copy.into_vec();
    assert_eq!(v.as_slice(), values(&d));
    assert_eq!(v.as_ptr(), p);
    let std = d.into_std();
    let mut d = VecDeque::try_from_std(std).unwrap();
    let budget = MemoryBudget::new(1024);
    let clone = d.try_clone_in(Budget::Limited(&budget)).unwrap();
    assert_eq!(values(&clone), values(&d));
    drop(clone);
    assert_eq!(budget.occupied(), 0);
    d.make_contiguous().sort();
    for v in d.iter() {
        assert!(d.binary_search(v).is_ok())
    }
    assert_eq!(d.partition_point(|v| *v < 12), 3);
    let mut iter = d.into_iter();
    assert!(iter.try_next().unwrap().is_some());
    assert!(iter.next_back().is_some());
    let copy = iter.try_clone().unwrap();
    assert_eq!(copy.collect::<Vec<_>>(), iter.collect::<Vec<_>>());
    let mut std: Std<_> = (0..8).collect();
    std.rotate_left(3);
    let (a, b) = std.as_slices();
    let expected = (a.to_vec(), b.to_vec());
    let small = MemoryBudget::new(1);
    let r = VecDeque::try_from_std_in(std, Budget::Limited(&small), Unlimited).unwrap_err();
    let (a, b) = r.value.as_slices();
    assert_eq!((a.to_vec(), b.to_vec()), expected);
    assert_eq!(small.occupied(), 0);
}

#[test]
fn copy_clone_from_reuses_storage() {
    let budget = MemoryBudget::new(64);
    let mut target = VecDeque::try_with_capacity_in(8, Budget::Limited(&budget)).unwrap();
    target.try_extend_copy(&[0u64, 1, 2, 3, 4, 5, 6]).unwrap();
    target.rotate_left(2).unwrap();
    let base = target.as_slices().1.as_ptr();
    let mut source = VecDeque::try_from_array_in([9u64, 10, 11], Budget::Unmetered).unwrap();
    source.rotate_left(1).unwrap();
    target.try_clone_from(&source).unwrap();
    assert_eq!(target.as_slices().0.as_ptr(), base);
    assert_eq!(target.capacity(), 8);
    assert_eq!(values(&target), [10, 11, 9]);
    assert_eq!((budget.occupied(), budget.reserved()), (64, 0));
    let larger = VecDeque::try_from_array_in([4u64; 9], Budget::Unmetered).unwrap();
    assert!(matches!(
        target.try_clone_from(&larger),
        Err(Error::Budget { .. })
    ));
    assert_eq!(values(&target), [10, 11, 9]);
    assert_eq!(target.as_slices().0.as_ptr(), base);
    assert_eq!(target.capacity(), 8);
    let mut bounded = VecDeque::with_limit(2);
    bounded.try_push_back(7u64).unwrap();
    let mut limited_source = VecDeque::with_limit(3);
    limited_source.try_extend_copy(&[1u64, 2, 3]).unwrap();
    assert!(bounded.try_clone_from(&limited_source).is_err());
    assert_eq!(values(&bounded), [7]);
    target
        .try_clone_from(&VecDeque::new_in(Budget::Unmetered))
        .unwrap();
    assert!(target.is_empty());
    assert_eq!((target.capacity(), budget.occupied()), (8, 64));
    #[repr(align(256))]
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Aligned(u64);
    impl TryClone for Aligned {
        fn try_clone(&self) -> Result<Self, Error> {
            Ok(*self)
        }
        fn copy_witness() -> Option<jzap::CopyWitness<Self>> {
            Some(jzap::CopyWitness::new())
        }
    }
    let mut aligned = VecDeque::try_from_array([Aligned(0); 3]).unwrap();
    aligned.rotate_left(2).unwrap();
    let mut source = VecDeque::try_from_array([Aligned(1), Aligned(2), Aligned(3)]).unwrap();
    source.rotate_left(1).unwrap();
    aligned.try_clone_from(&source).unwrap();
    assert_eq!(values(&aligned), [Aligned(2), Aligned(3), Aligned(1)]);
    let mut zero = VecDeque::try_from_array([(); 3]).unwrap();
    zero.try_clone_from(&VecDeque::try_from_array([(); 5]).unwrap())
        .unwrap();
    assert_eq!(zero.len(), 5);
    zero.try_clone_from(&VecDeque::new()).unwrap();
    assert!(zero.is_empty());
}

#[test]
fn empty_copy_extension_admission_and_wrap() {
    let bytes = 8 * core::mem::size_of::<usize>();
    let budget = MemoryBudget::new(bytes);
    let mut d = VecDeque::try_with_capacity_in(8, Budget::Limited(&budget)).unwrap();
    d.try_extend_copy(&[1usize, 2, 3]).unwrap();
    for _ in 0..3 {
        d.pop_front();
    }
    let before = d.as_slices().0.as_ptr();
    let base = d.as_slices().1.as_ptr();
    assert!(matches!(
        d.try_extend_copy(&[7; 9]),
        Err(Error::Budget { .. })
    ));
    assert_eq!(d.as_slices().0.as_ptr(), before);
    assert_eq!(
        (d.len(), d.capacity(), budget.occupied(), budget.reserved()),
        (0, 8, bytes, 0)
    );
    d.try_extend_copy(&[8, 9, 10, 11]).unwrap();
    assert_eq!(d.as_slices().0.as_ptr(), base);
    assert_eq!(values(&d), [8, 9, 10, 11]);
    d.rotate_left(3).unwrap();
    d.pop_back();
    d.pop_back();
    d.try_extend_copy(&[12, 13]).unwrap();
    assert_eq!(values(&d), [11, 8, 12, 13]);
    let before = (d.as_slices().0.as_ptr(), d.as_slices().1.as_ptr());
    d.try_extend_copy(&[]).unwrap();
    assert_eq!((d.as_slices().0.as_ptr(), d.as_slices().1.as_ptr()), before);
}

#[test]
fn no_drop_truncation_at_full_word_length() {
    let mut d = VecDeque::new();
    d.try_resize(usize::MAX, &()).unwrap();
    assert!(matches!(
        d.try_push_back(()),
        Err(jzap::Rejected {
            error: Error::Capacity,
            ..
        })
    ));
    assert_eq!(d.len(), usize::MAX);
    d.truncate(usize::MAX - 1);
    assert_eq!(d.len(), usize::MAX - 1);
    d.clear();
    assert!(d.is_empty());
    d.try_resize(usize::MAX, &()).unwrap();
    let mut drain = d.drain(1..usize::MAX - 1).unwrap();
    assert_eq!(drain.next(), Some(()));
    assert_eq!(drain.next_back(), Some(()));
    drop(drain);
    assert_eq!(d.len(), 2);
    d.try_resize(usize::MAX, &()).unwrap();
    drop(d.drain(..).unwrap());
    assert!(d.is_empty());
    #[derive(Debug)]
    struct NoDropNoCopy(u8);
    let mut values = VecDeque::try_from_array([NoDropNoCopy(1), NoDropNoCopy(2)]).unwrap();
    values.truncate(1);
    assert_eq!(values.front().unwrap().0, 1);
    values.clear();
    assert!(values.is_empty());
}
#[test]
fn zero_sized_alignment_and_forget() {
    #[derive(Debug)]
    #[repr(align(128))]
    struct Z;
    let mut d = VecDeque::with_limit(8);
    for _ in 0..8 {
        d.try_push_front(Z).unwrap()
    }
    assert!(d.try_push_back(Z).is_err());
    assert_eq!(d.capacity(), 8);
    assert!(d.as_slices().1.is_empty());
    d.rotate_left(3).unwrap();
    drop(d.remove(2));
    let mut iter = d.into_iter();
    drop(iter.next_back());
    assert_eq!(iter.count(), 6);
    let mut d = ring(8, 6, 7);
    std::mem::forget(d.drain(2..5).unwrap());
    assert_eq!(values(&d), [0, 1]);
    d.try_push_back(99).unwrap();
    assert_eq!(values(&d), [0, 1, 99]);
    assert_eq!(
        std::mem::size_of::<VecDeque<u8>>(),
        std::mem::size_of::<Std<u8>>()
    );
}

#[test]
fn allocation_ordinals_and_noncopy_policy_cleanup() {
    use jzap::budget::{OwnedBudget, OwnedConcurrentBudget};
    fn exercise<P: jzap::budget::AllocationPolicy>(policy: P) -> Result<(), Error> {
        let mut d = VecDeque::new_in(policy);
        for n in 0..19 {
            d.try_push_back(n).map_err(|r| r.error)?
        }
        for _ in 0..11 {
            d.pop_front();
        }
        for n in 19..39 {
            d.try_push_back(n).map_err(|r| r.error)?
        }
        d.try_reserve_exact(40)?;
        let mut copy = d.try_clone()?;
        let mut tail = copy.try_split_off(7)?;
        copy.try_append(&mut tail)?;
        copy.truncate(10);
        copy.try_shrink_to_fit()?;
        Ok(())
    }
    let owner = OwnedBudget::try_new(usize::MAX).unwrap();
    let (ok, calls) = common::track(0, || exercise(owner.try_clone().unwrap()));
    ok.unwrap();
    assert_eq!((owner.occupied(), owner.reserved()), (0, 0));
    for ordinal in 1..=calls.calls {
        let (result, _) = common::track(ordinal, || exercise(owner.try_clone().unwrap()));
        assert!(
            matches!(result, Err(Error::Allocation { .. })),
            "allocation {ordinal}/{calls:?}"
        );
        assert_eq!((owner.occupied(), owner.reserved()), (0, 0));
    }
    let owner = OwnedConcurrentBudget::try_new(usize::MAX).unwrap();
    exercise(owner.try_clone().unwrap()).unwrap();
    assert_eq!(owner.used(), 0);
    let mut d = ring(8, 5, 6);
    let ptr = d.as_slices().1.as_ptr() as usize;
    let (_, frees) = deallocations::deallocations_during(ptr, || {
        d.clear();
        d.try_shrink_to_fit().unwrap();
    });
    assert_eq!(frees, 1);
}

#[test]
fn consuming_fold_retires_current_on_callback_unwind() {
    let (mut d, drops) = tokens(8);
    d.rotate_left(5).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        d.into_iter().fold(0, |n, v| {
            if n == 3 {
                panic!("user callback")
            }
            drop(v);
            n + 1
        })
    }));
    assert!(result.is_err());
    assert!(drops.iter().all(|n| n.get() == 1));
    let (mut d, drops) = tokens(7);
    d.rotate_left(3).unwrap();
    assert_eq!(d.into_iter().count(), 7);
    assert!(drops.iter().all(|n| n.get() == 1));
    let (d, drops) = tokens(6);
    let value = d.into_iter().last().unwrap();
    assert_eq!(value.index, 5);
    drop(value);
    assert!(drops.iter().all(|n| n.get() == 1));
}

#[test]
fn array_witness_alignment_and_in_place_clone_errors() {
    assert!(<[u64; 32] as TryClone>::copy_witness().is_some());
    assert!(<[[u64; 4]; 8] as TryClone>::copy_witness().is_some());
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    #[repr(align(256))]
    struct Aligned([u8; 3]);
    impl TryClone for Aligned {
        fn try_clone(&self) -> Result<Self, Error> {
            Ok(*self)
        }
        fn copy_witness() -> Option<jzap::CopyWitness<Self>> {
            Some(jzap::CopyWitness::new())
        }
    }
    let mut d = VecDeque::try_with_capacity(5).unwrap();
    for i in 0..5 {
        d.try_push_front(Aligned([i, 1, 2])).unwrap()
    }
    d.rotate_left(2).unwrap();
    let expected = values(&d);
    assert_eq!(d.make_contiguous(), expected);
    d.rotate_right(2).unwrap();
    d.pop_back();
    d.try_resize(9, &Aligned([99, 1, 2])).unwrap();
    assert_eq!(d.get(0).unwrap(), &Aligned([4, 1, 2]));
    assert_eq!(d.back(), Some(&Aligned([99, 1, 2])));
    let clone = d.try_clone().unwrap();
    assert_eq!(clone, d);
    assert_eq!(clone.as_slices().0.as_ptr() as usize % 256, 0);
    let mut z = VecDeque::with_limit(8);
    z.try_resize(7, &()).unwrap();
    assert_eq!(z.len(), 7);
    assert_eq!(z.try_resize(9, &()), Err(Error::Capacity));
    assert_eq!(z.len(), 7);
    #[derive(Debug)]
    struct Fallible {
        fail: bool,
        live: Rc<Cell<usize>>,
    }
    impl TryClone for Fallible {
        fn try_clone(&self) -> Result<Self, Error> {
            if self.fail {
                return Err(Error::Bounds);
            }
            self.live.set(self.live.get() + 1);
            Ok(Self {
                fail: false,
                live: self.live.clone(),
            })
        }
    }
    impl Drop for Fallible {
        fn drop(&mut self) {
            self.live.set(self.live.get() - 1)
        }
    }
    let live = Rc::new(Cell::new(4));
    let source = std::array::from_fn::<_, 4, _>(|i| Fallible {
        fail: i == 2,
        live: live.clone(),
    });
    let d = VecDeque::try_from_array([source]).unwrap();
    assert!(<[Fallible; 4] as TryClone>::copy_witness().is_none());
    assert!(matches!(d.try_clone(), Err(Error::Bounds)));
    assert_eq!(live.get(), 4);
    drop(d);
    assert_eq!(live.get(), 0);
    #[derive(Debug)]
    struct NoWrite;
    impl TryClone for NoWrite {
        fn try_clone(&self) -> Result<Self, Error> {
            Ok(Self)
        }
        fn try_clone_into(&self, _: &mut jzap::init::InitSlot<'_, Self>) -> Result<(), Error> {
            Ok(())
        }
    }
    let d = VecDeque::try_from_array([NoWrite]).unwrap();
    assert!(matches!(d.try_clone(), Err(Error::Uninitialized)));
}

#[cfg(not(miri))]
proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(96))]
    #[test]
    fn bounded_random_sequences_preserve_std_contents(limit in 0usize..33,ops in proptest::collection::vec(proptest::arbitrary::any::<u16>(),0..128)) {
        let mut d=VecDeque::with_limit(limit);let mut model=Std::new();
        for word in ops {
            let n=word as usize%(model.len()+2);
            match word%8 {
                0|1=>{let r=if word%8==0 {d.try_push_back(word)}else{d.try_push_front(word)};
                    if model.len()==limit {let rejected=r.unwrap_err();proptest::prop_assert_eq!(rejected.error,Error::Capacity);proptest::prop_assert_eq!(rejected.value,word);}
                    else {r.unwrap();if word%8==0 {model.push_back(word)}else{model.push_front(word)}}
                },
                2=>proptest::prop_assert_eq!(d.pop_front(),model.pop_front()),
                3=>proptest::prop_assert_eq!(d.pop_back(),model.pop_back()),
                4=>{let r=d.rotate_left(n);if n>model.len(){proptest::prop_assert_eq!(r,Err(Error::Bounds))}else{r.unwrap();model.rotate_left(n)}},
                5=>{let new_len=word as usize%(limit+2);let r=d.try_resize(new_len,&word);
                    if new_len>limit {proptest::prop_assert_eq!(r,Err(Error::Capacity))}else{r.unwrap();model.resize(new_len,word)}},
                6=>{d.make_contiguous();},
                _=>{d.try_shrink_to_fit().unwrap();},
            }
            proptest::prop_assert!(d.len()<=d.capacity()&&d.capacity()<=limit);
            proptest::prop_assert_eq!(values(&d),model.iter().copied().collect::<Vec<_>>());
        }
    }
}

#[test]
fn traits_wrapped_ranges_and_search() {
    use jzap::TryDefault;
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };
    fn hash(value: &impl Hash) -> u64 {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }
    let mut d = ring(8, 6, 6);
    let contiguous = VecDeque::try_from_array([0usize, 1, 2, 3, 4, 5]).unwrap();
    let mut standard: Std<_> = (0usize..6).collect();
    assert_eq!(hash(&d), hash(&contiguous));
    assert_eq!(format!("{d:?}"), format!("{standard:?}"));
    assert_eq!(d.partial_cmp(&contiguous), Some(std::cmp::Ordering::Equal));
    assert_eq!(d.cmp(&contiguous), std::cmp::Ordering::Equal);
    assert!(d == [0, 1, 2, 3, 4, 5]);
    assert!(d == &[0, 1, 2, 3, 4, 5][..]);
    assert!(d == vec![0, 1, 2, 3, 4, 5]);
    let owned = jzap::vec::Vec::try_from_array([0usize, 1, 2, 3, 4, 5]).unwrap();
    assert!(d == owned);
    let mut array = [0usize, 1, 2, 3, 4, 5];
    assert!(d == &array);
    assert!(d == &mut array);
    assert!(VecDeque::<u64>::default().is_empty());
    assert!(VecDeque::<u64>::try_default().unwrap().is_empty());
    for (a, b) in [(0, 1), (2, 4), (1, 5)] {
        for value in d.range_mut(a..b).unwrap() {
            *value += 10;
        }
        for value in standard.range_mut(a..b) {
            *value += 10;
        }
    }
    *d.range_mut(0..1).unwrap().next_back().unwrap() += 1;
    *standard.get_mut(0).unwrap() += 1;
    assert_eq!(values(&d), standard.into_iter().collect::<Vec<_>>());
    assert!(d.range_mut(7..9).is_err());
    assert!(d.range_mut(2..=usize::MAX).is_err());
    assert!(d.drain(7..9).is_err());
    assert!(d.get_mut(9).is_err());
    let sorted = VecDeque::try_from_array([1u64, 3, 5]).unwrap();
    assert_eq!(sorted.binary_search(&4), Err(2));
    assert_eq!(sorted.partition_point(|n| *n <= 3), 2);
    let mut empty = VecDeque::<u64>::new();
    assert!(empty.front_mut().is_none() && empty.back_mut().is_none() && empty.back().is_none());
    assert_eq!(empty.try_pop_front_if(|_| Err::<bool, _>(())), Ok(None));
    empty.try_push_back(9).unwrap();
    assert_eq!(empty.try_pop_front_if(|_| Ok::<_, ()>(false)), Ok(None));
    assert_eq!(empty.try_pop_front_if(|_| Ok::<_, ()>(true)), Ok(Some(9)));
}

#[test]
fn noncopy_clone_refusals_and_deep_budget() {
    use jzap::string::String as Text;
    let mut source = VecDeque::try_from_array([
        Text::try_from_str("alpha").unwrap(),
        Text::try_from_str("beta").unwrap(),
        Text::try_from_str("gamma").unwrap(),
    ])
    .unwrap();
    source.rotate_left(1).unwrap();
    let expected = ["beta", "gamma", "alpha"];
    let mut target = VecDeque::try_from_array([Text::try_from_str("old").unwrap()]).unwrap();
    let (result, stats) = common::track(0, || target.try_clone_from(&source));
    result.unwrap();
    assert_eq!(
        target.iter().map(Text::as_str).collect::<Vec<_>>(),
        expected
    );
    for ordinal in 1..=stats.calls {
        let mut target = VecDeque::try_from_array([Text::try_from_str("old").unwrap()]).unwrap();
        let pointer = target.as_slices().0.as_ptr();
        let (result, live) = common::track(ordinal, || target.try_clone_from(&source));
        assert!(matches!(result, Err(Error::Allocation { .. })));
        assert_eq!(target.front().unwrap().as_str(), "old");
        assert_eq!(target.len(), 1);
        assert_eq!(target.as_slices().0.as_ptr(), pointer);
        assert_eq!(live.live, 0);
    }
    let fill = Text::try_from_str("extra").unwrap();
    let mut resized = source.try_clone().unwrap();
    resized.try_resize(6, &fill).unwrap();
    assert_eq!(resized.len(), 6);
    resized.try_resize_with(2, || Err::<Text, _>(())).unwrap();
    assert_eq!(
        resized.iter().map(Text::as_str).collect::<Vec<_>>(),
        &expected[..2]
    );
    resized
        .try_resize_with(5, || Text::try_from_str("new"))
        .unwrap();
    assert_eq!(resized.back().unwrap().as_str(), "new");
    resized.truncate(2);
    assert_eq!(resized.len(), 2);
    let mut owned = source.try_clone().unwrap().into_iter();
    drop(owned.next());
    let copied = owned.try_clone().unwrap();
    assert_eq!(
        copied.map(|v| v.as_str().to_owned()).collect::<Vec<_>>(),
        &expected[1..]
    );
    let budget = MemoryBudget::new(4096);
    let clone = source.try_clone_in(Budget::Limited(&budget)).unwrap();
    assert!(budget.occupied() > clone.capacity() * std::mem::size_of_val(clone.front().unwrap()));
    assert_eq!(
        clone.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
        expected
    );
    drop(clone);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
    let denied = MemoryBudget::new(0);
    assert!(matches!(
        source.try_clone_in(Budget::Limited(&denied)),
        Err(Error::Budget { .. })
    ));
    let (_, stats) = common::track(0, || {
        drop(source.try_clone_in(Budget::Limited(&budget)).unwrap())
    });
    for ordinal in 1..=stats.calls {
        let (result, live) =
            common::track(ordinal, || source.try_clone_in(Budget::Limited(&budget)));
        assert!(matches!(result, Err(Error::Allocation { .. })));
        assert_eq!((live.live, budget.occupied(), budget.reserved()), (0, 0, 0));
    }
    assert_eq!(
        source.iter().map(Text::as_str).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn admission_and_std_transfer_preserve_owners() {
    use jzap::vec::LengthLimit;
    assert!(matches!(
        VecDeque::<u64>::try_with_capacity_and_limit(3, 2),
        Err(Error::Capacity)
    ));
    let denied = MemoryBudget::new(0);
    let mut empty = VecDeque::new_in(Budget::Limited(&denied));
    let refused = empty.try_insert(0, 77u64).unwrap_err();
    assert_eq!(refused.value, 77);
    assert!(matches!(refused.error, Error::Budget { .. }));
    assert!(empty.is_empty());
    let left = MemoryBudget::new(32);
    let right = MemoryBudget::new(128);
    let mut a = VecDeque::try_from_array_in([1u64, 2, 3, 4], Budget::Limited(&left)).unwrap();
    let mut b = VecDeque::try_from_array_in([5u64, 6, 7], Budget::Limited(&right)).unwrap();
    assert!(matches!(a.try_append(&mut b), Err(Error::Budget { .. })));
    assert_eq!(values(&a), [1, 2, 3, 4]);
    assert_eq!(values(&b), [5, 6, 7]);
    assert_eq!((left.occupied(), right.occupied()), (32, 24));
    assert!(a.try_split_off(4).unwrap().is_empty());
    assert!(matches!(a.try_split_off(5), Err(Error::Bounds)));
    let mut original = Std::with_capacity(8);
    original.extend([1u64, 2, 3]);
    original.rotate_left(1);
    let before = (
        original.as_slices().0.as_ptr(),
        original.as_slices().1.as_ptr(),
    );
    let refusal =
        VecDeque::try_from_std_in(original, Budget::Limited(&denied), LengthLimit::new(3))
            .unwrap_err();
    assert_eq!(refusal.error, Error::Capacity);
    assert_eq!(
        (
            refusal.value.as_slices().0.as_ptr(),
            refusal.value.as_slices().1.as_ptr()
        ),
        before
    );
    let ledger = MemoryBudget::new(128);
    let standard = Std::from([4u64, 5, 6]);
    let capacity = standard.capacity();
    let adopted = VecDeque::try_from_std_in(standard, Budget::Limited(&ledger), Unlimited).unwrap();
    assert_eq!(ledger.occupied(), capacity * 8);
    let standard = adopted.into_std();
    assert_eq!((ledger.occupied(), ledger.reserved()), (0, 0));
    assert_eq!(standard, [4, 5, 6]);
    let mut z = Std::new();
    z.push_front(());
    z.push_front(());
    let mut z = VecDeque::try_from_std_in(z, Budget::Unmetered, LengthLimit::new(3)).unwrap();
    assert_eq!((z.len(), z.capacity()), (2, 3));
    z.try_push_back(()).unwrap();
    assert!(z.try_push_back(()).is_err());
    let mut tiny = VecDeque::<u8>::new();
    tiny.try_push_back(1).unwrap();
    let mut std_tiny = Std::new();
    std_tiny.push_back(1u8);
    assert_eq!(tiny.capacity(), std_tiny.capacity());
    let mut big = VecDeque::<[u8; 2048]>::new();
    big.try_push_back([0; 2048]).unwrap();
    let mut std_big = Std::new();
    std_big.push_back([0u8; 2048]);
    assert_eq!(big.capacity(), std_big.capacity());
}

#[test]
fn drain_unwind_restores_tail_once() {
    #[derive(Debug)]
    struct OnDrop {
        index: usize,
        counts: Rc<Vec<Cell<usize>>>,
        fail: Rc<Cell<bool>>,
    }
    impl Drop for OnDrop {
        fn drop(&mut self) {
            self.counts[self.index].set(self.counts[self.index].get() + 1);
            if self.index == 2 && self.fail.replace(false) {
                panic!("user destructor");
            }
        }
    }
    let counts: Rc<Vec<Cell<usize>>> = Rc::new((0..6).map(|_| Cell::new(0)).collect());
    let fail = Rc::new(Cell::new(true));
    let mut d = VecDeque::new();
    for index in 0..6 {
        d.try_push_back(OnDrop {
            index,
            counts: counts.clone(),
            fail: fail.clone(),
        })
        .unwrap();
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drop(d.drain(1..5).unwrap())
    }));
    assert!(result.is_err());
    assert_eq!(d.iter().map(|v| v.index).collect::<Vec<_>>(), [0, 5]);
    assert_eq!(
        counts.iter().map(Cell::get).collect::<Vec<_>>(),
        [0, 1, 1, 1, 1, 0]
    );
    drop(d);
    assert!(counts.iter().all(|v| v.get() == 1));
}
