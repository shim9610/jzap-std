use super::*;
use crate::{TryExactSizeIterator, TryFusedIterator};
use core::{
    convert::Infallible, iter::FusedIterator, marker::PhantomData, mem::ManuallyDrop, ptr::NonNull,
};

#[derive(Clone)]
pub(super) struct Scan {
    ctrl: NonNull<u8>,
    mask: usize,
    group: usize,
    bits: u16,
    remaining: Checked<usize>,
}
impl Scan {
    pub(super) fn new<K, V>(table: &RawTable<K, V>) -> Self {
        Self {
            ctrl: table.ctrl,
            mask: table.mask,
            group: 0,
            bits: 0,
            remaining: table.len,
        }
    }
    #[inline(always)]
    fn fill_group(&mut self) -> bool {
        while self.bits == 0 {
            if self.group > self.mask {
                self.remaining = Checked::new(0);
                return false;
            }
            // SAFETY: bounded group plus the initialized mirror; group advances
            // monotonically, so retired groups are never visited again.
            self.bits = unsafe { !group::vacant(self.ctrl.as_ptr().add(self.group)) };
            self.group = add(self.group, group::WIDTH);
        }
        true
    }
    /// Nonempty cached mask and positive remaining count, established by the
    /// valid frontier invariant (checked by the actual Rust frontier kernel).
    #[inline(always)]
    fn take_cached(&mut self) -> usize {
        let index = add(sub(self.group, group::WIDTH), group::first(self.bits));
        self.bits = group::clear_first(self.bits);
        self.remaining = Checked::new(sub(self.remaining.get(), 1));
        index
    }
    #[inline(always)]
    pub(super) fn next(&mut self) -> Option<usize> {
        if self.remaining.get() == 0 || !self.fill_group() {
            return None;
        }
        Some(self.take_cached())
    }
    #[inline(always)]
    fn fold_indices<B, F: FnMut(B, usize) -> B>(&mut self, mut value: B, mut f: F) -> B {
        while self.remaining.get() > 0 {
            if !self.fill_group() {
                break;
            }
            // A cached FULL bit implies a remaining initialized owner. Draining
            // the mask removes the count test from the per-entry fold path.
            while self.bits != 0 {
                value = f(value, self.take_cached());
            }
        }
        value
    }
}
pub struct Iter<'a, K, V> {
    scan: Scan,
    marker: PhantomData<&'a (K, V)>,
}
pub struct IterMut<'a, K, V> {
    scan: Scan,
    marker: PhantomData<(&'a K, &'a mut V)>,
}
impl<'a, K, V> Iter<'a, K, V> {
    pub(super) fn new(table: &'a RawTable<K, V>) -> Self {
        Self {
            scan: Scan::new(table),
            marker: PhantomData,
        }
    }
}
impl<'a, K, V> IterMut<'a, K, V> {
    pub(super) fn new(table: &'a mut RawTable<K, V>) -> Self {
        Self {
            scan: Scan::new(table),
            marker: PhantomData,
        }
    }
    pub fn as_iter(&self) -> Iter<'_, K, V> {
        Iter {
            scan: self.scan.clone(),
            marker: PhantomData,
        }
    }
}
impl<K, V> Clone for Iter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            scan: self.scan.clone(),
            marker: PhantomData,
        }
    }
}
impl<K, V> Default for Iter<'_, K, V> {
    fn default() -> Self {
        Self {
            scan: Scan::new(&RawTable::<K, V>::empty()),
            marker: PhantomData,
        }
    }
}
impl<K, V> Default for IterMut<'_, K, V> {
    fn default() -> Self {
        Self {
            scan: Scan::new(&RawTable::<K, V>::empty()),
            marker: PhantomData,
        }
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.scan.next()?;
        // SAFETY: scan yields each FULL slot once under the shared source borrow.
        unsafe {
            let p = self.scan.ctrl.as_ptr().cast::<(K, V)>().sub(add(index, 1));
            Some((&*ptr::addr_of!((*p).0), &*ptr::addr_of!((*p).1)))
        }
    }
    #[inline]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(mut self, init: B, mut f: F) -> B {
        let ctrl = self.scan.ctrl;
        self.scan.fold_indices(init, |value, index| {
            // SAFETY: the valid scan retires each initialized slot once. Field
            // borrows preserve earlier yielded mutable references.
            let item = unsafe {
                let p = ctrl.as_ptr().cast::<(K, V)>().sub(add(index, 1));
                (&*ptr::addr_of!((*p).0), &*ptr::addr_of!((*p).1))
            };
            f(value, item)
        })
    }
    fn count(self) -> usize {
        self.scan.remaining.get()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.scan.remaining.get();
        (n, Some(n))
    }
}
impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.scan.next()?;
        // SAFETY: scan never repeats a physical index. Field-only borrows avoid
        // aliasing earlier live mutable values in the same allocation.
        unsafe {
            let p = self.scan.ctrl.as_ptr().cast::<(K, V)>().sub(add(index, 1));
            Some((&*ptr::addr_of!((*p).0), &mut *ptr::addr_of_mut!((*p).1)))
        }
    }
    #[inline]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(mut self, init: B, mut f: F) -> B {
        let ctrl = self.scan.ctrl;
        self.scan.fold_indices(init, |value, index| {
            // SAFETY: the valid scan retires each initialized slot once. Field
            // borrows preserve earlier yielded mutable references.
            let item = unsafe {
                let p = ctrl.as_ptr().cast::<(K, V)>().sub(add(index, 1));
                (&*ptr::addr_of!((*p).0), &mut *ptr::addr_of_mut!((*p).1))
            };
            f(value, item)
        })
    }
    fn count(self) -> usize {
        self.scan.remaining.get()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.scan.remaining.get();
        (n, Some(n))
    }
}
// SAFETY: the iterators access only initialized key/value fields. Source-policy
// references are not used; the source borrow holds its allocation alive.
unsafe impl<K: Sync, V: Sync> Send for Iter<'_, K, V> {}
unsafe impl<K: Sync, V: Sync> Sync for Iter<'_, K, V> {}
unsafe impl<K: Send, V: Send> Send for IterMut<'_, K, V> {}
unsafe impl<K: Sync, V: Sync> Sync for IterMut<'_, K, V> {}

