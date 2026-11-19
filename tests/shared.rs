//! Shared protocol, ownership, failure and concurrency regression cases.
mod common;
use common::{FaultAllocator, track};
use jzap::{
    Error, TryClone,
    budget::{Budget, ConcurrentBudget, ConcurrentMemoryBudget, MemoryBudget, Unmetered},
    error::InitError,
    rc::Rc,
    shared::{
        Control, Strong, Weak,
        count::{AtomicCount, Counter, LocalCount},
    },
    sync::Arc,
};
use std::{
    cell::Cell,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};
#[global_allocator]
static ALLOC: FaultAllocator = FaultAllocator;

fn lifecycle<C: Counter>() {
    let value = Strong::<u64, C>::try_new(42).unwrap();
    assert_eq!(
        (Strong::strong_count(&value), Strong::weak_count(&value)),
        (1, 0)
    );
    let mut alias = value.try_clone().unwrap();
    let weak = Strong::try_downgrade(&value).unwrap();
    assert_eq!((Strong::strong_count(&value), weak.weak_count()), (2, 1));
    assert!(Strong::get_mut(&mut alias).is_none());
    assert!(Strong::ptr_eq(
        &value,
        &weak.try_upgrade().unwrap().unwrap()
    ));
    assert_eq!(*alias, 42);
    drop(value);
    assert!(Strong::get_mut(&mut alias).is_none());
    drop(weak);
    *Strong::get_mut(&mut alias).unwrap() = 73;
    assert_eq!(Strong::try_unwrap(alias).unwrap(), 73);
    let empty = Weak::<u64, C>::new();
    assert_eq!(empty.strong_count(), 0);
    assert!(empty.try_upgrade().unwrap().is_none());
    assert!(empty.as_ptr().is_none());
    assert!(empty.ptr_eq(&empty.try_clone().unwrap()));
}
#[test]
fn rc_lifecycle() {
    lifecycle::<LocalCount>();
}
#[test]
fn arc_lifecycle() {
    lifecycle::<AtomicCount>();
}

fn accounting<C: Counter>() {
    let budget = MemoryBudget::new(4096);
    let bytes = std::alloc::Layout::new::<Control<C, Budget<'_>, u64>>().size();
    let value = Strong::<u64, C, _>::try_new_in(42, Budget::Limited(&budget)).unwrap();
    assert_eq!(budget.occupied(), bytes);
    let alias = value.try_clone().unwrap();
    let weak = Strong::try_downgrade(&value).unwrap();
    assert_eq!(budget.occupied(), bytes);
    drop(alias);
    drop(value);
    assert!(weak.try_upgrade().unwrap().is_none());
    assert_eq!(budget.occupied(), bytes);
    assert_eq!(weak.weak_count(), 0);
    drop(weak);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
    let (result, state) = track(1, || {
        Strong::<u64, C, _>::try_new_in(99, Budget::Limited(&budget))
    });
    assert_eq!(result.unwrap_err().value, 99);
    assert_eq!(state.live, 0);
    assert_eq!(budget.occupied(), 0);
    let too_small = MemoryBudget::new(bytes - 1);
    let (r, state) = track(usize::MAX, || {
        Strong::<u64, C, _>::try_new_in(17, Budget::Limited(&too_small))
    });
    assert!(matches!(r.unwrap_err().error, Error::Budget { .. }));
    assert_eq!(state.calls, 0);
}
#[test]
fn rc_accounting_and_refusal() {
    accounting::<LocalCount>();
}
#[test]
fn arc_accounting_and_refusal() {
    accounting::<AtomicCount>();
}

#[derive(Debug)]
struct DropCount<'a>(&'a Cell<usize>);
impl Drop for DropCount<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn initialization<C: Counter>() {
    let budget = MemoryBudget::new(4096);
    let drops = Cell::new(0);
    for failure in 0..5 {
        let result = Strong::<[DropCount<'_>], C, _>::try_init_slice_in(
            5,
            Budget::Limited(&budget),
            |i, slot| {
                slot.write(DropCount(&drops)).unwrap();
                if i == failure { Err(9) } else { Ok(()) }
            },
        );
        assert!(matches!(result, Err(InitError::Initialize(9))));
        assert_eq!(budget.occupied(), 0);
    }
    assert_eq!(drops.get(), 15);
    let r = Strong::<u64, C, _>::try_init_in(
        Budget::Limited(&budget),
        |_: &mut jzap::init::InitSlot<'_, u64>| Ok::<_, ()>(()),
    );
    assert!(matches!(r, Err(InitError::Storage(Error::Uninitialized))));
    assert_eq!(budget.occupied(), 0);
    let (r, state) = track(1, || {
        Strong::<u64, C, _>::try_init_in(
            Budget::Limited(&budget),
            |s: &mut jzap::init::InitSlot<'_, u64>| s.write(1).map_err(|e| e.error),
        )
    });
    assert!(r.is_err());
    assert_eq!(state.live, 0);
}
#[test]
fn rc_partial_initialization() {
    initialization::<LocalCount>();
}
#[test]
fn arc_partial_initialization() {
    initialization::<AtomicCount>();
}

