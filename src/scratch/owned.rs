use super::*;
use crate::{
    TryCloneIn,
    budget::{AllocationPolicy, Budget, Unmetered},
    vec::{LengthLimit, LengthPolicy, Unlimited, Vec},
};
/// Vec-backed fixed storage. Mutable Vec access and growth/shrink methods are private.
pub struct ScratchBuf<T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    values: Vec<T, P, L>,
}
impl<T> ScratchBuf<T> {
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }
    pub fn try_with_capacity(capacity: usize) -> Result<Self, Error> {
        Self::try_with_capacity_in(capacity, Unmetered)
    }
    pub fn try_with_capacity_and_limit(
        capacity: usize,
        limit: usize,
    ) -> Result<ScratchBuf<T, Unmetered, LengthLimit>, Error> {
        Self::try_with_capacity_and_limit_in(capacity, limit, Unmetered)
    }
}
impl<T, P: AllocationPolicy> ScratchBuf<T, P> {
    pub fn try_with_capacity_in(capacity: usize, policy: P) -> Result<Self, Error> {
        Ok(Self {
            values: Vec::try_with_capacity_in(capacity, policy)?,
        })
    }
    pub fn try_with_capacity_and_limit_in(
        capacity: usize,
        limit: usize,
        policy: P,
    ) -> Result<ScratchBuf<T, P, LengthLimit>, Error> {
        Ok(ScratchBuf {
            values: Vec::try_with_capacity_and_limit_in(capacity, limit, policy)?,
        })
    }
}
impl<T> Default for ScratchBuf<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> crate::TryDefault for ScratchBuf<T> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> ScratchBuf<T, P, L> {
    pub fn from_vec(values: Vec<T, P, L>) -> Self {
        Self { values }
    }
    /// Explicit ownership transfer ends the fixed-storage contract without allocation.
    pub fn into_vec(self) -> Vec<T, P, L> {
        self.values
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.values.capacity()
    }
    pub fn max_len(&self) -> usize {
        self.values.max_len()
    }
    pub fn remaining_capacity(&self) -> usize {
        sub(self.capacity(), self.len())
    }
    pub fn allocation_policy(&self) -> &P {
        self.values.allocation_policy()
    }
    pub fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.values.as_mut_slice()
    }
    pub fn as_view(&self) -> Slice<'_, T> {
        Slice::new(self.as_slice())
    }
    pub fn as_view_mut(&mut self) -> SliceMut<'_, T> {
        SliceMut::new(self.as_mut_slice())
    }
    pub fn get(&self, index: usize) -> Result<&T, Error> {
        self.values.get(index)
    }
    pub fn get_mut(&mut self, index: usize) -> Result<&mut T, Error> {
        self.values.get_mut(index)
    }
    pub fn iter(&self) -> FromStd<core::slice::Iter<'_, T>> {
        from_std(self.as_slice().iter())
    }
    pub fn iter_mut(&mut self) -> FromStd<core::slice::IterMut<'_, T>> {
        from_std(self.as_mut_slice().iter_mut())
    }
    pub fn try_push(&mut self, value: T) -> Result<&mut T, Rejected<T>> {
        self.values.push_within_capacity(value)
    }
    pub fn pop(&mut self) -> Option<T> {
        self.values.pop()
    }
    pub fn clear(&mut self) {
        self.values.clear()
    }
    pub fn truncate(&mut self, len: usize) {
        self.values.truncate(len)
    }
    pub fn try_extend_copy(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: Copy,
    {
        geometry::admitted_end(self.len(), source.len(), self.capacity()).ok_or(Error::Capacity)?;
        self.values.try_extend_copy(source)
    }
    unsafe fn copy_admitted(&mut self, source: &[T], end: usize) -> Result<(), Error> {
        let start = self.len();
        // SAFETY: caller established bitwise clonability and an admitted uninitialized suffix.
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr(),
                self.values.as_mut_ptr().add(start),
                source.len(),
            );
            // end is in capacity, and every new slot was just initialized.
            self.values.try_set_len(end)
        }
    }
    pub fn try_extend_cloned(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: TryClone,
    {
        let start = self.len();
        let end =
            geometry::admitted_end(start, source.len(), self.capacity()).ok_or(Error::Capacity)?;
        if T::copy_witness().is_some() {
            // SAFETY: private capability and suffix admission. No mutable Vec is passed to user hooks.
            return unsafe { self.copy_admitted(source, end) };
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
        initialize: impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<(), InitError<E>> {
        geometry::admitted_end(0, new_len, self.capacity())
            .ok_or(InitError::Storage(Error::Capacity))?;
        self.values.try_resize_with_init(new_len, initialize)
    }
    pub fn try_resize(&mut self, new_len: usize, value: &T) -> Result<(), Error>
    where
        T: TryClone,
    {
        geometry::admitted_end(0, new_len, self.capacity()).ok_or(Error::Capacity)?;
        if mem::size_of::<T>() == 0 && T::copy_witness().is_some() {
            // SAFETY: valid bitwise-clonable ZSTs need no bytes or drop glue; new_len fits.
            return unsafe { self.values.try_set_len(new_len) };
        }
        self.values.try_resize(new_len, value)
    }
    pub fn try_extend<E>(
        &mut self,
        input: impl TryIntoIterator<Item = T, Error = E>,
    ) -> Result<(), InitError<E>> {
        let mut input = input.try_into_iter().map_err(InitError::Initialize)?;
        let original = self.len();
        let guard = OwnedRollback {
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
struct OwnedRollback<'a, T, P: AllocationPolicy, L: LengthPolicy> {
    owner: &'a mut ScratchBuf<T, P, L>,
    original: usize,
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for OwnedRollback<'_, T, P, L> {
    fn drop(&mut self) {
        self.owner.truncate(self.original)
    }
}
impl<T: TryClone, P: AllocationPolicy, L: LengthPolicy> TryClone for ScratchBuf<T, P, L> {
    fn try_clone(&self) -> Result<Self, Error> {
        let mut values = Vec::try_from_array_in(
            [],
            self.values.allocation_policy().try_clone()?,
            self.values.length_policy(),
        )
        .map_err(|r| r.error)?;
        values.try_reserve_exact(self.capacity())?;
        let mut result = Self { values };
        result.try_extend_cloned(self.as_slice())?;
        Ok(result)
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        geometry::admitted_end(0, source.len(), self.capacity()).ok_or(Error::Capacity)?;
        if source.is_empty() {
            self.clear();
            return Ok(());
        }
        if T::copy_witness().is_some() && !mem::needs_drop::<T>() {
            self.clear();
            // SAFETY: source length admitted, empty destination, private Copy capability.
            return unsafe { self.copy_admitted(source.as_slice(), source.len()) };
        }
        let original = self.len();
        self.try_extend_cloned(source.as_slice())?;
        // The old prefix is valid after staging; drain closes it without allocation.
        drop(self.values.try_drain(..original)?);
        Ok(())
    }
}
impl<'a, T: TryCloneIn<'a>, P: AllocationPolicy, L: LengthPolicy> TryCloneIn<'a>
    for ScratchBuf<T, P, L>
{
    type Output = ScratchBuf<T::Output, Budget<'a>, L>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        let mut values =
            Vec::try_from_array_in([], budget, self.values.length_policy()).map_err(|r| r.error)?;
        values.try_reserve_exact(self.capacity())?;
        let mut result = ScratchBuf { values };
        result
            .try_resize_with_init(self.len(), |index, slot| {
                self.get(index)?.try_clone_into_in(budget, slot)
            })
            .map_err(flatten)?;
        Ok(result)
    }
}
