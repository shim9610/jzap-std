//! Fallible grouped hashing. User Hash/Eq/Drop retain their execution contracts.
use crate::{
    Checked, Error, Rejected, TryClone, TryCloneIn, TryDefault, TryIntoIterator, TryIterator,
    arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub},
    budget::{AllocationPolicy, Budget, Unmetered},
};
use core::{
    borrow::Borrow,
    hash::{BuildHasher, Hash},
    mem, ptr,
};
#[cfg(all(
    feature = "std",
    any(
        target_os = "linux",
        target_os = "android",
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
mod entropy;
mod entry;
pub(crate) mod geometry;
pub(crate) mod group;
mod hash;
mod iter;
#[cfg(kani)]
mod proofs;
pub(crate) mod raw;
pub use entry::{Entry, EntryInsertResult, OccupiedEntry, VacantEntry};
pub use hash::{RandomState, SipHasher13};
pub use iter::{
    Drain, ExtractIf, IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Values, ValuesMut,
};
use raw::RawTable;

pub struct HashMap<K, V, S = RandomState, P: AllocationPolicy = Unmetered> {
    table: RawTable<K, V>,
    hasher: S,
    policy: P,
}
// SAFETY: exclusive ownership of every FULL pair and the allocation; all stored
// user components must themselves allow transfer/sharing.
unsafe impl<K: Send, V: Send, S: Send, P: AllocationPolicy + Send> Send for HashMap<K, V, S, P> {}
unsafe impl<K: Sync, V: Sync, S: Sync, P: AllocationPolicy + Sync> Sync for HashMap<K, V, S, P> {}
impl<K, V> HashMap<K, V> {
    pub fn try_new() -> Result<Self, Error> {
        Ok(Self::with_hasher(RandomState::try_new()?))
    }
    pub fn try_with_capacity(capacity: usize) -> Result<Self, Error> {
        Self::try_with_capacity_and_hasher(capacity, RandomState::try_new()?)
    }
}
impl<K, V, P: AllocationPolicy> HashMap<K, V, RandomState, P> {
    pub fn try_new_in(policy: P) -> Result<Self, Error> {
        Ok(Self::with_hasher_in(RandomState::try_new()?, policy))
    }
    pub fn try_with_capacity_in(capacity: usize, policy: P) -> Result<Self, Error> {
        Self::try_with_capacity_and_hasher_in(capacity, RandomState::try_new()?, policy)
    }
}
impl<K, V, S> HashMap<K, V, S> {
    pub const fn with_hasher(hasher: S) -> Self {
        Self::with_hasher_in(hasher, Unmetered)
    }
    pub fn try_with_capacity_and_hasher(capacity: usize, hasher: S) -> Result<Self, Error> {
        Self::try_with_capacity_and_hasher_in(capacity, hasher, Unmetered)
    }
}
impl<K, V, S, P: AllocationPolicy> HashMap<K, V, S, P> {
    pub const fn with_hasher_in(hasher: S, policy: P) -> Self {
        Self {
            table: RawTable::empty(),
            hasher,
            policy,
        }
    }
    pub fn try_with_capacity_and_hasher_in(
        capacity: usize,
        hasher: S,
        policy: P,
    ) -> Result<Self, Error> {
        let table = RawTable::allocate(raw::buckets_for(capacity)?, policy.budget())?;
        Ok(Self {
            table,
            hasher,
            policy,
        })
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.table.len.get()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.table.capacity()
    }
    pub fn hasher(&self) -> &S {
        &self.hasher
    }
    pub fn allocation_policy(&self) -> &P {
        &self.policy
    }
    pub fn clear(&mut self) {
        self.table.clear()
    }
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(&self.table)
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut::new(&mut self.table)
    }
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys(self.iter())
    }
    pub fn values(&self) -> Values<'_, K, V> {
        Values(self.iter())
    }
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut(self.iter_mut())
    }
    pub fn drain(&mut self) -> Drain<'_, K, V, S, P> {
        Drain::new(self)
    }
    pub fn into_keys(self) -> IntoKeys<K, V, P> {
        IntoKeys(self.into_iter())
    }
    pub fn into_values(self) -> IntoValues<K, V, P> {
        IntoValues(self.into_iter())
    }
    pub fn try_retain<E>(
        &mut self,
        mut predicate: impl FnMut(&K, &mut V) -> Result<bool, E>,
    ) -> Result<(), E> {
        let mut scan = iter::Scan::new(&self.table);
        while let Some(index) = scan.next() {
            // SAFETY: the scan yields one initialized slot at a time. Erasing
            // only this yielded slot leaves the remaining cached mask valid.
            unsafe {
                let p = self.table.slot(index);
                if !predicate(&*ptr::addr_of!((*p).0), &mut *ptr::addr_of_mut!((*p).1))? {
                    drop(self.table.erase(index));
                }
            }
        }
        Ok(())
    }
    pub fn try_extract_if<E, F: FnMut(&K, &mut V) -> Result<bool, E>>(
        &mut self,
        predicate: F,
    ) -> ExtractIf<'_, K, V, S, P, F> {
        ExtractIf::new(self, predicate)
    }
}
impl<K, V, S: BuildHasher, P: AllocationPolicy> HashMap<K, V, S, P> {
    #[inline]
    fn hash<Q: ?Sized + Hash>(&self, key: &Q) -> u64 {
        self.hasher.hash_one(key)
    }
    #[inline]
    fn find<Q: ?Sized + Hash + Eq>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
    {
        if self.is_empty() {
            return None;
        }
        self.table.find(self.hash(key), |k| k.borrow() == key)
    }
    #[inline(always)]
    pub fn get<Q: ?Sized + Hash + Eq>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        let index = self.find(key)?;
        // SAFETY: find returns an initialized pair held live by the map borrow.
        Some(unsafe { &*ptr::addr_of!((*self.table.slot(index)).1) })
    }
    #[inline]
    pub fn get_key_value<Q: ?Sized + Hash + Eq>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
    {
        let index = self.find(key)?;
        // SAFETY: initialized fields under shared map borrow.
        Some(unsafe {
            let p = self.table.slot(index);
            (&*ptr::addr_of!((*p).0), &*ptr::addr_of!((*p).1))
        })
    }
    #[inline(always)]
    pub fn contains_key<Q: ?Sized + Hash + Eq>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.find(key).is_some()
    }
    #[inline]
    pub fn get_mut<Q: ?Sized + Hash + Eq>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        let index = self.find(key)?;
        // SAFETY: exact initialized field under exclusive borrow.
        Some(unsafe { &mut *ptr::addr_of_mut!((*self.table.slot(index)).1) })
    }
    #[inline(always)]
    pub fn remove_entry<Q: ?Sized + Hash + Eq>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
    {
        let index = self.find(key)?;
        // SAFETY: find proves FULL and exclusive map borrow permits moving out.
        Some(unsafe { self.table.erase(index) })
    }
    #[inline(always)]
    pub fn remove<Q: ?Sized + Hash + Eq>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        // Drop the discarded half while the returned half is still a local: if that
        // destructor unwinds, the returned half is dropped too instead of leaking.
        let (discarded, value) = self.remove_entry(key)?;
        drop(discarded);
        Some(value)
    }
    pub fn try_get_disjoint_mut<Q: ?Sized + Hash + Eq, const N: usize>(
        &mut self,
        keys: [&Q; N],
    ) -> Result<[Option<&mut V>; N], Error>
    where
        K: Borrow<Q>,
    {
        let indices = keys.map(|key| self.find(key));
        let mut rest = indices.as_slice();
        while let Some((index, tail)) = rest.split_first() {
            if index.is_some() && tail.contains(index) {
                return Err(Error::Bounds);
            }
            rest = tail;
        }
        Ok(core::array::from_fn(|i| {
            let index = indices.get(i).copied().flatten()?;
            // SAFETY: all physical indices were checked distinct before any
            // mutable reference was constructed, independent of user equality.
            Some(unsafe { &mut *ptr::addr_of_mut!((*self.table.slot(index)).1) })
        }))
    }
    /// # Safety
    /// Every found value must occupy a distinct physical slot.
    pub unsafe fn get_disjoint_unchecked_mut<Q: ?Sized + Hash + Eq, const N: usize>(
        &mut self,
        keys: [&Q; N],
    ) -> [Option<&mut V>; N]
    where
        K: Borrow<Q>,
    {
        let indices = keys.map(|key| self.find(key));
        core::array::from_fn(|i| {
            let index = indices.get(i).copied().flatten()?;
            // SAFETY: caller promises distinct results; map borrow keeps them live.
            Some(unsafe { &mut *ptr::addr_of_mut!((*self.table.slot(index)).1) })
        })
    }
}
impl<K: Hash + Eq, S: BuildHasher, P: AllocationPolicy> HashMap<K, (), S, P> {
    /// Set-only replacement: equal keys must retain their hash/equality class.
    #[inline(always)]
    pub(super) fn try_replace_key(&mut self, key: K) -> Result<Option<K>, Rejected<K>> {
        let hash = self.hash(&key);
        let slot = self.table.find_insert(hash, |stored| stored == &key);
        if let Some(raw::InsertSlot::Occupied(index)) = slot {
            // SAFETY: the occupied search proves one initialized key under an
            // exclusive map borrow. Replacing equal keys preserves the tag,
            // probe class, length, growth credit and value-field ownership.
            return Ok(Some(unsafe { self.table.replace_key(index, key) }));
        }
        let candidate = match slot {
            Some(raw::InsertSlot::Vacant(index)) => Some(index),
            _ => None,
        };
        self.insert_vacant_at(hash, key, (), candidate)
            .map(|_| None)
            .map_err(|e| Rejected {
                value: e.value.0,
                error: e.error,
            })
    }
}
impl<K: Hash + Eq, V, S: BuildHasher, P: AllocationPolicy> HashMap<K, V, S, P> {
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), Error> {
        let required = self.len().checked_add(additional).ok_or(Error::Capacity)?;
        if additional <= self.table.growth.get() {
            return Ok(());
        }
        let full = raw::capacity_for(self.table.mask);
        // Match std's amortization rule: dense deletion churn must grow once,
        // rather than repeatedly paying an O(n) in-place rehash.
        if self.table.mask != 0 && required <= full / 2 {
            let hasher = &self.hasher;
            return self.table.rehash(|key| hasher.hash_one(key));
        }
        self.resize(raw::buckets_for(
            required.max(full.checked_add(1).ok_or(Error::Capacity)?),
        )?)
    }
    fn resize(&mut self, buckets: usize) -> Result<(), Error> {
        if buckets == self.table.buckets() {
            return Ok(());
        }
        let budget = self.policy.budget();
        // Copy, then publish (std's resize): until the swap the source keeps
        // sole ownership of every pair and is never written. The destination
        // holds bitwise copies and frees only its storage on an error or a
        // Hash unwind, so the map, its capacity and accounting stay unchanged.
        let mut destination = raw::Storage {
            table: RawTable::allocate(buckets, budget)?,
            budget,
        };
        let hasher = &self.hasher;
        let source = &self.table;
        let mut scan = iter::Scan::new(source);
        while let Some(index) = scan.next() {
            // SAFETY: the scan yields each initialized source slot once. The
            // vacant destination slot is in a disjoint allocation; its copy
            // stays unowned until publication and is never dropped there.
            unsafe {
                let hash = hasher.hash_one(&*ptr::addr_of!((*source.slot(index)).0));
                let target = destination
                    .table
                    .vacant(hash)
                    .ok_or(Error::InvalidStructure)?;
                ptr::copy_nonoverlapping(source.slot(index), destination.table.slot(target), 1);
                destination.table.set_control(target, raw::tag(hash));
            }
        }
        // Every copied slot was EMPTY, so the vacancy credit is set once.
        let len = self.table.len;
        let growth = raw::capacity_for(destination.table.mask)
            .checked_sub(len.get())
            .ok_or(Error::InvalidStructure)?;
        destination.table.len = len;
        destination.table.growth = Checked::new(growth);
        // Publish; `destination` then retires the source storage without
        // dropping the pairs it no longer owns.
        mem::swap(&mut self.table, &mut destination.table);
        Ok(())
    }
    pub fn try_shrink_to(&mut self, min_capacity: usize) -> Result<(), Error> {
        if min_capacity == 0 && self.is_empty() {
            return self.resize(0);
        }
        let required = min_capacity.max(self.len());
        if required >= raw::capacity_for(self.table.mask) {
            return Ok(());
        }
        let Ok(buckets) = raw::buckets_for(required) else {
            return Ok(());
        };
        if buckets >= self.table.buckets() {
            return Ok(());
        }
        self.resize(buckets)
    }
    pub fn try_shrink_to_fit(&mut self) -> Result<(), Error> {
        self.try_shrink_to(self.len())
    }
    #[inline]
    fn prepare_insert(&mut self, hash: u64) -> Result<usize, Error> {
        self.prepare_insert_at(hash, None)
    }
    #[inline(always)]
    fn prepare_insert_at(&mut self, hash: u64, candidate: Option<usize>) -> Result<usize, Error> {
        if let Some(index) = candidate.or_else(|| self.table.vacant(hash)) {
            // SAFETY: vacant returns a real admitted bucket. A tombstone can be
            // reused even when empty-bucket growth credit has reached zero.
            if self.table.growth.get() > 0 || unsafe { self.table.control(index) } == group::DELETED
            {
                return Ok(index);
            }
        }
        self.try_reserve(1)?;
        self.table.vacant(hash).ok_or(Error::InvalidStructure)
    }
    #[inline]
    fn insert_vacant(&mut self, hash: u64, key: K, value: V) -> Result<usize, Rejected<(K, V)>> {
        self.insert_vacant_at(hash, key, value, None)
    }
    #[inline]
    fn insert_vacant_at(
        &mut self,
        hash: u64,
        key: K,
        value: V,
        candidate: Option<usize>,
    ) -> Result<usize, Rejected<(K, V)>> {
        let index = match self.prepare_insert_at(hash, candidate) {
            Ok(index) => index,
            Err(error) => {
                return Err(Rejected {
                    value: (key, value),
                    error,
                });
            }
        };
        // SAFETY: admission completed before ownership moves into the vacant slot.
        unsafe {
            self.table.insert_at(index, hash, (key, value));
        }
        Ok(index)
    }
    #[inline(always)]
    pub fn try_insert(&mut self, key: K, value: V) -> Result<Option<V>, Rejected<(K, V)>> {
        let hash = self.hash(&key);
        let slot = self.table.find_insert(hash, |stored| stored == &key);
        if let Some(raw::InsertSlot::Occupied(index)) = slot {
            // SAFETY: found initialized slot exclusively borrowed; retain its key.
            let old = unsafe {
                mem::replace(&mut *ptr::addr_of_mut!((*self.table.slot(index)).1), value)
            };
            drop(key);
            return Ok(Some(old));
        }
        let candidate = match slot {
            Some(raw::InsertSlot::Vacant(index)) => Some(index),
            _ => None,
        };
        self.insert_vacant_at(hash, key, value, candidate)
            .map(|_| None)
    }
    #[inline(always)]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, S, P> {
        let hash = self.hash(&key);
        if let Some(index) = self.table.find(hash, |stored| stored == &key) {
            drop(key);
            Entry::Occupied(OccupiedEntry { map: self, index })
        } else {
            Entry::Vacant(VacantEntry {
                map: self,
                key,
                hash,
            })
        }
    }
    pub fn try_extend<I: TryIntoIterator<Item = (K, V)>>(
        &mut self,
        source: I,
    ) -> Result<(), ExtendError<K, V, I::Error>> {
        let mut source = source.try_into_iter().map_err(ExtendError::Source)?;
        while let Some((key, value)) = source.try_next().map_err(ExtendError::Source)? {
            self.try_insert(key, value).map_err(ExtendError::Insert)?;
        }
        Ok(())
    }
    pub fn try_from_iter_in<I: TryIntoIterator<Item = (K, V)>>(
        source: I,
        hasher: S,
        policy: P,
    ) -> Result<Self, ExtendError<K, V, I::Error>> {
        let mut map = Self::with_hasher_in(hasher, policy);
        map.try_extend(source)?;
        Ok(map)
    }
}
#[derive(Debug)]
pub enum ExtendError<K, V, E> {
    Storage(Error),
    Source(E),
    Insert(Rejected<(K, V)>),
}
impl<K: Hash + Eq, V> HashMap<K, V> {
    pub fn try_from_array<const N: usize>(
        source: [(K, V); N],
    ) -> Result<Self, ExtendError<K, V, core::convert::Infallible>> {
        Self::try_from_iter(source)
    }

