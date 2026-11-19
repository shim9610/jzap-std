//! An owned ordered map with explicit allocation failure and budget accounting.
//!
//! Insertion reserves its entire split chain before mutation. On a returned
//! error the map and the rejected key/value are preserved. Comparators and
//! destructors retain the user-code execution contract described in the crate
//! documentation. Support and verification status: `docs/btree-design.md`.
use crate::{
    Checked, Error, Rejected, TryClone,
    arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub},
    budget::{AllocationPolicy, Unmetered},
};
use core::{borrow::Borrow, cmp::Ordering, marker::PhantomData, mem, ptr};
mod bulk;
mod entry;
pub(crate) mod geometry;
#[cfg(kani)]
mod geometry_proofs;
mod iter;
mod node;
mod operations;
#[cfg(kani)]
mod proofs;
pub mod view;
pub use entry::{Entry, OccupiedEntry, VacantEntry};
use iter::Cursor;
pub use iter::{
    IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Range, RangeMut, Values, ValuesMut,
};
use node::{CAP, Link, MIN, Spares};
pub use operations::{ExtendError, ExtractIf};

/// An ordered map. The default policy adds no policy word to the map or nodes.
pub struct BTreeMap<K, V, P: AllocationPolicy = Unmetered> {
    root: Option<Link<K, V>>,
    height: Checked<usize>,
    len: Checked<usize>,
    policy: P,
    marker: PhantomData<(K, V)>,
}
// SAFETY: every node/payload is exclusively owned; policy has its own Send bounds.
unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send for BTreeMap<K, V, P> {}
// SAFETY: shared access exposes only shared keys/values and policy accesses.
unsafe impl<K: Sync, V: Sync, P: AllocationPolicy + Sync> Sync for BTreeMap<K, V, P> {}
impl<
    K: core::panic::RefUnwindSafe,
    V: core::panic::RefUnwindSafe,
    P: AllocationPolicy + core::panic::UnwindSafe,
