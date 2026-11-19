use jzap::{
    TryClone, TryIterator,
    budget::{Budget, MemoryBudget},
    collections::hash_map::{Entry, HashMap, RandomState},
};
use std::{
    cell::Cell,
    collections::HashMap as StdMap,
    hash::{BuildHasher, Hash, Hasher},
    rc::Rc,
};
mod common;
#[global_allocator]
static A: common::FaultAllocator = common::FaultAllocator;
fn map<K, V>() -> HashMap<K, V> {
    HashMap::with_hasher(RandomState::from_seed(0, 0))
}
fn rand(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn same(m: &HashMap<u64, u64>, s: &StdMap<u64, u64, RandomState>) {
    assert_eq!(m.len(), s.len());
    for (k, v) in s {
        assert_eq!(m.get(k), Some(v))
    }
    let mut a = m.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>();
    let mut b = s.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>();
    a.sort();
    b.sort();
    assert_eq!(a, b)
}
#[test]
fn siphash_matches_std_streaming_and_hash_protocol() {
    let state = RandomState::from_seed(0, 0);
    for n in 0..600 {
        if cfg!(miri) && n > 32 && ![63, 64, 127, 128, 255, 256, 511, 512, 599].contains(&n) {
            continue;
        }
        let data = (0..n).map(|i| (i * 73) as u8).collect::<Vec<_>>();
        for split in [0, n / 3, n / 2, n] {
            let mut a = state.build_hasher();
            let mut b = std::collections::hash_map::DefaultHasher::new();
            for p in [&data[..split], &data[split..]] {
                a.write(p);
                b.write(p)
            }
            assert_eq!(a.finish(), b.finish(), "{n}/{split}");
        }
    }
    for value in [
        "",
        "hello",
        "한글🚀",
        "abcdefghijklmnopqrstuvwxyz0123456789",
    ] {
        let mut a = state.build_hasher();
        let mut b = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut a);
        value.hash(&mut b);
        assert_eq!(a.finish(), b.finish());
    }
}
#[test]
fn differential_structural_edits_and_rehash() {
    let mut m = map();
    let mut s = StdMap::with_hasher(RandomState::from_seed(0, 0));
    let mut seed = 1984;
    for i in 0..if cfg!(miri) { 500 } else { 20000 } {
        let k = rand(&mut seed) % 500;
        match rand(&mut seed) % 9 {
            0..=3 => assert_eq!(m.try_insert(k, i).unwrap(), s.insert(k, i)),
            4 => assert_eq!(m.remove(&k), s.remove(&k)),
            5 => {
                *m.entry(k).try_or_insert(0).unwrap() += 1;
                *s.entry(k).or_insert(0) += 1
            }
            6 => {
                m.try_reserve((k % 8) as usize).unwrap();
                s.reserve((k % 8) as usize)
            }
            7 => {
                m.try_shrink_to_fit().unwrap();
                s.shrink_to_fit()
            }
            _ => {
                assert_eq!(m.get(&k), s.get(&k));
            }
        }
        if i % 19 == 0 {
            same(&m, &s)
        }
    }
    same(&m, &s);
}
#[derive(Clone, Default)]
struct Collision;
impl Hasher for Collision {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _: &[u8]) {}
}
#[derive(Default)]
struct CollisionState;
impl BuildHasher for CollisionState {
    type Hasher = Collision;
    fn build_hasher(&self) -> Collision {
        Collision
    }
}
impl jzap::TryClone for CollisionState {
    fn try_clone(&self) -> Result<Self, jzap::Error> {
        Ok(Self::default())
    }
}
#[test]
fn collision_chains_delete_refill_and_inplace_cleanup() {
    let mut m = HashMap::with_hasher(CollisionState);
    for k in 0..100 {
        m.try_insert(k, k * 2).unwrap();
    }
    for cycle in 0..if cfg!(miri) { 2 } else { 20 } {
        for k in 0..100 {
            if k % 2 == cycle % 2 {
                assert_eq!(m.remove(&k), Some(k * 2));
            }
        }
        m.try_reserve(20).unwrap();
        for k in 0..100 {
            m.try_insert(k, k * 2).unwrap();
        }
        for k in 0..100 {
            assert_eq!(m.get(&k), Some(&(k * 2)));
        }
    }
    let c = m.try_clone().unwrap();
    assert_eq!(m, c);
}
#[test]
fn every_growth_allocation_refuses_preserves_and_retries() {
    let n = if cfg!(miri) { 70 } else { 400 };
    let (_, baseline) = common::track(usize::MAX, || {
        let mut m = map();
        for k in 0..n {
            m.try_insert(k, k * 2).unwrap();
        }
    });
    for ordinal in 1..=baseline.calls {
        let budget = MemoryBudget::new(usize::MAX);
        let (_, stats) = common::track(ordinal, || {
            let mut m =
                HashMap::with_hasher_in(RandomState::from_seed(0, 0), Budget::Limited(&budget));
            let mut failed = false;
            for k in 0..n {
                let old = m.capacity();
                let charge = budget.occupied();
                match m.try_insert(k, k * 2) {
                    Ok(None) => {}
                    Ok(Some(_)) => panic!(),
                    Err(e) => {
                        failed = true;
                        assert_eq!(e.value, (k, k * 2));
                        assert_eq!(m.len(), k as usize);
                        assert_eq!(m.capacity(), old);
                        assert_eq!(budget.occupied(), charge);
                        for j in 0..k {
                            assert_eq!(m.get(&j), Some(&(j * 2)));
                        }
                        m.try_insert(e.value.0, e.value.1).unwrap();
                    }
                }
            }
            assert!(failed);
        });
        assert_eq!(stats.live, 0);
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
}
#[test]
fn duplicates_do_not_allocate_and_retain_stored_key() {
    #[derive(Debug)]
    struct Key {
        value: u32,
        tag: u32,
    }
    impl Hash for Key {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.value.hash(h)
        }
    }
    impl PartialEq for Key {
        fn eq(&self, b: &Self) -> bool {
            self.value == b.value
        }
    }
    impl Eq for Key {}
    let budget = MemoryBudget::new(4096);
    let mut m = HashMap::with_hasher_in(RandomState::from_seed(0, 0), Budget::Limited(&budget));
    m.try_insert(Key { value: 7, tag: 10 }, 2).unwrap();
    let before = budget.occupied();
    let (old, stats) = common::track(1, || m.try_insert(Key { value: 7, tag: 99 }, 3).unwrap());
    assert_eq!(old, Some(2));
    assert_eq!(stats.calls, 0);
    assert_eq!(budget.occupied(), before);
    assert_eq!(m.iter().next().unwrap().0.tag, 10);
}
#[test]
fn entry_callbacks_errors_and_physical_disjointness() {
    let mut m = map();
    let (_, s) = common::track(1, || {
        let called = Cell::new(false);
        let r = m.entry(5).try_or_insert_with_key(|_| {
            called.set(true);
            Ok::<_, ()>(7)
        });
        assert!(r.is_err());
        assert!(!called.get());
    });
    assert_eq!(s.live, 0);
    assert!(m.is_empty());
    assert!(m.entry(5).try_or_insert_with(|| Err::<u64, _>(17)).is_err());
    assert!(m.is_empty());
    assert_eq!(
        *m.entry(5)
            .try_or_insert_with_key(|k| Ok::<_, ()>(*k + 1))
            .unwrap(),
        6
    );
    m.entry(5)
        .try_and_modify(|v| {
            *v += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(m.get(&5), Some(&7));
    for k in 10..20 {
        m.try_insert(k, k).unwrap();
    }
    assert!(m.try_get_disjoint_mut([&5, &5]).is_err());
    let [a, b, c] = m.try_get_disjoint_mut([&5, &10, &99]).unwrap();
    *a.unwrap() += 1;
    *b.unwrap() += 2;
    assert!(c.is_none());
    assert_eq!(m.get(&5), Some(&8));
    assert_eq!(m.get(&10), Some(&12));
    assert!(matches!(m.entry(5), Entry::Occupied(_)));
}
#[test]
fn mutable_iterator_drain_extract_and_owning_frontiers() {
    let mut m = map();
    for k in 0..80 {
        m.try_insert(k, k).unwrap();
    }
    {
        let mut it = m.iter_mut();
        let a = it.next().unwrap().1;
        let b = it.next().unwrap().1;
        *a += 100;
        *b += 200;
        assert_eq!(it.as_iter().len(), 78);
    }
    {
        let mut it = m.try_extract_if(|k, v| {
            *v += 1;
            if *k == 20 { Err(1) } else { Ok(*k % 2 == 0) }
        });
        let mut error = false;
        loop {
            match it.try_next() {
                Ok(Some(_)) => {}
                Err(_) => error = true,
                Ok(None) => break,
            }
        }
        assert!(error);
    }
    assert!(m.contains_key(&20));
    let capacity = m.capacity();
    {
        let mut d = m.drain();
        assert!(d.next().is_some());
        assert!(d.len() > 0);
    }
    assert!(m.is_empty());
    assert!(m.capacity() >= capacity);
    for k in 0..80 {
        m.try_insert(k, k).unwrap();
    }
    let mut owned = m.into_iter();
    assert!(owned.next().is_some());
    let rest = owned.as_iter().len();
    assert_eq!(rest, 79);
    assert_eq!(owned.count(), rest);
}
#[test]
fn clone_refusals_and_noncopy_payload_cleanup() {
    #[derive(Debug)]
    struct Value {
        live: Rc<Cell<usize>>,
        attempt: Rc<Cell<usize>>,
        fail: usize,
    }
    impl Drop for Value {
        fn drop(&mut self) {
            self.live.set(self.live.get() - 1)
        }
    }
    impl TryClone for Value {
        fn try_clone(&self) -> Result<Self, jzap::Error> {
            let n = self.attempt.get() + 1;
            self.attempt.set(n);
            if n == self.fail {
                return Err(jzap::Error::Capacity);
            }
            self.live.set(self.live.get() + 1);
            Ok(Self {
                live: self.live.clone(),
                attempt: self.attempt.clone(),
                fail: self.fail,
            })
        }
    }
    for fail in 1..=20 {
        let live = Rc::new(Cell::new(0));
        let attempt = Rc::new(Cell::new(0));
        {
            let mut m = map();
            for k in 0..20 {
                live.set(live.get() + 1);
                m.try_insert(
                    k,
                    Value {
                        live: live.clone(),
                        attempt: attempt.clone(),
                        fail,
                    },
                )
                .unwrap();
            }
            assert!(m.try_clone().is_err());
            assert_eq!(live.get(), 20);
        }
        assert_eq!(live.get(), 0);
    }
}
#[test]
fn copy_clone_publication_preserves_sparse_controls_provenance_and_refusals() {
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[repr(align(256))]
    struct Aligned<'a>(&'a u64);
    impl TryClone for Aligned<'_> {
        fn try_clone(&self) -> Result<Self, jzap::Error> {
            Ok(*self)
        }
        fn copy_witness() -> Option<jzap::CopyWitness<Self>> {
            Some(jzap::CopyWitness::new())
        }
    }
    let value = 71;
    for n in [0u64, 1, 3, 7, 17, 63] {
        let budget = MemoryBudget::new(1 << 24);
        {
            let mut source = HashMap::with_hasher_in(CollisionState, Budget::Limited(&budget));
            source.try_reserve(n.max(1) as usize).unwrap();
            for k in 0..n {
                source.try_insert(k, Aligned(&value)).unwrap();
            }
            // Includes long collision chains with deleted leading slots, plus
            // the nonempty allocation whose logical length is zero.
            for k in (0..n).step_by(2) {
                source.remove(&k);
            }
            let held = budget.occupied();
            let (failure, state) = common::track(1, || source.try_clone());
            assert!(matches!(failure, Err(jzap::Error::Allocation { .. })));
            assert_eq!(state.calls, 1);
            assert_eq!(state.live, 0);
            assert_eq!(budget.occupied(), held);
            let reservation = budget.reserve(budget.limit() - held).unwrap();
            let (failure, state) = common::track(usize::MAX, || source.try_clone());
            assert!(matches!(failure, Err(jzap::Error::Budget { .. })));
            assert_eq!(state.calls, 0);
            assert_eq!(budget.occupied(), held);
            drop(reservation);
            let (mut copy, state) = common::track(usize::MAX, || source.try_clone().unwrap());
            assert_eq!(state.calls, 1);
            assert_eq!(state.live as usize, held);
            assert_eq!(budget.occupied(), held * 2);
            assert_eq!(copy.len(), source.len());
            assert_eq!(copy.capacity(), source.capacity());
            for k in 0..n {
                assert_eq!(copy.get(&k), source.get(&k));
                if let Some(v) = copy.get(&k) {
                    assert!(std::ptr::eq(v.0, &value));
                    assert_eq!(v as *const _ as usize % 256, 0);
                }
            }
            drop(source);
            assert_eq!(budget.occupied(), held);
            // The copied tombstones/mirror remain usable after source death.
            copy.try_insert(1000, Aligned(&value)).unwrap();
            assert_eq!(copy.get(&1000), Some(&Aligned(&value)));
        }
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
    let empty = map::<u64, ()>();
    let (_, state) = common::track(1, || empty.try_clone().unwrap());
    assert_eq!(state.calls, 0);
    let mut zst = map::<(), ()>();
    zst.try_insert((), ()).unwrap();
    assert_eq!(zst.try_clone().unwrap().get(&()), Some(&()));
}
#[test]
fn zero_size_alignment_and_capacity_overflow() {
    #[derive(Debug, Hash, PartialEq, Eq)]
    #[repr(align(256))]
    struct Aligned(u64);
    let mut m = map();
    for i in 0..30 {
        m.try_insert(Aligned(i), Aligned(i + 1)).unwrap();
    }
    assert_eq!(m.get(&Aligned(2)).unwrap().0, 3);
    let mut z = map();
    z.try_insert((), ()).unwrap();
    assert_eq!(z.len(), 1);
    assert_eq!(z.try_insert((), ()).unwrap(), Some(()));
    assert_eq!(z.remove(&()), Some(()));
    let mut m = map::<u64, u64>();
    assert!(m.try_reserve(usize::MAX).is_err());
    assert!(m.is_empty());
    m.try_shrink_to_fit().unwrap();
}
#[cfg(all(feature = "std", target_os = "linux"))]
#[test]
fn default_entropy_builder_is_fallible_and_varies_keys() {
    let a = RandomState::try_new().unwrap();
    let b = RandomState::try_new().unwrap();
    assert_ne!(a.hash_one(19), b.hash_one(19));
    let mut m = HashMap::try_new().unwrap();
    m.try_insert(1, 2).unwrap();
    assert_eq!(m.get(&1), Some(&2));
}

#[derive(Eq, PartialEq)]
struct UnwindKey(u32, Rc<Cell<usize>>);
impl Hash for UnwindKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let left = self.1.get();
        if left == 0 {
            panic!("injected Hash unwind")
        }
        self.1.set(left - 1);
        self.0.hash(state);
    }
}
struct DropToken(Rc<Cell<usize>>, Rc<Cell<bool>>);
impl Drop for DropToken {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        if self.1.replace(false) {
            panic!("one destructor unwind")
        }
    }
}
#[test]
fn hash_unwind_at_every_resize_position_preserves_map_and_budget() {
    let budget = MemoryBudget::new(1 << 20);
    let steps = Rc::new(Cell::new(usize::MAX));
    let mut m = HashMap::with_hasher_in(RandomState::from_seed(0, 0), Budget::Limited(&budget));
    for k in 0..28u32 {
        m.try_insert(UnwindKey(k, steps.clone()), k).ok().unwrap();
    }
    let (capacity, charge) = (m.capacity(), budget.occupied());
    // Growth and shrinking both resize; unwind on the first through last key.
    for position in 0..28 {
        for grow in [true, false] {
            if !grow {
                m.try_reserve(100).unwrap();
            }
            let (before_capacity, before_charge) = (m.capacity(), budget.occupied());
            steps.set(position);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if grow {
                    m.try_reserve(100)
                } else {
                    m.try_shrink_to_fit()
                }
            }));
            steps.set(usize::MAX);
            assert!(result.is_err());
            assert_eq!((m.len(), m.capacity()), (28, before_capacity));
            assert_eq!(budget.occupied(), before_charge);
            for k in 0..28 {
                assert_eq!(m.get(&UnwindKey(k, steps.clone())), Some(&k));
            }
            m.try_shrink_to_fit().unwrap();
            assert_eq!((m.capacity(), budget.occupied()), (capacity, charge));
        }
    }
    drop(m);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn hash_unwind_during_growth_and_tombstone_cleanup_is_safe() {
    let steps = Rc::new(Cell::new(usize::MAX));
    let drops = Rc::new(Cell::new(0));
    let bomb = Rc::new(Cell::new(false));
    let mut m = map();
    for k in 0..28 {
        m.try_insert(
            UnwindKey(k, steps.clone()),
            DropToken(drops.clone(), bomb.clone()),
        )
        .ok()
        .unwrap();
    }
    // A Hash unwind during growth leaves the map unchanged, like std: the
    // destination held only unowned copies and frees its storage alone.
    let capacity = m.capacity();
    steps.set(3);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.try_reserve(100)));
    assert!(result.is_err());
    steps.set(usize::MAX);
    assert_eq!((m.len(), m.capacity(), drops.get()), (28, capacity, 0));
    for k in 0..28 {
        assert!(m.contains_key(&UnwindKey(k, steps.clone())));
    }
    assert_eq!(m.iter().count(), 28);
    m.clear();
    assert_eq!(drops.get(), 28);
    for k in 0..100 {
        m.try_insert(
            UnwindKey(k, steps.clone()),
            DropToken(drops.clone(), bomb.clone()),
        )
        .ok()
        .unwrap();
    }
    // Erase dense runs until tombstone capacity is below physical capacity.
    for k in 0..80 {
        drop(m.remove(&UnwindKey(k, steps.clone())));
    }
    let old = m.len();
    steps.set(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        m.try_reserve(m.capacity() + 1)
    }));
    assert!(result.is_err());
    steps.set(usize::MAX);
    // Cleanup must leave every remaining FULL pair usable exactly once.
    assert!(m.len() <= old);
    assert_eq!(m.iter().count(), m.len());
    drop(m);
    assert_eq!(drops.get(), 128);
}
#[test]
fn destructor_unwind_clear_drain_and_map_drop_retire_each_owner_once() {
    for mode in 0..3 {
        let drops = Rc::new(Cell::new(0));
        let bomb = Rc::new(Cell::new(false));
        let budget = MemoryBudget::new(1 << 20);
        let mut m = HashMap::with_hasher_in(RandomState::from_seed(1, 2), Budget::Limited(&budget));
        for k in 0..19 {
            m.try_insert(k, DropToken(drops.clone(), bomb.clone()))
                .ok()
                .unwrap();
        }
        bomb.set(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if mode == 0 {
                m.clear()
            } else if mode == 1 {
                drop(m.drain())
            } else {
                drop(m);
                return;
            }
            assert!(m.is_empty());
        }));
        assert!(result.is_err());
        assert_eq!(drops.get(), 19);
        assert_eq!(budget.occupied(), 0);
    }
}
#[test]
fn heterogeneous_lookup_and_fallible_source_preserve_prefix() {
    let mut m = map();
    m.try_insert(String::from("key"), 4).unwrap();
    assert_eq!(m.get("key"), Some(&4));
    assert_eq!(m.remove("key"), Some(4));
    assert!(m.is_empty());
    let mut n = map();
    n.try_extend([(1, 2), (3, 4), (1, 9)]).unwrap();
    assert_eq!(n.len(), 2);
    assert_eq!(n.get(&1), Some(&9));
    assert_eq!(format!("{:?}", n.keys()).contains('1'), true);
}