fn cow<C: Counter>() {
    let budget = MemoryBudget::new(4096);
    let mut a = Strong::<u64, C, _>::try_new_in(11, Budget::Limited(&budget)).unwrap();
    let original = Strong::as_ptr(&a);
    *Strong::try_make_mut(&mut a).unwrap() = 12;
    assert_eq!(Strong::as_ptr(&a), original);
    let alias = a.try_clone().unwrap();
    let (r, state) = track(1, || Strong::try_make_mut(&mut a).map(|_| ()));
    assert!(r.is_err());
    assert_eq!(state.live, 0);
    assert_eq!(Strong::as_ptr(&a), original);
    assert_eq!(*alias, 12);
    *Strong::try_make_mut(&mut a).unwrap() = 13;
    assert_ne!(Strong::as_ptr(&a), original);
    assert_eq!(*alias, 12);
    drop(alias);
    let weak = Strong::try_downgrade(&a).unwrap();
    let old = Strong::as_ptr(&a);
    let (r, _) = track(1, || Strong::try_make_mut(&mut a).map(|_| ()));
    assert!(r.is_err());
    assert_eq!(Strong::as_ptr(&a), old);
    assert!(weak.try_upgrade().unwrap().is_some());
    *Strong::try_make_mut(&mut a).unwrap() = 14;
    assert_ne!(Strong::as_ptr(&a), old);
    assert!(weak.try_upgrade().unwrap().is_none());
    assert_eq!(*a, 14);
    drop(a);
    assert!(budget.occupied() > 0);
    drop(weak);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn rc_cow_and_atomic_failure() {
    cow::<LocalCount>();
}
#[test]
fn arc_cow_and_atomic_failure() {
    cow::<AtomicCount>();
}

fn dynamic<C: Counter>() {
    let mut a = Strong::<[u64], C>::try_copy_from_slice(&[1, 2, 3]).unwrap();
    let alias = a.try_clone().unwrap();
    Strong::try_make_mut(&mut a).unwrap()[1] = 7;
    assert_eq!(alias.as_slice(), [1, 2, 3]);
    assert_eq!(a.as_slice(), [1, 7, 3]);
    let weak = Strong::try_downgrade(&a).unwrap();
    drop(a);
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    let mut s = Strong::<str, C>::try_from_str("aé한😀").unwrap();
    let old = s.try_clone().unwrap();
    Strong::try_make_mut(&mut s).unwrap().make_ascii_uppercase();
    assert_eq!(old.as_str(), "aé한😀");
    assert_eq!(s.as_str(), "Aé한😀");
    let raw = Strong::into_raw(s);
    let s = unsafe { Strong::<str, C>::from_raw(raw) };
    assert_eq!(s.as_str(), "Aé한😀");
    let weak = Strong::try_downgrade(&s).unwrap();
    let raw = weak.into_raw();
    drop(s);
    let weak = unsafe { Weak::<str, C>::from_raw(raw) };
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    let empty = Strong::<[u64], C>::try_copy_from_slice(&[]).unwrap();
    assert!(empty.is_empty());
    let zst = Strong::<[()], C>::try_copy_from_slice(&[(); 128]).unwrap();
    assert_eq!(zst.len(), 128);
}
#[test]
fn rc_dynamic_storage() {
    dynamic::<LocalCount>();
}
#[test]
fn arc_dynamic_storage() {
    dynamic::<AtomicCount>();
}

fn zeroed<C: Counter>() {
    let mut value = Strong::<u64, C>::try_new_uninit().unwrap();
    Strong::get_mut(&mut value).unwrap().write(91);
    let value = unsafe { Strong::<MaybeUninit<u64>, C>::assume_init(value) };
    assert_eq!(*value, 91);
    let value = Strong::<u64, C>::try_new_zeroed().unwrap();
    let value = unsafe { Strong::<MaybeUninit<u64>, C>::assume_init(value) };
    assert_eq!(*value, 0);
    let value = Strong::<[u32], C>::try_new_zeroed_slice(31).unwrap();
    let value = unsafe { Strong::<[MaybeUninit<u32>], C>::assume_init(value) };
    assert_eq!(value.as_slice(), [0; 31]);
    assert!(Strong::<[u64], C>::try_new_uninit_slice(usize::MAX).is_err());
}
#[test]
fn rc_uninitialized_and_zeroed() {
    zeroed::<LocalCount>();
}
#[test]
fn arc_uninitialized_and_zeroed() {
    zeroed::<AtomicCount>();
}

fn cyclic<C: Counter>() {
    struct Node<C: Counter> {
        me: Weak<Node<C>, C>,
    }
    let a = Strong::<Node<C>, C>::try_new_cyclic(|weak| {
        assert!(weak.try_upgrade().unwrap().is_none());
        Ok::<_, Error>(Node {
            me: weak.try_clone()?,
        })
    })
    .unwrap();
    assert!(Strong::ptr_eq(&a, &a.me.try_upgrade().unwrap().unwrap()));
    let weak = Strong::try_downgrade(&a).unwrap();
    drop(a);
    assert!(weak.try_upgrade().unwrap().is_none());
    let budget = MemoryBudget::new(4096);
    let mut escaped = None;
    let result = Strong::<u64, C, _>::try_new_cyclic_in(Budget::Limited(&budget), |w| {
        escaped = Some(w.try_clone().unwrap());
        Err::<u64, _>(5)
    });
    assert!(matches!(result, Err(InitError::Initialize(5))));
    assert!(budget.occupied() > 0);
    assert!(escaped.as_ref().unwrap().try_upgrade().unwrap().is_none());
    drop(escaped);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn rc_cyclic_construction() {
    cyclic::<LocalCount>();
}
#[test]
fn arc_cyclic_construction() {
    cyclic::<AtomicCount>();
}

#[test]
fn compiler_checked_unsizing_and_downcasts() {
    let a = Rc::try_new([1u64, 2, 3]).unwrap();
    let weak = Rc::try_downgrade(&a).unwrap();
    let a = jzap::rc_unsize!(a=>[u64]);
    assert_eq!(a.as_slice(), [1, 2, 3]);
    drop(a);
    let weak = jzap::rc_weak_unsize!(weak=>[u64]);
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    let a = Arc::try_new(73u64).unwrap();
    let weak = Arc::try_downgrade(&a).unwrap();
    let a = jzap::arc_unsize!(a=>dyn std::any::Any+Send+Sync);
    let a = Arc::<dyn std::any::Any + Send + Sync>::downcast::<u32>(a).unwrap_err();
    let a = Arc::<dyn std::any::Any + Send + Sync>::downcast::<u64>(a).unwrap();
    assert_eq!(*a, 73);
    drop(a);
    let weak = jzap::arc_weak_unsize!(weak=>dyn std::any::Any+Send+Sync);
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
}
#[test]
fn box_vec_and_iterator_transfers() {
    let b = jzap::boxed::Box::try_new(41u64).unwrap();
    let p = jzap::boxed::Box::as_ptr(&b);
    let (r, _) = track(1, || Rc::try_from_box_in(b));
    let b = r.unwrap_err().value;
    assert_eq!(jzap::boxed::Box::as_ptr(&b), p);
    let a = Rc::try_from_box_in(b).unwrap();
    assert_eq!(*a, 41);
    let values = jzap::vec::Vec::try_from_array([11u64, 13, 17]).unwrap();
    let (r, _) = track(1, || Arc::try_from_vec_in(values));
    let values = r.unwrap_err().value;
    assert_eq!(values.as_slice(), [11, 13, 17]);
    let a = Arc::try_from_vec_in(values).unwrap();
    assert_eq!(a.as_slice(), [11, 13, 17]);
    let a = Rc::<[u64]>::try_from_iter([2, 3, 5]).unwrap();
    assert_eq!(a.as_slice(), [2, 3, 5]);
    let r = Arc::<[u64]>::try_from_fallible_iter_in(
        jzap::iter::from_results([Ok(1), Err(19), Ok(3)]),
        Unmetered,
    );
    assert!(matches!(r, Err(InitError::Initialize(19))));
}
#[test]
fn shared_clone_keeps_original_budget() {
    let original = MemoryBudget::new(4096);
    let target = MemoryBudget::new(0);
    let a = Arc::try_new_in(1u64, Budget::Limited(&original)).unwrap();
    let charge = original.occupied();
    let b = jzap::TryCloneIn::try_clone_in(&a, Budget::Limited(&target)).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(original.occupied(), charge);
    assert_eq!(target.occupied(), 0);
    assert!(Arc::<u64, _>::try_deep_clone_in(&a, Budget::Limited(&target)).is_err());
}
#[test]
fn concurrent_budget_tokens_and_container_integration() {
    let budget = ConcurrentMemoryBudget::new(4096);
    let policy = ConcurrentBudget(&budget);
    {
        let token = budget.reserve(100).unwrap();
        assert_eq!(token.bytes(), 100);
        assert_eq!(budget.used(), 100);
        let charge = token.commit();
        assert_eq!(charge.bytes(), 100);
        assert_eq!(budget.used(), 100);
        drop(charge);
    }
    assert_eq!(budget.used(), 0);
    let a = Arc::try_new_in(42u64, policy).unwrap();
    let weak = Arc::try_downgrade(&a).unwrap();
    let b = jzap::boxed::Box::try_new_in(17u64, policy).unwrap();
    let v = jzap::vec::Vec::try_from_slice_in(&[1u64, 2, 3], policy, jzap::vec::Unlimited).unwrap();
    let s = jzap::string::String::try_from_str_in("한😀", policy, jzap::vec::Unlimited).unwrap();
    assert!(budget.used() > 0);
    drop((a, b, v, s));
    assert!(budget.used() > 0);
    drop(weak);
    assert_eq!(budget.used(), 0);
    let (r, state) = track(1, || Arc::try_new_in(9u64, policy));
    assert!(r.is_err());
    assert_eq!(state.live, 0);
    assert_eq!(budget.used(), 0);
    let small = ConcurrentMemoryBudget::new(usize::MAX);
    let t = small.reserve(usize::MAX).unwrap();
    assert!(small.reserve(1).is_err());
    drop(t);
    assert_eq!(small.used(), 0);
}
#[test]
fn concurrent_budget_races_never_overadmit() {
    let budget = ConcurrentMemoryBudget::new(3);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..1000 {
                    if let Ok(r) = budget.reserve(2) {
                        assert!(budget.used() <= 3);
                        drop(r.commit());
                    }
                }
            });
        }
    });
    assert_eq!(budget.used(), 0);
}
#[test]
fn arc_upgrade_last_release_and_into_inner_races() {
    #[derive(Debug)]
    struct D<'a>(&'a AtomicUsize);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    for _ in 0..16 {
        let drops = AtomicUsize::new(0);
        let a = Arc::try_new(D(&drops)).unwrap();
        let w = Arc::try_downgrade(&a).unwrap();
        std::thread::scope(|s| {
            s.spawn(move || drop(a));
            s.spawn(move || {
                for _ in 0..8 {
                    if let Some(x) = w.try_upgrade().unwrap() {
                        assert_eq!(x.0.load(Ordering::Relaxed), 0);
                        drop(x);
                    }
                }
            });
        });
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
    let a = Arc::try_new(97u64).unwrap();
    let b = a.try_clone().unwrap();
    let (x, y) = std::thread::scope(|s| {
        let x = s.spawn(move || Arc::into_inner(a));
        let y = s.spawn(move || Arc::into_inner(b));
        (x.join().unwrap(), y.join().unwrap())
    });
    assert_eq!(x.or(y), Some(97));
    assert_ne!(x.is_some(), y.is_some());
}
#[test]
fn pinning_and_handle_sizes() {
    #[derive(Debug)]
    struct Immovable {
        value: u64,
        _pin: std::marker::PhantomPinned,
    }
    let p = Rc::try_pin(Immovable {
        value: 19,
        _pin: std::marker::PhantomPinned,
    })
    .unwrap();
    let q = p.try_clone().unwrap();
    assert_eq!(p.as_ref().value, 19);
    assert!(std::ptr::eq(p.as_ref().get_ref(), q.as_ref().get_ref()));
    assert_eq!(
        std::mem::size_of::<Rc<u64>>(),
        std::mem::size_of::<std::rc::Rc<u64>>()
    );
    assert_eq!(
        std::mem::size_of::<Arc<str>>(),
        std::mem::size_of::<std::sync::Arc<str>>()
    );
    assert_eq!(
        std::mem::size_of::<Arc<u64, ConcurrentBudget<'_>>>(),
        std::mem::size_of::<Arc<u64>>()
    );
    assert_eq!(
        std::mem::size_of::<jzap::boxed::Box<u64, Budget<'_>>>(),
        2 * std::mem::size_of::<usize>()
    );
}
#[cfg(not(miri))]
proptest::proptest! {
 #[test]fn local_and_atomic_differential_sequences(ops in proptest::collection::vec(0u8..8,0..80)){
    fn run<C:Counter>(ops:&[u8]){
        let mut actual=Some(Strong::<u64,C>::try_new(17).unwrap());let mut model=Some(std::rc::Rc::new(17));
        let mut aliases=Vec::new();let mut refs=Vec::new();let mut weaks=Vec::new();let mut weak_refs=Vec::new();
        for &op in ops{
            match op{
                0 if aliases.len()<3=>if let(Some(a),Some(m))=(&actual,&model){aliases.push(a.try_clone().unwrap());refs.push(m.clone());},
                1=>{aliases.pop();refs.pop();},
                2 if weaks.len()<3=>if let(Some(a),Some(m))=(&actual,&model){weaks.push(Strong::try_downgrade(a).unwrap());weak_refs.push(std::rc::Rc::downgrade(m));},
                3=>{weaks.pop();weak_refs.pop();},
                4=>if let(Some(a),Some(m))=(&mut actual,&mut model){*Strong::try_make_mut(a).unwrap()+=1;*std::rc::Rc::make_mut(m)+=1;},
                5=>for(w,r)in weaks.iter().zip(&weak_refs){assert_eq!(w.try_upgrade().unwrap().map(|a|*a),r.upgrade().map(|a|*a));},
                6=>if let(Some(a),Some(m))=(&actual,&model){assert_eq!(Strong::strong_count(a),std::rc::Rc::strong_count(m));assert_eq!(Strong::weak_count(a),std::rc::Rc::weak_count(m));},
                7=>{actual.take();model.take();},_=>{}
            }
        }
    }
    run::<LocalCount>(&ops);run::<AtomicCount>(&ops);
 }
}