    pub fn try_from_iter<I: TryIntoIterator<Item = (K, V)>>(
        source: I,
    ) -> Result<Self, ExtendError<K, V, I::Error>> {
        let hasher = RandomState::try_new().map_err(ExtendError::Storage)?;
        Self::try_from_iter_in(source, hasher, Unmetered)
    }
}
impl<K, V, S: TryDefault> TryDefault for HashMap<K, V, S> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::with_hasher(S::try_default()?))
    }
}
impl<K, V, S, P: AllocationPolicy> Drop for HashMap<K, V, S, P> {
    fn drop(&mut self) {
        let table = mem::replace(&mut self.table, RawTable::empty());
        drop(raw::Owner {
            table,
            budget: self.policy.budget(),
        });
    }
}
impl<K: TryClone, V: TryClone, S: TryClone, P: AllocationPolicy> TryClone for HashMap<K, V, S, P> {
    fn try_clone(&self) -> Result<Self, Error> {
        let mut result = Self::with_hasher_in(self.hasher.try_clone()?, self.policy.try_clone()?);
        if let (Some(keys), Some(values)) = (K::copy_witness(), V::copy_witness()) {
            result.table = self
                .table
                .try_clone_copy(result.policy.budget(), keys, values)?;
            return Ok(result);
        }
        result.table = RawTable::allocate(self.table.buckets(), result.policy.budget())?;
        for index in 0..self.table.buckets() {
            // SAFETY: source FULL slots are initialized. Publish the cloned tag
            // only after both fields exist. On error result drops its own prefix.
            unsafe {
                if self.table.control(index) & 0x80 == 0 {
                    let pair = (&*self.table.slot(index)).try_clone()?;
                    result.table.slot(index).write(pair);
                    result.table.set_control(index, self.table.control(index));
                    result.table.len = Checked::new(add(result.table.len.get(), 1));
                }
            }
        }
        if result.table.mask != 0 {
            result.table.growth =
                Checked::new(sub(raw::capacity_for(result.table.mask), result.len()));
        }
        // Copying occupied positions but discarding tombstones would break probe
        // chains. Preserve all metadata, including every tombstone and mirror.
        if self.table.mask != 0 {
            unsafe {
                ptr::copy_nonoverlapping(
                    self.table.ctrl.as_ptr(),
                    result.table.ctrl.as_ptr(),
                    add(self.table.buckets(), group::WIDTH),
                )
            };
            result.table.growth = self.table.growth;
        }
        Ok(result)
    }
}
impl<'a, K: TryCloneIn<'a>, V: TryCloneIn<'a>, S: BuildHasher + TryClone, P: AllocationPolicy>
    TryCloneIn<'a> for HashMap<K, V, S, P>
where
    K::Output: Hash + Eq,
{
    type Output = HashMap<K::Output, V::Output, S, Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        let mut result =
            HashMap::try_with_capacity_and_hasher_in(self.len(), self.hasher.try_clone()?, budget)?;
        for (key, value) in self.iter() {
            let k = key.try_clone_in(budget)?;
            let v = value.try_clone_in(budget)?;
            result.try_insert(k, v).map_err(|e| e.error)?;
        }
        Ok(result)
    }
}
impl<
    K: Eq + Hash,
    V: PartialEq,
    S: BuildHasher,
    T: BuildHasher,
    P: AllocationPolicy,
    Q: AllocationPolicy,
> PartialEq<HashMap<K, V, T, Q>> for HashMap<K, V, S, P>
{
    fn eq(&self, other: &HashMap<K, V, T, Q>) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}
impl<K: Eq + Hash, V: Eq, S: BuildHasher, P: AllocationPolicy> Eq for HashMap<K, V, S, P> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug, S, P: AllocationPolicy> core::fmt::Debug
    for HashMap<K, V, S, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
