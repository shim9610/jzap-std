use super::*;
#[cfg(all(kani, jzap_loop_contracts))]
#[path = "slice_proofs.rs"]
mod panic_proofs;
use crate::{Error, TryClone, TryCloneIn, init::InitSlot};
use core::{iter::FusedIterator, mem::MaybeUninit, slice::SliceIndex};

impl<T, P: AllocationPolicy> Box<[T], P> {
    pub fn empty_in(policy: P) -> Self {
        // SAFETY: empty slice uses aligned non-null storage, no bytes or charge.
        unsafe {
            Self::from_raw_parts(
                NonNull::slice_from_raw_parts(NonNull::dangling(), 0),
                policy,
            )
        }
    }
    pub fn len(&self) -> usize {
        self.ptr.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Explicit raw-slice interoperability. Ordinary slice operations retain std contracts.
    pub fn as_slice(&self) -> &[T] {
        self.as_ref()
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.as_mut()
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

    /// Allocate once, then initialize each final element slot. Refusal runs no callback.
    /// On failure all completed elements and the current written element are dropped.
    pub fn try_init_slice_in<E>(
        len: usize,
        policy: P,
        mut initialize: impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<Self, InitError<E>> {
        let layout = crate::layout::array::<T>(len).map_err(InitError::Storage)?;
        let ptr = storage::allocate(layout, policy.budget())
            .map_err(InitError::Storage)?
            .cast::<T>();
        let allocation = AllocationGuard(ptr.cast(), layout, policy.budget());
        // SAFETY: the admitted layout and allocation provide exclusive unwritten
        // storage for len elements; allocation stays guarded through initialization.
        unsafe { crate::init::prefix::initialize(ptr, len, &mut initialize) }?;
        core::mem::forget(allocation);
        // SAFETY: all len elements are initialized; guards have transferred ownership.
        Ok(unsafe { Self::from_raw_parts(NonNull::slice_from_raw_parts(ptr, len), policy) })
    }
    pub fn try_clone_from_slice_in(source: &[T], policy: P) -> Result<Self, Error>
    where
        T: TryClone,
    {
        Self::try_init_slice_in(source.len(), policy, |index, slot| {
            // SAFETY: callback receives only indices in source.len().
            unsafe { source.get_unchecked(index) }.try_clone_into(slot)
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
    pub fn try_copy_from_slice_in(source: &[T], policy: P) -> Result<Self, Error>
    where
        T: Copy,
    {
        let layout = crate::layout::array::<T>(source.len())?;
        let ptr = storage::allocate(layout, policy.budget())?.cast::<T>();
        // SAFETY: fresh disjoint destination with source's validated length/layout.
        unsafe {
            core::ptr::copy_nonoverlapping(source.as_ptr(), ptr.as_ptr(), source.len());
            Ok(Self::from_raw_parts(
                NonNull::slice_from_raw_parts(ptr, source.len()),
                policy,
            ))
        }
    }
    pub fn try_new_uninit_slice_in(
        len: usize,
        policy: P,
    ) -> Result<Box<[MaybeUninit<T>], P>, Error> {
        let layout = crate::layout::array::<T>(len)?;
        let ptr = storage::allocate(layout, policy.budget())?.cast::<MaybeUninit<T>>();
        // SAFETY: MaybeUninit elements accept the unwritten storage.
        Ok(unsafe { Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr, len), policy) })
    }
    pub fn try_new_zeroed_slice_in(
        len: usize,
        policy: P,
    ) -> Result<Box<[MaybeUninit<T>], P>, Error> {
        let layout = crate::layout::array::<T>(len)?;
        let ptr = storage::allocate_zeroed(layout, policy.budget())?.cast::<MaybeUninit<T>>();
        // SAFETY: zero bytes are valid MaybeUninit elements, not necessarily valid T.
        Ok(unsafe { Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr, len), policy) })
    }
    /// Length mismatch returns the original allocation and policy unchanged.
    pub fn try_into_array<const N: usize>(self) -> Result<Box<[T; N], P>, Self> {
        if self.len() != N {
            return Err(self);
        }
        let (ptr, policy) = Self::into_raw_parts(self);
        // SAFETY: equal element count and identical array layout.
        Ok(unsafe { Box::from_raw_parts(ptr.cast(), policy) })
    }
}
impl<T> Box<[T]> {
    pub fn empty() -> Self {
        Self::empty_in(Unmetered)
    }
    pub fn try_init_slice<E>(
        len: usize,
        initialize: impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<Self, InitError<E>> {
        Self::try_init_slice_in(len, Unmetered, initialize)
    }
    pub fn try_copy_from_slice(source: &[T]) -> Result<Self, Error>
    where
        T: Copy,
    {
        Self::try_copy_from_slice_in(source, Unmetered)
    }
    pub fn try_clone_from_slice(source: &[T]) -> Result<Self, Error>
    where
        T: TryClone,
    {
        Self::try_clone_from_slice_in(source, Unmetered)
    }
    pub fn try_new_uninit_slice(len: usize) -> Result<Box<[MaybeUninit<T>]>, Error> {
        Self::try_new_uninit_slice_in(len, Unmetered)
    }
    pub fn try_new_zeroed_slice(len: usize) -> Result<Box<[MaybeUninit<T>]>, Error> {
        Self::try_new_zeroed_slice_in(len, Unmetered)
    }
    /// Transfer to the standard Vec API without a new allocation.
    pub fn into_std_vec(self) -> alloc::vec::Vec<T> {
        let (ptr, _) = Self::into_raw_parts(self);
        let len = ptr.len();
        // SAFETY: exact global array layout, fully initialized; Vec takes sole ownership.
        unsafe { alloc::vec::Vec::from_raw_parts(ptr.cast::<T>().as_ptr(), len, len) }
    }
}
impl<T> Default for Box<[T]> {
    fn default() -> Self {
        Self::empty()
    }
}
impl<T, P: AllocationPolicy> Box<[MaybeUninit<T>], P> {
    /// # Safety
    /// Every element must contain a fully initialized, valid T.
    pub unsafe fn assume_init(this: Self) -> Box<[T], P> {
        let (ptr, policy) = Self::into_raw_parts(this);
        // SAFETY: identical element layout and count; caller supplies validity.
        unsafe {
            Box::from_raw_parts(
                NonNull::slice_from_raw_parts(ptr.cast::<T>(), ptr.len()),
                policy,
            )
        }
    }
}
impl<T, P: AllocationPolicy, const N: usize> Box<[T; N], P> {
    pub fn into_slice(self) -> Box<[T], P> {
        crate::box_unsize!(self => [T])
    }
}
impl<T: TryClone, P: AllocationPolicy> TryClone for Box<[T], P> {
    #[inline]
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        if self.len() == source.len()
            && let Some(result) = T::try_copy_slice_from(source.as_slice(), self.as_mut_slice())
        {
            return result;
        }
        let replacement =
            Self::try_clone_from_slice_in(source.as_slice(), self.policy.try_clone()?)?;
        *self = replacement;
        Ok(())
    }
    fn try_clone(&self) -> Result<Self, Error> {
        Self::try_clone_from_slice_in(self.as_slice(), self.policy.try_clone()?)
    }
}
impl<'a, T: TryCloneIn<'a>, P: AllocationPolicy> TryCloneIn<'a> for Box<[T], P> {
    type Output = Box<[T::Output], Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        Box::<[T::Output], _>::try_init_slice_in(self.len(), budget, |index, slot| {
            // SAFETY: initializer only supplies indices in the source slice length.
            unsafe { self.as_slice().get_unchecked(index) }.try_clone_into_in(budget, slot)
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}

/// Owned slice iteration; the original allocation is released even after partial consumption.
pub struct IntoIter<T, P: AllocationPolicy = Unmetered> {
    ptr: NonNull<T>,
    span: crate::span::Span,
    layout: Layout,
    policy: P,
    marker: PhantomData<T>,
}
impl<T, P: AllocationPolicy> IntoIterator for Box<[T], P> {
    type Item = T;
    type IntoIter = IntoIter<T, P>;
    fn into_iter(self) -> Self::IntoIter {
        let layout = Layout::for_value(self.as_slice());
        let (ptr, policy) = Self::into_raw_parts(self);
        IntoIter {
            ptr: ptr.cast(),
            span: crate::span::Span::full(ptr.len()),
            layout,
            policy,
            marker: PhantomData,
        }
    }
}
impl<T, P: AllocationPolicy> Iterator for IntoIter<T, P> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let i = self.span.next()?;
        // SAFETY: span removed the initialized index before transfer.
        Some(unsafe { self.ptr.as_ptr().add(i).read() })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.span.len();
        (len, Some(len))
    }
}
impl<T, P: AllocationPolicy> DoubleEndedIterator for IntoIter<T, P> {
    fn next_back(&mut self) -> Option<T> {
        let i = self.span.next_back()?;
        // SAFETY: span removed the initialized index before transfer.
        Some(unsafe { self.ptr.as_ptr().add(i).read() })
    }
}
impl<T, P: AllocationPolicy> ExactSizeIterator for IntoIter<T, P> {
    #[inline]
    fn len(&self) -> usize {
        self.size_hint().0
    }
}
impl<T, P: AllocationPolicy> FusedIterator for IntoIter<T, P> {}
impl<T, P: AllocationPolicy> Drop for IntoIter<T, P> {
    fn drop(&mut self) {
        let _allocation = AllocationGuard(self.ptr.cast(), self.layout, self.policy.budget());
        let remaining = self.span.len();
        if !core::mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: only remaining initialized elements are dropped; original Layout
        // is retained independently of the now partly uninitialized allocation.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr().add(self.span.front()),
                remaining,
            ))
        }
    }
}
// SAFETY: unique remaining elements and accounting policy are transferred together.
unsafe impl<T: Send, P: AllocationPolicy + Send> Send for IntoIter<T, P> {}
// SAFETY: shared access cannot mutate iterator state or owned elements.
unsafe impl<T: Sync, P: AllocationPolicy + Sync> Sync for IntoIter<T, P> {}
impl<'a, T, P: AllocationPolicy> IntoIterator for &'a Box<[T], P> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T, P: AllocationPolicy> IntoIterator for &'a mut Box<[T], P> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T, P: AllocationPolicy> Box<[T], P> {
    /// Consume a fallible iterator. On failure consumed elements are cleaned up;
    /// iterator side effects and consumption are not rolled back.
    pub fn try_from_fallible_iter_in<E>(
        source: impl crate::TryIntoIterator<Item = T, Error = E>,
        policy: P,
    ) -> Result<Self, InitError<E>> {
        let mut buffer = super::buffer::Buffer::new(policy);
        buffer.collect(source.try_into_iter().map_err(InitError::Initialize)?)?;
        buffer.finish().map_err(InitError::Storage)
    }
    pub fn try_from_iter_in(source: impl IntoIterator<Item = T>, policy: P) -> Result<Self, Error> {
        Self::try_from_fallible_iter_in(crate::iter::from_std(source), policy).map_err(
            |e| match e {
                InitError::Storage(e) => e,
                InitError::Initialize(never) => match never {},
            },
        )
    }
    pub fn copy_from(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: Copy,
    {
        crate::clone::try_copy_slice(source, self.as_mut_slice())
    }
}
impl<T> Box<[T]> {
    pub fn try_from_iter(source: impl IntoIterator<Item = T>) -> Result<Self, Error> {
        Self::try_from_iter_in(source, Unmetered)
    }
}

