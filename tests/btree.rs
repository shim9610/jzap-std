mod common;
use common::{FaultAllocator, track};
use jzap::{
    TryClone, TryIterator,
    budget::{Budget, MemoryBudget},
    collections::BTreeMap,
};
use std::collections::BTreeMap as Model;
#[global_allocator]
static ALLOC: FaultAllocator = FaultAllocator;

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn validate(map: &BTreeMap<i32, i32, impl jzap::budget::AllocationPolicy>) {
    fn visit(
        n: jzap::collections::btree_map::view::NodeView<'_, i32, i32>,
        root: bool,
        lo: Option<i32>,
        hi: Option<i32>,
    ) -> usize {
        assert_eq!(n.keys().len(), n.values().len());
        assert!(n.keys().len() <= 11);
        assert!(root || n.keys().len() >= 5);
        let keys = n.keys();
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
        assert!(
            keys.iter()
                .all(|k| lo.is_none_or(|l| l < *k) && hi.is_none_or(|h| *k < h))
        );
        let mut count = keys.len();
        if n.height() > 0 {
            for i in 0..=keys.len() {
                count += visit(
                    n.child(i).unwrap().unwrap(),
                    false,
                    if i == 0 { lo } else { Some(keys[i - 1]) },
                    keys.get(i).copied().or(hi),
                );
            }
        }
        count
    }
    assert_eq!(
        map.root_view().map_or(0, |n| visit(n, true, None, None)),
        map.len()
    );
}
#[test]
fn differential_insert_remove() {
    let mut m = BTreeMap::new();
    let mut model = Model::new();
    let mut seed = 19072026;
    let rounds = if cfg!(miri) { 400 } else { 30000 };
    for step in 0..rounds {
        let k = (rng(&mut seed) % 700) as i32;
        match rng(&mut seed) % 5 {
            0 | 1 => assert_eq!(m.try_insert(k, step).unwrap(), model.insert(k, step)),
            2 => assert_eq!(m.remove_entry(&k), model.remove_entry(&k)),
            3 => assert_eq!(m.pop_first(), model.pop_first()),
            _ => assert_eq!(m.pop_last(), model.pop_last()),
        }
        assert_eq!(m.len(), model.len());
        assert!(m.iter().eq(model.iter()));
        if step % 37 == 0 {
            validate(&m);
        }
    }
    validate(&m);
}
#[test]
fn all_insertion_allocation_ordinals_preserve_and_retry() {
    let (_, s) = track(usize::MAX, || {
        let mut m = BTreeMap::new();
        for i in 0..240 {
            m.try_insert(i, i * 3).unwrap();
        }
    });
    assert!(s.calls > 20);
    assert_eq!(s.live, 0);
    for fail in 1..=s.calls {
        let budget = MemoryBudget::new(usize::MAX);
        let (_, state) = track(fail, || {
            let mut m = BTreeMap::new_in(Budget::Limited(&budget));
            let mut refused = false;
            for i in 0..240 {
                let charge = budget.occupied();
                if let Err(e) = m.try_insert(i, i * 3) {
                    assert_eq!(e.value, (i, i * 3));
                    assert_eq!(budget.occupied(), charge);
                    assert_eq!(m.len(), i as usize);
                    assert!(m.iter().all(|(k, v)| *k < i && *v == *k * 3));
                    validate(&m);
                    refused = true;
                    m.try_insert(e.value.0, e.value.1).unwrap();
                }
            }
            assert!(refused);
            validate(&m);
        });
        assert_eq!(state.live, 0);
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
}
#[test]
fn owned_iteration_both_ends_and_early_drop() {
    let counts = if cfg!(miri) {
        vec![0, 1, 12, 31, 170]
    } else {
        vec![0, 1, 11, 12, 31, 170, 500]
    };
    for count in counts {
        for stop in [0, count / 2, count] {
            let budget = MemoryBudget::new(usize::MAX);
            let mut m = BTreeMap::new_in(Budget::Limited(&budget));
            let mut model = Model::new();
            for i in 0..count {
                m.try_insert(i, i * 2).unwrap();
                model.insert(i, i * 2);
            }
            let mut it = m.into_iter();
            let mut expected = model.into_iter().collect::<Vec<_>>().into_iter();
            for i in 0..stop {
                assert!(it.as_iter().map(|(k, v)| (*k, *v)).eq(expected.clone()));
                let x = if i % 3 == 0 {
                    it.next_back()
                } else {
                    it.next()
                };
                let y = if i % 3 == 0 {
                    expected.next_back()
                } else {
                    expected.next()
                };
                assert_eq!(x, y);
                assert_eq!(it.len(), expected.len());
            }
            drop(it);
            assert_eq!(budget.occupied(), 0);
        }
    }
}
#[test]
fn mutable_iterator_holds_disjoint_slots() {
    let mut m = BTreeMap::new();
    for i in 0..200 {
        m.try_insert(i, i).unwrap();
    }
    let mut it = m.iter_mut();
    let mut refs = Vec::new();
    while it.len() > 0 {
        refs.push(if it.len() % 2 == 0 {
            it.next().unwrap()
        } else {
            it.next_back().unwrap()
        });
    }
    for (k, v) in refs {
        *v = *k * 3;
    }
    assert!(m.iter().all(|(k, v)| *v == *k * 3));
}
#[test]
fn inconsistent_user_order_cannot_overlap_mutable_ranges() {
    use std::{cell::Cell, cmp::Ordering, rc::Rc};
    #[derive(Clone)]
    struct Key(u32, Rc<Cell<u32>>);
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Key {}
    impl PartialOrd for Key {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> Ordering {
            let n = self.1.get();
            self.1.set(n.wrapping_add(1));
            if n % 3 == 0 {
                self.0.cmp(&other.0).reverse()
            } else {
                self.0.cmp(&other.0)
            }
        }
    }
    struct ChangingBounds<'a> {
        low: &'a Key,
        high: &'a Key,
        reads: &'a Cell<(u32, u32)>,
    }
    impl std::ops::RangeBounds<Key> for ChangingBounds<'_> {
        fn start_bound(&self) -> std::ops::Bound<&Key> {
            let (a, b) = self.reads.get();
            self.reads.set((a + 1, b));
            if a == 0 {
                std::ops::Bound::Included(self.low)
            } else {
                std::ops::Bound::Unbounded
            }
        }
        fn end_bound(&self) -> std::ops::Bound<&Key> {
            let (a, b) = self.reads.get();
            self.reads.set((a, b + 1));
            if b == 0 {
                std::ops::Bound::Included(self.high)
            } else {
                std::ops::Bound::Unbounded
            }
        }
    }
    let counter = Rc::new(Cell::new(0));
    let mut map = BTreeMap::new();
    for k in 0..180 {
        map.try_insert(Key(k, counter.clone()), k).ok().unwrap();
    }
    for k in 0..if cfg!(miri) { 12 } else { 180 } {
        let a = Key(k, counter.clone());
        let b = Key(179 - k, counter.clone());
        let reads = Cell::new((0, 0));
        let result = map.try_range_mut(ChangingBounds {
            low: &a,
            high: &b,
            reads: &reads,
        });
        // Check before yielding references: a second callback must not change
        // which physical-order validation applies to the captured endpoints.
        assert_eq!(reads.get(), (1, 1));
        if let Ok(mut range) = result {
            let mut refs = Vec::new();
            loop {
                let item = if refs.len() % 2 == 0 {
                    range.next()
                } else {
                    range.next_back()
                };
                let Some((_, v)) = item else { break };
                refs.push(v);
            }
            // Keep every yielded exclusive reference alive simultaneously.
            for v in refs {
                *v = v.wrapping_add(1);
            }
        }
    }
}
#[test]
fn entry_and_no_allocation_deletion() {
    let mut m = BTreeMap::new();
    for i in 0..150 {
        *m.entry(i).try_or_insert(i * 7).unwrap() += 1;
    }
    let (_, s) = track(1, || {
        for i in (0..150).rev() {
            assert_eq!(m.remove(&i), Some(i * 7 + 1));
        }
    });
    assert_eq!(s.calls, 0);
    assert!(m.is_empty());
    m.try_insert(2, 3).unwrap();
    validate(&m);
}
#[test]
fn entry_initializers_reserve_before_callback_and_refund_on_error() {
    use std::cell::Cell;
    let budget = MemoryBudget::new(usize::MAX);
    let calls = Cell::new(0);
    let mut m = BTreeMap::new_in(Budget::Limited(&budget));
    for i in 0..11 {
        m.try_insert(i, i).unwrap();
    }
    let charge = budget.occupied();
    let (r, stats) = track(1, || {
        m.entry(11).try_or_insert_with(|| {
            calls.set(calls.get() + 1);
            Ok::<_, u8>(11)
        })
    });
    assert!(r.is_err());
    assert_eq!(stats.live, 0);
    assert_eq!(calls.get(), 0);
    assert_eq!(budget.occupied(), charge);
    let e = m
        .entry(11)
        .try_or_insert_with(|| {
            calls.set(calls.get() + 1);
            Err(17u8)
        })
        .unwrap_err();
    assert_eq!(e.value, 11);
    assert_eq!(e.error, jzap::error::InitError::Initialize(17));
    assert_eq!(calls.get(), 1);
    assert_eq!(budget.occupied(), charge);
    m.try_insert(11, 11).unwrap();
    *m.entry(12).try_or_insert_with(|| Ok::<_, u8>(12)).unwrap() = 24;
    assert_eq!(m.get(&12), Some(&24));
    validate(&m);
}
#[test]
fn stable_adapters_preserve_failure_ownership_and_callback_edits() {
    use jzap::{
        TryDefault,
        collections::btree_map::{Entry, ExtendError},
    };
    use std::hash::{Hash, Hasher};
    let mut map = BTreeMap::try_from_array([(3, 30), (1, 10), (3, 300)]).unwrap();
    assert_eq!(map.len(), 2);
    let source = jzap::iter::from_results([Ok((2, 20)), Err(7u8)]);
    assert!(matches!(
        map.try_extend(source),
        Err(ExtendError::Source(7))
    ));
    assert_eq!(map.get(&2), Some(&20));
    let result = map.entry(2).try_and_modify(|v| {
        *v = 77;
        Err(9u8)
    });
    assert!(matches!(result, Err(9)));
    assert_eq!(map.get(&2), Some(&77));
    assert_eq!(
        *map.entry(5)
            .try_or_insert_with_key(|k| Ok::<_, u8>(*k * 3))
            .unwrap(),
        15
    );
    assert_eq!(*map.entry(6).try_or_default().unwrap(), 0);
    if let Entry::Vacant(entry) = map.entry(9) {
        assert_eq!(entry.into_key(), 9);
    } else {
        panic!("expected vacancy")
    }
    let mut occupied = map.entry(3).try_insert_entry(301).unwrap();
    assert_eq!(occupied.insert(302), 301);
    *occupied.get_mut() = 303;
    assert_eq!(occupied.remove(), 303);
    assert_eq!(map.last_entry().unwrap().remove_entry(), (6, 0));
    assert_eq!(*map.first_entry().unwrap().into_mut(), 10);
    let mut copy = BTreeMap::try_default().unwrap();
    copy.try_clone_from(&map).unwrap();
    let model = Model::from_iter([(1, 10), (2, 77), (5, 15)]);
    assert_eq!(map, copy);
    assert_eq!(map.cmp(&copy), std::cmp::Ordering::Equal);
    assert_eq!(format!("{map:?}"), format!("{model:?}"));
    struct LimitedWrites(usize);
    impl std::fmt::Write for LimitedWrites {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            if self.0 == 0 {
                Err(std::fmt::Error)
            } else {
                self.0 -= 1;
                Ok(())
            }
        }
    }
    use std::fmt::Write;
    for writes in 0..32 {
        for pretty in [false, true] {
            let (mut ours, mut reference) = (LimitedWrites(writes), LimitedWrites(writes));
            let (a, b) = if pretty {
                (
                    write!(&mut ours, "{map:#?}"),
                    write!(&mut reference, "{model:#?}"),
                )
            } else {
                (
                    write!(&mut ours, "{map:?}"),
                    write!(&mut reference, "{model:?}"),
                )
            };
            assert_eq!(a.is_err(), b.is_err());
        }
    }
    assert_eq!(format!("{:?}", map.values()), "[10, 77, 15]");
    let (mut a, mut b) = (
        std::collections::hash_map::DefaultHasher::new(),
        std::collections::hash_map::DefaultHasher::new(),
    );
    map.hash(&mut a);
    model.hash(&mut b);
    assert_eq!(a.finish(), b.finish());
    assert_eq!(copy.into_keys().collect::<Vec<_>>(), vec![1, 2, 5]);
    assert_eq!(map.into_values().collect::<Vec<_>>(), vec![10, 77, 15]);

    let storage = MemoryBudget::new(0);
    let payload = MemoryBudget::new(64);
    let key = jzap::boxed::Box::try_new_in(55u64, Budget::Limited(&payload)).unwrap();
    let value = jzap::boxed::Box::try_new_in(66u64, Budget::Limited(&payload)).unwrap();
    let mut refused = BTreeMap::new_in(Budget::Limited(&storage));
    let error = refused.try_extend([(key, value)]).unwrap_err();
    assert!(refused.is_empty());
    assert_eq!(payload.occupied(), 16);
    match &error {
        ExtendError::Insert(e) => assert_eq!((*e.value.0, *e.value.1), (55, 66)),
        _ => panic!("expected insertion refusal"),
    }
    drop(error);
    assert_eq!(payload.occupied(), 0);
    let source = jzap::iter::from_results([Ok((4u64, 8u64)), Err(11u8)]);
    let temporary = MemoryBudget::new(usize::MAX);
    assert!(matches!(
        BTreeMap::try_from_iter_in(source, Budget::Limited(&temporary)),
        Err(ExtendError::Source(11))
    ));
    assert_eq!((temporary.occupied(), temporary.reserved()), (0, 0));
}
#[test]
fn split_every_border_and_height() {
    let max = if cfg!(miri) { 30 } else { 220 };
    for n in [0, 1, 11, 12, 60, 170, 400] {
        for key in (-1..=n + 1).step_by(if cfg!(miri) { 19 } else { 7 }) {
            let mut m = BTreeMap::new();
            let mut model = Model::new();
            let mut seed = 13;
            let mut order = (0..n).collect::<Vec<_>>();
            for i in (1..order.len()).rev() {
                let j = rng(&mut seed) as usize % (i + 1);
                order.swap(i, j);
            }
            for i in order {
                m.try_insert(i, i * 2).unwrap();
                model.insert(i, i * 2);
            }
            let right = m.try_split_off(&key).unwrap();
            let expected = model.split_off(&key);
            assert!(m.iter().eq(model.iter()));
            assert!(right.iter().eq(expected.iter()));
            validate(&m);
            validate(&right);
        }
    }
    let mut m = BTreeMap::new();
    for i in 0..max {
        m.try_insert(i, i).unwrap();
    }
    let c = m.try_clone().unwrap();
    assert_eq!(m, c);
    let (removed, _) = track(1, || {
        let mut e = m
            .try_extract_if(.., |k, v| {
                *v += 1;
                Ok::<_, ()>(*k % 3 == 0)
            })
            .unwrap();
        let mut count = 0;
        while e.try_next().unwrap().is_some() {
            count += 1;
        }
        count
    });
    assert_eq!(removed, (max as usize + 2) / 3);
    validate(&m);
}
#[test]
fn ranges_and_extract_errors() {
    use jzap::TryIterator;
    use std::ops::Bound::*;
    let mut m = BTreeMap::new();
    let mut model = Model::new();
    for i in 0..160 {
        m.try_insert(i, i).unwrap();
        model.insert(i, i);
    }
    let starts = if cfg!(miri) {
        vec![-1, 17, 180]
    } else {
        vec![-1, 0, 1, 6, 17, 83, 160, 180]
    };
    let ends = if cfg!(miri) {
        vec![13, 90]
    } else {
        vec![0, 6, 13, 90, 159, 170]
    };
    for a in starts {
        for b in &ends {
            let b = *b;
            for lo in [Included(a), Excluded(a), Unbounded] {
                for hi in [Included(b), Excluded(b), Unbounded] {
                    if matches!((lo,hi),(Included(x)|Excluded(x),Included(y)|Excluded(y)) if x>y || (x==y&&matches!((lo,hi),(Excluded(_),Excluded(_)))))
                    {
                        assert!(m.try_range((lo, hi)).is_err());
                        continue;
                    }
                    let got = m
                        .try_range((lo, hi))
                        .unwrap()
                        .map(|(k, v)| (*k, *v))
                        .collect::<Vec<_>>();
                    let exp = model
                        .range((lo, hi))
                        .map(|(k, v)| (*k, *v))
                        .collect::<Vec<_>>();
                    assert_eq!(got, exp);
                    let mut r = m.try_range_mut((lo, hi)).unwrap();
                    let mut held = Vec::new();
                    let mut back = false;
                    loop {
                        let next = if back { r.next_back() } else { r.next() };
                        back = !back;
                        match next {
                            Some(x) => held.push(x),
                            None => break,
                        }
                    }
                    assert_eq!(held.len(), exp.len());
                }
            }
        }
    }
    let mut e = m
        .try_extract_if(17..139, |k, v| {
            *v += 1;
            if *k == 51 { Err(7) } else { Ok(*k % 2 == 0) }
        })
        .unwrap();
    assert_eq!(e.try_next(), Ok(Some((18, 19))));
    loop {
        match e.try_next() {
            Err(7) => break,
            Ok(Some(_)) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    while e.try_next().unwrap().is_some() {}
    drop(e);
    validate(&m);
    assert_eq!(m.get(&51), Some(&52));
}
#[test]
fn append_recycling_differential_and_budget_transfer() {
    let sizes = if cfg!(miri) {
        vec![0, 1, 12, 65]
    } else {
        vec![0, 1, 5, 11, 12, 60, 144, 700, 5000]
    };
    for n in &sizes {
        for other in &sizes {
            for kind in 0..3 {
                let ba = MemoryBudget::new(usize::MAX);
                let bb = MemoryBudget::new(usize::MAX);
                let mut a = BTreeMap::new_in(Budget::Limited(&ba));
                let mut b = BTreeMap::new_in(Budget::Limited(&bb));
                let mut ma = Model::new();
                let mut mb = Model::new();
                for i in 0..*n {
                    let k = if kind == 0 { i * 2 } else { i };
                    a.try_insert(k, i).unwrap();
                    ma.insert(k, i);
                }
                for i in 0..*other {
                    let k = match kind {
                        0 => i * 2 + 1,
                        1 => i,
                        2 => i + n,
                        _ => unreachable!(),
                    };
                    b.try_insert(k, -i).unwrap();
                    mb.insert(k, -i);
                }
                a.try_append(&mut b).unwrap();
                ma.append(&mut mb);
                assert!(a.iter().eq(ma.iter()));
                assert!(b.is_empty());
                validate(&a);
                assert_eq!(bb.occupied(), 0);
                drop((a, b));
                assert_eq!(ba.occupied(), 0);
            }
        }
    }
}
#[test]
fn split_append_and_clone_allocation_refusals() {
    for op in 0..3 {
        for fail in 1..15 {
            let budget = MemoryBudget::new(usize::MAX);
            let mut a = BTreeMap::new_in(Budget::Limited(&budget));
            let mut b = BTreeMap::new_in(Budget::Limited(&budget));
            for i in 0..200 {
                a.try_insert(i, i).unwrap();
                b.try_insert(i + 77, -i).unwrap();
            }
            let before_a = a.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>();
            let before_b = b.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>();
            let charge = budget.occupied();
            let (failed, _) = track(fail, || match op {
                0 => a.try_split_off(&83).map(drop).is_err(),
                1 => a.try_append(&mut b).is_err(),
                _ => a.try_clone().map(drop).is_err(),
            });
            if failed {
                assert_eq!(charge, budget.occupied());
                assert!(a.iter().map(|(k, v)| (*k, *v)).eq(before_a));
                assert!(b.iter().map(|(k, v)| (*k, *v)).eq(before_b));
            }
            validate(&a);
            validate(&b);
            drop((a, b));
            assert_eq!(budget.occupied(), 0);
        }
    }
}
#[test]
fn caller_owned_node_slices_and_continuations() {
    use jzap::collections::btree_map::view::{Lookup, LookupStep, NodeSlice, Traversal, Visit};
    use std::ops::Bound::*;
    let root_k = [10, 20];
    let root_v = [100, 200];
    let ids = [1, 2, 3];
    let left = [1, 3, 7];
    let mid = [11, 15];
    let right = [22, 25, 29];
    let nodes = [
        NodeSlice::new(&root_k, &root_v, Some(&ids)).unwrap(),
        NodeSlice::new(&left, &left, None).unwrap(),
        NodeSlice::new(&mid, &mid, None).unwrap(),
        NodeSlice::new(&right, &right, None).unwrap(),
    ];
    for key in [0, 1, 3, 10, 11, 18, 20, 22, 29, 30] {
        let mut lookup = Lookup::new(0, 1, &key).unwrap();
        let mut seen = 0;
        while let Some(id) = lookup.requested_node() {
            seen += 1;
            match lookup.provide(id, nodes[id]).unwrap() {
                LookupStep::Found(k, _) => assert_eq!(*k, key),
                LookupStep::Absent => {
                    assert!(![1, 3, 7, 10, 11, 15, 20, 22, 25, 29].contains(&key))
                }
                LookupStep::NeedNode(_) => {}
            }
        }
        assert!(seen <= 2);
    }
    for (lo, hi) in [
        (Unbounded, Unbounded),
        (Included(&3), Excluded(&25)),
        (Excluded(&10), Included(&20)),
    ] {
        let mut traversal = Traversal::try_range(0, 1, lo, hi).unwrap();
        let mut got = Vec::new();
        loop {
            match traversal.advance().unwrap() {
                Visit::NeedNode(id) => traversal.provide(id, nodes[id]).unwrap(),
                Visit::Item(k, v) => got.push((*k, *v)),
                Visit::Done => break,
            }
        }
        let model = [
            (1, 1),
            (3, 3),
            (7, 7),
            (10, 100),
            (11, 11),
            (15, 15),
            (20, 200),
            (22, 22),
            (25, 25),
            (29, 29),
        ]
        .into_iter()
        .filter(|(k, _)| match lo {
            Unbounded => true,
            Included(x) => k >= x,
            Excluded(x) => k > x,
        })
        .filter(|(k, _)| match hi {
            Unbounded => true,
            Included(x) => k <= x,
            Excluded(x) => k < x,
        })
        .collect::<Vec<_>>();
        assert_eq!(got, model);
    }
    assert!(NodeSlice::<_, _, usize>::new(&[2, 1], &[2, 1], None).is_err());
    assert!(NodeSlice::new(&[10], &[10], Some(&[0])).is_err());
    let cycle = [0, 1];
    let malformed = NodeSlice::new(&[10], &[10], Some(&cycle)).unwrap();
    let mut lookup = Lookup::new(0, 1, &0).unwrap();
    assert!(lookup.provide(0, malformed).is_err());
    assert_eq!(lookup.requested_node(), Some(0));
}
#[test]
fn local_slice_lookup_allows_reusing_the_same_caller_buffers() {
    use jzap::collections::btree_map::view::{LookupStep, NodeSlice};
    let mut keys = Vec::with_capacity(8);
    let mut values = Vec::with_capacity(8);
    let children = [1usize, 2];
    keys.push(10);
    values.push(100);
    let original = keys.as_ptr();
    let next = {
        let root = NodeSlice::new(&keys, &values, Some(&children)).unwrap();
        match root.lookup(&15).unwrap() {
            LookupStep::NeedNode(id) => id,
            _ => panic!("expected another slice"),
        }
    };
    assert_eq!(next, 2);
    keys.clear();
    values.clear();
    keys.extend([11, 15, 19]);
    values.extend([111, 115, 119]);
    assert_eq!(original, keys.as_ptr());
    let leaf = NodeSlice::<_, _, usize>::new(&keys, &values, None).unwrap();
    assert_eq!(leaf.lookup(&15), Ok(LookupStep::Found(&15, &115)));
}

#[test]
fn bplus_point_lookup_matches_std() {
    use jzap::collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node},
    };
    use std::collections::BTreeMap;

    let mut random = 617_u64;
    for (fanout, leaf_width) in [
        (1, 1),
        (2, 8),
        (3, 9),
        (5, 3),
        (8, 8),
        (9, 8),
        (17, 17),
        (32, 32),
    ] {
        let keys: Vec<u64> = (0..fanout * leaf_width).map(|i| i as u64 * 2 + 2).collect();
        let model: BTreeMap<_, _> = keys.iter().map(|&key| (key, key * 3)).collect();
        let minima: Vec<_> = keys.chunks_exact(leaf_width).map(|leaf| leaf[0]).collect();
        let children: Vec<_> = (1..=fanout).collect();
        for _ in 0..256 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let query = random % (keys.len() as u64 * 2 + 4);
            let mut lookup = Lookup::new(0, 1, &query).unwrap();
            let root = Node::<_, u64, _>::branch(&minima, &children).unwrap();
            let actual = match lookup.provide(0, &root).unwrap() {
                LookupStep::Absent => None,
                LookupStep::NeedNode(id) => {
                    let keys = &keys[(id - 1) * leaf_width..id * leaf_width];
                    let values: Vec<_> = keys.iter().map(|key| key * 3).collect();
                    match lookup
                        .provide(id, &Node::leaf(keys, &values).unwrap())
                        .unwrap()
                    {
                        LookupStep::Found(key, value) => Some((*key, *value)),
                        LookupStep::Absent => None,
                        LookupStep::NeedNode(_) => unreachable!(),
                    }
                }
                LookupStep::Found(_, _) => unreachable!(),
            };
            assert_eq!(actual, model.get_key_value(&query).map(|(&k, &v)| (k, v)));
            assert_eq!(lookup.requested_node(), None);
        }
    }
}