> core::panic::UnwindSafe for BTreeMap<K, V, P>
{
}
impl<K, V> BTreeMap<K, V> {
    pub const fn new() -> Self {
        Self::new_in(Unmetered)
    }
}
impl<K, V> Default for BTreeMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    pub const fn new_in(policy: P) -> Self {
        Self {
            root: None,
            height: Checked::new(0),
            len: Checked::new(0),
            policy,
            marker: PhantomData,
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.len.get()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn allocation_policy(&self) -> &P {
        &self.policy
    }
    pub fn clear(&mut self) {
        let root = self.root.take();
        let h = self.height.get();
        self.len = Checked::new(0);
        self.height = Checked::new(0);
        if let Some(root) = root {
            // SAFETY: detach unique complete tree before any user Drop.
            unsafe { node::destroy(root, h, &self.policy) };
        }
    }
    fn first_cursor(&self) -> Option<Cursor<K, V>> {
        let root = self.root?;
        // SAFETY: map owns the complete initialized tree.
        unsafe { Cursor::first(root, self.height.get()) }
    }
    fn last_cursor(&self) -> Option<Cursor<K, V>> {
        let root = self.root?;
        // SAFETY: map owns the complete initialized tree.
        unsafe { Cursor::last(root, self.height.get()) }
    }
    #[inline]
    pub fn first_key_value(&self) -> Option<(&K, &V)> {
        let c = self.first_cursor()?;
        // SAFETY: initialized cursor tied to the map's shared borrow.
        Some(unsafe {
            (
                &*node::key(c.node, c.index()),
                &*node::val(c.node, c.index()),
            )
        })
    }
    #[inline]
    pub fn last_key_value(&self) -> Option<(&K, &V)> {
        let c = self.last_cursor()?;
        // SAFETY: initialized cursor tied to the map's shared borrow.
        Some(unsafe {
            (
                &*node::key(c.node, c.index()),
                &*node::val(c.node, c.index()),
            )
        })
    }
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(self)
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut::new(self)
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
    pub fn pop_first(&mut self) -> Option<(K, V)> {
        let c = self.first_cursor()?;
        // SAFETY: unique map and valid initialized cursor.
        Some(unsafe { self.remove_at(c, &mut []) })
    }
    pub fn pop_last(&mut self) -> Option<(K, V)> {
        let c = self.last_cursor()?;
        // SAFETY: unique map and valid initialized cursor.
        Some(unsafe { self.remove_at(c, &mut []) })
    }
    /// A borrowed view of this map's initialized node slices.
    pub fn root_view(&self) -> Option<view::NodeView<'_, K, V>> {
        self.root.map(|node| view::NodeView {
            node,
            height: self.height.get(),
            marker: PhantomData,
        })
    }
    /// # Safety
    /// c is an initialized cursor in this map. Tracked cursors are live pairs
    /// other than c and are updated for every key relocation during deletion.
    unsafe fn remove_at(
        &mut self,
        mut c: Cursor<K, V>,
        tracked: &mut [Option<Cursor<K, V>>],
    ) -> (K, V) {
        // SAFETY: complete unique tree, all structural transitions preserve ownership.
        unsafe {
            if c.height() > 0 {
                let mut pred = Cursor::last(node::child(c.node, c.index()), sub(c.height(), 1));
                if let Some(p) = pred.take() {
                    ptr::swap(node::key(c.node, c.index()), node::key(p.node, p.index()));
                    ptr::swap(node::val(c.node, c.index()), node::val(p.node, p.index()));
                    for t in tracked.iter_mut().flatten() {
                        if *t == p {
                            *t = c;
                        }
                    }
                    c = p;
                }
            }
            let result = node::remove_leaf(c.node, c.index());
            for t in tracked.iter_mut().flatten() {
                if t.node == c.node && t.index() > c.index() {
                    t.set_index(sub(t.index(), 1));
                }
            }
            self.len = Checked::new(sub(self.len(), 1));
            let mut n = c.node;
            let mut h = 0;
            while node::len(n) < MIN {
                let Some((p, i)) = node::parent(n) else { break };
                let pn = node::len(p);
                if geometry::consider_left(i)
                    && geometry::borrow_required(node::len(node::child(p, sub(i, 1))), node::len(n))
                {
                    let sep = sub(i, 1);
                    let left = node::child(p, sep);
                    let last = sub(node::len(left), 1);
                    for t in tracked.iter_mut().flatten() {
                        if t.node == n {
                            t.set_index(add(t.index(), 1));
                        } else if t.node == p && t.index() == sep {
                            *t = Cursor::new(n, 0, h);
                        } else if t.node == left && t.index() == last {
                            *t = Cursor::new(p, sep, add(h, 1));
                        }
                    }
                    node::steal_left(p, sep, h);
                    break;
                }
                if geometry::consider_right(i, pn)
                    && geometry::borrow_required(node::len(node::child(p, add(i, 1))), node::len(n))
                {
                    let right = node::child(p, add(i, 1));
                    let l = node::len(n);
                    for t in tracked.iter_mut().flatten() {
                        if t.node == p && t.index() == i {
                            *t = Cursor::new(n, l, h);
                        } else if t.node == right {
                            if t.index() == 0 {
                                *t = Cursor::new(p, i, add(h, 1));
                            } else {
                                t.set_index(sub(t.index(), 1));
                            }
                        }
                    }
                    node::steal_right(p, i, h);
                    break;
                }
                let sep = if i > 0 { sub(i, 1) } else { 0 };
                let left = node::child(p, sep);
                let right = node::child(p, add(sep, 1));
                let l = node::len(left);
                for t in tracked.iter_mut().flatten() {
                    if t.node == right {
                        *t = Cursor::new(left, add(add(l, 1), t.index()), h);
                    } else if t.node == p {
                        if t.index() == sep {
                            *t = Cursor::new(left, l, h);
                        } else if t.index() > sep {
                            t.set_index(sub(t.index(), 1));
                        }
                    }
                }
                node::merge(p, sep, h, &self.policy);
                n = p;
                h = add(h, 1);
            }
            self.shrink_root();
            result
        }
    }
    /// # Safety
    /// Complete unique tree. Zero-key internal roots have exactly one child.
    unsafe fn shrink_root(&mut self) {
        // SAFETY: each iteration removes an empty root and transfers its sole child.
        unsafe {
            while self.height.get() > 0 {
                let Some(root) = self.root else { break };
                if node::len(root) > 0 {
                    break;
                }
                let child = node::child(root, 0);
                (*child.as_ptr()).parent = None;
                self.root = Some(child);
                node::free(root, self.height.get(), &self.policy);
                self.height = Checked::new(sub(self.height.get(), 1));
            }
        }
    }
}
impl<K, V, P: AllocationPolicy> Drop for BTreeMap<K, V, P> {
    fn drop(&mut self) {
        self.clear()
    }
}

