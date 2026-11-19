use super::*;
use count::{Counter, LocalCount};

#[cfg(target_has_atomic = "ptr")]
#[kani::proof]
fn retirement_preserves_every_valid_weak_credit() {
    let credits: usize = kani::any();
    let bytes: usize = kani::any();
    kani::assume(credits > 0 && credits <= count::WEAK_MAX_COUNT);
    kani::assume(bytes <= isize::MAX as usize);
    let strong = unsafe { count::AtomicCount::new(0) };
    let weak = unsafe { count::AtomicCount::new(credits) };
    strong.retire(&weak, bytes);
    assert_eq!(weak.load(), count::RETIRED | credits);
    assert_eq!(count::retired_bytes(strong.load()), bytes);
    assert_eq!(weak.load() & !count::RETIRED, credits);
    assert_ne!(weak.load(), count::LOCK);
}

#[cfg(target_has_atomic = "ptr")]
#[kani::proof]
fn buffered_increment_rolls_back_exactly_its_own_credit() {
    let live: usize = kani::any();
    let pending: usize = kani::any();
    kani::assume(live > 0 && live <= count::MAX_COUNT);
    // The numeric kernel is conditional on pending reservations. The separate
    // platform refinement supplies this bound for the native dispatcher.
    kani::assume(pending < count::MAX_COUNT);
    let raw = live.checked_add(pending).unwrap();
    let strong = unsafe { count::AtomicCount::new(raw) };
    let result = strong.buffered_clone_strong();
    if raw < count::MAX_COUNT {
        assert_eq!(result, Ok(()));
        assert_eq!(strong.load(), raw + 1);
    } else {
        assert_eq!(result, Err(Error::RefCount));
        assert_eq!(strong.load(), raw);
    }
    assert!(strong.load() > 0);
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64",
    not(jzap_small_counts)
))]
#[kani::proof]
fn linux_native_activation_bound_fits_strong_headroom() {
    let live: usize = kani::any();
    let running: u32 = kani::any();
    let suspended: usize = kani::any();
    kani::assume(live > 0 && live <= count::MAX_COUNT);
    // Trusted Linux native-ABI refinement, checked against pinned kernel
    // sources: distinct TIDs are positive i32; suspended native activations
    // occupy distinct nonempty signal/continuation storage below 2^56 bytes.
    kani::assume(running <= i32::MAX as u32);
    kani::assume(suspended < (1usize << 56));
    let pending = suspended.checked_add(running as usize).unwrap();
    assert!(pending < count::MAX_COUNT);
    let raw = live.checked_add(pending).unwrap();
    assert!(raw.checked_add(1).is_some());
}

#[cfg(all(target_has_atomic = "ptr", jzap_small_counts))]
#[kani::proof]
fn reduced_model_guard_matches_every_bounded_raw_reservation() {
    let raw: usize = kani::any();
    // Independently stated model domain: seven admitted credits and fewer
    // than seven outstanding reservations produce raw words 1 through 13.
    kani::assume(raw >= 1 && raw <= 13);
    let strong = unsafe { count::AtomicCount::new(raw) };
    let result = strong.clone_strong();
    if raw < 7 {
        assert_eq!(result, Ok(()));
        assert_eq!(strong.load(), raw + 1);
    } else {
        assert_eq!(result, Err(Error::RefCount));
        assert_eq!(strong.load(), raw);
    }
}

#[cfg(target_has_atomic = "ptr")]
#[kani::proof]
#[kani::unwind(3)]
fn flagged_weak_clone_release_preserves_layout_and_lock_separation() {
    let credits: usize = kani::any();
    let retired: bool = kani::any();
    kani::assume(credits > 0 && credits <= count::WEAK_MAX_COUNT);
    let flag = if retired { count::RETIRED } else { 0 };
    let raw = flag | credits;
    let weak = unsafe { count::AtomicCount::new(raw) };
    let result = weak.clone_weak();
    if credits == count::WEAK_MAX_COUNT {
        assert_eq!(result, Err(Error::RefCount));
        assert_eq!(weak.load(), raw);
    } else {
        assert_eq!(result, Ok(()));
        assert_eq!(weak.load() & count::RETIRED, flag);
        assert_ne!(weak.load(), count::LOCK);
        assert!(!weak.release_weak());
        assert_eq!(weak.load(), raw);
    }
    assert_eq!(weak.release_weak(), credits == 1);
    assert_eq!(weak.load(), flag | (credits - 1));
}

