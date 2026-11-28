//! A fallible ring buffer with explicit allocation, length and callback failures.
use crate::{
    Checked, Error, Rejected, TryClone, TryCloneIn,
    arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub},
    budget::{Accounting, AllocationPolicy, Budget, Unmetered},
    capacity::Capacity,
    storage,
    vec::{LengthLimit, LengthPolicy, Unlimited, Vec},
};
use core::{
    marker::PhantomData,
    mem,
    ops::RangeBounds,
    ptr::{self, NonNull},
    slice,
};
mod edit;
pub(crate) mod geometry;
mod iter;
#[cfg(kani)]
mod proofs;
mod traits;
pub use iter::{Drain, IntoIter, Iter, IterMut};
pub use traits::ExtendError;

pub struct VecDeque<T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    ptr: NonNull<T>,
    cap: Capacity<T>,
    head: Checked<usize>,
    len: Checked<usize>,
    policy: P,
    limit: L,
    marker: PhantomData<T>,
}
// SAFETY: unique ring storage; all transferred/shared elements and policies obey these bounds.
unsafe impl<T: Send, P: AllocationPolicy + Send, L: LengthPolicy + Send> Send
    for VecDeque<T, P, L>
{
}
unsafe impl<T: Sync, P: AllocationPolicy + Sync, L: LengthPolicy + Sync> Sync
    for VecDeque<T, P, L>
{
}

