//! Non-Copy policy oracles exercise actual container ownership and cleanup.
use super::{Accounting, AllocationPolicy, MemoryBudget};
use crate::{Error, TryClone, boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use core::cell::Cell;

#[derive(Debug)]
struct State {
    budget: MemoryBudget,
    owners: Cell<usize>,
    attempts: Cell<usize>,
    fail_at: Cell<Option<usize>>,
}
impl State {
    fn new() -> Self {
        Self {
            budget: MemoryBudget::new(usize::MAX),
            owners: Cell::new(0),
            attempts: Cell::new(0),
            fail_at: Cell::new(None),
        }
    }
    fn policy(&self) -> Policy<'_> {
        self.owners.set(self.owners.get() + 1);
        Policy(self)
    }
    fn reject(&self, at: Option<usize>) {
        self.attempts.set(0);
        self.fail_at.set(at);
    }
    fn closed(&self) {
        assert_eq!(
            (
                self.owners.get(),
                self.budget.occupied(),
                self.budget.reserved()
            ),
            (0, 0, 0)
        );
    }
}
#[derive(Debug)]
struct Policy<'a>(&'a State);
impl super::sealed::Sealed for Policy<'_> {}
impl AllocationPolicy for Policy<'_> {
    fn budget(&self) -> Accounting<'_> {
        Accounting::Local(&self.0.budget)
    }
}
impl TryClone for Policy<'_> {
    fn try_clone(&self) -> Result<Self, Error> {
        let attempt = self.0.attempts.get();
        self.0.attempts.set(attempt + 1);
        if self.0.fail_at.get() == Some(attempt) {
            return Err(Error::RefCount);
        }
        Ok(self.0.policy())
    }
}
impl Drop for Policy<'_> {
    fn drop(&mut self) {
        let owners = self.0.owners.get();
        assert!(owners > 0, "policy dropped twice");
        if owners == 1 {
            assert_eq!(
                self.0.budget.occupied(),
                0,
                "last policy dropped before refund"
            );
            assert_eq!(
                self.0.budget.reserved(),
                0,
                "last policy dropped before cancellation"
            );
        }
        self.0.owners.set(owners - 1);
    }
}

#[test]
fn vec_and_box_duplication_refusals_preserve_owners_and_data() {
    let state = State::new();
    let mut source = Vec::new_in(state.policy());
    source.try_extend_copy(&[1u64, 2, 3]).unwrap();
    let used = state.budget.occupied();
    state.reject(Some(0));
    assert_eq!(source.try_clone().unwrap_err(), Error::RefCount);
    state.reject(Some(0));
    assert_eq!(source.try_split_off(1).unwrap_err(), Error::RefCount);
    assert_eq!(source.as_slice(), [1, 2, 3]);
    assert_eq!((state.owners.get(), state.budget.occupied()), (1, used));
    let mut destination = Vec::new_in(state.policy());
    destination.try_push(9).unwrap();
    let used = state.budget.occupied();
    state.reject(Some(0));
    assert_eq!(destination.try_clone_from(&source), Err(Error::RefCount));
    assert_eq!(destination.as_slice(), [9]);
    assert_eq!(source.as_slice(), [1, 2, 3]);
    assert_eq!(state.budget.occupied(), used);
    drop((source, destination));
    let owner = Box::try_new_in(17u64, state.policy()).unwrap();
    state.reject(Some(0));
    assert_eq!(owner.try_clone().unwrap_err(), Error::RefCount);
    assert_eq!(*owner, 17);
    drop(owner);
    state.closed();
}

#[test]
fn consuming_and_standard_transfers_move_one_policy() {
    let state = State::new();
    let owner = Box::try_new_in(11u64, state.policy()).unwrap();
    state.reject(Some(0));
    let (ptr, policy) = Box::into_raw_parts(owner);
    assert_eq!(state.owners.get(), 1);
    // SAFETY: recover the same pointer, payload and transferred policy credit.
    let owner = unsafe { Box::from_raw_parts(ptr, policy) };
    assert_eq!(Box::into_inner(owner), 11);
    state.closed();
    let owner = Box::try_from_std_in(alloc::boxed::Box::new(13u64), state.policy()).unwrap();
    assert_eq!(Box::into_inner(owner), 13);
    state.closed();
    let source = alloc::vec![2u64, 3, 5];
    let owner = Box::<[u64], _>::try_from_std_vec_in(source, state.policy()).unwrap();
    let owner = Vec::from_boxed_slice(owner);
    assert_eq!(owner.into_std(), [2, 3, 5]);
    state.closed();
    let owner =
        Vec::try_from_std_in(alloc::vec![7u64, 9], state.policy(), crate::vec::Unlimited).unwrap();
    let owner = owner.try_into_boxed_slice().unwrap();
    assert_eq!(owner.as_slice(), [7, 9]);
    drop(owner);
    assert_eq!(state.attempts.get(), 0);
    state.closed();
}