#[test]
fn source_error_keeps_accepted_prefix_and_duplicate_physical_slots_are_rejected() {
    struct Source(u8);
    impl TryIterator for Source {
        type Item = (u8, u8);
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<Self::Item>, u8> {
            self.0 += 1;
            if self.0 == 3 {
                Err(7)
            } else {
                Ok(Some((self.0, 9)))
            }
        }
    }
    let mut m = map();
    assert!(matches!(
        m.try_extend(Source(0)),
        Err(jzap::collections::hash_map::ExtendError::Source(7))
    ));
    assert_eq!(m.len(), 2);
    assert_eq!(m.get(&2), Some(&9));
    // Deliberately inconsistent Eq is a logic error, but cannot permit aliasing.
    #[derive(Hash)]
    struct BadEq(u8);
    impl PartialEq for BadEq {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }
    impl Eq for BadEq {}
    let mut m = HashMap::with_hasher(CollisionState);
    m.try_insert(BadEq(0), 42u8).ok().unwrap();
    let (a, b) = (BadEq(1), BadEq(2));
    assert!(matches!(
        m.try_get_disjoint_mut([&a, &b]),
        Err(jzap::Error::Bounds)
    ));
}
#[test]
fn owned_policies_and_clone_in_release_all_ledgers() {
    use jzap::{
        TryCloneIn,
        budget::{OwnedBudget, OwnedConcurrentBudget},
    };
    let local = OwnedBudget::try_new(1 << 20).unwrap();
    let concurrent = OwnedConcurrentBudget::try_new(1 << 20).unwrap();
    {
        let mut a =
            HashMap::with_hasher_in(RandomState::from_seed(3, 4), local.try_clone().unwrap());
        let mut b = HashMap::with_hasher_in(
            RandomState::from_seed(3, 4),
            concurrent.try_clone().unwrap(),
        );
        for k in 0..39u64 {
            a.try_insert(k, k * 2).unwrap();
            b.try_insert(k, k * 2).unwrap();
        }
        let ledger = MemoryBudget::new(1 << 20);
        let c = a.try_clone_in(Budget::Limited(&ledger)).unwrap();
        assert_eq!(c, b);
        drop(c);
        assert_eq!(ledger.occupied(), 0);
        let d = b.try_clone().unwrap();
        drop(b);
        assert_eq!(d.len(), 39);
    }
    assert_eq!(
        (local.occupied(), local.reserved(), concurrent.used()),
        (0, 0, 0)
    );
}

