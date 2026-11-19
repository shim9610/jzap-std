//! Tests for the generated experiment, including explicit non-production controls.
use jzap::{Error, shared::count::{AtomicCount, Counter, MAX_COUNT}};
use std::sync::atomic::{AtomicUsize,Ordering};

#[test]
fn normal_credit_and_saturation_rollback() {
    // These standalone numeric counters begin with the stated model credits;
    // no allocation or payload destructor is associated with their test values.
    let normal = unsafe {AtomicCount::new(1)};
    normal.clone_owned_strong().unwrap();
    assert_eq!(normal.load(),2);
    assert!(!normal.release());
    assert_eq!(normal.load(),1);
    let full = unsafe {AtomicCount::new(MAX_COUNT)};
    for _ in 0..8 {
        assert_eq!(full.clone_owned_strong(),Err(Error::RefCount));
        assert_eq!(full.load(),MAX_COUNT);
    }
}

#[test]
fn concurrent_refused_and_successful_credits_restore_the_initial_count() {
    for initial in [1,MAX_COUNT-1,MAX_COUNT] {
        let counter=unsafe {AtomicCount::new(initial)};
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let counter=&counter;
                scope.spawn(move || {
                    for _ in 0..if cfg!(miri) {8} else {10000} {
                        if counter.clone_owned_strong().is_ok() {assert!(!counter.release());}
                    }
                });
            }
        });
        assert_eq!(counter.load(),initial);
    }
}

#[test]
fn known_retired_tag_conflict_is_exposed_before_rollback() {
    // Reproduce the exact intermediate word after fetch_add at the soft limit.
    // An unchanged Weak upgrade classifies this live overflow reservation as dead.
    let raw=AtomicUsize::new(MAX_COUNT);
    let old=raw.fetch_add(1,Ordering::Relaxed);
    assert_eq!(old,MAX_COUNT);
    let intermediate=unsafe {AtomicCount::new(raw.load(Ordering::Relaxed))};
    assert_eq!(intermediate.increment(true),Ok(false));
    raw.fetch_sub(1,Ordering::Release);
    assert_eq!(raw.load(Ordering::Relaxed),MAX_COUNT);
}

#[test]
fn known_exhausted_four_bit_headroom_reopens_the_success_range() {
    let mut raw=7u8;
    let mut outcomes=Vec::new();
    for _ in 0..10 {
        // Ten callers paused after increment and before checking/rolling back.
        raw=raw.wrapping_add(1)&15;
        outcomes.push(raw&8==0);
    }
    assert!(outcomes[..8].iter().all(|&ok|!ok));
    assert!(outcomes[8] && outcomes[9]);
    assert_eq!(raw,1);
}
