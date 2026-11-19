use super::{BTreeMap, Cursor, node};
use crate::{
    Error, Rejected,
    budget::{AllocationPolicy, Unmetered},
    error::InitError,
};
pub enum Entry<'a, K, V, P: AllocationPolicy = Unmetered> {
    Occupied(OccupiedEntry<'a, K, V, P>),
    Vacant(VacantEntry<'a, K, V, P>),
}
pub struct OccupiedEntry<'a, K, V, P: AllocationPolicy = Unmetered> {
    pub(super) map: &'a mut BTreeMap<K, V, P>,
    pub(super) at: Cursor<K, V>,
}
pub struct VacantEntry<'a, K, V, P: AllocationPolicy = Unmetered> {
    pub(super) map: &'a mut BTreeMap<K, V, P>,
    pub(super) key: K,
    pub(super) at: Option<Cursor<K, V>>,
}
// SAFETY: an occupied entry transfers an exclusive map borrow and its cursor.
unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send for OccupiedEntry<'_, K, V, P> {}
// SAFETY: a shared entry borrow only exposes shared key/value access.
unsafe impl<K: Sync, V: Sync, P: AllocationPolicy + Sync> Sync for OccupiedEntry<'_, K, V, P> {}
// SAFETY: the vacant key and exclusively borrowed map move together.
unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send for VacantEntry<'_, K, V, P> {}
// SAFETY: shared access cannot mutate the key, map or insertion cursor.
unsafe impl<K: Sync, V: Sync, P: AllocationPolicy + Sync> Sync for VacantEntry<'_, K, V, P> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug
    for OccupiedEntry<'_, K, V, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}
impl<K: core::fmt::Debug, V, P: AllocationPolicy> core::fmt::Debug for VacantEntry<'_, K, V, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VacantEntry")
            .field("key", self.key())
            .finish()
    }
}
impl<K: core::fmt::Debug, V: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug
    for Entry<'_, K, V, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Occupied(e) => e.fmt(f),
            Self::Vacant(e) => e.fmt(f),
        }
    }
}
impl<'a, K, V, P: AllocationPolicy> OccupiedEntry<'a, K, V, P> {
    pub fn key(&self) -> &K {
        // SAFETY: occupied cursor remains initialized during this entry's borrow.
        unsafe { &*node::key(self.at.node, self.at.index()) }
    }
    pub fn get(&self) -> &V {
        // SAFETY: initialized value, shared borrow of exclusive entry.
        unsafe { &*node::val(self.at.node, self.at.index()) }
    }
    pub fn get_mut(&mut self) -> &mut V {
        // SAFETY: unique entry borrow and initialized value.
        unsafe { &mut *node::val(self.at.node, self.at.index()) }
    }
    pub fn into_mut(self) -> &'a mut V {
        // SAFETY: consume exclusive entry, transfer its map borrow to this slot.
        unsafe { &mut *node::val(self.at.node, self.at.index()) }
    }
    pub fn insert(&mut self, value: V) -> V {
        core::mem::replace(self.get_mut(), value)
    }
    pub fn remove_entry(self) -> (K, V) {
        // SAFETY: valid initialized cursor and exclusively borrowed complete map.
        unsafe { self.map.remove_at(self.at, &mut []) }
    }
    pub fn remove(self) -> V {
        // Drop the discarded half while the returned half is still a local: if that
        // destructor unwinds, the returned half is dropped too instead of leaking.
        let (discarded, value) = self.remove_entry();
        drop(discarded);
        value
    }
}
impl<'a, K, V, P: AllocationPolicy> VacantEntry<'a, K, V, P> {
    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn into_key(self) -> K {
        self.key
    }
    pub fn try_insert(self, value: V) -> Result<&'a mut V, Rejected<(K, V)>> {
        self.try_insert_entry(value).map(OccupiedEntry::into_mut)
    }
    pub fn try_insert_entry(
        self,
        value: V,
    ) -> Result<OccupiedEntry<'a, K, V, P>, Rejected<(K, V)>> {
        let at = self.map.try_insert_at(self.at, self.key, value)?;
        Ok(OccupiedEntry { map: self.map, at })
    }
    /// Reserve before invoking f. A callback error preserves the map and key;
    /// callback side effects remain. The callback does not run on storage refusal.
    pub fn try_insert_with<E>(
        self,
        f: impl FnOnce(&K) -> Result<V, E>,
    ) -> Result<&'a mut V, Rejected<K, InitError<E>>> {
        if let Some(at) = self.at {
            // SAFETY: entry exclusively borrows the map and its searched leaf.
            if unsafe { node::len(at.node) } < super::CAP {
                let new_len = match self.map.len + crate::Checked::new(1) {
                    Ok(n) => n,
                    Err(_) => {
                        return Err(Rejected {
                            value: self.key,
                            error: InitError::Storage(Error::Capacity),
                        });
                    }
                };
                let value = match f(&self.key) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(Rejected {
                            value: self.key,
                            error: InitError::Initialize(e),
                        });
                    }
                };
                // SAFETY: safe callback code cannot change the borrowed map's
                // structure. Length admission preceded the callback and writes.
                let at = unsafe { self.map.commit_fit(at, new_len, self.key, value) };
                return Ok(OccupiedEntry { map: self.map, at }.into_mut());
            }
        }
        let plan = match self.map.plan_insert(self.at) {
            Ok(p) => p,
            Err(e) => {
                return Err(Rejected {
                    value: self.key,
                    error: InitError::Storage(e),
                });
            }
        };
        let value = match f(&self.key) {
            Ok(v) => v,
            Err(e) => {
                return Err(Rejected {
                    value: self.key,
                    error: InitError::Initialize(e),
                });
            }
        };
        let at = self.map.commit_insert(plan, self.key, value);
        // SAFETY: successful commit initialized this unique value slot.
        Ok(unsafe { &mut *node::val(at.node, at.index()) })
    }
}
impl<'a, K, V, P: AllocationPolicy> Entry<'a, K, V, P> {
    pub fn try_insert_entry(
        self,
        value: V,
    ) -> Result<OccupiedEntry<'a, K, V, P>, Rejected<(K, V)>> {
        match self {
            Self::Occupied(mut e) => {
                drop(e.insert(value));
                Ok(e)
            }
            Self::Vacant(e) => e.try_insert_entry(value),
        }
    }
    pub fn key(&self) -> &K {
        match self {
            Self::Occupied(e) => e.key(),
            Self::Vacant(e) => e.key(),
        }
    }
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
        f: impl FnOnce() -> Result<V, E>,
    ) -> Result<&'a mut V, Rejected<K, InitError<E>>> {
        self.try_or_insert_with_key(|_| f())
    }
    pub fn try_or_insert_with_key<E>(
        self,
        f: impl FnOnce(&K) -> Result<V, E>,
    ) -> Result<&'a mut V, Rejected<K, InitError<E>>> {
        match self {
            Self::Occupied(e) => Ok(e.into_mut()),
            Self::Vacant(e) => e.try_insert_with(f),
        }
    }
    pub fn try_or_default(self) -> Result<&'a mut V, Rejected<K, InitError<Error>>>
    where
        V: crate::TryDefault,
    {
        self.try_or_insert_with(V::try_default)
    }
    pub fn try_and_modify<E>(self, f: impl FnOnce(&mut V) -> Result<(), E>) -> Result<Self, E> {
        match self {
            Self::Occupied(mut e) => {
                f(e.get_mut())?;
                Ok(Self::Occupied(e))
            }
            other => Ok(other),
        }
    }
}
