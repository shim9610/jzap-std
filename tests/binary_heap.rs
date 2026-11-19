use jzap::{
    Error, TryClone, TryCloneIn, TryExactSizeIterator, TryIterator,
    budget::{Budget, MemoryBudget},
    collections::{
        BinaryHeap,
        binary_heap::{ExtendError, PeekMut},
    },
};
use std::{cell::Cell, cmp::Reverse, collections::BinaryHeap as StdHeap, rc::Rc};
mod common;
#[path = "common/deallocations.rs"]
mod deallocations;
#[global_allocator]
static A: common::FaultAllocator = common::FaultAllocator;
fn random(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn valid<T: Ord, P: jzap::budget::AllocationPolicy, L: jzap::vec::LengthPolicy>(
    heap: &BinaryHeap<T, P, L>,
) {
    for i in 1..heap.len() {
        assert!(heap.as_slice()[(i - 1) / 2] >= heap.as_slice()[i]);
    }
}
#[test]
fn differential_random_lifecycle() {
    let mut a = BinaryHeap::new();
    let mut b = StdHeap::new();
    let mut seed = 9610;
    for _ in 0..if cfg!(miri) { 250 } else { 20000 } {
        let value = random(&mut seed) % 10000;
        match random(&mut seed) % 10 {
            0..=3 => {
                a.try_push(value).unwrap();
                b.push(value);
            }
            4 => assert_eq!(a.pop(), b.pop()),
            5 => {
                if let Some(mut p) = a.peek_mut() {
                    *p = value;
                }
                if let Some(mut p) = b.peek_mut() {
                    *p = value;
                }
            }
            6 => {
                let predicate = |x: &u64| *x % 3 != 0;
                a.try_retain(|x| Ok::<_, ()>(predicate(x))).unwrap();
                b.retain(predicate);
            }
            7 => {
                a.try_reserve(value as usize % 5).unwrap();
                b.reserve(value as usize % 5);
            }
            8 => {
                a.try_shrink_to_fit().unwrap();
                b.shrink_to_fit();
            }
            _ => {
                let mut c = BinaryHeap::try_from_array([value, value / 2, 0]).unwrap();
                let mut d = StdHeap::from([value, value / 2, 0]);
                a.try_append(&mut c).unwrap();
                b.append(&mut d);
                assert!(c.is_empty());
            }
        }
        assert_eq!(a.len(), b.len());
        assert_eq!(a.peek(), b.peek());
        valid(&a);
    }
    assert_eq!(a.into_sorted_vec().as_slice(), b.into_sorted_vec());
}
#[test]
fn constructors_sorted_recovery_and_allocation_identity() {
    for n in 0..if cfg!(miri) { 15 } else { 100 } {
        for direction in 0..3 {
            let mut seed = 617;
            let values: Vec<u64> = (0..n)
                .map(|i| match direction {
                    0 => i,
                    1 => n - i,
                    _ => random(&mut seed),
                })
                .collect();
            let mut data = jzap::vec::Vec::new();
            data.try_extend_copy(&values).unwrap();
            let ptr = data.as_ptr();
            let cap = data.capacity();
            let heap = BinaryHeap::from_vec(data);
            assert_eq!(heap.as_slice().as_ptr(), ptr);
            assert_eq!(heap.capacity(), cap);
            valid(&heap);
            let sorted = heap.into_sorted_vec();
            let mut expected = values;
            expected.sort();
            assert_eq!(sorted.as_slice(), expected);
            assert_eq!(sorted.as_ptr(), ptr);
        }
    }
    let mut min = BinaryHeap::try_from_array([Reverse(3), Reverse(1), Reverse(2)]).unwrap();
    let cloned = min.try_clone().unwrap();
    assert_eq!(cloned.peek(), Some(&Reverse(1)));
    assert_eq!(min.pop(), Some(Reverse(1)));
}
#[test]
fn allocation_refusal_returns_input_and_preserves_both_owners() {
    let mut heap = BinaryHeap::try_with_capacity(4).unwrap();
    for n in 0..4 {
        heap.try_push(n).unwrap();
    }
    let before = heap.as_slice().to_vec();
    let cap = heap.capacity();
    let (result, _) = common::track(1, || heap.try_push(99));
    let error = result.unwrap_err();
    assert_eq!(error.value, 99);
    assert!(matches!(error.error, Error::Allocation { .. }));
    assert_eq!(heap.as_slice(), before);
    assert_eq!(heap.capacity(), cap);
    let mut other = BinaryHeap::try_from_array([6, 7, 8, 9, 10, 11, 12, 13]).unwrap();
    let old = other.as_slice().to_vec();
    let (result, _) = common::track(1, || heap.try_append(&mut other));
    assert!(matches!(result, Err(Error::Allocation { .. })));
    assert_eq!(heap.as_slice(), before);
    assert_eq!(other.as_slice(), old);
    let (result, _) = common::track(1, || BinaryHeap::try_from_array([5, 4, 3]));
    assert_eq!(result.unwrap_err().value, [5, 4, 3]);
}
#[test]
fn budgets_limits_and_owned_domains_stay_separate() {
    let budget = MemoryBudget::new(32);
    let mut heap = BinaryHeap::new_in(Budget::Limited(&budget));
    for n in 0u64..4 {
        heap.try_push(n).unwrap();
    }
    let used = budget.occupied();
    let err = heap.try_push(10).unwrap_err();
    assert!(matches!(err.error, Error::Budget { .. }));
    assert_eq!(err.value, 10);
    assert_eq!(budget.occupied(), used);
    valid(&heap);
    let other_budget = MemoryBudget::new(1024);
    let mut other =
        BinaryHeap::try_from_array_in([9u64, 8], Budget::Limited(&other_budget)).unwrap();
    assert!(heap.try_append(&mut other).is_err());
    assert_eq!(other.len(), 2);
    assert_eq!(other_budget.occupied(), 16);
    drop(heap);
    assert_eq!(budget.occupied(), 0);
    drop(other);
    assert_eq!(other_budget.occupied(), 0);
    let data = jzap::vec::Vec::with_limit(2);
    let mut heap = BinaryHeap::from_vec(data);
    heap.try_push(1).unwrap();
    heap.try_push(2).unwrap();
    assert_eq!(heap.try_push(3).unwrap_err().error, Error::Capacity);
    assert_eq!(heap.max_len(), 2);
}
#[test]
fn metered_append_exact_growth_preserves_admission_and_refusal() {
    for refusal in 0..3 {
        // Original small-receiver growth needs exactly 9 new u64 slots while
        // the original 1+8 slots remain charged. Large-side doubling would fail.
        let budget = MemoryBudget::new(if refusal == 1 { 143 } else { 144 });
        let mut a = BinaryHeap::try_from_array_in([20u64], Budget::Limited(&budget)).unwrap();
        let mut b =
            BinaryHeap::try_from_array_in([1u64, 2, 3, 4, 5, 6, 7, 8], Budget::Limited(&budget))
                .unwrap();
        let before_a = a.as_slice().to_vec();
        let before_b = b.as_slice().to_vec();
        let pointers = (a.as_slice().as_ptr(), b.as_slice().as_ptr());
        let (result, _) = common::track(usize::from(refusal == 2), || a.try_append(&mut b));
        if refusal == 0 {
            result.unwrap();
            valid(&a);
            assert_eq!(a.len(), 9);
            assert_eq!(a.capacity(), 9);
            assert!(b.is_empty());
            assert_eq!(b.capacity(), 1);
            assert_eq!(budget.occupied(), 80);
        } else {
            let error = result.unwrap_err();
            assert!(if refusal == 1 {
                matches!(error, Error::Budget { .. })
            } else {
                matches!(error, Error::Allocation { .. })
            });
            assert_eq!(a.as_slice(), before_a);
            assert_eq!(b.as_slice(), before_b);
            assert_eq!((a.capacity(), b.capacity()), (1, 8));
            assert_eq!((a.as_slice().as_ptr(), b.as_slice().as_ptr()), pointers);
            assert_eq!(budget.occupied(), 72);
        }
        drop((a, b));
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
}
#[test]
fn metered_append_keeps_original_spare_capacity_and_distinct_limits() {
    let budget = MemoryBudget::new(192);
    let mut a = BinaryHeap::try_with_capacity_in(16, Budget::Limited(&budget)).unwrap();
    a.try_push(20u64).unwrap();
    let mut b =
        BinaryHeap::try_from_array_in([1u64, 2, 3, 4, 5, 6, 7, 8], Budget::Limited(&budget))
            .unwrap();
    assert_eq!(budget.available(), 0);
    let (result, _) = common::track(1, || a.try_append(&mut b));
    result.unwrap(); // A no-allocation append must not acquire a new reservation.
    assert_eq!((a.capacity(), b.capacity()), (16, 8));
    valid(&a);
    drop((a, b));
    assert_eq!(budget.occupied(), 0);

    let mut a = BinaryHeap::from_vec(jzap::vec::Vec::with_limit_in(20, Budget::Limited(&budget)));
    let mut b = BinaryHeap::from_vec(jzap::vec::Vec::with_limit_in(8, Budget::Limited(&budget)));
    a.try_push(20u64).unwrap();
    b.try_extend([1u64, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    a.try_append(&mut b).unwrap();
    assert_eq!((a.max_len(), b.max_len()), (20, 8));
    valid(&a);
    drop((a, b));
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn metered_append_preserves_policy_handles_and_concurrent_domains() {
    let budget = MemoryBudget::new(usize::MAX);
    let policy_a = Budget::Limited(&budget);
    let policy_b = Budget::Limited(&budget);
    let drops = Rc::new(Cell::new(0));
    let object = |key| Object {
        key,
        drops: drops.clone(),
        clone_fail: false,
    };
    let mut a = BinaryHeap::try_from_array_in([object(9)], &policy_a).unwrap();
    let mut b =
        BinaryHeap::try_from_array_in([object(1), object(4), object(3)], &policy_b).unwrap();
    a.try_append(&mut b).unwrap();
    assert!(std::ptr::eq(*a.allocation_policy(), &policy_a));
    assert!(std::ptr::eq(*b.allocation_policy(), &policy_b));
    valid(&a);
    drop((a, b));
    assert_eq!(drops.get(), 4);
    assert_eq!(budget.occupied(), 0);

    let distinct = MemoryBudget::new(usize::MAX);
    let mut a = BinaryHeap::try_from_array_in([9u64], Budget::Limited(&budget)).unwrap();
    let mut b = BinaryHeap::try_from_array_in([1u64, 4, 3], Budget::Limited(&distinct)).unwrap();
    a.try_append(&mut b).unwrap();
    assert_eq!(b.capacity(), 3);
    assert_eq!(distinct.occupied(), 24);
    drop((a, b));
    assert_eq!(budget.occupied(), 0);
    assert_eq!(distinct.occupied(), 0);

    use jzap::budget::{ConcurrentBudget, ConcurrentMemoryBudget};
    let ledger_a = ConcurrentMemoryBudget::new(usize::MAX);
    let ledger_b = ConcurrentMemoryBudget::new(usize::MAX);
    for same_domain in [false, true] {
        let mut a = BinaryHeap::try_from_array_in([9u64], ConcurrentBudget(&ledger_a)).unwrap();
        let mut b = BinaryHeap::try_from_array_in(
            [1u64, 4, 3],
            ConcurrentBudget(if same_domain { &ledger_a } else { &ledger_b }),
        )
        .unwrap();
        a.try_append(&mut b).unwrap();
        valid(&a);
        assert!(b.is_empty());
        assert_eq!(b.capacity(), if same_domain { 1 } else { 3 });
        if !same_domain {
            assert_eq!(ledger_b.available(), usize::MAX - 24);
        }
        drop((a, b));
        assert_eq!(ledger_a.available(), usize::MAX);
        assert_eq!(ledger_b.available(), usize::MAX);
    }
}
#[test]
fn predicate_and_source_refusal_repair_successful_prefix() {
    let mut heap = BinaryHeap::try_from_array([9, 8, 7, 6, 5, 4, 3, 2, 1]).unwrap();
    let mut calls = 0;
    let result = heap.try_retain(|x| {
        calls += 1;
        if calls == 5 { Err(()) } else { Ok(x % 2 == 0) }
    });
    assert!(result.is_err());
    valid(&heap);
    assert_eq!(heap.len(), 7);
    let mut heap = BinaryHeap::new();
    let source = jzap::iter::from_results([Ok(2), Ok(7), Err("stop"), Ok(9)].into_iter());
    assert!(matches!(
        heap.try_extend(source),
        Err(ExtendError::Source("stop"))
    ));
    valid(&heap);
    assert_eq!(heap.pop(), Some(7));
    assert_eq!(heap.pop(), Some(2));
    let mut heap = BinaryHeap::from_vec(jzap::vec::Vec::with_limit(2));
    let error = heap.try_extend([3, 8, 9]).unwrap_err();
    match error {
        ExtendError::Push(e) => assert_eq!(e.value, 9),
        _ => panic!(),
    };
    valid(&heap);
    assert_eq!(heap.len(), 2);
}
#[test]
fn peek_mut_pop_read_mutation_forget_and_drain() {
    let mut heap = BinaryHeap::try_from_array([9, 8, 7, 6]).unwrap();
    let before = heap.as_slice().to_vec();
    assert_eq!(**heap.peek_mut().as_ref().unwrap(), 9);
    assert_eq!(heap.as_slice(), before);
    {
        let mut p = heap.peek_mut().unwrap();
        *p = 1;
        assert_eq!(*p, 1);
    }
    valid(&heap);
    assert_eq!(heap.peek(), Some(&8));
    let mut p = heap.peek_mut().unwrap();
    *p = 0;
    assert_eq!(PeekMut::pop(p), 0);
    valid(&heap);
    assert_eq!(heap.len(), 3);
    let mut p = heap.peek_mut().unwrap();
    *p = 5;
    std::mem::forget(p);
    assert_eq!(heap.len(), 1);
    assert_eq!(heap.pop(), Some(5));
    let mut heap = BinaryHeap::try_from_array([1, 2, 3, 4]).unwrap();
    let cap = heap.capacity();
    {
        let mut drain = heap.drain();
        assert_eq!(drain.try_len().unwrap(), 4);
        assert!(drain.try_next().unwrap().is_some());
        assert!(drain.next_back().is_some());
        assert_eq!(drain.len(), 2);
    }
    assert!(heap.is_empty());
    assert_eq!(heap.capacity(), cap);
    heap.try_push(5).unwrap();
    std::mem::forget(heap.drain());
    assert!(heap.is_empty());
    heap.try_push(6).unwrap();
    assert_eq!(heap.pop(), Some(6));
}
#[derive(Debug)]
struct Object {
    key: u8,
    drops: Rc<Cell<usize>>,
    clone_fail: bool,
}
impl PartialEq for Object {
    fn eq(&self, o: &Self) -> bool {
        self.key == o.key
    }
}
impl Eq for Object {}
impl PartialOrd for Object {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Object {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.key.cmp(&o.key)
    }
}
impl Drop for Object {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1)
    }
}
impl TryClone for Object {
    fn try_clone(&self) -> Result<Self, Error> {
        if self.clone_fail {
            Err(Error::Capacity)
        } else {
            Ok(Self {
                key: self.key,
                drops: self.drops.clone(),
                clone_fail: false,
            })
        }
    }
}
#[test]
fn exactly_once_drop_partial_clone_and_transactional_clone_from() {
    let drops = Rc::new(Cell::new(0));
    let object = |key, clone_fail| Object {
        key,
        drops: drops.clone(),
        clone_fail,
    };
    let heap =
        BinaryHeap::try_from_array([object(1, false), object(3, false), object(2, true)]).unwrap();
    assert!(heap.try_clone().is_err());
    assert_eq!(drops.get(), 2);
    let mut destination = BinaryHeap::try_from_array([object(10, false)]).unwrap();
    assert!(destination.try_clone_from(&heap).is_err());
    assert_eq!(destination.peek().unwrap().key, 10);
    let prior = drops.get();
    drop(heap);
    assert_eq!(drops.get(), prior + 3);
    {
        let mut drain = destination.drain();
        drop(drain.next_back());
    }
    assert!(destination.is_empty());
    assert_eq!(drops.get(), prior + 4);
}
#[test]
fn iterator_mirrors_clone_and_target_budget() {
    use jzap::{TryDoubleEndedIterator, TryExactSizeIterator};
    let heap = BinaryHeap::try_from_array([7, 3, 6, 1]).unwrap();
    let budget = MemoryBudget::new(4096);
    let cloned = heap.try_clone_in(Budget::Limited(&budget)).unwrap();
    valid(&cloned);
    assert_eq!(budget.occupied(), 16);
    drop(cloned);
    assert_eq!(budget.occupied(), 0);
    let mut iter = heap.iter();
    assert_eq!(iter.try_len().unwrap(), 4);
    assert!(iter.try_next_back().unwrap().is_some());
    let mut iter = heap.into_iter();
    assert!(iter.try_next().unwrap().is_some());
    let cloned = iter.try_clone().unwrap();
    assert_eq!(cloned.as_slice(), iter.as_slice());
    assert_eq!(iter.try_len().unwrap(), 3);
}
#[test]
fn zst_capacity_and_repeated_boundary_operations() {
    let mut heap = BinaryHeap::new();
    assert_eq!(heap.capacity(), usize::MAX);
    for _ in 0..50 {
        heap.try_push(()).unwrap();
    }
    valid(&heap);
    *heap.peek_mut().unwrap() = ();
    assert_eq!(PeekMut::pop(heap.peek_mut().unwrap()), ());
    let mut other = BinaryHeap::try_from_array([(); 5]).unwrap();
    heap.try_append(&mut other).unwrap();
    assert_eq!(heap.len(), 54);
    assert_eq!(heap.drain().count(), 54);
    assert!(heap.is_empty());
}
#[test]
fn owned_and_concurrent_budgets_support_heap_storage() {
    use jzap::budget::{
        ConcurrentBudget, ConcurrentMemoryBudget, OwnedBudget, OwnedConcurrentBudget,
    };
    let p = OwnedBudget::try_new(4096).unwrap();
    let mut a = BinaryHeap::new_in(p);
    a.try_push(5u64).unwrap();
    drop(a);
    let p = OwnedConcurrentBudget::try_new(4096).unwrap();
    let mut a = BinaryHeap::new_in(p);
    a.try_push(6u64).unwrap();
    std::thread::spawn(move || assert_eq!(a.pop(), Some(6)))
        .join()
        .unwrap();
    let b = ConcurrentMemoryBudget::new(4096);
    let mut a = BinaryHeap::new_in(ConcurrentBudget(&b));
    a.try_push(7u64).unwrap();
    drop(a);
    assert_eq!(b.used(), 0);

    let p = OwnedBudget::try_new(144).unwrap();
    let mut a = BinaryHeap::try_from_array_in([9u64], p.try_clone().unwrap()).unwrap();
    let mut b =
        BinaryHeap::try_from_array_in([1u64, 2, 3, 4, 5, 6, 7, 8], p.try_clone().unwrap()).unwrap();
    a.try_append(&mut b).unwrap();
    assert_eq!(p.occupied(), 80);
    assert!(std::ptr::eq(a.allocation_policy().ledger(), p.ledger()));
    valid(&a);
    drop((a, b));
    assert_eq!(p.occupied(), 0);

    let p = OwnedConcurrentBudget::try_new(144).unwrap();
    let mut a = BinaryHeap::try_from_array_in([9u64], p.try_clone().unwrap()).unwrap();
    let mut b =
        BinaryHeap::try_from_array_in([1u64, 2, 3, 4, 5, 6, 7, 8], p.try_clone().unwrap()).unwrap();
    a.try_append(&mut b).unwrap();
    assert_eq!(p.available(), 64);
    assert!(std::ptr::eq(a.allocation_policy().ledger(), p.ledger()));
    valid(&a);
    drop((a, b));
    assert_eq!(p.available(), 144);
}
#[test]
fn comparator_unwind_restores_initialized_holes() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    #[derive(Debug)]
    struct Bomb {
        key: u8,
        drops: Rc<Cell<usize>>,
        trigger: Rc<Cell<bool>>,
    }
    impl PartialEq for Bomb {
        fn eq(&self, o: &Self) -> bool {
            self.key == o.key
        }
    }
    impl Eq for Bomb {}
    impl PartialOrd for Bomb {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Bomb {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            assert!(!self.trigger.get());
            self.key.cmp(&o.key)
        }
    }
    impl Drop for Bomb {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1)
        }
    }
    for mode in 0..4 {
        let drops = Rc::new(Cell::new(0));
        let trigger = Rc::new(Cell::new(false));
        let make = |key| Bomb {
            key,
            drops: drops.clone(),
            trigger: trigger.clone(),
        };
        let mut heap = BinaryHeap::try_from_array([make(9), make(7), make(8), make(1)]).unwrap();
        trigger.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(move || match mode {
                0 => {
                    heap.try_push(make(10)).unwrap();
                }
                1 => {
                    drop(heap.pop());
                }
                2 => {
                    let mut p = heap.peek_mut().unwrap();
                    p.key = 0;
                }
                _ => {
                    heap.into_sorted_vec();
                    return;
                }
            }))
            .is_err()
        );
        // Each mode uses a separate ownership path below because sorting consumes.
        // Remaining objects are cleaned by the scope even if comparison unwinds.
        assert_eq!(drops.get(), if mode == 0 { 5 } else { 4 });
    }
}