enum Search<K, V> {
    Found(Cursor<K, V>),
    Vacant(Option<Cursor<K, V>>),
}
impl<K, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    fn search<Q: ?Sized + Ord>(&self, key: &Q) -> Search<K, V>
    where
        K: Borrow<Q>,
    {
        let Some(mut n) = self.root else {
            return Search::Vacant(None);
        };
        let mut h = self.height.get();
        loop {
            // SAFETY: tree invariant bounds this node's initialized pair/edge prefix.
            unsafe {
                let mut i = 0;
                let count = node::len(n);
                while i < count {
                    match key.cmp((&*node::key(n, i)).borrow()) {
                        Ordering::Equal => return Search::Found(Cursor::new(n, i, h)),
                        Ordering::Less => break,
                        Ordering::Greater => i = add(i, 1),
                    }
                }
                if h == 0 {
                    return Search::Vacant(Some(Cursor::new(n, i, 0)));
                }
                n = node::child(n, i);
                h = sub(h, 1);
            }
        }
    }
    pub fn get<Q: ?Sized + Ord>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        match self.search(key) {
            Search::Found(c) => {
                // SAFETY: found initialized value under the map borrow.
                Some(unsafe { &*node::val(c.node, c.index()) })
            }
            _ => None,
        }
    }
    pub fn get_key_value<Q: ?Sized + Ord>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
    {
        match self.search(key) {
            Search::Found(c) => {
                // SAFETY: both found slots initialized and shared-borrowed.
                Some(unsafe {
                    (
                        &*node::key(c.node, c.index()),
                        &*node::val(c.node, c.index()),
                    )
                })
            }
            _ => None,
        }
    }
    pub fn get_mut<Q: ?Sized + Ord>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        match self.search(key) {
            Search::Found(c) => {
                // SAFETY: unique map borrow and initialized value.
                Some(unsafe { &mut *node::val(c.node, c.index()) })
            }
            _ => None,
        }
    }
    pub fn contains_key<Q: ?Sized + Ord>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        matches!(self.search(key), Search::Found(_))
    }
    pub fn remove_entry<Q: ?Sized + Ord>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
    {
        match self.search(key) {
            Search::Found(c) => {
                // SAFETY: valid cursor and unique map borrow.
                Some(unsafe { self.remove_at(c, &mut []) })
            }
            _ => None,
        }
    }
    pub fn remove<Q: ?Sized + Ord>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        // Drop the discarded half while the returned half is still a local: if that
        // destructor unwinds, the returned half is dropped too instead of leaking.
        let (discarded, value) = self.remove_entry(key)?;
        drop(discarded);
        Some(value)
    }
}

