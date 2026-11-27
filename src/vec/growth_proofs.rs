//! Original storage-growth publication, with unrestricted numeric unit storage.
use super::*;

fn publication<L: LengthPolicy>(
    len: usize,
    capacity: usize,
    needed: usize,
    policy: L,
    exact: bool,
) {
    let mut owner = super::query_proofs::numeric_unit_owner(len, capacity, policy);
    let pointer = owner.as_slice().as_ptr();
    let limit = owner.max_len();
    let expected = geometry::growth_for_layout(capacity, needed, limit, 0, exact).unwrap();
    assert!(owner.grow_to(needed, exact) == Ok(()));
    assert!(owner.len() == len && owner.capacity() == expected && owner.max_len() == limit);
    assert!(needed <= owner.capacity() && owner.capacity() <= limit);
    assert!(owner.as_slice().as_ptr() == pointer && owner.layout().size() == 0);
}

#[kani::proof]
fn storage_growth_publishes_only_the_admitted_capacity_for_every_numeric_owner() {
    let len: usize = kani::any();
    let capacity: usize = kani::any();
    let needed: usize = kani::any();
    let limit: usize = kani::any();
    let exact: bool = kani::any();
    kani::assume(len <= capacity && capacity < needed && needed <= limit);
    publication(len, capacity, needed, LengthLimit::new(limit), exact);
    publication(len, capacity, needed, Unlimited, exact);
}

/// A full owner's push grows exactly as `grow(1, false)` proposes, or refuses
/// with the same error and leaves length and capacity unchanged.
fn push_growth<L: LengthPolicy>(len: usize, policy: L) {
    let mut owner = super::query_proofs::numeric_unit_owner(len, len, policy);
    let limit = owner.max_len();
    let expected = required_len(len, 1, limit)
        .and_then(|needed| geometry::growth_for_layout(len, needed, limit, 0, false));
    match owner.try_push(()) {
        Ok(()) => {
            assert!(Ok(owner.capacity()) == expected);
            assert!(owner.len() == len + 1 && owner.capacity() <= limit);
        }
        Err(rejected) => {
            assert!(expected.err() == Some(rejected.error));
            assert!(owner.len() == len && owner.capacity() == len);
        }
    }
}

#[kani::proof]
fn push_growth_matches_grow_one_for_every_full_numeric_owner() {
    let len: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(len <= limit);
    push_growth(len, LengthLimit::new(limit));
    push_growth(len, Unlimited);
}

// Closure harnesses (issue #16, unit 1 step 5): the capacity writes of
// `resize_storage` and `grow_for_push`, driven through every unconditional
// entry point from an arbitrary owner that satisfies only `closure_invariant`.
use super::query_proofs::{closure_exit, closure_owner};

