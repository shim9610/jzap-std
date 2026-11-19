#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod common;
#[path = "common/deallocations.rs"]
mod deallocations;
use common::{FaultAllocator, track};
use jzap::{
    TryClone, TryCloneIn,
    boxed::Box as JBox,
    budget::{Budget, MemoryBudget},
    error::{Error, InitError},
    init::InitSlot,
};
use std::cell::Cell;
#[global_allocator]
static ALLOC: FaultAllocator = FaultAllocator;
#[test]
fn array_initialization_cleans_each_failure_position() {
    struct Value<'a>(&'a Cell<usize>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for fail_at in 0..8 {
        for mode in 0..3 {
            let drops = Cell::new(0);
            let budget = MemoryBudget::new(1024);
            let (result, stats) = track(usize::MAX, || {
                JBox::<[Value<'_>; 8], _>::try_init_array_in(Budget::Limited(&budget), |i, slot| {
                    if i == fail_at && mode == 0 {
                        return Err(Error::Capacity);
                    }
                    if i == fail_at && mode == 2 {
                        return Ok(());
                    }
                    slot.write(Value(&drops)).map_err(|e| e.error)?;
                    if i == fail_at {
                        return Err(Error::Capacity);
                    }
                    Ok(())
                })
            });
            assert!(result.is_err());
            assert_eq!(drops.get(), fail_at + usize::from(mode == 1));
            assert_eq!(
                (
                    stats.calls,
                    stats.live,
                    budget.occupied(),
                    budget.reserved()
                ),
                (1, 0, 0, 0)
            );
            if mode == 2 {
                assert!(matches!(
                    result,
                    Err(InitError::Storage(Error::Uninitialized))
                ));
            }
        }
    }
}
#[test]
fn array_initializer_retries_and_rejects_reinitialization() {
    use jzap::budget::Unmetered;
    let value = JBox::<[u64; 4]>::try_init_in(Unmetered, |slot: &mut InitSlot<'_, [u64; 4]>| {
        let failed = slot.try_init_array(|i, element| {
            if i == 2 {
                return Err(Error::Capacity);
            }
            element.write(i as u64).map_err(|e| e.error)
        });
        assert!(failed.is_err());
        assert!(slot.get().is_none());
        slot.try_init_array(|i, element| element.write(i as u64 + 10).map_err(|e| e.error))?;
        let result = slot.try_init_array(|_, _| -> Result<(), Error> {
            panic!("must not run twice");
        });
        assert!(matches!(
            result,
            Err(InitError::Storage(Error::AlreadyInitialized))
        ));
        Ok::<_, InitError<Error>>(())
    })
    .unwrap();
    assert_eq!(*value, [10, 11, 12, 13]);
    let calls = Cell::new(0);
    let (result, stats) = track(1, || {
        JBox::<[u64; 4]>::try_init_array_in(Unmetered, |_, _| {
            calls.set(calls.get() + 1);
            Ok::<_, Error>(())
        })
    });
    assert!(result.is_err());
    assert_eq!(calls.get(), 0);
    assert_eq!(stats.calls, 1);
}
#[test]
fn zero_length_and_zero_sized_arrays_need_no_allocation() {
    use jzap::budget::Unmetered;
    thread_local! { static DROPS: Cell<usize> = const { Cell::new(0) }; }
    #[repr(align(4096))]
    struct AlignedZero;
    impl Drop for AlignedZero {
        fn drop(&mut self) {
            DROPS.with(|v| v.set(v.get() + 1));
        }
    }
    DROPS.with(|v| v.set(0));
    let ((), stats) = track(1, || {
        drop(
            JBox::<[u64; 0]>::try_init_array_in(Unmetered, |_, _| -> Result<(), Error> {
                panic!("empty");
            })
            .unwrap(),
        );
        drop(
            JBox::<[(); 8]>::try_init_array_in(Unmetered, |_, element| {
                element.write(()).map_err(|e| e.error)
            })
            .unwrap(),
        );
        let value = JBox::<[AlignedZero; 8]>::try_init_array_in(Unmetered, |_, element| {
            element.write(AlignedZero).map_err(|e| e.error)
        })
        .unwrap();
        assert_eq!((&*value as *const [AlignedZero; 8]).addr() % 4096, 0);
        drop(value);
    });
    assert_eq!(stats.calls, 0);
    DROPS.with(|v| assert_eq!(v.get(), 8));
}

