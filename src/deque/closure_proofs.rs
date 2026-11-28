//! Closure harnesses of the VecDeque owner (issue #16, unit 4; docs/premise-proofs.md).
//!
//! Each harness builds its owner with one fixture helper of this module, whose
//! only assumption is the owner invariant, runs an actual owner method and
//! asserts the invariant on every return. `closure_deque` gives a VecDeque of
//! zero-sized elements; `closure_vec_owner` gives a Vec of zero-sized elements
//! under the Vec invariant (its owner is Vec, so a harness that uses it is
//! evidence only while Vec is closed; scripts/owner_closure.py).
use super::*;

/// The VecDeque invariant, read from the owner's own words: `len <= cap <=
/// limit`, the byte bound of `cap`, and the ring bound on `head`.
fn closure_invariant<T, P: AllocationPolicy, L: LengthPolicy>(owner: &VecDeque<T, P, L>) -> bool {
    owner.len.get() <= owner.cap.get()
        && owner.cap.get() <= owner.limit.max_len()
        && crate::layout::array::<T>(owner.cap.get()).is_ok()
        && if mem::size_of::<T>() == 0 || owner.cap.get() == 0 {
            owner.head.get() == 0
        } else {
            owner.head.get() < owner.cap.get()
        }
        // Rule B pairing at an empty or zero-sized owner, read through probe_ptr.
        && (mem::size_of::<T>() != 0 && owner.cap.get() != 0
            || owner.probe_ptr().addr() == mem::align_of::<T>())
}

/// An arbitrary owner of zero-sized elements: every head, length and capacity
/// word that satisfies `closure_invariant` under the given limit, and nothing
/// else. A zero-sized element needs no bytes, so the aligned dangling pointer
/// is valid storage for every count.
fn closure_deque<T, P: AllocationPolicy, L: LengthPolicy>(
    policy: P,
    limit: L,
) -> VecDeque<T, P, L> {
    assert!(mem::size_of::<T>() == 0);
    let owner = VecDeque {
        ptr: NonNull::dangling(),
        // SAFETY: every count of a zero-sized element has a valid zero-byte layout.
        cap: unsafe { Capacity::from_validated_raw(kani::any()) },
        head: Checked::new(kani::any()),
        len: Checked::new(kani::any()),
        policy,
        limit,
        marker: PhantomData,
    };
    kani::assume(closure_invariant(&owner));
    owner
}

/// An arbitrary Vec of zero-sized elements under the Vec invariant
/// `len <= cap <= limit`, read through the reviewed one-expression getters.
fn closure_vec_owner<T, P: AllocationPolicy, L: LengthPolicy>(policy: P, limit: L) -> Vec<T, P, L> {
    assert!(mem::size_of::<T>() == 0);
    // SAFETY: zero-sized elements need no storage; the words are restricted below.
    let owner = unsafe {
        Vec::from_raw_parts(NonNull::dangling(), kani::any(), kani::any(), policy, limit)
    };
    kani::assume(owner.len() <= owner.capacity() && owner.capacity() <= owner.max_len());
    owner
}

/// The exit check of a closure harness: the invariant, under the same limit.
fn closure_exit<T, P: AllocationPolicy, L: LengthPolicy>(owner: &VecDeque<T, P, L>, limit: usize) {
    assert!(closure_invariant(owner));
    assert!(owner.max_len() == limit);
}

/// A unit slice of any length, at an aligned non-null address.
fn closure_unit_slice<'a>() -> &'a [()] {
    // SAFETY: zero-sized values need no storage.
    unsafe { core::slice::from_raw_parts(NonNull::<()>::dangling().as_ptr(), kani::any()) }
}

/// Any range endpoint.
fn closure_bound() -> core::ops::Bound<usize> {
    match kani::any::<u8>() {
        0 => core::ops::Bound::Included(kani::any()),
        1 => core::ops::Bound::Excluded(kani::any()),
        _ => core::ops::Bound::Unbounded,
    }
}