fn shared_lifecycle<C: crate::shared::count::Counter>() {
    use crate::shared::Strong;
    let state = State::new();
    let first = Strong::<u64, C, _>::try_new_in(17, state.policy()).unwrap();
    let mut second = Strong::try_clone(&first).unwrap();
    let weak = Strong::try_downgrade(&first).unwrap();
    assert_eq!(state.owners.get(), 1);
    state.reject(Some(0));
    assert_eq!(
        Strong::try_make_mut(&mut second).unwrap_err(),
        Error::RefCount
    );
    assert_eq!((*first, *second), (17, 17));
    assert_eq!(Strong::strong_count(&first), 2);
    drop((first, second));
    assert_eq!(state.owners.get(), 1);
    assert!(state.budget.occupied() > 0);
    state.reject(Some(0));
    drop(weak);
    assert_eq!(state.attempts.get(), 0);
    state.closed();
}
#[test]
fn rc_header_retains_policy_through_last_weak() {
    shared_lifecycle::<crate::shared::count::LocalCount>();
}
#[cfg(target_has_atomic = "ptr")]
#[test]
fn arc_header_retains_policy_through_last_weak() {
    shared_lifecycle::<crate::shared::count::AtomicCount>();
}

#[test]
fn failed_initialization_and_escaped_weak_release_policy_after_refund() {
    use crate::{error::InitError, rc::Rc};
    let state = State::new();
    let failed = Box::<u64, _>::try_init_in(
        state.policy(),
        |slot: &mut crate::init::InitSlot<'_, u64>| {
            slot.write(19).unwrap();
            Err::<(), _>(7u8)
        },
    );
    assert!(matches!(failed, Err(InitError::Initialize(7))));
    state.closed();
    let failed = Rc::<u64, _>::try_init_in(
        state.policy(),
        |slot: &mut crate::init::InitSlot<'_, u64>| {
            slot.write(23).unwrap();
            Err::<(), _>(9u8)
        },
    );
    assert!(matches!(failed, Err(InitError::Initialize(9))));
    state.closed();
    let mut escaped = None;
    let failed = Rc::<u64, _>::try_new_cyclic_in(state.policy(), |weak| {
        escaped = Some(weak.try_clone().unwrap());
        Err::<u64, _>(11u8)
    });
    assert!(matches!(failed, Err(InitError::Initialize(11))));
    assert_eq!(state.owners.get(), 1);
    assert!(state.budget.occupied() > 0);
    drop(escaped);
    state.closed();
}

#[test]
fn shared_conversions_reject_before_consuming_input_policy() {
    use crate::rc::Rc;
    let state = State::new();
    let owner = Box::try_new_in(29u64, state.policy()).unwrap();
    let address = Box::as_ptr(&owner);
    let used = state.budget.occupied();
    state.reject(Some(0));
    let rejected = Rc::try_from_box_in(owner).unwrap_err();
    assert_eq!(rejected.error, Error::RefCount);
    assert_eq!(Box::as_ptr(&rejected.value), address);
    assert_eq!((state.owners.get(), state.budget.occupied()), (1, used));
    state.reject(None);
    drop(Rc::try_from_box_in(rejected.value).unwrap());
    state.closed();
    let owner = String::try_from_str_in("é", state.policy(), crate::vec::Unlimited).unwrap();
    state.reject(Some(0));
    let rejected = Rc::<str, _>::try_from_string_in(owner).unwrap_err();
    assert_eq!(rejected.error, Error::RefCount);
    assert_eq!(rejected.value.as_str(), "é");
    state.reject(None);
    drop(Rc::<str, _>::try_from_string_in(rejected.value).unwrap());
    state.closed();
    let owner =
        Vec::try_from_slice_in(&[31u64, 37], state.policy(), crate::vec::Unlimited).unwrap();
    state.reject(Some(0));
    let rejected = Rc::<[u64], _>::try_from_vec_in(owner).unwrap_err();
    assert_eq!(rejected.error, Error::RefCount);
    assert_eq!(rejected.value.as_slice(), [31, 37]);
    state.reject(None);
    drop(Rc::<[u64], _>::try_from_vec_in(rejected.value).unwrap());
    state.closed();
}

