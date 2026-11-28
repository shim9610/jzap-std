//! Actual Buffer reservation, including layout and metadata publication at all word states.
use super::*;
#[kani::proof]
fn every_valid_unit_owner_reservation_matches_the_numeric_result() {
    let len: usize = kani::any();
    let cap: usize = kani::any();
    kani::assume(len <= cap);
    let additional: usize = kani::any();
    let hint: usize = kani::any();
    let ptr = NonNull::dangling();
    let layout = Layout::new::<()>();
    let mut owner = Buffer::<(), crate::budget::Unmetered> {
        ptr,
        len: Checked::new(len),
        cap: Checked::new(cap),
        layout,
        policy: crate::budget::Unmetered,
    };
    let expected = crate::boxed::buffer_numeric::reservation(len, cap, additional, hint);
    let actual = owner.reserve(additional, hint);
    assert!(owner.ptr == ptr && owner.len.get() == len && owner.layout == layout);
    match expected {
        Ok((needed, proposed)) => {
            assert!(actual == Ok(needed));
            assert!(owner.cap.get() == proposed);
        }
        Err(error) => {
            assert!(actual == Err(error));
            assert!(owner.cap.get() == cap);
        }
    }
    drop(owner);
}

// Closure harnesses (issue #16, unit 1 step 4). Each one builds its owner with
// `closure_owner`, whose only assumption is `closure_invariant`, runs one
// actual Buffer method and asserts `closure_invariant` again on every return.

/// The Buffer invariant in the owner's own words: the initialized length is at
/// most the capacity, and the stored layout is the array layout of that
/// capacity. The second part is the byte bound `cap * size <= isize::MAX`; for
/// a zero-sized element every capacity has the same zero-byte layout.
fn closure_invariant<T, P: AllocationPolicy>(owner: &Buffer<T, P>) -> bool {
    owner.len.get() <= owner.cap.get()
        && crate::layout::array::<T>(owner.cap.get()) == Ok(owner.layout)
}

/// An arbitrary owner: every length and capacity word that satisfies
/// `closure_invariant`, and nothing else. The pointer is never dereferenced
/// for bytes by the harnesses that use it.
fn closure_owner<T>() -> Buffer<T, crate::budget::Unmetered> {
    let cap: usize = kani::any();
    let layout = crate::layout::array::<T>(cap);
    kani::assume(layout.is_ok());
    let owner = Buffer {
        ptr: NonNull::dangling(),
        len: Checked::new(kani::any()),
        cap: Checked::new(cap),
        layout: layout.unwrap(),
        policy: crate::budget::Unmetered,
    };
    kani::assume(closure_invariant(&owner));
    owner
}

#[kani::proof]
fn closure_new_establishes_the_invariant() {
    #[repr(align(64))]
    struct Aligned([u8; 64]);
    macro_rules! one {
        ($ty:ty) => {{
            let owner = Buffer::<$ty, _>::new(crate::budget::Unmetered);
            assert!(closure_invariant(&owner));
            assert!(owner.len.get() == 0 && owner.cap.get() == 0);
        }};
    }
    one!(());
    one!(u8);
    one!(u64);
    one!([u64; 200]);
    one!(Aligned);
}

#[kani::proof]
fn closure_reserve_preserves_the_invariant_for_every_unit_owner() {
    let mut owner = closure_owner::<()>();
    let len = owner.len.get();
    let result = owner.reserve(kani::any(), kani::any());
    assert!(closure_invariant(&owner));
    assert!(owner.len.get() == len);
    if let Ok(needed) = result {
        // The value `push` and `append` publish as the new length.
        assert!(len <= needed && needed <= owner.cap.get());
    }
}

#[kani::proof]
fn closure_push_preserves_the_invariant_for_every_unit_owner() {
    let mut owner = closure_owner::<()>();
    let len = owner.len.get();
    let result = owner.push((), kani::any());
    assert!(closure_invariant(&owner));
    match result {
        Ok(()) => assert!(len < usize::MAX && owner.len.get() == len + 1),
        Err(_) => assert!(owner.len.get() == len),
    }
}