#[test]
fn array_target_clone_accounts_for_changed_handle_layout() {
    let source = JBox::try_new(std::array::from_fn::<_, 4, _>(|i| {
        JBox::try_new(i as u8).unwrap()
    }))
    .ok()
    .unwrap();
    let required = 4 * std::mem::size_of::<JBox<u8, Budget<'_>>>() + 4;
    for limit in [required - 1, required] {
        let budget = MemoryBudget::new(limit);
        let ((), stats) = track(usize::MAX, || {
            let result = source.try_clone_in(Budget::Limited(&budget));
            assert_eq!(result.is_ok(), limit == required);
            if let Ok(copy) = result {
                assert_eq!(budget.occupied(), required);
                for (i, value) in copy.iter().enumerate() {
                    assert_eq!(**value, i as u8);
                }
                drop(copy);
            }
            assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
        });
        assert_eq!(stats.live, 0);
    }
}
#[test]
fn box_array_clone_uses_destination_initialization() {
    struct DestinationOnly(u64);
    impl TryClone for DestinationOnly {
        fn try_clone(&self) -> Result<Self, Error> {
            panic!("by-value clone must be bypassed");
        }
        fn try_clone_into(&self, slot: &mut InitSlot<'_, Self>) -> Result<(), Error> {
            slot.write(Self(self.0)).map_err(|e| e.error)
        }
    }
    let source = JBox::try_new([[DestinationOnly(7), DestinationOnly(8)]])
        .ok()
        .unwrap();
    let cloned = source.try_clone().unwrap();
    assert_eq!(cloned[0][0].0, 7);
    assert_eq!(cloned[0][1].0, 8);
}
#[test]
fn panicking_destructor_releases_storage_like_std() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Panics<'a>(&'a Cell<usize>);
    impl Drop for Panics<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            std::panic::panic_any(());
        }
    }
    let drops = Cell::new(0);
    let standard = Box::new(Panics(&drops));
    let address = (&*standard as *const Panics<'_>).addr();
    let (result, count) = deallocations::deallocations_during(address, || {
        catch_unwind(AssertUnwindSafe(|| drop(standard)))
    });
    assert!(result.is_err());
    assert_eq!(count, 1);
    let budget = MemoryBudget::new(1024);
    let value = JBox::try_new_in(Panics(&drops), Budget::Limited(&budget))
        .ok()
        .unwrap();
    let address = (&*value as *const Panics<'_>).addr();
    let (result, count) = deallocations::deallocations_during(address, || {
        catch_unwind(AssertUnwindSafe(|| drop(value)))
    });
    assert!(result.is_err());
    assert_eq!(drops.get(), 2);
    assert_eq!(count, 1);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
}
#[test]
fn budget_reservation_commit_cancel_and_overflow() {
    let b = MemoryBudget::new(usize::MAX);
    let r = b.reserve(usize::MAX).unwrap();
    assert_eq!(b.available(), 0);
    assert!(b.reserve(1).is_err());
    drop(r);
    assert_eq!(b.available(), usize::MAX);
    let charge = b.reserve(42).unwrap().commit();
    assert_eq!((b.occupied(), b.reserved()), (42, 0));
    drop(charge);
    assert_eq!(b.occupied(), 0);
}
#[test]
fn box_failure_returns_input_and_skips_initializer() {
    let b = MemoryBudget::new(1024);
    let calls = Cell::new(0);
    let (r, s) = track(1, || JBox::try_new_in(73u64, Budget::Limited(&b)));
    assert_eq!(r.err().unwrap().value, 73);
    assert_eq!((s.calls, s.live, b.occupied(), b.reserved()), (1, 0, 0, 0));
    let (r, _) = track(1, || {
        JBox::<u64, _>::try_new_with_in(Budget::Limited(&b), || {
            calls.set(1);
            Ok::<_, u8>(5)
        })
    });
    assert!(matches!(
        r,
        Err(InitError::Storage(Error::Allocation { .. }))
    ));
    assert_eq!(calls.get(), 0);
    let (r, s) = track(usize::MAX, || {
        JBox::<u64, _>::try_new_with_in(Budget::Limited(&b), || Err(17u8))
    });
    assert!(matches!(r, Err(InitError::Initialize(17))));
    assert_eq!((s.live, b.occupied(), b.reserved()), (0, 0, 0));
}
#[test]
fn box_zst_alignment_and_drop_once() {
    #[repr(align(4096))]
    struct Aligned(u8);
    let b = MemoryBudget::new(8192);
    let a = JBox::try_new_in(Aligned(7), Budget::Limited(&b))
        .ok()
        .unwrap();
    assert_eq!((&*a as *const Aligned as usize) % 4096, 0);
    assert_eq!(a.0, 7);
    drop(a);
    assert_eq!(b.occupied(), 0);
    let (_, s) = track(1, || {
        drop(JBox::try_new_in((), Budget::Limited(&b)).ok().unwrap())
    });
    assert_eq!(s.calls, 0);
    struct D<'a>(&'a Cell<usize>);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    let drops = Cell::new(0);
    drop(
        JBox::try_new_in(D(&drops), Budget::Limited(&b))
            .ok()
            .unwrap(),
    );
    assert_eq!(drops.get(), 1);
}
#[test]
fn initializer_cannot_claim_unwritten_success_or_write_twice() {
    let b = MemoryBudget::new(1000);
    let r = JBox::<u64, _>::try_init_in(Budget::Limited(&b), |_: &mut InitSlot<'_, u64>| {
        Ok::<_, u8>(())
    });
    assert!(matches!(r, Err(InitError::Storage(Error::Uninitialized))));
    assert_eq!(b.occupied(), 0);
    let r = JBox::try_init_in(Budget::Limited(&b), |slot: &mut InitSlot<'_, u64>| {
        assert!(slot.get().is_none());
        slot.write(7).unwrap();
        let e = slot.write(9).unwrap_err();
        assert_eq!(e.value, 9);
        assert_eq!(e.error, Error::AlreadyInitialized);
        assert_eq!(slot.get(), Some(&7));
        Ok::<_, u8>(())
    })
    .unwrap();
    assert_eq!(*r, 7);
    drop(r);
    assert_eq!(b.occupied(), 0);
}
#[test]
fn initialized_value_is_dropped_on_callback_error() {
    struct D<'a>(&'a Cell<usize>);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    let count = Cell::new(0);
    let b = MemoryBudget::new(1024);
    fn initializer<'a>(count: &'a Cell<usize>) -> impl jzap::init::TryInit<D<'a>, Error = u8> {
        move |slot: &mut InitSlot<'_, D<'a>>| {
            slot.write(D(count)).ok().unwrap();
            Err(19)
        }
    }
    let (r, s) = track(usize::MAX, || {
        JBox::try_init_in(Budget::Limited(&b), initializer(&count))
    });
    assert!(matches!(r, Err(InitError::Initialize(19))));
    assert_eq!(count.get(), 1);
    assert_eq!(s.live, 0);
    assert_eq!(b.occupied(), 0);
}
#[test]
fn scalar_option_tuple_array_and_copy_are_allocation_free() {
    let ((), s) = track(1, || {
        let x = (Some(17u64), [3u16; 16], Ok::<u32, u8>(9));
        assert_eq!(x.try_clone().unwrap(), x);
        assert_eq!(x.try_clone_in(Budget::Unmetered).unwrap(), x);
        let mut out = [0; 16];
        jzap::clone::try_copy_slice(&[8u8; 16], &mut out).unwrap();
        assert_eq!(out, [8; 16]);
        assert!(jzap::clone::try_copy_slice(&[1u8], &mut out).is_err());
        assert_eq!(out, [8; 16]);
    });
    assert_eq!(s.calls, 0);
}
#[test]
fn nested_clone_transfers_every_allocation_to_target_budget() {
    let target = MemoryBudget::new(10000);
    let result = {
        let source = MemoryBudget::new(10000);
        let inner =
            JBox::try_new_in(42u64, Budget::Limited(&source)).unwrap_or_else(|_| unreachable!());
        let outer = JBox::try_new_in(inner, Budget::Limited(&source))
            .ok()
            .unwrap();
        let copy = outer.try_clone_in(Budget::Limited(&target)).unwrap();
        assert_eq!(**copy, 42);
        assert_eq!(source.occupied(), target.occupied());
        drop(outer);
        assert_eq!(source.occupied(), 0);
        copy
    };
    assert_eq!(**result, 42);
    drop(result);
    assert_eq!(target.occupied(), 0);
}
#[test]
fn every_nested_clone_allocation_fails_cleanly_and_retries() {
    let source = MemoryBudget::new(10000);
    let target = MemoryBudget::new(10000);
    let originals: [JBox<u64, Budget<'_>>; 8] = std::array::from_fn(|i| {
        JBox::try_new_in(i as u64, Budget::Limited(&source))
            .ok()
            .unwrap()
    });
    let outer = JBox::try_new_in(originals, Budget::Limited(&source))
        .ok()
        .unwrap();
    let source_charge = source.occupied();
    for fail in 1..=9 {
        let (r, s) = track(fail, || outer.try_clone_in(Budget::Limited(&target)));
        assert!(matches!(r, Err(Error::Allocation { .. })));
        assert_eq!(s.calls, fail);
        assert_eq!(s.live, 0);
        assert_eq!(target.occupied(), 0);
        assert_eq!(target.reserved(), 0);
        assert_eq!(source.occupied(), source_charge);
        for (i, v) in outer.iter().enumerate() {
            assert_eq!(**v, i as u64)
        }
    }
    let ((), s) = track(usize::MAX, || {
        let copy = outer.try_clone_in(Budget::Limited(&target)).unwrap();
        assert_eq!(target.occupied(), source_charge);
        drop(copy)
    });
    assert_eq!(s.calls, 9);
    assert_eq!(s.live, 0);
    drop(outer);
    assert_eq!(source.occupied(), 0);
}
#[test]
fn partial_array_clone_drops_only_completed_prefix() {
    struct Item<'a> {
        id: usize,
        calls: &'a Cell<usize>,
        drops: &'a Cell<usize>,
        fail: usize,
    }
    impl TryClone for Item<'_> {
        fn try_clone(&self) -> Result<Self, Error> {
            let n = self.calls.get();
            self.calls.set(n + 1);
            if n == self.fail {
                return Err(Error::Capacity);
            }
            Ok(Self { ..*self })
        }
    }
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1)
        }
    }
    for fail in 0..8 {
        let calls = Cell::new(0);
        let drops = Cell::new(0);
        let a: [Item<'_>; 8] = std::array::from_fn(|id| Item {
            id,
            calls: &calls,
            drops: &drops,
            fail,
        });
        assert!(a.try_clone().is_err());
        assert_eq!(drops.get(), fail);
        assert_eq!(calls.get(), fail + 1);
        assert_eq!(a[7].id, 7);
        drop(a);
        assert_eq!(drops.get(), fail + 8);
    }
}
#[test]
fn budget_denial_and_clone_order_are_explicit() {
    struct Probe<'a>(&'a Cell<usize>);
    impl TryClone for Probe<'_> {
        fn try_clone(&self) -> Result<Self, Error> {
            self.0.set(self.0.get() + 1);
            Ok(Self(self.0))
        }
    }
    let calls = Cell::new(0);
    let b = MemoryBudget::new(std::mem::size_of::<Probe<'_>>());
    let a = JBox::try_new_in(Probe(&calls), Budget::Limited(&b))
        .ok()
        .unwrap();
    let (r, s) = track(1, || a.try_clone());
    assert!(matches!(r, Err(Error::Budget { .. })));
    assert_eq!(s.calls, 0);
    assert_eq!(calls.get(), 0);
    drop(a);
    assert_eq!(b.occupied(), 0);
}
#[test]
fn interleaved_reservations_and_charges_preserve_limit() {
    for limit in 0..128 {
        let b = MemoryBudget::new(limit);
        for a in 0..=limit {
            let r = b.reserve(a).unwrap();
            assert_eq!(
                (b.occupied(), b.reserved(), b.available()),
                (0, a, limit - a)
            );
            let c = b.reserve(limit - a).unwrap().commit();
            assert_eq!(
                (b.occupied(), b.reserved(), b.available()),
                (limit - a, a, 0)
            );
            drop(r);
            assert_eq!(
                (b.occupied(), b.reserved(), b.available()),
                (limit - a, 0, a)
            );
            drop(c);
            assert_eq!((b.occupied(), b.reserved(), b.available()), (0, 0, limit));
        }
    }
}