#[test]
fn append_rejects_every_policy_copy_before_mutating_either_tree() {
    fn run(fail_at: Option<usize>) -> usize {
        let state = State::new();
        let (mut left, mut right) = (
            BTreeMap::new_in(state.policy()),
            BTreeMap::new_in(state.policy()),
        );
        for k in 0u64..24 {
            left.try_insert(k, k).unwrap();
        }
        for k in 12u64..36 {
            right.try_insert(k, k + 1).unwrap();
        }
        let a = left
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<alloc::vec::Vec<_>>();
        let b = right
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<alloc::vec::Vec<_>>();
        let used = state.budget.occupied();
        state.reject(fail_at);
        let result = left.try_append(&mut right);
        let attempts = state.attempts.get();
        if fail_at.is_some() {
            assert_eq!(result, Err(Error::RefCount));
            assert_eq!(
                left.iter()
                    .map(|(k, v)| (*k, *v))
                    .collect::<alloc::vec::Vec<_>>(),
                a
            );
            assert_eq!(
                right
                    .iter()
                    .map(|(k, v)| (*k, *v))
                    .collect::<alloc::vec::Vec<_>>(),
                b
            );
            assert_eq!((state.owners.get(), state.budget.occupied()), (2, used));
        } else {
            result.unwrap();
            assert!(right.is_empty());
            assert_eq!(left.len(), 36);
            assert_eq!(state.owners.get(), 2);
        }
        drop((left, right));
        state.closed();
        attempts
    }
    let copies = run(None);
    assert!(copies > 0);
    for fail_at in 0..copies {
        run(Some(fail_at));
    }
}

#[test]
fn tree_insert_refusal_and_iterator_drop_need_no_fallible_cleanup() {
    #[derive(Debug)]
    struct Item<'a>(&'a Cell<usize>);
    impl Drop for Item<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let state = State::new();
    let drops = Cell::new(0);
    let mut map = BTreeMap::new_in(state.policy());
    state.reject(Some(0));
    let rejected = map.try_insert(0, 41u64).unwrap_err();
    assert_eq!((rejected.error, rejected.value), (Error::RefCount, (0, 41)));
    assert!(map.is_empty());
    assert_eq!(state.budget.occupied(), 0);
    drop(map);
    state.closed();
    state.reject(None);
    let mut map = BTreeMap::new_in(state.policy());
    for k in 0..40 {
        map.try_insert(k, Item(&drops)).unwrap();
    }
    state.reject(Some(0));
    let mut iter = map.into_iter();
    drop(iter.next());
    drop(iter.next_back());
    drop(iter);
    assert_eq!(drops.get(), 40);
    assert_eq!(state.attempts.get(), 0);
    state.closed();
}

#[test]
fn splice_policy_refusal_preserves_committed_prefix_and_old_tail() {
    let state = State::new();
    let mut value = Vec::new_in(state.policy());
    value.try_extend_copy(&[1u64, 2, 3, 4]).unwrap();
    state.reject(Some(0));
    let result = value
        .try_splice(1..2, (10u64..30).filter(|_| true))
        .unwrap()
        .finish();
    assert_eq!(result, Err(Error::RefCount));
    let data = value.as_slice();
    assert_eq!(data[0], 1);
    assert!(data.ends_with(&[3, 4]));
    let prefix = &data[1..data.len() - 2];
    assert!(!prefix.is_empty());
    assert!(prefix.iter().copied().eq(10..10 + prefix.len() as u64));
    assert_eq!(state.owners.get(), 1);
    drop(value);
    state.closed();
}

#[test]
fn empty_destination_append_and_scoped_borrows_need_no_owned_copy() {
    let state = State::new();
    let mut left = BTreeMap::new_in(state.policy());
    let mut right = BTreeMap::new_in(state.policy());
    right.try_insert(3u64, 7u64).unwrap();
    state.reject(Some(0));
    left.try_append(&mut right).unwrap();
    assert_eq!(left.get(&3), Some(&7));
    assert!(right.is_empty());
    assert_eq!(state.attempts.get(), 0);
    drop((left, right));
    state.closed();
    let policy = state.policy();
    state.reject(Some(0));
    let mut value = Vec::new_in(&policy);
    value.try_push(41u64).unwrap();
    let copy = value.try_clone().unwrap();
    assert_eq!(copy.as_slice(), [41]);
    assert_eq!((state.owners.get(), state.attempts.get()), (1, 0));
    drop((value, copy));
    drop(policy);
    state.closed();
}

