//! Concrete gaps identified by the unsafe-site/branch assessment.
mod common;
use common::{FaultAllocator, track};
use jzap::{
    Error, TryClone,
    boxed::Box as B,
    budget::{Budget, MemoryBudget, Unmetered},
    init::InitSlot,
    vec::{LengthLimit, Vec as V},
};
use std::{cell::Cell, mem::MaybeUninit};

#[global_allocator]
static ALLOC: FaultAllocator = FaultAllocator;

#[test]
fn sized_uninitialized_box_preserves_alignment_and_refunds_each_refusal() {
    #[repr(align(128))]
    struct Value<'a>(&'a Cell<usize>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0);
    let budget = MemoryBudget::new(256);
    let ((), state) = track(usize::MAX, || {
        let mut slot = B::<Value<'_>, _>::try_new_uninit_in(Budget::Limited(&budget)).unwrap();
        assert_eq!(B::as_ptr(&slot).addr() % 128, 0);
        slot.write(Value(&drops));
        // SAFETY: the sole slot was initialized immediately above.
        let value = unsafe { B::<MaybeUninit<Value<'_>>, _>::assume_init(slot) };
        drop(value);
    });
    assert_eq!((drops.get(), state.live, budget.occupied()), (1, 0, 0));
    let (result, state) = track(1, || {
        B::<Value<'_>, _>::try_new_uninit_in(Budget::Limited(&budget))
    });
    assert!(result.is_err());
    assert_eq!(
        (
            state.calls,
            state.live,
            budget.occupied(),
            budget.reserved()
        ),
        (1, 0, 0, 0)
    );
    let small = MemoryBudget::new(1);
    let (result, state) = track(usize::MAX, || {
        B::<Value<'_>, _>::try_new_uninit_in(Budget::Limited(&small))
    });
    assert!(result.is_err());
    assert_eq!((state.calls, small.occupied(), drops.get()), (0, 0, 1));
}

#[test]
fn extraction_debug_borrow_tracks_progress_error_and_exhaustion() {
    let mut value = V::try_from_array([11, 17, 23]).unwrap();
    {
        let mut scan = value.try_extract_if(.., |x| Ok::<_, u8>(*x != 17)).unwrap();
        assert!(format!("{scan:?}").contains("Some(11)"));
        assert_eq!(scan.next(), Some(Ok(11)));
        assert!(format!("{scan:?}").contains("Some(17)"));
        assert_eq!(scan.next(), Some(Ok(23)));
        assert!(format!("{scan:?}").contains("None"));
        assert_eq!(scan.next(), None);
    }
    assert_eq!(value.as_slice(), &[17]);
    {
        let mut scan = value.try_extract_if(.., |_| Err::<bool, _>(7)).unwrap();
        assert!(format!("{scan:?}").contains("Some(17)"));
        assert_eq!(scan.next(), Some(Err(7)));
        assert!(format!("{scan:?}").contains("None"));
    }
    assert_eq!(value.as_slice(), &[17]);
}

#[test]
fn slot_defensive_rejections_preserve_the_initialized_value() {
    let value = B::<u64>::try_init_in(Unmetered, |slot: &mut InitSlot<'_, u64>| {
        let ptr = slot.as_uninit_ptr().unwrap();
        // SAFETY: the slot provides exclusive aligned, uninitialized storage.
        unsafe { ptr.as_ptr().write(97) };
        // SAFETY: the previous statement wrote a valid u64 to the sole slot.
        unsafe { slot.mark_initialized() }.unwrap();
        assert_eq!(slot.as_uninit_ptr(), Err(Error::AlreadyInitialized));
        // SAFETY: all bytes remain a valid, exclusively owned initialized u64.
        assert_eq!(
            unsafe { slot.mark_initialized() },
            Err(Error::AlreadyInitialized)
        );
        assert_eq!(slot.get(), Some(&97));
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(*value, 97);
}

#[test]
fn owned_resize_detects_unwritten_clone_and_preserves_input_and_prefix() {
    #[derive(Debug)]
    struct Unwritten<'a>(&'a Cell<usize>, u64);
    impl Drop for Unwritten<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    impl TryClone for Unwritten<'_> {
        fn try_clone(&self) -> Result<Self, Error> {
            Ok(Self(self.0, self.1))
        }
        fn try_clone_into(&self, _: &mut InitSlot<'_, Self>) -> Result<(), Error> {
            Ok(())
        }
    }
    let drops = Cell::new(0);
    let mut value = V::try_from_array([Unwritten(&drops, 11)]).unwrap();
    let rejected = value
        .try_resize_owned(4, Unwritten(&drops, 97))
        .unwrap_err();
    assert_eq!(rejected.error, Error::Uninitialized);
    assert_eq!(rejected.value.1, 97);
    assert_eq!(value.len(), 1);
    assert_eq!(value.get(0).unwrap().1, 11);
    assert_eq!(drops.get(), 0);
    drop(rejected);
    drop(value);
    assert_eq!(drops.get(), 2);
}

