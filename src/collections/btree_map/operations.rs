use super::*;
use crate::{TryCloneIn, TryIntoIterator, TryIterator, budget::Budget, init::InitSlot};
use core::ops::{Bound, RangeBounds};
use iter::{BorrowState, State};

/// Compare physical in-order positions, independently of a user's Ord behavior.
fn ordered<K, V>(a: Cursor<K, V>, b: Cursor<K, V>) -> bool {
    let (mut an, mut bn) = (a.node, b.node);
    let (mut ah, mut bh) = (a.height(), b.height());
    let (mut ai, mut bi) = (
        add(a.index(), add(a.index(), 1)),
        add(b.index(), add(b.index(), 1)),
    );
    // SAFETY: cursors belong to the same live tree, with initialized parent links.
    unsafe {
        while ah < bh {
            if let Some((p, i)) = node::parent(an) {
                an = p;
                ai = add(i, i);
            }
            ah = add(ah, 1);
        }
        while bh < ah {
            if let Some((p, i)) = node::parent(bn) {
                bn = p;
                bi = add(i, i);
            }
            bh = add(bh, 1);
        }
        while an != bn {
            match (node::parent(an), node::parent(bn)) {
                (Some((ap, ax)), Some((bp, bx))) => {
                    an = ap;
                    bn = bp;
                    ai = add(ax, ax);
                    bi = add(bx, bx);
                }
                _ => return false,
            }
        }
    }
    ai <= bi
}
impl<K, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    /// # Safety
    /// Only the selected outer border may be underfull; all other nonroot
    /// nodes meet MIN. This repairs that border without allocation or callbacks.
    unsafe fn fix_border(&mut self, left: bool) {
        // SAFETY: top-down repair provisions six keys before descending, so a
        // child merge cannot underfill a previously repaired nonroot parent.
        unsafe {
            self.shrink_root();
            let Some(mut n) = self.root else { return };
            let mut h = self.height.get();
            while h > 0 {
                let count = node::len(n);
                if count == 0 {
                    break;
                }
                let sep = if left { 0 } else { sub(count, 1) };
                let ch = sub(h, 1);
                let a = node::child(n, sep);
                let b = node::child(n, add(sep, 1));
                if add(add(node::len(a), 1), node::len(b)) <= CAP {
                    n = node::merge(n, sep, ch, &self.policy);
                } else {
                    let parent = n;
                    n = if left { a } else { b };
                    while node::len(n) < 6 {
                        if left {
                            node::steal_right(parent, sep, ch);
                        } else {
                            node::steal_left(parent, sep, ch);
                        }
                    }
                }
                h = ch;
            }
            self.shrink_root();
        }
    }
    /// # Safety
    /// Complete subtree; its count is bounded by the previously checked map len.
    unsafe fn count_subtree(n: Link<K, V>, height: usize) -> usize {
        // SAFETY: only initialized edges are visited, each subtree exactly once.
        unsafe {
            let count = node::len(n);
            let mut total = count;
            if height > 0 {
                for i in 0..=count {
                    total = add(
                        total,
                        Self::count_subtree(node::child(n, i), sub(height, 1)),
                    );
                }
            }
            total
        }
    }
    /// Split at the first key >= key. Reserve the entire new border before
    /// moving anything. On refusal the original map and its shape are preserved.
    pub fn try_split_off<Q: ?Sized + Ord>(&mut self, key: &Q) -> Result<Self, Error>
    where
        K: Borrow<Q>,
    {
        let Some(root) = self.root else {
            return Ok(Self::new_in(self.policy.try_clone()?));
        };
        // A valid degree-six tree with usize-bounded length has fewer than
        // usize::BITS levels. The checked array access also rejects a bad path.
        let mut path = [0u16; usize::BITS as usize];
        let mut level = 0;
        let mut n = root;
        let mut h = self.height.get();
        // SAFETY: all comparisons precede reservation and mutation, and every
        // descent uses a key-bounded edge in the currently valid tree.
        unsafe {
            loop {
                let mut i = 0;
                while i < node::len(n) && (&*node::key(n, i)).borrow().cmp(key) == Ordering::Less {
                    i = add(i, 1);
                }
                *path.get_mut(level).ok_or(Error::Capacity)? = i as u16;
                if h == 0 {
                    break;
                }
                n = node::child(n, i);
                h = sub(h, 1);
                level = add(level, 1);
            }
        }
        let mut right = Self::new_in(self.policy.try_clone()?);
        right.root = Some(node::allocate(0, &self.policy)?);
        while right.height < self.height {
            let parent = node::allocate(add(right.height.get(), 1), &self.policy)?;
            if let Some(child) = right.root {
                // SAFETY: right owns a complete pillar, new parent owns edge zero.
                unsafe { node::set_child(parent, 0, child) };
            }
            right.root = Some(parent);
            right.height = Checked::new(add(right.height.get(), 1));
        }
        let mut a = self.root;
        let mut b = right.root;
        let mut height = self.height.get();
        // Read path before moving: every required entry was recorded above. The
        // iterator visits exactly height+1 initialized entries; no indexing panic.
        // SAFETY: both complete trees are uniquely owned, with matching heights.
        unsafe {
            for index in path.iter().take(add(height, 1)) {
                if let (Some(left), Some(dest)) = (a, b) {
                    let i = usize::from(*index);
                    node::move_suffix(left, i, dest, height);
                    if height == 0 {
                        break;
                    }
                    a = Some(node::child(left, i));
                    b = Some(node::child(dest, 0));
                    height = sub(height, 1);
                }
            }
            self.fix_border(false);
            right.fix_border(true);
            let old = self.len();
            if self.height < right.height {
                let count = self
                    .root
                    .map_or(0, |r| Self::count_subtree(r, self.height.get()));
                self.len = Checked::new(count);
                right.len = Checked::new(sub(old, count));
            } else {
                let count = right
                    .root
                    .map_or(0, |r| Self::count_subtree(r, right.height.get()));
                right.len = Checked::new(count);
                self.len = Checked::new(sub(old, count));
            }
        }
        Ok(right)
    }
    fn lower<Q: ?Sized + Ord>(&self, key: &Q, inclusive: bool) -> Option<Cursor<K, V>>
    where
        K: Borrow<Q>,
    {
        // SAFETY: search result is either an initialized pair or a leaf edge.
        unsafe {
            match self.search(key) {
                Search::Found(c) => {
                    if inclusive {
                        Some(c)
                    } else {
                        c.next()
                    }
                }
                Search::Vacant(Some(c)) => {
                    if c.index() < node::len(c.node) {
                        return Some(c);
                    }
                    let mut n = c.node;
                    let mut h = 0;
                    while let Some((p, i)) = node::parent(n) {
                        h = add(h, 1);
                        if i < node::len(p) {
                            return Some(Cursor::new(p, i, h));
                        }
                        n = p;
                    }
                    None
                }
                _ => None,
            }
        }
    }
    fn upper<Q: ?Sized + Ord>(&self, key: &Q, inclusive: bool) -> Option<Cursor<K, V>>
    where
        K: Borrow<Q>,
    {
        // SAFETY: all predecessor traversal stays inside the initialized tree.
        unsafe {
            match self.search(key) {
                Search::Found(c) => {
                    if inclusive {
                        Some(c)
                    } else {
                        c.prev()
                    }
                }
                Search::Vacant(Some(c)) => {
                    if c.index() > 0 {
                        return Some(Cursor::new(c.node, sub(c.index(), 1), 0));
                    }
                    let mut n = c.node;
                    let mut h = 0;
                    while let Some((p, i)) = node::parent(n) {
                        h = add(h, 1);
                        if i > 0 {
                            return Some(Cursor::new(p, sub(i, 1), h));
                        }
                        n = p;
                    }
                    None
                }
                _ => None,
            }
        }
    }
    fn range_state<Q: ?Sized + Ord, R: RangeBounds<Q>>(
        &self,
        range: R,
    ) -> Result<State<K, V>, Error>
    where
        K: Borrow<Q>,
    {
        let (start, end) = (range.start_bound(), range.end_bound());
        if let (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b)) =
            (start, end)
        {
            let order = a.cmp(b);
            if order == Ordering::Greater
                || (order == Ordering::Equal
                    && matches!((start, end), (Bound::Excluded(_), Bound::Excluded(_))))
            {
                return Err(Error::Bounds);
            }
        }
        let front = match start {
            Bound::Unbounded => self.first_cursor(),
            Bound::Included(k) => self.lower(k, true),
            Bound::Excluded(k) => self.lower(k, false),
        };
        let back = match end {
            Bound::Unbounded => self.last_cursor(),
            Bound::Included(k) => self.upper(k, true),
            Bound::Excluded(k) => self.upper(k, false),
        };
        match (front, back) {
            (Some(a), Some(b))
                if matches!(start, Bound::Unbounded)
                    || matches!(end, Bound::Unbounded)
                    || ordered(a, b) =>
            {
                Ok(State {
                    front,
                    back,
                    remaining: self.len,
                })
            }
            _ => Ok(State::empty()),
        }
    }
    pub fn try_range<Q: ?Sized + Ord, R: RangeBounds<Q>>(
        &self,
        range: R,
    ) -> Result<Range<'_, K, V>, Error>
    where
        K: Borrow<Q>,
    {
        Ok(Range(Iter {
            // SAFETY: the map borrow keeps the full checked interval live.
            state: unsafe { BorrowState::interval(self.range_state(range)?) },
            marker: PhantomData,
        }))
    }
    pub fn try_range_mut<Q: ?Sized + Ord, R: RangeBounds<Q>>(
        &mut self,
        range: R,
    ) -> Result<RangeMut<'_, K, V>, Error>
    where
        K: Borrow<Q>,
    {
        Ok(RangeMut(IterMut {
            // SAFETY: the map borrow keeps the full checked interval live.
            state: unsafe { BorrowState::interval(self.range_state(range)?) },
            marker: PhantomData,
        }))
    }
    /// Error consumes that predicate visit, retains the current pair and its
    /// value edits, and leaves the iterator ready for the next candidate.
    pub fn try_extract_if<R: RangeBounds<K>, E, F: FnMut(&K, &mut V) -> Result<bool, E>>(
        &mut self,
        range: R,
        predicate: F,
    ) -> Result<ExtractIf<'_, K, V, P, F, E>, Error>
    where
        K: Ord,
    {
        let state = self.range_state::<K, R>(range)?;
        Ok(ExtractIf {
            map: self,
            state,
            predicate,
            marker: PhantomData,
        })
    }
    /// Prefix removals and value edits remain on callback failure.
    pub fn try_retain<E>(
        &mut self,
        mut predicate: impl FnMut(&K, &mut V) -> Result<bool, E>,
    ) -> Result<(), E> {
        let state = State {
            front: self.first_cursor(),
            back: self.last_cursor(),
            remaining: self.len,
        };
        let mut iter = ExtractIf {
            map: self,
            state,
            predicate: |k: &K, v: &mut V| predicate(k, v).map(|keep| !keep),
            marker: PhantomData,
        };
        while let Some(pair) = iter.try_next()? {
            drop(pair);
        }
        Ok(())
    }
}
pub struct ExtractIf<'a, K, V, P: AllocationPolicy, F, E> {
    map: &'a mut BTreeMap<K, V, P>,
    state: State<K, V>,
    predicate: F,
    marker: PhantomData<fn() -> E>,
}
// SAFETY: exclusive map/cursor ownership and the predicate move together.
unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send, F: Send, E> Send
    for ExtractIf<'_, K, V, P, F, E>
{
}
// SAFETY: shared access cannot invoke the predicate or advance the cursor.
unsafe impl<K: Sync, V: Sync, P: AllocationPolicy + Sync, F: Sync, E> Sync
    for ExtractIf<'_, K, V, P, F, E>
{
}
impl<K: core::fmt::Debug, V: core::fmt::Debug, P: AllocationPolicy, F, E> core::fmt::Debug
    for ExtractIf<'_, K, V, P, F, E>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let front = self.state.front;
        let state = State {
            front,
            back: front,
            remaining: Checked::new(usize::from(
                front.is_some() && self.state.remaining.get() > 0,
            )),
        };
        let mut peek = Iter {
            state: BorrowState::remaining(state),
            marker: PhantomData,
        };
        f.debug_struct("ExtractIf")
            .field("peek", &peek.next())
            .finish_non_exhaustive()
    }
}
impl<K, V, P: AllocationPolicy, E, F: FnMut(&K, &mut V) -> Result<bool, E>> TryIterator
    for ExtractIf<'_, K, V, P, F, E>
{
    type Item = (K, V);
    type Error = E;
    fn try_next(&mut self) -> Result<Option<(K, V)>, E> {
        // SAFETY: exclusive map borrow, candidate removed from the iteration
        // frontier before the callback. All cursor relocations are tracked.
        unsafe {
            while let Some(c) = self.state.take(false) {
                if (self.predicate)(
                    &*node::key(c.node, c.index()),
                    &mut *node::val(c.node, c.index()),
                )? {
                    let mut tracked = [self.state.front, self.state.back];
                    let pair = self.map.remove_at(c, &mut tracked);
                    [self.state.front, self.state.back] = tracked;
                    return Ok(Some(pair));
                }
            }
        }
        Ok(None)
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), E> {
        Ok((0, Some(self.state.remaining.get())))
    }
}
impl<K, V, P: AllocationPolicy, E, F: FnMut(&K, &mut V) -> Result<bool, E>> crate::TryFusedIterator
    for ExtractIf<'_, K, V, P, F, E>
{
}