#[test]
fn dead_trait_object_payload_and_overaligned_headers() {
    let a = Arc::try_new(Box::new(73u64)).unwrap();
    let a = jzap::arc_unsize!(a=>dyn std::any::Any+Send+Sync);
    let weak = Arc::try_downgrade(&a).unwrap();
    drop(a);
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    #[repr(align(256))]
    #[derive(Debug)]
    struct Aligned(u8);
    let budget = MemoryBudget::new(4096);
    let a = Rc::try_new_in(Aligned(5), Budget::Limited(&budget)).unwrap();
    assert_eq!(Rc::as_ptr(&a).addr() % 256, 0);
    assert_eq!(a.0, 5);
    let w = Rc::try_downgrade(&a).unwrap();
    drop(a);
    drop(w);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn last_strong_unwind_releases_implicit_weak_once() {
    #[derive(Debug)]
    struct Panics;
    impl Drop for Panics {
        fn drop(&mut self) {
            panic!("user destructor control");
        }
    }
    let budget = MemoryBudget::new(4096);
    let value = Rc::try_new_in(Panics, Budget::Limited(&budget)).unwrap();
    let ptr = Rc::into_raw(value);
    let value = unsafe { Rc::<Panics, _>::from_raw(ptr) };
    let (result, count) = common::deallocations_during(ptr.as_ptr().addr(), || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value)))
    });
    assert!(result.is_err());
    assert_eq!(count, 1);
    assert_eq!(budget.occupied(), 0);
    let value = Arc::try_new_in(Panics, Budget::Limited(&budget)).unwrap();
    let weak = Arc::try_downgrade(&value).unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value))).is_err());
    assert!(budget.occupied() > 0);
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn copy_on_write_clone_refusal_drops_partial_slice_and_preserves_source() {
    #[derive(Debug)]
    struct Fails<'a> {
        calls: &'a Cell<usize>,
        drops: &'a Cell<usize>,
        failure: usize,
    }
    impl Drop for Fails<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    impl TryClone for Fails<'_> {
        fn try_clone(&self) -> Result<Self, Error> {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n == self.failure {
                Err(Error::Capacity)
            } else {
                Ok(Self {
                    calls: self.calls,
                    drops: self.drops,
                    failure: self.failure,
                })
            }
        }
    }
    for failure in 1..=4 {
        let budget = MemoryBudget::new(4096);
        let calls = Cell::new(0);
        let drops = Cell::new(0);
        let mut a =
            Arc::<[Fails<'_>], _>::try_init_slice_in(4, Budget::Limited(&budget), |_, s| {
                s.write(Fails {
                    calls: &calls,
                    drops: &drops,
                    failure,
                })
                .map_err(|e| e.error)
            })
            .unwrap();
        let b = a.try_clone().unwrap();
        let ptr = Arc::as_ptr(&a);
        let charge = budget.occupied();
        assert!(Arc::try_make_mut(&mut a).is_err());
        assert_eq!(Arc::as_ptr(&a), ptr);
        assert_eq!(budget.occupied(), charge);
        assert_eq!(drops.get(), failure - 1);
        assert_eq!(a.len(), 4);
        drop((a, b));
        assert_eq!(drops.get(), failure + 3);
        assert_eq!(budget.occupied(), 0);
    }
}
#[cfg(jzap_small_counts)]
#[test]
fn reduced_count_limits_reject_without_mutation() {
    fn check<C: Counter>(weak_maximum: usize) {
        let a = Strong::<u64, C>::try_new(17).unwrap();
        let mut strong = Vec::new();
        for _ in 1..jzap::shared::count::MAX_COUNT {
            strong.push(a.try_clone().unwrap());
        }
        assert_eq!(Strong::strong_count(&a), jzap::shared::count::MAX_COUNT);
        assert!(matches!(a.try_clone(), Err(Error::RefCount)));
        let w = Strong::try_downgrade(&a).unwrap();
        assert!(matches!(w.try_upgrade(), Err(Error::RefCount)));
        let raw = Strong::into_raw(strong.pop().unwrap());
        assert!(unsafe { Strong::<u64, C>::try_increment_strong_count(raw) }.is_err());
        drop(unsafe { Strong::<u64, C>::from_raw(raw) });
        assert!(w.try_upgrade().unwrap().is_some());
        let mut weaks = Vec::new();
        for _ in 2..weak_maximum {
            weaks.push(w.try_clone().unwrap());
        }
        assert!(matches!(w.try_clone(), Err(Error::RefCount)));
        assert!(matches!(Strong::try_downgrade(&a), Err(Error::RefCount)));
        drop(strong);
        drop(a);
        assert!(w.try_upgrade().unwrap().is_none());
        drop(weaks);
        drop(w);
    }
    check::<LocalCount>(jzap::shared::count::MAX_COUNT);
    check::<AtomicCount>(jzap::shared::count::WEAK_MAX_COUNT);
}