#[test]
fn entry_surface_iterator_defaults_and_transactional_replacement() {
    use jzap::collections::hash_map::{IntoIter, Iter, Keys, Values};
    let mut m = map();
    assert_eq!(*m.entry(1u64).try_or_default().unwrap(), 0u64);
    assert_eq!(m.entry(1).key(), &1);
    assert_eq!(
        *m.entry(1).try_or_insert_with(|| Err::<u64, _>(7)).unwrap(),
        0
    );
    let mut e = m.entry(2).try_insert_entry(4).unwrap();
    assert_eq!(e.key(), &2);
    assert_eq!(e.get(), &4);
    assert_eq!(e.insert(5), 4);
    *e.get_mut() = 6;
    assert_eq!(*e.into_mut(), 6);
    match m.entry(2) {
        Entry::Occupied(e) => assert_eq!(e.remove_entry(), (2, 6)),
        _ => panic!("occupied"),
    }
    match m.entry(3) {
        Entry::Vacant(e) => {
            assert_eq!(e.key(), &3);
            assert_eq!(e.into_key(), 3)
        }
        _ => panic!("vacant"),
    }
    match m.entry(1) {
        Entry::Occupied(e) => assert_eq!(e.remove(), 0),
        _ => panic!("occupied"),
    }
    m.entry(4).try_or_insert_with(|| Ok::<_, ()>(8)).unwrap();
    m.entry(4)
        .try_and_modify(|v| {
            *v += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(m.get(&4), Some(&9));
    assert_eq!(
        m.entry(5)
            .try_and_modify(|_| Err(7))
            .unwrap()
            .try_or_insert(10)
            .unwrap(),
        &10
    );
    assert_eq!(m.iter_mut().as_iter().len(), 2);
    assert_eq!(m.iter().clone().count(), 2);
    assert_eq!(m.values().clone().count(), 2);
    assert_eq!(Iter::<u64, u64>::default().len(), 0);
    assert_eq!(Keys::<u64, u64>::default().count(), 0);
    assert_eq!(Values::<u64, u64>::default().len(), 0);
    assert_eq!(IntoIter::<u64, u64>::default().count(), 0);
    assert_eq!(
        jzap::collections::hash_map::IterMut::<u64, u64>::default().len(),
        0
    );
    assert_eq!(
        jzap::collections::hash_map::ValuesMut::<u64, u64>::default().len(),
        0
    );
    assert_eq!(
        jzap::collections::hash_map::IntoKeys::<u64, u64>::default().len(),
        0
    );
    assert_eq!(
        jzap::collections::hash_map::IntoValues::<u64, u64>::default().len(),
        0
    );
    let extract = m.try_extract_if(|_, _| Ok::<_, ()>(false));
    assert_eq!(format!("{extract:?}"), "ExtractIf { .. }");
    drop(extract);
    assert_eq!(m.try_clone().unwrap().into_keys().count(), 2);
    assert_eq!(m.try_clone().unwrap().into_values().count(), 2);
    let mut d = map();
    d.try_insert(99, 7).unwrap();
    let (r, calls) = common::track(1, || d.try_clone_from(&m));
    assert!(r.is_err());
    assert_eq!(calls.calls, 1);
    assert_eq!(d.get(&99), Some(&7));
    d.try_clone_from(&m).unwrap();
    assert_eq!(d, m);
    let cap = m.capacity();
    let (r, calls) = common::track(1, || m.try_reserve(4096));
    assert!(r.is_err());
    assert_eq!(calls.calls, 1);
    assert_eq!(m.capacity(), cap);
    assert_eq!(m.get(&4), Some(&9));
}

#[test]
fn copy_without_optin_keeps_fallible_clone_and_unchecked_disjoint_honors_its_contract() {
    #[derive(Copy, Clone, Debug)]
    struct Fallible(u8);
    impl TryClone for Fallible {
        fn try_clone(&self) -> Result<Self, jzap::Error> {
            if self.0 == 0 {
                Err(jzap::Error::Capacity)
            } else {
                Ok(*self)
            }
        }
    }
    let mut m = map();
    m.try_insert(1u8, Fallible(0)).unwrap();
    assert!(matches!(m.try_clone(), Err(jzap::Error::Capacity)));
    assert_eq!(m.get(&1).unwrap().0, 0);
    let mut m = map();
    m.try_extend([(1u8, 2u8), (3, 4)]).unwrap();
    // SAFETY: two different existing keys, no repeated physical slot.
    let [a, b, c] = unsafe { m.get_disjoint_unchecked_mut([&1, &3, &9]) };
    *a.unwrap() = 7;
    *b.unwrap() = 8;
    assert!(c.is_none());
    assert_eq!(m.get(&1), Some(&7));
    assert_eq!(m.get(&3), Some(&8));
}

#[test]
fn no_drop_consumers_preserve_unique_mutable_reference_ownership() {
    let (mut a, mut b, mut c) = (1u64, 2u64, 3u64);
    let mut m = map();
    m.try_extend([(1u8, &mut a), (2, &mut b), (3, &mut c)])
        .unwrap();
    let mut it = m.into_iter();
    let (_, first) = it.next().unwrap();
    assert_eq!(it.as_iter().count(), 2);
    *first += 10;
    let (_, second) = it.next().unwrap();
    *second += 20;
    assert_eq!(it.as_iter().count(), 1);
    drop(it);
    assert_eq!(a + b + c, 36);
    let mut m = map();
    m.try_extend([(1u8, &mut a), (2, &mut b), (3, &mut c)])
        .unwrap();
    let (_, value) = m.drain().next().unwrap();
    *value += 4;
    assert!(m.is_empty());
    drop(m);
    assert_eq!(a + b + c, 40);
}
#[test]
fn standard_fixed_size_builders_clone_without_new_entropy_or_allocations() {
    use std::hash::BuildHasherDefault;
    type Builder = BuildHasherDefault<std::collections::hash_map::DefaultHasher>;
    let mut m = HashMap::<u64, u64, Builder>::with_hasher(Builder::default());
    m.try_insert(3, 7).unwrap();
    assert_eq!(m.try_clone().unwrap().get(&3), Some(&7));
    // This adapter belongs to the optional std feature; the core builder
    // above remains exercised in alloc-only builds.
    #[cfg(feature = "std")]
    {
        let builder = std::collections::hash_map::RandomState::new();
        let hash = builder.hash_one(7u8);
        let (clone, state) = common::track(1, || TryClone::try_clone(&builder).unwrap());
        assert_eq!(state.calls, 0);
        assert_eq!(clone.hash_one(7u8), hash);
    }
}

#[test]
fn tombstone_shrink_matches_std_and_refusal_preserves_the_original() {
    let budget = MemoryBudget::new(1 << 20);
    let mut m = HashMap::with_hasher_in(CollisionState, Budget::Limited(&budget));
    let mut s = StdMap::with_hasher(CollisionState);
    for k in 0..112u64 {
        m.try_insert(k, k * 3).unwrap();
        s.insert(k, k * 3);
    }
    for k in 0..96 {
        assert_eq!(m.remove(&k), s.remove(&k));
    }
    assert_eq!(m.capacity(), m.len());
    assert_eq!(m.capacity(), s.capacity());
    let held = budget.occupied();
    let (result, state) = common::track(1, || m.try_shrink_to_fit());
    assert!(result.is_err());
    assert_eq!(state.calls, 1);
    assert_eq!(budget.occupied(), held);
    for (k, v) in &s {
        assert_eq!(m.get(k), Some(v));
    }
    m.try_shrink_to_fit().unwrap();
    s.shrink_to_fit();
    assert_eq!(m.capacity(), s.capacity());
    assert!(budget.occupied() < held);
    for (k, v) in &s {
        assert_eq!(m.get(k), Some(v));
    }
    let cap = m.capacity();
    let (result, state) = common::track(1, || m.try_shrink_to(usize::MAX));
    assert!(result.is_ok());
    assert_eq!(state.calls, 0);
    assert_eq!(m.capacity(), cap);
}
#[test]
fn entry_initializer_reuses_a_tombstone_without_allocating() {
    let budget = MemoryBudget::new(1 << 20);
    let mut m = HashMap::with_hasher_in(CollisionState, Budget::Limited(&budget));
    for k in 0..112u64 {
        m.try_insert(k, k).unwrap();
    }
    assert_eq!(m.remove(&0), Some(0));
    assert_eq!(m.capacity(), m.len());
    let occupied = budget.occupied();
    let calls = Cell::new(0);
    let (result, state) = common::track(1, || {
        m.entry(999)
            .try_or_insert_with(|| {
                calls.set(calls.get() + 1);
                Ok::<_, ()>(77)
            })
            .map(|v| *v)
    });
    assert_eq!(result.unwrap(), 77);
    assert_eq!(state.calls, 0);
    assert_eq!(calls.get(), 1);
    assert_eq!(budget.occupied(), occupied);
    assert_eq!(m.get(&999), Some(&77));
    assert_eq!(m.remove(&999), Some(77));
    let (result, state) = common::track(1, || {
        m.entry(999)
            .try_or_insert_with_key(|k| {
                assert_eq!(*k, 999);
                Err::<u64, _>(7)
            })
            .map(|v| *v)
    });
    assert!(matches!(result, Err(jzap::error::InitError::Initialize(7))));
    assert_eq!(state.calls, 0);
    assert_eq!(budget.occupied(), occupied);
    assert!(!m.contains_key(&999));
}

#[test]
fn grouped_folds_keep_mutable_loans_and_unwind_ownership_exact() {
    let mut m = map();
    m.try_extend([(1u64, 2u64), (3, 4), (5, 6)]).unwrap();
    let values = m.iter_mut().fold(Vec::new(), |mut out, (_, v)| {
        out.push(v);
        out
    });
    for v in values {
        *v += 1;
    }
    assert_eq!(m.values().fold(0, |s, v| s + v), 15);
    for drain in [false, true] {
        let budget = MemoryBudget::new(1 << 20);
        let drops = Rc::new(Cell::new(0));
        let bomb = Rc::new(Cell::new(false));
        let mut m = HashMap::with_hasher_in(RandomState::from_seed(0, 0), Budget::Limited(&budget));
        for k in 0..19 {
            m.try_insert(k, DropToken(drops.clone(), bomb.clone()))
                .ok()
                .unwrap();
        }
        if drain {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                m.drain().fold((), |(), _pair| panic!("fold callback"))
            }));
            assert!(result.is_err());
            assert_eq!(drops.get(), 19);
            assert!(m.is_empty());
            drop(m);
        } else {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                m.into_iter().fold((), |(), _pair| panic!("fold callback"))
            }));
            assert!(result.is_err());
            assert_eq!(drops.get(), 19);
        }
        assert_eq!(budget.occupied(), 0);
    }
}

