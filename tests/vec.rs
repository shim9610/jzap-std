use jzap::{
    Error, TryClone, TryCloneIn,
    boxed::Box as B,
    budget::{Budget, MemoryBudget, Unmetered},
    error::InitError,
    vec::{LengthLimit, Unlimited, Vec as V},
};
use std::{
    cell::Cell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};
mod common;
#[path = "common/deallocations.rs"]
mod deallocations;
#[global_allocator]
static ALLOC: common::FaultAllocator = common::FaultAllocator;
fn refuse_after(n: usize) {
    common::STATE.with(|s| {
        let mut v = s.get();
        v.fail = v.calls + n;
        s.set(v);
    });
}
fn allow() {
    common::STATE.with(|s| {
        let mut v = s.get();
        v.fail = usize::MAX;
        s.set(v);
    });
}

#[test]
fn limit_creation_exact_boundary_growth_clamping_and_reuse() {
    let exact = V::<u8>::try_with_capacity_and_limit(3, 3).unwrap();
    assert_eq!(exact.capacity(), 3);
    drop(exact);
    let ((), stats) = common::track(usize::MAX, || {
        let mut v = V::<u8>::with_limit(3);
        assert_eq!((v.len(), v.capacity(), v.max_len()), (0, 0, 3));
        assert_eq!(common::STATE.with(Cell::get).calls, 0);
        for i in 0..3 {
            v.try_push(i).unwrap();
            assert!(v.capacity() <= 3);
        }
        let ptr = v.as_ptr();
        let rejected = v.try_push(9).unwrap_err();
        assert_eq!((rejected.value, rejected.error), (9, Error::Capacity));
        assert_eq!(v.as_slice(), &[0, 1, 2]);
        assert_eq!(v.as_ptr(), ptr);
        assert_eq!(v.pop(), Some(2));
        v.try_push(8).unwrap();
        v.clear();
        v.try_extend_copy(&[4, 5, 6]).unwrap();
        assert_eq!(v.capacity(), 3);
        assert_eq!(v.as_ptr(), ptr);
        assert_eq!(v.try_reserve(1), Err(Error::Capacity));
        assert_eq!(v.try_reserve_exact(usize::MAX), Err(Error::Capacity));
        drop(v);
    });
    assert_eq!(stats.calls, 1);
    assert_eq!(stats.live, 0);
    let (r, stats) = common::track(1, || V::<u64>::try_with_capacity_and_limit(5, 4));
    assert_eq!(r.unwrap_err(), Error::Capacity);
    assert_eq!(stats.calls, 0);
    assert_eq!(size_of::<V<u64>>(), size_of::<Vec<u64>>());
    assert_eq!(
        size_of::<V<u64, Unmetered, LengthLimit>>(),
        size_of::<Vec<u64>>() + size_of::<usize>()
    );
}
#[test]
fn every_growth_entry_checks_the_limit_before_allocating_or_callbacks() {
    let ((), stats) = common::track(usize::MAX, || {
        let mut v = V::<u8>::with_limit(4);
        v.try_extend_copy(&[1, 2, 3]).unwrap();
        let mut source = V::<u8>::new();
        source.try_extend_copy(&[8, 9]).unwrap();
        let original = v.as_ptr();
        let source_ptr = source.as_ptr();
        refuse_after(1);
        assert_eq!(v.try_insert(4, 7).unwrap_err().error, Error::Bounds);
        assert_eq!(v.try_extend_copy(&[8, 9]), Err(Error::Capacity));
        assert_eq!(v.try_extend_cloned(&[8, 9]), Err(Error::Capacity));
        assert_eq!(v.try_append(&mut source), Err(Error::Capacity));
        assert_eq!(
            v.try_resize_with(5, || -> Result<u8, ()> { panic!("must not run") }),
            Err(InitError::Storage(Error::Capacity))
        );
        assert_eq!(v.try_resize(5, &1), Err(Error::Capacity));
        assert_eq!(v.as_slice(), &[1, 2, 3]);
        assert_eq!(v.as_ptr(), original);
        assert_eq!(source.as_slice(), &[8, 9]);
        assert_eq!(source.as_ptr(), source_ptr);
        allow();
        v.try_insert(1, 7).unwrap();
        assert_eq!(v.try_insert(1, 6).unwrap_err().value, 6);
    });
    assert_eq!(stats.calls, 2);
    assert_eq!(stats.live, 0);
}
#[test]
fn growth_allocator_refusal_preserves_exact_owner_and_can_retry() {
    for operation in 0..4 {
        let ((), stats) = common::track(usize::MAX, || {
            let budget = MemoryBudget::new(4096);
            let mut v = V::<u64, _>::with_limit_in(9, Budget::Limited(&budget));
            v.try_extend_copy(&[1, 2, 3, 4]).unwrap();
            let old = (v.as_ptr(), v.len(), v.capacity(), budget.occupied());
            refuse_after(1);
            let error = match operation {
                0 => v.try_push(9).unwrap_err().error,
                1 => v.try_insert(1, 9).unwrap_err().error,
                2 => v.try_reserve(3).unwrap_err(),
                _ => v.try_reserve_exact(3).unwrap_err(),
            };
            assert!(matches!(error, Error::Allocation { .. }));
            assert_eq!((v.as_ptr(), v.len(), v.capacity(), budget.occupied()), old);
            assert_eq!(v.as_slice(), &[1, 2, 3, 4]);
            assert_eq!(budget.reserved(), 0);
            allow();
            v.try_push(5).unwrap();
            assert_eq!(v.capacity(), 8);
            drop(v);
            assert_eq!(budget.occupied(), 0);
        });
        assert_eq!(stats.live, 0);
    }
}
#[test]
fn budget_growing_reserves_whole_new_request_and_refunds_after_release() {
    let budget = MemoryBudget::new(24);
    let mut v = V::<u64, _>::with_limit_in(4, Budget::Limited(&budget));
    v.try_reserve_exact(2).unwrap();
    v.try_extend_copy(&[1, 2]).unwrap();
    assert_eq!(budget.occupied(), 16);
    assert_eq!(
        v.try_push(3).unwrap_err().error,
        Error::Budget {
            requested: 32,
            available: 8
        }
    );
    assert_eq!(
        (v.len(), v.capacity(), budget.occupied(), budget.reserved()),
        (2, 2, 16, 0)
    );
    v.pop();
    v.try_shrink_to_fit().unwrap();
    assert_eq!(budget.occupied(), 8);
    v.clear();
    v.try_shrink_to_fit().unwrap();
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn zero_limit_zero_size_alignment_and_layout_overflow() {
    let ((), stats) = common::track(1, || {
        let mut zero = V::<u64>::with_limit(0);
        assert_eq!(zero.try_push(9).unwrap_err().error, Error::Capacity);
        assert_eq!(zero.try_reserve(1), Err(Error::Capacity));
        zero.try_reserve(0).unwrap();
        let mut z = V::<()>::with_limit(2);
        assert_eq!(z.capacity(), 2);
        z.try_push(()).unwrap();
        z.try_push(()).unwrap();
        assert_eq!(z.try_push(()).unwrap_err().error, Error::Capacity);
        let mut huge = V::<()>::new();
        huge.try_reserve_exact(usize::MAX).unwrap();
        assert_eq!(
            V::<u64>::try_with_capacity(usize::MAX).unwrap_err(),
            Error::Layout
        );
    });
    assert_eq!(stats.calls, 0);
    #[repr(align(4096))]
    struct Aligned(u8);
    let mut v = V::with_limit(3);
    v.try_push(Aligned(7)).ok().unwrap();
    assert_eq!(v.as_ptr().addr() % 4096, 0);
    assert_eq!(v.get(0).unwrap().0, 7);
    let drops = Cell::new(0);
    struct Count<'a>(&'a Cell<usize>);
    impl Drop for Count<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let mut v = V::with_limit(3);
    for _ in 0..3 {
        v.try_push(Count(&drops)).ok().unwrap();
    }
    let mut i = v.into_iter();
    drop(i.next());
    drop(i.next_back());
    drop(i);
    assert_eq!(drops.get(), 3);
}
#[test]
fn editing_split_append_and_owned_iteration_match_std() {
    let mut v = V::with_limit(12);
    v.try_extend(0u64..8).unwrap();
    assert_eq!(v.remove(2), Ok(2));
    assert_eq!(v.swap_remove(1), Ok(1));
    v.try_insert(2, 99).unwrap();
    let mut s: Vec<_> = (0u64..8).collect();
    s.remove(2);
    s.swap_remove(1);
    s.insert(2, 99);
    assert_eq!(v.as_slice(), s.as_slice());
    let mut tail = v.try_split_off(3).unwrap();
    let mut st = s.split_off(3);
    assert_eq!(tail.max_len(), 12);
    assert_eq!(tail.as_slice(), st.as_slice());
    v.try_append(&mut tail).unwrap();
    s.append(&mut st);
    assert!(tail.is_empty());
    let mut ours = v.into_iter();
    let mut standard = s.into_iter();
    assert_eq!(ours.next(), standard.next());
    assert_eq!(ours.next_back(), standard.next_back());
    assert_eq!(ours.as_slice(), standard.as_slice());
    assert_eq!(ours.collect::<Vec<_>>(), standard.collect::<Vec<_>>());
}
#[test]
fn clone_and_resize_partial_failure_drop_suffix_and_preserve_prefix() {
    for failure in 1..=5 {
        let ((), stats) = common::track(usize::MAX, || {
            let mut source = V::with_limit(6);
            for i in 0..4 {
                source.try_push(B::try_new(i as u64).unwrap()).unwrap();
            }
            let mut target = V::with_limit(6);
            target.try_push(B::try_new(77).unwrap()).unwrap();
            refuse_after(failure);
            assert!(target.try_extend_cloned(source.as_slice()).is_err());
            assert_eq!(target.len(), 1);
            assert_eq!(**target.get(0).unwrap(), 77);
            assert_eq!(source.len(), 4);
            allow();
            target.try_extend_cloned(source.as_slice()).unwrap();
            assert_eq!(target.len(), 5);
        });
        assert_eq!(stats.live, 0);
    }
    for position in 0..4 {
        for after_write in [false, true] {
            let drops = Cell::new(0);
            struct D<'a>(&'a Cell<usize>);
            impl Drop for D<'_> {
                fn drop(&mut self) {
                    self.0.set(self.0.get() + 1);
                }
            }
            let mut v = V::with_limit(8);
            v.try_push(D(&drops)).ok().unwrap();
            let r = v.try_resize_with_init(5, |i, slot| {
                if i == position + 1 && !after_write {
                    return Err(9);
                }
                slot.write(D(&drops)).ok().unwrap();
                if i == position + 1 {
                    return Err(9);
                }
                Ok(())
            });
            assert_eq!(r, Err(InitError::Initialize(9)));
            assert_eq!(v.len(), 1);
            assert_eq!(drops.get(), position + usize::from(after_write));
            drop(v);
            assert_eq!(drops.get(), position + usize::from(after_write) + 1);
        }
    }
    let mut v = V::<u64>::with_limit(2);
    assert_eq!(
        v.try_resize_with_init(1, |_, _| Ok::<_, ()>(())),
        Err(InitError::Storage(Error::Uninitialized))
    );
    assert!(v.is_empty());
}
#[test]
fn iterator_failure_limit_and_dishonest_hints_roll_back_contents() {
    let mut v = V::with_limit(3);
    v.try_push(7).unwrap();
    assert_eq!(v.try_extend([1, 2, 3]), Err(Error::Capacity));
    assert_eq!(v.as_slice(), &[7]);
    assert_eq!(
        v.try_extend_fallible(jzap::iter::from_results([Ok(1), Err(42), Ok(3)])),
        Err(InitError::Initialize(42))
    );
    assert_eq!(v.as_slice(), &[7]);
    struct Dishonest(bool);
    impl Iterator for Dishonest {
        type Item = u8;
        fn next(&mut self) -> Option<u8> {
            if self.0 {
                self.0 = false;
                Some(8)
            } else {
                None
            }
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (usize::MAX, Some(usize::MAX))
        }
    }
    v.try_extend(Dishonest(true)).unwrap();
    assert_eq!(v.as_slice(), &[7, 8]);
}
#[test]
fn clone_keeps_limits_and_replacement_retains_destination_policy() {
    let source_budget = MemoryBudget::new(1024);
    let dest_budget = MemoryBudget::new(1024);
    let mut source = V::<u64, _>::with_limit_in(5, Budget::Limited(&source_budget));
    source.try_extend_copy(&[1, 2, 3]).unwrap();
    let cloned = source.try_clone_in(Budget::Limited(&dest_budget)).unwrap();
    assert_eq!(cloned.max_len(), 5);
    assert_eq!(cloned.as_slice(), &[1, 2, 3]);
    let mut small = V::<u64, _>::with_limit_in(2, Budget::Limited(&dest_budget));
    small.try_push(9).unwrap();
    assert_eq!(small.try_clone_from(&source), Err(Error::Capacity));
    assert_eq!(small.as_slice(), &[9]);
    let mut target = V::<u64, _>::with_limit_in(9, Budget::Limited(&dest_budget));
    target.try_extend_copy(&[7, 8, 9]).unwrap();
    let p = target.as_ptr();
    let occupied = dest_budget.occupied();
    target.try_clone_from(&source).unwrap();
    assert_eq!(target.max_len(), 9);
    assert_eq!(target.as_ptr(), p);
    assert_eq!(dest_budget.occupied(), occupied);
    drop((source, cloned, small, target));
    assert_eq!(source_budget.occupied(), 0);
    assert_eq!(dest_budget.occupied(), 0);
}
#[test]
fn copy_witnessed_arrays_clone_in_one_bulk_copy_with_limits_and_refusals() {
    // Arrays of primitives have a copy witness but no own bulk hook; cloning
    // takes the Vec bulk branch. Check contents, limit, allocation refusal and
    // budget accounting on that branch.
    let budget = MemoryBudget::new(1024);
    let mut source = V::<[u16; 3], _>::with_limit_in(4, Budget::Limited(&budget));
    source.try_push([1, 2, 3]).unwrap();
    source.try_push([4, 5, 6]).unwrap();
    source.try_push([7, 8, 9]).unwrap();
    let copy = source.try_clone().unwrap();
    assert_eq!(copy.as_slice(), source.as_slice());
    assert_eq!(copy.max_len(), 4);
    assert_ne!(copy.as_ptr(), source.as_ptr());
    let mut target = V::<[u16; 3], _>::with_limit_in(4, Budget::Limited(&budget));
    target.try_push([0, 0, 0]).unwrap();
    target.try_push([0, 0, 1]).unwrap();
    assert_eq!(
        target.try_extend_cloned(source.as_slice()),
        Err(Error::Capacity)
    );
    assert_eq!(target.as_slice(), &[[0, 0, 0], [0, 0, 1]]);
    target.try_extend_cloned(&source.as_slice()[1..3]).unwrap();
    assert_eq!(
        target.as_slice(),
        &[[0, 0, 0], [0, 0, 1], [4, 5, 6], [7, 8, 9]]
    );
    let occupied = budget.occupied();
    let mut fresh = V::<[u16; 3], _>::new_in(Budget::Limited(&budget));
    let (refused, state) = common::track(1, || fresh.try_extend_cloned(source.as_slice()));
    assert!(matches!(refused, Err(Error::Allocation { .. })));
    assert_eq!((state.calls, state.live), (1, 0));
    assert!(fresh.is_empty());
    assert_eq!(budget.occupied(), occupied);
    let nested = V::<[[u8; 2]; 2]>::try_from_array([[[1, 2], [3, 4]], [[5, 6], [7, 8]]]).unwrap();
    assert_eq!(nested.try_clone().unwrap().as_slice(), nested.as_slice());
    drop((source, copy, target, fresh));
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn nested_target_clone_uses_output_layout_and_target_budget() {
    let mut source = V::with_limit(3);
    source.try_push(B::try_new(11u64).unwrap()).unwrap();
    let budget = MemoryBudget::new(16 + 8);
    let copy = source.try_clone_in(Budget::Limited(&budget)).unwrap();
    assert_eq!(copy.max_len(), 3);
    assert_eq!(**copy.get(0).unwrap(), 11);
    assert_eq!(budget.occupied(), size_of::<B<u64, Budget<'_>>>() + 8);
    drop(copy);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn owner_transfers_preserve_pointer_and_reject_capacity_or_budget() {
    let mut s = Vec::with_capacity(8);
    s.extend([1u64, 2, 3]);
    let ptr = s.as_ptr();
    let r = V::try_from_std_in(s, Unmetered, LengthLimit::new(4)).unwrap_err();
    assert_eq!(r.error, Error::Capacity);
    assert_eq!(r.value.as_ptr(), ptr);
    assert_eq!(r.value.capacity(), 8);
    let budget = MemoryBudget::new(63);
    let r = V::try_from_std_in(r.value, Budget::Limited(&budget), LengthLimit::new(8)).unwrap_err();
    assert!(matches!(r.error, Error::Budget { .. }));
    assert_eq!(r.value.as_ptr(), ptr);
    let mut v = V::try_from_std_in(r.value, Unmetered, LengthLimit::new(8)).unwrap();
    let parts = v.into_raw_parts();
    v = unsafe { V::from_raw_parts(parts.0, parts.1, parts.2, parts.3, parts.4) };
    assert_eq!(v.as_ptr(), ptr);
    let standard = v.into_std();
    assert_eq!(standard.as_ptr(), ptr);
    let b = B::<[u64]>::try_copy_from_slice(&[4, 5]).unwrap();
    let p = b.as_slice().as_ptr();
    let v = V::from_boxed_slice(b);
    assert_eq!(v.as_ptr(), p);
    assert_eq!(v.try_into_boxed_slice().unwrap().as_slice().as_ptr(), p);
    let z = V::try_from_std_in(vec![(); 3], Unmetered, LengthLimit::new(4)).unwrap();
    assert_eq!((z.len(), z.capacity()), (3, 4));
    assert_eq!(z.into_std().len(), 3);
}
#[test]
fn shrink_box_conversion_split_and_append_refusal_preserve_owners() {
    let ((), stats) = common::track(usize::MAX, || {
        let mut v = V::<u64>::try_with_capacity_and_limit(8, 10).unwrap();
        v.try_extend_copy(&[1, 2, 3]).unwrap();
        let ptr = v.as_ptr();
        refuse_after(1);
        let r = v.try_into_boxed_slice().unwrap_err();
        assert!(matches!(r.error, Error::Allocation { .. }));
        let mut v = r.value;
        assert_eq!((v.as_ptr(), v.len(), v.capacity()), (ptr, 3, 8));
        refuse_after(1);
        assert!(v.try_split_off(1).is_err());
        assert_eq!(v.as_slice(), &[1, 2, 3]);
        allow();
        let mut other = V::new();
        other.try_extend_copy(&[4, 5, 6, 7, 8, 9]).unwrap();
        let p = other.as_ptr();
        refuse_after(1);
        assert!(v.try_append(&mut other).is_err());
        assert_eq!(v.as_slice(), &[1, 2, 3]);
        assert_eq!(other.as_ptr(), p);
        assert_eq!(other.len(), 6);
        allow();
        let b = v.try_into_boxed_slice().unwrap();
        assert_eq!(b.as_slice(), &[1, 2, 3]);
    });
    assert_eq!(stats.live, 0);
}
#[test]
fn unwind_cleans_partial_initialization_iterator_suffix_and_allocation() {
    struct Boom;
    impl Drop for Boom {
        fn drop(&mut self) {
            panic!("drop test");
        }
    }
    let mut v = V::with_limit(1);
    v.try_push(Boom).ok().unwrap();
    let ptr = v.as_ptr().addr();
    let (_, freed) = deallocations::deallocations_during(ptr, || {
        assert!(catch_unwind(AssertUnwindSafe(|| drop(v))).is_err());
    });
    // Boom is a ZST, so no allocator call is expected.
    assert_eq!(freed, 0);
    struct BigBoom(u64);
    impl Drop for BigBoom {
        fn drop(&mut self) {
            assert_eq!(self.0, 9);
            panic!("drop test");
        }
    }
    let mut v = V::with_limit(1);
    v.try_push(BigBoom(9)).ok().unwrap();
    let ptr = v.as_ptr().addr();
    let (_, freed) = deallocations::deallocations_during(ptr, || {
        assert!(catch_unwind(AssertUnwindSafe(|| drop(v.into_iter()))).is_err());
    });
    assert_eq!(freed, 1);
    let mut v = V::with_limit(8);
    v.try_push(77u64).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = v.try_resize_with(4, || -> Result<_, ()> { panic!("init test") });
        }))
        .is_err()
    );
    assert_eq!(v.as_slice(), &[77]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = v.try_extend((0..3).map(|i| {
                if i == 2 {
                    panic!("iterator test")
                }
                i
            }));
        }))
        .is_err()
    );
    assert_eq!(v.as_slice(), &[77]);
}
#[test]
fn unrestricted_capacity_schedule_matches_std_for_growth() {
    let mut ours = V::new();
    let mut standard = Vec::new();
    for i in 0u64..1000 {
        ours.try_push(i).unwrap();
        standard.push(i);
        assert_eq!(ours.capacity(), standard.capacity());
    }
    assert_eq!(ours.as_slice(), standard.as_slice());
    let limited = V::try_from_std_in(standard, Unmetered, Unlimited).unwrap();
    assert_eq!(limited.as_slice(), ours.as_slice());
}