#[test]
fn pinned_unsizing_and_empty_weak_unsizing() {
    let p = Arc::try_pin(19u64).unwrap();
    let p = jzap::pin_arc_unsize!(p=>dyn std::any::Any+Send+Sync);
    assert_eq!(p.as_ref().get_ref().downcast_ref::<u64>(), Some(&19));
    let w = jzap::rc::Weak::<[u8; 0]>::new();
    let w = jzap::rc_weak_unsize!(w=>[u8]);
    assert!(w.try_upgrade().unwrap().is_none());
    assert!(w.as_ptr().is_none());
    #[derive(Debug)]
    struct Immovable(std::marker::PhantomPinned);
    fn unpin<T: Unpin>() {}
    unpin::<Arc<Immovable>>();
    unpin::<Rc<Immovable>>();
}
#[test]
fn nested_allocation_refusals_preserve_copy_on_write_source() {
    for failure in 1..=2 {
        let budget = MemoryBudget::new(4096);
        let inner = jzap::vec::Vec::try_from_slice_in(
            &[11u64, 13, 17],
            Budget::Limited(&budget),
            jzap::vec::Unlimited,
        )
        .unwrap();
        let mut a = Arc::try_new_in(inner, Budget::Limited(&budget)).unwrap();
        let b = a.try_clone().unwrap();
        let ptr = Arc::as_ptr(&a);
        let occupied = budget.occupied();
        let (r, state) = track(failure, || Arc::try_make_mut(&mut a).map(|_| ()));
        assert!(r.is_err());
        assert_eq!(state.live, 0);
        assert_eq!(budget.occupied(), occupied);
        assert_eq!(Arc::as_ptr(&a), ptr);
        assert_eq!(a.as_slice(), [11, 13, 17]);
        drop((a, b));
        assert_eq!(budget.occupied(), 0);
    }
}

#[test]
fn cyclic_slots_require_recorded_initialization_and_cleanup_before_refund() {
    let budget = MemoryBudget::new(4096);
    let mut escaped = None;
    let r = Arc::<u64, _>::try_init_cyclic_in(Budget::Limited(&budget), |w, _| {
        escaped = Some(w.try_clone().unwrap());
        Ok::<_, ()>(())
    });
    assert!(matches!(r, Err(InitError::Storage(Error::Uninitialized))));
    assert!(escaped.as_ref().unwrap().try_upgrade().unwrap().is_none());
    drop(escaped);
    assert_eq!(budget.occupied(), 0);
    let drops = Cell::new(0);
    let r = Rc::<DropCount<'_>, _>::try_init_cyclic_in(Budget::Limited(&budget), |w, s| {
        assert!(w.try_upgrade().unwrap().is_none());
        s.write(DropCount(&drops)).unwrap();
        Err(7)
    });
    assert!(matches!(r, Err(InitError::Initialize(7))));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.occupied(), 0);
    let a = Arc::<u64, _>::try_init_cyclic_in(Budget::Limited(&budget), |w, s| {
        assert!(w.try_upgrade().unwrap().is_none());
        s.write(79).map_err(|e| e.error)
    })
    .unwrap();
    assert_eq!(*a, 79);
    drop(a);
    assert_eq!(budget.occupied(), 0);
}

