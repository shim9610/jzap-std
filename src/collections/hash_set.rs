//! Unique hashed ownership with explicit entropy, allocation and callback errors.
//! Uses the existing HashMap table, budgets and ownership protocol.
use super::hash_map::{self, HashMap, RandomState};
use crate::{
    Error, Rejected, TryClone, TryCloneIn, TryDefault, TryIntoIterator, TryIterator,
    budget::{AllocationPolicy, Budget, Unmetered},
};
use core::{
    borrow::Borrow,
    hash::{BuildHasher, Hash},
};
mod algebra;
mod iter;
#[cfg(kani)]
mod proofs;
pub use algebra::{Difference, Intersection, SymmetricDifference, Union};
pub use iter::{Drain, ExtractIf, IntoIter, Iter};

pub struct HashSet<T, S = RandomState, P: AllocationPolicy = Unmetered> {
    map: HashMap<T, (), S, P>,
}
impl<T> HashSet<T> {
    pub fn try_new() -> Result<Self, Error> {
        HashMap::try_new().map(|map| Self { map })
    }
    pub fn try_with_capacity(capacity: usize) -> Result<Self, Error> {
        HashMap::try_with_capacity(capacity).map(|map| Self { map })
    }
}
impl<T, P: AllocationPolicy> HashSet<T, RandomState, P> {
    pub fn try_new_in(policy: P) -> Result<Self, Error> {
        HashMap::try_new_in(policy).map(|map| Self { map })
    }
    pub fn try_with_capacity_in(capacity: usize, policy: P) -> Result<Self, Error> {
        HashMap::try_with_capacity_in(capacity, policy).map(|map| Self { map })
    }
}
impl<T, S> HashSet<T, S> {
    pub const fn with_hasher(hasher: S) -> Self {
        Self::with_hasher_in(hasher, Unmetered)
    }
    pub fn try_with_capacity_and_hasher(capacity: usize, hasher: S) -> Result<Self, Error> {
        Self::try_with_capacity_and_hasher_in(capacity, hasher, Unmetered)
    }
}
impl<T, S, P: AllocationPolicy> HashSet<T, S, P> {
    pub const fn with_hasher_in(hasher: S, policy: P) -> Self {
        Self {
            map: HashMap::with_hasher_in(hasher, policy),
        }
    }
    pub fn try_with_capacity_and_hasher_in(
        capacity: usize,
        hasher: S,
        policy: P,
    ) -> Result<Self, Error> {
        HashMap::try_with_capacity_and_hasher_in(capacity, hasher, policy).map(|map| Self { map })
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }
    pub fn hasher(&self) -> &S {
        self.map.hasher()
    }
    pub fn allocation_policy(&self) -> &P {
        self.map.allocation_policy()
    }
    pub fn clear(&mut self) {
        self.map.clear()
    }
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.map.keys())
    }
    pub fn drain(&mut self) -> Drain<'_, T, S, P> {
        Drain(self.map.drain())
    }
    /// Earlier removals remain on callback error; the current value is retained.
    pub fn try_retain<E>(
        &mut self,
        mut predicate: impl FnMut(&T) -> Result<bool, E>,
    ) -> Result<(), E> {
        self.map.try_retain(|key, _| predicate(key))
    }
    /// Errors retain the current value and consume that visit. Unvisited values
    /// remain in the set when this iterator is dropped.
    pub fn try_extract_if<E, F: FnMut(&T) -> Result<bool, E>>(
        &mut self,
        mut predicate: F,
    ) -> ExtractIf<'_, T, S, P, impl FnMut(&T, &mut ()) -> Result<bool, E>> {
        ExtractIf(self.map.try_extract_if(move |key, _| predicate(key)))
    }
}
impl<T, S: BuildHasher, P: AllocationPolicy> HashSet<T, S, P> {
    #[inline(always)]
    pub fn contains<Q: ?Sized + Hash + Eq>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.map.contains_key(value)
    }
    #[inline]
    pub fn get<Q: ?Sized + Hash + Eq>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
    {
        self.map.get_key_value(value).map(|(key, _)| key)
    }
    #[inline(always)]
    pub fn remove<Q: ?Sized + Hash + Eq>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.take(value).is_some()
    }
    #[inline(always)]
    pub fn take<Q: ?Sized + Hash + Eq>(&mut self, value: &Q) -> Option<T>
    where
        T: Borrow<Q>,
    {
        self.map.remove_entry(value).map(|(key, _)| key)
    }
}
fn rejected<T>(e: Rejected<(T, ())>) -> Rejected<T> {
    Rejected {
        value: e.value.0,
        error: e.error,
    }
}
impl<T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> HashSet<T, S, P> {
    #[inline(always)]
    pub fn try_insert(&mut self, value: T) -> Result<bool, Rejected<T>> {
        self.map
            .try_insert(value, ())
            .map(|old| old.is_none())
            .map_err(rejected)
    }
    #[inline(always)]
    pub fn try_replace(&mut self, value: T) -> Result<Option<T>, Rejected<T>> {
        self.map.try_replace_key(value)
    }
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), Error> {
        self.map.try_reserve(additional)
    }
    pub fn try_shrink_to(&mut self, min_capacity: usize) -> Result<(), Error> {
        self.map.try_shrink_to(min_capacity)
    }
    pub fn try_shrink_to_fit(&mut self) -> Result<(), Error> {
        self.map.try_shrink_to_fit()
    }
    pub fn try_extend<I: TryIntoIterator<Item = T>>(
        &mut self,
        source: I,
    ) -> Result<(), ExtendError<T, I::Error>> {
        let mut source = source.try_into_iter().map_err(ExtendError::Source)?;
        while let Some(value) = source.try_next().map_err(ExtendError::Source)? {
            self.try_insert(value).map_err(ExtendError::Insert)?;
        }
        Ok(())
    }
    pub fn try_extend_copied<'a, I: TryIntoIterator<Item = &'a T>>(
        &mut self,
        source: I,
    ) -> Result<(), ExtendError<T, I::Error>>
    where
        T: Copy + 'a,
    {
        let mut source = source.try_into_iter().map_err(ExtendError::Source)?;
        while let Some(value) = source.try_next().map_err(ExtendError::Source)? {
            self.try_insert(*value).map_err(ExtendError::Insert)?;
        }
        Ok(())
    }
    pub fn try_from_iter_in<I: TryIntoIterator<Item = T>>(
        source: I,
        hasher: S,
        policy: P,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        let mut result = Self::with_hasher_in(hasher, policy);
        result.try_extend(source)?;
        Ok(result)
    }
}
impl<T: Hash + Eq> HashSet<T> {
    pub fn try_from_iter<I: TryIntoIterator<Item = T>>(
        source: I,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        let hasher = RandomState::try_new().map_err(ExtendError::Storage)?;
        Self::try_from_iter_in(source, hasher, Unmetered)
    }
    pub fn try_from_array<const N: usize>(
        source: [T; N],
    ) -> Result<Self, ExtendError<T, core::convert::Infallible>> {
        Self::try_from_iter(source)
    }
}
#[derive(Debug)]
pub enum ExtendError<T, E> {
    Storage(Error),
    Source(E),
    Insert(Rejected<T>),
}
impl<T, S: TryDefault> TryDefault for HashSet<T, S> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::with_hasher(S::try_default()?))
    }
}
impl<T: TryClone, S: TryClone, P: AllocationPolicy> TryClone for HashSet<T, S, P> {
    fn try_clone(&self) -> Result<Self, Error> {
        self.map.try_clone().map(|map| Self { map })
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        self.map.try_clone_from(&source.map)
    }
}
impl<'a, T: TryCloneIn<'a>, S: BuildHasher + TryClone, P: AllocationPolicy> TryCloneIn<'a>
    for HashSet<T, S, P>
where
    T::Output: Hash + Eq,
{
    type Output = HashSet<T::Output, S, Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        self.map.try_clone_in(budget).map(|map| HashSet { map })
    }
}
impl<T: Hash + Eq, S: BuildHasher, R: BuildHasher, P: AllocationPolicy, Q: AllocationPolicy>
    PartialEq<HashSet<T, R, Q>> for HashSet<T, S, P>
{
    fn eq(&self, other: &HashSet<T, R, Q>) -> bool {
        self.len() == other.len() && self.iter().all(|key| other.contains(key))
    }
}
impl<T: Hash + Eq, S: BuildHasher, P: AllocationPolicy> Eq for HashSet<T, S, P> {}
impl<T: core::fmt::Debug, S, P: AllocationPolicy> core::fmt::Debug for HashSet<T, S, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
