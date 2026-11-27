//! Checked initialization, conversions, and non-allocating trait adapters.
use super::*;
use core::{mem::MaybeUninit, ops::RangeBounds};

impl<T, P: AllocationPolicy, L: LengthPolicy> Vec<T, P, L> {
    #[inline]
    pub fn try_push_mut(&mut self, value: T) -> Result<&mut T, Rejected<T>> {
        let index = self.len.get();
        self.try_push(value)?;
        // SAFETY: successful push initialized the final element.
        Ok(unsafe { &mut *self.ptr.as_ptr().add(index) })
    }
    pub fn try_insert_mut(&mut self, index: usize, value: T) -> Result<&mut T, Rejected<T>> {
        self.try_insert(index, value)?;
        // SAFETY: successful insertion initialized the checked index.
        Ok(unsafe { &mut *self.ptr.as_ptr().add(index) })
    }
    /// Never allocates. The configured limit still applies (cap <= max_len).
    pub fn push_within_capacity(&mut self, value: T) -> Result<&mut T, Rejected<T>> {
        let next = match geometry::append_cursor(self.len, self.cap.get()) {
            Ok(next) => next,
            Err(error) => {
                return Err(Rejected { value, error });
            }
        };
        // Keep the initialized address rather than reconstructing len-1 later.
        // SAFETY: spare slot, within the logical limit.
        let place = unsafe { self.ptr.as_ptr().add(self.len.get()) };
        unsafe {
            place.write(value);
        }
        self.len = next;
        // SAFETY: just initialized, exclusively borrowed.
        Ok(unsafe { &mut *place })
    }
    pub fn try_pop_if<E>(
        &mut self,
        predicate: impl FnOnce(&mut T) -> Result<bool, E>,
    ) -> Result<Option<T>, E> {
        let Some(last) = self.last_mut() else {
            return Ok(None);
        };
        if predicate(last)? {
            Ok(self.pop())
        } else {
            Ok(None)
        }
    }
    pub fn pop_if(&mut self, predicate: impl FnOnce(&mut T) -> bool) -> Option<T> {
        let last = self.last_mut()?;
        if predicate(last) { self.pop() } else { None }
    }
    pub fn as_non_null(&mut self) -> NonNull<T> {
        self.ptr
    }
    pub fn allocation_policy(&self) -> &P {
        &self.policy
    }
    pub fn length_policy(&self) -> L {
        self.limit
    }
    pub fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<T>] {
        // SAFETY: uniquely borrowed uninitialized suffix, with validated layout.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.ptr.as_ptr().add(self.len.get()).cast(),
                crate::arithmetic::bounded::wrapping_sub(self.cap.get(), self.len.get()),
            )
        }
    }
    pub fn split_at_spare_mut(&mut self) -> (&mut [T], &mut [MaybeUninit<T>]) {
        // SAFETY: initialized prefix and spare suffix are disjoint logical ranges.
        unsafe {
            (
                core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len.get()),
                core::slice::from_raw_parts_mut(
                    self.ptr.as_ptr().add(self.len.get()).cast(),
                    crate::arithmetic::bounded::wrapping_sub(self.cap.get(), self.len.get()),
                ),
            )
        }
    }
    /// Set the initialized length after writing spare storage. Bounds refusal
    /// preserves length. Shrinking deliberately does not run element destructors.
    /// # Safety
    /// When growing to an in-bounds length, every new element must be initialized.
    pub unsafe fn try_set_len(&mut self, new_len: usize) -> Result<(), Error> {
        required_len(0, new_len, self.cap.get())?;
        self.len = Checked::new(new_len);
        Ok(())
    }
    /// Explicitly leak storage and its budget charge. The policy must outlive the
    /// returned borrow; raw recovery requires the original capacity and policy.
    pub fn leak<'a>(self) -> &'a mut [T]
    where
        T: 'a,
        P: 'a,
        L: 'a,
    {
        let (ptr, len, _, policy, _) = self.into_raw_parts();
        core::mem::forget(policy);
        // SAFETY: unique owner forgotten, initialized prefix remains allocated.
        unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr(), len) }
    }
    /// Clone an internal range after reserving; pointer is obtained after possible
    /// reallocation. Error drops the appended prefix and preserves original values.
    pub fn try_extend_from_within(&mut self, range: impl RangeBounds<usize>) -> Result<(), Error>
    where
        T: TryClone,
    {
        let (start, end) = edit::checked_range(range, self.len.get())?;
        let count = (Checked::new(end) - Checked::new(start)).map_err(|_| Error::Bounds)?;
        self.try_reserve(count.get())?;
        let original = self.len.get();
        let guard = SuffixGuard {
            owner: self,
            original,
        };
        #[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(original <= guard.owner.len() && guard.owner.len() - original == kani::index))]
        for i in start..end {
            let next = (guard.owner.len + Checked::new(1)).map_err(|_| Error::Capacity)?;
            // SAFETY: old prefix and new suffix disjoint; reservation already finished.
            let source = unsafe { &*guard.owner.ptr.as_ptr().add(i) };
            let ptr = unsafe {
                NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(guard.owner.len.get()))
            };
            let mut slot = unsafe { InitSlot::new(ptr) };
            source.try_clone_into(&mut slot)?;
            slot.finish()?;
            guard.owner.len = next;
        }
        mem::forget(guard);
        Ok(())
    }
    /// Bulk Copy variant, with a single checked reservation and copy.
    pub fn try_extend_copy_from_within(
        &mut self,
        range: impl RangeBounds<usize>,
    ) -> Result<(), Error>
    where
        T: Copy,
    {
        let (start, end) = edit::checked_range(range, self.len.get())?;
        let count = (Checked::new(end) - Checked::new(start)).map_err(|_| Error::Bounds)?;
        let needed = self.reserve_length(count.get())?;
        // SAFETY: source lies in the original initialized prefix, destination after it.
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.ptr.as_ptr().add(start),
                self.ptr.as_ptr().add(self.len.get()),
                count.get(),
            );
        }
        self.len = Checked::new(needed);
        Ok(())
    }
    /// Owned-value resize matches std's n-1 clones followed by a move. Refusal
    /// returns the original input and rolls back the appended initialized suffix.
    pub fn try_resize_owned(&mut self, new_len: usize, value: T) -> Result<(), Rejected<T>>
    where
        T: TryClone,
    {
        if new_len <= self.len.get() {
            self.truncate(new_len);
            return Ok(());
        }
        let last = match Checked::new(new_len) - Checked::new(1) {
            Ok(last) => last.get(),
            Err(_) => {
                return Err(Rejected {
                    value,
                    error: Error::Capacity,
                });
            }
        };
        let additional = match Checked::new(new_len) - self.len {
            Ok(additional) => additional,
            Err(_) => {
                return Err(Rejected {
                    value,
                    error: Error::Capacity,
                });
            }
        };
        if let Err(error) = self.try_reserve(additional.get()) {
            return Err(Rejected { value, error });
        }
        let original = self.len.get();
        let guard = SuffixGuard {
            owner: self,
            original,
        };
        #[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(original <= guard.owner.len() && guard.owner.len() <= last))]
        while guard.owner.len.get() < last {
            let next = match guard.owner.len + Checked::new(1) {
                Ok(next) => next,
                Err(_) => {
                    return Err(Rejected {
                        value,
                        error: Error::Capacity,
                    });
                }
            };
            // SAFETY: reserved unwritten suffix slot, recorded initialization.
            let ptr = unsafe {
                NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(guard.owner.len.get()))
            };
            let mut slot = unsafe { InitSlot::new(ptr) };
            if let Err(error) = value.try_clone_into(&mut slot) {
                return Err(Rejected { value, error });
            }
            if let Err(error) = slot.finish() {
                return Err(Rejected { value, error });
            }
            guard.owner.len = next;
        }
        let next = match guard.owner.len + Checked::new(1) {
            Ok(next) => next,
            Err(_) => {
                return Err(Rejected {
                    value,
                    error: Error::Capacity,
                });
            }
        };
        // SAFETY: final reserved slot, move input instead of cloning it.
        unsafe {
            guard
                .owner
                .ptr
                .as_ptr()
                .add(guard.owner.len.get())
                .write(value);
        }
        guard.owner.len = next;
        mem::forget(guard);
        Ok(())
    }
    /// Move into an array; mismatch returns the original Vec. This by-value API
    /// has the same caller stack-size constraints as returning any [T; N].
    pub fn try_into_array<const N: usize>(mut self) -> Result<[T; N], Rejected<Self>> {
        if self.len.get() != N {
            return Err(Rejected {
                value: self,
                error: Error::Bounds,
            });
        }
        self.len = Checked::new(0);
        // SAFETY: exact initialized length; ownership moved once, Vec frees only storage.
        Ok(unsafe { self.ptr.as_ptr().cast::<[T; N]>().read() })
    }
    pub fn try_from_array_in<const N: usize>(
        values: [T; N],
        policy: P,
        limit: L,
    ) -> Result<Self, Rejected<[T; N]>> {
        let mut result = Self::empty(policy, limit);
        if let Err(error) = result.try_reserve_exact(N) {
            return Err(Rejected {
                value: values,
                error,
            });
        }
        let values = mem::ManuallyDrop::new(values);
        // SAFETY: exact reserved destination; transfer array elements, no duplicate Drop.
        unsafe {
            core::ptr::copy_nonoverlapping(values.as_ptr(), result.ptr.as_ptr(), N);
        }
        result.len = Checked::new(N);
        Ok(result)
    }
    pub fn try_from_slice_in(source: &[T], policy: P, limit: L) -> Result<Self, Error>
    where
        T: TryClone,
    {
        let mut result = Self::empty(policy, limit);
        result.try_reserve_exact(source.len())?;
        result.try_extend_cloned(source)?;
        Ok(result)
    }
    pub fn try_from_iter_in(
        iter: impl IntoIterator<Item = T>,
        policy: P,
        limit: L,
    ) -> Result<Self, Error> {
        let mut result = Self::empty(policy, limit);
        result.try_extend(iter)?;
        Ok(result)
    }
    pub fn try_from_fallible_iter_in<E>(
        iter: impl crate::TryIntoIterator<Item = T, Error = E>,
        policy: P,
        limit: L,
    ) -> Result<Self, InitError<E>> {
        let mut result = Self::empty(policy, limit);
        result.try_extend_fallible(iter)?;
        Ok(result)
    }
}
impl<T> Vec<T> {
    pub fn try_from_array<const N: usize>(values: [T; N]) -> Result<Self, Rejected<[T; N]>> {
        Self::try_from_array_in(values, Unmetered, Unlimited)
    }
    pub fn try_from_slice(source: &[T]) -> Result<Self, Error>
    where
        T: TryClone,
    {
        Self::try_from_slice_in(source, Unmetered, Unlimited)
    }
    pub fn try_from_iter(iter: impl IntoIterator<Item = T>) -> Result<Self, Error> {
        Self::try_from_iter_in(iter, Unmetered, Unlimited)
    }
    pub fn try_from_fallible_iter<E>(
        iter: impl crate::TryIntoIterator<Item = T, Error = E>,
    ) -> Result<Self, InitError<E>> {
        Self::try_from_fallible_iter_in(iter, Unmetered, Unlimited)
    }
    pub fn try_from_elem(value: T, n: usize) -> Result<Self, Rejected<T>>
    where
        T: TryClone,
    {
        let mut result = Self::new();
        result.try_resize_owned(n, value)?;
        Ok(result)
    }
    /// Cow is an explicit std interoperability boundary. Borrowed values use
    /// TryClone; an owned standard Vec transfers its global allocation unchanged.
    pub fn try_from_cow(
        source: alloc::borrow::Cow<'_, [T]>,
    ) -> Result<Self, Rejected<alloc::borrow::Cow<'_, [T]>>>
    where
        T: Clone + TryClone,
        [T]: alloc::borrow::ToOwned<Owned = alloc::vec::Vec<T>>,
    {
        match source {
            alloc::borrow::Cow::Owned(v) => {
                Self::try_from_std_in(v, Unmetered, Unlimited).map_err(|e| Rejected {
                    value: alloc::borrow::Cow::Owned(e.value),
                    error: e.error,
                })
            }
            alloc::borrow::Cow::Borrowed(s) => Self::try_from_slice(s).map_err(|error| Rejected {
                value: alloc::borrow::Cow::Borrowed(s),
                error,
            }),
        }
    }
}
impl Vec<u8> {
    pub fn try_from_str(source: &str) -> Result<Self, Error> {
        Self::try_from_slice(source.as_bytes())
    }
}
impl<T, const N: usize, P: AllocationPolicy, L: LengthPolicy> Vec<[T; N], P, L> {
    /// Reinterpret storage without allocation. The numeric element limit stays
    /// fixed; reject if the flattened length or capacity would exceed it.
    pub fn try_into_flattened(self) -> Result<Vec<T, P, L>, Rejected<Self>> {
        let Some((len, cap)) = super::geometry::flatten(
            self.len.get(),
            self.cap.get(),
            self.max_len(),
            mem::size_of::<T>(),
            N,
        ) else {
            return Err(Rejected {
                value: self,
                error: Error::Capacity,
            });
        };
        let (ptr, _, _, policy, limit) = self.into_raw_parts();
        // SAFETY: equal alignment/byte allocation, N=0 owns no allocation or T;
        // otherwise each array becomes N initialized elements. Charge unchanged.
        Ok(unsafe { Vec::from_raw_parts(ptr.cast(), len, cap, policy, limit) })
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> core::borrow::Borrow<[T]> for Vec<T, P, L> {
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> core::borrow::BorrowMut<[T]> for Vec<T, P, L> {
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: PartialOrd, P: AllocationPolicy, L: LengthPolicy> PartialOrd for Vec<T, P, L> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> Ord for Vec<T, P, L> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<P: AllocationPolicy, L: LengthPolicy> Vec<u8, P, L> {
    /// Fallible byte writer. All bytes are accepted or no bytes are appended.
    pub fn try_write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        self.try_extend_copy(bytes)?;
        Ok(bytes.len())
    }
    pub fn try_write_all(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.try_extend_copy(bytes)
    }
    pub fn flush(&mut self) {}
    /// Reserve the complete checked sum before copying any input.
    pub fn try_write_vectored(&mut self, parts: &[&[u8]]) -> Result<usize, Error> {
        let mut total = Checked::new(0usize);
        for part in parts {
            total = (total + Checked::new(part.len())).map_err(|_| Error::Capacity)?;
        }
        let needed = self.reserve_length(total.get())?;
        // No fallible operation or user callback follows the reservation. Keep
        // a cursor into the reserved suffix and commit the checked length once.
        let mut destination = unsafe { self.ptr.as_ptr().add(self.len.get()) };
        for part in parts {
            // SAFETY: total space reserved, disjoint input borrows.
            unsafe {
                core::ptr::copy_nonoverlapping(part.as_ptr(), destination, part.len());
            }
            // SAFETY: checked total bounds every intermediate position, including
            // the one-past destination after the final copy.
            destination = unsafe { destination.add(part.len()) };
        }
        self.len = Checked::new(needed);
        Ok(total.get())
    }
    /// Roll back appended bytes on formatting/storage failure, including when a
    /// formatter ignores a failed write. Formatter side effects are not rolled back.
    pub fn try_write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<(), Error> {
        struct Writer<'a, P: AllocationPolicy, L: LengthPolicy> {
            owner: &'a mut Vec<u8, P, L>,
            error: Option<Error>,
        }
        impl<P: AllocationPolicy, L: LengthPolicy> core::fmt::Write for Writer<'_, P, L> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                if self.error.is_some() {
                    return Err(core::fmt::Error);
                }
                if let Err(e) = self.owner.try_extend_copy(s.as_bytes()) {
                    self.error = Some(e);
                    return Err(core::fmt::Error);
                }
                Ok(())
            }
        }
        let original = self.len.get();
        let guard = SuffixGuard {
            owner: self,
            original,
        };
        let mut writer = Writer {
            owner: guard.owner,
            error: None,
        };
        let result = core::fmt::write(&mut writer, args);
        if let Some(e) = writer.error {
            return Err(e);
        }
        if result.is_err() {
            return Err(Error::InvalidStructure);
        }
        mem::forget(guard);
        Ok(())
    }
}

impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy> PartialEq<[U]> for Vec<T, P, L> {
    fn eq(&self, other: &[U]) -> bool {
        self.as_slice() == other
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy> PartialEq<&[U]> for Vec<T, P, L> {
    fn eq(&self, other: &&[U]) -> bool {
        self.as_slice() == *other
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy, const N: usize> PartialEq<[U; N]>
    for Vec<T, P, L>
{
    fn eq(&self, other: &[U; N]) -> bool {
        self.as_slice() == other
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy, const N: usize> PartialEq<&[U; N]>
    for Vec<T, P, L>
{
    fn eq(&self, other: &&[U; N]) -> bool {
        self.as_slice() == *other
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy> PartialEq<Vec<U, P, L>> for [T] {
    fn eq(&self, other: &Vec<U, P, L>) -> bool {
        self == other.as_slice()
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy> PartialEq<Vec<U, P, L>> for &[T] {
    fn eq(&self, other: &Vec<U, P, L>) -> bool {
        *self == other.as_slice()
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy, const N: usize>
    PartialEq<Vec<U, P, L>> for [T; N]
{
    fn eq(&self, other: &Vec<U, P, L>) -> bool {
        self == other.as_slice()
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy, const N: usize>
    PartialEq<Vec<U, P, L>> for &[T; N]
{
    fn eq(&self, other: &Vec<U, P, L>) -> bool {
        *self == other.as_slice()
    }
}
