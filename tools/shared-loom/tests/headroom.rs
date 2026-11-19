//! Regression: saturated live Arc values must never be classified as dead.
use jzap::{Error, TryClone, budget::OwnedConcurrentBudget, shared::count::MAX_COUNT, sync::Arc};
use loom::{model, sync::Arc as ModelArc, thread};

#[test]
fn saturated_live_arc_upgrade_must_not_report_dead() {
    model(|| {
        let root = ModelArc::new(Arc::try_new(19u64).unwrap());
        let owners: Vec<_> = (1..MAX_COUNT)
            .map(|_| Arc::try_clone(&root).unwrap())
            .collect();
        let weak = Arc::try_downgrade(&root).unwrap();
        let other = root.clone();
        let worker = thread::spawn(move || {
            assert!(matches!(Arc::try_clone(&other), Err(Error::RefCount)));
        });
        // All MAX_COUNT strong owners remain live throughout this check.
        assert!(
            matches!(weak.try_upgrade(), Err(Error::RefCount)),
            "temporary overflow incorrectly reported a live Arc as dead"
        );
        worker.join().unwrap();
        drop(owners);
    });
}

#[test]
fn simultaneous_rejected_clones_preserve_every_existing_owner() {
    model(|| {
        let root = ModelArc::new(Arc::try_new(23u64).unwrap());
        let owners: Vec<_> = (1..MAX_COUNT)
            .map(|_| Arc::try_clone(&root).unwrap())
            .collect();
        let other = root.clone();
        let worker = thread::spawn(move || {
            assert!(matches!(Arc::try_clone(&other), Err(Error::RefCount)));
            assert_eq!(**other, 23);
        });
        assert!(matches!(Arc::try_clone(&root), Err(Error::RefCount)));
        worker.join().unwrap();
        assert_eq!(Arc::strong_count(&root), MAX_COUNT);
        drop(owners);
        assert_eq!(Arc::strong_count(&root), 1);
        assert_eq!(
            Arc::try_unwrap(ModelArc::try_unwrap(root).unwrap()).unwrap(),
            23
        );
    });
}

#[test]
fn retired_flag_and_weak_clone_race_refund_the_same_domain_once() {
    model(|| {
        let budget = OwnedConcurrentBudget::try_new(1024).unwrap();
        let owner = Arc::try_new_in(29u64, budget.try_clone().unwrap()).unwrap();
        let weak = Arc::try_downgrade(&owner).unwrap();
        let other = weak.try_clone().unwrap();
        let bytes = budget.used();
        let worker = thread::spawn(move || {
            let cloned = other.try_clone().unwrap();
            if let Some(value) = cloned.try_upgrade().unwrap() {
                assert_eq!(*value, 29);
            }
        });
        drop(owner);
        worker.join().unwrap();
        assert!(weak.try_upgrade().unwrap().is_none());
        assert_eq!(budget.used(), bytes);
        drop(weak);
        assert_eq!(budget.used(), 0);
    });
}

fn metered_cyclic_publication(fail: bool) {
    model(move || {
        let budget = OwnedConcurrentBudget::try_new(1024).unwrap();
        let mut worker = None;
        let mut escaped = None;
        let result = Arc::try_new_cyclic_in(budget.try_clone().unwrap(), |weak| {
            escaped = Some(weak.try_clone().unwrap());
            let other = weak.try_clone().unwrap();
            worker = Some(thread::spawn(move || {
                let cloned = other.try_clone().unwrap();
                if let Some(value) = cloned.try_upgrade().unwrap() {
                    assert!(!fail);
                    assert_eq!(*value, 31u64);
                }
            }));
            if fail { Err(()) } else { Ok(31u64) }
        });
        if let Ok(owner) = result {
            assert!(!fail);
            assert_eq!(*owner, 31);
            drop(owner);
        }
        worker.take().unwrap().join().unwrap();
        let weak = escaped.take().unwrap();
        assert!(weak.try_upgrade().unwrap().is_none());
        assert!(budget.used() > 0);
        drop(weak);
        assert_eq!(budget.used(), 0);
    });
}

#[test]
fn metered_cyclic_initialization_publishes_flag_and_payload_in_order() {
    metered_cyclic_publication(false);
}

#[test]
fn metered_cyclic_failure_keeps_escaped_weaks_dead_until_refund() {
    metered_cyclic_publication(true);
}

#[test]
fn weak_created_by_another_strong_owner_survives_final_retirement() {
    model(|| {
        let budget = OwnedConcurrentBudget::try_new(1024).unwrap();
        let owner = Arc::try_new_in(37u64, budget.try_clone().unwrap()).unwrap();
        let other = owner.try_clone().unwrap();
        let bytes = budget.used();
        let worker = thread::spawn(move || {
            let weak = Arc::try_downgrade(&other).unwrap();
            drop(other);
            weak
        });
        drop(owner);
        let weak = worker.join().unwrap();
        if budget.used() != bytes {
            // A bad count overwrite may already have freed the header. Fail
            // without dereferencing or dropping that deliberately bad oracle.
            core::mem::forget(weak);
            panic!("retirement discarded a surviving weak credit");
        }
        assert!(weak.try_upgrade().unwrap().is_none());
        drop(weak);
        assert_eq!(budget.used(), 0);
    });
}
