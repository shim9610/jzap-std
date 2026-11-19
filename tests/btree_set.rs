mod common;
use common::{FaultAllocator, track};
use jzap::{
    Error, TryClone, TryCloneIn, TryIterator,
    budget::{Budget, MemoryBudget, OwnedBudget, OwnedConcurrentBudget},
    collections::BTreeSet,
};
use std::{cell::Cell, cmp::Ordering, collections::BTreeSet as Model, rc::Rc};
#[global_allocator]
static ALLOC: FaultAllocator = FaultAllocator;
fn random(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn set(values: impl IntoIterator<Item = i32>) -> BTreeSet<i32> {
    let mut s = BTreeSet::new();
    for v in values {
        s.try_insert(v).unwrap();
    }
    s
}
#[test]
fn differential_edits_ranges_and_alternating_frontiers() {
    let (mut actual, mut model) = (BTreeSet::new(), Model::new());
    let mut seed = 37112026;
    for _ in 0..if cfg!(miri) { 240 } else { 20000 } {
        let value = (random(&mut seed) % 300) as i32;
        match random(&mut seed) % 7 {
            0 | 1 => assert_eq!(actual.try_insert(value).unwrap(), model.insert(value)),
            2 => assert_eq!(actual.try_replace(value).unwrap(), model.replace(value)),
            3 => assert_eq!(actual.take(&value), model.take(&value)),
            4 => assert_eq!(actual.remove(&value), model.remove(&value)),
            5 => assert_eq!(actual.pop_first(), model.pop_first()),
            _ => assert_eq!(actual.pop_last(), model.pop_last()),
        }
        assert_eq!(actual.len(), model.len());
        assert_eq!(actual.first(), model.first());
        assert_eq!(actual.last(), model.last());
        assert!(actual.iter().eq(model.iter()));
        assert!(actual.iter().rev().eq(model.iter().rev()));
        assert!(actual.try_range(value..).unwrap().eq(model.range(value..)));
    }
    let (mut a, mut b) = (actual.into_iter(), model.into_iter());
    while b.len() != 0 {
        assert_eq!(a.next(), b.next());
        assert_eq!(a.next_back(), b.next_back());
        assert_eq!(a.as_iter().len(), b.len());
    }
    assert_eq!(a.next(), None);
    assert_eq!(a.next_back(), None);
}
#[test]
fn lazy_algebra_matches_std_and_bounds_after_each_yield() {
    let masks = if cfg!(miri) { 8 } else { 32 };
    for a in 0..masks {
        for b in 0..masks {
            let (av, bv): (Vec<_>, Vec<_>) = (
                (0..5).filter(|i| a & (1 << i) != 0).collect(),
                (0..5).filter(|i| b & (1 << i) != 0).collect(),
            );
            let (sa, sb) = (set(av.iter().copied()), set(bv.iter().copied()));
            let (ma, mb) = (Model::from_iter(av), Model::from_iter(bv));
            assert!(sa.union(&sb).eq(ma.union(&mb)));
            assert!(sa.intersection(&sb).eq(ma.intersection(&mb)));
            assert!(sa.difference(&sb).eq(ma.difference(&mb)));
            assert!(
                sa.symmetric_difference(&sb)
                    .eq(ma.symmetric_difference(&mb))
            );
            assert_eq!(sa.is_subset(&sb), ma.is_subset(&mb));
            assert_eq!(sa.is_superset(&sb), ma.is_superset(&mb));
            assert_eq!(sa.is_disjoint(&sb), ma.is_disjoint(&mb));
            fn check<'a>(mut it: impl Iterator<Item = &'a i32> + Clone) {
                loop {
                    let remaining = it.clone().count();
                    let (lo, hi) = it.size_hint();
                    assert!(lo <= remaining);
                    assert!(hi.is_none_or(|h| remaining <= h));
                    if it.next().is_none() {
                        break;
                    }
                }
                assert!(it.next().is_none());
            }
            check(sa.union(&sb));
            check(sa.intersection(&sb));
            check(sa.difference(&sb));
            check(sa.symmetric_difference(&sb));
        }
    }
}
#[test]
fn algebra_search_stitch_disjoint_and_touching_endpoints() {
    let count = if cfg!(miri) { 48 } else { 800 };
    let corpora = [
        vec![],
        vec![0],
        vec![count],
        vec![count + 1],
        vec![0, count],
        vec![1, 3, 7],
        (0..count).collect(),
        (1..=count).collect(),
        (count..count * 2).collect(),
    ];
    for av in &corpora {
        for bv in &corpora {
            let (sa, sb) = (set(av.iter().copied()), set(bv.iter().copied()));
            let (ma, mb) = (
                Model::from_iter(av.iter().copied()),
                Model::from_iter(bv.iter().copied()),
            );
            assert!(sa.union(&sb).eq(ma.union(&mb)));
            assert!(sa.intersection(&sb).eq(ma.intersection(&mb)));
            assert!(sa.difference(&sb).eq(ma.difference(&mb)));
            assert!(
                sa.symmetric_difference(&sb)
                    .eq(ma.symmetric_difference(&mb))
            );
            assert_eq!(sa.is_subset(&sb), ma.is_subset(&mb));
            assert_eq!(sa.is_disjoint(&sb), ma.is_disjoint(&mb));
        }
    }
}
#[test]
fn insertion_allocation_ordinals_return_input_and_allow_retry() {
    let count = if cfg!(miri) { 36 } else { 240 };
    let (_, baseline) = track(usize::MAX, || {
        let mut s = BTreeSet::new();
        for i in 0..count {
            s.try_insert(i).unwrap();
        }
    });
    assert_eq!(baseline.live, 0);
    for fail in 1..=baseline.calls {
        let budget = MemoryBudget::new(usize::MAX);
        let (_, record) = track(fail, || {
            let mut s = BTreeSet::new_in(Budget::Limited(&budget));
            let mut refused = false;
            for i in 0..count {
                let used = budget.occupied();
                if let Err(e) = s.try_insert(i) {
                    assert_eq!(e.value, i);
                    assert!(matches!(e.error, Error::Allocation { .. }));
                    assert_eq!(s.len(), i as usize);
                    assert!(s.iter().copied().eq(0..i));
                    assert_eq!(used, budget.occupied());
                    refused = true;
                    s.try_insert(e.value).unwrap();
                }
            }
            assert!(refused);
        });
        assert_eq!(record.live, 0);
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
}
#[derive(Debug)]
struct Key {
    i: i32,
    tag: i32,
    drops: Rc<Cell<usize>>,
}
impl PartialEq for Key {
    fn eq(&self, b: &Self) -> bool {
        self.i == b.i
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Key {
    fn cmp(&self, b: &Self) -> Ordering {
        self.i.cmp(&b.i)
    }
}
impl std::borrow::Borrow<i32> for Key {
    fn borrow(&self) -> &i32 {
        &self.i
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1)
    }
}
#[test]
fn duplicate_and_replacement_preserve_object_identity_without_allocation() {
    let drops = Rc::new(Cell::new(0));
    let key = |tag| Key {
        i: 7,
        tag,
        drops: drops.clone(),
    };
    let mut s = BTreeSet::new();
    s.try_insert(key(1)).unwrap();
    let (a, record) = track(1, || s.try_insert(key(2)).unwrap());
    assert!(!a);
    assert_eq!(record.calls, 0);
    assert_eq!(s.get(&7).unwrap().tag, 1);
    assert_eq!(drops.get(), 1);
    let (old, record) = track(1, || s.try_replace(key(3)).unwrap().unwrap());
    assert_eq!(record.calls, 0);
    assert_eq!(old.tag, 1);
    assert_eq!(s.get(&7).unwrap().tag, 3);
    drop(old);
    let stored = s.take(&7).unwrap();
    assert_eq!(stored.tag, 3);
    drop(stored);
    assert_eq!(drops.get(), 3);
    let (mut a, mut b) = (BTreeSet::new(), BTreeSet::new());
    a.try_insert(key(4)).unwrap();
    b.try_insert(key(5)).unwrap();
    a.try_append(&mut b).unwrap();
    assert_eq!(a.get(&7).unwrap().tag, 4);
    assert!(b.is_empty());
    drop(a);
    drop(b);
    assert_eq!(drops.get(), 5);
}
#[test]
fn missing_replacement_failure_preserves_set_and_input() {
    let (mut s, mut model) = (set(0..11), Model::from_iter(0..11));
    let (result, record) = track(1, || s.try_replace(20));
    let e = result.unwrap_err();
    assert_eq!(e.value, 20);
    assert!(matches!(e.error, Error::Allocation { .. }));
    assert_eq!(record.live, 0);
    assert!(s.iter().eq(model.iter()));
    assert_eq!(s.try_replace(e.value).unwrap(), model.replace(20));
    assert!(s.iter().eq(model.iter()));
}
#[test]
fn checked_range_and_heterogeneous_lookup() {
    use core::ops::Bound::*;
    let s = set(0..10);
    assert!(matches!(s.try_range(8..2), Err(Error::Bounds)));
    assert!(matches!(
        s.try_range((Excluded(3), Excluded(3))),
        Err(Error::Bounds)
    ));
    assert!(
        s.try_range((Included(3), Included(3)))
            .unwrap()
            .copied()
            .eq([3])
    );
    let mut strings = BTreeSet::try_from_array(["alpha".to_owned(), "beta".to_owned()]).unwrap();
    assert!(strings.contains("alpha"));
    assert_eq!(strings.get("beta").map(String::as_str), Some("beta"));
    assert_eq!(strings.take("alpha").as_deref(), Some("alpha"));
}
#[test]
fn retain_and_extraction_errors_preserve_current_candidate() {
    let mut s = set(0..12);
    let err = s.try_retain(|v| if *v == 5 { Err(91) } else { Ok(v % 2 == 0) });
    assert_eq!(err, Err(91));
    assert!(s.iter().copied().eq([0, 2, 4, 5, 6, 7, 8, 9, 10, 11]));
    let mut ex = s
        .try_extract_if(2..10, |v| if *v == 5 { Err(92) } else { Ok(v % 2 == 0) })
        .unwrap();
    assert_eq!(ex.try_next(), Ok(Some(2)));
    assert_eq!(ex.try_next(), Ok(Some(4)));
    assert_eq!(ex.try_next(), Err(92));
    assert_eq!(ex.try_next(), Ok(Some(6)));
    drop(ex);
    assert!(s.iter().copied().eq([0, 5, 7, 8, 9, 10, 11]));
}
#[test]
fn split_append_allocation_failures_preserve_both_inputs() {
    let count = if cfg!(miri) { 24 } else { 120 };
    let mut refusals = 0;
    for fail in 1..=12 {
        let mut s = set(0..count);
        let (result, record) = track(fail, || s.try_split_off(&(count / 2)));
        match result {
            Err(Error::Allocation { .. }) => {
                refusals += 1;
                assert!(s.iter().copied().eq(0..count));
                assert_eq!(record.live, 0);
            }
            Ok(t) => {
                assert!(s.iter().copied().eq(0..count / 2));
                assert!(t.iter().copied().eq(count / 2..count));
            }
            Err(e) => panic!("unexpected {e:?}"),
        }
    }
    assert!(refusals > 0);
    let b1 = MemoryBudget::new(usize::MAX);
    let b2 = MemoryBudget::new(0);
    let mut a = BTreeSet::new_in(Budget::Limited(&b2));
    let mut b = BTreeSet::new_in(Budget::Limited(&b1));
    for i in 0..count {
        b.try_insert(i).unwrap();
    }
    let charge = b1.occupied();
    assert!(matches!(a.try_append(&mut b), Err(Error::Budget { .. })));
    assert!(a.is_empty());
    assert!(b.iter().copied().eq(0..count));
    assert_eq!(b1.occupied(), charge);
    assert_eq!(b2.occupied(), 0);
}
#[test]
fn clone_and_owned_algebra_failures_cleanup_without_changing_inputs() {
    let (a, b) = (set(0..25), set(15..40));
    for fail in 1..=10 {
        for op in 0..5 {
            let (result, record) = track(fail, || match op {
                0 => a.try_clone(),
                1 => a.try_union(&b),
                2 => a.try_intersection(&b),
                3 => a.try_difference(&b),
                _ => a.try_symmetric_difference(&b),
            });
            if let Err(e) = result {
                assert!(matches!(e, Error::Allocation { .. }));
                assert_eq!(record.live, 0);
            }
            assert!(a.iter().copied().eq(0..25));
            assert!(b.iter().copied().eq(15..40));
        }
    }
    let mut destination = set(80..90);
    let (_, record) = track(1, || assert!(destination.try_clone_from(&a).is_err()));
    assert_eq!(record.live, 0);
    assert!(destination.iter().copied().eq(80..90));
}
#[test]
fn payload_clone_refusal_cleans_prefix_and_preserves_destination() {
    #[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Refuses(u8);
    impl TryClone for Refuses {
        fn try_clone(&self) -> Result<Self, Error> {
            if self.0 == 3 {
                Err(Error::Capacity)
            } else {
                Ok(Self(self.0))
            }
        }
    }
    let source = BTreeSet::try_from_array([Refuses(1), Refuses(3), Refuses(5)]).unwrap();
    let mut old = BTreeSet::try_from_array([Refuses(9)]).unwrap();
    assert_eq!(source.try_clone().unwrap_err(), Error::Capacity);
    assert_eq!(old.try_clone_from(&source), Err(Error::Capacity));
    assert_eq!(old.first(), Some(&Refuses(9)));
}
#[test]
fn owned_policies_outlive_creator_and_target_clone_uses_destination_budget() {
    let mut local = {
        let p = OwnedBudget::try_new(4096).unwrap();
        let mut s = BTreeSet::new_in(p.try_clone().unwrap());
        s.try_insert(7).unwrap();
        s
    };
    let b = MemoryBudget::new(4096);
    let copy = local.try_clone_in(Budget::Limited(&b)).unwrap();
    assert_eq!(copy.first(), Some(&7));
    assert!(b.occupied() > 0);
    drop(copy);
    assert_eq!(b.occupied(), 0);
    local.try_replace(9).unwrap();
    let concurrent = {
        let p = OwnedConcurrentBudget::try_new(4096).unwrap();
        let mut s = BTreeSet::new_in(p.try_clone().unwrap());
        s.try_insert(11).unwrap();
        s
    };
    if !cfg!(miri) {
        std::thread::spawn(move || assert_eq!(concurrent.first(), Some(&11)))
            .join()
            .unwrap();
    }
}
#[test]
fn fallible_source_error_keeps_prefix_and_distinguishes_insert_refusal() {
    struct Source(u8);
    impl TryIterator for Source {
        type Item = i32;
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<i32>, u8> {
            self.0 += 1;
            if self.0 == 3 {
                Err(42)
            } else {
                Ok(Some(i32::from(self.0)))
            }
        }
    }
    let mut s = set([0]);
    assert!(matches!(
        s.try_extend(Source(0)),
        Err(jzap::collections::btree_set::ExtendError::Source(42))
    ));
    assert!(s.iter().copied().eq([0, 1, 2]));
    let b = MemoryBudget::new(0);
    let mut empty = BTreeSet::new_in(Budget::Limited(&b));
    match empty.try_extend([7]) {
        Err(jzap::collections::btree_set::ExtendError::Insert(e)) => assert_eq!(e.value, 7),
        _ => panic!("missing rejection"),
    }
}
#[test]
fn iterator_traits_debug_order_and_hash_match_contents() {
    use std::hash::{Hash, Hasher};
    let a = set([3, 1, 2]);
    let b = Model::from([1, 2, 3]);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    let mut ah = std::collections::hash_map::DefaultHasher::new();
    let mut bh = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut ah);
    b.hash(&mut bh);
    assert_eq!(ah.finish(), bh.finish());
    assert_eq!(a.cmp(&set([1, 2, 4])), Ordering::Less);
    assert_eq!(a.partial_cmp(&set([1, 2, 3])), Some(Ordering::Equal));
    let mut iter = a.iter();
    assert_eq!(iter.try_next().unwrap(), Some(&1));
    assert_eq!(
        jzap::TryDoubleEndedIterator::try_next_back(&mut iter).unwrap(),
        Some(&3)
    );
    assert_eq!(jzap::TryExactSizeIterator::try_len(&iter).unwrap(), 1);
    let b = set([2, 4]);
    assert_eq!(format!("{:?}", a.union(&b)), "[1, 2, 3, 4]");
    assert_eq!(format!("{:?}", a.intersection(&b)), "[2]");
    let empty = jzap::collections::btree_set::Iter::<i32>::default();
    assert_eq!(empty.len(), 0);
    assert!(
        jzap::collections::btree_set::Range::<i32>::default()
            .next()
            .is_none()
    );
}
#[test]
fn partial_owned_iteration_and_extraction_drop_each_object_once() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut s = BTreeSet::new();
        for i in 0..if cfg!(miri) { 24 } else { 144 } {
            s.try_insert(Key {
                i,
                tag: 0,
                drops: drops.clone(),
            })
            .unwrap();
        }
        let len = s.len();
        let mut it = s.into_iter();
        drop(it.next());
        drop(it.next_back());
        assert_eq!(it.len(), len - 2);
        assert_eq!(it.as_iter().len(), len - 2);
        drop(it);
        assert_eq!(drops.get(), len);
    }
}
#[test]
fn over_aligned_and_zero_sized_keys_are_owned_once() {
    #[repr(align(256))]
    #[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Aligned(u8);
    let mut s = BTreeSet::new();
    for i in 0..20 {
        s.try_insert(Aligned(i)).unwrap();
    }
    assert_eq!(s.try_replace(Aligned(10)).unwrap(), Some(Aligned(10)));
    assert_eq!(s.into_iter().count(), 20);
    let mut z = BTreeSet::new();
    assert!(z.try_insert(()).unwrap());
    assert!(!z.try_insert(()).unwrap());
    assert_eq!(z.try_replace(()).unwrap(), Some(()));
    assert_eq!(z.pop_last(), Some(()));
}