/// All fallible work for insertion, owning empty allocations until committed.
struct InsertPlan<K, V, P: AllocationPolicy> {
    at: Cursor<K, V>,
    stop: Cursor<K, V>,
    spares: Spares<K, V, P>,
    root: Option<Link<K, V>>,
    new_len: Checked<usize>,
    new_height: Checked<usize>,
}
impl<K, V, P: AllocationPolicy> Drop for InsertPlan<K, V, P> {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            // SAFETY: unconsumed root reservation is empty and uniquely owned.
            unsafe { node::free(root, self.new_height.get(), &self.spares.policy) };
        }
    }
}
impl<K, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    /// # Safety
    /// at is this map's vacant leaf edge, len(at.node)<CAP, and new_len is the
    /// checked old map length plus one. No allocation or callback occurs here.
    #[inline]
    unsafe fn commit_fit(
        &mut self,
        at: Cursor<K, V>,
        new_len: Checked<usize>,
        key: K,
        value: V,
    ) -> Cursor<K, V> {
        // SAFETY: inherited leaf capacity and initialized-prefix partition.
        unsafe { node::insert(at.node, at.index(), key, value, None) };
        self.len = new_len;
        at
    }
    #[inline]
    fn try_insert_at(
        &mut self,
        at: Option<Cursor<K, V>>,
        key: K,
        value: V,
    ) -> Result<Cursor<K, V>, Rejected<(K, V)>> {
        if let Some(at) = at {
            // SAFETY: search supplied a live vacant leaf edge in this map.
            if unsafe { node::len(at.node) } < CAP {
                let new_len = match self.len + Checked::new(1) {
                    Ok(n) => n,
                    Err(_) => {
                        return Err(Rejected {
                            value: (key, value),
                            error: Error::Capacity,
                        });
                    }
                };
                // SAFETY: admission precedes publication and no user code ran
                // between the capacity/length checks and this in-place commit.
                return Ok(unsafe { self.commit_fit(at, new_len, key, value) });
            }
        }
        let plan = match self.plan_insert(at) {
            Ok(plan) => plan,
            Err(error) => {
                return Err(Rejected {
                    value: (key, value),
                    error,
                });
            }
        };
        Ok(self.commit_insert(plan, key, value))
    }
    fn plan_insert(&self, at: Option<Cursor<K, V>>) -> Result<InsertPlan<K, V, P>, Error> {
        let new_len = (self.len + Checked::new(1)).map_err(|_| Error::Capacity)?;
        let spares = Spares::new(self.policy.try_clone()?);
        let c = match at {
            Some(c) => c,
            None => {
                let root = node::allocate(0, &self.policy)?;
                let c = Cursor::new(root, 0, 0);
                return Ok(InsertPlan {
                    at: c,
                    stop: c,
                    spares,
                    root: Some(root),
                    new_len,
                    new_height: self.height,
                });
            }
        };
        let mut plan = InsertPlan {
            at: c,
            stop: c,
            spares,
            root: None,
            new_len,
            new_height: self.height,
        };
        // SAFETY: vacant cursor is at a leaf edge in this live tree.
        unsafe {
            if node::len(c.node) < CAP {
                return Ok(plan);
            }
            let mut top = c.node;
            let mut h = 0;
            loop {
                match node::parent(top) {
                    Some((p, _)) if node::len(p) == CAP => {
                        top = p;
                        h = add(h, 1);
                    }
                    _ => break,
                }
            }
            if node::parent(top).is_none() {
                // This closed height bound is unreachable for a representable
                // degree-six tree, and keeps every cursor's narrowing explicit.
                if self.height.get() >= sub(usize::BITS as usize, 1) {
                    return Err(Error::Capacity);
                }
                plan.new_height = (self.height + Checked::new(1)).map_err(|_| Error::Capacity)?;
                plan.root = Some(node::allocate(plan.new_height.get(), &self.policy)?);
                if let Some(root) = plan.root {
                    plan.stop = Cursor::new(root, 0, plan.new_height.get());
                }
            } else if let Some((parent, index)) = node::parent(top) {
                plan.stop = Cursor::new(parent, index, add(h, 1));
            }
            // Reserve top-down so leaf rights are consumed first at commit.
            loop {
                plan.spares.reserve(h)?;
                if h == 0 {
                    break;
                }
                h = sub(h, 1);
            }
        }
        Ok(plan)
    }
    fn commit_insert(&mut self, mut plan: InsertPlan<K, V, P>, mut k: K, mut v: V) -> Cursor<K, V> {
        let at = plan.at;
        if self.root.is_none() {
            // SAFETY: plan owns its initialized empty root and exactly one pair.
            unsafe { node::insert(at.node, 0, k, v, None) };
            self.root = plan.root.take();
            self.len = plan.new_len;
            return at;
        }
        let mut cur = at;
        let mut right_child = None;
        let mut inserted = at;
        // SAFETY: plan owns every needed empty node and checked final state. All
        // operations below transfer initialized ownership with no user callbacks.
        unsafe {
            while let Some(right) = plan.spares.take() {
                let mid = node::splitpoint(cur.index());
                let (upk, upv) = node::split(cur.node, mid, right, cur.height());
                let (target, index) = if cur.index() <= mid {
                    (cur.node, cur.index())
                } else {
                    (right, sub(cur.index(), add(mid, 1)))
                };
                node::insert(target, index, k, v, right_child);
                if cur.height() == 0 {
                    inserted = Cursor::new(target, index, 0);
                }
                k = upk;
                v = upv;
                right_child = Some(right);
                match node::parent(cur.node) {
                    Some((parent, index)) => cur = Cursor::new(parent, index, add(cur.height(), 1)),
                    None => {
                        node::set_child(plan.stop.node, 0, cur.node);
                        cur = plan.stop;
                    }
                }
            }
            node::insert(cur.node, cur.index(), k, v, right_child);
            if let Some(root) = plan.root.take() {
                self.root = Some(root);
                self.height = plan.new_height;
            }
        }
        self.len = plan.new_len;
        inserted
    }
}
impl<K: Ord, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    /// Set-only replacement: one search, unchanged initialized slot and shape.
    pub(super) fn try_replace_key(
        &mut self,
        key: K,
        value: V,
    ) -> Result<Option<K>, Rejected<(K, V)>> {
        match self.search(&key) {
            Search::Found(c) => {
                // SAFETY: search returned an initialized key slot under this
                // exclusive map borrow. Replacing an equal key changes neither
                // slot initialization nor any link/count. No user code runs
                // between reading the old object and publishing its replacement.
                let old = unsafe { node::replace_key(c.node, c.index(), key) };
                drop(value);
                Ok(Some(old))
            }
            Search::Vacant(at) => {
                self.try_insert_at(at, key, value)?;
                Ok(None)
            }
        }
    }
    pub fn try_insert(&mut self, key: K, value: V) -> Result<Option<V>, Rejected<(K, V)>> {
        match self.search(&key) {
            Search::Found(c) => {
                // SAFETY: unique initialized value; stored key is retained.
                let old = unsafe {
                    let slot = node::val(c.node, c.index());
                    let old = slot.read();
                    slot.write(value);
                    old
                };
                drop(key);
                Ok(Some(old))
            }
            Search::Vacant(at) => {
                self.try_insert_at(at, key, value)?;
                Ok(None)
            }
        }
    }
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, P> {
        match self.search(&key) {
            Search::Found(at) => {
                drop(key);
                Entry::Occupied(OccupiedEntry { map: self, at })
            }
            Search::Vacant(at) => Entry::Vacant(VacantEntry { map: self, key, at }),
        }
    }
    pub fn first_entry(&mut self) -> Option<OccupiedEntry<'_, K, V, P>> {
        let at = self.first_cursor()?;
        Some(OccupiedEntry { map: self, at })
    }
    pub fn last_entry(&mut self) -> Option<OccupiedEntry<'_, K, V, P>> {
        let at = self.last_cursor()?;
        Some(OccupiedEntry { map: self, at })
    }
}

impl<K: core::fmt::Debug, V: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug
    for BTreeMap<K, V, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<K: PartialEq, V: PartialEq, P: AllocationPolicy, Q: AllocationPolicy>
    PartialEq<BTreeMap<K, V, Q>> for BTreeMap<K, V, P>
{
    fn eq(&self, other: &BTreeMap<K, V, Q>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<K: Eq, V: Eq, P: AllocationPolicy> Eq for BTreeMap<K, V, P> {}
impl<K: Ord, V: Ord, P: AllocationPolicy> Ord for BTreeMap<K, V, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}
impl<K: PartialOrd, V: PartialOrd, P: AllocationPolicy> PartialOrd for BTreeMap<K, V, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}
impl<K: core::hash::Hash, V: core::hash::Hash, P: AllocationPolicy> core::hash::Hash
    for BTreeMap<K, V, P>
{
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for pair in self.iter() {
            pair.hash(state);
        }
    }
}