#[test]
fn vector_transfer_drain_and_failure_release_storage_exactly_once() {
    let mut heap = BinaryHeap::try_from_array([4u64, 1, 3, 2]).unwrap();
    let address = heap.as_slice().as_ptr().addr();
    let (_, freed) = deallocations::deallocations_during(address, || {
        drop(heap.drain());
    });
    assert_eq!(freed, 0);
    let data = heap.into_vec();
    assert_eq!(data.as_ptr().addr(), address);
    let (_, freed) = deallocations::deallocations_during(address, || drop(data));
    assert_eq!(freed, 1);
}

#[test]
fn borrowed_copy_extension_and_consuming_mirrors() {
    use jzap::TryIntoIterator;
    let mut heap = BinaryHeap::new();
    heap.try_extend_copy(&[1, 7, 2][..]).unwrap();
    let mut borrowed = (&heap).try_into_iter().unwrap();
    assert_eq!(borrowed.try_next().unwrap(), heap.peek());
    let mut target = BinaryHeap::new();
    target.try_extend(heap).unwrap();
    assert_eq!(target.pop(), Some(7));
}

#[test]
fn aligned_large_noncopy_values_preserve_payload_and_drop_ownership() {
    #[repr(align(64))]
    #[derive(Debug)]
    struct Large {
        key: u64,
        bytes: [u8; 256],
        drops: Rc<Cell<usize>>,
    }
    impl PartialEq for Large {
        fn eq(&self, b: &Self) -> bool {
            self.key == b.key
        }
    }
    impl Eq for Large {}
    impl PartialOrd for Large {
        fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(b))
        }
    }
    impl Ord for Large {
        fn cmp(&self, b: &Self) -> std::cmp::Ordering {
            self.key.cmp(&b.key)
        }
    }
    impl Drop for Large {
        fn drop(&mut self) {
            assert_eq!(self.bytes, [self.key as u8; 256]);
            self.drops.set(self.drops.get() + 1)
        }
    }
    let drops = Rc::new(Cell::new(0));
    let make = |key| Large {
        key,
        bytes: [key as u8; 256],
        drops: drops.clone(),
    };
    let mut heap = BinaryHeap::new();
    for key in 0..40 {
        heap.try_push(make(key)).unwrap();
    }
    let mut other = BinaryHeap::try_from_array([make(45), make(46), make(47)]).unwrap();
    heap.try_append(&mut other).unwrap();
    assert_eq!(heap.as_slice().as_ptr().addr() % 64, 0);
    {
        let mut p = heap.peek_mut().unwrap();
        p.key = 12;
        p.bytes = [12; 256];
    }
    let mut previous = u64::MAX;
    for value in heap.into_sorted_vec().into_iter().rev() {
        assert!(value.key <= previous);
        previous = value.key;
        drop(value);
    }
    assert_eq!(drops.get(), 43);
    let mut heap = BinaryHeap::try_from_array([make(8), make(6), make(7)]).unwrap();
    {
        let mut drain = heap.drain();
        drop(drain.next());
        drop(drain.next_back());
    }
    assert_eq!(drops.get(), 46);
    assert!(heap.is_empty());
}