#[test]
fn packed_algebra_refuses_every_allocation_and_retries_without_leaks() {
    let n = if cfg!(miri) { 144 } else { 360 };
    let (a, b) = (set(0..n), set(n / 2..n + n / 2));
    let build = |op| match op {
        0 => a.try_union(&b),
        1 => a.try_intersection(&b),
        2 => a.try_difference(&b),
        _ => a.try_symmetric_difference(&b),
    };
    for op in 0..4 {
        let (_, baseline) = track(usize::MAX, || drop(build(op).unwrap()));
        assert_eq!(baseline.live, 0);
        for fail in 1..=baseline.calls {
            let (error, record) = track(fail, || build(op).unwrap_err());
            assert!(matches!(error, Error::Allocation { .. }));
            assert_eq!(record.live, 0);
            assert!(a.iter().copied().eq(0..n));
            assert!(b.iter().copied().eq(n / 2..n + n / 2));
        }
        drop(build(op).unwrap());
    }
}

#[test]
fn packed_prefix_clone_errors_drop_underfull_pillars_exactly_once() {
    #[derive(Debug)]
    struct State {
        attempt: Cell<usize>,
        fail: Cell<usize>,
        drops: Cell<usize>,
    }
    #[derive(Debug)]
    struct Item {
        key: u16,
        state: Rc<State>,
    }
    impl PartialEq for Item {
        fn eq(&self, b: &Self) -> bool {
            self.key == b.key
        }
    }
    impl Eq for Item {}
    impl PartialOrd for Item {
        fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
            Some(self.cmp(b))
        }
    }
    impl Ord for Item {
        fn cmp(&self, b: &Self) -> Ordering {
            self.key.cmp(&b.key)
        }
    }
    impl Drop for Item {
        fn drop(&mut self) {
            self.state.drops.set(self.state.drops.get() + 1);
        }
    }
    impl TryClone for Item {
        fn try_clone(&self) -> Result<Self, Error> {
            let n = self.state.attempt.get() + 1;
            self.state.attempt.set(n);
            if n == self.state.fail.get() {
                return Err(Error::Capacity);
            }
            Ok(Self {
                key: self.key,
                state: self.state.clone(),
            })
        }
    }
    let state = Rc::new(State {
        attempt: Cell::new(0),
        fail: Cell::new(usize::MAX),
        drops: Cell::new(0),
    });
    let (mut a, mut b) = (BTreeSet::new(), BTreeSet::new());
    for key in 0..160 {
        a.try_insert(Item {
            key,
            state: state.clone(),
        })
        .unwrap();
    }
    for key in 120..200 {
        b.try_insert(Item {
            key,
            state: state.clone(),
        })
        .unwrap();
    }
    for fail in [13, 143, 145, 200] {
        state.attempt.set(0);
        state.drops.set(0);
        state.fail.set(fail);
        assert_eq!(a.try_union(&b).unwrap_err(), Error::Capacity);
        assert_eq!(state.drops.get(), fail - 1);
        assert_eq!(a.len(), 160);
        assert_eq!(b.len(), 80);
    }
}
