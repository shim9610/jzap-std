//! A valid moving allocator makes the old-plus-new admission cost observable.
mod common;
#[path = "common/deallocations.rs"]
mod deallocations;
use common::{FaultAllocator, track};
use deallocations::deallocations_during;
use jzap::{Error, budget::AllocationPolicy, vec::Vec};
use std::alloc::{GlobalAlloc, Layout};

struct MovingAllocator;
// SAFETY: allocation/deallocation delegate to System through FaultAllocator.
// Reallocation keeps the old block live until a disjoint replacement exists.
unsafe impl GlobalAlloc for MovingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { FaultAllocator.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { FaultAllocator.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { FaultAllocator.dealloc(ptr, layout) }
    }
    // Deliberately inherit GlobalAlloc's default allocate-copy-deallocate
    // realloc: this is the pinned standard-library implementation itself.
}
#[global_allocator]
static ALLOCATOR: MovingAllocator = MovingAllocator;

#[derive(Clone, Copy)]
enum Outcome {
    Budget,
    Allocation,
    Success,
}
fn scenario<P: AllocationPolicy>(
    policy: P,
    used: impl Fn() -> usize,
    limit: usize,
    old: usize,
    new: usize,
    expected: Outcome,
) {
    let fail = if matches!(expected, Outcome::Allocation) {
        2
    } else {
        usize::MAX
    };
    let ((values, result, original), state) = track(fail, || {
        let mut values = Vec::<u8, _>::try_with_capacity_in(old, policy).unwrap();
        values.try_extend_copy(&[7; 60]).unwrap();
        let original = values.as_slice().as_ptr();
        let result = if new < old {
            values.try_shrink_to_fit()
        } else {
            values.try_reserve_exact(new - values.len())
        };
        (values, result, original)
    });
    assert_eq!(values.as_slice(), [7; 60]);
    match expected {
        Outcome::Success => {
            result.unwrap();
            assert_ne!(values.as_slice().as_ptr(), original);
            assert_eq!((values.capacity(), used()), (new, new));
            assert_eq!(
                (state.calls, state.live, state.peak),
                (2, new as isize, (old + new) as isize)
            );
        }
        Outcome::Budget => {
            assert_eq!(
                result,
                Err(Error::Budget {
                    requested: new,
                    available: limit - old
                })
            );
            assert_eq!(
                (state.calls, state.live, state.peak),
                (1, old as isize, old as isize)
            );
        }
        Outcome::Allocation => {
            assert!(matches!(result, Err(Error::Allocation { .. })));
            assert_eq!(
                (state.calls, state.live, state.peak),
                (2, old as isize, old as isize)
            );
        }
    }
    if !matches!(expected, Outcome::Success) {
        assert_eq!(values.as_slice().as_ptr(), original);
        assert_eq!((values.capacity(), used()), (old, old));
    }
    let address = values.as_slice().as_ptr().addr();
    let (_, releases) = deallocations_during(address, || drop(values));
    assert_eq!(releases, 1);
    assert_eq!(used(), 0);
}

#[test]
fn moving_growth_shrink_refusal_and_cancellation_preserve_conservative_admission() {
    use jzap::budget::{Budget, MemoryBudget};
    for (limit, old, new, outcome) in [
        (150, 60, 100, Outcome::Budget),
        (160, 60, 100, Outcome::Success),
        (160, 60, 100, Outcome::Allocation),
        (100, 100, 60, Outcome::Budget),
        (160, 100, 60, Outcome::Success),
    ] {
        let local = MemoryBudget::new(limit);
        scenario(
            Budget::Limited(&local),
            || local.occupied(),
            limit,
            old,
            new,
            outcome,
        );
        assert_eq!(local.reserved(), 0);
        #[cfg(target_has_atomic = "ptr")]
        {
            use jzap::budget::{ConcurrentBudget, ConcurrentMemoryBudget};
            let concurrent = ConcurrentMemoryBudget::new(limit);
            scenario(
                ConcurrentBudget(&concurrent),
                || concurrent.used(),
                limit,
                old,
                new,
                outcome,
            );
        }
    }
}
