//! Fixed backing storage. Element callbacks retain their own allocation contract.
use crate::{
    Checked, Error, Rejected, TryClone, TryIntoIterator, TryIterator,
    arithmetic::bounded::wrapping_sub as sub,
    error::InitError,
    init::InitSlot,
    iter::{FromStd, from_std},
    slice::{Slice, SliceMut, geometry},
};
use core::{
    mem::{self, MaybeUninit},
    ptr::NonNull,
    slice,
};
#[cfg(feature = "alloc")]
mod owned;
#[cfg(feature = "alloc")]
pub use owned::ScratchBuf;

/// Borrowed uninitialized slots; only the recorded prefix contains owned values.
/// Safe byte-backed construction never exposes or writes uninitialized bytes.
pub struct Scratch<'a, T> {
    storage: &'a mut [MaybeUninit<T>],
    len: Checked<usize>,
}
impl<'a, T> Scratch<'a, T> {
    pub fn new(storage: &'a mut [MaybeUninit<T>]) -> Self {
        Self {
            storage,
            len: Checked::new(0),
        }
    }
    pub fn len(&self) -> usize {
        self.len.get()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn capacity(&self) -> usize {
        self.storage.len()
    }
    pub fn remaining_capacity(&self) -> usize {
        sub(self.capacity(), self.len())
    }
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: exactly the initialized prefix, borrowed from its single owner.
        unsafe { slice::from_raw_parts(self.storage.as_ptr().cast(), self.len()) }
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.len();
        // SAFETY: only initialized prefix, under the exclusive storage borrow.
        unsafe { slice::from_raw_parts_mut(self.storage.as_mut_ptr().cast(), len) }
    }
    pub fn as_view(&self) -> Slice<'_, T> {
        Slice::new(self.as_slice())
    }
    pub fn as_view_mut(&mut self) -> SliceMut<'_, T> {
        SliceMut::new(self.as_mut_slice())
    }
    pub fn get(&self, index: usize) -> Result<&T, Error> {
        self.as_slice().get(index).ok_or(Error::Bounds)
    }
    pub fn get_mut(&mut self, index: usize) -> Result<&mut T, Error> {
        self.as_mut_slice().get_mut(index).ok_or(Error::Bounds)
    }
    pub fn iter(&self) -> FromStd<core::slice::Iter<'_, T>> {
        from_std(self.as_slice().iter())
    }
    pub fn iter_mut(&mut self) -> FromStd<core::slice::IterMut<'_, T>> {
        from_std(self.as_mut_slice().iter_mut())
    }
    pub fn try_push(&mut self, value: T) -> Result<&mut T, Rejected<T>> {
        let end = match geometry::admitted_end(self.len(), 1, self.capacity()) {
            Some(n) => n,
            None => {
                return Err(Rejected {
                    value,
                    error: Error::Capacity,
                });
            }
        };
        let index = self.len();
        let Some(place) = self.storage.get_mut(index) else {
            return Err(Rejected {
                value,
                error: Error::Capacity,
            });
        };
        self.len = Checked::new(end);
        Ok(place.write(value))
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let index = geometry::retired_last(self.len());
        self.len = Checked::new(index);
        // SAFETY: retire exactly one initialized slot before moving its value out.
        Some(unsafe { self.storage.get_unchecked(index).assume_init_read() })
    }
    pub fn clear(&mut self) {
        self.truncate(0);
    }
    pub fn truncate(&mut self, len: usize) {
        let old = self.len();
        if len >= old {
            return;
        }
        self.len = Checked::new(len);
        if !mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: retired suffix is initialized and disjoint from retained prefix.
        // Slice drop glue cleans the remaining retired values if one destructor unwinds.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.storage.as_mut_ptr().cast::<T>().add(len),
                sub(old, len),
            ));
        }
    }
    /// Caller admits end == len + source.len and establishes bitwise clonability.
    unsafe fn copy_admitted(&mut self, source: &[T], end: usize) {
        // SAFETY: admitted uninitialized suffix, distinct shared input, Copy capability.
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr(),
                self.storage.as_mut_ptr().cast::<T>().add(self.len()),
                source.len(),
            );
        }
        self.len = Checked::new(end);
    }
    pub fn try_extend_copy(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: Copy,
    {
        let end = geometry::admitted_end(self.len(), source.len(), self.capacity())
            .ok_or(Error::Capacity)?;
        // SAFETY: T:Copy and full suffix admission precede the write.
        unsafe { self.copy_admitted(source, end) };
        Ok(())
    }
    pub fn try_extend_cloned(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: TryClone,
    {
        let start = self.len();
        let end =
            geometry::admitted_end(start, source.len(), self.capacity()).ok_or(Error::Capacity)?;
        if T::copy_witness().is_some() {
            // SAFETY: private capability permits copying; full suffix fits.
            unsafe { self.copy_admitted(source, end) };
            return Ok(());
        }
        self.try_resize_with_init(end, |index, slot| {
            source
                .get(sub(index, start))
                .ok_or(Error::Bounds)?
                .try_clone_into(slot)
        })
        .map_err(flatten)
    }
    pub fn try_resize_with_init<E>(
        &mut self,
        new_len: usize,
        mut initialize: impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<(), InitError<E>> {
        if new_len <= self.len() {
            self.truncate(new_len);
            return Ok(());
        }
        geometry::admitted_end(0, new_len, self.capacity())
            .ok_or(InitError::Storage(Error::Capacity))?;
        let original = self.len();
        let guard = Rollback {
            owner: self,
            original,
        };
        while guard.owner.len() < new_len {
            let index = guard.owner.len();
            let next = geometry::initialized_next(index, new_len);
            let cell = guard
                .owner
                .storage
                .get_mut(index)
                .ok_or(InitError::Storage(Error::Bounds))?;
            // SAFETY: exclusive spare slot. It is unowned even when backing bytes are
            // already initialized; u8 cleanup never invalidates those backing bytes.
            let mut slot = unsafe { InitSlot::new(NonNull::from(cell).cast()) };
            initialize(index, &mut slot).map_err(InitError::Initialize)?;
            slot.finish().map_err(InitError::Storage)?;
            guard.owner.len = Checked::new(next);
        }
        mem::forget(guard);
        Ok(())
    }
    pub fn try_resize(&mut self, new_len: usize, value: &T) -> Result<(), Error>
    where
        T: TryClone,
    {
        if new_len <= self.len() {
            self.truncate(new_len);
            return Ok(());
        }
        geometry::admitted_end(0, new_len, self.capacity()).ok_or(Error::Capacity)?;
        if mem::size_of::<T>() == 0 && T::copy_witness().is_some() {
            self.len = Checked::new(new_len);
            return Ok(());
        }
        self.try_resize_with_init(new_len, |_, slot| value.try_clone_into(slot))
            .map_err(flatten)
    }
    pub fn try_extend<E>(
        &mut self,
        input: impl TryIntoIterator<Item = T, Error = E>,
    ) -> Result<(), InitError<E>> {
        let mut input = input.try_into_iter().map_err(InitError::Initialize)?;
        let original = self.len();
        let guard = Rollback {
            owner: self,
            original,
        };
        while let Some(value) = input.try_next().map_err(InitError::Initialize)? {
            guard
                .owner
                .try_push(value)
                .map_err(|r| InitError::Storage(r.error))?;
        }
        mem::forget(guard);
        Ok(())
    }
}
impl<'a> Scratch<'a, u8> {
    /// Borrow initialized bytes. Safe methods preserve byte validity across reset/drop.
    pub fn from_bytes(bytes: &'a mut [u8]) -> Self {
        // SAFETY: MaybeUninit<u8> has the same layout. No safe method exposes backing
        // uninitialized storage or writes an invalid u8; original bytes remain valid.
        let storage = unsafe {
            slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<MaybeUninit<u8>>(), bytes.len())
        };
        Self::new(storage)
    }
}
impl<T> Drop for Scratch<'_, T> {
    fn drop(&mut self) {
        self.clear()
    }
}
struct Rollback<'a, 'b, T> {
    owner: &'a mut Scratch<'b, T>,
    original: usize,
}
impl<T> Drop for Rollback<'_, '_, T> {
    fn drop(&mut self) {
        self.owner.truncate(self.original)
    }
}
fn flatten(error: InitError<Error>) -> Error {
    match error {
        InitError::Storage(e) | InitError::Initialize(e) => e,
    }
}
#[cfg(kani)]
#[path = "scratch/proofs.rs"]
mod proofs;
