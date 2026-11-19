use super::*;
use crate::{TryDoubleEndedIterator, TryExactSizeIterator, TryFusedIterator, span::Span};
use ::core::{convert::Infallible, iter::FusedIterator};
impl<T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for BinaryHeap<T, P, L> {
    type Item = T;
    type IntoIter = IntoIter<T, P, L>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for &'a BinaryHeap<T, P, L> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
/// Unsorted drain; forgetting it leaks the remaining values and keeps capacity.
pub struct Drain<'a, T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    heap: &'a mut BinaryHeap<T, P, L>,
    span: Span,
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> Drain<'a, T, P, L> {
    pub(super) fn new(heap: &'a mut BinaryHeap<T, P, L>) -> Self {
        let span = Span::full(heap.len());
        // SAFETY: zero is within capacity; drain takes ownership of the old prefix.
        unsafe { super::core::set_len(&mut heap.data, 0) };
        Self { heap, span }
    }
    fn remaining(&self) -> &[T] {
        // SAFETY: span only retires original initialized prefix, exclusive borrow
        // prevents reallocation; its hole-free remaining interval is initialized.
        unsafe {
            ::core::slice::from_raw_parts(
                self.heap.data.as_ptr().add(self.span.front()),
                self.span.len(),
            )
        }
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Iterator for Drain<'_, T, P, L> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.span.next().map(|index| {
            // SAFETY: retire a unique initialized old-prefix index before moving it.
            unsafe { self.heap.data.as_ptr().add(index).read() }
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.span.len();
        (len, Some(len))
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> DoubleEndedIterator for Drain<'_, T, P, L> {
    fn next_back(&mut self) -> Option<T> {
        self.span.next_back().map(|index| {
            // SAFETY: disjoint retired index from the same ordered span.
            unsafe { self.heap.data.as_ptr().add(index).read() }
        })
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> ExactSizeIterator for Drain<'_, T, P, L> {
    fn len(&self) -> usize {
        self.span.len()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> FusedIterator for Drain<'_, T, P, L> {}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Drain<'_, T, P, L> {
    fn drop(&mut self) {
        let front = self.span.front();
        let len = self.span.len();
        self.span.clear();
        // SAFETY: remaining span exclusively owns these initialized objects;
        // retire before destructors, and leave Vec length zero on unwind.
        unsafe {
            ::core::ptr::drop_in_place(::core::ptr::slice_from_raw_parts_mut(
                self.heap.data.as_mut_ptr().add(front),
                len,
            ))
        };
    }
}
impl<T: fmt::Debug, P: AllocationPolicy, L: LengthPolicy> fmt::Debug for Drain<'_, T, P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.remaining()).finish()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> TryIterator for Drain<'_, T, P, L> {
    type Item = T;
    type Error = Infallible;
    fn try_next(&mut self) -> Result<Option<T>, Infallible> {
        Ok(self.next())
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(self.size_hint())
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> TryDoubleEndedIterator for Drain<'_, T, P, L> {
    fn try_next_back(&mut self) -> Result<Option<T>, Infallible> {
        Ok(self.next_back())
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> TryExactSizeIterator for Drain<'_, T, P, L> {
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(self.len())
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> TryFusedIterator for Drain<'_, T, P, L> {}

impl<T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator for BinaryHeap<T, P, L> {
    type Item = T;
    type Error = Infallible;
    type IntoIter = IntoIter<T, P, L>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.into_iter())
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator for &'a BinaryHeap<T, P, L> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = Iter<'a, T>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
pub(super) struct Copied<I>(pub(super) I);
impl<'a, T: Copy + 'a, I: TryIterator<Item = &'a T>> TryIterator for Copied<I> {
    type Item = T;
    type Error = I::Error;
    fn try_next(&mut self) -> Result<Option<T>, I::Error> {
        self.0.try_next().map(|v| v.copied())
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), I::Error> {
        self.0.try_size_hint()
    }
}
