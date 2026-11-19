
#[test]
fn headroom_two_refusals_restore_saturated_word() {
    use jzap::shared::count::{AtomicCount,Counter,MAX_COUNT};
    model(|| {
        let c=LArc::new(unsafe {AtomicCount::new(MAX_COUNT)});
        let other=c.clone();
        let t=thread::spawn(move || assert_eq!(other.clone_owned_strong(),Err(Error::RefCount)));
        assert_eq!(c.clone_owned_strong(),Err(Error::RefCount));
        t.join().unwrap();
        assert_eq!(c.load(),MAX_COUNT);
    });
}

#[test]
fn headroom_admission_release_and_rollback_conserve_credits() {
    use jzap::shared::count::{AtomicCount,Counter,MAX_COUNT};
    model(|| {
        let c=LArc::new(unsafe {AtomicCount::new(MAX_COUNT-1)});
        let other=c.clone();
        let t=thread::spawn(move || {
            if other.clone_owned_strong().is_ok() {assert!(!other.release());}
        });
        if c.clone_owned_strong().is_ok() {assert!(!c.release());}
        t.join().unwrap();
        assert_eq!(c.load(),MAX_COUNT-1);
    });
}