fn closure_constructors<T>(limit: usize) {
    closure_exit(&VecDeque::<T>::new(), usize::MAX);
    closure_exit(&VecDeque::<T>::new_in(Unmetered), usize::MAX);
    closure_exit(&VecDeque::<T>::with_limit(limit), limit);
    closure_exit(&VecDeque::<T>::with_limit_in(limit, Unmetered), limit);
    closure_exit(&<VecDeque<T> as Default>::default(), usize::MAX);
    let Ok(owner) = <VecDeque<T> as crate::TryDefault>::try_default() else {
        unreachable!()
    };
    closure_exit(&owner, usize::MAX);
}
#[kani::proof]
fn closure_empty_constructors_establish_the_invariant() {
    #[repr(align(64))]
    struct Aligned([u8; 64]);
    let limit: usize = kani::any();
    closure_constructors::<()>(limit);
    closure_constructors::<u8>(limit);
    closure_constructors::<u64>(limit);
    closure_constructors::<[u64; 200]>(limit);
    closure_constructors::<Aligned>(limit);
}

#[kani::proof]
fn closure_capacity_constructors_establish_the_invariant() {
    let capacity: usize = kani::any();
    let limit: usize = kani::any();
    let Ok(owner) = VecDeque::<()>::try_with_capacity(capacity) else {
        unreachable!()
    };
    closure_exit(&owner, usize::MAX);
    drop(owner);
    let Ok(owner) = VecDeque::<()>::try_with_capacity_in(capacity, Unmetered) else {
        unreachable!()
    };
    closure_exit(&owner, usize::MAX);
    drop(owner);
    match VecDeque::<()>::try_with_capacity_and_limit(capacity, limit) {
        Ok(owner) => {
            assert!(capacity <= limit);
            closure_exit(&owner, limit);
        }
        Err(error) => assert!(capacity > limit && error == Error::Capacity),
    }
    match VecDeque::<()>::try_with_capacity_and_limit_in(capacity, limit, Unmetered) {
        Ok(owner) => {
            assert!(capacity <= limit);
            closure_exit(&owner, limit);
        }
        Err(error) => assert!(capacity > limit && error == Error::Capacity),
    }
}

fn closure_reserve_case<L: LengthPolicy>(limit: L, exact: bool) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let additional: usize = kani::any();
    let result = if exact {
        owner.try_reserve_exact(additional)
    } else {
        owner.try_reserve(additional)
    };
    closure_exit(&owner, max);
    assert!(owner.len() == len && owner.capacity() >= cap);
    kani::cover!(owner.capacity() > cap);
    kani::cover!(result.is_err());
    if result.is_ok() {
        assert!(owner.capacity() - owner.len() >= additional);
    } else {
        assert!(owner.capacity() == cap);
    }
}
#[kani::proof]
fn closure_reserve_preserves_the_invariant() {
    closure_reserve_case(LengthLimit::new(kani::any()), false);
    closure_reserve_case(LengthLimit::new(kani::any()), true);
    closure_reserve_case(Unlimited, false);
    closure_reserve_case(Unlimited, true);
}

fn closure_push_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let max = owner.max_len();
    let len = owner.len();
    let result = owner.try_push_back(());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { len + 1 } else { len });
    kani::cover!(result.is_ok() && len > 0);
    kani::cover!(result.is_err());
    let len = owner.len();
    let result = owner.try_push_front(());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { len + 1 } else { len });
    let len = owner.len();
    let result = owner.try_push_back_mut(()).is_ok();
    closure_exit(&owner, max);
    assert!(owner.len() == if result { len + 1 } else { len });
    let len = owner.len();
    let result = owner.try_push_front_mut(()).is_ok();
    closure_exit(&owner, max);
    assert!(owner.len() == if result { len + 1 } else { len });
}
#[kani::proof]
fn closure_push_preserves_the_invariant() {
    closure_push_case(LengthLimit::new(kani::any()));
    closure_push_case(Unlimited);
}