pub struct IntoIter<K, V, P: AllocationPolicy = Unmetered> {
    table: RawTable<K, V>,
    scan: Scan,
    policy: P,
}
impl<K, V, P: AllocationPolicy> IntoIter<K, V, P> {
    pub fn as_iter(&self) -> Iter<'_, K, V> {
        Iter {
            scan: self.scan.clone(),
            marker: PhantomData,
        }
    }
}
impl<K, V, P: AllocationPolicy> Iterator for IntoIter<K, V, P> {
    type Item = (K, V);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.scan.next()?;
        // SAFETY: one owned initialized pair; never searched after consumption.
        unsafe {
            // No-drop consumers own only the unvisited Scan frontier: stale
            // tags of moved slots are never read as values or searched. Their
            // final cleanup resets metadata (Drain) or frees storage (IntoIter).
            if core::mem::needs_drop::<(K, V)>() {
                self.table.set_control(index, group::EMPTY);
                self.table.len = Checked::new(sub(self.table.len.get(), 1));
            }
            Some(self.table.slot(index).read())
        }
    }
    #[inline]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(mut self, init: B, mut f: F) -> B {
        let table = &mut self.table;
        self.scan.fold_indices(init, |value, index| {
            // SAFETY: same retirement protocol as next; publish retirement
            // before the callback, whose unwind drops only the unvisited owners.
            let item = unsafe {
                if core::mem::needs_drop::<(K, V)>() {
                    table.set_control(index, group::EMPTY);
                    table.len = Checked::new(sub(table.len.get(), 1));
                }
                table.slot(index).read()
            };
            f(value, item)
        })
    }
    fn count(self) -> usize {
        self.scan.remaining.get()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.scan.remaining.get();
        (n, Some(n))
    }
}
impl<K, V, P: AllocationPolicy> Drop for IntoIter<K, V, P> {
    fn drop(&mut self) {
        let table = mem::replace(&mut self.table, RawTable::empty());
        drop(raw::Owner {
            table,
            budget: self.policy.budget(),
        });
    }
}
unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send for IntoIter<K, V, P> {}
unsafe impl<K: Sync, V: Sync, P: AllocationPolicy + Sync> Sync for IntoIter<K, V, P> {}
impl<K, V, S, P: AllocationPolicy> IntoIterator for HashMap<K, V, S, P> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V, P>;
    fn into_iter(self) -> Self::IntoIter {
        let mut source = ManuallyDrop::new(self);
        // SAFETY: move table/policy once; suppress source drop. Construct the
        // owning iterator before dropping S, so an S destructor unwind cleans it.
        unsafe {
            let table = ptr::read(&source.table);
            let scan = Scan::new(&table);
            let policy = ptr::read(&source.policy);
            let result = IntoIter {
                table,
                scan,
                policy,
            };
            ptr::drop_in_place(&mut source.hasher);
            result
        }
    }
}
impl<'a, K, V, S, P: AllocationPolicy> IntoIterator for &'a HashMap<K, V, S, P> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, K, V, S, P: AllocationPolicy> IntoIterator for &'a mut HashMap<K, V, S, P> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

