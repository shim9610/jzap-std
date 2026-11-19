use super::*;
use crate::error::InitError;
pub type EntryInsertResult<'a, K, V, S, P> =
    Result<OccupiedEntry<'a, K, V, S, P>, Rejected<(K, V)>>;

pub enum Entry<'a, K, V, S = RandomState, P: AllocationPolicy = Unmetered> {
    Occupied(OccupiedEntry<'a, K, V, S, P>),
    Vacant(VacantEntry<'a, K, V, S, P>),
}
pub struct OccupiedEntry<'a, K, V, S = RandomState, P: AllocationPolicy = Unmetered> {
    pub(super) map: &'a mut HashMap<K, V, S, P>,
    pub(super) index: usize,
}
pub struct VacantEntry<'a, K, V, S = RandomState, P: AllocationPolicy = Unmetered> {
    pub(super) map: &'a mut HashMap<K, V, S, P>,
    pub(super) key: K,
    pub(super) hash: u64,
}
impl<K, V, S, P: AllocationPolicy> OccupiedEntry<'_, K, V, S, P> {
    pub fn key(&self) -> &K {
        // SAFETY: exclusive map borrow holds this initialized slot in place.
        unsafe { &*ptr::addr_of!((*self.map.table.slot(self.index)).0) }
    }
    pub fn get(&self) -> &V {
        // SAFETY: same occupied slot, shared field borrow.
        unsafe { &*ptr::addr_of!((*self.map.table.slot(self.index)).1) }
    }
    pub fn get_mut(&mut self) -> &mut V {
        // SAFETY: entry exclusively borrows its map and initialized value.
        unsafe { &mut *ptr::addr_of_mut!((*self.map.table.slot(self.index)).1) }
    }
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }
    pub fn remove_entry(self) -> (K, V) {
        // SAFETY: occupied slot, no references survive entry consumption.
        unsafe { self.map.table.erase(self.index) }
    }
    pub fn remove(self) -> V {
        // Drop the discarded half while the returned half is still a local: if that
        // destructor unwinds, the returned half is dropped too instead of leaking.
        let (discarded, value) = self.remove_entry();
        drop(discarded);
        value
    }
}
impl<'a, K, V, S, P: AllocationPolicy> OccupiedEntry<'a, K, V, S, P> {
    pub fn into_mut(self) -> &'a mut V {
        // SAFETY: consume exclusive entry to transfer its original map lifetime.
        unsafe { &mut *ptr::addr_of_mut!((*self.map.table.slot(self.index)).1) }
    }
}
impl<K, V, S, P: AllocationPolicy> VacantEntry<'_, K, V, S, P> {
    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn into_key(self) -> K {
        self.key
    }
}
impl<'a, K: Hash + Eq, V, S: BuildHasher, P: AllocationPolicy> VacantEntry<'a, K, V, S, P> {
    pub fn try_insert(self, value: V) -> Result<&'a mut V, Rejected<(K, V)>> {
        self.try_insert_entry(value).map(OccupiedEntry::into_mut)
    }
    pub fn try_insert_entry(self, value: V) -> EntryInsertResult<'a, K, V, S, P> {
        let index = self.map.insert_vacant(self.hash, self.key, value)?;
        Ok(OccupiedEntry {
            map: self.map,
            index,
        })
    }
    fn initialize<E>(
        self,
        init: impl FnOnce(&K) -> Result<V, E>,
    ) -> Result<&'a mut V, InitError<E>> {
        let index = self
            .map
            .prepare_insert(self.hash)
            .map_err(InitError::Storage)?;
        let value = init(&self.key).map_err(InitError::Initialize)?;
        // SAFETY: admission precedes the callback, which cannot structurally
        // access the exclusively borrowed map. Publish only the complete pair.
        unsafe {
            self.map
                .table
                .insert_at(index, self.hash, (self.key, value));
            Ok(&mut *ptr::addr_of_mut!((*self.map.table.slot(index)).1))
        }
    }
}
impl<'a, K, V, S, P: AllocationPolicy> Entry<'a, K, V, S, P> {
    pub fn key(&self) -> &K {
        match self {
            Self::Occupied(e) => e.key(),
            Self::Vacant(e) => e.key(),
        }
    }
    pub fn try_and_modify<E>(
        mut self,
        modify: impl FnOnce(&mut V) -> Result<(), E>,
    ) -> Result<Self, E> {
        if let Self::Occupied(e) = &mut self {
            modify(e.get_mut())?
        }
        Ok(self)
    }
}
impl<'a, K: Hash + Eq, V, S: BuildHasher, P: AllocationPolicy> Entry<'a, K, V, S, P> {
    pub fn try_or_insert(self, value: V) -> Result<&'a mut V, Rejected<(K, V)>> {
        match self {
            Self::Occupied(e) => {
                drop(value);
                Ok(e.into_mut())
            }
            Self::Vacant(e) => e.try_insert(value),
        }
    }
    pub fn try_or_insert_with<E>(
        self,
        init: impl FnOnce() -> Result<V, E>,
    ) -> Result<&'a mut V, InitError<E>> {
        self.try_or_insert_with_key(|_| init())
    }
    pub fn try_or_insert_with_key<E>(
        self,
        init: impl FnOnce(&K) -> Result<V, E>,
    ) -> Result<&'a mut V, InitError<E>> {
        match self {
            Self::Occupied(e) => Ok(e.into_mut()),
            Self::Vacant(e) => e.initialize(init),
        }
    }
    pub fn try_or_default(self) -> Result<&'a mut V, Error>
    where
        V: TryDefault,
    {
        self.try_or_insert_with(V::try_default)
            .map_err(|e| match e {
                InitError::Storage(e) | InitError::Initialize(e) => e,
            })
    }
    pub fn try_insert_entry(self, value: V) -> EntryInsertResult<'a, K, V, S, P> {
        match self {
            Self::Occupied(mut e) => {
                drop(e.insert(value));
                Ok(e)
            }
            Self::Vacant(e) => e.try_insert_entry(value),
        }
    }
}
impl<K: core::fmt::Debug, V: core::fmt::Debug, S, P: AllocationPolicy> core::fmt::Debug
    for OccupiedEntry<'_, K, V, S, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}
impl<K: core::fmt::Debug, V, S, P: AllocationPolicy> core::fmt::Debug
    for VacantEntry<'_, K, V, S, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VacantEntry")
            .field("key", self.key())
            .finish()
    }
}
impl<K: core::fmt::Debug, V: core::fmt::Debug, S, P: AllocationPolicy> core::fmt::Debug
    for Entry<'_, K, V, S, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Occupied(e) => e.fmt(f),
            Self::Vacant(e) => e.fmt(f),
        }
    }
}