#[cfg(target_has_atomic = "ptr")]
#[kani::proof]
#[kani::unwind(3)]
fn atomic_publication_and_retirement_distinguish_all_valid_layouts() {
    let bytes: usize = kani::any();
    kani::assume(bytes <= isize::MAX as usize);
    let strong = unsafe { count::AtomicCount::new(count::retired_state(bytes)) };
    let weak = unsafe { count::AtomicCount::new(count::AtomicCount::initial_weak(true)) };
    assert_eq!(strong.upgrade(&weak), Ok(false));
    strong.publish(&weak);
    assert_eq!(strong.load(), 1);
    assert_eq!(weak.load(), 1);
    assert_eq!(strong.upgrade(&weak), Ok(true));
    assert!(!strong.release());
    assert!(strong.release());
    strong.retire(&weak, bytes);
    assert_eq!(strong.upgrade(&weak), Ok(false));
    assert_eq!(count::retired_bytes(strong.load()), bytes);
    assert!(weak.release_weak());
}

#[cfg(target_has_atomic = "ptr")]
#[kani::proof]
#[kani::unwind(3)]
fn temporary_high_count_reports_refusal_without_resurrection_or_death() {
    let raw: usize = kani::any();
    kani::assume(raw > count::MAX_COUNT);
    let strong = unsafe { count::AtomicCount::new(raw) };
    let weak = unsafe { count::AtomicCount::new(1) };
    assert_eq!(strong.upgrade(&weak), Err(Error::RefCount));
    assert_eq!(strong.live_count(&weak), count::MAX_COUNT);
    assert_eq!(strong.load(), raw);
    assert_eq!(weak.load(), 1);
}
#[kani::proof]
fn count_admission_all_machine_words() {
    let old: usize = kani::any();
    let result = count::admitted_count(old);
    match result {
        Ok(next) => {
            assert!(old < count::MAX_COUNT);
            assert_eq!(next as u128, old as u128 + 1);
            assert!(next <= count::MAX_COUNT);
        }
        Err(_) => assert!(old >= count::MAX_COUNT),
    }
}
#[kani::proof]
fn local_count_admission_never_changes_on_failure() {
    let old: usize = kani::any();
    let live: bool = kani::any();
    kani::assume(old <= count::MAX_COUNT);
    let counter = unsafe { LocalCount::new(old) };
    let result = counter.increment(live);
    if live && old == 0 {
        assert_eq!(result, Ok(false));
        assert_eq!(counter.load(), old);
    } else if old == count::MAX_COUNT {
        assert_eq!(result, Err(Error::RefCount));
        assert_eq!(counter.load(), old);
    } else {
        assert_eq!(result, Ok(true));
        assert_eq!(counter.load() as u128, old as u128 + 1);
    }
}
#[kani::proof]
fn local_release_consumes_one_credit_without_wrap() {
    let old: usize = kani::any();
    kani::assume(old > 0 && old <= count::MAX_COUNT);
    let c = unsafe { LocalCount::new(old) };
    let last = c.release();
    assert_eq!(last, old == 1);
    assert_eq!(c.load() as u128 + 1, old as u128);
}
#[cfg(target_has_atomic = "ptr")]
#[kani::proof]
#[kani::unwind(3)]
fn atomic_admission_and_release_sequential_projection() {
    let old: usize = kani::any();
    let live: bool = kani::any();
    kani::assume(old <= count::MAX_COUNT);
    let c = unsafe { count::AtomicCount::new(old) };
    let result = c.increment(live);
    if live && old == 0 {
        assert_eq!(result, Ok(false));
        assert_eq!(c.load(), old);
    } else if old == count::MAX_COUNT {
        assert_eq!(result, Err(Error::RefCount));
        assert_eq!(c.load(), old);
    } else {
        assert_eq!(result, Ok(true));
        let last = c.release();
        assert_eq!(last, old == 0);
        assert_eq!(c.load(), old);
    }
}
#[kani::proof]
fn uniqueness_and_retirement_all_valid_counts() {
    let s: usize = kani::any();
    let w: usize = kani::any();
    kani::assume(s > 0 && s <= count::MAX_COUNT && w > 0 && w <= count::MAX_COUNT);
    let strong = unsafe { LocalCount::new(s) };
    let weak = unsafe { LocalCount::new(w) };
    assert_eq!(LocalCount::unique(&strong, &weak), s == 1 && w == 1);
    let last = strong.take_last();
    assert_eq!(last, s == 1);
    assert_eq!(strong.load(), if last { 0 } else { s });
}
fn check_control_layout<C: Counter, P: SharedPolicy>(value: Layout) {
    let Ok((layout, offset)) = allocation_layout::<C, P>(value) else {
        return;
    };
    assert!(offset >= core::mem::size_of::<Control<C, P, ()>>());
    assert_eq!(offset % value.align(), 0);
    assert!(layout.align() >= value.align());
    assert!(offset.checked_add(value.size()).unwrap() <= layout.size());
    assert!(layout.size() <= isize::MAX as usize);
}
#[kani::proof]
fn control_layout_bounds_all_machine_words() {
    let size: usize = kani::any();
    let align: usize = kani::any();
    let Ok(value) = Layout::from_size_align(size, align) else {
        return;
    };
    check_control_layout::<LocalCount, Unmetered>(value);
    check_control_layout::<LocalCount, Budget<'static>>(value);
    #[cfg(target_has_atomic = "ptr")]
    check_control_layout::<count::AtomicCount, crate::budget::ConcurrentBudget<'static>>(value);
}
#[kani::proof]
#[kani::unwind(5)]
fn actual_rc_weak_last_owner_releases_payload() {
    let value: u8 = kani::any();
    let owner = crate::rc::Rc::try_new(value);
    let Ok(owner) = owner else {
        return;
    };
    let Ok(weak) = crate::rc::Rc::try_downgrade(&owner) else {
        return;
    };
    let Ok(other) = crate::rc::Rc::try_clone(&owner) else {
        return;
    };
    drop(owner);
    assert_eq!(*other, value);
    drop(other);
    assert!(weak.try_upgrade().unwrap().is_none());
}
#[kani::proof]
#[kani::unwind(5)]
fn actual_rc_copy_on_write_preserves_alias() {
    let value: u8 = kani::any();
    let Ok(mut owner) = crate::rc::Rc::try_new(value) else {
        return;
    };
    let Ok(alias) = crate::rc::Rc::try_clone(&owner) else {
        return;
    };
    if let Ok(out) = crate::rc::Rc::try_make_mut(&mut owner) {
        *out = value.wrapping_add(1);
        assert_eq!(*alias, value);
    }
}
#[kani::proof]
#[kani::unwind(5)]
fn actual_rc_weak_only_detachment_keeps_dead_header() {
    let value: u8 = kani::any();
    let Ok(mut owner) = crate::rc::Rc::try_new(value) else {
        return;
    };
    let Ok(weak) = crate::rc::Rc::try_downgrade(&owner) else {
        return;
    };
    if crate::rc::Rc::try_make_mut(&mut owner).is_ok() {
        assert!(weak.try_upgrade().unwrap().is_none());
        assert_eq!(*owner, value);
    }
}

