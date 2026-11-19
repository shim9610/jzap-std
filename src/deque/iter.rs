use super::*;
use crate::{
    TryDoubleEndedIterator, TryExactSizeIterator, TryFusedIterator, TryIntoIterator, TryIterator,
    span::Span,
};
use core::{convert::Infallible, iter::FusedIterator};

pub struct Iter<'a, T> {
    first: slice::Iter<'a, T>,
    second: slice::Iter<'a, T>,
}
pub struct IterMut<'a, T> {
    first: slice::IterMut<'a, T>,
    second: slice::IterMut<'a, T>,
}
impl<T> Default for Iter<'_, T> {
    fn default() -> Self {
        Self {
            first: Default::default(),
            second: Default::default(),
        }
    }
}
impl<T> Default for IterMut<'_, T> {
    fn default() -> Self {
        Self {
            first: Default::default(),
            second: Default::default(),
        }
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for IterMut<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.first.as_slice())
            .entries(self.second.as_slice())
            .finish()
    }
}
impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            first: self.first.clone(),
            second: self.second.clone(),
        }
    }
}
impl<T> TryClone for Iter<'_, T> {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(self.clone())
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
macro_rules! borrowed {
    ($name:ident,$item:ty) => {
        impl<'a, T> Iterator for $name<'a, T> {
            type Item = $item;
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.first.next().or_else(|| self.second.next())
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                let n = self.len();
                (n, Some(n))
            }
            fn fold<B, F: FnMut(B, Self::Item) -> B>(self, initial: B, mut f: F) -> B {
                let b = self.first.fold(initial, &mut f);
                self.second.fold(b, f)
            }
        }
        impl<T> DoubleEndedIterator for $name<'_, T> {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.second.next_back().or_else(|| self.first.next_back())
            }
            fn rfold<B, F: FnMut(B, Self::Item) -> B>(self, initial: B, mut f: F) -> B {
                let b = self.second.rfold(initial, &mut f);
                self.first.rfold(b, f)
            }
        }
        impl<T> ExactSizeIterator for $name<'_, T> {
            fn len(&self) -> usize {
                add(self.first.len(), self.second.len())
            }
        }
        impl<T> FusedIterator for $name<'_, T> {}
        impl<'a, T> TryIterator for $name<'a, T> {
            type Item = $item;
            type Error = Infallible;
            fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
                Ok(self.next())
            }
            fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
                Ok(self.size_hint())
            }
        }
        impl<T> TryDoubleEndedIterator for $name<'_, T> {
            fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
                Ok(self.next_back())
            }
        }
        impl<T> TryExactSizeIterator for $name<'_, T> {
            fn try_len(&self) -> Result<usize, Infallible> {
                Ok(self.len())
            }
        }
        impl<T> TryFusedIterator for $name<'_, T> {}
    };
}
borrowed!(Iter, &'a T);
borrowed!(IterMut, &'a mut T);
impl<T, P: AllocationPolicy, L: LengthPolicy> VecDeque<T, P, L> {
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        let (a, b) = self.as_slices();
        Iter {
            first: a.iter(),
            second: b.iter(),
        }
    }
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        let (a, b) = self.as_mut_slices();
        IterMut {
            first: a.iter_mut(),
            second: b.iter_mut(),
        }
    }
    pub fn range(&self, range: impl RangeBounds<usize>) -> Result<Iter<'_, T>, Error> {
        let (start, end) = crate::vec::edit::checked_range(range, self.len())?;
        let (a, b) = self.as_slices();
        let n = a.len();
        let (a, b) = if start >= n {
            (
                &[][..],
                b.get(sub(start, n)..sub(end, n)).ok_or(Error::Bounds)?,
            )
        } else if end <= n {
            (a.get(start..end).ok_or(Error::Bounds)?, &[][..])
        } else {
            (
                a.get(start..).ok_or(Error::Bounds)?,
                b.get(..sub(end, n)).ok_or(Error::Bounds)?,
            )
        };
        Ok(Iter {
            first: a.iter(),
            second: b.iter(),
        })
    }
    pub fn range_mut(&mut self, range: impl RangeBounds<usize>) -> Result<IterMut<'_, T>, Error> {
        let (start, end) = crate::vec::edit::checked_range(range, self.len())?;
        let (a, b) = self.as_mut_slices();
        let n = a.len();
        let (a, b) = if start >= n {
            (
                &mut [][..],
                b.get_mut(sub(start, n)..sub(end, n)).ok_or(Error::Bounds)?,
            )
        } else if end <= n {
            (a.get_mut(start..end).ok_or(Error::Bounds)?, &mut [][..])
        } else {
            (
                a.get_mut(start..).ok_or(Error::Bounds)?,
                b.get_mut(..sub(end, n)).ok_or(Error::Bounds)?,
            )
        };
        Ok(IterMut {
            first: a.iter_mut(),
            second: b.iter_mut(),
        })
    }
    pub fn drain(&mut self, range: impl RangeBounds<usize>) -> Result<Drain<'_, T, P, L>, Error> {
        let (start, end) = crate::vec::edit::checked_range(range, self.len())?;
        let span = Span::try_new(start, end)?;
        let tail = sub(self.len(), end);
        self.len = Checked::new(start);
        Ok(Drain {
            owner: self,
            span,
            start,
            end,
            tail,
        })
    }
}