/// One hasher input: bytes, a fixed-width integer or a string.
#[cfg(not(miri))]
#[derive(Clone, Debug)]
enum HashInput {
    Bytes(Vec<u8>),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
    Str(String),
}
#[cfg(not(miri))]
fn feed(h: &mut impl Hasher, input: &HashInput) {
    match input {
        HashInput::Bytes(b) => h.write(b),
        HashInput::U8(v) => h.write_u8(*v),
        HashInput::U16(v) => h.write_u16(*v),
        HashInput::U32(v) => h.write_u32(*v),
        HashInput::U64(v) => h.write_u64(*v),
        HashInput::U128(v) => h.write_u128(*v),
        HashInput::Usize(v) => h.write_usize(*v),
        HashInput::Str(s) => s.hash(h),
    }
}
#[cfg(not(miri))]
use proptest::strategy::Strategy as _;
#[cfg(not(miri))]
proptest::proptest! {
    /// SipHash-1-3 with keys (0, 0) must equal std's `DefaultHasher::new()` for
    /// every split of the input stream, including lengths beyond 255 bytes.
    #[test]
    fn sip_hasher_matches_std_default_hasher(inputs in proptest::collection::vec(proptest::prop_oneof![
        proptest::collection::vec(proptest::num::u8::ANY, 0..40).prop_map(HashInput::Bytes),
        proptest::num::u8::ANY.prop_map(HashInput::U8),
        proptest::num::u16::ANY.prop_map(HashInput::U16),
        proptest::num::u32::ANY.prop_map(HashInput::U32),
        proptest::num::u64::ANY.prop_map(HashInput::U64),
        proptest::num::u128::ANY.prop_map(HashInput::U128),
        proptest::num::usize::ANY.prop_map(HashInput::Usize),
        ".{0,12}".prop_map(HashInput::Str),
    ], 0..40)) {
        let mut ours = jzap::collections::hash_map::SipHasher13::new(0, 0);
        let mut standard = std::hash::DefaultHasher::new();
        for input in &inputs {
            feed(&mut ours, input);
            feed(&mut standard, input);
            proptest::prop_assert_eq!(ours.finish(), standard.finish());
        }
    }
}
#[test]
fn sip_hasher_matches_std_for_long_and_split_inputs() {
    let data = (0..1000u32)
        .map(|i| (i.wrapping_mul(31) ^ (i >> 3)) as u8)
        .collect::<Vec<_>>();
    for split in [0usize, 1, 3, 7, 8, 9, 15, 16, 17, 255, 256, 257, 999] {
        let mut ours = jzap::collections::hash_map::SipHasher13::new(0, 0);
        let mut standard = std::hash::DefaultHasher::new();
        for chunk in data.chunks(split.max(1)) {
            ours.write(chunk);
            standard.write(chunk);
            assert_eq!(ours.finish(), standard.finish(), "split {split}");
        }
    }
    // Integer writes after every buffered tail length 0..=7 and in sequence:
    // the dedicated integer path must equal std's byte path on any endianness.
    for prefix in 0..8usize {
        for width in 0..6 {
            let mut ours = jzap::collections::hash_map::SipHasher13::new(0, 0);
            let mut standard = std::hash::DefaultHasher::new();
            ours.write(&data[..prefix]);
            standard.write(&data[..prefix]);
            for step in 0..9u64 {
                let value = 0x0123_4567_89ab_cdefu64.rotate_left((step * 7) as u32) ^ step;
                for h in [&mut ours as &mut dyn Hasher, &mut standard] {
                    match (width + step as usize) % 6 {
                        0 => h.write_u8(value as u8),
                        1 => h.write_u16(value as u16),
                        2 => h.write_u32(value as u32),
                        3 => h.write_u64(value),
                        4 => h.write_usize(value as usize),
                        _ => h.write_i64(value as i64),
                    }
                }
                assert_eq!(
                    ours.finish(),
                    standard.finish(),
                    "prefix {prefix} width {width} step {step}"
                );
            }
        }
    }
    // Keyed hashing: two different jzap keys must disagree.
    let mut a = jzap::collections::hash_map::SipHasher13::new(1, 2);
    let mut b = jzap::collections::hash_map::SipHasher13::new(2, 1);
    a.write_u64(7);
    b.write_u64(7);
    assert_ne!(a.finish(), b.finish());
}