#[test]
fn bplus_reuses_parent_storage_and_checks_inherited_fences_on_retry() {
    use jzap::Error;
    use jzap::collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node},
    };

    let query = 10;
    let mut keys = [10, 20];
    let mut lookup = Lookup::new(0, 2, &query).unwrap();
    assert_eq!(
        lookup.provide(0, &Node::<_, u32, _>::branch(&keys, &[1, 2]).unwrap()),
        Ok(LookupStep::NeedNode(1))
    );
    keys = [10, 12];
    assert_eq!(
        lookup.provide(4, &Node::<_, u32, _>::branch(&keys[..1], &[3]).unwrap()),
        Err(Error::InvalidStructure)
    );
    assert_eq!(
        lookup.provide(1, &Node::<_, u32, _>::branch(&keys[..1], &[0]).unwrap()),
        Err(Error::InvalidStructure)
    );
    assert_eq!(
        lookup.provide(1, &Node::leaf(&keys, &[55, 56]).unwrap()),
        Err(Error::InvalidStructure)
    );
    assert_eq!(lookup.requested_node(), Some(1));
    assert_eq!(
        lookup.provide(1, &Node::<_, u32, _>::branch(&keys[..1], &[3]).unwrap()),
        Ok(LookupStep::NeedNode(3))
    );

    for malformed in [[9, 11], [11, 13], [10, 20], [10, 21]] {
        keys = malformed;
        assert_eq!(
            lookup.provide(3, &Node::leaf(&keys, &[55, 56]).unwrap()),
            Err(Error::InvalidStructure)
        );
        assert_eq!(lookup.requested_node(), Some(3));
    }
    assert_eq!(
        lookup.provide(3, &Node::leaf(&[], &[] as &[u32]).unwrap()),
        Err(Error::InvalidStructure)
    );
    keys = [10, 19];
    assert_eq!(
        lookup.provide(3, &Node::leaf(&keys, &[55, 56]).unwrap()),
        Ok(LookupStep::Found(&10, &55))
    );
    assert_eq!(lookup.requested_node(), None);
    assert_eq!(
        lookup.provide(3, &Node::leaf(&keys, &[55, 56]).unwrap()),
        Err(Error::InvalidStructure)
    );

    assert!(Node::<u32, (), usize>::branch(&[], &[]).is_err());
    assert!(Node::<_, (), _>::branch(&[1, 2], &[3]).is_err());
    assert!(Node::<_, (), _>::branch(&[2, 1], &[3, 4]).is_err());
    assert!(Node::<_, _, usize>::leaf(&[1, 1], &[1, 2]).is_err());
    assert!(Node::<_, _, usize>::leaf(&[1, 2], &[1]).is_err());
    assert!(Lookup::<i32, _>::new(0, usize::BITS as usize, &query).is_err());
    let mut empty = Lookup::<i32, _>::new(0, 0, &query).unwrap();
    assert_eq!(
        empty.provide(0, &Node::leaf(&[], &[] as &[u32]).unwrap()),
        Ok(LookupStep::Absent)
    );
}