#[test]
fn pin_conversion_rejects_movable_aliases_and_weak_upgrades() {
    #[derive(Debug)]
    struct AddressStable {
        address: Cell<*const Self>,
        _pin: std::marker::PhantomPinned,
    }
    impl AddressStable {
        fn initialize(self: std::pin::Pin<&Self>) {
            self.address.set(self.get_ref());
        }
    }
    impl Drop for AddressStable {
        fn drop(&mut self) {
            let p = self.address.get();
            if !p.is_null() {
                assert!(std::ptr::eq(self, p));
                unsafe {
                    assert_eq!((*p).address.get(), p);
                }
            }
        }
    }
    let a = Rc::try_new(AddressStable {
        address: Cell::new(std::ptr::null()),
        _pin: std::marker::PhantomPinned,
    })
    .unwrap();
    let b = a.try_clone().unwrap();
    let a = match Rc::try_into_pin(a) {
        Err(a) => a,
        Ok(_) => panic!("accepted ordinary alias"),
    };
    drop(b);
    let w = Rc::try_downgrade(&a).unwrap();
    let a = match Rc::try_into_pin(a) {
        Err(a) => a,
        Ok(_) => panic!("accepted weak upgrade"),
    };
    drop(w);
    let p = Rc::try_into_pin(a).ok().unwrap();
    p.as_ref().initialize();
    let q = p.try_clone().unwrap();
    drop(p);
    drop(q);
}

#[test]
fn standard_conversion_and_default_adapters() {
    let array = Rc::<[u64]>::try_from_array([1, 2, 3]).unwrap();
    let alias = array.try_clone().unwrap();
    let array = Rc::<[u64]>::try_into_array::<2>(array).unwrap_err();
    let array = Rc::<[u64]>::try_into_array::<3>(array).unwrap();
    assert_eq!(array.as_ref(), &[1, 2, 3]);
    assert_eq!(alias.as_slice(), [1, 2, 3]);
    let string = Arc::<str>::try_from_std_string("aé😀".to_owned()).unwrap();
    let alias = string.try_clone().unwrap();
    let mut bytes = Arc::<str>::into_bytes(string);
    assert_eq!(bytes.as_slice(), "aé😀".as_bytes());
    assert!(Arc::get_mut(&mut bytes).is_none());
    Arc::try_make_mut(&mut bytes).unwrap()[0] = b'b';
    assert_eq!(alias.as_str(), "aé😀");
    let cow = std::borrow::Cow::Borrowed("borrowed");
    let a = Rc::<str>::try_from_cow(cow).unwrap();
    assert_eq!(a.as_str(), "borrowed");
    let owned: std::borrow::Cow<'_, str> = std::borrow::Cow::Owned("owned".to_owned());
    let budget = MemoryBudget::new(0);
    let r = Rc::<str, _>::try_from_cow_in(owned, Budget::Limited(&budget));
    assert_eq!(r.unwrap_err().value, "owned");
    let a = Arc::<[u64]>::try_from_std_vec(vec![1, 3, 7]).unwrap();
    assert_eq!(a.as_slice(), [1, 3, 7]);
    let empty: Rc<[std::string::String]> = jzap::TryDefault::try_default().unwrap();
    assert!(empty.is_empty());
    let empty: Arc<str> = jzap::TryDefault::try_default().unwrap();
    assert_eq!(empty.as_str(), "");
    let empty: Arc<std::ffi::CStr> = jzap::TryDefault::try_default().unwrap();
    assert_eq!(empty.as_ref(), c"");
}
#[test]
fn generic_box_transfer_preserves_dyn_metadata_and_allocation_provenance() {
    trait Value: Send + Sync {
        fn get(&self) -> u64;
    }
    #[repr(align(256))]
    struct Aligned(u64);
    impl Value for Aligned {
        fn get(&self) -> u64 {
            self.0
        }
    }
    let source: Box<dyn Value> = Box::new(Aligned(83));
    let original = (&*source as *const dyn Value).addr();
    let (r, _) = track(1, || Arc::<dyn Value>::try_from_std_box(source));
    let source = r.err().unwrap().value;
    assert_eq!((&*source as *const dyn Value).addr(), original);
    let a = Arc::<dyn Value>::try_from_std_box(source).ok().unwrap();
    assert_eq!(a.as_ref().get(), 83);
    let weak = Arc::try_downgrade(&a).unwrap();
    drop(a);
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    let source: jzap::boxed::Box<dyn std::any::Any> =
        jzap::box_unsize!(jzap::boxed::Box::try_new(()).unwrap()=>dyn std::any::Any);
    let a = Rc::<dyn std::any::Any>::try_from_box_in(source)
        .ok()
        .unwrap();
    assert!(a.as_ref().is::<()>());
    struct Envelope<T: ?Sized> {
        tag: u16,
        tail: T,
    }
    let source: Box<Envelope<[u8]>> = Box::new(Envelope {
        tag: 101,
        tail: [1, 2, 3],
    });
    let owner = Rc::<Envelope<[u8]>>::try_from_std_box(source).ok().unwrap();
    assert_eq!(owner.as_ref().tag, 101);
    assert_eq!(&owner.as_ref().tail, &[1, 2, 3]);
    let w = Rc::try_downgrade(&owner).unwrap();
    drop(owner);
    drop(w);
    fn unwind<T: std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}
    unwind::<Rc<u64>>();
    unwind::<Arc<u64>>();
    let source: Box<[Box<u64>]> = vec![Box::new(89), Box::new(97)].into_boxed_slice();
    let a = Arc::<[Box<u64>]>::try_from_std_box(source).ok().unwrap();
    assert_eq!(**a.get(1).unwrap(), 97);
}
#[test]
fn c_string_and_native_string_shapes() {
    let mut a = Rc::<std::ffi::CStr>::try_from_ref(c"native").unwrap();
    let b = a.try_clone().unwrap();
    assert_eq!(a.as_ref(), c"native");
    assert_eq!(Rc::try_make_mut(&mut a).unwrap().to_bytes(), b"native");
    drop(b);
    let source: Box<std::ffi::CStr> = std::ffi::CString::new("boxed").unwrap().into_boxed_c_str();
    let a = Arc::<std::ffi::CStr>::try_from_std_box(source).unwrap();
    let w = Arc::try_downgrade(&a).unwrap();
    drop(a);
    drop(w);
    #[cfg(feature = "std")]
    {
        use std::{ffi::OsStr, path::Path};
        let mut a = Arc::<OsStr>::try_from_ref(OsStr::new("native")).unwrap();
        let b = a.try_clone().unwrap();
        assert_eq!(Arc::try_make_mut(&mut a).unwrap(), OsStr::new("native"));
        drop(b);
        let path = Rc::<Path>::try_from_ref(Path::new("a/b")).unwrap();
        let weak = Rc::try_downgrade(&path).unwrap();
        drop(path);
        drop(weak);
        let _: Arc<OsStr> = jzap::TryDefault::try_default().unwrap();
        let _: Rc<Path> = jzap::TryDefault::try_default().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let value = OsStr::from_bytes(b"x\xffy");
            let a = Arc::<OsStr>::try_from_ref(value).unwrap();
            assert_eq!(a.as_ref().as_bytes(), b"x\xffy");
        }
    }
}