#[cfg(not(miri))]
proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(9610),
        ..proptest::test_runner::Config::default()
    })]
    #[test]
    fn generated_operations_match_std(
        actions in proptest::collection::vec((0u8..8,proptest::prelude::any::<u16>()),0..128)
    ) {
        let mut a=BinaryHeap::new();let mut b=StdHeap::new();
        for (op,value)in actions {
            match op {
                0..=2=>{a.try_push(value).unwrap();b.push(value);},
                3=>proptest::prop_assert_eq!(a.pop(),b.pop()),
                4=>{if let Some(mut p)=a.peek_mut(){*p=value}if let Some(mut p)=b.peek_mut(){*p=value}},
                5=>{a.try_retain(|x|Ok::<_,()>(x&3!=value&3)).unwrap();b.retain(|x|x&3!=value&3);},
                6=>{let mut x=BinaryHeap::try_from_array([value,value.rotate_left(3)]).unwrap();let mut y=StdHeap::from([value,value.rotate_left(3)]);a.try_append(&mut x).unwrap();b.append(&mut y);proptest::prop_assert!(x.is_empty());},
                _=>{a.try_shrink_to_fit().unwrap();b.shrink_to_fit();},
            }
            proptest::prop_assert_eq!(a.len(),b.len());proptest::prop_assert_eq!(a.peek(),b.peek());valid(&a);
        }
        let a=a.into_sorted_vec();let b=b.into_sorted_vec();
        proptest::prop_assert_eq!(a.as_slice(),b.as_slice());
    }
}