#[test]
fn bplus_initializes_only_visited_ancestors_at_maximum_height() {
    use jzap::Error;
    use jzap::collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node},
    };
    use std::num::NonZeroU32;

    let height = usize::BITS as usize - 1;
    let node_id = |depth| NonZeroU32::new(u32::try_from(depth + 1).unwrap()).unwrap();
    let mut lookup = Lookup::<&str, _, str>::new(node_id(0), height, "middle").unwrap();

    for depth in 0..height {
        // The first descent is a self-edge. Later attempts point to the root.
        let cycle = [node_id(0)];
        let invalid = Node::<_, u32, _>::branch(&["first"], &cycle).unwrap();
        assert_eq!(
            lookup.provide(node_id(depth), &invalid),
            Err(Error::InvalidStructure)
        );
        assert_eq!(lookup.requested_node(), Some(node_id(depth)));

        let child = [node_id(depth + 1)];
        let branch = Node::<_, u32, _>::branch(&["first"], &child).unwrap();
        assert_eq!(
            lookup.provide(node_id(depth), &branch),
            Ok(LookupStep::NeedNode(child[0]))
        );
    }

    let leaf = Node::leaf(&["first", "middle", "third"], &[10, 20, 30]).unwrap();
    assert_eq!(
        lookup.provide(node_id(height), &leaf),
        Ok(LookupStep::Found(&"middle", &20))
    );
}