pub struct IntoIter<T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    pub(super) owner: VecDeque<T, P, L>,
}
impl<T, P: AllocationPolicy, L: LengthPolicy> IntoIter<T, P, L> {
    pub fn as_slices(&self) -> (&[T], &[T]) {
        self.owner.as_slices()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Iterator for IntoIter<T, P, L> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.owner.pop_front()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
    fn count(self) -> usize {
        self.owner.len()
    }
    fn last(mut self) -> Option<T> {
        self.owner.pop_back()
    }
    /// Like std's VecDeque consuming fold, scan contiguous segments and publish
    /// retirement once. The guard also retires the current value before unwinding.
    fn fold<B, F: FnMut(B, T) -> B>(mut self, initial: B, mut f: F) -> B {
        struct Retire<'a, T, P: AllocationPolicy, L: LengthPolicy> {
            owner: &'a mut VecDeque<T, P, L>,
            consumed: usize,
        }
        impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Retire<'_, T, P, L> {
            fn drop(&mut self) {
                if self.consumed != 0 {
                    self.owner.len = Checked::new(sub(self.owner.len(), self.consumed));
                    if mem::size_of::<T>() != 0 {
                        self.owner.head = Checked::new(geometry::physical(
                            self.owner.head.get(),
                            self.consumed,
                            self.owner.capacity(),
                        ))
                    }
                }
            }
        }
        let mut guard = Retire {
            owner: &mut self.owner,
            consumed: 0,
        };
        let (a, b) = guard.owner.as_slices();
        let count = &mut guard.consumed;
        let mut consume = |acc, value: &T| {
            *count = add(*count, 1);
            // SAFETY: each segment slot is visited once; guard retires it before
            // the moved value can reach callback destruction/unwinding.
            f(acc, unsafe { ptr::read(value) })
        };
        let value = a.iter().fold(initial, &mut consume);
        b.iter().fold(value, consume)
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> DoubleEndedIterator for IntoIter<T, P, L> {
    fn next_back(&mut self) -> Option<T> {
        self.owner.pop_back()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> ExactSizeIterator for IntoIter<T, P, L> {
    fn len(&self) -> usize {
        self.owner.len()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> FusedIterator for IntoIter<T, P, L> {}
impl<T: TryClone, P: AllocationPolicy, L: LengthPolicy> TryClone for IntoIter<T, P, L> {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self {
            owner: self.owner.try_clone()?,
        })
    }
}
impl<T: core::fmt::Debug, P: AllocationPolicy, L: LengthPolicy> core::fmt::Debug
    for IntoIter<T, P, L>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.owner.fmt(f)
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for VecDeque<T, P, L> {
    type Item = T;
    type IntoIter = IntoIter<T, P, L>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter { owner: self }
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for &'a VecDeque<T, P, L> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for &'a mut VecDeque<T, P, L> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator for VecDeque<T, P, L> {
    type Item = T;
    type Error = Infallible;
    type IntoIter = IntoIter<T, P, L>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.into_iter())
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator for &'a VecDeque<T, P, L> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = Iter<'a, T>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> TryIntoIterator for &'a mut VecDeque<T, P, L> {
    type Item = &'a mut T;
    type Error = Infallible;
    type IntoIter = IterMut<'a, T>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter_mut())
    }
}

/// Forgets may leak removed elements and the tail; the retained prefix remains valid.
pub struct Drain<'a, T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    owner: &'a mut VecDeque<T, P, L>,
    span: Span,
    start: usize,
    end: usize,
    tail: usize,
}
impl<T: core::fmt::Debug, P: AllocationPolicy, L: LengthPolicy> core::fmt::Debug
    for Drain<'_, T, P, L>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut remaining = self.span;
        f.debug_list()
            .entries(core::iter::from_fn(|| {
                remaining.next().map(|i| {
                    // SAFETY: the copied span visits only remaining initialized drain
                    // slots. The shared borrow prevents retirement during formatting.
                    unsafe { &*self.owner.slot(i) }
                })
            }))
            .finish()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Iterator for Drain<'_, T, P, L> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.span.next().map(|i| {
            // SAFETY: span retires one original initialized slot before transferring ownership.
            unsafe { self.owner.slot(i).read() }
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> DoubleEndedIterator for Drain<'_, T, P, L> {
    fn next_back(&mut self) -> Option<T> {
        self.span.next_back().map(|i| {
            // SAFETY: unique old initialized slot retired by the same bounded span.
            unsafe { self.owner.slot(i).read() }
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
        struct Finish<'a, 'b, T, P: AllocationPolicy, L: LengthPolicy>(&'a mut Drain<'b, T, P, L>);
        impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Finish<'_, '_, T, P, L> {
            fn drop(&mut self) {
                // Finish retirement even if a destructor unwinds once; a second panic is the callback contract.
                for value in self.0.by_ref() {
                    drop(value)
                }
                let d = &mut self.0;
                let removed = sub(d.end, d.start);
                if removed != 0 {
                    // SAFETY: all drain slots retired; move the shorter initialized side into the hole.
                    unsafe {
                        if d.start < d.tail {
                            d.owner.copy_ring(
                                d.owner.physical(0),
                                d.owner.physical(removed),
                                d.start,
                                true,
                            );
                            if mem::size_of::<T>() != 0 {
                                d.owner.head = Checked::new(geometry::physical(
                                    d.owner.head.get(),
                                    removed,
                                    d.owner.capacity(),
                                ))
                            }
                        } else if d.tail != 0 {
                            d.owner.copy_ring(
                                d.owner.physical(d.end),
                                d.owner.physical(d.start),
                                d.tail,
                                false,
                            )
                        }
                    }
                }
                d.owner.len = Checked::new(add(d.start, d.tail));
            }
        }
        if !mem::needs_drop::<T>() {
            // Retire all remaining trivial values at once, including unbounded
            // logical ZST lengths. Finish still closes the removed range.
            self.span.clear();
        }
        let guard = Finish(self);
        for value in guard.0.by_ref() {
            drop(value)
        }
    }
}
macro_rules! owned_fallible {
    ($name:ident $(,$lt:lifetime)?) => {
        impl<$($lt,)?T,P:AllocationPolicy,L:LengthPolicy> TryIterator for $name<$($lt,)?T,P,L>{type Item=T;type Error=Infallible;
            fn try_next(&mut self)->Result<Option<T>,Infallible>{Ok(self.next())}
            fn try_size_hint(&self)->Result<(usize,Option<usize>),Infallible>{Ok(self.size_hint())}}
        impl<$($lt,)?T,P:AllocationPolicy,L:LengthPolicy> TryDoubleEndedIterator for $name<$($lt,)?T,P,L>{fn try_next_back(&mut self)->Result<Option<T>,Infallible>{Ok(self.next_back())}}
        impl<$($lt,)?T,P:AllocationPolicy,L:LengthPolicy> TryExactSizeIterator for $name<$($lt,)?T,P,L>{fn try_len(&self)->Result<usize,Infallible>{Ok(self.len())}}
        impl<$($lt,)?T,P:AllocationPolicy,L:LengthPolicy> TryFusedIterator for $name<$($lt,)?T,P,L>{}
    }
}
owned_fallible!(IntoIter);
owned_fallible!(Drain,'a);