#[cfg(target_has_atomic = "ptr")]
#[kani::proof]
fn atomic_core_orderings_are_valid_for_all_counts() {
    let s: usize = kani::any();
    let w: usize = kani::any();
    kani::assume(s > 0 && s <= count::MAX_COUNT && w > 0 && w <= count::MAX_COUNT);
    let strong = unsafe { count::AtomicCount::new(s) };
    let weak = unsafe { count::AtomicCount::new(w) };
    assert_eq!(count::AtomicCount::unique(&strong, &weak), s == 1 && w == 1);
    assert_eq!(weak.load(), w);
    let initial = unsafe { count::AtomicCount::new(0) };
    initial.publish(&weak);
    assert_eq!(initial.load(), 1);
}

#[kani::proof]
#[kani::unwind(3)]
fn retired_word_preserves_every_valid_allocation_size() {
    let bytes: usize = kani::any();
    kani::assume(bytes <= isize::MAX as usize);
    let state = count::retired_state(bytes);
    assert_eq!(count::retired_bytes(state), bytes);
    assert_eq!(state as u128, bytes as u128 + count::RETIRED as u128);
    assert!(state > count::MAX_COUNT);
    let c = unsafe { LocalCount::new(state) };
    assert_eq!(c.increment(true), Ok(false));
    assert_eq!(c.load(), state);
    #[cfg(target_has_atomic = "ptr")]
    {
        let c = unsafe { count::AtomicCount::new(state) };
        let weak = unsafe { count::AtomicCount::new(count::AtomicCount::initial_weak(true)) };
        assert_eq!(c.upgrade(&weak), Ok(false));
        assert_eq!(c.load(), state);
    }
}
#[kani::proof]
#[kani::unwind(5)]
fn actual_retired_weak_refunds_the_original_layout() {
    let budget = crate::budget::MemoryBudget::new(4096);
    let value: u8 = kani::any();
    let Ok(a) = crate::rc::Rc::try_new_in(value, Budget::Limited(&budget)) else {
        return;
    };
    let bytes = budget.occupied();
    let Ok(w) = crate::rc::Rc::try_downgrade(&a) else {
        return;
    };
    drop(a);
    assert_eq!(w.strong_count(), 0);
    assert!(w.try_upgrade().unwrap().is_none());
    assert_eq!(budget.occupied(), bytes);
    drop(w);
    assert_eq!(budget.occupied(), 0);
}