fn closure_reserve_case<L: LengthPolicy>(limit: L, exact: bool) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let additional: usize = kani::any();
    let result = if exact {
        owner.try_reserve_exact(additional)
    } else {
        owner.try_reserve(additional)
    };
    closure_exit(&owner, max);
    assert!(owner.len() == len);
    match result {
        Ok(()) => assert!(cap <= owner.capacity() && owner.capacity() - len >= additional),
        Err(_) => assert!(owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_reserve_preserves_the_invariant() {
    closure_reserve_case(LengthLimit::new(kani::any()), false);
    closure_reserve_case(Unlimited, false);
}
#[kani::proof]
fn closure_reserve_exact_preserves_the_invariant() {
    closure_reserve_case(LengthLimit::new(kani::any()), true);
    closure_reserve_case(Unlimited, true);
}

fn closure_grow_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let additional: usize = kani::any();
    let result = owner.grow(additional, kani::any());
    closure_exit(&owner, max);
    assert!(owner.len() == len);
    match result {
        Ok(()) => assert!(owner.capacity() - len >= additional),
        Err(_) => assert!(owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_grow_preserves_the_invariant() {
    closure_grow_case(LengthLimit::new(kani::any()));
    closure_grow_case(Unlimited);
}

fn closure_reserve_length_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let additional: usize = kani::any();
    let result = owner.reserve_length(additional);
    closure_exit(&owner, max);
    assert!(owner.len() == len);
    match result {
        // The value the bulk writers publish as the new length.
        Ok(needed) => {
            assert!(len.checked_add(additional) == Some(needed) && needed <= owner.capacity())
        }
        Err(_) => assert!(owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_reserve_length_preserves_the_invariant() {
    closure_reserve_length_case(LengthLimit::new(kani::any()));
    closure_reserve_length_case(Unlimited);
}

fn closure_push_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let result = owner.try_push(());
    closure_exit(&owner, max);
    match result {
        Ok(()) => assert!(owner.len() == len + 1 && cap <= owner.capacity()),
        Err(_) => assert!(owner.len() == len && owner.capacity() == cap),
    }
}
#[kani::proof]
fn closure_push_preserves_the_invariant() {
    closure_push_case(LengthLimit::new(kani::any()));
    closure_push_case(Unlimited);
}

fn closure_shrink_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    // A zero-sized element returns before the capacity write; the sized
    // instantiations are in the stubbed harnesses.
    assert!(owner.try_shrink_to(kani::any()) == Ok(()));
    assert!(owner.try_shrink_to_fit() == Ok(()));
    closure_exit(&owner, max);
    assert!(owner.len() == len && owner.capacity() == cap);
}
#[kani::proof]
fn closure_shrink_preserves_the_invariant_of_a_unit_owner() {
    closure_shrink_case(LengthLimit::new(kani::any()));
    closure_shrink_case(Unlimited);
}

// Second instantiation (issue #16, unit 1 step 6): the same capacity paths at
// sized elements. No storage exists: `-Z stubbing` replaces the three storage
// functions, so the owner's words range over every admitted value and the
// allocator succeeds or refuses on every call. The module compiles only with
// `--cfg jzap_closure_stubs` (a stub attribute needs `-Z stubbing`).
#[cfg(jzap_closure_stubs)]
mod sized {
    use super::super::query_proofs::{closure_exit, closure_invariant};
    use super::*;

    /// The byte bound of the capacity word: `cap * size <= isize::MAX`. It is
    /// the invariant of the closed `Capacity` type.
    fn closure_byte_bound<T, P: AllocationPolicy, L: LengthPolicy>(owner: &Vec<T, P, L>) -> bool {
        crate::layout::array::<T>(owner.cap.get()).is_ok()
    }

    /// An arbitrary owner of sized elements for the pointer-free capacity
    /// paths: every length and every capacity that `Capacity::try_new` admits,
    /// under `closure_invariant`, and nothing else. The pointer names no
    /// storage; the storage functions are replaced and no element is read or
    /// written.
    fn closure_sized_owner<T, L: LengthPolicy>(limit: L) -> Vec<T, Unmetered, L> {
        assert!(mem::size_of::<T>() != 0);
        let Ok((cap, _)) = crate::capacity::Capacity::<T>::try_new(kani::any()) else {
            kani::assume(false);
            unreachable!()
        };
        let owner = Vec {
            ptr: NonNull::dangling(),
            len: Checked::new(kani::any()),
            cap,
            policy: Unmetered,
            limit,
            marker: PhantomData,
        };
        kani::assume(closure_invariant(&owner));
        owner
    }

    fn refusal(layout: Layout) -> Error {
        Error::Allocation {
            size: layout.size(),
            align: layout.align(),
        }
    }
    /// Stands in for `storage::allocate`: an aligned address or a refusal.
    fn allocate(layout: Layout, _budget: Accounting<'_>) -> Result<NonNull<u8>, Error> {
        if kani::any() {
            Ok(NonNull::without_provenance(
                core::num::NonZeroUsize::new(layout.align()).unwrap(),
            ))
        } else {
            Err(refusal(layout))
        }
    }
    /// Stands in for `storage::reallocate`: the same address or a refusal.
    unsafe fn reallocate(
        ptr: NonNull<u8>,
        _old: Layout,
        new: Layout,
        _budget: Accounting<'_>,
    ) -> Result<NonNull<u8>, Error> {
        if kani::any() {
            Ok(ptr)
        } else {
            Err(refusal(new))
        }
    }
    /// Stands in for `storage::deallocate`: the fixture owns no allocation.
    unsafe fn deallocate(_ptr: NonNull<u8>, _layout: Layout, _budget: Accounting<'_>) {}

    fn exit<T, L: LengthPolicy>(owner: &Vec<T, Unmetered, L>, limit: usize) {
        closure_exit(owner, limit);
        assert!(closure_byte_bound(owner));
    }

    fn reserve<T, L: LengthPolicy>(limit: L, exact: bool) {
        let mut owner = closure_sized_owner::<T, _>(limit);
        let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
        let additional: usize = kani::any();
        let result = if exact {
            owner.try_reserve_exact(additional)
        } else {
            owner.try_reserve(additional)
        };
        exit(&owner, max);
        assert!(owner.len() == len);
        match result {
            Ok(()) => assert!(cap <= owner.capacity() && owner.capacity() - len >= additional),
            Err(_) => assert!(owner.capacity() == cap),
        }
        kani::cover!(result.is_ok() && owner.capacity() > cap);
        kani::cover!(matches!(result, Err(Error::Allocation { .. })));
        kani::cover!(matches!(result, Err(Error::Layout)));
    }
    fn grow<T, L: LengthPolicy>(limit: L) {
        let mut owner = closure_sized_owner::<T, _>(limit);
        let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
        let additional: usize = kani::any();
        let result = owner.grow(additional, kani::any());
        exit(&owner, max);
        assert!(owner.len() == len);
        match result {
            Ok(()) => assert!(owner.capacity() - len >= additional),
            Err(_) => assert!(owner.capacity() == cap),
        }
    }
    fn reserve_length<T, L: LengthPolicy>(limit: L) {
        let mut owner = closure_sized_owner::<T, _>(limit);
        let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
        let additional: usize = kani::any();
        let result = owner.reserve_length(additional);
        exit(&owner, max);
        assert!(owner.len() == len);
        match result {
            Ok(needed) => {
                assert!(len.checked_add(additional) == Some(needed) && needed <= owner.capacity())
            }
            Err(_) => assert!(owner.capacity() == cap),
        }
    }
    fn shrink<T, L: LengthPolicy>(limit: L) {
        let mut owner = closure_sized_owner::<T, _>(limit);
        let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
        let minimum: usize = kani::any();
        let result = if kani::any() {
            owner.try_shrink_to(minimum)
        } else {
            owner.try_shrink_to_fit()
        };
        exit(&owner, max);
        assert!(owner.len() == len && owner.capacity() <= cap);
        if result.is_err() {
            assert!(owner.capacity() == cap);
        }
        kani::cover!(result.is_ok() && owner.capacity() < cap);
        kani::cover!(result.is_err());
    }
    /// The storage transition of a full owner's push, on the words `try_push`
    /// passes and stores back. The element write that follows needs storage
    /// and is not run here.
    fn push_growth<T, L: LengthPolicy>(limit: L) {
        let mut owner = closure_sized_owner::<T, _>(limit);
        let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
        if len >= cap {
            let (mut ptr, mut words) = (owner.ptr, owner.cap);
            // SAFETY: the storage functions are replaced; no storage is touched.
            let result = unsafe {
                grow_for_push(
                    &mut ptr,
                    &mut words,
                    len,
                    max,
                    crate::budget::Accounting::Unmetered,
                )
            };
            match result {
                Ok(()) => {
                    owner.ptr = ptr;
                    owner.cap = words;
                    assert!(len < owner.capacity());
                }
                Err(_) => assert!(words.get() == cap && ptr == owner.ptr),
            }
        }
        exit(&owner, max);
        assert!(owner.len() == len);
    }
    fn constructors<T>() {
        let capacity: usize = kani::any();
        let limit: usize = kani::any();
        match Vec::<T>::try_with_capacity_in(capacity, Unmetered) {
            Ok(owner) => {
                assert!(owner.len() == 0 && owner.capacity() >= capacity);
                exit(&owner, usize::MAX);
            }
            Err(error) => assert!(matches!(error, Error::Layout | Error::Allocation { .. })),
        }
        match Vec::<T>::try_with_capacity_and_limit_in(capacity, limit, Unmetered) {
            Ok(owner) => {
                assert!(owner.len() == 0 && owner.capacity() >= capacity);
                exit(&owner, limit);
            }
            Err(_) => {}
        }
    }

    #[repr(align(64))]
    struct Aligned([u8; 64]);
    macro_rules! every_type {
        ($case:ident $(, $argument:expr)*) => {{
            $case::<u8, _>(LengthLimit::new(kani::any()) $(, $argument)*);
            $case::<u8, _>(Unlimited $(, $argument)*);
            $case::<u64, _>(LengthLimit::new(kani::any()) $(, $argument)*);
            $case::<u64, _>(Unlimited $(, $argument)*);
            $case::<[u64; 200], _>(LengthLimit::new(kani::any()) $(, $argument)*);
            $case::<[u64; 200], _>(Unlimited $(, $argument)*);
            $case::<Aligned, _>(LengthLimit::new(kani::any()) $(, $argument)*);
            $case::<Aligned, _>(Unlimited $(, $argument)*);
        }};
    }

    #[kani::proof]
    #[kani::stub(crate::storage::allocate, allocate)]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_reserve_preserves_the_invariant_of_every_sized_owner() {
        every_type!(reserve, false);
    }
    #[kani::proof]
    #[kani::stub(crate::storage::allocate, allocate)]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_reserve_exact_preserves_the_invariant_of_every_sized_owner() {
        every_type!(reserve, true);
    }
    #[kani::proof]
    #[kani::stub(crate::storage::allocate, allocate)]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_grow_preserves_the_invariant_of_every_sized_owner() {
        every_type!(grow);
    }
    #[kani::proof]
    #[kani::stub(crate::storage::allocate, allocate)]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_reserve_length_preserves_the_invariant_of_every_sized_owner() {
        every_type!(reserve_length);
    }
    #[kani::proof]
    #[kani::stub(crate::storage::allocate, allocate)]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_shrink_preserves_the_invariant_of_every_sized_owner() {
        every_type!(shrink);
    }
    #[kani::proof]
    #[kani::stub(crate::storage::allocate, allocate)]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_push_growth_preserves_the_invariant_of_every_sized_owner() {
        every_type!(push_growth);
    }
    #[kani::proof]
    #[kani::stub(crate::storage::allocate, allocate)]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_capacity_constructors_establish_the_invariant_of_every_sized_owner() {
        constructors::<u8>();
        constructors::<u64>();
        constructors::<[u64; 200]>();
        constructors::<Aligned>();
    }
}