pub struct Keys<'a, K, V>(pub(super) Iter<'a, K, V>);
pub struct Values<'a, K, V>(pub(super) Iter<'a, K, V>);
pub struct ValuesMut<'a, K, V>(pub(super) IterMut<'a, K, V>);
pub struct IntoKeys<K, V, P: AllocationPolicy = Unmetered>(pub(super) IntoIter<K, V, P>);
pub struct IntoValues<K, V, P: AllocationPolicy = Unmetered>(pub(super) IntoIter<K, V, P>);
macro_rules! wrapper{
    ([$($g:tt)*] $ty:ty,$item:ty,$map:expr)=>{
        impl<$($g)*> Iterator for $ty{type Item=$item;#[inline]fn next(&mut self)->Option<Self::Item>{self.0.next().map($map)}fn size_hint(&self)->(usize,Option<usize>){self.0.size_hint()}fn count(self)->usize{self.0.count()}fn fold<B,F:FnMut(B,Self::Item)->B>(self,init:B,mut f:F)->B{self.0.fold(init,|a,item|f(a,($map)(item)))}}
    };
}
wrapper!(['a,K,V] Keys<'a,K,V>,&'a K,|(k,_)|k);
wrapper!(['a,K,V] Values<'a,K,V>,&'a V,|(_,v)|v);
wrapper!(['a,K,V] ValuesMut<'a,K,V>,&'a mut V,|(_,v)|v);
// Each owning projection drops the discarded half while the returned half is still a local:
// if that destructor unwinds, the returned half is dropped too instead of leaking.
wrapper!([K,V,P:AllocationPolicy] IntoKeys<K,V,P>,K,|(k,v)|{drop(v);k});
wrapper!([K,V,P:AllocationPolicy] IntoValues<K,V,P>,V,|(k,v)|{drop(k);v});
impl<K, V> Clone for Keys<'_, K, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<K, V> Clone for Values<'_, K, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<K, V> Default for Keys<'_, K, V> {
    fn default() -> Self {
        Self(Iter::default())
    }
}
impl<K, V> Default for Values<'_, K, V> {
    fn default() -> Self {
        Self(Iter::default())
    }
}

impl<K, V> Default for ValuesMut<'_, K, V> {
    fn default() -> Self {
        Self(IterMut::default())
    }
}
impl<K, V> Default for IntoKeys<K, V> {
    fn default() -> Self {
        Self(IntoIter::default())
    }
}
impl<K, V> Default for IntoValues<K, V> {
    fn default() -> Self {
        Self(IntoIter::default())
    }
}

pub struct Drain<'a, K, V, S = RandomState, P: AllocationPolicy = Unmetered> {
    map: &'a mut HashMap<K, V, S, P>,
    table: RawTable<K, V>,
    scan: Scan,
}
impl<'a, K, V, S, P: AllocationPolicy> Drain<'a, K, V, S, P> {
    pub(super) fn new(map: &'a mut HashMap<K, V, S, P>) -> Self {
        let table = mem::replace(&mut map.table, RawTable::empty());
        let scan = Scan::new(&table);
        Self { map, table, scan }
    }
    pub(in crate::collections) fn as_iter(&self) -> Iter<'_, K, V> {
        // The active frontier, rather than stale no-drop table tags, identifies
        // the remaining owners. The returned borrow cannot outlive this drain.
        Iter {
            scan: self.scan.clone(),
            marker: PhantomData,
        }
    }
}
impl<K, V, S, P: AllocationPolicy> Iterator for Drain<'_, K, V, S, P> {
    type Item = (K, V);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.scan.next()?;
        // SAFETY: detached table owns each yielded pair; no hash lookup occurs.
        unsafe {
            // No-drop consumers own only the unvisited Scan frontier: stale
            // tags of moved slots are never read as values or searched. Their
            // final cleanup resets metadata (Drain) or frees storage (IntoIter).
            if core::mem::needs_drop::<(K, V)>() {
                self.table.set_control(index, group::EMPTY);
                self.table.len = Checked::new(sub(self.table.len.get(), 1));
            }
            Some(self.table.slot(index).read())
        }
    }
    #[inline]
    fn fold<B, F: FnMut(B, Self::Item) -> B>(mut self, init: B, mut f: F) -> B {
        let table = &mut self.table;
        self.scan.fold_indices(init, |value, index| {
            // SAFETY: same retirement protocol as next; publish retirement
            // before the callback, whose unwind drops only the unvisited owners.
            let item = unsafe {
                if core::mem::needs_drop::<(K, V)>() {
                    table.set_control(index, group::EMPTY);
                    table.len = Checked::new(sub(table.len.get(), 1));
                }
                table.slot(index).read()
            };
            f(value, item)
        })
    }
    fn count(self) -> usize {
        self.scan.remaining.get()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.scan.remaining.get();
        (n, Some(n))
    }
}
struct Restore<'a, K, V> {
    source: &'a mut RawTable<K, V>,
    drained: &'a mut RawTable<K, V>,
}
impl<K, V> Drop for Restore<'_, K, V> {
    fn drop(&mut self) {
        mem::swap(self.source, self.drained)
    }
}
impl<K, V, S, P: AllocationPolicy> Drop for Drain<'_, K, V, S, P> {
    fn drop(&mut self) {
        let restore = Restore {
            source: &mut self.map.table,
            drained: &mut self.table,
        };
        restore.drained.clear();
    }
}
// SAFETY: detached ownership and the exclusive source borrow follow these bounds.
// The drain never accesses or drops the builder S; policy bounds remain conservative.
unsafe impl<K: Send, V: Send, S, P: AllocationPolicy + Send> Send for Drain<'_, K, V, S, P> {}
unsafe impl<K: Sync, V: Sync, S, P: AllocationPolicy + Sync> Sync for Drain<'_, K, V, S, P> {}