#[kani::proof]
#[kani::unwind(6)]
fn actual_box_slice_transfer_preserves_metadata_and_new_provenance() {
    let values: [u8; 3] = kani::any();
    let Ok(source) = crate::boxed::Box::<[u8]>::try_copy_from_slice(&values) else {
        return;
    };
    let Ok(owner) = crate::rc::Rc::<[u8]>::try_from_box_in(source) else {
        return;
    };
    assert_eq!(owner.len(), 3);
    assert_eq!(owner.get(0), Some(&values[0]));
    assert_eq!(owner.get(2), Some(&values[2]));
}

#[kani::proof]
#[kani::unwind(6)]
fn actual_owned_cyclic_publication_and_failure_refund() {
    let value: u8 = kani::any();
    let fail: bool = kani::any();
    let budget = crate::budget::MemoryBudget::new(64);
    let mut escaped = None;
    let result = crate::rc::Rc::try_new_cyclic_in(Budget::Limited(&budget), |weak| {
        escaped = Some(weak.try_clone().map_err(|_| ())?);
        if fail { Err(()) } else { Ok(value) }
    });
    match result {
        Ok(owner) => {
            assert!(!fail);
            assert_eq!(*owner, value);
            let upgraded = escaped.as_ref().unwrap().try_upgrade().unwrap().unwrap();
            assert_eq!(*upgraded, value);
            drop(upgraded);
            drop(owner);
        }
        Err(_) => {
            if escaped.is_some() {
                assert!(fail);
            }
        }
    }
    if let Some(weak) = escaped {
        assert!(weak.try_upgrade().unwrap().is_none());
        assert!(budget.occupied() > 0);
        drop(weak);
    }
    assert_eq!(budget.occupied(), 0);
    assert_eq!(budget.reserved(), 0);
}