fn closure_pop_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let max = owner.max_len();
    let len = owner.len();
    let result = owner.pop_front();
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_some() { len - 1 } else { len });
    kani::cover!(result.is_some());
    let len = owner.len();
    let result = owner.pop_back();
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_some() { len - 1 } else { len });
    let len = owner.len();
    let result = owner.try_pop_front_if(|_| {
        if kani::any() {
            Ok(kani::any())
        } else {
            Err(())
        }
    });
    closure_exit(&owner, max);
    assert!(
        owner.len()
            == if matches!(result, Ok(Some(()))) {
                len - 1
            } else {
                len
            }
    );
    let len = owner.len();
    let result = owner.try_pop_back_if(|_| {
        if kani::any() {
            Ok(kani::any())
        } else {
            Err(())
        }
    });
    closure_exit(&owner, max);
    assert!(
        owner.len()
            == if matches!(result, Ok(Some(()))) {
                len - 1
            } else {
                len
            }
    );
    let len = owner.len();
    let result = owner.swap_remove_front(kani::any());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_some() { len - 1 } else { len });
    let len = owner.len();
    let result = owner.swap_remove_back(kani::any());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_some() { len - 1 } else { len });
}
#[kani::proof]
fn closure_pop_preserves_the_invariant() {
    closure_pop_case(LengthLimit::new(kani::any()));
    closure_pop_case(Unlimited);
}

fn closure_truncate_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let keep: usize = kani::any();
    owner.truncate(keep);
    closure_exit(&owner, max);
    assert!(owner.len() == if keep < len { keep } else { len });
    owner.clear();
    closure_exit(&owner, max);
    assert!(owner.is_empty());
}
#[kani::proof]
fn closure_truncate_preserves_the_invariant() {
    closure_truncate_case(LengthLimit::new(kani::any()));
    closure_truncate_case(Unlimited);
}

fn closure_make_contiguous_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    assert!(owner.make_contiguous().len() == len);
    closure_exit(&owner, max);
    assert!(owner.len() == len && owner.capacity() == cap);
}
#[kani::proof]
fn closure_make_contiguous_preserves_the_invariant() {
    closure_make_contiguous_case(LengthLimit::new(kani::any()));
    closure_make_contiguous_case(Unlimited);
}

fn closure_shrink_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    assert!(owner.try_shrink_to(kani::any()).is_ok());
    closure_exit(&owner, max);
    assert!(owner.try_shrink_to_fit().is_ok());
    closure_exit(&owner, max);
    // A zero-sized element returns before any write.
    assert!(owner.len() == len && owner.capacity() == cap);
}
#[kani::proof]
fn closure_shrink_preserves_the_invariant_of_a_unit_owner() {
    closure_shrink_case(LengthLimit::new(kani::any()));
    closure_shrink_case(Unlimited);
}

fn closure_insert_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let max = owner.max_len();
    let len = owner.len();
    let result = owner.try_insert(kani::any(), ());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { len + 1 } else { len });
    let len = owner.len();
    let result = owner.try_insert_mut(kani::any(), ()).is_ok();
    closure_exit(&owner, max);
    assert!(owner.len() == if result { len + 1 } else { len });
}
#[kani::proof]
fn closure_insert_preserves_the_invariant() {
    closure_insert_case(LengthLimit::new(kani::any()));
    closure_insert_case(Unlimited);
}

fn closure_remove_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let result = owner.remove(kani::any());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_some() { len - 1 } else { len });
}
#[kani::proof]
fn closure_remove_preserves_the_invariant() {
    closure_remove_case(LengthLimit::new(kani::any()));
    closure_remove_case(Unlimited);
}

fn closure_rotate_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let n: usize = kani::any();
    let result = owner.rotate_left(n);
    closure_exit(&owner, max);
    assert!(result.is_ok() == (n <= len));
    let result = owner.rotate_right(n);
    closure_exit(&owner, max);
    assert!(result.is_ok() == (n <= len));
    assert!(owner.len() == len && owner.capacity() == cap);
}
#[kani::proof]
fn closure_rotate_preserves_the_invariant() {
    closure_rotate_case(LengthLimit::new(kani::any()));
    closure_rotate_case(Unlimited);
}