#[kani::proof]
fn closure_append_preserves_the_invariant_for_every_unit_owner() {
    let mut owner = closure_owner::<()>();
    let len = owner.len.get();
    let count: usize = kani::any();
    // SAFETY: a unit slice of any length is valid at an aligned non-null address.
    let values = unsafe { core::slice::from_raw_parts(NonNull::<()>::dangling().as_ptr(), count) };
    let result = owner.append(values);
    assert!(closure_invariant(&owner));
    match result {
        Ok(()) => assert!(len.checked_add(count) == Some(owner.len.get())),
        Err(_) => assert!(owner.len.get() == len),
    }
}

/// A source that yields at most `remaining` values and then ends. Every call
/// may fail instead, and every size hint is an arbitrary word or an error.
struct ClosureSource {
    remaining: u8,
}
impl TryIterator for ClosureSource {
    type Item = ();
    type Error = u8;
    fn try_next(&mut self) -> Result<Option<()>, u8> {
        if kani::any() {
            return Err(23);
        }
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        Ok(Some(()))
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), u8> {
        if kani::any() {
            Err(29)
        } else {
            Ok((kani::any(), None))
        }
    }
}

// Runs the first span of `collect` and, when the source has a value left after
// that span is full, `collect_rest` with its `loop`. The source is bounded to
// two values, so both loops are unwound; no loop invariant is used.
#[kani::proof]
#[kani::unwind(5)]
fn closure_collect_preserves_the_invariant_for_every_unit_owner_and_two_value_source() {
    let mut owner = closure_owner::<()>();
    let len = owner.len.get();
    let remaining: u8 = kani::any();
    kani::assume(remaining <= 2);
    let result = owner.collect(ClosureSource { remaining });
    assert!(closure_invariant(&owner));
    assert!(len <= owner.len.get() && owner.len.get() - len <= remaining as usize);
    kani::cover!(result.is_ok() && owner.len.get() - len == 2);
    kani::cover!(matches!(result, Err(InitError::Storage(_))));
    kani::cover!(matches!(result, Err(InitError::Initialize(_))));
}

// The two lends of the owner length to `init::collection::fill` (issue #16,
// unit 1 step 9), for a source without a bound. Each harness makes the lend
// itself: its `fill` statement is the product statement of `collect` or of
// `collect_rest` with `owner` for `self`, and `scripts/owner_closure.py`
// compares the two texts. The cursor loop of the driver runs under its
// contract (`--cfg jzap_init_loop_contracts`), so the count of values has no
// bound. The `loop` of `collect_rest` itself is not run here.

/// A source without a bound: every call ends, fails or yields a value, and
/// every size hint is an arbitrary word or an error.
#[cfg(jzap_init_loop_contracts)]
struct ClosureOpenSource;
#[cfg(jzap_init_loop_contracts)]
impl TryIterator for ClosureOpenSource {
    type Item = ();
    type Error = u8;
    fn try_next(&mut self) -> Result<Option<()>, u8> {
        if kani::any() {
            return Err(23);
        }
        Ok(if kani::any() { Some(()) } else { None })
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), u8> {
        if kani::any() {
            Err(29)
        } else {
            Ok((kani::any(), None))
        }
    }
}

// The lend of `collect`: the end is the value `reserve` returned for the lower
// bound of the hint. A hint is any word, so `lower` is any word.
// The unwind bound is for verifier-internal bookkeeping; the product loop runs
// under its contract.
#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(12)]
fn closure_collect_lend_preserves_the_invariant_for_every_unit_owner_and_open_source() {
    let mut owner = closure_owner::<()>();
    let len = owner.len.get();
    let mut iter = ClosureOpenSource;
    let lower: usize = kani::any();
    let reserved = owner.reserve(lower, lower);
    assert!(closure_invariant(&owner));
    let Ok(reserved_end) = reserved else {
        assert!(owner.len.get() == len);
        return;
    };
    // SAFETY: a unit element needs no storage; the span ends at the value
    // `reserve` admitted.
    let result = unsafe {
        crate::init::collection::fill(owner.ptr, &mut owner.len, reserved_end, &mut iter)
    };
    assert!(closure_invariant(&owner));
    assert!(len <= owner.len.get() && owner.len.get() <= reserved_end);
    if let Ok(true) = result {
        assert!(owner.len.get() == reserved_end);
    }
    kani::cover!(matches!(result, Ok(true)) && owner.len.get() > len);
    kani::cover!(matches!(result, Ok(false)) && owner.len.get() > len);
    kani::cover!(result.is_err());
}

