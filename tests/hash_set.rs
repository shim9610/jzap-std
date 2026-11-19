use jzap::{
    Error, TryClone, TryCloneIn, TryIterator,
    budget::{Budget, MemoryBudget},
    collections::{HashSet, hash_map::RandomState},
};
use std::{
    borrow::Borrow,
    cell::Cell,
    collections::HashSet as StdSet,
    hash::{BuildHasher, Hash, Hasher},
    rc::Rc,
};
mod common;
#[global_allocator]
static A: common::FaultAllocator = common::FaultAllocator;
fn set<T>() -> HashSet<T> {
    HashSet::with_hasher(RandomState::from_seed(0, 0))
}
fn rand(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn same<S: BuildHasher>(a: &HashSet<u64, S>, b: &StdSet<u64, S>) {
    assert_eq!(a.len(), b.len());
    for k in b {
        assert!(a.contains(k));
    }
    assert_eq!(a.iter().count(), b.len());
}
#[test]
fn differential_growth_replace_remove_and_shrink() {
    let mut a = set();
    let mut b = StdSet::with_hasher(RandomState::from_seed(0, 0));
    let mut seed = 9610;
    for i in 0..if cfg!(miri) { 300 } else { 15000 } {
        let k = rand(&mut seed) % 300;
        match rand(&mut seed) % 8 {
            0..=2 => assert_eq!(a.try_insert(k).unwrap(), b.insert(k)),
            3 => assert_eq!(a.try_replace(k).unwrap(), b.replace(k)),
            4 => assert_eq!(a.take(&k), b.take(&k)),
            5 => {
                a.try_reserve((k % 5) as usize).unwrap();
                b.reserve((k % 5) as usize);
            }
            6 => {
                a.try_shrink_to_fit().unwrap();
                b.shrink_to_fit();
            }
            _ => assert_eq!(a.get(&k), b.get(&k)),
        }
        if i % 17 == 0 {
            same(&a, &b);
        }
    }
    same(&a, &b);
}
#[derive(Default)]
struct CollisionState;
struct Collision;
impl Hasher for Collision {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _: &[u8]) {}
}
impl BuildHasher for CollisionState {
    type Hasher = Collision;
    fn build_hasher(&self) -> Collision {
        Collision
    }
}
impl TryClone for CollisionState {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self)
    }
}
#[test]
fn collisions_tombstones_and_cloned_mirrors_remain_searchable() {
    let mut a = HashSet::with_hasher(CollisionState);
    for k in 0u64..80 {
        a.try_insert(k).unwrap();
    }
    for k in 0..40 {
        assert!(a.remove(&k));
    }
    let mut b = a.try_clone().unwrap();
    drop(a);
    for k in 40..80 {
        assert!(b.contains(&k));
        assert_eq!(b.try_replace(k).unwrap(), Some(k));
    }
    for k in 0..40 {
        b.try_insert(k).unwrap();
    }
    b.try_reserve(160).unwrap();
    b.try_shrink_to_fit().unwrap();
    for k in 0..80 {
        assert!(b.contains(&k));
    }
    let old = b.capacity();
    b.try_shrink_to(usize::MAX).unwrap();
    assert_eq!(b.capacity(), old);
}
#[derive(Debug)]
struct Object {
    id: u64,
    serial: u64,
    drops: Rc<Cell<usize>>,
}
impl Hash for Object {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.id.hash(h);
    }
}
impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Object {}
impl Borrow<u64> for Object {
    fn borrow(&self) -> &u64 {
        &self.id
    }
}
impl Drop for Object {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
#[test]
fn duplicate_retains_old_object_replace_returns_old_without_allocation() {
    let drops = Rc::new(Cell::new(0));
    let object = |serial| Object {
        id: 7,
        serial,
        drops: drops.clone(),
    };
    let mut a = set();
    a.try_insert(object(1)).unwrap();
    let input = object(2);
    let (answer, state) = common::track(1, || a.try_insert(input));
    assert!(!answer.unwrap());
    assert_eq!(state.calls, 0);
    assert_eq!(drops.get(), 1);
    assert_eq!(a.get(&7).unwrap().serial, 1);
    let input = object(3);
    let capacity = a.capacity();
    let (answer, state) = common::track(1, || a.try_replace(input));
    let old = answer.unwrap().unwrap();
    assert_eq!(old.serial, 1);
    assert_eq!(state.calls, 0);
    assert_eq!(a.capacity(), capacity);
    assert_eq!(a.len(), 1);
    assert_eq!(a.get(&7).unwrap().serial, 3);
    assert_eq!(drops.get(), 1);
    drop(old);
    assert_eq!(drops.get(), 2);
    assert_eq!(a.take(&7).unwrap().serial, 3);
    assert_eq!(drops.get(), 3);
    assert!(a.is_empty());
    let mut empty = set();
    let input = object(4);
    let (answer, state) = common::track(1, || empty.try_replace(input));
    let rejected = answer.unwrap_err();
    assert_eq!(rejected.value.serial, 4);
    assert_eq!(state.calls, 1);
    assert_eq!(state.live, 0);
    assert_eq!(drops.get(), 3);
    assert!(empty.is_empty());
    assert!(empty.try_replace(rejected.value).unwrap().is_none());
    drop(empty);
    assert_eq!(drops.get(), 4);
}
#[test]
fn every_growth_refusal_returns_input_and_retains_contents_budget_and_capacity() {
    let count = if cfg!(miri) { 45 } else { 200 };
    let (_, baseline) = common::track(usize::MAX, || {
        let mut a = set();
        for k in 0..count {
            a.try_insert(k).unwrap();
        }
    });
    for fail in 1..=baseline.calls {
        let budget = MemoryBudget::new(1 << 20);
        {
            let mut a =
                HashSet::with_hasher_in(RandomState::from_seed(0, 0), Budget::Limited(&budget));
            let (result, state) = common::track(fail, || {
                for k in 0..count {
                    let cap = a.capacity();
                    let held = budget.occupied();
                    if let Err(e) = a.try_insert(k) {
                        assert_eq!(e.value, k);
                        assert_eq!(a.len(), k as usize);
                        assert_eq!(a.capacity(), cap);
                        assert_eq!(budget.occupied(), held);
                        return Some(k);
                    }
                }
                None
            });
            let refused = result.unwrap();
            assert_eq!(state.calls, fail);
            for k in 0..refused {
                assert!(a.contains(&k));
            }
            a.try_insert(refused).unwrap();
            assert!(a.contains(&refused));
        }
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
}
#[test]
fn clone_reserve_shrink_and_replace_refusals_are_transactional() {
    let budget = MemoryBudget::new(1 << 20);
    let mut a = HashSet::with_hasher_in(RandomState::from_seed(0, 0), Budget::Limited(&budget));
    for k in 0u64..35 {
        a.try_insert(k).unwrap();
    }
    a.try_reserve(500).unwrap();
    for operation in 0..3 {
        let held = budget.occupied();
        let cap = a.capacity();
        let (answer, state) = common::track(1, || match operation {
            0 => a.try_clone().map(drop),
            1 => a.try_reserve(1000),
            _ => a.try_shrink_to_fit(),
        });
        assert!(matches!(answer, Err(Error::Allocation { .. })));
        assert_eq!(state.calls, 1);
        assert_eq!(state.live, 0);
        assert_eq!(a.len(), 35);
        assert_eq!(a.capacity(), cap);
        assert_eq!(budget.occupied(), held);
        for k in 0..35 {
            assert!(a.contains(&k));
        }
    }
    let mut empty = HashSet::with_hasher_in(RandomState::from_seed(0, 0), Budget::Limited(&budget));
    let (answer, state) = common::track(1, || empty.try_replace(999));
    assert_eq!(answer.unwrap_err().value, 999);
    assert!(empty.is_empty());
    assert_eq!(state.live, 0);
    let held = budget.occupied();
    let reservation = budget.reserve(budget.limit() - held).unwrap();
    assert!(matches!(a.try_clone(), Err(Error::Budget { .. })));
    assert!(matches!(a.try_reserve(1000), Err(Error::Budget { .. })));
    assert_eq!(a.try_replace(8).unwrap(), Some(8));
    drop(reservation);
    drop(a);
    assert_eq!(budget.occupied(), 0);
    assert_eq!(budget.reserved(), 0);
}
fn algebra_iter<'a>(
    operation: usize,
    a: &'a HashSet<u64>,
    b: &'a HashSet<u64>,
) -> Box<dyn Iterator<Item = &'a u64> + 'a> {
    match operation {
        0 => Box::new(a.union(b)),
        1 => Box::new(a.intersection(b)),
        2 => Box::new(a.difference(b)),
        _ => Box::new(a.symmetric_difference(b)),
    }
}
#[test]
fn algebra_membership_uniqueness_clone_fusion_and_remaining_hints() {
    let samples = if cfg!(miri) { 8 } else { 96 };
    let mut seed = 74;
    for _ in 0..samples {
        let mut a = set();
        let mut b = set();
        let mut sa = StdSet::new();
        let mut sb = StdSet::new();
        for i in 0..24u64 {
            if rand(&mut seed) & 1 == 0 {
                a.try_insert(i).unwrap();
                sa.insert(i);
            }
            if rand(&mut seed) & 1 == 0 {
                b.try_insert(i).unwrap();
                sb.insert(i);
            }
        }
        assert_eq!(a.is_disjoint(&b), sa.is_disjoint(&sb));
        assert_eq!(a.is_subset(&b), sa.is_subset(&sb));
        assert_eq!(a.is_superset(&b), sa.is_superset(&sb));
        for operation in 0..4 {
            let expected: StdSet<u64> = match operation {
                0 => sa.union(&sb).copied().collect(),
                1 => sa.intersection(&sb).copied().collect(),
                2 => sa.difference(&sb).copied().collect(),
                _ => sa.symmetric_difference(&sb).copied().collect(),
            };
            let mut iter = algebra_iter(operation, &a, &b);
            let mut seen = StdSet::new();
            loop {
                let remaining = expected.len() - seen.len();
                let (lo, hi) = iter.size_hint();
                assert!(lo <= remaining);
                assert!(hi.is_none_or(|hi| hi >= remaining));
                let Some(&key) = iter.next() else { break };
                assert!(seen.insert(key));
                assert!(expected.contains(&key));
            }
            assert_eq!(seen, expected);
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
        assert_eq!(a.union(&b).clone().count(), sa.union(&sb).count());
        assert_eq!(
            a.intersection(&b).clone().count(),
            sa.intersection(&sb).count()
        );
        assert_eq!(a.difference(&b).clone().count(), sa.difference(&sb).count());
        assert_eq!(
            a.symmetric_difference(&b).clone().count(),
            sa.symmetric_difference(&sb).count()
        );
        assert_eq!(
            a.try_union(&b)
                .unwrap()
                .iter()
                .copied()
                .collect::<StdSet<_>>(),
            sa.union(&sb).copied().collect()
        );
        assert_eq!(
            a.try_intersection(&b)
                .unwrap()
                .iter()
                .copied()
                .collect::<StdSet<_>>(),
            sa.intersection(&sb).copied().collect()
        );
        assert_eq!(
            a.try_difference(&b)
                .unwrap()
                .iter()
                .copied()
                .collect::<StdSet<_>>(),
            sa.difference(&sb).copied().collect()
        );
        assert_eq!(
            a.try_symmetric_difference(&b)
                .unwrap()
                .iter()
                .copied()
                .collect::<StdSet<_>>(),
            sa.symmetric_difference(&sb).copied().collect()
        );
    }
}
#[test]
fn disjoint_uses_smaller_input_stops_at_first_match_and_never_allocates() {
    #[derive(Debug)]
    struct Counted<'a> {
        id: u64,
        hashes: &'a Cell<usize>,
    }
    impl Hash for Counted<'_> {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.hashes.set(self.hashes.get() + 1);
            self.id.hash(h);
        }
    }
    impl PartialEq for Counted<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Counted<'_> {}
    let large_hashes = Cell::new(0);
    let small_hashes = Cell::new(0);
    let mut large = set();
    let mut small = set();
    let empty = set();
    for id in 0..64 {
        large
            .try_insert(Counted {
                id,
                hashes: &large_hashes,
            })
            .unwrap();
    }
    for id in 100..103 {
        small
            .try_insert(Counted {
                id,
                hashes: &small_hashes,
            })
            .unwrap();
    }
    for reverse in [false, true] {
        large_hashes.set(0);
        small_hashes.set(0);
        let (result, allocation) = common::track(1, || {
            if reverse {
                small.is_disjoint(&large)
            } else {
                large.is_disjoint(&small)
            }
        });
        assert!(result);
        assert_eq!(allocation.calls, 0);
        assert_eq!(large_hashes.get(), 0);
        assert_eq!(small_hashes.get(), small.len());
    }
    large_hashes.set(0);
    assert!(!large.is_disjoint(&large));
    assert_eq!(large_hashes.get(), 1);
    large_hashes.set(0);
    assert!(large.is_disjoint(&empty));
    assert!(empty.is_disjoint(&large));
    assert_eq!(large_hashes.get(), 0);
    assert_eq!(large.len(), 64);
    assert_eq!(small.len(), 3);
}
#[derive(Debug)]
struct Cloned {
    id: u64,
    live: Rc<Cell<usize>>,
    step: Rc<Cell<usize>>,
    fail: usize,
}
impl Hash for Cloned {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.id.hash(h);
    }
}
impl PartialEq for Cloned {
    fn eq(&self, b: &Self) -> bool {
        self.id == b.id
    }
}
impl Eq for Cloned {}
impl Drop for Cloned {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}
impl TryClone for Cloned {
    fn try_clone(&self) -> Result<Self, Error> {
        let n = self.step.get() + 1;
        self.step.set(n);
        if n == self.fail {
            return Err(Error::Capacity);
        }
        self.live.set(self.live.get() + 1);
        Ok(Self {
            id: self.id,
            live: self.live.clone(),
            step: self.step.clone(),
            fail: self.fail,
        })
    }
}
#[test]
fn owned_algebra_and_clone_failures_destroy_only_the_private_prefix() {
    for (operation, length) in [(0, 12), (1, 4), (2, 4), (3, 8), (4, 8)] {
        for fail in 1..=length {
            let live = Rc::new(Cell::new(0));
            let step = Rc::new(Cell::new(0));
            {
                let mut a = set();
                let mut b = set();
                for i in 0..16 {
                    live.set(live.get() + 1);
                    let value = Cloned {
                        id: if i < 8 { i } else { i - 4 },
                        live: live.clone(),
                        step: step.clone(),
                        fail,
                    };
                    if i < 8 {
                        a.try_insert(value).unwrap();
                    } else {
                        b.try_insert(value).unwrap();
                    }
                }
                let answer = match operation {
                    0 => a.try_union(&b),
                    1 => a.try_intersection(&b),
                    2 => a.try_difference(&b),
                    3 => a.try_symmetric_difference(&b),
                    _ => a.try_clone(),
                };
                assert!(matches!(answer, Err(Error::Capacity)));
                assert_eq!(live.get(), 16);
                assert_eq!(a.len(), 8);
                assert_eq!(b.len(), 8);
            }
            assert_eq!(live.get(), 0);
        }
    }
}
#[test]
fn owned_algebra_injected_allocation_failure_drops_the_result_and_preserves_inputs() {
    let mut a = set();
    let mut b = set();
    for k in 0..35u64 {
        a.try_insert(k).unwrap();
        b.try_insert(k + 17).unwrap();
    }
    for operation in 0..4 {
        let run = || match operation {
            0 => a.try_union(&b),
            1 => a.try_intersection(&b),
            2 => a.try_difference(&b),
            _ => a.try_symmetric_difference(&b),
        };
        let (answer, baseline) = common::track(usize::MAX, || run().map(drop));
        answer.unwrap();
        assert_eq!(baseline.live, 0);
        for fail in 1..=baseline.calls {
            let (answer, state) = common::track(fail, run);
            assert!(matches!(answer, Err(Error::Allocation { .. })));
            assert_eq!(state.live, 0);
            assert_eq!(a.len(), 35);
            assert_eq!(b.len(), 35);
        }
    }
}
#[test]
fn retain_extract_errors_keep_the_current_value_and_allow_resume() {
    let mut a = set();
    for k in 0..20u64 {
        a.try_insert(k).unwrap();
    }
    let order: Vec<_> = a.iter().copied().collect();
    let mut visits = 0;
    assert_eq!(
        a.try_retain(|_| {
            visits += 1;
            if visits == 3 { Err(19) } else { Ok(false) }
        }),
        Err(19)
    );
    assert_eq!(a.len(), 18);
    assert!(a.contains(&order[2]));
    for k in &order[3..] {
        assert!(a.contains(k));
    }
    let mut visits = 0;
    let order: Vec<_> = a.iter().copied().collect();
    {
        let mut i = a.try_extract_if(|_| {
            visits += 1;
            if visits == 2 { Err(7) } else { Ok(true) }
        });
        assert!(i.try_next().unwrap().is_some());
        assert_eq!(i.try_next(), Err(7));
        assert!(i.try_next().unwrap().is_some());
    }
    assert_eq!(a.len(), 16);
    assert!(a.contains(&order[1]));
    let mut i = a.try_extract_if(|_| Ok::<_, ()>(false));
    assert!(format!("{i:?}").contains("ExtractIf"));
    assert_eq!(i.try_next(), Ok(None));
    assert_eq!(i.try_next(), Ok(None));
}
#[test]
fn source_errors_and_storage_refusal_retain_the_successful_prefix() {
    struct Source {
        i: u64,
    }
    impl TryIterator for Source {
        type Item = u64;
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<u64>, u8> {
            if self.i == 3 {
                return Err(9);
            }
            let k = self.i;
            self.i += 1;
            Ok(Some(k))
        }
    }
    let mut a = set();
    assert!(matches!(
        a.try_extend(Source { i: 0 }),
        Err(jzap::collections::hash_set::ExtendError::Source(9))
    ));
    assert_eq!(a.len(), 3);
    a.try_extend_copied([1, 2, 3].as_slice()).unwrap();
    assert_eq!(a.len(), 4);
    let (answer, _) = common::track(1, || {
        HashSet::try_from_iter_in(
            [4u64, 5, 6],
            RandomState::from_seed(0, 0),
            jzap::budget::Unmetered,
        )
    });
    match answer {
        Err(jzap::collections::hash_set::ExtendError::Insert(e)) => assert_eq!(e.value, 4),
        _ => panic!("expected rejected input"),
    }
}
#[test]
fn owning_borrowed_and_concurrent_budget_lifetimes_return_all_charges() {
    use jzap::budget::{OwnedBudget, OwnedConcurrentBudget};
    let local = OwnedBudget::try_new(1 << 20).unwrap();
    let concurrent = OwnedConcurrentBudget::try_new(1 << 20).unwrap();
    let target = MemoryBudget::new(1 << 20);
    {
        let mut a =
            HashSet::with_hasher_in(RandomState::from_seed(1, 2), local.try_clone().unwrap());
        let mut b = HashSet::with_hasher_in(
            RandomState::from_seed(1, 2),
            concurrent.try_clone().unwrap(),
        );
        for k in 0..20u64 {
            a.try_insert(k).unwrap();
            b.try_insert(k).unwrap();
        }
        assert_eq!(a, b);
        let c = a.try_clone_in(Budget::Limited(&target)).unwrap();
        assert_eq!(a, c);
        let d = b.try_clone().unwrap();
        drop(b);
        assert_eq!(d.len(), 20);
    }
    assert_eq!(local.occupied(), 0);
    assert_eq!(concurrent.used(), 0);
    assert_eq!(target.occupied(), 0);
    let moved = concurrent.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut a = HashSet::with_hasher_in(RandomState::from_seed(0, 0), moved);
        a.try_insert(7).unwrap();
    })
    .join()
    .unwrap();
    assert_eq!(concurrent.used(), 0);
}
#[test]
fn zero_size_large_alignment_and_copy_reference_provenance() {
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    #[repr(align(256))]
    struct Aligned<'a>(&'a u64);
    impl TryClone for Aligned<'_> {
        fn try_clone(&self) -> Result<Self, Error> {
            Ok(*self)
        }
        fn copy_witness() -> Option<jzap::CopyWitness<Self>> {
            Some(jzap::CopyWitness::new())
        }
    }
    let value = 47;
    let mut a = set();
    a.try_insert(Aligned(&value)).unwrap();
    let b = a.try_clone().unwrap();
    drop(a);
    let v = b.get(&Aligned(&value)).unwrap();
    assert!(std::ptr::eq(v.0, &value));
    assert_eq!((v as *const _ as usize) % 256, 0);
    let mut z = set();
    assert!(z.try_insert(()).unwrap());
    assert!(!z.try_insert(()).unwrap());
    assert_eq!(z.try_replace(()).unwrap(), Some(()));
    assert_eq!(z.try_clone().unwrap().len(), 1);
    let mut empty = set::<u8>();
    assert!(empty.try_reserve(usize::MAX).is_err());
    assert!(empty.is_empty());
    empty.try_reserve(8).unwrap();
    let c = empty.try_clone().unwrap();
    assert_eq!(c.capacity(), empty.capacity());
    drop(c);
    empty.try_shrink_to_fit().unwrap();
    assert_eq!(empty.capacity(), 0);
}
#[test]
fn replacement_moves_unique_references_without_aliasing_stored_keys() {
    let mut first = 11u64;
    let mut second = 11u64;
    let mut a = set();
    a.try_insert(&mut first).unwrap();
    let old = a.try_replace(&mut second).unwrap().unwrap();
    *old = 19;
    assert_eq!(**a.iter().next().unwrap(), 11);
    let stored = a.into_iter().next().unwrap();
    *stored = 23;
    assert_eq!(*old, 19);
    assert_eq!(*stored, 23);
}
#[test]
fn iterators_default_exact_folds_format_and_transactional_clone_from() {
    use jzap::collections::hash_set::{IntoIter, Iter};
    assert_eq!(Iter::<Object>::default().len(), 0);
    assert_eq!(IntoIter::<Object>::default().len(), 0);
    let mut a = set();
    a.try_extend([1u64, 2, 3]).unwrap();
    assert_eq!(a.iter().len(), 3);
    assert_eq!(a.iter().clone().count(), 3);
    assert_eq!(a.iter().fold(0, |s, k| s + k), 6);
    assert_eq!(a.try_clone().unwrap().into_iter().fold(0, |s, k| s + k), 6);
    assert!(format!("{:?}", a.iter()).contains('1'));
    assert!(format!("{:?}", a.union(&a)).contains('2'));
    assert!(format!("{:?}", a.try_clone().unwrap().into_iter()).contains('3'));
    let mut destination = set();
    destination.try_insert(9).unwrap();
    let (answer, _) = common::track(1, || destination.try_clone_from(&a));
    assert!(answer.is_err());
    assert!(destination.contains(&9));
    destination.try_clone_from(&a).unwrap();
    assert_eq!(a, destination);
    {
        let mut d = a.drain();
        assert_eq!(d.len(), 3);
        assert!(format!("{d:?}").contains('1'));
        let removed = d.next().unwrap();
        assert_eq!(d.len(), 2);
        let remaining = format!("{d:?}");
        assert!(!remaining.contains(&removed.to_string()));
        assert!(!remaining.contains("()"));
    }
    assert!(a.is_empty());
    a.try_insert(12).unwrap();
    assert_eq!(a.drain().fold(0, |s, k| s + k), 12);
    assert!(a.is_empty());
    assert_eq!(format!("{:?}", set::<u8>()), "{}");
}
#[test]
fn destructor_and_callback_unwind_retire_every_owned_value_once() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let drops = Rc::new(Cell::new(0));
    {
        let mut a = set();
        for k in 0..12 {
            a.try_insert(Object {
                id: k,
                serial: k,
                drops: drops.clone(),
            })
            .unwrap();
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| a
                .drain()
                .fold(0, |_, _| -> usize { panic!("callback") })))
            .is_err()
        );
        assert!(a.is_empty());
    }
    assert_eq!(drops.get(), 12);
    let drops = Rc::new(Cell::new(0));
    {
        let mut a = set();
        for k in 0..12 {
            a.try_insert(Object {
                id: k,
                serial: k,
                drops: drops.clone(),
            })
            .unwrap();
        }
        let mut i = a.into_iter();
        i.next();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || i.fold(0, |_, _| -> usize { panic!("callback") })
            ))
            .is_err()
        );
    }
    assert_eq!(drops.get(), 12);
}
#[test]
fn default_builder_entropy_and_array_creation_are_explicit() {
    #[cfg(feature = "std")]
    {
        let a = HashSet::<u64>::try_new().unwrap();
        assert!(a.is_empty());
        assert_eq!(HashSet::try_from_array([1u64, 1, 2]).unwrap().len(), 2);
        assert_eq!(
            jzap::TryDefault::try_default().map(|a: HashSet<u8>| a.len()),
            Ok(0)
        );
    }
    #[cfg(not(feature = "std"))]
    assert!(matches!(
        HashSet::<u64>::try_new(),
        Err(Error::Entropy { .. })
    ));
}

#[cfg(not(miri))]
proptest::proptest! {
    #[test]
    fn generated_state_sequences_match_std(ops in proptest::collection::vec((0u8..5, 0u64..80), 0..400)) {
        let mut a = set(); let mut b = StdSet::with_hasher(RandomState::from_seed(0, 0));
        for (op, k) in ops { match op { 0 => assert_eq!(a.try_insert(k).unwrap(), b.insert(k)), 1 => assert_eq!(a.try_replace(k).unwrap(), b.replace(k)), 2 => assert_eq!(a.remove(&k), b.remove(&k)), 3 => { a.try_shrink_to_fit().unwrap(); b.shrink_to_fit(); }, _ => assert_eq!(a.get(&k), b.get(&k)) }; same(&a, &b); }
    }
}