#[test]
fn deliberate_leaks_retain_their_policy_until_raw_recovery() {
    use core::ptr::NonNull;
    let state = State::new();
    let owner = Box::try_new_in(43u64, state.policy()).unwrap();
    let value = Box::leak(owner);
    assert_eq!(*value, 43);
    assert_eq!(state.owners.get(), 1);
    // SAFETY: reclaim the leaked pointer and the one deliberately forgotten
    // policy credit; Policy(&state) does not acquire another credit here.
    let owner = unsafe { Box::from_raw_parts(NonNull::from(value), Policy(&state)) };
    drop(owner);
    state.closed();
    let mut owner = Vec::new_in(state.policy());
    owner.try_extend_copy(&[47u64, 53]).unwrap();
    // Recovery from the returned slice pointer requires that its borrow covers
    // the entire allocation, rather than only a prefix with spare capacity.
    owner.try_shrink_to_fit().unwrap();
    let capacity = owner.capacity();
    let values = owner.leak();
    let len = values.len();
    let ptr = NonNull::new(values.as_mut_ptr()).unwrap();
    assert_eq!(state.owners.get(), 1);
    // SAFETY: exact leaked initialized range/capacity, and its original charge.
    let owner =
        unsafe { Vec::from_raw_parts(ptr, len, capacity, Policy(&state), crate::vec::Unlimited) };
    drop(owner);
    state.closed();
}

#[cfg(feature = "std")]
#[test]
fn payload_unwind_refunds_storage_before_final_policy_drop() {
    #[derive(Debug)]
    struct Payload<'a>(&'a Cell<bool>);
    impl Drop for Payload<'_> {
        fn drop(&mut self) {
            if !self.0.replace(true) {
                panic!("one payload destructor failure");
            }
        }
    }
    for kind in 0..5 {
        let state = State::new();
        let panicked = Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match kind {
            0 => drop(Box::try_new_in(Payload(&panicked), state.policy()).unwrap()),
            1 => {
                let mut owner = Vec::new_in(state.policy());
                for _ in 0..16 {
                    owner.try_push(Payload(&panicked)).unwrap();
                }
                drop(owner);
            }
            2 => drop(crate::rc::Rc::try_new_in(Payload(&panicked), state.policy()).unwrap()),
            #[cfg(target_has_atomic = "ptr")]
            3 => drop(crate::sync::Arc::try_new_in(Payload(&panicked), state.policy()).unwrap()),
            _ => {
                let mut owner = BTreeMap::new_in(state.policy());
                for key in 0..64 {
                    owner.try_insert(key, Payload(&panicked)).unwrap();
                }
                state.reject(Some(0));
                drop(owner.into_iter());
                assert_eq!(state.attempts.get(), 0);
            }
        }));
        assert!(result.is_err());
        assert!(panicked.get());
        state.closed();
    }
}

#[test]
fn constructor_budget_refusal_and_missing_write_release_consumed_policy() {
    let state = State {
        budget: MemoryBudget::new(0),
        ..State::new()
    };
    let rejected = Box::try_new_in(23u64, state.policy()).unwrap_err();
    assert_eq!(rejected.value, 23);
    assert!(matches!(rejected.error, Error::Budget { .. }));
    state.closed();
    let state = State::new();
    let result = crate::rc::Rc::<u64, _>::try_init_in(
        state.policy(),
        |_: &mut crate::init::InitSlot<'_, u64>| Ok::<(), u8>(()),
    );
    assert!(matches!(
        result,
        Err(crate::error::InitError::Storage(Error::Uninitialized))
    ));
    state.closed();
}

#[test]
fn tree_append_transfers_between_noncopy_policy_domains_after_all_admissions() {
    for empty in [false, true] {
        for refusal in 0..6 {
            let left_state = State::new();
            let right_state = State::new();
            let mut left = BTreeMap::new_in(left_state.policy());
            let mut right = BTreeMap::new_in(right_state.policy());
            if !empty {
                for k in 0u64..24 {
                    left.try_insert(k, k).unwrap();
                }
            }
            for k in 12u64..36 {
                right.try_insert(k, k + 1).unwrap();
            }
            let old = (left_state.budget.occupied(), right_state.budget.occupied());
            left_state.reject(if refusal < 4 { Some(refusal) } else { None });
            right_state.reject(if refusal == 4 { Some(0) } else { None });
            let result = left.try_append(&mut right);
            if !empty && refusal < 5 {
                assert_eq!(result, Err(Error::RefCount));
                assert_eq!(left.len(), 24);
                assert_eq!(right.len(), 24);
                for k in 0..24 {
                    assert_eq!(left.get(&k), Some(&k));
                }
                for k in 12..36 {
                    assert_eq!(right.get(&k), Some(&(k + 1)));
                }
                assert_eq!(
                    (left_state.budget.occupied(), right_state.budget.occupied()),
                    old
                );
            } else {
                result.unwrap();
                assert!(right.is_empty());
                assert_eq!(left.len(), if empty { 24 } else { 36 });
                assert_eq!(right_state.budget.occupied(), 0);
                if empty {
                    assert_eq!(
                        (left_state.attempts.get(), right_state.attempts.get()),
                        (0, 0)
                    );
                }
            }
            assert_eq!((left_state.owners.get(), right_state.owners.get()), (1, 1));
            drop(right);
            right_state.closed();
            drop(left);
            left_state.closed();
        }
    }
}