fn closure_append_case<L: LengthPolicy>(first: L, second: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, first);
    let mut other = closure_deque::<(), _, _>(Unmetered, second);
    let (len, more, max, other_max) = (owner.len(), other.len(), owner.max_len(), other.max_len());
    let result = owner.try_append(&mut other);
    closure_exit(&owner, max);
    closure_exit(&other, other_max);
    match result {
        Ok(()) => assert!(owner.len() == len + more && other.is_empty()),
        Err(_) => assert!(owner.len() == len && other.len() == more),
    }
}
#[kani::proof]
fn closure_append_preserves_the_invariant_of_both_owners() {
    closure_append_case(LengthLimit::new(kani::any()), LengthLimit::new(kani::any()));
    closure_append_case(Unlimited, Unlimited);
}

fn closure_split_off_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let at: usize = kani::any();
    match owner.try_split_off(at) {
        Ok(tail) => {
            closure_exit(&tail, max);
            assert!(at <= len && tail.len() == len - at && owner.len() == at);
        }
        Err(_) => assert!(owner.len() == len),
    }
    closure_exit(&owner, max);
}
#[kani::proof]
fn closure_split_off_preserves_the_invariant_of_both_owners() {
    closure_split_off_case(LengthLimit::new(kani::any()));
    closure_split_off_case(Unlimited);
}

fn closure_drain_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let range = (closure_bound(), closure_bound());
    let Ok(mut drain) = owner.drain(range) else {
        closure_exit(&owner, max);
        assert!(owner.len() == len);
        return;
    };
    let count = drain.len();
    match kani::any::<u8>() {
        0 => {
            mem::forget(drain);
            closure_exit(&owner, max);
            assert!(owner.len() <= len);
            return;
        }
        1 => {
            let _ = drain.next();
        }
        2 => {
            let _ = drain.next_back();
        }
        _ => {}
    }
    drop(drain);
    closure_exit(&owner, max);
    assert!(owner.len() == len - count);
    kani::cover!(count > 0 && owner.len() > 0);
}
#[kani::proof]
#[kani::unwind(2)]
fn closure_drain_preserves_the_invariant() {
    closure_drain_case(LengthLimit::new(kani::any()));
    closure_drain_case(Unlimited);
}

fn closure_clone_case<L: LengthPolicy>(limit: L) {
    let owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let Ok(copy) = owner.try_clone() else {
        unreachable!()
    };
    closure_exit(&copy, max);
    closure_exit(&owner, max);
    assert!(copy.len() == len);
}
#[kani::proof]
fn closure_clone_establishes_the_invariant_of_the_copy() {
    closure_clone_case(LengthLimit::new(kani::any()));
    closure_clone_case(Unlimited);
}

fn closure_clone_from_case<L: LengthPolicy>(first: L, second: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, first);
    let source = closure_deque::<(), _, _>(Unmetered, second);
    let (len, max) = (owner.len(), owner.max_len());
    let result = owner.try_clone_from(&source);
    closure_exit(&owner, max);
    closure_exit(&source, second.max_len());
    match result {
        Ok(()) => assert!(owner.len() == source.len()),
        Err(_) => assert!(owner.len() == len && source.len() > max),
    }
}
#[kani::proof]
fn closure_clone_from_preserves_the_invariant_of_both_owners() {
    closure_clone_from_case(LengthLimit::new(kani::any()), LengthLimit::new(kani::any()));
    closure_clone_from_case(Unlimited, Unlimited);
}

fn closure_extend_copy_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let values = closure_unit_slice();
    let result = owner.try_extend_copy(values);
    closure_exit(&owner, max);
    assert!(
        owner.len()
            == if result.is_ok() {
                len + values.len()
            } else {
                len
            }
    );
}
#[kani::proof]
fn closure_extend_copy_preserves_the_invariant() {
    closure_extend_copy_case(LengthLimit::new(kani::any()));
    closure_extend_copy_case(Unlimited);
}

fn closure_resize_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let new_len: usize = kani::any();
    let result = owner.try_resize(new_len, &());
    closure_exit(&owner, max);
    assert!(owner.len() == if result.is_ok() { new_len } else { len });
}
#[kani::proof]
fn closure_resize_preserves_the_invariant() {
    closure_resize_case(LengthLimit::new(kani::any()));
    closure_resize_case(Unlimited);
}