// The lend of `collect_rest`: the end is the capacity word of the owner.
#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(12)]
fn closure_collect_rest_lend_preserves_the_invariant_for_every_unit_owner_and_open_source() {
    let mut owner = closure_owner::<()>();
    let (len, cap) = (owner.len.get(), owner.cap.get());
    let mut iter = ClosureOpenSource;
    // SAFETY: a unit element needs no storage; the span ends at the capacity.
    let result = unsafe {
        crate::init::collection::fill(owner.ptr, &mut owner.len, owner.cap.get(), &mut iter)
    };
    assert!(closure_invariant(&owner));
    assert!(len <= owner.len.get() && owner.cap.get() == cap);
    if let Ok(true) = result {
        assert!(owner.len.get() == cap);
    }
    kani::cover!(matches!(result, Ok(true)) && owner.len.get() > len);
    kani::cover!(matches!(result, Ok(false)) && owner.len.get() > len);
    kani::cover!(result.is_err());
}

#[kani::proof]
fn closure_finish_transfers_exactly_the_length_of_every_unit_owner() {
    let owner = closure_owner::<()>();
    let (ptr, len) = (owner.ptr, owner.len.get());
    let result = owner.finish();
    // `finish` consumes the owner: the invariant ends with the exact-length Box.
    let Ok(slice) = result else { unreachable!() };
    assert!(slice.len() == len && slice.as_slice().as_ptr() == ptr.as_ptr());
    drop(slice);
}

// Sized elements: the reservation and the exact-size finish with the storage
// functions replaced (`-Z stubbing`), so that no byte of storage is needed and
// the capacity word ranges over every value with a valid array layout. The
// module compiles only with `--cfg jzap_closure_stubs`: a harness with a stub
// attribute does not compile in a Kani run without `-Z stubbing`. The name is
// not in the manifest's check-cfg list, so every Kani build warns about it.
#[cfg(jzap_closure_stubs)]
mod sized {
    use super::*;

    /// Stands in for `storage::reallocate`: the allocator keeps the storage
    /// or refuses. No byte is read or written.
    unsafe fn reallocate(
        ptr: NonNull<u8>,
        _old: Layout,
        new: Layout,
        _budget: Accounting<'_>,
    ) -> Result<NonNull<u8>, Error> {
        if kani::any() {
            Ok(ptr)
        } else {
            Err(Error::Allocation {
                size: new.size(),
                align: new.align(),
            })
        }
    }
    /// Stands in for `storage::deallocate`: the fixture owns no allocation.
    unsafe fn deallocate(_ptr: NonNull<u8>, _layout: Layout, _budget: Accounting<'_>) {}

    fn reserve<T>() {
        let mut owner = closure_owner::<T>();
        let (len, cap) = (owner.len.get(), owner.cap.get());
        let result = owner.reserve(kani::any(), kani::any());
        assert!(closure_invariant(&owner));
        assert!(owner.len.get() == len);
        match result {
            Ok(needed) => assert!(len <= needed && needed <= owner.cap.get()),
            Err(_) => assert!(owner.cap.get() == cap),
        }
        kani::cover!(result.is_ok() && owner.cap.get() > cap);
        kani::cover!(matches!(result, Err(Error::Allocation { .. })));
        kani::cover!(matches!(result, Err(Error::Layout)));
    }
    fn finish<T>() {
        let owner = closure_owner::<T>();
        let (ptr, len) = (owner.ptr, owner.len.get());
        match owner.finish() {
            Ok(slice) => {
                // Not an allocation: read the words without forming a slice
                // reference, and release nothing.
                let (words, _policy) = crate::boxed::Box::into_raw_parts(slice);
                assert!(words.len() == len && words.cast::<T>() == ptr);
            }
            // The refused owner was released inside the call.
            Err(error) => assert!(matches!(error, Error::Allocation { .. })),
        }
    }
    #[repr(align(64))]
    struct Aligned([u8; 64]);

    #[kani::proof]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_reserve_preserves_the_invariant_for_every_sized_owner() {
        reserve::<u8>();
        reserve::<u64>();
        reserve::<[u64; 200]>();
        reserve::<Aligned>();
    }

    #[kani::proof]
    #[kani::stub(crate::storage::reallocate, reallocate)]
    #[kani::stub(crate::storage::deallocate, deallocate)]
    fn closure_finish_transfers_exactly_the_length_of_every_sized_owner() {
        finish::<u8>();
        finish::<u64>();
        finish::<[u64; 200]>();
        finish::<Aligned>();
    }
}