#[test]
fn shrink_resize_and_conditional_pop_cover_both_valid_outcomes() {
    let mut value = V::try_from_array([11u64, 17, 23]).unwrap();
    let ((), state) = track(1, || value.try_resize_owned(1, 97).unwrap());
    assert_eq!(state.calls, 0);
    assert_eq!(value.as_slice(), &[11]);
    assert_eq!(
        value.try_pop_if(|x| {
            *x += 1;
            Ok::<_, ()>(false)
        }),
        Ok(None)
    );
    assert_eq!(value.try_pop_if(|x| Ok::<_, ()>(*x == 12)), Ok(Some(12)));
    assert_eq!(
        value.try_pop_if(|_| -> Result<bool, ()> { panic!("empty predicate called") }),
        Ok(None)
    );
}

#[test]
fn zero_sized_flattening_and_std_adoption_preserve_metadata_on_rejection() {
    let value = V::try_from_array([[(); 3]; 2])
        .unwrap()
        .try_into_flattened()
        .unwrap();
    assert_eq!(value.len(), 6);
    let mut standard = Vec::with_capacity(8);
    standard.extend([11u64, 17]);
    let pointer = standard.as_ptr();
    let rejected = V::try_from_std_in(standard, Unmetered, LengthLimit::new(3)).unwrap_err();
    assert_eq!(rejected.error, Error::Capacity);
    assert_eq!(rejected.value.as_ptr(), pointer);
    assert_eq!(rejected.value, [11, 17]);
    let budget = MemoryBudget::new(128);
    let value = V::try_from_std_in(
        rejected.value,
        Budget::Limited(&budget),
        LengthLimit::new(8),
    )
    .unwrap();
    assert_eq!(value.as_ptr(), pointer);
    assert_eq!(budget.occupied(), 64);
    drop(value);
    assert_eq!(budget.occupied(), 0);
}

#[test]
fn split_borrows_reallocation_and_noncopy_values_keep_distinct_ownership() {
    let mut value =
        V::try_from_array([B::try_new(11u64).unwrap(), B::try_new(17).unwrap()]).unwrap();
    value.try_reserve_exact(2).unwrap();
    {
        let (prefix, spare) = value.split_at_spare_mut();
        **prefix.get_mut(0).unwrap() = 23;
        spare.get_mut(0).unwrap().write(B::try_new(31).unwrap());
        assert_eq!(**prefix.get(0).unwrap(), 23);
        assert_eq!(**prefix.get(1).unwrap(), 17);
    }
    // SAFETY: one spare element was initialized and the old prefix remains live.
    unsafe { value.try_set_len(3) }.unwrap();
    value.try_reserve_exact(128).unwrap();
    assert_eq!(
        value.as_slice().iter().map(|x| **x).collect::<Vec<_>>(),
        [23, 17, 31]
    );
}

#[test]
fn empty_slot_observation_never_reads_uninitialized_storage() {
    let value = B::<u64>::try_init_in(Unmetered, |slot: &mut InitSlot<'_, u64>| {
        assert_eq!(slot.get().copied(), None);
        slot.write(97).map_err(|e| e.error)?;
        assert_eq!(slot.get().copied(), Some(97));
        Ok::<_, Error>(())
    })
    .unwrap();
    assert_eq!(*value, 97);
}

