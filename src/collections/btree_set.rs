//! Ordered unique ownership with explicit allocation and callback failures.
//!
//! Uses the existing map's node and ownership protocol. Borrowed algebra does
//! not allocate; owned algebra is explicit and fallible. See `docs/btree-set-design.md`.
use super::btree_map::{self, BTreeMap};
use crate::{
    Error, Rejected, TryClone, TryCloneIn, TryIntoIterator, TryIterator,
    budget::{AllocationPolicy, Budget, Unmetered},
};
use core::{borrow::Borrow, cmp::Ordering, ops::RangeBounds};
mod algebra;
mod iter;
#[cfg(kani)]
mod proofs;
pub use algebra::{Difference, Intersection, SymmetricDifference, Union};
pub use iter::{ExtractIf, IntoIter, Iter, Range};

pub struct BTreeSet<T, P: AllocationPolicy = Unmetered> {
    map: BTreeMap<T, (), P>,
}
impl<T> BTreeSet<T> {
    pub const fn new() -> Self {
        Self::new_in(Unmetered)
    }
}
impl<T> Default for BTreeSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> crate::TryDefault for BTreeSet<T> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
impl<T, P: AllocationPolicy> BTreeSet<T, P> {
    pub const fn new_in(policy: P) -> Self {
        Self {
            map: BTreeMap::new_in(policy),
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn allocation_policy(&self) -> &P {
        self.map.allocation_policy()
    }
    pub fn clear(&mut self) {
        self.map.clear()
    }
    #[inline]
    pub fn first(&self) -> Option<&T> {
        self.map.first_key_value().map(|(k, _)| k)
    }
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.map.last_key_value().map(|(k, _)| k)
    }
    pub fn pop_first(&mut self) -> Option<T> {
        self.map.pop_first().map(|(k, _)| k)
    }
    pub fn pop_last(&mut self) -> Option<T> {
        self.map.pop_last().map(|(k, _)| k)
    }
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.map.iter())
    }
    pub fn contains<Q: ?Sized + Ord>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.map.contains_key(value)
    }
    pub fn get<Q: ?Sized + Ord>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
    {
        self.map.get_key_value(value).map(|(k, _)| k)
    }
    pub fn remove<Q: ?Sized + Ord>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.take(value).is_some()
    }
    pub fn take<Q: ?Sized + Ord>(&mut self, value: &Q) -> Option<T>
    where
        T: Borrow<Q>,
    {
        self.map.remove_entry(value).map(|(k, _)| k)
    }
    pub fn try_range<Q: ?Sized + Ord, R: RangeBounds<Q>>(
        &self,
        range: R,
    ) -> Result<Range<'_, T>, Error>
    where
        T: Borrow<Q>,
    {
        self.map.try_range(range).map(Range)
    }
    pub fn try_split_off<Q: ?Sized + Ord>(&mut self, value: &Q) -> Result<Self, Error>
    where
        T: Borrow<Q>,
    {
        self.map.try_split_off(value).map(|map| Self { map })
    }
    /// Earlier removals remain on predicate error; the current object is retained.
    pub fn try_retain<E>(
        &mut self,
        mut predicate: impl FnMut(&T) -> Result<bool, E>,
    ) -> Result<(), E> {
        self.map.try_retain(|k, _| predicate(k))
    }
}
fn rejected<T>(e: Rejected<(T, ())>) -> Rejected<T> {
    Rejected {
        value: e.value.0,
        error: e.error,
    }
}
impl<T: Ord, P: AllocationPolicy> BTreeSet<T, P> {
    pub fn try_insert(&mut self, value: T) -> Result<bool, Rejected<T>> {
        self.map
            .try_insert(value, ())
            .map(|old| old.is_none())
            .map_err(rejected)
    }
    pub fn try_replace(&mut self, value: T) -> Result<Option<T>, Rejected<T>> {
        self.map.try_replace_key(value, ()).map_err(rejected)
    }
    pub fn try_append(&mut self, other: &mut Self) -> Result<(), Error> {
        self.map.try_append(&mut other.map)
    }
    /// A callback error consumes that visit and retains the candidate. Dropping
    /// the iterator leaves unvisited objects in the set.
    pub fn try_extract_if<R: RangeBounds<T>, E, F: FnMut(&T) -> Result<bool, E>>(
        &mut self,
        range: R,
        mut predicate: F,
    ) -> Result<impl crate::TryFusedIterator<Item = T, Error = E>, Error> {
        self.map
            .try_extract_if(range, move |k, _| predicate(k))
            .map(ExtractIf)
    }
    pub fn try_extend<I: TryIntoIterator<Item = T>>(
        &mut self,
        source: I,
    ) -> Result<(), ExtendError<T, I::Error>> {
        let mut iter = source.try_into_iter().map_err(ExtendError::Source)?;
        while let Some(value) = iter.try_next().map_err(ExtendError::Source)? {
            self.try_insert(value).map_err(ExtendError::Insert)?;
        }
        Ok(())
    }
    pub fn try_from_iter_in<I: TryIntoIterator<Item = T>>(
        source: I,
        policy: P,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        let mut result = Self::new_in(policy);
        result.try_extend(source)?;
        Ok(result)
    }
}
impl<T: Ord> BTreeSet<T> {
    pub fn try_from_iter<I: TryIntoIterator<Item = T>>(
        source: I,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        Self::try_from_iter_in(source, Unmetered)
    }
    pub fn try_from_array<const N: usize>(
        source: [T; N],
    ) -> Result<Self, ExtendError<T, core::convert::Infallible>> {
        Self::try_from_iter(source)
    }
}
#[derive(Debug)]
pub enum ExtendError<T, E> {
    Source(E),
    Insert(Rejected<T>),
}
impl<T: TryClone, P: AllocationPolicy> TryClone for BTreeSet<T, P> {
    fn try_clone(&self) -> Result<Self, Error> {
        self.map.try_clone().map(|map| Self { map })
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        self.map.try_clone_from(&source.map)
    }
}
impl<'a, T: TryCloneIn<'a>, P: AllocationPolicy> TryCloneIn<'a> for BTreeSet<T, P> {
    type Output = BTreeSet<T::Output, Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        self.map.try_clone_in(budget).map(|map| BTreeSet { map })
    }
}
impl<T: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug for BTreeSet<T, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
impl<T: PartialEq, P: AllocationPolicy, Q: AllocationPolicy> PartialEq<BTreeSet<T, Q>>
    for BTreeSet<T, P>
{
    fn eq(&self, other: &BTreeSet<T, Q>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<T: Eq, P: AllocationPolicy> Eq for BTreeSet<T, P> {}
impl<T: PartialOrd, P: AllocationPolicy> PartialOrd for BTreeSet<T, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}
impl<T: Ord, P: AllocationPolicy> Ord for BTreeSet<T, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}
impl<T: core::hash::Hash, P: AllocationPolicy> core::hash::Hash for BTreeSet<T, P> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for key in self.iter() {
            key.hash(state)
        }
    }
}