#[test]
fn layouts_reject_before_allocator_and_preserve_zst() {
    let (_, s) = track(1, || {
        assert_eq!(jzap::layout::array::<u64>(usize::MAX), Err(Error::Layout));
        assert_eq!(jzap::layout::array::<()>(usize::MAX).unwrap().size(), 0);
        assert_eq!(jzap::layout::array::<u64>(7).unwrap().size(), 56);
    });
    assert_eq!(s.calls, 0);
}

#[test]
fn unmetered_box_layout_matches_std_and_handles_failure() {
    assert_eq!(
        std::mem::size_of::<JBox<u64>>(),
        std::mem::size_of::<std::boxed::Box<u64>>()
    );
    let (r, s) = track(1, || JBox::try_new(19u64));
    assert_eq!(r.err().unwrap().value, 19);
    assert_eq!((s.calls, s.live), (1, 0));
    let b = JBox::try_new(31u64).ok().unwrap();
    let copy = b.try_clone().unwrap();
    assert_eq!(*copy, *b);
}

#[test]
fn shared_reference_clone_preserves_identity_and_result_error_value() {
    let input = 13u64;
    let reference = &input;
    let copied = <&u64 as TryClone>::try_clone(&reference).unwrap();
    assert!(std::ptr::eq(reference, copied));
    let error: Result<u64, u8> = Err(19);
    assert_eq!(error.try_clone().unwrap(), error);
    assert_eq!(error.try_clone_in(Budget::Unmetered).unwrap(), error);
    let mut value = JBox::try_new(1u64).ok().unwrap();
    *value = 2;
    assert_eq!(*value, 2);
    fn send_sync<T: Send + Sync>() {}
    send_sync::<JBox<u64>>();
}

#[test]
fn error_formatting_uses_no_allocation() {
    use std::fmt::Write;
    struct Sink(usize);
    impl std::fmt::Write for Sink {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0 += s.len();
            Ok(())
        }
    }
    let ((), stats) = track(1, || {
        for e in [
            Error::Allocation { size: 8, align: 8 },
            Error::Layout,
            Error::Budget {
                requested: 8,
                available: 0,
            },
            Error::Capacity,
            Error::Bounds,
            Error::InvalidStructure,
            Error::Uninitialized,
            Error::AlreadyInitialized,
        ] {
            let mut sink = Sink(0);
            write!(&mut sink, "{e}").unwrap();
            assert!(sink.0 > 0);
        }
    });
    assert_eq!(stats.calls, 0);
}