fn closure_from_vec_case<L: LengthPolicy>(limit: L) {
    let max = limit.max_len();
    let source = closure_vec_owner::<(), _, _>(Unmetered, limit);
    let (len, cap) = (source.len(), source.capacity());
    let owner = VecDeque::from_vec(source);
    closure_exit(&owner, max);
    assert!(owner.len() == len && owner.capacity() == cap);
    kani::cover!(len > 0 && cap > len);
}
#[kani::proof]
fn closure_from_vec_establishes_the_invariant() {
    closure_from_vec_case(LengthLimit::new(kani::any()));
    closure_from_vec_case(Unlimited);
}

fn closure_into_vec_case<L: LengthPolicy>(limit: L) {
    let owner = closure_deque::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let values = owner.into_vec();
    // The Vec invariant, read through the reviewed Vec getters.
    assert!(values.len() <= values.capacity() && values.capacity() <= values.max_len());
    assert!(values.len() == len && values.capacity() == cap && values.max_len() == max);
    // Rule B pairing of the Vec at a zero-sized element, read through as_ptr.
    assert!(values.as_ptr().addr() == mem::align_of::<()>());
    kani::cover!(len > 0 && cap > len);
}
#[kani::proof]
fn closure_into_vec_establishes_the_vec_invariant() {
    closure_into_vec_case(LengthLimit::new(kani::any()));
    closure_into_vec_case(Unlimited);
}

fn closure_adopting_case<L: LengthPolicy>(limit: L) {
    let length: usize = kani::any();
    let max = limit.max_len();
    let mut values = alloc::vec::Vec::<()>::new();
    // SAFETY: every unit is valid without bytes; std's unit capacity is usize::MAX.
    unsafe {
        values.set_len(length);
    }
    let source = alloc::collections::VecDeque::from(values);
    match VecDeque::try_from_std_in(source, Unmetered, limit) {
        Ok(owner) => {
            assert!(owner.len() == length && owner.capacity() == max);
            closure_exit(&owner, max);
            kani::cover!(length > 0);
        }
        Err(rejected) => {
            assert!(
                length > max && rejected.value.len() == length && rejected.error == Error::Capacity
            );
            kani::cover!(true);
        }
    }
}
#[kani::proof]
fn closure_adopting_constructor_establishes_the_invariant_for_unit_storage() {
    closure_adopting_case(LengthLimit::new(kani::any()));
    closure_adopting_case(Unlimited);
}

// The retain loop runs under its loop contract (`--cfg jzap_loop_contracts`,
// `-Z loop-contracts`): every length, a callback that keeps, removes or fails
// on every call, no bound on the number of turns.
#[cfg(jzap_loop_contracts)]
mod loops {
    use super::*;

    fn closure_choice() -> Result<bool, u8> {
        match kani::any::<u8>() {
            0 => Ok(true),
            1 => Ok(false),
            _ => Err(7),
        }
    }
    fn closure_retain_case<L: LengthPolicy>(limit: L) {
        let mut owner = closure_deque::<(), _, _>(Unmetered, limit);
        let (len, max) = (owner.len(), owner.max_len());
        let result = owner.try_retain_mut(|_| closure_choice());
        closure_exit(&owner, max);
        assert!(owner.len() <= len);
        kani::cover!(result.is_ok() && owner.len() < len && owner.len() > 0);
        kani::cover!(result.is_err() && len > 1);
        let len = owner.len();
        let result = owner.try_retain(|_| closure_choice());
        closure_exit(&owner, max);
        assert!(owner.len() <= len);
        kani::cover!(result.is_ok());
    }
    #[kani::proof]
    fn closure_retain_preserves_the_invariant() {
        closure_retain_case(LengthLimit::new(kani::any()));
        closure_retain_case(Unlimited);
    }
}

// Second instantiation: the capacity constructors at sized elements. No
// storage exists: `-Z stubbing` replaces the storage functions, so the
// capacity word ranges over every admitted count and the allocator succeeds
// or refuses on every call. A fresh owner has capacity 0, so `grow` takes no
// copy path. The module compiles only with `--cfg jzap_closure_stubs`.
#[cfg(jzap_closure_stubs)]
mod sized {
    use super::*;
    use core::alloc::Layout;

