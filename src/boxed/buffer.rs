//! Private growable storage for Box collection, never exposed as an owning Vec API.
use super::*;
use crate::{Checked, Error, TryIterator};
pub(super) struct Buffer<T, P: AllocationPolicy> {
    ptr: NonNull<T>,
    len: Checked<usize>,
    cap: Checked<usize>,
    layout: Layout,
    policy: P,
}
impl<T, P: AllocationPolicy> Buffer<T, P> {
    pub(super) fn new(policy: P) -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: Checked::new(0),
            cap: Checked::new(0),
            layout: Layout::new::<[T; 0]>(),
            policy,
        }
    }
    #[inline]
    fn reserve(&mut self, additional: usize, hint: usize) -> Result<usize, Error> {
        let (needed, proposed) =
            super::buffer_numeric::reservation(self.len.get(), self.cap.get(), additional, hint)?;
        if needed <= self.cap.get() {
            return Ok(needed);
        }
        let layout = crate::layout::array::<T>(proposed)?;
        // SAFETY: unique storage, no references into it survive this call; all
        // initialized elements fit the new allocation and retain ownership.
        let ptr = unsafe {
            storage::reallocate(self.ptr.cast(), self.layout, layout, self.policy.budget())
        }?;
        self.ptr = ptr.cast();
        self.cap = Checked::new(proposed);
        self.layout = layout;
        Ok(needed)
    }
    #[inline]
    pub(super) fn push(&mut self, value: T, hint: usize) -> Result<(), Error> {
        let needed = self.reserve(1, hint)?;
        // SAFETY: reserve checked len+1 and allocated the still unwritten slot.
        unsafe { self.ptr.as_ptr().add(self.len.get()).write(value) };
        self.len = Checked::new(needed);
        Ok(())
    }
    #[inline]
    pub(super) fn append(&mut self, values: &[T]) -> Result<(), Error>
    where
        T: Copy,
    {
        let needed = self.reserve(values.len(), values.len())?;
        // SAFETY: validated byte bounds, fresh destination capacity, disjoint input.
        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr(),
                self.ptr.as_ptr().add(self.len.get()),
                values.len(),
            )
        };
        self.len = Checked::new(needed);
        Ok(())
    }
    #[inline]
    pub(super) fn finish(mut self) -> Result<Box<[T], P>, Error> {
        if self.cap.get() != self.len.get() {
            let layout = crate::layout::array::<T>(self.len.get())?;
            // SAFETY: discard only spare capacity, preserve all initialized values.
            let ptr = unsafe {
                storage::reallocate(self.ptr.cast(), self.layout, layout, self.policy.budget())
            }?;
            self.ptr = ptr.cast();
            self.layout = layout;
        }
        let ptr = NonNull::slice_from_raw_parts(self.ptr, self.len.get());
        let owner = core::mem::ManuallyDrop::new(self);
        // SAFETY: the disarmed buffer transfers its single policy owner.
        let policy = unsafe { core::ptr::read(&owner.policy) };
        // SAFETY: guards transferred exact, fully initialized allocation and charge.
        Ok(unsafe { Box::from_raw_parts(ptr, policy) })
    }
}
impl<T, P: AllocationPolicy> Drop for Buffer<T, P> {
    fn drop(&mut self) {
        let _allocation = AllocationGuard(self.ptr.cast(), self.layout, self.policy.budget());
        if !core::mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: only initialized prefix is dropped; allocation guard uses capacity layout.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr(),
                self.len.get(),
            ))
        }
    }
}

impl<T, P: AllocationPolicy> Buffer<T, P> {
    /// Fill already-reserved spans, keeping growth out of the element loop.
    /// Size hints are allocation hints, never trusted for safety. The private
    /// end bound limits writes independently of the source's iterator methods.
    #[inline]
    pub(super) fn collect<E>(
        &mut self,
        mut iter: impl TryIterator<Item = T, Error = E>,
    ) -> Result<(), InitError<E>> {
        let lower = iter.try_size_hint().map_err(InitError::Initialize)?.0;
        let reserved_end = self.reserve(lower, lower).map_err(InitError::Storage)?;
        // SAFETY: reserve admitted this exclusive unwritten span and no source
        // callback can access or replace the buffer's private allocation.
        if !unsafe {
            crate::init::collection::fill(self.ptr, &mut self.len, reserved_end, &mut iter)
        }
        .map_err(InitError::Initialize)?
        {
            return Ok(());
        }
        let Some(first) = iter.try_next().map_err(InitError::Initialize)? else {
            return Ok(());
        };
        self.collect_rest(first, iter)
    }
    // Keep growth and realloc paths outside the initial, newly allocated span.
    // This also preserves the allocation's no-alias information in the common loop.
    #[inline(never)]
    fn collect_rest<E>(
        &mut self,
        first: T,
        mut iter: impl TryIterator<Item = T, Error = E>,
    ) -> Result<(), InitError<E>> {
        self.push(
            first,
            iter.try_size_hint()
                .map_err(InitError::Initialize)?
                .0
                .saturating_add(1),
        )
        .map_err(InitError::Storage)?;
        loop {
            // SAFETY: initialized length is bounded by private capacity;
            // the source is borrowed and writes stay inside this admitted span.
            if !unsafe {
                crate::init::collection::fill(self.ptr, &mut self.len, self.cap.get(), &mut iter)
            }
            .map_err(InitError::Initialize)?
            {
                return Ok(());
            }
            let Some(item) = iter.try_next().map_err(InitError::Initialize)? else {
                return Ok(());
            };
            self.push(
                item,
                iter.try_size_hint()
                    .map_err(InitError::Initialize)?
                    .0
                    .saturating_add(1),
            )
            .map_err(InitError::Storage)?;
        }
    }
}

#[cfg(kani)]
#[path = "buffer_proofs.rs"]
mod proofs;