impl<T> VecDeque<T> {
    pub const fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            cap: Capacity::empty(usize::MAX),
            head: Checked::new(0),
            len: Checked::new(0),
            policy: Unmetered,
            limit: Unlimited,
            marker: PhantomData,
        }
    }
    pub fn try_with_capacity(capacity: usize) -> Result<Self, Error> {
        Self::try_with_capacity_in(capacity, Unmetered)
    }
    pub fn with_limit(max: usize) -> VecDeque<T, Unmetered, LengthLimit> {
        Self::with_limit_in(max, Unmetered)
    }
    pub fn try_with_capacity_and_limit(
        cap: usize,
        max: usize,
    ) -> Result<VecDeque<T, Unmetered, LengthLimit>, Error> {
        Self::try_with_capacity_and_limit_in(cap, max, Unmetered)
    }
    pub fn try_from_std(
        source: alloc::collections::VecDeque<T>,
    ) -> Result<Self, Rejected<alloc::collections::VecDeque<T>>> {
        Self::try_from_std_in(source, Unmetered, Unlimited)
    }
}
impl<T, P: AllocationPolicy> VecDeque<T, P> {
    pub fn new_in(policy: P) -> Self {
        Self::empty(policy, Unlimited)
    }
    pub fn with_limit_in(max: usize, policy: P) -> VecDeque<T, P, LengthLimit> {
        VecDeque::empty(policy, LengthLimit::new(max))
    }
    pub fn try_with_capacity_in(cap: usize, policy: P) -> Result<Self, Error> {
        let mut value = Self::new_in(policy);
        value.try_reserve_exact(cap)?;
        Ok(value)
    }
    pub fn try_with_capacity_and_limit_in(
        cap: usize,
        max: usize,
        policy: P,
    ) -> Result<VecDeque<T, P, LengthLimit>, Error> {
        let mut value = Self::with_limit_in(max, policy);
        value.try_reserve_exact(cap)?;
        Ok(value)
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> VecDeque<T, P, L> {
    fn empty(policy: P, limit: L) -> Self {
        Self {
            ptr: NonNull::dangling(),
            cap: Capacity::empty(limit.max_len()),
            head: Checked::new(0),
            len: Checked::new(0),
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
        self.len() == 0
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap.get()
    }
    pub fn max_len(&self) -> usize {
        self.limit.max_len()
    }
    #[cfg(kani)]
    fn probe_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }
    pub fn allocation_policy(&self) -> &P {
        &self.policy
    }
    pub fn remaining_limit(&self) -> usize {
        sub(self.max_len(), self.len())
    }
    #[inline]
    fn physical(&self, index: usize) -> usize {
        if mem::size_of::<T>() == 0 {
            0
        } else {
            geometry::allocated_physical(self.head.get(), index, self.capacity())
        }
    }
    /// Caller supplies a logical slot within this allocation; initialization is separate.
    #[inline]
    unsafe fn slot(&self, index: usize) -> *mut T {
        // SAFETY: physical maps the admitted slot within the same allocation (or ZST dangling).
        unsafe { self.ptr.as_ptr().add(self.physical(index)) }
    }
    #[inline]
    pub fn as_slices(&self) -> (&[T], &[T]) {
        let (a, b) = geometry::split(self.head.get(), self.len(), self.capacity());
        // SAFETY: the two ranges contain exactly the initialized ring, with one shared owner.
        unsafe {
            (
                slice::from_raw_parts(self.ptr.as_ptr().add(self.head.get()), a),
                slice::from_raw_parts(self.ptr.as_ptr(), b),
            )
        }
    }
    #[inline]
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        let (a, b) = geometry::split(self.head.get(), self.len(), self.capacity());
        // SAFETY: distinct initialized ranges under an exclusive borrow; ZST second range is empty.
        unsafe {
            (
                slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.head.get()), a),
                slice::from_raw_parts_mut(self.ptr.as_ptr(), b),
            )
        }
    }
    pub fn get(&self, index: usize) -> Result<&T, Error> {
        if index >= self.len() {
            return Err(Error::Bounds);
        }
        // SAFETY: checked live logical index, shared borrow.
        Ok(unsafe { &*self.slot(index) })
    }
    pub fn get_mut(&mut self, index: usize) -> Result<&mut T, Error> {
        if index >= self.len() {
            return Err(Error::Bounds);
        }
        // SAFETY: checked live logical index, exclusive borrow.
        Ok(unsafe { &mut *self.slot(index) })
    }
    pub fn front(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }
        // SAFETY: nonempty ring owns head directly; ZST head is canonical zero.
        Some(unsafe { &*self.ptr.as_ptr().add(self.head.get()) })
    }
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.is_empty() {
            return None;
        }
        // SAFETY: initialized head under the exclusive owner borrow.
        Some(unsafe { &mut *self.ptr.as_ptr().add(self.head.get()) })
    }
    pub fn back(&self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            self.get(sub(self.len(), 1)).ok()
        }
    }
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if self.is_empty() {
            None
        } else {
            self.get_mut(sub(self.len(), 1)).ok()
        }
    }
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), Error> {
        if additional > sub(self.capacity(), self.len()) {
            self.grow(additional, false)?
        }
        Ok(())
    }
    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), Error> {
        if additional > sub(self.capacity(), self.len()) {
            self.grow(additional, true)?
        }
        Ok(())
    }
    #[cold]
    #[inline(never)]
    fn grow(&mut self, additional: usize, exact: bool) -> Result<(), Error> {
        let needed = crate::vec::required_len(self.len(), additional, self.max_len())?;
        let size = mem::size_of::<T>();
        let ceiling = self.max_len().min(
            (isize::MAX as usize)
                .checked_div(size)
                .unwrap_or(usize::MAX),
        );
        if needed > ceiling {
            return Err(Error::Capacity);
        }
        let minimum = if size == 1 {
            8
        } else if size <= 1024 {
            4
        } else {
            1
        };
        let new = if exact {
            needed
        } else {
            crate::vec::growth_capacity(self.capacity(), needed, ceiling, minimum)
        };
        let (cap, layout) = Capacity::try_new(new)?;
        let old = self.capacity();
        // SAFETY: uniquely owned global allocation, matching layouts and accounting. Refusal preserves it.
        let ptr = unsafe {
            storage::reallocate(
                self.ptr.cast(),
                self.cap.layout(),
                layout,
                self.policy.budget(),
            )?
        }
        .cast::<T>();
        if old > 0 {
            let head = geometry::grown_head(self.head.get(), self.len(), old, new);
            let (a, b) = geometry::split(self.head.get(), self.len(), old);
            if b > 0 {
                // SAFETY: reservation succeeded; the selected segment's destination fits new capacity.
                // Old duplicated bytes are unowned after publishing the new ring.
                unsafe {
                    if head == self.head.get() {
                        ptr::copy_nonoverlapping(ptr.as_ptr(), ptr.as_ptr().add(old), b)
                    } else {
                        ptr::copy(ptr.as_ptr().add(self.head.get()), ptr.as_ptr().add(head), a)
                    }
                }
            }
            self.head = Checked::new(head);
        }
        self.ptr = ptr;
        self.cap = cap;
        Ok(())
    }
    #[inline]
    pub fn try_push_back(&mut self, value: T) -> Result<(), Rejected<T>> {
        let len = self.len();
        if len == self.capacity()
            && let Err(error) = self.grow(1, false)
        {
            return Err(Rejected { value, error });
        }
        // SAFETY: successful admission supplies the unique spare slot.
        unsafe { self.slot(len).write(value) };
        self.len = Checked::new(add(len, 1));
        Ok(())
    }
    #[inline]
    pub fn try_push_front(&mut self, value: T) -> Result<(), Rejected<T>> {
        if self.len() == self.capacity()
            && let Err(error) = self.grow(1, false)
        {
            return Err(Rejected { value, error });
        }
        if mem::size_of::<T>() != 0 {
            self.head = Checked::new(geometry::retreat(self.head.get(), 1, self.capacity()))
        }
        self.len = Checked::new(add(self.len(), 1));
        // SAFETY: admission established the unique spare head; publishing length
        // and writing have no intervening allocation, callback or failure.
        unsafe { self.ptr.as_ptr().add(self.head.get()).write(value) };
        Ok(())
    }
    #[inline]
    pub fn try_push_back_mut(&mut self, value: T) -> Result<&mut T, Rejected<T>> {
        self.try_push_back(value)?;
        // SAFETY: successful push establishes a live last element and exclusive borrow.
        Ok(unsafe { &mut *self.slot(sub(self.len(), 1)) })
    }
    #[inline]
    pub fn try_push_front_mut(&mut self, value: T) -> Result<&mut T, Rejected<T>> {
        self.try_push_front(value)?;
        // SAFETY: successful push establishes the initialized head, exclusively borrowed.
        Ok(unsafe { &mut *self.ptr.as_ptr().add(self.head.get()) })
    }
    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let old_head = self.head.get();
        self.len = Checked::new(geometry::after_pop(self.len(), self.capacity()));
        if mem::size_of::<T>() != 0 {
            self.head = Checked::new(geometry::allocated_physical(
                self.head.get(),
                1,
                self.capacity(),
            ))
        }
        // SAFETY: after_pop proves spare capacity for every valid nonempty owner.
        // This is the same optimizer fact as Rust 1.95 VecDeque::pop_front.
        let spare = self.len() < self.capacity();
        #[cfg(kani)]
        assert!(spare, "verified pop spare capacity");
        unsafe { core::hint::assert_unchecked(spare) };
        // SAFETY: retire the old initialized head once, then move out its value.
        Some(unsafe { self.ptr.as_ptr().add(old_head).read() })
    }
    #[inline]
    pub fn pop_back(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let last = geometry::after_pop(self.len(), self.capacity());
        self.len = Checked::new(last);
        // SAFETY: the directly verified after_pop contract proves this predicate.
        let spare = self.len() < self.capacity();
        #[cfg(kani)]
        assert!(spare, "verified pop spare capacity");
        unsafe { core::hint::assert_unchecked(spare) };
        // SAFETY: old last remains live after shortening, ownership transfers once.
        Some(unsafe { self.slot(last).read() })
    }
    pub fn try_pop_front_if<E>(
        &mut self,
        predicate: impl FnOnce(&mut T) -> Result<bool, E>,
    ) -> Result<Option<T>, E> {
        let Some(v) = self.front_mut() else {
            return Ok(None);
        };
        if predicate(v)? {
            Ok(self.pop_front())
        } else {
            Ok(None)
        }
    }
    pub fn try_pop_back_if<E>(
        &mut self,
        predicate: impl FnOnce(&mut T) -> Result<bool, E>,
    ) -> Result<Option<T>, E> {
        let Some(v) = self.back_mut() else {
            return Ok(None);
        };
        if predicate(v)? {
            Ok(self.pop_back())
        } else {
            Ok(None)
        }
    }
    pub fn clear(&mut self) {
        self.truncate(0)
    }
    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        if !mem::needs_drop::<T>() {
            self.len = Checked::new(len);
            return;
        }
        while self.len() > len {
            drop(self.pop_back())
        }
    }
    /// Rearrange only initialized elements, without allocating or invoking callbacks.
    /// Algorithm adapted from Rust alloc VecDeque (MIT OR Apache-2.0), pinned in the design document.
    pub fn make_contiguous(&mut self) -> &mut [T] {
        let cap = self.capacity();
        let len = self.len();
        let head = self.head.get();
        let (a, b) = geometry::split(head, len, cap);
        let kind = geometry::contiguous_kind(head, len, cap);
        if kind != 0 {
            let free = sub(cap, len);
            let p = self.ptr.as_ptr();
            // SAFETY: each move preserves the two disjoint live segments; only the resulting
            // initialized slice is rotated. The rotation argument is strictly below its length.
            unsafe {
                if kind == 1 {
                    ptr::copy(p, p.add(a), b);
                    ptr::copy_nonoverlapping(p.add(head), p, a);
                    self.head = Checked::new(0)
                } else if kind == 2 {
                    ptr::copy(p.add(head), p.add(b), a);
                    ptr::copy_nonoverlapping(p, p.add(add(b, a)), b);
                    self.head = Checked::new(b)
                } else if kind == 3 {
                    if free != 0 {
                        ptr::copy(p, p.add(free), b)
                    }
                    slice::from_raw_parts_mut(p.add(free), len).rotate_left(b);
                    self.head = Checked::new(free);
                } else {
                    if free != 0 {
                        ptr::copy(p.add(head), p.add(b), a)
                    }
                    slice::from_raw_parts_mut(p, len).rotate_right(a);
                    self.head = Checked::new(0);
                }
            }
        }
        // SAFETY: now the initialized ring occupies one contiguous range.
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.head.get()), len) }
    }
    fn normalize(&mut self) {
        self.make_contiguous();
        if self.head.get() != 0 {
            // SAFETY: move the contiguous initialized slice to the allocation prefix, overlap allowed.
            unsafe {
                ptr::copy(
                    self.ptr.as_ptr().add(self.head.get()),
                    self.ptr.as_ptr(),
                    self.len(),
                )
            }
            self.head = Checked::new(0);
        }
    }
    pub fn try_shrink_to_fit(&mut self) -> Result<(), Error> {
        self.try_shrink_to(self.len())
    }
    pub fn try_shrink_to(&mut self, min: usize) -> Result<(), Error> {
        let new = min.max(self.len());
        if mem::size_of::<T>() == 0 || new >= self.capacity() {
            return Ok(());
        }
        let (cap, layout) = Capacity::try_new(new)?;
        let head = self.head.get();
        let len = self.len();
        let (a, b) = geometry::split(head, len, self.capacity());
        let moved = head > sub(new, len);
        if moved {
            self.normalize()
        }
        // SAFETY: live positions all fit new layout, accounting belongs to this owner.
        match unsafe {
            storage::reallocate(
                self.ptr.cast(),
                self.cap.layout(),
                layout,
                self.policy.budget(),
            )
        } {
            Ok(ptr) => {
                self.ptr = ptr.cast();
                self.cap = cap;
                // An empty ring may keep `head == new`; reset it so `head < cap` holds.
                if moved || len == 0 {
                    self.head = Checked::new(0)
                };
                Ok(())
            }
            Err(e) => {
                if moved {
                    // SAFETY: failed realloc retains original allocation. Reconstruct its exact old
                    // split from initialized prefix without allocation/callback, then restore head.
                    unsafe {
                        if b == 0 {
                            ptr::copy(self.ptr.as_ptr(), self.ptr.as_ptr().add(head), len)
                        } else {
                            slice::from_raw_parts_mut(self.ptr.as_ptr(), len).rotate_left(a);
                            ptr::copy(self.ptr.as_ptr().add(b), self.ptr.as_ptr().add(head), a);
                        }
                    }
                    self.head = Checked::new(head);
                }
                Err(e)
            }
        }
    }
    pub fn from_vec(source: Vec<T, P, L>) -> Self {
        let (ptr, len, cap, policy, limit) = source.into_raw_parts();
        // SAFETY: exact allocation/layout/policy transfer from unique contiguous owner.
        Self {
            ptr,
            len: Checked::new(len),
            head: Checked::new(0),
            cap: unsafe { Capacity::from_validated_raw(cap) },
            policy,
            limit,
            marker: PhantomData,
        }
    }
    pub fn into_vec(mut self) -> Vec<T, P, L> {
        self.normalize();
        let owner = mem::ManuallyDrop::new(self);
        // SAFETY: disarm once, transfer same initialized prefix, allocation, limit and charge.
        unsafe {
            Vec::from_raw_parts(
                owner.ptr,
                owner.len(),
                owner.capacity(),
                ptr::read(&owner.policy),
                owner.limit,
            )
        }
    }
    /// End the jzap policy contract, transferring the global allocation without allocation.
    pub fn into_std(self) -> alloc::collections::VecDeque<T> {
        self.into_vec().into_std().into()
    }
    /// std's Deque-to-Vec adapter only rearranges the existing allocation (Rust 1.95).
    /// Admission occurs before rearrangement, so refusal returns the original deque.
    pub fn try_from_std_in(
        source: alloc::collections::VecDeque<T>,
        policy: P,
        limit: L,
    ) -> Result<Self, Rejected<alloc::collections::VecDeque<T>>> {
        let cap = if mem::size_of::<T>() == 0 {
            limit.max_len()
        } else {
            source.capacity()
        };
        if source.len() > limit.max_len() || cap > limit.max_len() {
            return Err(Rejected {
                value: source,
                error: Error::Capacity,
            });
        }
        let (_, layout) = match Capacity::<T>::try_new(cap) {
            Ok(x) => x,
            Err(error) => {
                return Err(Rejected {
                    value: source,
                    error,
                });
            }
        };
        {
            let reservation = match policy.budget().reserve(layout.size()) {
                Ok(r) => r,
                Err(error) => {
                    return Err(Rejected {
                        value: source,
                        error,
                    });
                }
            };
            if let Some(r) = reservation {
                r.commit().retain()
            }
        }
        let mut vec = mem::ManuallyDrop::new(alloc::vec::Vec::from(source));
        // SAFETY: std adapter preserves global allocation, initialized prefix; charge just admitted.
        let owner = unsafe {
            Vec::from_raw_parts(
                NonNull::new_unchecked(vec.as_mut_ptr()),
                vec.len(),
                cap,
                policy,
                limit,
            )
        };
        Ok(Self::from_vec(owner))
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for VecDeque<T, P, L> {
    fn drop(&mut self) {
        struct Allocation<'a>(NonNull<u8>, core::alloc::Layout, Accounting<'a>);
        impl Drop for Allocation<'_> {
            fn drop(&mut self) {
                // SAFETY: unique matching global allocation and accounting transferred by the owner.
                unsafe { storage::deallocate(self.0, self.1, self.2) }
            }
        }
        let _allocation = Allocation(self.ptr.cast(), self.cap.layout(), self.policy.budget());
        if !mem::needs_drop::<T>() {
            return;
        }
        let (a, b) = geometry::split(self.head.get(), self.len(), self.capacity());
        struct Tail<T>(*mut [T]);
        impl<T> Drop for Tail<T> {
            fn drop(&mut self) {
                // SAFETY: the second disjoint live segment, retired once even if the first unwinds.
                unsafe { ptr::drop_in_place(self.0) }
            }
        }
        let _tail = Tail(ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), b));
        // SAFETY: first live segment. Tail and allocation guards handle unwind cleanup.
        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr().add(self.head.get()),
                a,
            ))
        }
    }
}