#[test]
fn formatting_remembers_the_first_refusal_even_if_display_ignores_it() {
    struct IgnoresRefusal;
    impl std::fmt::Display for IgnoresRefusal {
        fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            assert!(output.write_str("too long").is_err());
            // This write would fit by itself. The first error must stay latched.
            assert!(output.write_str("x").is_err());
            Ok(())
        }
    }
    let budget = MemoryBudget::new(32);
    let mut value = V::with_limit_in(2, Budget::Limited(&budget));
    value.try_push(b'a').unwrap();
    assert_eq!(
        value.try_write_fmt(format_args!("{IgnoresRefusal}")),
        Err(Error::Capacity)
    );
    assert_eq!(value.as_slice(), b"a");
    value.try_write_fmt(format_args!("x")).unwrap();
    assert_eq!(value.as_slice(), b"ax");
    drop(value);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
}

#[test]
fn macro_expansions_preserve_field_pin_and_call_ownership() {
    struct Record {
        a: u32,
        b: u64,
        _pin: std::marker::PhantomPinned,
    }
    let record = B::<Record>::try_init_in(Unmetered, |slot: &mut InitSlot<'_, Record>| {
        jzap::try_init_struct!(
            slot,
            Record {
                a: |s: &mut InitSlot<'_, u32>| s.write(11).map_err(|e| e.error),
                b: |s: &mut InitSlot<'_, u64>| s.write(17).map_err(|e| e.error),
                _pin: |s: &mut InitSlot<'_, std::marker::PhantomPinned>| s
                    .write(std::marker::PhantomPinned)
                    .map_err(|e| e.error),
            }
        )
    })
    .unwrap();
    let address = B::as_ptr(&record).addr();
    let erased = jzap::box_unsize!(record => dyn std::any::Any);
    let record = erased.downcast::<Record>().unwrap();
    let pinned = B::into_pin(record);
    let pinned = jzap::pin_box_unsize!(pinned => dyn std::any::Any);
    let borrowed = pinned.as_ref().get_ref().downcast_ref::<Record>().unwrap();
    assert_eq!(borrowed as *const Record as usize, address);
    assert_eq!(borrowed.a as u64 + borrowed.b, 28);
    let budget = MemoryBudget::new(128);
    let capture = B::try_new_in(31u64, Budget::Limited(&budget)).unwrap();
    let callback = B::try_new_in(move |x: u64| *capture + x, Budget::Limited(&budget))
        .unwrap_or_else(|_| panic!("callable allocation refused"));
    assert_eq!(jzap::box_call_once!(callback; 11), 42);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
}

#[test]
fn all_six_send_sync_impls_support_transfer_and_scoped_shared_access() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct Watch(Arc<AtomicUsize>, u64);
    impl Drop for Watch {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let boxed = B::try_new(Watch(drops.clone(), 11)).unwrap();
    let vector = V::try_from_array([Watch(drops.clone(), 17), Watch(drops.clone(), 23)]).unwrap();
    let iter = V::try_from_array([Watch(drops.clone(), 31), Watch(drops.clone(), 37)])
        .unwrap()
        .try_into_boxed_slice()
        .unwrap()
        .into_iter();
    std::thread::scope(|scope| {
        let a = scope.spawn(|| boxed.1);
        let b = scope.spawn(|| vector.as_slice().iter().map(|x| x.1).sum::<u64>());
        let c = scope.spawn(|| iter.len());
        assert_eq!(
            (a.join().unwrap(), b.join().unwrap(), c.join().unwrap()),
            (11, 40, 2)
        );
    });
    let a = std::thread::spawn(move || {
        let result = boxed.1;
        drop(boxed);
        result
    });
    let b = std::thread::spawn(move || vector.into_iter().map(|x| x.1).sum::<u64>());
    let c = std::thread::spawn(move || iter.map(|x| x.1).sum::<u64>());
    assert_eq!(
        (a.join().unwrap(), b.join().unwrap(), c.join().unwrap()),
        (11, 40, 68)
    );
    assert_eq!(drops.load(Ordering::SeqCst), 5);
}
