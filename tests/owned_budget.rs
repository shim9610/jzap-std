//! Heap-ledger lifetime, failure, transfer and async-state boundaries.
mod common;
#[path = "common/deallocations.rs"]
mod deallocations;
use common::{FaultAllocator, track};
use deallocations::deallocations_during;
use jzap::{
    Error, TryClone,
    boxed::Box,
    budget::{AllocationPolicy, OwnedBudget},
    collections::BTreeMap,
    string::String,
    vec::Vec,
};
#[global_allocator]
static ALLOCATOR: FaultAllocator = FaultAllocator;

trait Observed: AllocationPolicy {
    fn used(&self) -> usize;
}
impl Observed for OwnedBudget {
    fn used(&self) -> usize {
        self.occupied()
    }
}
#[cfg(target_has_atomic = "ptr")]
impl Observed for jzap::budget::OwnedConcurrentBudget {
    fn used(&self) -> usize {
        self.used()
    }
}
fn owners<P: Observed>(policy: P) {
    let mut bytes = Vec::<u8, _>::new_in(policy.try_clone().unwrap());
    bytes.try_extend_copy(&[7; 60]).unwrap();
    bytes.try_reserve_exact(40).unwrap();
    let mut text =
        String::try_from_str_in("aé한", policy.try_clone().unwrap(), jzap::vec::Unlimited).unwrap();
    text.try_push_str("🦀").unwrap();
    let value = Box::try_new_in(23u64, policy.try_clone().unwrap()).unwrap();
    let mut tree = BTreeMap::new_in(policy.try_clone().unwrap());
    for k in 0u64..64 {
        tree.try_insert(k, k + 1).unwrap();
    }
    assert!(policy.used() > 0);
    drop(policy);
    assert_eq!(bytes.as_slice(), [7; 60]);
    assert_eq!(text.as_str(), "aé한🦀");
    assert_eq!(*value, 23);
    let copied = bytes.try_clone().unwrap();
    let tail = bytes.try_split_off(30).unwrap();
    let (ptr, len, cap, policy, limit) = copied.into_raw_parts();
    // SAFETY: reconstitute the exact transferred storage and policy once.
    let copied = unsafe { Vec::from_raw_parts(ptr, len, cap, policy, limit) };
    drop((value, text, tail, copied));
    assert_eq!(tree.into_iter().count(), 64);
    assert!(bytes.allocation_policy().used() > 0);
    bytes.clear();
    bytes.try_shrink_to_fit().unwrap();
    assert_eq!(bytes.allocation_policy().used(), 0);
}
#[test]
fn local_owners_outlive_creator_and_transfer_without_borrowed_lifetimes() {
    let (_, state) = track(usize::MAX, || owners(OwnedBudget::try_new(4096).unwrap()));
    assert_eq!(state.live, 0);
}
#[cfg(target_has_atomic = "ptr")]
#[test]
fn concurrent_owners_outlive_creator_and_transfer_without_borrowed_lifetimes() {
    let (_, state) = track(usize::MAX, || {
        owners(jzap::budget::OwnedConcurrentBudget::try_new(4096).unwrap())
    });
    assert_eq!(state.live, 0);
}
#[test]
fn bootstrap_failure_and_zero_payload_budget_are_explicit() {
    let (failed, state) = track(1, || OwnedBudget::try_new(0));
    assert!(matches!(failed, Err(Error::Allocation { .. })));
    assert_eq!((state.calls, state.live), (1, 0));
    let ((), state) = track(usize::MAX, || {
        let policy = OwnedBudget::try_new(0).unwrap();
        assert_eq!(
            (policy.limit(), policy.occupied(), policy.reserved()),
            (0, 0, 0)
        );
        let mut values = Vec::<u8, _>::new_in(policy);
        assert!(matches!(
            values.try_push(1).unwrap_err().error,
            Error::Budget { .. }
        ));
    });
    assert_eq!((state.calls, state.live), (1, 0));
    #[cfg(target_has_atomic = "ptr")]
    {
        let (failed, state) = track(1, || jzap::budget::OwnedConcurrentBudget::try_new(0));
        assert!(matches!(failed, Err(Error::Allocation { .. })));
        assert_eq!((state.calls, state.live), (1, 0));
    }
}
fn weak_retention<C: jzap::shared::count::Counter, P: Observed>(policy: P) {
    use jzap::shared::Strong;
    let value = Strong::<u64, C, P>::try_new_in(19, policy).unwrap();
    let raw = Strong::into_raw(value);
    let address = raw.as_ptr().addr();
    // SAFETY: restore the exact raw strong credit; its header owns the policy.
    let value = unsafe { Strong::from_raw(raw) };
    let weak = Strong::try_downgrade(&value).unwrap();
    let duplicate = weak.try_clone().unwrap();
    drop(value);
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    let (_, releases) = deallocations_during(address, || drop(duplicate));
    assert_eq!(releases, 1);
}
#[test]
fn last_weak_retains_the_only_local_domain_owner() {
    let (_, state) = track(usize::MAX, || {
        weak_retention::<jzap::shared::count::LocalCount, _>(OwnedBudget::try_new(4096).unwrap())
    });
    assert_eq!(state.live, 0);
}
#[cfg(target_has_atomic = "ptr")]
#[test]
fn last_weak_retains_the_only_concurrent_domain_owner() {
    let (_, state) = track(usize::MAX, || {
        weak_retention::<jzap::shared::count::AtomicCount, _>(
            jzap::budget::OwnedConcurrentBudget::try_new(4096).unwrap(),
        )
    });
    assert_eq!(state.live, 0);
}
#[test]
fn reservation_tokens_cancel_and_commit_against_the_owned_ledger() {
    let budget = OwnedBudget::try_new(8).unwrap();
    let reservation = budget.reserve(5).unwrap();
    assert_eq!(
        (budget.occupied(), budget.reserved(), budget.available()),
        (0, 5, 3)
    );
    drop(reservation);
    assert_eq!((budget.reserved(), budget.available()), (0, 8));
    let charge = budget.reserve(8).unwrap().commit();
    assert_eq!(budget.occupied(), 8);
    drop(charge);
    assert_eq!(budget.occupied(), 0);
}
#[cfg(target_has_atomic = "ptr")]
#[test]
fn owned_state_is_send_sync_static_and_survives_thread_transfer() {
    use jzap::budget::OwnedConcurrentBudget;
    struct State {
        data: Vec<u8, OwnedConcurrentBudget>,
    }
    fn require<T: Send + Sync + 'static>() {}
    require::<State>();
    fn build() -> State {
        State {
            data: Vec::new_in(OwnedConcurrentBudget::try_new(4096).unwrap()),
        }
    }
    let mut state = std::thread::spawn(|| {
        let mut state = build();
        state.data.try_extend_copy(&[1, 2, 3]).unwrap();
        state
    })
    .join()
    .unwrap();
    state.data.try_push(4).unwrap();
    assert_eq!(state.data.as_slice(), [1, 2, 3, 4]);
    let future = async move { state.data.len() };
    fn require_future<F: core::future::Future + Send + 'static>(_: F) {}
    require_future(future);
}

