//! Fallible contiguous ownership with an optional construction-time element limit.
//!
//! `capacity` is allocated element storage; `max_len` is a fixed logical ceiling.
//! Limit selection itself does not allocate. All growth is explicitly fallible.
//! See `docs/vec-contracts.md` for failure and iterator-consumption contracts.
pub(crate) mod compaction_driver;
pub(crate) mod staging_driver;
use crate::{
    Error, Rejected, TryClone, TryCloneIn,
    arithmetic::Checked,
    budget::{Accounting, AllocationPolicy, Budget, Unmetered},
    error::InitError,
    init::InitSlot,
    storage,
};
use core::{alloc::Layout, marker::PhantomData, mem, ptr::NonNull, slice::SliceIndex};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
pub(crate) mod bounds;
#[cfg(kani)]
mod bounds_proofs;
mod convenience;
#[cfg(kani)]
mod cursor_proofs;
pub(crate) mod edit;
pub(crate) mod geometry;
#[cfg(kani)]
mod geometry_proofs;
#[cfg(kani)]
mod growth_proofs;
mod ownership;
#[cfg(kani)]
mod query_proofs;
pub use edit::{Drain, ExtractIf, Splice, SpliceIn};
pub use ownership::IntoIter;

mod sealed {
    #[cfg(verus_keep_ghost)]
    use vstd::prelude::*;
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub trait Sealed {}
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    impl Sealed for super::Unlimited {}
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    impl Sealed for super::LengthLimit {}
}
/// Sealed element-count policy. Byte accounting is a separate allocation policy.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub trait LengthPolicy: sealed::Sealed + Copy {
    fn max_len(self) -> usize;
}
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct Unlimited;
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl LengthPolicy for Unlimited {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == usize::MAX,))]
    fn max_len(self) -> usize {
        usize::MAX
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct LengthLimit(usize);
#[cfg(verus_keep_ghost)]
verus! { impl LengthLimit { pub closed spec fn view(self) -> usize { self.0 } } }
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl LengthLimit {
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out@ == max_len,))]
    pub const fn new(max_len: usize) -> Self {
        Self(max_len)
    }
}
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl LengthPolicy for LengthLimit {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@,))]
    fn max_len(self) -> usize {
        self.0
    }
}

pub struct Vec<T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    ptr: NonNull<T>,
    len: Checked<usize>,
    cap: crate::capacity::Capacity<T>,
    policy: P,
    limit: L,
    marker: PhantomData<T>,
}
// SAFETY: unique element/storage ownership; moving/sharing also respects both policies.
unsafe impl<T: Send, P: AllocationPolicy + Send, L: LengthPolicy + Send> Send for Vec<T, P, L> {}
unsafe impl<T: Sync, P: AllocationPolicy + Sync, L: LengthPolicy + Sync> Sync for Vec<T, P, L> {}