/// # Safety
/// Source is a complete initialized subtree of this height. Closures initialize
/// only their supplied tracked slots; destination ownership stays in the guard.
unsafe fn clone_node<K, V, A, B, P: AllocationPolicy>(
    source: Link<K, V>,
    height: usize,
    policy: &P,
    key: &mut impl FnMut(&K, &mut InitSlot<'_, A>) -> Result<(), Error>,
    value: &mut impl FnMut(&V, &mut InitSlot<'_, B>) -> Result<(), Error>,
) -> Result<Link<A, B>, Error> {
    let node = node::allocate(height, policy)?;
    let mut guard = node::NodeGuard {
        node,
        height,
        pairs: 0,
        children: 0,
        policy,
    };
    // SAFETY: source stays shared-borrowed; fresh destination slots are disjoint.
    unsafe {
        let n = node::len(source);
        while guard.pairs < n {
            let i = guard.pairs;
            let mut k = InitSlot::new(core::ptr::NonNull::new_unchecked(node::key(node, i)));
            let mut v = InitSlot::new(core::ptr::NonNull::new_unchecked(node::val(node, i)));
            key(&*node::key(source, i), &mut k)?;
            value(&*node::val(source, i), &mut v)?;
            if k.get().is_none() || v.get().is_none() {
                return Err(Error::Uninitialized);
            }
            k.finish()?;
            v.finish()?;
            guard.pairs = add(i, 1);
            node::set_len(node, guard.pairs);
        }
        if height > 0 {
            while guard.children <= n {
                let i = guard.children;
                let child = clone_node(node::child(source, i), sub(height, 1), policy, key, value)?;
                node::set_child(node, i, child);
                guard.children = add(i, 1);
            }
        }
    }
    mem::forget(guard);
    Ok(node)
}
impl<K: TryClone, V: TryClone, P: AllocationPolicy> TryClone for BTreeMap<K, V, P> {
    fn try_clone(&self) -> Result<Self, Error> {
        let mut result = Self::new_in(self.policy.try_clone()?);
        if let Some(root) = self.root {
            // SAFETY: complete shared source and tracked clone initialization.
            result.root = Some(unsafe {
                clone_node(
                    root,
                    self.height.get(),
                    &result.policy,
                    &mut K::try_clone_into,
                    &mut V::try_clone_into,
                )
            }?);
        }
        result.height = self.height;
        result.len = self.len;
        Ok(result)
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        let mut replacement = Self::new_in(self.policy.try_clone()?);
        if let Some(root) = source.root {
            // SAFETY: same tracked construction, destination's storage policy.
            replacement.root = Some(unsafe {
                clone_node(
                    root,
                    source.height.get(),
                    &replacement.policy,
                    &mut K::try_clone_into,
                    &mut V::try_clone_into,
                )
            }?);
        }
        replacement.height = source.height;
        replacement.len = source.len;
        mem::swap(self, &mut replacement);
        Ok(())
    }
}
impl<'a, K: TryCloneIn<'a>, V: TryCloneIn<'a>, P: AllocationPolicy> TryCloneIn<'a>
    for BTreeMap<K, V, P>
{
    type Output = BTreeMap<K::Output, V::Output, Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        let mut result = BTreeMap::new_in(budget);
        if let Some(root) = self.root {
            // SAFETY: original types are read only; output slots track their own
            // initialization and every nested clone receives the target budget.
            result.root = Some(unsafe {
                clone_node(
                    root,
                    self.height.get(),
                    &budget,
                    &mut |k, slot| k.try_clone_into_in(budget, slot),
                    &mut |v, slot| v.try_clone_into_in(budget, slot),
                )
            }?);
        }
        result.height = self.height;
        result.len = self.len;
        Ok(result)
    }
}
impl<K: Ord, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    /// Prefix commits remain on source/insertion failure. The rejected pair is
    /// returned separately; an iterator error contains no invented rejected pair.
    pub fn try_extend<I: TryIntoIterator<Item = (K, V)>>(
        &mut self,
        source: I,
    ) -> Result<(), ExtendError<K, V, I::Error>> {
        let mut iter = source.try_into_iter().map_err(ExtendError::Source)?;
        while let Some((k, v)) = iter.try_next().map_err(ExtendError::Source)? {
            self.try_insert(k, v).map_err(ExtendError::Insert)?;
        }
        Ok(())
    }
    pub fn try_from_iter_in<I: TryIntoIterator<Item = (K, V)>>(
        source: I,
        policy: P,
    ) -> Result<Self, ExtendError<K, V, I::Error>> {
        let mut result = Self::new_in(policy);
        result.try_extend(source)?;
        Ok(result)
    }
}
impl<K: Ord, V> BTreeMap<K, V> {
    pub fn try_from_iter<I: TryIntoIterator<Item = (K, V)>>(
        source: I,
    ) -> Result<Self, ExtendError<K, V, I::Error>> {
        Self::try_from_iter_in(source, Unmetered)
    }
    pub fn try_from_array<const N: usize>(
        source: [(K, V); N],
    ) -> Result<Self, ExtendError<K, V, core::convert::Infallible>> {
        Self::try_from_iter(source)
    }
}
impl<K, V> crate::TryDefault for BTreeMap<K, V> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
#[derive(Debug)]
pub enum ExtendError<K, V, E> {
    Source(E),
    Insert(Rejected<(K, V)>),
}
