//! Fallible iteration for existing library owners and borrowed slices.
use super::*;
#[cfg(feature = "alloc")]
use crate::{budget::AllocationPolicy, vec::LengthPolicy};
impl<'a, T> TryIterator for core::slice::Iter<'a, T> {
    type Item = &'a T;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(Iterator::next(self))
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(Iterator::size_hint(self))
    }
}
impl<'a, T> TryDoubleEndedIterator for core::slice::Iter<'a, T> {
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(DoubleEndedIterator::next_back(self))
    }
}
impl<'a, T> TryExactSizeIterator for core::slice::Iter<'a, T> {
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(ExactSizeIterator::len(self))
    }
}
impl<'a, T> TryFusedIterator for core::slice::Iter<'a, T> {}
impl<'a, T> TryIterator for core::slice::IterMut<'a, T> {
    type Item = &'a mut T;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(Iterator::next(self))
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(Iterator::size_hint(self))
    }
}
impl<'a, T> TryDoubleEndedIterator for core::slice::IterMut<'a, T> {
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(DoubleEndedIterator::next_back(self))
    }
}
impl<'a, T> TryExactSizeIterator for core::slice::IterMut<'a, T> {
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(ExactSizeIterator::len(self))
    }
}
impl<'a, T> TryFusedIterator for core::slice::IterMut<'a, T> {}
impl<T, const N: usize> TryIterator for core::array::IntoIter<T, N> {
    type Item = T;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(Iterator::next(self))
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(Iterator::size_hint(self))
    }
}
impl<T, const N: usize> TryDoubleEndedIterator for core::array::IntoIter<T, N> {
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(DoubleEndedIterator::next_back(self))
    }
}
impl<T, const N: usize> TryExactSizeIterator for core::array::IntoIter<T, N> {
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(ExactSizeIterator::len(self))
    }
}
impl<T, const N: usize> TryFusedIterator for core::array::IntoIter<T, N> {}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryIterator for crate::vec::IntoIter<T, P, L> {
    type Item = T;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(Iterator::next(self))
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(Iterator::size_hint(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryDoubleEndedIterator
    for crate::vec::IntoIter<T, P, L>
{
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(DoubleEndedIterator::next_back(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryExactSizeIterator
    for crate::vec::IntoIter<T, P, L>
{
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(ExactSizeIterator::len(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryFusedIterator for crate::vec::IntoIter<T, P, L> {}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy> TryIterator for crate::boxed::IntoIter<T, P> {
    type Item = T;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(Iterator::next(self))
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(Iterator::size_hint(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy> TryDoubleEndedIterator for crate::boxed::IntoIter<T, P> {
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(DoubleEndedIterator::next_back(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy> TryExactSizeIterator for crate::boxed::IntoIter<T, P> {
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(ExactSizeIterator::len(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy> TryFusedIterator for crate::boxed::IntoIter<T, P> {}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryIterator for crate::vec::Drain<'_, T, P, L> {
    type Item = T;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(Iterator::next(self))
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(Iterator::size_hint(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryDoubleEndedIterator
    for crate::vec::Drain<'_, T, P, L>
{
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(DoubleEndedIterator::next_back(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryExactSizeIterator
    for crate::vec::Drain<'_, T, P, L>
{
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(ExactSizeIterator::len(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryFusedIterator for crate::vec::Drain<'_, T, P, L> {}
#[cfg(feature = "alloc")]
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> TryIterator
    for crate::vec::Splice<'_, T, I, P, L>
{
    type Item = T;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(Iterator::next(self))
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(Iterator::size_hint(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> TryDoubleEndedIterator
    for crate::vec::Splice<'_, T, I, P, L>
{
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
        Ok(DoubleEndedIterator::next_back(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> TryExactSizeIterator
    for crate::vec::Splice<'_, T, I, P, L>
{
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(ExactSizeIterator::len(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> TryFusedIterator
    for crate::vec::Splice<'_, T, I, P, L>
{
}

impl<'a, T> TryIntoIterator for &'a [T] {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = core::slice::Iter<'a, T>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
impl<'a, T> TryIntoIterator for &'a mut [T] {
    type Item = &'a mut T;
    type Error = Infallible;
    type IntoIter = core::slice::IterMut<'a, T>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter_mut())
    }
}
impl<T, const N: usize> TryIntoIterator for [T; N] {
    type Item = T;
    type Error = Infallible;
    type IntoIter = core::array::IntoIter<T, N>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.into_iter())
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator for crate::vec::Vec<T, P, L> {
    type Item = T;
    type Error = Infallible;
    type IntoIter = crate::vec::IntoIter<T, P, L>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.into_iter())
    }
}
#[cfg(feature = "alloc")]
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator for &'a crate::vec::Vec<T, P, L> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = core::slice::Iter<'a, T>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
#[cfg(feature = "alloc")]
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator
    for &'a mut crate::vec::Vec<T, P, L>
{
    type Item = &'a mut T;
    type Error = Infallible;
    type IntoIter = core::slice::IterMut<'a, T>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter_mut())
    }
}
#[cfg(feature = "alloc")]
impl<T, P: AllocationPolicy> TryIntoIterator for crate::boxed::Box<[T], P> {
    type Item = T;
    type Error = Infallible;
    type IntoIter = crate::boxed::IntoIter<T, P>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.into_iter())
    }
}
#[cfg(feature = "alloc")]
impl<'a, T, P: AllocationPolicy> TryIntoIterator for &'a crate::boxed::Box<[T], P> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = core::slice::Iter<'a, T>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
#[cfg(feature = "alloc")]
impl<'a, T, P: AllocationPolicy> TryIntoIterator for &'a mut crate::boxed::Box<[T], P> {
    type Item = &'a mut T;
    type Error = Infallible;
    type IntoIter = core::slice::IterMut<'a, T>;
    #[inline]
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter_mut())
    }
}

#[cfg(feature = "alloc")]
impl<T, E, F: FnMut(&mut T) -> Result<bool, E>, P: AllocationPolicy, L: LengthPolicy> TryIterator
    for crate::vec::ExtractIf<'_, T, F, P, L>
{
    type Item = T;
    type Error = E;
    #[inline]
    fn try_next(&mut self) -> Result<Option<T>, E> {
        Iterator::next(self).transpose()
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), E> {
        Ok(Iterator::size_hint(self))
    }
}
#[cfg(feature = "alloc")]
impl<T, E, F: FnMut(&mut T) -> Result<bool, E>, P: AllocationPolicy, L: LengthPolicy>
    TryFusedIterator for crate::vec::ExtractIf<'_, T, F, P, L>
{
}