#[test]
fn true_zero_size_drop_counts_and_maximum_logical_length() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    struct Z;
    impl Drop for Z {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }
    DROPS.store(0, Ordering::Relaxed);
    let ((), s) = common::track(1, || {
        let mut v = V::<Z>::with_limit(4);
        for _ in 0..4 {
            v.try_push(Z).ok().unwrap();
        }
        let rejected = v.try_push(Z).err().unwrap();
        assert_eq!(rejected.error, Error::Capacity);
        drop(rejected);
        drop(v.remove(1).unwrap());
        drop(v.swap_remove(1).unwrap());
        let mut i = v.into_iter();
        drop(i.next_back());
        drop(i);
    });
    assert_eq!(DROPS.load(Ordering::Relaxed), 5);
    assert_eq!(s.calls, 0);
    // SAFETY: units require no bytes, all logical units are valid; no allocation/charge exists.
    let mut huge = unsafe {
        V::<()>::from_raw_parts(
            std::ptr::NonNull::dangling(),
            usize::MAX,
            usize::MAX,
            Unmetered,
            Unlimited,
        )
    };
    assert_eq!(huge.try_push(()).unwrap_err().error, Error::Capacity);
    assert_eq!(huge.try_reserve(1), Err(Error::Capacity));
    assert_eq!(huge.pop(), Some(()));
    huge.try_push(()).unwrap();
    assert_eq!(huge.len(), usize::MAX);
    let mut i = huge.into_iter();
    assert_eq!(i.next(), Some(()));
    assert_eq!(i.next_back(), Some(()));
    assert_eq!(i.len(), usize::MAX - 2);
}
#[test]
fn clone_replacement_failure_preserves_pointer_contents_capacity_and_limit() {
    for failure in 1..=4 {
        let ((), stats) = common::track(usize::MAX, || {
            let mut source = V::with_limit(3);
            for i in 0..3 {
                source.try_push(B::try_new(i as u64).unwrap()).unwrap();
            }
            let mut dest = V::with_limit(7);
            dest.try_push(B::try_new(77).unwrap()).unwrap();
            let before = (dest.as_ptr(), dest.capacity());
            refuse_after(failure);
            assert!(dest.try_clone_from(&source).is_err());
            assert_eq!((dest.as_ptr(), dest.capacity()), before);
            assert_eq!((dest.len(), dest.max_len()), (1, 7));
            assert_eq!(**dest.get(0).unwrap(), 77);
            allow();
            dest.try_clone_from(&source).unwrap();
            assert_eq!(dest.max_len(), 7);
        });
        assert_eq!(stats.live, 0);
    }
}
#[test]
fn initialization_panic_after_write_cleans_current_and_completed_suffix() {
    struct D<'a>(&'a Cell<usize>);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0);
    let mut v = V::with_limit(5);
    v.try_push(D(&drops)).ok().unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = v.try_resize_with_init(5, |i, slot| -> Result<(), ()> {
                slot.write(D(&drops)).ok().unwrap();
                if i == 3 {
                    panic!("after write")
                };
                Ok(())
            });
        }))
        .is_err()
    );
    assert_eq!(v.len(), 1);
    assert_eq!(drops.get(), 3);
    drop(v);
    assert_eq!(drops.get(), 4);
}

#[path = "vec_cases/complete.rs"]
mod complete;