pub struct ExtractIf<'a, K, V, S, P: AllocationPolicy, F> {
    map: &'a mut HashMap<K, V, S, P>,
    scan: Scan,
    predicate: F,
}
impl<'a, K, V, S, P: AllocationPolicy, F> ExtractIf<'a, K, V, S, P, F> {
    pub(super) fn new(map: &'a mut HashMap<K, V, S, P>, predicate: F) -> Self {
        let scan = Scan::new(&map.table);
        Self {
            map,
            scan,
            predicate,
        }
    }
}
impl<K, V, S, P: AllocationPolicy, E, F: FnMut(&K, &mut V) -> Result<bool, E>> TryIterator
    for ExtractIf<'_, K, V, S, P, F>
{
    type Item = (K, V);
    type Error = E;
    #[inline]
    fn try_next(&mut self) -> Result<Option<Self::Item>, E> {
        while let Some(index) = self.scan.next() {
            // SAFETY: unvisited FULL fields, held by exclusive map borrow.
            unsafe {
                let p = self.map.table.slot(index);
                if (self.predicate)(&*ptr::addr_of!((*p).0), &mut *ptr::addr_of_mut!((*p).1))? {
                    return Ok(Some(self.map.table.erase(index)));
                }
            }
        }
        Ok(None)
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), E> {
        Ok((0, Some(self.scan.remaining.get())))
    }
}
impl<K, V, S, P: AllocationPolicy, E, F: FnMut(&K, &mut V) -> Result<bool, E>> TryFusedIterator
    for ExtractIf<'_, K, V, S, P, F>
{
}
// SAFETY: extraction touches only table key/value fields and the predicate.
// Its exclusive source borrow keeps storage alive; the builder is never accessed.
unsafe impl<K: Send, V: Send, S, P: AllocationPolicy + Send, F: Send> Send
    for ExtractIf<'_, K, V, S, P, F>
{
}
unsafe impl<K: Sync, V: Sync, S, P: AllocationPolicy + Sync, F: Sync> Sync
    for ExtractIf<'_, K, V, S, P, F>
{
}
impl<K: core::fmt::Debug, V: core::fmt::Debug, S, P: AllocationPolicy, F> core::fmt::Debug
    for ExtractIf<'_, K, V, S, P, F>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExtractIf").finish_non_exhaustive()
    }
}
macro_rules! mirrors{
    ([$($g:tt)*] $ty:ty)=>{
        impl<$($g)*> ExactSizeIterator for $ty{fn len(&self)->usize{self.size_hint().0}}
        impl<$($g)*> FusedIterator for $ty{}
        impl<$($g)*> TryIterator for $ty{type Item=<Self as Iterator>::Item;type Error=Infallible;#[inline]fn try_next(&mut self)->Result<Option<Self::Item>,Infallible>{Ok(self.next())}fn try_size_hint(&self)->Result<(usize,Option<usize>),Infallible>{Ok(self.size_hint())}}
        impl<$($g)*> TryExactSizeIterator for $ty{fn try_len(&self)->Result<usize,Infallible>{Ok(self.len())}}
        impl<$($g)*> TryFusedIterator for $ty{}
    };
}
mirrors!(['a,K,V] Iter<'a,K,V>);
mirrors!(['a,K,V] IterMut<'a,K,V>);
mirrors!(['a,K,V] Keys<'a,K,V>);
mirrors!(['a,K,V] Values<'a,K,V>);
mirrors!(['a,K,V] ValuesMut<'a,K,V>);
mirrors!([K,V,P:AllocationPolicy] IntoIter<K,V,P>);
mirrors!([K,V,P:AllocationPolicy] IntoKeys<K,V,P>);
mirrors!([K,V,P:AllocationPolicy] IntoValues<K,V,P>);
mirrors!(['a,K,V,S,P:AllocationPolicy] Drain<'a,K,V,S,P>);

impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for IterMut<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.as_iter()).finish()
    }
}
impl<K: core::fmt::Debug, V> core::fmt::Debug for Keys<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<K, V: core::fmt::Debug> core::fmt::Debug for Values<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<K, V: core::fmt::Debug> core::fmt::Debug for ValuesMut<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.0.as_iter().map(|(_, v)| v))
            .finish()
    }
}
impl<K: core::fmt::Debug, V: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug
    for IntoIter<K, V, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.as_iter()).finish()
    }
}
impl<K: core::fmt::Debug, V, P: AllocationPolicy> core::fmt::Debug for IntoKeys<K, V, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.0.as_iter().map(|(k, _)| k))
            .finish()
    }
}
impl<K, V: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug for IntoValues<K, V, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.0.as_iter().map(|(_, v)| v))
            .finish()
    }
}
impl<K: core::fmt::Debug, V: core::fmt::Debug, S, P: AllocationPolicy> core::fmt::Debug
    for Drain<'_, K, V, S, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(Iter::<K, V> {
                scan: self.scan.clone(),
                marker: PhantomData,
            })
            .finish()
    }
}

impl<K, V> Default for IntoIter<K, V> {
    fn default() -> Self {
        let table = RawTable::empty();
        let scan = Scan::new(&table);
        Self {
            table,
            scan,
            policy: Unmetered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forgotten_drain_leaves_owner_empty_and_can_be_reclaimed_by_test() {
        let mut m = HashMap::with_hasher(RandomState::from_seed(0, 0));
        for k in 0..11 {
            m.try_insert(k, k * 2).unwrap();
        }
        let mut drain = m.drain();
        // Test-only access retains the detached allocation for explicit cleanup;
        // forgetting the real public drain intentionally leaks, exactly as std.
        let detached = mem::replace(&mut drain.table, RawTable::empty());
        mem::forget(drain);
        assert!(m.is_empty());
        assert_eq!(m.try_insert(4, 9).unwrap(), None);
        assert_eq!(m.get(&4), Some(&9));
        drop(raw::Owner {
            table: detached,
            budget: Unmetered.budget(),
        });
    }
}

#[cfg(kani)]
#[path = "scan_proofs.rs"]
mod scan_proofs;
