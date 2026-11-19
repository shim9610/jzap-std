//! Model the actual product implementation, replacing only atomic primitives.
use jzap::{
    Error, TryClone,
    budget::{ConcurrentBudget, ConcurrentMemoryBudget},
    sync::Arc,
};
use loom::{
    cell::UnsafeCell,
    model,
    sync::{
        Arc as LArc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};
struct Payload {
    value: UnsafeCell<usize>,
    drops: LArc<AtomicUsize>,
}
impl Payload {
    fn read(&self) -> usize {
        self.value.with(|p| unsafe { *p })
    }
}
impl TryClone for Payload {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self {
            value: UnsafeCell::new(self.read()),
            drops: self.drops.clone(),
        })
    }
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.value.with_mut(|p| unsafe { *p = usize::MAX });
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}
#[test]
fn upgrade_against_final_release() {
    model(|| {
        let drops = LArc::new(AtomicUsize::new(0));
        let a = Arc::try_new(Payload {
            value: UnsafeCell::new(19),
            drops: drops.clone(),
        })
        .ok()
        .unwrap();
        let weak = Arc::try_downgrade(&a).unwrap();
        let t = thread::spawn(move || {
            if let Some(a) = weak.try_upgrade().unwrap() {
                assert_eq!(a.read(), 19);
            }
        });
        drop(a);
        t.join().unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    });
}
#[test]
fn copy_on_write_against_upgrade() {
    model(|| {
        let drops = LArc::new(AtomicUsize::new(0));
        let mut a = Arc::try_new(Payload {
            value: UnsafeCell::new(23),
            drops: drops.clone(),
        })
        .ok()
        .unwrap();
        let weak = Arc::try_downgrade(&a).unwrap();
        let t = thread::spawn(move || {
            if let Some(a) = weak.try_upgrade().unwrap() {
                assert_eq!(a.read(), 23);
            }
        });
        Arc::try_make_mut(&mut a)
            .unwrap()
            .value
            .with_mut(|p| unsafe { *p = 29 });
        t.join().unwrap();
        assert_eq!(a.read(), 29);
        drop(a);
        assert!((1..=2).contains(&drops.load(Ordering::Relaxed)));
    });
}
#[test]
fn uniqueness_against_downgrade_and_release() {
    model(|| {
        let mut a = Arc::try_new(31usize).unwrap();
        let b = a.try_clone().unwrap();
        let t = thread::spawn(move || {
            let w = Arc::try_downgrade(&b).unwrap();
            drop(b);
            if let Some(v) = w.try_upgrade().unwrap() {
                assert_eq!(*v, 31);
            }
        });
        if let Some(value) = Arc::get_mut(&mut a) {
            *value = 37;
        }
        t.join().unwrap();
        assert!(*a == 31 || *a == 37);
    });
}
#[test]
fn concurrent_into_inner_has_one_winner() {
    model(|| {
        let a = Arc::try_new(41usize).unwrap();
        let b = a.try_clone().unwrap();
        let t = thread::spawn(move || Arc::into_inner(b));
        let first = Arc::into_inner(a);
        let second = t.join().unwrap();
        assert_ne!(first.is_some(), second.is_some());
        assert_eq!(first.or(second), Some(41));
    });
}
#[test]
fn cyclic_initialization_publication() {
    model(|| {
        let mut spawned = None;
        let a = Arc::try_new_cyclic(|w| {
            let w = w.try_clone().unwrap();
            spawned = Some(thread::spawn(move || {
                if let Some(v) = w.try_upgrade().unwrap() {
                    assert_eq!(*v, 43usize);
                }
            }));
            Ok::<_, ()>(43usize)
        })
        .unwrap();
        spawned.take().unwrap().join().unwrap();
        assert_eq!(*a, 43);
    });
}
#[test]
fn cloned_strong_and_weak_release_once() {
    model(|| {
        let drops = LArc::new(AtomicUsize::new(0));
        let a = Arc::try_new(Payload {
            value: UnsafeCell::new(47),
            drops: drops.clone(),
        })
        .ok()
        .unwrap();
        let b = a.try_clone().unwrap();
        let w = Arc::try_downgrade(&a).unwrap();
        let q = w.try_clone().unwrap();
        let t = thread::spawn(move || {
            assert_eq!(b.read(), 47);
            drop(b);
            drop(w);
        });
        drop(a);
        drop(q);
        t.join().unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    });
}
#[test]
fn budget_reservation_commit_cancellation() {
    model(|| {
        let b = LArc::new(ConcurrentMemoryBudget::new(3));
        let c = b.clone();
        let t = thread::spawn(move || {
            if let Ok(r) = c.reserve(2) {
                assert!(c.used() <= 3);
                let charge = r.commit();
                thread::yield_now();
                drop(charge);
            }
        });
        if let Ok(r) = b.reserve(2) {
            assert!(b.used() <= 3);
            thread::yield_now();
            drop(r);
        }
        t.join().unwrap();
        assert_eq!(b.used(), 0);
    });
}
#[test]
fn concurrent_allocations_and_retained_weak_charges() {
    model(|| {
        let b = LArc::new(ConcurrentMemoryBudget::new(128));
        let c = b.clone();
        let t = thread::spawn(move || {
            if let Ok(a) = Arc::try_new_in(53u64, ConcurrentBudget(&c)) {
                let weak = Arc::try_downgrade(&a).unwrap();
                drop(a);
                thread::yield_now();
                drop(weak);
            }
        });
        if let Ok(a) = Arc::try_new_in(59u64, ConcurrentBudget(&b)) {
            let clone = a.try_clone().unwrap();
            drop(a);
            thread::yield_now();
            drop(clone);
        }
        t.join().unwrap();
        assert_eq!(b.used(), 0);
    });
}