#[test]
fn destructors_unwind_once_and_still_release_every_node() {
    use std::cell::Cell;
    struct Item<'a>(&'a Cell<usize>, &'a Cell<bool>);
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            if self.1.replace(false) {
                panic!("one user destructor");
            }
        }
    }
    for owning_iter in [false, true] {
        let drops = Cell::new(0);
        let panic = Cell::new(false);
        let budget = MemoryBudget::new(usize::MAX);
        let mut m = BTreeMap::new_in(Budget::Limited(&budget));
        for i in 0..100 {
            m.try_insert(i, Item(&drops, &panic))
                .map_err(|e| e.error)
                .unwrap();
        }
        panic.set(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if owning_iter {
                let mut iter = m.into_iter();
                drop(iter.next());
            } else {
                m.clear();
            }
        }));
        assert!(result.is_err());
        assert_eq!(drops.get(), 100);
        assert_eq!(budget.occupied(), 0);
    }
}
#[test]
fn clone_failure_each_key_and_value_cleans_the_partial_tree() {
    use std::cell::Cell;
    #[derive(Debug)]
    struct Item<'a> {
        id: i32,
        live: &'a Cell<usize>,
        calls: &'a Cell<usize>,
        fail: &'a Cell<usize>,
    }
    impl Item<'_> {
        fn copy(&self) -> Self {
            self.live.set(self.live.get() + 1);
            Self {
                id: self.id,
                live: self.live,
                calls: self.calls,
                fail: self.fail,
            }
        }
    }
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.live.set(self.live.get() - 1);
        }
    }
    impl PartialEq for Item<'_> {
        fn eq(&self, b: &Self) -> bool {
            self.id == b.id
        }
    }
    impl Eq for Item<'_> {}
    impl PartialOrd for Item<'_> {
        fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(b))
        }
    }
    impl Ord for Item<'_> {
        fn cmp(&self, b: &Self) -> std::cmp::Ordering {
            self.id.cmp(&b.id)
        }
    }
    impl TryClone for Item<'_> {
        fn try_clone(&self) -> Result<Self, jzap::Error> {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n == self.fail.get() {
                Err(jzap::Error::Capacity)
            } else {
                Ok(self.copy())
            }
        }
    }
    let live = Cell::new(0);
    let calls = Cell::new(0);
    let fail = Cell::new(usize::MAX);
    let budget = MemoryBudget::new(usize::MAX);
    let base = Item {
        id: 0,
        live: &live,
        calls: &calls,
        fail: &fail,
    };
    live.set(1);
    let mut m = BTreeMap::new_in(Budget::Limited(&budget));
    for id in 0..35 {
        let mut k = base.copy();
        k.id = id;
        let mut v = base.copy();
        v.id = id * 3;
        m.try_insert(k, v).unwrap();
    }
    let charge = budget.occupied();
    for i in 1..=70 {
        calls.set(0);
        fail.set(i);
        assert!(m.try_clone().is_err());
        assert_eq!(live.get(), 71);
        assert_eq!(budget.occupied(), charge);
    }
    fail.set(usize::MAX);
    let c = m.try_clone().unwrap();
    assert_eq!(c, m);
    drop(c);
    drop(m);
    assert_eq!(live.get(), 1);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn zero_sized_overaligned_and_nested_budget_payloads() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static ZST_DROPS: AtomicUsize = AtomicUsize::new(0);
    #[derive(Debug)]
    struct ZstDrop;
    impl Drop for ZstDrop {
        fn drop(&mut self) {
            ZST_DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }
    #[repr(align(4096))]
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct Aligned(u32);
    let mut z = BTreeMap::new();
    assert_eq!(z.try_insert((), ()).unwrap(), None);
    assert_eq!(z.try_insert((), ()).unwrap(), Some(()));
    assert_eq!(z.pop_last(), Some(((), ())));
    let mut zst = BTreeMap::new();
    for i in 0..35 {
        zst.try_insert(i, ZstDrop).unwrap();
    }
    let mut owner = zst.into_iter();
    drop(owner.next());
    drop(owner.next_back());
    drop(owner);
    assert_eq!(ZST_DROPS.load(Ordering::Relaxed), 35);
    let mut a = BTreeMap::new();
    for i in 0..33 {
        a.try_insert(Aligned(i), Aligned(i + 1)).unwrap();
    }
    for (k, v) in &a {
        assert_eq!((k as *const Aligned).addr() % 4096, 0);
        assert_eq!((v as *const Aligned).addr() % 4096, 0);
    }
    let b1 = MemoryBudget::new(usize::MAX);
    let b2 = MemoryBudget::new(usize::MAX);
    let mut m = BTreeMap::new_in(Budget::Limited(&b1));
    for i in 0..20 {
        m.try_insert(
            i,
            jzap::boxed::Box::try_new_in(i, Budget::Limited(&b1)).unwrap(),
        )
        .unwrap();
    }
    let c = jzap::TryCloneIn::try_clone_in(&m, Budget::Limited(&b2)).unwrap();
    assert!(m.iter().all(|(k, v)| **v == *k));
    assert!(c.iter().all(|(k, v)| **v == *k));
    drop(m);
    assert_eq!(b1.occupied(), 0);
    assert!(b2.occupied() > 0);
    drop(c);
    assert_eq!(b2.occupied(), 0);
}
#[test]
fn drop_fields_in_place_in_map_and_partly_consumed_owner() {
    use std::cell::Cell;
    #[derive(Debug)]
    struct Value<'a> {
        expected: Cell<usize>,
        drops: &'a Cell<usize>,
    }
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            let at = self.expected.get();
            if at != 0 {
                assert_eq!(at, (self as *const Self).addr());
            }
            self.drops.set(self.drops.get() + 1);
        }
    }
    for owning in [false, true] {
        let drops = Cell::new(0);
        let budget = MemoryBudget::new(usize::MAX);
        let mut m = BTreeMap::new_in(Budget::Limited(&budget));
        for i in 0..45 {
            m.try_insert(
                i,
                Value {
                    expected: Cell::new(0),
                    drops: &drops,
                },
            )
            .unwrap();
        }
        for (_, v) in &m {
            v.expected.set((v as *const Value<'_>).addr());
        }
        if owning {
            let mut it = m.into_iter();
            for i in 0..11 {
                let (_, v) = if i % 2 == 0 {
                    it.next().unwrap()
                } else {
                    it.next_back().unwrap()
                };
                v.expected.set(0);
                drop(v);
            }
            drop(it);
        } else {
            m.clear();
        }
        assert_eq!(drops.get(), 45);
        assert_eq!(budget.occupied(), 0);
    }
}
#[test]
fn node_requests_and_retained_bytes_match_std_after_rebalancing() {
    for count in [0, 1, 11, 12, 144, 400] {
        let mut keys = (0..count).collect::<Vec<i32>>();
        let mut seed = 617;
        for i in (1..keys.len()).rev() {
            let j = rng(&mut seed) as usize % (i + 1);
            keys.swap(i, j);
        }
        for mode in 0..4 {
            let ((expected, other), s) = track(usize::MAX, || {
                let mut m = Model::new();
                for k in &keys {
                    m.insert(*k, *k);
                }
                match mode {
                    1 => {
                        for k in keys.iter().step_by(3) {
                            m.remove(k);
                        }
                    }
                    2 => m.retain(|k, _| *k % 3 != 0),
                    _ => {}
                }
                let r = if mode == 3 {
                    m.split_off(&(count / 2))
                } else {
                    Model::new()
                };
                (m, r)
            });
            let ((actual, right), j) = track(usize::MAX, || {
                let mut m = BTreeMap::new();
                for k in &keys {
                    m.try_insert(*k, *k).unwrap();
                }
                match mode {
                    1 => {
                        for k in keys.iter().step_by(3) {
                            m.remove(k);
                        }
                    }
                    2 => m.try_retain(|k, _| Ok::<_, ()>(*k % 3 != 0)).unwrap(),
                    _ => {}
                }
                let r = if mode == 3 {
                    m.try_split_off(&(count / 2)).unwrap()
                } else {
                    BTreeMap::new()
                };
                (m, r)
            });
            assert!(actual.iter().eq(expected.iter()));
            assert!(right.iter().eq(other.iter()));
            assert_eq!(
                (j.live, j.peak, j.calls),
                (s.live, s.peak, s.calls),
                "count={count},mode={mode}"
            );
            validate(&actual);
            validate(&right);
        }
    }
}
fn property_config() -> proptest::test_runner::Config {
    let mut config = proptest::test_runner::Config::with_cases(if cfg!(miri) { 2 } else { 64 });
    if cfg!(miri) {
        // Miri isolation has no host cwd; keep property cases deterministic and
        // in memory instead of invoking the failure-persistence filesystem API.
        config.failure_persistence = None;
        config.rng_seed = proptest::test_runner::RngSeed::Fixed(617);
    }
    config
}
proptest::proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn mixed_structural_operations(ops in proptest::collection::vec((0u8..7,-500i32..500),0..if cfg!(miri){40}else{250})) {
        let mut a=BTreeMap::new();let mut b=Model::new();
        for (op,key) in ops {
            match op {
                0|1=>{assert_eq!(a.try_insert(key,key*7).unwrap(),b.insert(key,key*7));}
                2=>{assert_eq!(a.remove_entry(&key),b.remove_entry(&key));}
                3=>{let mut ar=a.try_split_off(&key).unwrap();let mut br=b.split_off(&key);validate(&a);validate(&ar);a.try_append(&mut ar).unwrap();b.append(&mut br);}
                4=>{a.try_retain(|k,_|Ok::<_,()>(*k%5!=key%5)).unwrap();b.retain(|k,_|*k%5!=key%5);}
                5=>assert_eq!(a.pop_first(),b.pop_first()),
                _=>assert_eq!(a.pop_last(),b.pop_last()),
            }
            assert!(a.iter().eq(b.iter()));validate(&a);
        }
    }
}