impl<T> Vec<T> {
    pub const fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: Checked::new(0),
            cap: crate::capacity::Capacity::empty(usize::MAX),
            policy: Unmetered,
            limit: Unlimited,
            marker: PhantomData,
        }
    }
    pub fn with_limit(max_len: usize) -> Vec<T, Unmetered, LengthLimit> {
        Vec::with_limit_in(max_len, Unmetered)
    }
    pub fn try_with_capacity(capacity: usize) -> Result<Self, Error> {
        Self::try_with_capacity_in(capacity, Unmetered)
    }
    pub fn try_with_capacity_and_limit(
        capacity: usize,
        max_len: usize,
    ) -> Result<Vec<T, Unmetered, LengthLimit>, Error> {
        Vec::try_with_capacity_and_limit_in(capacity, max_len, Unmetered)
    }
}
impl<T, P: AllocationPolicy> Vec<T, P> {
    pub fn new_in(policy: P) -> Self {
        Self::empty(policy, Unlimited)
    }
    pub fn with_limit_in(max_len: usize, policy: P) -> Vec<T, P, LengthLimit> {
        Vec::empty(policy, LengthLimit(max_len))
    }
    pub fn try_with_capacity_in(capacity: usize, policy: P) -> Result<Self, Error> {
        let mut result = Self::new_in(policy);
        result.try_reserve_exact(capacity)?;
        Ok(result)
    }
    pub fn try_with_capacity_and_limit_in(
        capacity: usize,
        max_len: usize,
        policy: P,
    ) -> Result<Vec<T, P, LengthLimit>, Error> {
        let mut result = Self::with_limit_in(max_len, policy);
        result.try_reserve_exact(capacity)?;
        Ok(result)
    }
}
impl<T> Default for Vec<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Check logical addition before forming a layout or invoking the allocator.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(n) => n as int == len as int + additional as int && n <= limit,
        Err(Error::Capacity) => len as int + additional as int > usize::MAX
            || len as int + additional as int > limit,
        _ => false,
    },
))]
pub(crate) fn required_len(len: usize, additional: usize, limit: usize) -> Result<usize, Error> {
    let needed = (Checked::new(len) + Checked::new(additional))
        .map_err(
            #[cfg_attr(verus_keep_ghost, verus_spec(error: Error => ensures error == Error::Capacity,))]
            |_error| Error::Capacity,
        )?
        .get();
    if needed > limit {
        return Err(Error::Capacity);
    }
    Ok(needed)
}
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires needed <= ceiling,
    ensures needed <= out <= ceiling,
))]
pub(crate) fn growth_capacity(
    current: usize,
    needed: usize,
    ceiling: usize,
    minimum: usize,
) -> usize {
    current
        .saturating_mul(2)
        .max(needed)
        .max(minimum)
        .min(ceiling)
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Vec<T, P, L> {
    fn empty(policy: P, limit: L) -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: Checked::new(0),
            cap: crate::capacity::Capacity::empty(limit.max_len()),
            policy,
            limit,
            marker: PhantomData,
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.len.get()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len.get() == 0
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap.get()
    }
    #[inline]
    pub fn max_len(&self) -> usize {
        self.limit.max_len()
    }
    #[inline]
    pub fn remaining_limit(&self) -> usize {
        crate::arithmetic::bounded::wrapping_sub(self.max_len(), self.len.get())
    }
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr.as_ptr()
    }
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: exactly len initialized elements, valid alignment even for empty/ZST storage.
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len.get()) }
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: exclusive borrow of the initialized prefix.
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len.get()) }
    }
    pub fn get<I: SliceIndex<[T]>>(&self, index: I) -> Result<&I::Output, Error> {
        self.as_slice().get(index).ok_or(Error::Bounds)
    }
    pub fn get_mut<I: SliceIndex<[T]>>(&mut self, index: I) -> Result<&mut I::Output, Error> {
        self.as_mut_slice().get_mut(index).ok_or(Error::Bounds)
    }
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }
    pub fn first(&self) -> Option<&T> {
        self.as_slice().first()
    }
    pub fn last(&self) -> Option<&T> {
        self.as_slice().last()
    }
    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.as_mut_slice().first_mut()
    }
    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.as_mut_slice().last_mut()
    }
    /// Layout of storage, not just initialized values.
    #[inline]
    fn layout(&self) -> Layout {
        self.cap.layout()
    }
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), Error> {
        // cap <= max_len also makes this fast path enforce the logical limit.
        let spare = (self.cap - self.len).map_err(|_| Error::Capacity)?;
        if additional > spare.get() {
            self.grow(additional, false)?;
        }
        Ok(())
    }
    #[inline(always)]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), Error> {
        let spare = (self.cap - self.len).map_err(|_| Error::Capacity)?;
        if additional > spare.get() {
            self.grow(additional, true)?;
        }
        Ok(())
    }
    #[inline]
    fn grow(&mut self, additional: usize, exact: bool) -> Result<(), Error> {
        let needed = required_len(self.len.get(), additional, self.max_len())?;
        self.grow_to(needed, exact)
    }
    /// Return the validated final length so bulk writes do not recompute an
    /// unchecked sum after transferring element ownership.
    #[inline]
    fn reserve_length(&mut self, additional: usize) -> Result<usize, Error> {
        let needed = required_len(self.len.get(), additional, self.max_len())?;
        if needed > self.cap.get() {
            self.grow_to(needed, false)?;
        }
        Ok(needed)
    }
    #[inline]
    fn grow_to(&mut self, needed: usize, exact: bool) -> Result<(), Error> {
        let cap = geometry::growth_for_layout(
            self.cap.get(),
            needed,
            self.max_len(),
            mem::size_of::<T>(),
            exact,
        )?;
        self.resize_storage(cap)
    }
    #[inline(always)]
    fn resize_storage(&mut self, cap: usize) -> Result<(), Error> {
        let (capacity, new_layout) = crate::capacity::Capacity::try_new(cap)?;
        // SAFETY: checked new layout, identical alignment, unique storage; caller
        // keeps all initialized elements and does not exceed the limit.
        let ptr = if self.cap.get() == 0 {
            storage::allocate(new_layout, self.policy.budget())?
        } else {
            // SAFETY: existing unique allocation, same alignment, initialized prefix retained.
            unsafe {
                storage::reallocate(
                    self.ptr.cast(),
                    self.layout(),
                    new_layout,
                    self.policy.budget(),
                )
            }?
        };
        self.ptr = ptr.cast();
        self.cap = capacity;
        Ok(())
    }
    pub fn try_shrink_to(&mut self, minimum_capacity: usize) -> Result<(), Error> {
        if mem::size_of::<T>() == 0 {
            return Ok(());
        }
        let cap = self.len.get().max(minimum_capacity);
        if cap < self.cap.get() {
            self.resize_storage(cap)?;
        }
        Ok(())
    }
    pub fn try_shrink_to_fit(&mut self) -> Result<(), Error> {
        self.try_shrink_to(self.len.get())
    }
    /// Refusal retains the exact pointer, capacity, contents and input value.
    #[inline]
    pub fn try_push(&mut self, value: T) -> Result<(), Rejected<T>> {
        if self.len.get() >= self.cap.get() {
            // The cold growth takes the storage words by value. Passing `&mut self`
            // would keep the owner in memory and store `len` on every push.
            // The new words come back through these locals: a returned
            // `Result<(NonNull<T>, Capacity<T>), Error>` exhausts the Kani solver.
            let (mut ptr, mut cap, len, max_len) =
                (self.ptr, self.cap, self.len.get(), self.max_len());
            let budget = self.policy.budget();
            // SAFETY: ptr and cap describe this owner's unique storage, charged to
            // this policy's accounting; both are replaced only on success.
            match unsafe { grow_for_push(&mut ptr, &mut cap, len, max_len, budget) } {
                Ok(()) => {
                    self.ptr = ptr;
                    self.cap = cap;
                }
                Err(error) => return Err(Rejected { value, error }),
            }
        }
        self.push_within_capacity(value).map(|_| ())
    }
    pub fn try_insert(&mut self, index: usize, value: T) -> Result<(), Rejected<T>> {
        let shifted = match geometry::insertion(self.len, index) {
            Ok(shifted) => shifted,
            Err(error) => return Err(Rejected { value, error }),
        };
        let needed = match self.reserve_length(1) {
            Ok(needed) => needed,
            Err(error) => return Err(Rejected { value, error }),
        };
        // SAFETY: index <= len, one spare slot; overlapping move transfers ownership.
        unsafe {
            let place = self.ptr.as_ptr().add(index);
            core::ptr::copy(place, place.add(1), shifted.get());
            place.write(value);
        }
        self.len = Checked::new(needed);
        Ok(())
    }
    pub fn pop(&mut self) -> Option<T> {
        let next = (self.len - Checked::new(1)).ok()?;
        self.len = next;
        // SAFETY: checked subtraction excludes empty storage, ownership removed once.
        Some(unsafe { self.ptr.as_ptr().add(next.get()).read() })
    }
    pub fn remove(&mut self, index: usize) -> Result<T, Error> {
        let (next, shifted) = geometry::removal(self.len, index)?;
        // SAFETY: index is initialized; shift remaining ownership left, excluding duplicate tail.
        unsafe {
            let place = self.ptr.as_ptr().add(index);
            let value = place.read();
            core::ptr::copy(place.add(1), place, shifted.get());
            self.len = next;
            Ok(value)
        }
    }
    pub fn swap_remove(&mut self, index: usize) -> Result<T, Error> {
        let next = geometry::shortened(self.len, index)?;
        self.len = next;
        // SAFETY: read removed value then transfer the former last value, including self-copy.
        unsafe {
            let place = self.ptr.as_ptr().add(index);
            let value = place.read();
            core::ptr::copy(self.ptr.as_ptr().add(self.len.get()), place, 1);
            Ok(value)
        }
    }
    pub fn truncate(&mut self, len: usize) {
        let next = Checked::new(len);
        let Some(removed) = geometry::removed_suffix(self.len, next) else {
            return;
        };
        self.len = next;
        if !mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: removed suffix is initialized; len is reduced before destructors can unwind.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr().add(len),
                removed.get(),
            ))
        };
    }
    pub fn clear(&mut self) {
        self.truncate(0);
    }
    #[inline]
    pub fn try_extend_copy(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: Copy,
    {
        let needed = self.reserve_length(source.len())?;
        // SAFETY: disjoint borrowed input and reserved unwritten suffix.
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr(),
                self.ptr.as_ptr().add(self.len.get()),
                source.len(),
            )
        };
        self.len = Checked::new(needed);
        Ok(())
    }
    pub fn try_append<Q: AllocationPolicy, M: LengthPolicy>(
        &mut self,
        other: &mut Vec<T, Q, M>,
    ) -> Result<(), Error> {
        let needed = self.reserve_length(other.len.get())?;
        // SAFETY: exclusive distinct owners; destination has enough unused slots.
        unsafe {
            core::ptr::copy_nonoverlapping(
                other.ptr.as_ptr(),
                self.ptr.as_ptr().add(self.len.get()),
                other.len.get(),
            )
        };
        self.len = Checked::new(needed);
        other.len = Checked::new(0);
        Ok(())
    }
    /// Heap append may reuse the larger allocation within one accounting
    /// domain and equal limits. Preserve the actual policy/limit handles.
    /// Reserve before exchange, so refusal leaves both owners unchanged.
    pub(crate) fn try_reuse_larger_buffer(&mut self, other: &mut Self) -> Result<bool, Error> {
        if self.len() >= other.len()
            || self.max_len() != other.max_len()
            || !self.policy.budget().is_metered()
            || !self.policy.budget().same_domain(other.policy.budget())
        {
            return Ok(false);
        }
        let needed = required_len(self.len(), other.len(), self.max_len())?;
        if other.capacity() < needed {
            // Do not introduce allocation where the original receiver already
            // has enough storage. Otherwise exact growth reserves no more than
            // the original receiver's geometric growth would have requested.
            if self.capacity() >= needed {
                return Ok(false);
            }
            other.try_reserve_exact(self.len())?;
        }
        // No callback, allocation or destructor runs between these typed swaps.
        // On completion every pointer keeps its matching length and capacity.
        mem::swap(&mut self.ptr, &mut other.ptr);
        mem::swap(&mut self.len, &mut other.len);
        mem::swap(&mut self.cap, &mut other.cap);
        Ok(true)
    }
    pub fn try_split_off(&mut self, at: usize) -> Result<Self, Error> {
        if at > self.len.get() {
            return Err(Error::Bounds);
        }
        let len = (self.len - Checked::new(at))
            .map_err(|_| Error::Bounds)?
            .get();
        let mut tail = Self::empty(self.policy.try_clone()?, self.limit);
        tail.try_reserve_exact(len)?;
        // SAFETY: fresh disjoint storage; transfer the initialized suffix once.
        unsafe {
            core::ptr::copy_nonoverlapping(self.ptr.as_ptr().add(at), tail.ptr.as_ptr(), len)
        };
        tail.len = Checked::new(len);
        self.len = Checked::new(at);
        Ok(tail)
    }
    /// Growth initializes directly in destination slots. On error, original values
    /// and length survive; a successful reservation may have increased capacity.
    pub fn try_resize_with_init<E>(
        &mut self,
        new_len: usize,
        mut initialize: impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<(), InitError<E>> {
        if new_len <= self.len.get() {
            self.truncate(new_len);
            return Ok(());
        }
        let additional =
            (Checked::new(new_len) - self.len).map_err(|_| InitError::Storage(Error::Capacity))?;
        self.try_reserve(additional.get())
            .map_err(InitError::Storage)?;
        let original = self.len.get();
        let guard = SuffixGuard {
            owner: self,
            original,
        };
        #[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(original <= guard.owner.len() && guard.owner.len() <= new_len))]
        while guard.owner.len.get() < new_len {
            let index = guard.owner.len.get();
            let next = (guard.owner.len + Checked::new(1))
                .map_err(|_| InitError::Storage(Error::Capacity))?;
            // SAFETY: reserved exclusive unwritten element; slot records initialization.
            let ptr = unsafe { NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(index)) };
            let mut slot = unsafe { InitSlot::new(ptr) };
            initialize(index, &mut slot).map_err(InitError::Initialize)?;
            slot.finish().map_err(InitError::Storage)?;
            guard.owner.len = next;
        }
        mem::forget(guard);
        Ok(())
    }
    pub fn try_resize_with<E>(
        &mut self,
        new_len: usize,
        mut initialize: impl FnMut() -> Result<T, E>,
    ) -> Result<(), InitError<E>> {
        self.try_resize_with_init(new_len, |_, slot| {
            let value = initialize().map_err(InitError::Initialize)?;
            slot.write(value).map_err(|e| InitError::Storage(e.error))
        })
        .map_err(flatten_init)
    }
    /// Borrow the repeated value; no owned input is lost on refusal.
    pub fn try_resize(&mut self, new_len: usize, value: &T) -> Result<(), Error>
    where
        T: TryClone,
    {
        self.try_resize_with_init(new_len, |_, slot| value.try_clone_into(slot))
            .map_err(flatten_error)
    }
    #[inline]
    pub fn try_extend_cloned(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: TryClone,
    {
        let end = required_len(self.len.get(), source.len(), self.max_len())?;
        if let Some(result) = T::try_extend_vec(source, self) {
            return result;
        }
        if T::copy_witness().is_some() {
            // Bitwise-clonable values without their own bulk hook (arrays of
            // primitives) are copied once instead of initializing each element
            // through a slot (one call per element and per array item).
            let needed = self.reserve_length(source.len())?;
            // SAFETY: the private witness permits bitwise duplication of the
            // initialized source; the borrowed input is disjoint from the
            // reserved unwritten suffix, which then becomes initialized.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    source.as_ptr(),
                    self.ptr.as_ptr().add(self.len.get()),
                    source.len(),
                )
            };
            self.len = Checked::new(needed);
            return Ok(());
        }
        let start = self.len.get();
        self.try_resize_with_init(end, |index, slot| {
            // SAFETY: initialization indexes range from start to end; source has end-start elements.
            let offset = (Checked::new(index) - Checked::new(start))
                .map_err(|_| Error::Bounds)?
                .get();
            unsafe { source.get_unchecked(offset) }.try_clone_into(slot)
        })
        .map_err(flatten_error)
    }
    /// Transactional suffix extension. Input already yielded (including an item
    /// rejected at the limit) is consumed. Iterator side effects are not rolled back.
    /// Hints are not used as a reason to reject an otherwise finite fitting input.
    pub fn try_extend_fallible<E>(
        &mut self,
        iter: impl crate::TryIntoIterator<Item = T, Error = E>,
    ) -> Result<(), InitError<E>> {
        let mut iter = iter.try_into_iter().map_err(InitError::Initialize)?;
        let original = self.len.get();
        let guard = SuffixGuard {
            owner: self,
            original,
        };
        while let Some(item) =
            crate::TryIterator::try_next(&mut iter).map_err(InitError::Initialize)?
        {
            guard
                .owner
                .try_push(item)
                .map_err(|e| InitError::Storage(e.error))?;
        }
        mem::forget(guard);
        Ok(())
    }
    pub fn try_extend(&mut self, iter: impl IntoIterator<Item = T>) -> Result<(), Error> {
        self.try_extend_fallible(crate::iter::from_std(iter))
            .map_err(|error| match error {
                InitError::Storage(error) => error,
                InitError::Initialize(never) => match never {},
            })
    }
}
fn flatten_error(e: InitError<Error>) -> Error {
    match e {
        InitError::Storage(e) | InitError::Initialize(e) => e,
    }
}
fn flatten_init<E>(e: InitError<InitError<E>>) -> InitError<E> {
    match e {
        InitError::Storage(e) => InitError::Storage(e),
        InitError::Initialize(e) => e,
    }
}
struct SuffixGuard<'a, T, P: AllocationPolicy, L: LengthPolicy> {
    owner: &'a mut Vec<T, P, L>,
    original: usize,
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for SuffixGuard<'_, T, P, L> {
    fn drop(&mut self) {
        self.owner.truncate(self.original);
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Vec<T, P, L> {
    #[inline]
    fn drop(&mut self) {
        struct Allocation<'a>(NonNull<u8>, Layout, Accounting<'a>);
        impl Drop for Allocation<'_> {
            #[inline]
            fn drop(&mut self) {
                // SAFETY: owner supplied unique allocation and validated capacity layout.
                unsafe { storage::deallocate(self.0, self.1, self.2) };
            }
        }
        let _allocation = Allocation(self.ptr.cast(), self.layout(), self.policy.budget());
        if !mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: initialized prefix, released before allocation guard even on unwind.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr(),
                self.len.get(),
            ))
        };
    }
}
impl<T: TryClone, P: AllocationPolicy, L: LengthPolicy> TryClone for Vec<T, P, L> {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        let mut copy = Self::empty(self.policy.try_clone()?, self.limit);
        copy.try_reserve_exact(self.len.get())?;
        // Reservation preserves the empty initialized prefix. Restate that
        // postcondition after the possibly outlined allocation helper so LLVM
        // can remove source-offset checks from this fresh-owner copy.
        copy.len = Checked::new(0);
        copy.try_extend_cloned(self.as_slice())?;
        Ok(copy)
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        required_len(0, source.len.get(), self.max_len())?;
        if self.len.get() == source.len.get()
            && let Some(result) = T::try_copy_slice_from(source.as_slice(), self.as_mut_slice())
        {
            return result;
        }
        let mut replacement = Self::empty(self.policy.try_clone()?, self.limit);
        replacement.try_reserve_exact(source.len.get())?;
        replacement.try_extend_cloned(source.as_slice())?;
        mem::swap(self, &mut replacement);
        Ok(())
    }
}
impl<'a, T: TryCloneIn<'a>, P: AllocationPolicy, L: LengthPolicy> TryCloneIn<'a> for Vec<T, P, L> {
    type Output = Vec<T::Output, Budget<'a>, L>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        let mut copy = Vec::empty(budget, self.limit);
        copy.try_reserve_exact(self.len.get())?;
        copy.try_resize_with_init(self.len.get(), |i, slot| {
            // SAFETY: initialization covers exactly the source's initialized length.
            unsafe { self.as_slice().get_unchecked(i) }.try_clone_into_in(budget, slot)
        })
        .map_err(flatten_error)?;
        Ok(copy)
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> AsRef<[T]> for Vec<T, P, L> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> AsMut<[T]> for Vec<T, P, L> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: core::fmt::Debug, P: AllocationPolicy, L: LengthPolicy> core::fmt::Debug for Vec<T, P, L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, Q: AllocationPolicy, L: LengthPolicy, M: LengthPolicy>
    PartialEq<Vec<U, Q, M>> for Vec<T, P, L>
{
    fn eq(&self, other: &Vec<U, Q, M>) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl<T: Eq, P: AllocationPolicy, L: LengthPolicy> Eq for Vec<T, P, L> {}
impl<T: core::hash::Hash, P: AllocationPolicy, L: LengthPolicy> core::hash::Hash for Vec<T, P, L> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

/// Compile-time boundaries for checked access, sharing, and allocation.
/// ```compile_fail
/// let v = jzap::vec::Vec::<u8>::new();
/// let _ = v[0];
/// ```
/// ```compile_fail
/// let v = jzap::vec::Vec::<u8>::new();
/// let _ = v.clone();
/// ```
/// ```compile_fail
/// fn send<T: Send>(_: T) {}
/// let budget = jzap::budget::MemoryBudget::new(32);
/// let v = jzap::vec::Vec::<u8, _>::with_limit_in(4, jzap::budget::Budget::Limited(&budget));
/// send(v);
/// ```
/// ```compile_fail
/// let v;
/// { let budget = jzap::budget::MemoryBudget::new(32);
///   v = jzap::vec::Vec::<u8, _>::with_limit_in(4, jzap::budget::Budget::Limited(&budget)); }
/// drop(v);
/// ```
/// ```compile_fail
/// let mut v = jzap::vec::Vec::try_from_array([1,2]).unwrap();
/// let mut drain = v.try_drain(..).unwrap();
/// v.try_push(3).unwrap();
/// let _ = drain.next();
/// ```
/// ```compile_fail
/// let mut v = jzap::vec::Vec::<u8>::try_with_capacity(4).unwrap();
/// v.try_set_len(4).unwrap(); // Initialization proof requires an unsafe caller.
/// ```
const _: () = ();

#[cfg(kani)]
#[path = "vec/metadata_proofs.rs"]
mod metadata_proofs;

/// The storage transition of `Vec::grow(1, false)` on the pointer and capacity
/// words: the same length admission, growth proposal and layout, then a fresh
/// allocation or a reallocation. Refusal leaves the caller's words unchanged.
///
/// # Safety
/// `ptr` and `cap` are a live owner's unique storage (dangling when `cap` is
/// zero), charged to `budget`. Only success writes the new words; the caller
/// then replaces its own with them and must not use the old pointer.
#[cold]
#[inline(never)]
unsafe fn grow_for_push<T>(
    ptr: &mut NonNull<T>,
    cap: &mut crate::capacity::Capacity<T>,
    len: usize,
    max_len: usize,
    budget: crate::budget::Accounting<'_>,
) -> Result<(), Error> {
    let needed = required_len(len, 1, max_len)?;
    let grown =
        geometry::growth_for_layout(cap.get(), needed, max_len, mem::size_of::<T>(), false)?;
    let (capacity, new_layout) = crate::capacity::Capacity::try_new(grown)?;
    let new = if cap.get() == 0 {
        storage::allocate(new_layout, budget)?
    } else {
        // SAFETY: caller's unique allocation, same alignment, initialized prefix retained.
        unsafe { storage::reallocate(ptr.cast(), cap.layout(), new_layout, budget) }?
    };
    *ptr = new.cast();
    *cap = capacity;
    Ok(())
}