// ---------------------------------------------------------------------------
// Slice initialization through the shared prefix driver.
//
// These cases reach `<[T] as CloneValue>::clone_into` (through
// `try_from_slice[_in]` and `try_make_mut`) and `Strong::try_init_slice_in`
// with zero length, zero-sized elements and an initializer that returns
// `Ok(())` without writing. `Rc` and `Arc` are `Strong` over `LocalCount` and
// `AtomicCount`, so each scenario is generic over the counter.
// ---------------------------------------------------------------------------

/// Element that counts clones and drops. The `fail_at`-th clone (1-based,
/// shared through `clones`) is refused; zero never refuses.
#[derive(Debug)]
struct CloneProbe<'a> {
    value: usize,
    clones: &'a Cell<usize>,
    drops: &'a Cell<usize>,
    fail_at: usize,
}
impl Drop for CloneProbe<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl TryClone for CloneProbe<'_> {
    fn try_clone(&self) -> Result<Self, Error> {
        let n = self.clones.get() + 1;
        self.clones.set(n);
        if n == self.fail_at {
            Err(Error::Capacity)
        } else {
            Ok(Self {
                value: self.value,
                clones: self.clones,
                drops: self.drops,
                fail_at: self.fail_at,
            })
        }
    }
}
const PROBE_BASE: usize = 100;
fn probes<'a>(
    len: usize,
    clones: &'a Cell<usize>,
    drops: &'a Cell<usize>,
    fail_at: usize,
) -> Vec<CloneProbe<'a>> {
    (0..len)
        .map(|i| CloneProbe {
            value: PROBE_BASE + i,
            clones,
            drops,
            fail_at,
        })
        .collect()
}
fn probe_values_intact(values: &[CloneProbe<'_>]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(i, probe)| probe.value == PROBE_BASE + i)
}

thread_local! {
    static ZST_CLONES: Cell<usize> = const { Cell::new(0) };
    static ZST_DROPS: Cell<usize> = const { Cell::new(0) };
}
/// Zero-sized element with a destructor. Counters are per test thread.
#[derive(Debug)]
struct ZstProbe;
impl Drop for ZstProbe {
    fn drop(&mut self) {
        ZST_DROPS.with(|d| d.set(d.get() + 1));
    }
}
impl TryClone for ZstProbe {
    fn try_clone(&self) -> Result<Self, Error> {
        ZST_CLONES.with(|c| c.set(c.get() + 1));
        Ok(ZstProbe)
    }
}
fn reset_zst_counters() {
    ZST_CLONES.with(|c| c.set(0));
    ZST_DROPS.with(|d| d.set(0));
}
fn zst_clones() -> usize {
    ZST_CLONES.with(Cell::get)
}
fn zst_drops() -> usize {
    ZST_DROPS.with(Cell::get)
}

/// Occupied and reserved bytes, compared before and after a call.
fn ledger(budget: &MemoryBudget) -> (usize, usize) {
    (budget.occupied(), budget.reserved())
}

fn slice_from_slice_clones_each_element_and_drops_once<C: Counter>() {
    const LEN: usize = 5;
    let budget = MemoryBudget::new(4096);
    // A live charge keeps the baseline above zero so a refund to the wrong
    // value cannot hide behind an empty ledger.
    let anchor = Strong::<u64, C, _>::try_new_in(7, Budget::Limited(&budget)).unwrap();
    let before = ledger(&budget);
    assert!(before.0 > 0);

    let clones = Cell::new(0);
    let drops = Cell::new(0);
    let source = probes(LEN, &clones, &drops, 0);
    let copy =
        Strong::<[CloneProbe<'_>], C, _>::try_from_slice_in(&source, Budget::Limited(&budget))
            .ok()
            .unwrap();
    assert_eq!(copy.len(), LEN);
    assert!(
        copy.as_slice()
            .iter()
            .map(|p| p.value)
            .eq(source.iter().map(|p| p.value))
    );
    assert_eq!((clones.get(), drops.get()), (LEN, 0));
    assert!(budget.occupied() > before.0);
    assert_eq!(budget.reserved(), before.1);
    drop(copy);
    assert_eq!(drops.get(), LEN);
    assert_eq!(ledger(&budget), before);
    assert!(probe_values_intact(&source));
    drop(source);
    assert_eq!(drops.get(), 2 * LEN);
    drop(anchor);
    assert_eq!(ledger(&budget), (0, 0));

    // Unmetered policy: the single allocation is released with the copy.
    let clones = Cell::new(0);
    let drops = Cell::new(0);
    let source = probes(LEN, &clones, &drops, 0);
    let ((), state) = track(usize::MAX, || {
        let copy = Strong::<[CloneProbe<'_>], C>::try_from_slice(&source)
            .ok()
            .unwrap();
        assert_eq!(copy.len(), LEN);
        assert!(
            copy.as_slice()
                .iter()
                .map(|p| p.value)
                .eq(source.iter().map(|p| p.value))
        );
        assert_eq!(drops.get(), 0);
        drop(copy);
    });
    assert_eq!(state.live, 0);
    assert!(state.peak > 0);
    assert_eq!((clones.get(), drops.get()), (LEN, LEN));
    drop(source);
    assert_eq!(drops.get(), 2 * LEN);
}
#[test]
fn rc_slice_from_slice_clones_each_element_and_drops_once() {
    slice_from_slice_clones_each_element_and_drops_once::<LocalCount>();
}
#[test]
fn arc_slice_from_slice_clones_each_element_and_drops_once() {
    slice_from_slice_clones_each_element_and_drops_once::<AtomicCount>();
}

fn slice_from_slice_refusal_drops_completed_prefix_once<C: Counter>() {
    const LEN: usize = 5;
    let budget = MemoryBudget::new(4096);
    let anchor = Strong::<u64, C, _>::try_new_in(7, Budget::Limited(&budget)).unwrap();
    let before = ledger(&budget);
    assert!(before.0 > 0);
    // `fail_at` is the 1-based clone number, so the refused element index is
    // `fail_at - 1`, covering positions 0 through LEN - 1.
    for fail_at in 1..=LEN {
        let completed = fail_at - 1;
        let clones = Cell::new(0);
        let drops = Cell::new(0);
        let source = probes(LEN, &clones, &drops, fail_at);
        let result =
            Strong::<[CloneProbe<'_>], C, _>::try_from_slice_in(&source, Budget::Limited(&budget));
        assert!(matches!(result, Err(Error::Capacity)));
        assert_eq!(clones.get(), fail_at, "cloning stops at the first refusal");
        assert_eq!(drops.get(), completed, "completed prefix is dropped once");
        assert_eq!(ledger(&budget), before);
        assert_eq!(source.len(), LEN);
        assert!(probe_values_intact(&source));
        drop(source);
        assert_eq!(drops.get(), completed + LEN);

        let clones = Cell::new(0);
        let drops = Cell::new(0);
        let source = probes(LEN, &clones, &drops, fail_at);
        let (result, state) = track(usize::MAX, || {
            Strong::<[CloneProbe<'_>], C>::try_from_slice(&source)
        });
        assert!(matches!(result, Err(Error::Capacity)));
        assert_eq!((clones.get(), drops.get()), (fail_at, completed));
        assert_eq!(state.live, 0, "the allocation is released on refusal");
        assert!(state.peak > 0);
        assert!(probe_values_intact(&source));
        drop(source);
        assert_eq!(drops.get(), completed + LEN);
    }
    drop(anchor);
    assert_eq!(ledger(&budget), (0, 0));
}
#[test]
fn rc_slice_from_slice_refusal_drops_completed_prefix_once() {
    slice_from_slice_refusal_drops_completed_prefix_once::<LocalCount>();
}
#[test]
fn arc_slice_from_slice_refusal_drops_completed_prefix_once() {
    slice_from_slice_refusal_drops_completed_prefix_once::<AtomicCount>();
}

#[test]
fn rc_copy_on_write_clone_refusal_drops_partial_slice_and_preserves_source() {
    const LEN: usize = 4;
    for failure in 1..=LEN {
        let budget = MemoryBudget::new(4096);
        let clones = Cell::new(0);
        let drops = Cell::new(0);
        let mut a =
            Rc::<[CloneProbe<'_>], _>::try_init_slice_in(LEN, Budget::Limited(&budget), |i, s| {
                s.write(CloneProbe {
                    value: PROBE_BASE + i,
                    clones: &clones,
                    drops: &drops,
                    fail_at: failure,
                })
                .map_err(|e| e.error)
            })
            .unwrap();
        let b = a.try_clone().unwrap();
        assert_eq!(Strong::strong_count(&a), 2);
        let ptr = Rc::as_ptr(&a);
        let before = ledger(&budget);
        assert!(Rc::try_make_mut(&mut a).is_err());
        assert_eq!(Rc::as_ptr(&a), ptr);
        assert_eq!(ledger(&budget), before);
        assert_eq!(clones.get(), failure);
        assert_eq!(drops.get(), failure - 1);
        assert_eq!(a.len(), LEN);
        assert!(
            a.as_slice()
                .iter()
                .enumerate()
                .all(|(i, p)| p.value == PROBE_BASE + i)
        );
        assert_eq!(Strong::strong_count(&a), 2);
        drop((a, b));
        assert_eq!(drops.get(), failure + LEN - 1);
        assert_eq!(ledger(&budget), (0, 0));
    }
}

fn zero_length_slice_init_never_calls_initializer<C: Counter, T>(budget: &MemoryBudget) {
    let before = ledger(budget);
    let calls = Cell::new(0usize);
    let value = Strong::<[T], C, _>::try_init_slice_in(0, Budget::Limited(budget), |_, _| {
        calls.set(calls.get() + 1);
        Ok::<(), ()>(())
    })
    .ok()
    .unwrap();
    assert_eq!(value.len(), 0);
    assert_eq!(calls.get(), 0);
    drop(value);
    assert_eq!(ledger(budget), before);
    let (len, state) = track(usize::MAX, || {
        let value = Strong::<[T], C, _>::try_init_slice_in(0, Unmetered, |_, _| {
            calls.set(calls.get() + 1);
            Ok::<(), ()>(())
        })
        .ok()
        .unwrap();
        let len = value.len();
        drop(value);
        len
    });
    assert_eq!((len, calls.get()), (0, 0));
    assert_eq!(state.live, 0, "the allocation is released with the value");
}
fn zero_length_slice_init<C: Counter>() {
    let budget = MemoryBudget::new(4096);
    let anchor = Strong::<u64, C, _>::try_new_in(7, Budget::Limited(&budget)).unwrap();
    reset_zst_counters();
    zero_length_slice_init_never_calls_initializer::<C, u64>(&budget);
    zero_length_slice_init_never_calls_initializer::<C, ()>(&budget);
    zero_length_slice_init_never_calls_initializer::<C, DropCount<'static>>(&budget);
    zero_length_slice_init_never_calls_initializer::<C, ZstProbe>(&budget);
    assert_eq!(zst_drops(), 0, "no element exists, so none is dropped");
    drop(anchor);
    assert_eq!(ledger(&budget), (0, 0));
}
#[test]
fn rc_slice_init_zero_length_never_calls_initializer() {
    zero_length_slice_init::<LocalCount>();
}
#[test]
fn arc_slice_init_zero_length_never_calls_initializer() {
    zero_length_slice_init::<AtomicCount>();
}

fn zero_sized_slice_init_counts_callbacks_and_drops<C: Counter>() {
    const LEN: usize = 7;
    assert_eq!(size_of::<ZstProbe>(), 0);
    let budget = MemoryBudget::new(4096);
    let anchor = Strong::<u64, C, _>::try_new_in(7, Budget::Limited(&budget)).unwrap();
    let before = ledger(&budget);

    // Success: one callback per element and one drop per element.
    reset_zst_counters();
    let calls = Cell::new(0usize);
    let value =
        Strong::<[ZstProbe], C, _>::try_init_slice_in(LEN, Budget::Limited(&budget), |_, s| {
            calls.set(calls.get() + 1);
            s.write(ZstProbe).map_err(|e| e.error)
        })
        .ok()
        .unwrap();
    assert_eq!((value.len(), calls.get(), zst_drops()), (LEN, LEN, 0));
    drop(value);
    assert_eq!(zst_drops(), LEN);
    assert_eq!(ledger(&budget), before);

    // Zero-sized elements without a destructor still run the callback.
    let calls = Cell::new(0usize);
    let plain = Strong::<[()], C, _>::try_init_slice_in(LEN, Budget::Limited(&budget), |_, s| {
        calls.set(calls.get() + 1);
        s.write(()).map_err(|e| e.error)
    })
    .ok()
    .unwrap();
    assert_eq!((plain.len(), calls.get()), (LEN, LEN));
    drop(plain);
    assert_eq!(ledger(&budget), before);

    // Callback error at the first, a middle and the last position, with and
    // without a value written into the failing slot beforehand.
    for failing in [0, 3, LEN - 1] {
        for write_first in [false, true] {
            reset_zst_counters();
            let calls = Cell::new(0usize);
            let result = Strong::<[ZstProbe], C, _>::try_init_slice_in(
                LEN,
                Budget::Limited(&budget),
                |i, s| {
                    calls.set(calls.get() + 1);
                    if i == failing && !write_first {
                        return Err(9u8);
                    }
                    s.write(ZstProbe).unwrap();
                    if i == failing { Err(9u8) } else { Ok(()) }
                },
            );
            assert!(matches!(result, Err(InitError::Initialize(9))));
            assert_eq!(calls.get(), failing + 1);
            let expected = failing + usize::from(write_first);
            assert_eq!(
                zst_drops(),
                expected,
                "failing={failing} write_first={write_first}"
            );
            assert_eq!(ledger(&budget), before);
        }
    }
    drop(anchor);
    assert_eq!(ledger(&budget), (0, 0));
}
#[test]
fn rc_slice_init_zero_sized_elements_count_callbacks_and_drops() {
    zero_sized_slice_init_counts_callbacks_and_drops::<LocalCount>();
}
#[test]
fn arc_slice_init_zero_sized_elements_count_callbacks_and_drops() {
    zero_sized_slice_init_counts_callbacks_and_drops::<AtomicCount>();
}

fn slice_from_slice_handles_zero_sized_and_empty_sources<C: Counter>() {
    let budget = MemoryBudget::new(4096);
    let anchor = Strong::<u64, C, _>::try_new_in(7, Budget::Limited(&budget)).unwrap();
    let before = ledger(&budget);

    reset_zst_counters();
    let source = [ZstProbe, ZstProbe, ZstProbe, ZstProbe];
    let copy = Strong::<[ZstProbe], C, _>::try_from_slice_in(&source, Budget::Limited(&budget))
        .ok()
        .unwrap();
    assert_eq!((copy.len(), zst_clones(), zst_drops()), (4, 4, 0));
    drop(copy);
    assert_eq!(zst_drops(), 4);
    assert_eq!(ledger(&budget), before);
    let ((), state) = track(usize::MAX, || {
        let copy = Strong::<[ZstProbe], C>::try_from_slice(&source)
            .ok()
            .unwrap();
        assert_eq!((copy.len(), zst_clones(), zst_drops()), (4, 8, 4));
        drop(copy);
    });
    assert_eq!(state.live, 0, "the allocation is released with the copy");
    assert_eq!(zst_drops(), 8);
    drop(source);
    assert_eq!(zst_drops(), 12);

    // Empty sources: no element is cloned or dropped.
    reset_zst_counters();
    let no_zst: [ZstProbe; 0] = [];
    let empty = Strong::<[ZstProbe], C, _>::try_from_slice_in(&no_zst, Budget::Limited(&budget))
        .ok()
        .unwrap();
    assert_eq!((empty.len(), zst_clones()), (0, 0));
    drop(empty);
    assert_eq!((zst_drops(), ledger(&budget)), (0, before));

    let clones = Cell::new(0);
    let drops = Cell::new(0);
    let no_probes = probes(0, &clones, &drops, 0);
    let empty =
        Strong::<[CloneProbe<'_>], C, _>::try_from_slice_in(&no_probes, Budget::Limited(&budget))
            .ok()
            .unwrap();
    assert_eq!((empty.len(), clones.get(), drops.get()), (0, 0, 0));
    drop(empty);
    assert_eq!(ledger(&budget), before);
    let (len, state) = track(usize::MAX, || {
        let empty = Strong::<[CloneProbe<'_>], C>::try_from_slice(&no_probes)
            .ok()
            .unwrap();
        let len = empty.len();
        drop(empty);
        len
    });
    assert_eq!((len, clones.get(), drops.get()), (0, 0, 0));
    assert_eq!(state.live, 0, "the allocation is released with the value");
    drop(anchor);
    assert_eq!(ledger(&budget), (0, 0));
}
#[test]
fn rc_slice_from_slice_handles_zero_sized_and_empty_sources() {
    slice_from_slice_handles_zero_sized_and_empty_sources::<LocalCount>();
}
#[test]
fn arc_slice_from_slice_handles_zero_sized_and_empty_sources() {
    slice_from_slice_handles_zero_sized_and_empty_sources::<AtomicCount>();
}

fn slice_init_unwritten_slot_reports_uninitialized_and_drops_prefix<C: Counter>() {
    const LEN: usize = 5;
    let budget = MemoryBudget::new(4096);
    let anchor = Strong::<u64, C, _>::try_new_in(7, Budget::Limited(&budget)).unwrap();
    let before = ledger(&budget);
    assert!(before.0 > 0);
    // First, a middle and the last slot are left unwritten in turn. Only the
    // completed prefix exists, so exactly `missing` elements are dropped.
    for missing in [0, 2, LEN - 1] {
        let drops = Cell::new(0);
        let calls = Cell::new(0usize);
        let result = Strong::<[DropCount<'_>], C, _>::try_init_slice_in(
            LEN,
            Budget::Limited(&budget),
            |i, slot| {
                calls.set(calls.get() + 1);
                if i != missing {
                    slot.write(DropCount(&drops)).unwrap();
                }
                Ok::<(), ()>(())
            },
        );
        assert!(matches!(
            result,
            Err(InitError::Storage(Error::Uninitialized))
        ));
        assert_eq!(calls.get(), missing + 1, "initialization stops at the slot");
        assert_eq!(drops.get(), missing, "missing={missing}");
        assert_eq!(ledger(&budget), before);

        let drops = Cell::new(0);
        let calls = Cell::new(0usize);
        let (result, state) = track(usize::MAX, || {
            Strong::<[DropCount<'_>], C, _>::try_init_slice_in(LEN, Unmetered, |i, slot| {
                calls.set(calls.get() + 1);
                if i != missing {
                    slot.write(DropCount(&drops)).unwrap();
                }
                Ok::<(), ()>(())
            })
        });
        assert!(matches!(
            result,
            Err(InitError::Storage(Error::Uninitialized))
        ));
        assert_eq!((calls.get(), drops.get()), (missing + 1, missing));
        assert_eq!(state.live, 0);
        assert!(state.peak > 0);

        // Zero-sized elements record initialization too.
        reset_zst_counters();
        let result = Strong::<[ZstProbe], C, _>::try_init_slice_in(
            LEN,
            Budget::Limited(&budget),
            |i, slot| {
                if i != missing {
                    slot.write(ZstProbe).unwrap();
                }
                Ok::<(), ()>(())
            },
        );
        assert!(matches!(
            result,
            Err(InitError::Storage(Error::Uninitialized))
        ));
        assert_eq!(zst_drops(), missing);
        assert_eq!(ledger(&budget), before);
    }
    drop(anchor);
    assert_eq!(ledger(&budget), (0, 0));
}
#[test]
fn rc_slice_init_unwritten_slot_reports_uninitialized_and_drops_prefix() {
    slice_init_unwritten_slot_reports_uninitialized_and_drops_prefix::<LocalCount>();
}
#[test]
fn arc_slice_init_unwritten_slot_reports_uninitialized_and_drops_prefix() {
    slice_init_unwritten_slot_reports_uninitialized_and_drops_prefix::<AtomicCount>();
}