#[test]
fn fixed_bplus_validates_a_complete_snapshot() {
    use jzap::collections::btree_map::view::bplus::FixedTree;

    let leaves = [
        ([2, 3], [20, 30]),
        ([5, 7], [50, 70]),
        ([11, 13], [110, 130]),
        ([17, 19], [170, 190]),
    ];
    let branches = [([2, 5], [0, 1]), ([11, 17], [2, 3]), ([2, 11], [4, 5])];
    let tree = FixedTree::try_new(&leaves, &branches).unwrap();
    for key in 0..21 {
        let expected = leaves.iter().find_map(|(keys, values)| {
            keys.iter()
                .zip(values)
                .find_map(|(candidate, value)| (*candidate == key).then_some(value))
        });
        assert_eq!(tree.get(&key), Ok(expected));
    }
    assert!(matches!(
        FixedTree::try_new(&leaves, &[branches[0], branches[1], ([2, 11], [4, 6])]),
        Err(jzap::Error::InvalidStructure)
    ));
    assert!(matches!(
        FixedTree::try_new(&leaves, &[branches[0], branches[0], branches[2]]),
        Err(jzap::Error::InvalidStructure)
    ));
    assert!(matches!(
        FixedTree::try_new(&leaves, &[branches[0]]),
        Err(jzap::Error::InvalidStructure)
    ));
    assert!(matches!(
        FixedTree::try_new(&leaves, &[branches[0], branches[1], ([2, 5], [4, 1])]),
        Err(jzap::Error::InvalidStructure)
    ));
    let overlapped = [([2, 8], [20, 80]), ([5, 7], [50, 70])];
    assert!(matches!(
        FixedTree::try_new(&overlapped, &[branches[0]]),
        Err(jzap::Error::InvalidStructure)
    ));
    assert_eq!(
        FixedTree::<i32, (), 1>::try_new(&[], &[]).unwrap().get(&0),
        Ok(None)
    );
    assert_eq!(
        FixedTree::try_new(&[([2], [17])], &[]).unwrap().get(&2),
        Ok(Some(&17))
    );
    assert!(matches!(
        FixedTree::<i32, (), 0>::try_new(&[], &[]),
        Err(jzap::Error::InvalidStructure)
    ));
}

