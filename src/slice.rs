//! Checked, allocation-free initialized slice views. Raw interoperability is explicit.
use crate::{
    Error, TryClone, TryIntoIterator,
    iter::{FromStd, from_std},
};
use core::{
    convert::Infallible,
    fmt,
    ops::{Bound, RangeBounds},
};
pub(crate) mod geometry;

pub struct Slice<'a, T> {
    inner: &'a [T],
}
pub struct SliceMut<'a, T> {
    inner: &'a mut [T],
}
impl<T> Copy for Slice<'_, T> {}
impl<T> Clone for Slice<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> TryClone for Slice<'_, T> {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(*self)
    }
}

fn normalized_bounds(
    range: impl RangeBounds<usize>,
    length: usize,
) -> Result<(usize, usize), Error> {
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(n) => *n,
        Bound::Excluded(n) => n.checked_add(1).ok_or(Error::Bounds)?,
    };
    let end = match range.end_bound() {
        Bound::Unbounded => length,
        Bound::Excluded(n) => *n,
        Bound::Included(n) => n.checked_add(1).ok_or(Error::Bounds)?,
    };
    Ok((start, end))
}
pub(crate) fn checked_range(
    bounds: impl RangeBounds<usize>,
    length: usize,
) -> Result<(usize, usize), Error> {
    let (start, end) = normalized_bounds(bounds, length)?;
    geometry::range(start, end, length)
}
impl<'a, T> Slice<'a, T> {
    pub const fn new(inner: &'a [T]) -> Self {
        Self { inner }
    }
    pub const fn len(&self) -> usize {
        self.inner.len()
    }
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Explicit standard-slice interoperability; standard operations retain their own contracts.
    pub const fn as_slice(&self) -> &'a [T] {
        self.inner
    }
    pub fn get(&self, index: usize) -> Result<&'a T, Error> {
        self.inner.get(index).ok_or(Error::Bounds)
    }
    pub fn first(&self) -> Option<&'a T> {
        self.inner.first()
    }
    pub fn last(&self) -> Option<&'a T> {
        self.inner.last()
    }
    pub fn range(&self, bounds: impl RangeBounds<usize>) -> Result<Self, Error> {
        let (a, b) = normalized_bounds(bounds, self.len())?;
        self.inner.get(a..b).map(Self::new).ok_or(Error::Bounds)
    }
    pub fn split_at(&self, mid: usize) -> Result<(Self, Self), Error> {
        self.inner
            .split_at_checked(mid)
            .map(|(a, b)| (Self::new(a), Self::new(b)))
            .ok_or(Error::Bounds)
    }
    pub fn iter(&self) -> FromStd<core::slice::Iter<'a, T>> {
        from_std(self.inner.iter())
    }
    #[cfg(feature = "alloc")]
    pub fn try_to_vec(&self) -> Result<crate::vec::Vec<T>, Error>
    where
        T: TryClone,
    {
        crate::vec::Vec::try_from_slice(self.inner)
    }
}
impl<'a, T> SliceMut<'a, T> {
    pub fn new(inner: &'a mut [T]) -> Self {
        Self { inner }
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    pub fn as_view(&self) -> Slice<'_, T> {
        Slice::new(self.inner)
    }
    /// Explicit standard-slice interoperability.
    pub fn as_slice(&self) -> &[T] {
        self.inner
    }
    /// Explicit standard-slice interoperability under the exclusive borrow.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.inner
    }
    pub fn into_slice(self) -> &'a mut [T] {
        self.inner
    }
    pub fn into_shared(self) -> Slice<'a, T> {
        Slice::new(self.inner)
    }
    pub fn get(&self, index: usize) -> Result<&T, Error> {
        self.inner.get(index).ok_or(Error::Bounds)
    }
    pub fn get_mut(&mut self, index: usize) -> Result<&mut T, Error> {
        self.inner.get_mut(index).ok_or(Error::Bounds)
    }
    pub fn range(&self, bounds: impl RangeBounds<usize>) -> Result<Slice<'_, T>, Error> {
        self.as_view().range(bounds)
    }
    pub fn range_mut(&mut self, bounds: impl RangeBounds<usize>) -> Result<SliceMut<'_, T>, Error> {
        let (a, b) = normalized_bounds(bounds, self.len())?;
        self.inner
            .get_mut(a..b)
            .map(SliceMut::new)
            .ok_or(Error::Bounds)
    }
    pub fn split_at_mut(
        &mut self,
        mid: usize,
    ) -> Result<(SliceMut<'_, T>, SliceMut<'_, T>), Error> {
        self.inner
            .split_at_mut_checked(mid)
            .map(|(a, b)| (SliceMut::new(a), SliceMut::new(b)))
            .ok_or(Error::Bounds)
    }
    pub fn copy_from_slice(&mut self, source: &[T]) -> Result<(), Error>
    where
        T: Copy,
    {
        if self.len() != source.len() {
            return Err(Error::Bounds);
        }
        self.inner.copy_from_slice(source);
        Ok(())
    }
    pub fn swap_with_slice(&mut self, other: &mut [T]) -> Result<(), Error> {
        if self.len() != other.len() {
            return Err(Error::Bounds);
        }
        self.inner.swap_with_slice(other);
        Ok(())
    }
    pub fn swap(&mut self, a: usize, b: usize) -> Result<(), Error> {
        if a >= self.len() || b >= self.len() {
            return Err(Error::Bounds);
        }
        if a == b {
            return Ok(());
        }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let (left, right) = self.inner.split_at_mut_checked(hi).ok_or(Error::Bounds)?;
        core::mem::swap(
            left.get_mut(lo).ok_or(Error::Bounds)?,
            right.first_mut().ok_or(Error::Bounds)?,
        );
        Ok(())
    }
    pub fn copy_within(
        &mut self,
        source: impl RangeBounds<usize>,
        destination: usize,
    ) -> Result<(), Error>
    where
        T: Copy,
    {
        let (a, b) = checked_range(source, self.len())?;
        let count = crate::arithmetic::bounded::wrapping_sub(b, a);
        geometry::admitted_end(destination, count, self.len()).ok_or(Error::Bounds)?;
        self.inner.copy_within(a..b, destination);
        Ok(())
    }
    pub fn iter(&self) -> FromStd<core::slice::Iter<'_, T>> {
        from_std(self.inner.iter())
    }
    pub fn iter_mut(&mut self) -> FromStd<core::slice::IterMut<'_, T>> {
        from_std(self.inner.iter_mut())
    }
    #[cfg(feature = "alloc")]
    pub fn try_to_vec(&self) -> Result<crate::vec::Vec<T>, Error>
    where
        T: TryClone,
    {
        self.as_view().try_to_vec()
    }
}
impl<T: fmt::Debug> fmt::Debug for Slice<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
impl<T: fmt::Debug> fmt::Debug for SliceMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
impl<T: PartialEq<U>, U> PartialEq<Slice<'_, U>> for Slice<'_, T> {
    fn eq(&self, other: &Slice<'_, U>) -> bool {
        self.inner == other.inner
    }
}
impl<T: Eq> Eq for Slice<'_, T> {}
impl<'a, T> TryIntoIterator for Slice<'a, T> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = FromStd<core::slice::Iter<'a, T>>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
impl<'a, T> TryIntoIterator for &Slice<'a, T> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = FromStd<core::slice::Iter<'a, T>>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
impl<'a, 'b, T> TryIntoIterator for &'b mut SliceMut<'a, T> {
    type Item = &'b mut T;
    type Error = Infallible;
    type IntoIter = FromStd<core::slice::IterMut<'b, T>>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter_mut())
    }
}
#[cfg(kani)]
#[path = "slice/proofs.rs"]
mod proofs;
