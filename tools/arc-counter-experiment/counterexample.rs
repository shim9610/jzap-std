//! Run separately: this must FAIL for the current performance-only experiment.
use jzap::{Error, shared::count::MAX_COUNT, sync::Arc};
use loom::{model,sync::Arc as ModelArc,thread};

#[test]
fn saturated_live_arc_upgrade_must_not_report_dead() {
    model(|| {
        let root=ModelArc::new(Arc::try_new(19u64).unwrap());
        let owners:Vec<_>=(1..MAX_COUNT).map(|_|Arc::try_clone(&root).unwrap()).collect();
        let weak=Arc::try_downgrade(&root).unwrap();
        let other=root.clone();
        let worker=thread::spawn(move || {
            assert!(matches!(Arc::try_clone(&other),Err(Error::RefCount)));
        });
        // All MAX_COUNT strong owners remain live throughout this check.
        assert!(matches!(weak.try_upgrade(),Err(Error::RefCount)),
                "temporary overflow incorrectly reported a live Arc as dead");
        worker.join().unwrap();
        drop(owners);
    });
}