impl<T, P: AllocationPolicy> Box<[T], P> {
    /// Adopt Vec storage. If excess capacity requires a shrinking realloc and it
    /// fails, return the original Vec (including its pointer, length and capacity).
    pub fn try_from_std_vec_in(
        value: alloc::vec::Vec<T>,
        policy: P,
    ) -> Result<Self, Rejected<alloc::vec::Vec<T>>> {
        let len = value.len();
        let old = match crate::layout::array::<T>(value.capacity()) {
            Ok(layout) => layout,
            Err(error) => return Err(Rejected { value, error }),
        };
        let new = match crate::layout::array::<T>(len) {
            Ok(layout) => layout,
            Err(error) => return Err(Rejected { value, error }),
        };
        let ptr = {
            let reservation = match policy.budget().reserve(new.size()) {
                Ok(r) => r,
                Err(error) => return Err(Rejected { value, error }),
            };
            let mut value = core::mem::ManuallyDrop::new(value);
            // SAFETY: Vec's pointer is aligned and non-null, including empty/ZST cases.
            let ptr = unsafe { NonNull::new_unchecked(value.as_mut_ptr()) };
            // SAFETY: no live references, initialized prefix fits the new layout. The
            // original allocation has no jzap charge; target reservation is separate.
            let ptr = match unsafe {
                storage::reallocate(ptr.cast(), old, new, crate::budget::Accounting::Unmetered)
            } {
                Ok(ptr) => ptr,
                Err(error) => {
                    return Err(Rejected {
                        value: core::mem::ManuallyDrop::into_inner(value),
                        error,
                    });
                }
            };
            if let Some(r) = reservation {
                r.commit().retain();
            }
            ptr
        };
        // SAFETY: exact initialized array allocation; old Vec owner was disarmed.
        Ok(unsafe { Self::from_raw_parts(NonNull::slice_from_raw_parts(ptr.cast(), len), policy) })
    }
}
impl<T> crate::TryDefault for Box<[T]> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::empty())
    }
}
