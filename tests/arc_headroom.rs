//! Numeric state and representation regression checks for Arc's two counters.
use jzap::{
    Error,
    shared::count::{AtomicCount, Counter, MAX_COUNT, RETIRED, WEAK_MAX_COUNT},
};

#[test]
fn strong_guard_refusal_restores_the_exact_current_count() {
    for old in [
        1,
        MAX_COUNT.saturating_sub(1),
        MAX_COUNT,
        MAX_COUNT.saturating_add(3),
    ] {
        let strong = unsafe { AtomicCount::new(old) };
        if old < MAX_COUNT {
            assert_eq!(strong.clone_strong(), Ok(()));
            assert_eq!(strong.load(), old.checked_add(1).unwrap());
            assert!(!strong.release());
        } else {
            assert_eq!(strong.clone_strong(), Err(Error::RefCount));
        }
        assert_eq!(strong.load(), old);
    }
}

#[test]
fn temporary_saturation_is_live_and_upgrade_reports_refcount() {
    let provisional = unsafe { AtomicCount::new(MAX_COUNT.saturating_add(1)) };
    let weak = unsafe { AtomicCount::new(1) };
    assert_eq!(provisional.upgrade(&weak), Err(Error::RefCount));
    assert_eq!(provisional.live_count(&weak), MAX_COUNT);
    assert_eq!(provisional.load(), MAX_COUNT.saturating_add(1));
}

#[test]
fn zero_and_published_retirement_are_dead() {
    let strong = unsafe { AtomicCount::new(0) };
    let weak = unsafe { AtomicCount::new(1) };
    assert_eq!(strong.upgrade(&weak), Ok(false));
    strong.retire(&weak, 128);
    assert_eq!(strong.upgrade(&weak), Ok(false));
    assert_eq!(strong.live_count(&weak), 0);
    assert_eq!(weak.load(), RETIRED | 1);
    weak.clone_weak().unwrap();
    assert_eq!(weak.load(), RETIRED | 2);
    assert!(!weak.release_weak());
    assert!(weak.release_weak());
}

#[test]
fn cyclic_publication_clears_metadata_flag_before_exposing_payload() {
    let strong = unsafe { AtomicCount::new(RETIRED | 128) };
    let weak = unsafe { AtomicCount::new(AtomicCount::initial_weak(true)) };
    weak.clone_weak().unwrap();
    assert_eq!(strong.upgrade(&weak), Ok(false));
    strong.publish(&weak);
    assert_eq!(weak.load(), 2);
    assert_eq!(strong.live_count(&weak), 1);
    assert_eq!(strong.upgrade(&weak), Ok(true));
    assert_eq!(strong.load(), 2);
}

#[test]
fn weak_saturation_keeps_the_retired_flag_and_never_collides_with_lock() {
    for flag in [0, RETIRED] {
        let weak = unsafe { AtomicCount::new(flag | WEAK_MAX_COUNT) };
        assert_ne!(weak.load(), usize::MAX);
        assert_eq!(weak.clone_weak(), Err(Error::RefCount));
        assert_eq!(weak.load(), flag | WEAK_MAX_COUNT);
    }
}
