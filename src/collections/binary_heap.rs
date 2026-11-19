//! Max-priority ownership with explicit allocation, source and predicate failures.
//! Storage and budget policies are those of the existing fallible Vec.
use crate::{
    Error, Rejected, TryClone, TryCloneIn, TryIntoIterator, TryIterator,
    budget::{AllocationPolicy, Budget, Unmetered},
    vec::{LengthPolicy, Unlimited, Vec},
};
use ::core::{
    fmt,
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
};
mod core;
pub(crate) mod geometry;
mod iter;
#[cfg(kani)]
mod proofs;
pub use iter::Drain;
pub type Iter<'a, T> = ::core::slice::Iter<'a, T>;
pub type IntoIter<T, P = Unmetered, L = Unlimited> = crate::vec::IntoIter<T, P, L>;

pub struct BinaryHeap<T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    data: Vec<T, P, L>,
}
impl<T> BinaryHeap<T> {
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }
    pub fn try_with_capacity(capacity: usize) -> Result<Self, Error> {
        Vec::try_with_capacity(capacity).map(|data| Self { data })
    }
}
impl<T> Default for BinaryHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> crate::TryDefault for BinaryHeap<T> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
impl<T, P: AllocationPolicy> BinaryHeap<T, P> {
    pub fn new_in(policy: P) -> Self {
        Self {
            data: Vec::new_in(policy),
        }
    }
    pub fn try_with_capacity_in(capacity: usize, policy: P) -> Result<Self, Error> {
        Vec::try_with_capacity_in(capacity, policy).map(|data| Self { data })
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> BinaryHeap<T, P, L> {
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }
    pub fn max_len(&self) -> usize {
        self.data.max_len()
    }
    pub fn allocation_policy(&self) -> &P {
        self.data.allocation_policy()
    }
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.data.as_slice()
    }
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        self.data.iter()
    }
    #[inline]
    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }
    pub fn clear(&mut self) {
        self.data.clear()
    }
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), Error> {
        self.data.try_reserve(additional)
    }
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), Error> {
        self.data.try_reserve_exact(additional)
    }
    pub fn try_shrink_to(&mut self, min: usize) -> Result<(), Error> {
        self.data.try_shrink_to(min)
    }
    pub fn try_shrink_to_fit(&mut self) -> Result<(), Error> {
        self.data.try_shrink_to_fit()
    }
    pub fn into_vec(self) -> Vec<T, P, L> {
        self.data
    }
    pub fn drain(&mut self) -> Drain<'_, T, P, L> {
        Drain::new(self)
    }
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> BinaryHeap<T, P, L> {
    pub fn from_vec(mut data: Vec<T, P, L>) -> Self {
        core::rebuild(data.as_mut_slice());
        Self { data }
    }
    #[inline]
    pub fn try_push(&mut self, value: T) -> Result<(), Rejected<T>> {
        self.data.try_push(value)?;
        // Capture after the outlined Vec growth. Successful push proves len > 0;
        // deriving the index from the published length also exposes that fact to LLVM.
        let pos = crate::arithmetic::bounded::wrapping_sub(self.len(), 1);
        // SAFETY: successful append initializes pos; the old prefix is a heap.
        unsafe { core::sift_up(self.data.as_mut_slice(), 0, pos) };
        Ok(())
    }
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            // SAFETY: explicitly checked nonempty heap.
            Some(unsafe { core::pop_nonempty(&mut self.data) })
        }
    }
    pub fn peek_mut(&mut self) -> Option<PeekMut<'_, T, P, L>> {
        if self.is_empty() {
            None
        } else {
            Some(PeekMut {
                heap: self,
                original_len: None,
            })
        }
    }
    pub fn into_sorted_vec(mut self) -> Vec<T, P, L> {
        let mut end = self.len();
        while end > 1 {
            end = crate::arithmetic::bounded::wrapping_sub(end, 1);
            let ptr = self.data.as_mut_ptr();
            // SAFETY: 0 < end < original len, both initialized and distinct.
            unsafe { ::core::ptr::swap(ptr, ptr.add(end)) };
            // SAFETY: root swap leaves child subtrees valid within [0,end).
            unsafe { core::sift_down(self.data.as_mut_slice(), 0, end) };
        }
        self.data
    }
    /// Storage refusal preserves both heaps and their allocation policies.
    pub fn try_append(&mut self, other: &mut Self) -> Result<(), Error> {
        let reused = self.data.try_reuse_larger_buffer(&mut other.data)?;
        let swapped = !reused
            && self.len() < other.len()
            && !self.allocation_policy().budget().is_metered()
            && !other.allocation_policy().budget().is_metered()
            && self.max_len() == other.max_len();
        if swapped {
            ::core::mem::swap(self, other);
        }
        let start = self.len();
        if let Err(error) = self.data.try_append(&mut other.data) {
            if swapped {
                ::core::mem::swap(self, other);
            }
            return Err(error);
        }
        core::rebuild_tail(self.data.as_mut_slice(), start);
        Ok(())
    }
    /// Earlier removals remain on predicate refusal. Repair also runs on unwind.
    pub fn try_retain<E>(
        &mut self,
        mut predicate: impl FnMut(&T) -> Result<bool, E>,
    ) -> Result<(), E> {
        struct Repair<'a, T: Ord, P: AllocationPolicy, L: LengthPolicy> {
            heap: &'a mut BinaryHeap<T, P, L>,
            start: usize,
        }
        impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> Drop for Repair<'_, T, P, L> {
            #[inline]
            fn drop(&mut self) {
                // The unchanged prefix needs no repair when every removal was
                // in the tail. Avoid clamping the index on this no-work path.
                if self.start < self.heap.len() {
                    core::rebuild_tail(self.heap.data.as_mut_slice(), self.start);
                }
            }
        }
        let mut guard = Repair {
            start: self.len(),
            heap: self,
        };
        let mut visited = 0usize;
        let start = &mut guard.start;
        guard.heap.data.try_retain(|value| {
            let keep = predicate(value)?;
            if !keep {
                *start = (*start).min(visited);
            }
            // One callback per original element, visited < original len <= usize::MAX.
            visited = crate::arithmetic::bounded::wrapping_add(visited, 1);
            Ok(keep)
        })
    }
    /// Explicit counterpart of std Extend<&T> for Copy elements.
    pub fn try_extend_copy<'a, I>(&mut self, source: I) -> Result<(), ExtendError<T, I::Error>>
    where
        T: Copy + 'a,
        I: TryIntoIterator<Item = &'a T>,
    {
        let source = source.try_into_iter().map_err(ExtendError::Source)?;
        self.try_extend(iter::Copied(source))
    }
    /// Successful prefix remains a heap on source/storage refusal.
    pub fn try_extend<I: TryIntoIterator<Item = T>>(
        &mut self,
        source: I,
    ) -> Result<(), ExtendError<T, I::Error>> {
        struct Repair<'a, T: Ord, P: AllocationPolicy, L: LengthPolicy> {
            heap: &'a mut BinaryHeap<T, P, L>,
            start: usize,
        }
        impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> Drop for Repair<'_, T, P, L> {
            fn drop(&mut self) {
                core::rebuild_tail(self.heap.data.as_mut_slice(), self.start);
            }
        }
        let mut source = source.try_into_iter().map_err(ExtendError::Source)?;
        let start = self.len();
        let guard = Repair { heap: self, start };
        while let Some(value) = source.try_next().map_err(ExtendError::Source)? {
            guard.heap.data.try_push(value).map_err(ExtendError::Push)?;
        }
        Ok(())
    }
}
impl<T: Ord, P: AllocationPolicy> BinaryHeap<T, P> {
    pub fn try_from_iter_in<I: TryIntoIterator<Item = T>>(
        source: I,
        policy: P,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        let mut result = Self::new_in(policy);
        result.try_extend(source)?;
        Ok(result)
    }
    pub fn try_from_array_in<const N: usize>(
        values: [T; N],
        policy: P,
    ) -> Result<Self, Rejected<[T; N]>> {
        Vec::try_from_array_in(values, policy, Unlimited).map(Self::from_vec)
    }
}
impl<T: Ord> BinaryHeap<T> {
    pub fn try_from_iter<I: TryIntoIterator<Item = T>>(
        source: I,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        Self::try_from_iter_in(source, Unmetered)
    }
    pub fn try_from_array<const N: usize>(values: [T; N]) -> Result<Self, Rejected<[T; N]>> {
        Self::try_from_array_in(values, Unmetered)
    }
}
#[derive(Debug)]
pub enum ExtendError<T, E> {
    Source(E),
    Push(Rejected<T>),
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> From<Vec<T, P, L>> for BinaryHeap<T, P, L> {
    fn from(data: Vec<T, P, L>) -> Self {
        Self::from_vec(data)
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> From<BinaryHeap<T, P, L>> for Vec<T, P, L> {
    fn from(heap: BinaryHeap<T, P, L>) -> Self {
        heap.into_vec()
    }
}
impl<T: TryClone, P: AllocationPolicy, L: LengthPolicy> TryClone for BinaryHeap<T, P, L> {
    fn try_clone(&self) -> Result<Self, Error> {
        self.data.try_clone().map(|data| Self { data })
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        self.data.try_clone_from(&source.data)
    }
}
impl<'a, T: TryCloneIn<'a>, P: AllocationPolicy, L: LengthPolicy> TryCloneIn<'a>
    for BinaryHeap<T, P, L>
where
    T::Output: Ord,
{
    type Output = BinaryHeap<T::Output, Budget<'a>, L>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        self.data.try_clone_in(budget).map(BinaryHeap::from_vec)
    }
}
impl<T: fmt::Debug, P: AllocationPolicy, L: LengthPolicy> fmt::Debug for BinaryHeap<T, P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Exclusive root view. Forgetting it after mutation retains only the root,
/// leaking the hidden tail, as std does; it never exposes uninitialized storage.
pub struct PeekMut<'a, T: Ord, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    heap: &'a mut BinaryHeap<T, P, L>,
    original_len: Option<NonZeroUsize>,
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> Deref for PeekMut<'_, T, P, L> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: only constructed for nonempty heap; mutable access retains root.
        unsafe { &*self.heap.data.as_ptr() }
    }
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> DerefMut for PeekMut<'_, T, P, L> {
    fn deref_mut(&mut self) -> &mut T {
        if self.original_len.is_none() && self.heap.len() > 1 {
            self.original_len = NonZeroUsize::new(self.heap.len());
            // SAFETY: hide the initialized tail without changing capacity.
            unsafe { core::set_len(&mut self.heap.data, 1) };
        }
        // SAFETY: exclusive guard, root always initialized and retained.
        unsafe { &mut *self.heap.data.as_mut_ptr() }
    }
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> Drop for PeekMut<'_, T, P, L> {
    fn drop(&mut self) {
        if let Some(len) = self.original_len.take() {
            // SAFETY: tail remained initialized; exclusive guard prevents capacity changes.
            unsafe { core::set_len(&mut self.heap.data, len.get()) };
            // SAFETY: root alone may have changed, child subtrees remain heaps.
            unsafe { core::sift_down(self.heap.data.as_mut_slice(), 0, len.get()) };
        }
    }
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> PeekMut<'_, T, P, L> {
    pub fn pop(mut this: Self) -> T {
        if let Some(len) = this.original_len.take() {
            // SAFETY: same initialized hidden tail and unchanged capacity as Drop.
            unsafe { core::set_len(&mut this.heap.data, len.get()) };
        }
        // SAFETY: construction and DerefMut preserve at least the initialized root.
        unsafe { core::pop_nonempty(&mut this.heap.data) }
    }
}
impl<T: Ord + fmt::Debug, P: AllocationPolicy, L: LengthPolicy> fmt::Debug
    for PeekMut<'_, T, P, L>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PeekMut").field(&**self).finish()
    }
}
