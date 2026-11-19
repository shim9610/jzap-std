use super::*;
use crate::TryFusedIterator;
use core::{convert::Infallible, iter::FusedIterator};

pub struct Difference<'a, T, S = RandomState, P: AllocationPolicy = Unmetered> {
    iter: Iter<'a, T>,
    other: &'a HashSet<T, S, P>,
}
pub struct Intersection<'a, T, S = RandomState, P: AllocationPolicy = Unmetered> {
    iter: Iter<'a, T>,
    other: &'a HashSet<T, S, P>,
}
pub struct Union<'a, T, S = RandomState, P: AllocationPolicy = Unmetered> {
    first: Iter<'a, T>,
    rest: Difference<'a, T, S, P>,
}
pub struct SymmetricDifference<'a, T, S = RandomState, P: AllocationPolicy = Unmetered> {
    first: Difference<'a, T, S, P>,
    rest: Difference<'a, T, S, P>,
}
pub(super) fn combined_hint(a: usize, b: usize) -> (usize, Option<usize>) {
    (0, a.checked_add(b))
}
// Original generic product bodies, shared by the set iterators and Kani.
#[inline]
pub(super) fn filtered_next<I: Iterator>(
    iter: &mut I,
    mut keep: impl FnMut(&I::Item) -> bool,
) -> Option<I::Item> {
    iter.find(|item| keep(item))
}
#[inline(always)]
pub(super) fn filtered_fold<I: Iterator, B>(
    iter: I,
    init: B,
    mut keep: impl FnMut(&I::Item) -> bool,
    mut f: impl FnMut(B, I::Item) -> B,
) -> B {
    iter.fold(
        init,
        |value, item| if keep(&item) { f(value, item) } else { value },
    )
}
#[inline]
pub(super) fn chain_next<A: Iterator, B: Iterator<Item = A::Item>>(
    first: &mut A,
    rest: &mut B,
) -> Option<A::Item> {
    first.next().or_else(|| rest.next())
}
#[inline(always)]
pub(super) fn chain_fold<A: Iterator, B: Iterator<Item = A::Item>, C>(
    first: A,
    rest: B,
    init: C,
    mut f: impl FnMut(C, A::Item) -> C,
) -> C {
    rest.fold(first.fold(init, &mut f), f)
}
#[inline]
pub(super) fn count_step(n: usize) -> usize {
    // A valid set output is bounded by two allocations, each below isize::MAX.
    // The all-word kernel proves saturation unreachable for those counts.
    n.saturating_add(1)
}
impl<T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> HashSet<T, S, P> {
    pub fn difference<'a>(&'a self, other: &'a Self) -> Difference<'a, T, S, P> {
        Difference {
            iter: self.iter(),
            other,
        }
    }
    pub fn intersection<'a>(&'a self, other: &'a Self) -> Intersection<'a, T, S, P> {
        let (small, large) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        Intersection {
            iter: small.iter(),
            other: large,
        }
    }
    pub fn union<'a>(&'a self, other: &'a Self) -> Union<'a, T, S, P> {
        let (small, large) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        Union {
            first: large.iter(),
            rest: small.difference(large),
        }
    }
    pub fn symmetric_difference<'a>(&'a self, other: &'a Self) -> SymmetricDifference<'a, T, S, P> {
        SymmetricDifference {
            first: self.difference(other),
            rest: other.difference(self),
        }
    }
    pub fn is_disjoint(&self, other: &Self) -> bool {
        // Keep the size relation visible in each scan. The optimizer can then
        // infer that a nonempty smaller input implies a nonempty lookup table.
        if self.len() <= other.len() {
            self.iter().all(|key| !other.contains(key))
        } else {
            other.iter().all(|key| !self.contains(key))
        }
    }
    pub fn is_subset(&self, other: &Self) -> bool {
        self.len() <= other.len() && self.iter().all(|key| other.contains(key))
    }
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }
}
impl<T: Hash + Eq + TryClone, S: BuildHasher + TryClone, P: AllocationPolicy> HashSet<T, S, P> {
    /// Owned algebra preserves the left builder and allocation policy. Clone
    /// or storage failure destroys only the private result, retaining both inputs.
    fn try_own<'a>(&self, items: impl Iterator<Item = &'a T>) -> Result<Self, Error>
    where
        T: 'a,
    {
        let mut result = Self::with_hasher_in(
            self.hasher().try_clone()?,
            self.allocation_policy().try_clone()?,
        );
        // The lower bound avoids reserving a large mostly-filtered input.
        result.try_reserve(items.size_hint().0)?;
        for key in items {
            result.try_insert(key.try_clone()?).map_err(|e| e.error)?;
        }
        Ok(result)
    }
    pub fn try_union(&self, other: &Self) -> Result<Self, Error> {
        self.try_own(self.union(other))
    }
    pub fn try_intersection(&self, other: &Self) -> Result<Self, Error> {
        self.try_own(self.intersection(other))
    }
    pub fn try_difference(&self, other: &Self) -> Result<Self, Error> {
        self.try_own(self.difference(other))
    }
    pub fn try_symmetric_difference(&self, other: &Self) -> Result<Self, Error> {
        self.try_own(self.symmetric_difference(other))
    }
}
impl<'a, T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> Iterator for Difference<'a, T, S, P> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        filtered_next(&mut self.iter, |key| !self.other.contains(*key))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.iter.len()))
    }
    #[inline(always)]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, f: F) -> B {
        let other = self.other;
        filtered_fold(self.iter, init, |key| !other.contains(*key), f)
    }
    fn count(self) -> usize {
        self.fold(0, |n, _| count_step(n))
    }
}
impl<'a, T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> Iterator for Intersection<'a, T, S, P> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        filtered_next(&mut self.iter, |key| self.other.contains(*key))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.iter.len()))
    }
    #[inline(always)]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, f: F) -> B {
        let other = self.other;
        filtered_fold(self.iter, init, |key| other.contains(*key), f)
    }
    fn count(self) -> usize {
        self.fold(0, |n, _| count_step(n))
    }
}
impl<'a, T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> Iterator for Union<'a, T, S, P> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        chain_next(&mut self.first, &mut self.rest)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.first.len(),
            combined_hint(self.first.len(), self.rest.iter.len()).1,
        )
    }
    #[inline(always)]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, f: F) -> B {
        chain_fold(self.first, self.rest, init, f)
    }
    fn count(self) -> usize {
        self.fold(0, |n, _| count_step(n))
    }
}
impl<'a, T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> Iterator
    for SymmetricDifference<'a, T, S, P>
{
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        chain_next(&mut self.first, &mut self.rest)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        combined_hint(self.first.iter.len(), self.rest.iter.len())
    }
    #[inline(always)]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, f: F) -> B {
        chain_fold(self.first, self.rest, init, f)
    }
    fn count(self) -> usize {
        self.fold(0, |n, _| count_step(n))
    }
}
impl<T, S, P: AllocationPolicy> Clone for Difference<'_, T, S, P> {
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            other: self.other,
        }
    }
}
impl<T, S, P: AllocationPolicy> Clone for Intersection<'_, T, S, P> {
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            other: self.other,
        }
    }
}
impl<T, S, P: AllocationPolicy> Clone for Union<'_, T, S, P> {
    fn clone(&self) -> Self {
        Self {
            first: self.first.clone(),
            rest: self.rest.clone(),
        }
    }
}
impl<T, S, P: AllocationPolicy> Clone for SymmetricDifference<'_, T, S, P> {
    fn clone(&self) -> Self {
        Self {
            first: self.first.clone(),
            rest: self.rest.clone(),
        }
    }
}
macro_rules! mirrors {
    ($name:ident) => {
        impl<T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> FusedIterator
            for $name<'_, T, S, P>
        {
        }
        impl<T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> TryIterator for $name<'_, T, S, P> {
            type Item = <Self as Iterator>::Item;
            type Error = Infallible;
            #[inline]
            fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
                Ok(self.next())
            }
            fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
                Ok(self.size_hint())
            }
        }
        impl<T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> TryFusedIterator
            for $name<'_, T, S, P>
        {
        }
        impl<T: Hash + Eq + core::fmt::Debug, S: BuildHasher, P: AllocationPolicy> core::fmt::Debug
            for $name<'_, T, S, P>
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_list().entries(self.clone()).finish()
            }
        }
    };
}
mirrors!(Difference);
mirrors!(Intersection);
mirrors!(Union);
mirrors!(SymmetricDifference);