    fn refusal(layout: Layout) -> Error {
        Error::Allocation {
            size: layout.size(),
            align: layout.align(),
        }
    }
    // Rule B (issue #16, unit 7): the single allocation the owner holds, as
    // (address, layout), as in vec::growth_proofs::sized.
    static mut LIVE: Option<(usize, Layout)> = None;
    static mut NEXT: usize = 2;
    fn fresh(layout: Layout) -> NonNull<u8> {
        let index = unsafe { NEXT };
        unsafe { NEXT = index + 1 };
        NonNull::without_provenance(core::num::NonZeroUsize::new(layout.align() * index).unwrap())
    }
    /// The pairing of rule B, read through `probe_ptr`.
    fn paired<T, L: LengthPolicy>(owner: &VecDeque<T, Unmetered, L>) -> bool {
        match crate::layout::array::<T>(owner.capacity()) {
            Ok(layout) if layout.size() != 0 => {
                (unsafe { LIVE }) == Some((owner.probe_ptr().addr(), layout))
            }
            Ok(_) => owner.probe_ptr().addr() == mem::align_of::<T>(),
            Err(_) => false,
        }
    }
    /// Stands in for `storage::reallocate`: requires the recorded allocation
    /// of `old`; returns the same or a fresh recorded allocation of `new` (the
    /// dangling address of an empty `new`), or a refusal that keeps `old`.
    unsafe fn reallocate(
        ptr: NonNull<u8>,
        old: Layout,
        new: Layout,
        _budget: Accounting<'_>,
    ) -> Result<NonNull<u8>, Error> {
        if old.size() != 0 {
            assert!(unsafe { LIVE } == Some((ptr.as_ptr().addr(), old)));
        }
        if !kani::any::<bool>() {
            return Err(refusal(new));
        }
        if new.size() == 0 {
            unsafe { LIVE = None };
            return Ok(NonNull::without_provenance(
                core::num::NonZeroUsize::new(new.align()).unwrap(),
            ));
        }
        let p = if old.size() != 0 && kani::any() {
            ptr
        } else {
            fresh(new)
        };
        unsafe { LIVE = Some((p.as_ptr().addr(), new)) };
        Ok(p)
    }
    /// Stands in for `storage::deallocate`: requires the recorded allocation.
    unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout, _budget: Accounting<'_>) {
        if layout.size() != 0 {
            assert!(unsafe { LIVE } == Some((ptr.as_ptr().addr(), layout)));
            unsafe { LIVE = None };
        }
    }

    fn constructors<T>() {
        let capacity: usize = kani::any();
        let limit: usize = kani::any();
        match VecDeque::<T>::try_with_capacity_in(capacity, Unmetered) {
            Ok(owner) => {
                assert!(owner.is_empty() && owner.capacity() == capacity);
                closure_exit(&owner, usize::MAX);
                assert!(paired(&owner));
                kani::cover!(capacity > 0);
            }
            Err(error) => {
                assert!(matches!(
                    error,
                    Error::Capacity | Error::Layout | Error::Allocation { .. }
                ));
                kani::cover!(matches!(error, Error::Allocation { .. }));
                kani::cover!(matches!(error, Error::Layout | Error::Capacity));
            }
        }
        match VecDeque::<T>::try_with_capacity_and_limit_in(capacity, limit, Unmetered) {
            Ok(owner) => {
                assert!(owner.is_empty() && owner.capacity() == capacity && capacity <= limit);
                closure_exit(&owner, limit);
                assert!(paired(&owner));
            }
            Err(error) => assert!(matches!(
                error,
                Error::Capacity | Error::Layout | Error::Allocation { .. }
            )),
        }
    }
    #[repr(align(64))]
    struct Aligned([u8; 64]);
    #[kani::proof]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_capacity_constructors_establish_the_invariant_of_every_sized_owner() {
        constructors::<u8>();
        constructors::<u64>();
        constructors::<[u64; 200]>();
        constructors::<Aligned>();
    }
}