fn conversions_and_cow<C: jzap::shared::count::Counter, P: Observed>(policy: P) {
    use jzap::shared::Strong;
    let input = Box::try_new_in(23u64, policy).unwrap();
    let mut value = Strong::<u64, C, P>::try_from_box_in(input).unwrap();
    let weak = Strong::try_downgrade(&value).unwrap();
    *Strong::try_make_mut(&mut value).unwrap() = 29;
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
    let shared = value.try_clone().unwrap();
    *Strong::try_make_mut(&mut value).unwrap() = 31;
    assert_eq!((*shared, *value), (29, 31));
    drop((shared, value));
}
#[test]
fn shared_conversion_and_both_cow_paths_keep_the_sole_domain_alive() {
    let (_, state) = track(usize::MAX, || {
        conversions_and_cow::<jzap::shared::count::LocalCount, _>(
            OwnedBudget::try_new(4096).unwrap(),
        );
        #[cfg(target_has_atomic = "ptr")]
        conversions_and_cow::<jzap::shared::count::AtomicCount, _>(
            jzap::budget::OwnedConcurrentBudget::try_new(4096).unwrap(),
        );
    });
    assert_eq!(state.live, 0);
}

#[test]
fn documented_bootstrap_and_handle_sizes_are_observable() {
    let width = std::mem::size_of::<usize>();
    assert_eq!(std::mem::size_of::<OwnedBudget>(), width);
    assert_eq!(std::mem::size_of::<Box<u64, OwnedBudget>>(), 2 * width);
    let (budget, state) = track(usize::MAX, || OwnedBudget::try_new(0).unwrap());
    assert_eq!((state.calls, state.live), (1, (5 * width) as isize));
    drop(budget);
    #[cfg(target_has_atomic = "ptr")]
    {
        use jzap::budget::OwnedConcurrentBudget;
        assert_eq!(std::mem::size_of::<OwnedConcurrentBudget>(), width);
        let (budget, state) = track(usize::MAX, || OwnedConcurrentBudget::try_new(0).unwrap());
        assert_eq!((state.calls, state.live), (1, (4 * width) as isize));
        drop(budget);
    }
}

fn initializer_failure<C: jzap::shared::count::Counter, P: Observed>(policy: P) {
    let failed = jzap::shared::Strong::<u64, C, P>::try_init_in(
        policy,
        |slot: &mut jzap::init::InitSlot<'_, u64>| {
            slot.write(13).unwrap();
            Err::<(), _>(7u8)
        },
    );
    assert!(matches!(failed, Err(jzap::error::InitError::Initialize(7))));
}
fn cyclic_failure<C: jzap::shared::count::Counter, P: Observed>(policy: P) {
    let mut escaped = None;
    let failed = jzap::shared::Strong::<u64, C, P>::try_new_cyclic_in(policy, |weak| {
        escaped = Some(weak.try_clone().unwrap());
        Err::<u64, _>(7u8)
    });
    assert!(matches!(failed, Err(jzap::error::InitError::Initialize(7))));
    let weak = escaped.unwrap();
    assert!(weak.try_upgrade().unwrap().is_none());
    drop(weak);
}
#[test]
fn initialization_failure_and_escaped_weak_keep_real_heap_ledgers_valid() {
    use jzap::shared::count::LocalCount;
    let (_, state) = track(usize::MAX, || {
        initializer_failure::<LocalCount, _>(OwnedBudget::try_new(4096).unwrap());
        cyclic_failure::<LocalCount, _>(OwnedBudget::try_new(4096).unwrap());
        #[cfg(target_has_atomic = "ptr")]
        {
            use jzap::{budget::OwnedConcurrentBudget, shared::count::AtomicCount};
            initializer_failure::<AtomicCount, _>(OwnedConcurrentBudget::try_new(4096).unwrap());
            cyclic_failure::<AtomicCount, _>(OwnedConcurrentBudget::try_new(4096).unwrap());
        }
    });
    assert_eq!(state.live, 0);
}
