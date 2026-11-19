use super::*;
use crate::{TryExactSizeIterator, TryFusedIterator};
use core::{convert::Infallible, iter::FusedIterator};

pub struct Iter<'a, T>(pub(super) hash_map::Keys<'a, T, ()>);
pub struct IntoIter<T, P: AllocationPolicy = Unmetered>(hash_map::IntoKeys<T, (), P>);
pub struct Drain<'a, T, S = RandomState, P: AllocationPolicy = Unmetered>(
    pub(super) hash_map::Drain<'a, T, (), S, P>,
);
pub struct ExtractIf<'a, T, S, P: AllocationPolicy, F>(
    pub(super) hash_map::ExtractIf<'a, T, (), S, P, F>,
);

impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Default for Iter<'_, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl<T> Default for IntoIter<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
    fn count(self) -> usize {
        self.0.count()
    }
    #[inline(always)]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, f: F) -> B {
        self.0.fold(init, f)
    }
}
impl<T, P: AllocationPolicy> Iterator for IntoIter<T, P> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
    fn count(self) -> usize {
        self.0.count()
    }
    #[inline(always)]
    fn fold<B, F: FnMut(B, T) -> B>(self, init: B, f: F) -> B {
        self.0.fold(init, f)
    }
}
impl<T, S, P: AllocationPolicy> Iterator for Drain<'_, T, S, P> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.next().map(|(key, _)| key)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
    fn count(self) -> usize {
        self.0.count()
    }
    #[inline(always)]
    fn fold<B, F: FnMut(B, T) -> B>(self, init: B, mut f: F) -> B {
        self.0.fold(init, |value, (key, _)| f(value, key))
    }
}
macro_rules! mirrors {
    ([$($g:tt)*] $ty:ty) => {
        impl<$($g)*> ExactSizeIterator for $ty { fn len(&self) -> usize { self.size_hint().0 } }
        impl<$($g)*> FusedIterator for $ty {}
        impl<$($g)*> TryIterator for $ty {
            type Item = <Self as Iterator>::Item;
            type Error = Infallible;
            #[inline] fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> { Ok(self.next()) }
            fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> { Ok(self.size_hint()) }
        }
        impl<$($g)*> TryExactSizeIterator for $ty { fn try_len(&self) -> Result<usize, Infallible> { Ok(self.len()) } }
        impl<$($g)*> TryFusedIterator for $ty {}
    };
}
mirrors!(['a, T] Iter<'a, T>);
mirrors!([T, P: AllocationPolicy] IntoIter<T, P>);
mirrors!(['a, T, S, P: AllocationPolicy] Drain<'a, T, S, P>);
impl<T, S, P: AllocationPolicy> IntoIterator for HashSet<T, S, P> {
    type Item = T;
    type IntoIter = IntoIter<T, P>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.map.into_keys())
    }
}
impl<'a, T, S, P: AllocationPolicy> IntoIterator for &'a HashSet<T, S, P> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T, S, P: AllocationPolicy> IntoIterator for &'a mut HashSet<T, S, P> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T, S, P: AllocationPolicy> crate::TryIntoIterator for HashSet<T, S, P> {
    type Item = T;
    type Error = Infallible;
    type IntoIter = IntoIter<T, P>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.into_iter())
    }
}
impl<'a, T, S, P: AllocationPolicy> crate::TryIntoIterator for &'a HashSet<T, S, P> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = Iter<'a, T>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
impl<'a, T, S, P: AllocationPolicy> crate::TryIntoIterator for &'a mut HashSet<T, S, P> {
    type Item = &'a T;
    type Error = Infallible;
    type IntoIter = Iter<'a, T>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Infallible> {
        Ok(self.iter())
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
impl<T: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug for IntoIter<T, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
impl<T: core::fmt::Debug, S, P: AllocationPolicy> core::fmt::Debug for Drain<'_, T, S, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.0.as_iter().map(|(key, _)| key))
            .finish()
    }
}
impl<T, S, P: AllocationPolicy, E, F: FnMut(&T, &mut ()) -> Result<bool, E>> TryIterator
    for ExtractIf<'_, T, S, P, F>
{
    type Item = T;
    type Error = E;
    fn try_next(&mut self) -> Result<Option<T>, E> {
        self.0.try_next().map(|p| p.map(|(key, _)| key))
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), E> {
        self.0.try_size_hint()
    }
}
impl<T, S, P: AllocationPolicy, E, F: FnMut(&T, &mut ()) -> Result<bool, E>> TryFusedIterator
    for ExtractIf<'_, T, S, P, F>
{
}
impl<T: core::fmt::Debug, S, P: AllocationPolicy, F> core::fmt::Debug
    for ExtractIf<'_, T, S, P, F>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