struct ReleaseWitness(UnsafeCell<usize>);
// This fixture permits one writer, then access only after its owner is released.
// It exposes the required release/acquire relation to Loom's cell tracking.
unsafe impl Send for ReleaseWitness {}
unsafe impl Sync for ReleaseWitness {}
impl Drop for ReleaseWitness {
    fn drop(&mut self) {
        self.0.with(|p| unsafe { assert_eq!(*p, 67) });
    }
}
#[test]
fn last_release_acquires_previous_owner_writes() {
    model(|| {
        let a = Arc::try_new(ReleaseWitness(UnsafeCell::new(0)))
            .ok()
            .unwrap();
        let b = a.try_clone().unwrap();
        let t = thread::spawn(move || {
            b.0.with_mut(|p| unsafe { *p = 67 });
            drop(b);
        });
        drop(a);
        t.join().unwrap();
    });
}

#[test]
fn owning_policy_refusal_preserves_the_cow_source_and_domain() {
    use jzap::budget::OwnedConcurrentBudget;
    model(|| {
        let budget = OwnedConcurrentBudget::try_new(1024).unwrap();
        let mut value = Arc::try_new_in(7u64, budget.try_clone().unwrap()).unwrap();
        let alias = value.try_clone().unwrap();
        let bytes = budget.used();
        let mut credits = Vec::new();
        for _ in 0..5 {
            credits.push(budget.try_clone().unwrap());
        }
        assert_eq!(Arc::try_make_mut(&mut value).unwrap_err(), Error::RefCount);
        assert!(Arc::ptr_eq(&value, &alias));
        assert_eq!((*value, budget.used()), (7, bytes));
        drop(credits);
        *Arc::try_make_mut(&mut value).unwrap() = 11;
        assert_eq!((*value, *alias), (11, 7));
        drop((value, alias));
        assert_eq!(budget.used(), 0);
    });
}

#[test]
fn owning_budget_domain_survives_parallel_allocation_after_creator_drop() {
    use jzap::{budget::OwnedConcurrentBudget, vec::Vec as JVec};
    let mut builder = loom::model::Builder::new();
    builder.preemption_bound = Some(2);
    builder.check(|| {
        let budget = OwnedConcurrentBudget::try_new(8).unwrap();
        let a = budget.try_clone().unwrap();
        let b = budget.try_clone().unwrap();
        drop(budget);
        let worker =
            |policy: OwnedConcurrentBudget| match JVec::<u8, _>::try_with_capacity_in(8, policy) {
                Ok(mut data) => {
                    data.try_push(7).unwrap();
                    assert!(data.allocation_policy().used() <= 8);
                    Some(data)
                }
                Err(Error::Budget { .. }) => None,
                Err(error) => panic!("unexpected storage error: {error:?}"),
            };
        let one = thread::spawn(move || worker(a));
        let two = thread::spawn(move || worker(b));
        let one = one.join().unwrap();
        let two = two.join().unwrap();
        assert_ne!(one.is_some(), two.is_some());
        for data in one.iter().chain(two.iter()) {
            assert_eq!(data.as_slice(), [7]);
        }
        drop((one, two));
    });
}

#[test]
fn owning_domain_is_retained_during_upgrade_and_final_weak_release() {
    use jzap::budget::OwnedConcurrentBudget;
    let mut builder = loom::model::Builder::new();
    builder.preemption_bound = Some(2);
    builder.check(|| {
        let value = Arc::try_new_in(19u64, OwnedConcurrentBudget::try_new(1024).unwrap()).unwrap();
        let weak = Arc::try_downgrade(&value).unwrap();
        let t = thread::spawn(move || drop(value));
        if let Some(value) = weak.try_upgrade().unwrap() {
            assert_eq!(*value, 19);
        }
        drop(weak);
        t.join().unwrap();
    });
}