#[test]
fn fixed_bplus_range_matches_std_across_scrambled_leaf_ids() {
    use core::ops::Bound::{Excluded, Included, Unbounded};
    use jzap::{Error, collections::btree_map::view::bplus::FixedTree};

    let leaves = [
        ([17, 19], [170, 190]),
        ([2, 3], [20, 30]),
        ([11, 13], [110, 130]),
        ([5, 7], [50, 70]),
    ];
    let branches = [([11, 17], [2, 0]), ([2, 5], [1, 3]), ([2, 11], [5, 4])];
    let tree = FixedTree::try_new(&leaves, &branches).unwrap();
    let expected: Model<_, _> = leaves
        .iter()
        .flat_map(|(keys, values)| keys.iter().copied().zip(values.iter().copied()))
        .collect();
    let mut bounds = vec![Unbounded];
    for key in &[0, 2, 3, 4, 5, 7, 10, 11, 12, 13, 17, 19, 20] {
        bounds.extend([Included(key), Excluded(key)]);
    }
    for start in bounds.iter().copied() {
        for end in bounds.iter().copied() {
            let invalid = match (start, end) {
                (Excluded(a), Excluded(b)) => a >= b,
                (Included(a) | Excluded(a), Included(b) | Excluded(b)) => a > b,
                _ => false,
            };
            if invalid {
                assert!(matches!(tree.range(start, end), Err(Error::Bounds)));
                continue;
            }
            let (_, memory) = track(usize::MAX, || {
                let mut cursor = tree.range(start, end).unwrap();
                for item in expected.range::<i32, _>((start, end)) {
                    assert_eq!(cursor.try_next(), Ok(Some(item)));
                }
                assert_eq!(cursor.try_next(), Ok(None));
                assert_eq!(cursor.try_next(), Ok(None));
            });
            assert_eq!(memory.calls, 0);
        }
    }

    // A mixed-radix rank also crosses parent boundaries at non-power-of-two widths.
    let leaves: Vec<_> = (0..9)
        .rev()
        .map(|leaf| {
            let keys = std::array::from_fn(|slot| 2 * (leaf * 3 + slot) as i32);
            (keys, keys.map(|key| key * 10))
        })
        .collect::<Vec<([i32; 3], [i32; 3])>>();
    let branches = [
        ([0, 6, 12], [8, 7, 6]),
        ([18, 24, 30], [5, 4, 3]),
        ([36, 42, 48], [2, 1, 0]),
        ([0, 18, 36], [9, 10, 11]),
    ];
    let tree = FixedTree::try_new(&leaves, &branches).unwrap();
    let mut cursor = tree.range(Excluded(&16), Included(&48)).unwrap();
    for key in (18..=48).step_by(2) {
        assert_eq!(
            cursor.try_next().unwrap().map(|(&k, &v)| (k, v)),
            Some((key, key * 10))
        );
    }
    assert_eq!(cursor.try_next(), Ok(None));
    assert_eq!(cursor.try_next(), Ok(None));
}

#[test]
fn fixed_bplus_range_handles_empty_and_deep_single_child_snapshots() {
    use core::ops::Bound::{Excluded, Included, Unbounded};
    use jzap::collections::btree_map::view::bplus::FixedTree;

    let empty = FixedTree::<i32, i32, 1>::try_new(&[], &[]).unwrap();
    let mut range = empty.range::<i32>(Unbounded, Unbounded).unwrap();
    assert_eq!(range.try_next(), Ok(None));

    let leaves = [([String::from("entry")], [42])];
    let branches: Vec<_> = (0..=usize::BITS as usize)
        .map(|child| ([String::from("entry")], [child]))
        .collect();
    let deep = FixedTree::try_new(&leaves, &branches).unwrap();
    let (_, memory) = track(usize::MAX, || {
        let mut hit = deep.range(Included("entry"), Included("entry")).unwrap();
        assert_eq!(hit.try_next(), Ok(Some((&leaves[0].0[0], &42))));
        assert_eq!(hit.try_next(), Ok(None));
        let mut miss = deep.range(Excluded("entry"), Unbounded).unwrap();
        assert_eq!(miss.try_next(), Ok(None));
    });
    assert_eq!(memory.calls, 0);
}
