use super::{
    BTreeMap, add,
    node::{self, Link},
    sub,
};
use crate::{
    Checked, TryDoubleEndedIterator, TryExactSizeIterator, TryFusedIterator, TryIterator,
    budget::{AllocationPolicy, Unmetered},
};
use core::{convert::Infallible, iter::FusedIterator, marker::PhantomData};

pub(super) struct Cursor<K, V> {
    pub(super) node: Link<K, V>,
    // One scalar avoids partial-width stores followed by a merged wider load.
    // Four index bits cover all eleven leaf slots and twelve child edges.
    position: u16,
}
impl<K, V> Copy for Cursor<K, V> {}
impl<K, V> Clone for Cursor<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K, V> PartialEq for Cursor<K, V> {
    fn eq(&self, b: &Self) -> bool {
        self.node == b.node && self.position == b.position
    }
}
impl<K, V> Eq for Cursor<K, V> {}
impl<K, V> Cursor<K, V> {
    pub(super) fn new(node: Link<K, V>, index: usize, height: usize) -> Self {
        Self {
            node,
            position: super::geometry::pack(index, height),
        }
    }
    pub(super) fn index(self) -> usize {
        super::geometry::index(self.position)
    }
    pub(super) fn set_index(&mut self, index: usize) {
        self.position = super::geometry::replace_index(self.position, index);
    }
    pub(super) fn height(self) -> usize {
        super::geometry::height(self.position)
    }
    /// # Safety
    /// Complete live subtree with initialized children and correct height.
    #[inline]
    pub(super) unsafe fn first(mut node: Link<K, V>, mut height: usize) -> Option<Self> {
        // SAFETY: descent follows initialized edges until the leaf level.
        unsafe {
            while height > 0 {
                node = node::child(node, 0);
                height = sub(height, 1);
            }
            if node::len(node) == 0 {
                None
            } else {
                Some(Self::new(node, 0, 0))
            }
        }
    }
    /// # Safety
    /// Complete live subtree with initialized children and correct height.
    #[inline]
    pub(super) unsafe fn last(mut node: Link<K, V>, mut height: usize) -> Option<Self> {
        // SAFETY: descent follows initialized last edges to a live leaf.
        unsafe {
            while height > 0 {
                node = node::child(node, node::len(node));
                height = sub(height, 1);
            }
            let n = node::len(node);
            if n == 0 {
                None
            } else {
                Some(Self::new(node, sub(n, 1), 0))
            }
        }
    }
    /// # Safety
    /// Initialized cursor, its unconsumed successor path and parents remain live.
    pub(super) unsafe fn next(self) -> Option<Self> {
        // SAFETY: each edge/slot used is in the initialized node prefix.
        unsafe {
            if self.height() > 0 {
                return Self::first(
                    node::child(self.node, add(self.index(), 1)),
                    sub(self.height(), 1),
                );
            }
            if add(self.index(), 1) < node::len(self.node) {
                return Some(Self::new(self.node, add(self.index(), 1), 0));
            }
            let mut n = self.node;
            let mut h = 0;
            while let Some((p, i)) = node::parent(n) {
                h = add(h, 1);
                if i < node::len(p) {
                    return Some(Self::new(p, i, h));
                }
                n = p;
            }
            None
        }
    }
    /// # Safety
    /// Initialized cursor, its unconsumed predecessor path and parents remain live.
    pub(super) unsafe fn prev(self) -> Option<Self> {
        // SAFETY: symmetric in-order traversal through initialized child edges.
        unsafe {
            if self.height() > 0 {
                return Self::last(node::child(self.node, self.index()), sub(self.height(), 1));
            }
            if self.index() > 0 {
                return Some(Self::new(self.node, sub(self.index(), 1), 0));
            }
            let mut n = self.node;
            let mut h = 0;
            while let Some((p, i)) = node::parent(n) {
                h = add(h, 1);
                if i > 0 {
                    return Some(Self::new(p, sub(i, 1), h));
                }
                n = p;
            }
            None
        }
    }
}

pub(super) struct State<K, V> {
    pub(super) front: Option<Cursor<K, V>>,
    pub(super) back: Option<Cursor<K, V>>,
    pub(super) remaining: Checked<usize>,
}
impl<K, V> Clone for State<K, V> {
    fn clone(&self) -> Self {
        Self {
            front: self.front,
            back: self.back,
            remaining: self.remaining,
        }
    }
}
impl<K, V> State<K, V> {
    pub(super) fn new<P: AllocationPolicy>(map: &BTreeMap<K, V, P>) -> Self {
        Self {
            front: map.first_cursor(),
            back: map.last_cursor(),
            remaining: Checked::new(map.len()),
        }
    }
    pub(super) fn empty() -> Self {
        Self {
            front: None,
            back: None,
            remaining: Checked::new(0),
        }
    }
    /// # Safety
    /// State bounds one live initialized interval. next paths are live until end.
    pub(super) unsafe fn take(&mut self, back: bool) -> Option<Cursor<K, V>> {
        if self.remaining.get() == 0 {
            return None;
        }
        let c = if back { self.back? } else { self.front? };
        if self.front == self.back {
            self.remaining = Checked::new(0);
            self.front = None;
            self.back = None;
        } else {
            self.remaining = Checked::new(sub(self.remaining.get(), 1));
            // SAFETY: non-final cursor has an unconsumed neighbor in this interval.
            unsafe {
                if back {
                    self.back = c.prev();
                } else {
                    self.front = c.next();
                }
            }
        }
        Some(c)
    }
}

impl<K, V> State<K, V> {
    /// # Safety
    /// c is the selected initialized frontier and remaining>0. An owning caller
    /// moves the pair before retiring exhausted allocations exactly once. A
    /// borrowed planning caller only observes retirement, keeps all source
    /// storage unchanged and holds its shared source borrow throughout.
    pub(super) unsafe fn advance_owned(
        &mut self,
        c: Cursor<K, V>,
        back: bool,
        mut retire: impl FnMut(Link<K, V>, usize),
    ) {
        // SAFETY: only unconsumed neighbor paths and initialized parent headers
        // are followed. Read the parent before handing storage to retire.
        unsafe {
            self.remaining = Checked::new(sub(self.remaining.get(), 1));
            if self.remaining.get() == 0 {
                let mut n = c.node;
                let mut h = c.height();
                loop {
                    let parent = node::parent(n);
                    retire(n, h);
                    match parent {
                        Some((p, _)) => {
                            n = p;
                            h = add(h, 1);
                        }
                        None => break,
                    }
                }
                self.front = None;
                self.back = None;
                return;
            }
            let next = if c.height() > 0 {
                if back {
                    Cursor::last(node::child(c.node, c.index()), sub(c.height(), 1))
                } else {
                    Cursor::first(node::child(c.node, add(c.index(), 1)), sub(c.height(), 1))
                }
            } else if back && c.index() > 0 {
                Some(Cursor::new(c.node, sub(c.index(), 1), 0))
            } else if !back && add(c.index(), 1) < node::len(c.node) {
                Some(Cursor::new(c.node, add(c.index(), 1), 0))
            } else {
                let mut n = c.node;
                let mut h = 0;
                loop {
                    let parent = node::parent(n);
                    retire(n, h);
                    let Some((p, i)) = parent else { break None };
                    h = add(h, 1);
                    if back && i > 0 {
                        break Some(Cursor::new(p, sub(i, 1), h));
                    }
                    if !back && i < node::len(p) {
                        break Some(Cursor::new(p, i, h));
                    }
                    n = p;
                }
            };
            if back {
                self.back = next;
            } else {
                self.front = next;
            }
        }
    }
}

/// Borrowed traversal stores leaf *edges*, so advancing inside a leaf needs
/// neither a height change nor comparison of two key/value frontiers. A height
/// above zero denotes one pending internal pair from a partially owned iterator;
/// its already consumed child must never be followed.
// Borrowed iteration advances native-width indices without decoding a packed
// owning cursor on every item. IntoIter retains the compact Cursor protocol.
struct BorrowEdge<K, V> {
    node: Link<K, V>,
    index: usize,
    height: usize,
}
impl<K, V> Copy for BorrowEdge<K, V> {}
impl<K, V> Clone for BorrowEdge<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K, V> PartialEq for BorrowEdge<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node && self.index == other.index && self.height == other.height
    }
}
impl<K, V> Eq for BorrowEdge<K, V> {}
impl<K, V> BorrowEdge<K, V> {
    fn new(node: Link<K, V>, index: usize, height: usize) -> Self {
        Self {
            node,
            index,
            height,
        }
    }
    fn from_cursor(cursor: Cursor<K, V>) -> Self {
        Self::new(cursor.node, cursor.index(), cursor.height())
    }
    fn index(self) -> usize {
        self.index
    }
    fn height(self) -> usize {
        self.height
    }
    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}

#[cfg(kani)]
mod borrowed_edge_proofs {
    use super::*;
    // Establish the constructor invariant and its inductive step on an actual
    // allocation. Arbitrary lo/hi and either direction cover every reachable
    // leaf interval without unrolling exponentially many complete histories.
    #[kani::proof]
    #[kani::unwind(3)]
    fn borrowed_leaf_interval_preserves_each_slot_and_ends() {
        let count: usize = kani::any();
        let lo: usize = kani::any();
        let hi: usize = kani::any();
        let back: bool = kani::any();
        kani::assume(lo <= hi && hi <= count && count <= super::super::CAP);
        if let Ok(root) = node::allocate::<u8, u8, _>(0, &Unmetered) {
            let data = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            // SAFETY: distinct arrays initialize precisely the allocated prefix.
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr(), node::key(root, 0), count);
                core::ptr::copy_nonoverlapping(data.as_ptr(), node::val(root, 0), count);
                node::set_len(root, count);
            }
            let mut map = core::mem::ManuallyDrop::new(BTreeMap::new());
            map.root = Some(root);
            map.len = Checked::new(count);
            let mut iter = map.iter();
            assert!(iter.state.front == BorrowEdge::new(root, 0, 0));
            assert!(iter.state.back == BorrowEdge::new(root, count, 0));
            assert_eq!(iter.len(), count);
            iter.state.front.set_index(lo);
            iter.state.back.set_index(hi);
            iter.state.remaining = Checked::new(hi - lo);
            let actual = if back { iter.next_back() } else { iter.next() };
            if lo == hi {
                assert_eq!(actual, None);
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next_back(), None);
            } else {
                let expected = if back { hi - 1 } else { lo } as u8;
                assert_eq!(actual, Some((&expected, &expected)));
                assert_eq!(iter.state.front.index(), lo + usize::from(!back));
                assert_eq!(iter.state.back.index(), hi - usize::from(back));
                assert_eq!(iter.len(), hi - lo - 1);
            }
            assert_eq!(iter.state.front.height(), 0);
            assert_eq!(iter.state.back.height(), 0);
            // SAFETY: no references survive this point; the ManuallyDrop owner
            // is intentionally not traversed. Map destruction has separate tests.
            unsafe { node::free(root, 0, &Unmetered) };
        }
    }
    #[kani::proof]
    fn native_words_and_packed_conversion_preserve_indices() {
        let index: usize = kani::any();
        let height: usize = kani::any();
        let replacement: usize = kani::any();
        let node = Link::<u8, u8>::dangling();
        let mut edge = BorrowEdge::new(node, index, height);
        assert_eq!(edge.node, node);
        assert_eq!(edge.index(), index);
        assert_eq!(edge.height(), height);
        edge.set_index(replacement);
        assert_eq!(edge.index(), replacement);
        assert_eq!(edge.height(), height);
        if index <= super::super::CAP && height < usize::BITS as usize {
            let decoded = BorrowEdge::from_cursor(Cursor::new(node, index, height));
            assert_eq!(decoded.node, node);
            assert_eq!(decoded.index(), index);
            assert_eq!(decoded.height(), height);
        }
    }

    #[kani::proof]
    fn packed_cursor_methods_preserve_every_admitted_coordinate() {
        let index: usize = kani::any();
        let height: usize = kani::any();
        let position: u16 = kani::any();
        kani::assume(index <= 11 && height < 64 && position < 1024);
        let node = Link::<u8, u8>::dangling();
        let cursor = Cursor::new(node, index, height);
        assert!(cursor.node == node && cursor.index() == index && cursor.height() == height);
        assert!(cursor.position < 1024);
        let mut cursor = Cursor { node, position };
        cursor.set_index(index);
        assert!(cursor.node == node && cursor.index() == index);
        assert!(cursor.height() == (position >> 4) as usize && cursor.position < 1024);
    }
}

pub(super) struct BorrowState<K, V> {
    front: BorrowEdge<K, V>,
    back: BorrowEdge<K, V>,
    pub(super) remaining: Checked<usize>,
}
impl<K, V> Clone for BorrowState<K, V> {
    fn clone(&self) -> Self {
        Self {
            front: self.front,
            back: self.back,
            remaining: self.remaining,
        }
    }
}
impl<K, V> BorrowState<K, V> {
    fn empty() -> Self {
        Self {
            front: BorrowEdge::new(Link::dangling(), 0, 0),
            back: BorrowEdge::new(Link::dangling(), 0, 0),
            remaining: Checked::new(0),
        }
    }
    /// # Safety
    /// Every descent belongs to a complete live subtree at the given height.
    #[inline(always)]
    unsafe fn edge(mut node: Link<K, V>, mut height: usize, back: bool) -> BorrowEdge<K, V> {
        // SAFETY: all internal child edges and final leaf headers are live.
        unsafe {
            while height > 0 {
                node = node::child(node, if back { node::len(node) } else { 0 });
                height = sub(height, 1);
            }
            BorrowEdge::new(node, if back { node::len(node) } else { 0 }, 0)
        }
    }
    #[inline(always)]
    fn new<P: AllocationPolicy>(map: &BTreeMap<K, V, P>) -> Self {
        let Some(root) = map.root else {
            return Self::empty();
        };
        // SAFETY: the map holds its complete tree live. Canonical borrowed
        // iteration starts at leaf edges of height zero; the packed owning
        // cursor conversion is needed only by a partially consumed IntoIter.
        unsafe {
            Self {
                front: Self::edge(root, map.height.get(), false),
                back: Self::edge(root, map.height.get(), true),
                remaining: Checked::new(map.len()),
            }
        }
    }
    /// Preserve an internal pending pair: its consumed-side child may already
    /// have been freed by IntoIter. Only leaf key positions become edges here.
    pub(super) fn remaining(state: State<K, V>) -> Self {
        Self {
            front: state
                .front
                .map(BorrowEdge::from_cursor)
                .unwrap_or_else(|| BorrowEdge::new(Link::dangling(), 0, 0)),
            back: state
                .back
                .map(|mut c| {
                    if c.height() == 0 {
                        c.set_index(add(c.index(), 1));
                    }
                    BorrowEdge::from_cursor(c)
                })
                .unwrap_or_else(|| BorrowEdge::new(Link::dangling(), 0, 0)),
            remaining: state.remaining,
        }
    }
    /// # Safety
    /// The interval's entire source tree remains live (unlike IntoIter).
    pub(super) unsafe fn interval(state: State<K, V>) -> Self {
        let mut result = Self::remaining(state);
        // SAFETY: convert internal endpoint pairs to the adjacent leaf edges
        // bounding the same physical interval, independently of user ordering.
        unsafe {
            if result.front.height() > 0 {
                let c = result.front;
                result.front = Self::edge(node::child(c.node, c.index()), sub(c.height(), 1), true);
            }
            if result.back.height() > 0 {
                let c = result.back;
                result.back = Self::edge(
                    node::child(c.node, add(c.index(), 1)),
                    sub(c.height(), 1),
                    false,
                );
            }
        }
        result
    }
    /// # Safety
    /// Live initialized interval. EXACT requires an exact remaining pair count;
    /// otherwise both cursors are leaf edges delimiting a checked range.
    #[inline]
    unsafe fn take<const EXACT: bool>(&mut self, back: bool) -> Option<BorrowEdge<K, V>> {
        let remaining = self.remaining.get();
        if remaining == 0 {
            return None;
        }
        if !EXACT && self.front == self.back {
            self.remaining = Checked::new(0);
            return None;
        }
        let mut c = if back { self.back } else { self.front };
        let last = EXACT && remaining == 1;
        // SAFETY: leaf edges lie in 0..=len. At a boundary, parent indices and
        // heights describe the live remaining ancestor path. Last-item handling
        // avoids descending into a child already retired at the other end.
        unsafe {
            let next;
            if c.height() > 0 {
                next = if last {
                    c
                } else {
                    Self::edge(
                        node::child(c.node, if back { c.index() } else { add(c.index(), 1) }),
                        sub(c.height(), 1),
                        back,
                    )
                };
            } else if (back && c.index() > 0) || (!back && c.index() < node::len(c.node)) {
                let (i, edge) = super::geometry::leaf_edge(c.index(), node::len(c.node), back);
                next = BorrowEdge::new(c.node, edge, 0);
                c.set_index(i);
            } else {
                // Leaf boundaries occur once per leaf; keep the in-leaf step as
                // the fall-through loop body, as std's layout does.
                core::hint::cold_path();
                let mut n = c.node;
                let mut h = 0;
                loop {
                    let (parent, i) = node::parent(n)?;
                    h = add(h, 1);
                    if (back && i > 0) || (!back && i < node::len(parent)) {
                        let at = if back { sub(i, 1) } else { i };
                        c = BorrowEdge::new(parent, at, h);
                        next = if last {
                            c
                        } else {
                            Self::edge(
                                node::child(parent, if back { at } else { add(at, 1) }),
                                sub(h, 1),
                                back,
                            )
                        };
                        break;
                    }
                    n = parent;
                }
            }
            if EXACT {
                self.remaining = Checked::new(sub(remaining, 1));
            }
            if back {
                self.back = next;
            } else {
                self.front = next;
            }
        }
        Some(c)
    }
}

pub struct Iter<'a, K, V> {
    pub(super) state: BorrowState<K, V>,
    pub(super) marker: PhantomData<&'a (K, V)>,
}
impl<'a, K, V> Iter<'a, K, V> {
    #[inline(always)]
    pub(super) fn new<P: AllocationPolicy>(map: &'a BTreeMap<K, V, P>) -> Self {
        Self {
            state: BorrowState::new(map),
            marker: PhantomData,
        }
    }
}
impl<K, V> Clone for Iter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: immutable map borrow holds every initialized slot live.
        unsafe {
            let c = self.state.take::<true>(false)?;
            Some((
                &*node::key(c.node, c.index()),
                &*node::val(c.node, c.index()),
            ))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.state.remaining.get();
        (n, Some(n))
    }
}
impl<K, V> DoubleEndedIterator for Iter<'_, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        // SAFETY: shared map borrow and a non-overlapping remaining interval.
        unsafe {
            let c = self.state.take::<true>(true)?;
            Some((
                &*node::key(c.node, c.index()),
                &*node::val(c.node, c.index()),
            ))
        }
    }
}
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}
impl<K, V> FusedIterator for Iter<'_, K, V> {}
// SAFETY: shared traversal touches only headers and immutable payload slots.
unsafe impl<K: Sync, V: Sync> Send for Iter<'_, K, V> {}
// SAFETY: observing the iterator through a shared borrow does not advance it.
unsafe impl<K: Sync, V: Sync> Sync for Iter<'_, K, V> {}
impl<K, V> Default for Iter<'_, K, V> {
    fn default() -> Self {
        Self {
            state: BorrowState::empty(),
            marker: PhantomData,
        }
    }
}
impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
pub struct IterMut<'a, K, V> {
    pub(super) state: BorrowState<K, V>,
    pub(super) marker: PhantomData<(&'a K, &'a mut V)>,
}
impl<'a, K, V> IterMut<'a, K, V> {
    pub(super) fn new<P: AllocationPolicy>(map: &'a mut BTreeMap<K, V, P>) -> Self {
        Self {
            state: BorrowState::new(map),
            marker: PhantomData,
        }
    }
}
impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: removes one distinct slot before yielding; raw header traversal
        // never materializes a reference spanning previously yielded values.
        unsafe {
            let c = self.state.take::<true>(false)?;
            Some((
                &*node::key(c.node, c.index()),
                &mut *node::val(c.node, c.index()),
            ))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.state.remaining.get();
        (n, Some(n))
    }
}
impl<K, V> DoubleEndedIterator for IterMut<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        // SAFETY: shared frontier count excludes slots yielded from either end.
        unsafe {
            let c = self.state.take::<true>(true)?;
            Some((
                &*node::key(c.node, c.index()),
                &mut *node::val(c.node, c.index()),
            ))
        }
    }
}
impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {}
impl<K, V> FusedIterator for IterMut<'_, K, V> {}
impl<K, V> Default for IterMut<'_, K, V> {
    fn default() -> Self {
        Self {
            state: BorrowState::empty(),
            marker: PhantomData,
        }
    }
}
// SAFETY: exclusive payload ownership moves with the remaining iterator range.
unsafe impl<K: Send, V: Send> Send for IterMut<'_, K, V> {}
// SAFETY: a shared iterator borrow permits shared inspection only.
unsafe impl<K: Sync, V: Sync> Sync for IterMut<'_, K, V> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for IterMut<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(Iter {
                state: self.state.clone(),
                marker: PhantomData,
            })
            .finish()
    }
}

macro_rules! projection {
    ($name:ident,$base:ident,$item:ty,$pat:pat=>$value:expr) => {
        pub struct $name<'a, K, V>(pub(super) $base<'a, K, V>);
        impl<'a, K, V> Iterator for $name<'a, K, V> {
            type Item = $item;
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|$pat| $value)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }
        impl<K, V> DoubleEndedIterator for $name<'_, K, V> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map(|$pat| $value)
            }
        }
        impl<K, V> ExactSizeIterator for $name<'_, K, V> {}
        impl<K, V> FusedIterator for $name<'_, K, V> {}
        impl<K, V> Default for $name<'_, K, V> {
            fn default() -> Self {
                Self($base::default())
            }
        }
    };
}
projection!(Keys,Iter,&'a K,(k,_)=>k);
projection!(Values,Iter,&'a V,(_,v)=>v);
projection!(ValuesMut,IterMut,&'a mut V,(_,v)=>v);
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
            .entries(
                Iter {
                    state: self.0.state.clone(),
                    marker: PhantomData,
                }
                .map(|(_, v)| v),
            )
            .finish()
    }
}

pub struct Range<'a, K, V>(pub(super) Iter<'a, K, V>);
impl<K, V> Clone for Range<'_, K, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<'a, K, V> Iterator for Range<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: checked range edges bound distinct live initialized slots.
        unsafe {
            let c = self.0.state.take::<false>(false)?;
            Some((
                &*node::key(c.node, c.index()),
                &*node::val(c.node, c.index()),
            ))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.0.state.remaining.get()))
    }
}
impl<K, V> DoubleEndedIterator for Range<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        // SAFETY: the two leaf edges exclude all previously yielded slots.
        unsafe {
            let c = self.0.state.take::<false>(true)?;
            Some((
                &*node::key(c.node, c.index()),
                &*node::val(c.node, c.index()),
            ))
        }
    }
}
impl<K, V> FusedIterator for Range<'_, K, V> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for Range<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<K, V> Default for Range<'_, K, V> {
    fn default() -> Self {
        Self(Iter::default())
    }
}
pub struct RangeMut<'a, K, V>(pub(super) IterMut<'a, K, V>);
impl<'a, K, V> Iterator for RangeMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: advance the checked frontier before exposing one unique slot.
        unsafe {
            let c = self.0.state.take::<false>(false)?;
            Some((
                &*node::key(c.node, c.index()),
                &mut *node::val(c.node, c.index()),
            ))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.0.state.remaining.get()))
    }
}
impl<K, V> DoubleEndedIterator for RangeMut<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        // SAFETY: the shared physical frontier excludes prior mutable yields.
        unsafe {
            let c = self.0.state.take::<false>(true)?;
            Some((
                &*node::key(c.node, c.index()),
                &mut *node::val(c.node, c.index()),
            ))
        }
    }
}
impl<K, V> FusedIterator for RangeMut<'_, K, V> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for RangeMut<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(Range(Iter {
                state: self.0.state.clone(),
                marker: PhantomData,
            }))
            .finish()
    }
}
impl<K, V> Default for RangeMut<'_, K, V> {
    fn default() -> Self {
        Self(IterMut::default())
    }
}

pub struct IntoIter<K, V, P: AllocationPolicy = Unmetered> {
    state: State<K, V>,
    policy: P,
}
impl<K, V, P: AllocationPolicy> IntoIterator for BTreeMap<K, V, P> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V, P>;
    fn into_iter(mut self) -> Self::IntoIter {
        let state = State::new(&self);
        let root = self.root.take();
        self.len = Checked::new(0);
        if state.remaining.get() == 0
            && let Some(root) = root
        {
            // SAFETY: empty tree has only an empty leaf root.
            unsafe { node::free(root, 0, &self.policy) };
        }
        let owner = core::mem::ManuallyDrop::new(self);
        IntoIter {
            state,
            // SAFETY: detached forest and policy are moved into this iterator once.
            policy: unsafe { core::ptr::read(&owner.policy) },
        }
    }
}
impl<'a, K, V, P: AllocationPolicy> IntoIterator for &'a BTreeMap<K, V, P> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, K, V, P: AllocationPolicy> IntoIterator for &'a mut BTreeMap<K, V, P> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<K, V, P: AllocationPolicy> IntoIter<K, V, P> {
    pub fn as_iter(&self) -> Iter<'_, K, V> {
        Iter {
            state: BorrowState::remaining(self.state.clone()),
            marker: PhantomData,
        }
    }
    fn take(&mut self, back: bool) -> Option<(K, V)> {
        // Snapshot the borrowed ledger, not a reference into this iterator.
        // Capturing &self.policy makes LLVM spill the local-budget cursor and
        // remaining count on every element; the Copy receipt owns no policy.
        let accounting = self.policy.budget();
        Self::take_state(&mut self.state, back, |n, h| {
            // SAFETY: take_state retires each exhausted unique allocation once.
            unsafe { crate::storage::deallocate(n.cast(), node::layout::<K, V>(h), accounting) };
        })
    }
    pub(super) fn take_with(
        &mut self,
        back: bool,
        retire: impl FnMut(Link<K, V>, usize),
    ) -> Option<(K, V)> {
        Self::take_state(&mut self.state, back, retire)
    }
    fn take_state(
        state: &mut State<K, V>,
        back: bool,
        retire: impl FnMut(Link<K, V>, usize),
    ) -> Option<(K, V)> {
        if state.remaining.get() == 0 {
            return None;
        }
        let c = if back { state.back? } else { state.front? };
        // SAFETY: exclusively owned remaining cursor. Read pair before freeing an
        // exhausted node; do not follow a stale child edge beyond the other frontier.
        unsafe {
            let pair = (
                node::key(c.node, c.index()).read(),
                node::val(c.node, c.index()).read(),
            );
            state.advance_owned(c, back, retire);
            Some(pair)
        }
    }
    fn drop_next(&mut self) -> bool {
        if self.state.remaining.get() == 0 {
            return false;
        }
        let Some(mut c) = self.state.front else {
            return false;
        };
        let policy = &self.policy;
        if !core::mem::needs_drop::<K>() && !core::mem::needs_drop::<V>() {
            // SAFETY: no payload requires a destructor. Discard the remaining
            // leaf prefix in one step, capped by the exact unconsumed count.
            unsafe {
                if c.height() == 0 {
                    let skip = super::geometry::drop_skip(
                        c.index(),
                        node::len(c.node),
                        self.state.remaining.get(),
                    );
                    c.set_index(add(c.index(), skip));
                    self.state.front = Some(c);
                    self.state.remaining = Checked::new(sub(self.state.remaining.get(), skip));
                }
                self.state
                    .advance_owned(c, false, |n, h| node::free(n, h, policy));
            }
        } else {
            let mut deferred = node::Spares::new(policy);
            // SAFETY: remove the pair from the traversal frontier before user
            // Drop, but retain retired storage until in-place cleanup finishes.
            // Unwinding drops the pair's value, then deferred storage, then the
            // outer iterator cleanup guard resumes the remaining interval.
            unsafe {
                self.state
                    .advance_owned(c, false, |n, h| deferred.recycle(n, h));
                node::drop_pair(c.node, c.index());
            }
        }
        true
    }
}
impl<K, V, P: AllocationPolicy> Iterator for IntoIter<K, V, P> {
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        self.take(false)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.state.remaining.get();
        (n, Some(n))
    }
}
impl<K, V, P: AllocationPolicy> DoubleEndedIterator for IntoIter<K, V, P> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.take(true)
    }
}
impl<K, V, P: AllocationPolicy> ExactSizeIterator for IntoIter<K, V, P> {}
impl<K, V, P: AllocationPolicy> FusedIterator for IntoIter<K, V, P> {}
impl<K, V> Default for IntoIter<K, V> {
    fn default() -> Self {
        BTreeMap::new().into_iter()
    }
}
// SAFETY: remaining initialized slots and all unreleased nodes move exclusively.
unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send for IntoIter<K, V, P> {}
// SAFETY: shared access exposes only an immutable view of remaining slots.
unsafe impl<K: Sync, V: Sync, P: AllocationPolicy + Sync> Sync for IntoIter<K, V, P> {}
impl<K: core::fmt::Debug, V: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug
    for IntoIter<K, V, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.as_iter()).finish()
    }
}
impl<K, V, P: AllocationPolicy> Drop for IntoIter<K, V, P> {
    fn drop(&mut self) {
        struct Cleanup<'a, K, V, P: AllocationPolicy>(&'a mut IntoIter<K, V, P>);
        impl<K, V, P: AllocationPolicy> Drop for Cleanup<'_, K, V, P> {
            fn drop(&mut self) {
                while self.0.drop_next() {}
            }
        }
        let guard = Cleanup(self);
        while guard.0.drop_next() {}
    }
}
pub struct IntoKeys<K, V, P: AllocationPolicy = Unmetered>(pub(super) IntoIter<K, V, P>);
pub struct IntoValues<K, V, P: AllocationPolicy = Unmetered>(pub(super) IntoIter<K, V, P>);
macro_rules! owned_projection {
    ($name:ident,$item:ty,$pat:pat=>$value:expr) => {
        impl<K, V, P: AllocationPolicy> Iterator for $name<K, V, P> {
            type Item = $item;
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|$pat| $value)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }
        impl<K, V, P: AllocationPolicy> DoubleEndedIterator for $name<K, V, P> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map(|$pat| $value)
            }
        }
        impl<K, V, P: AllocationPolicy> ExactSizeIterator for $name<K, V, P> {}
        impl<K, V, P: AllocationPolicy> FusedIterator for $name<K, V, P> {}
    };
}
// Each projection drops the discarded half while the returned half is still a local: if
// that destructor unwinds, the returned half is dropped too instead of leaking.
owned_projection!(IntoKeys,K,(k,v)=>{drop(v);k});
owned_projection!(IntoValues,V,(k,v)=>{drop(k);v});
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
impl<K, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    pub fn into_keys(self) -> IntoKeys<K, V, P> {
        IntoKeys(self.into_iter())
    }
    pub fn into_values(self) -> IntoValues<K, V, P> {
        IntoValues(self.into_iter())
    }
}

macro_rules! fallible_iter {
    ([$($g:tt)*] $ty:ty)=>{
        impl<$($g)*> TryIterator for $ty {
            type Item=<Self as Iterator>::Item;type Error=Infallible;
            fn try_next(&mut self)->Result<Option<Self::Item>,Infallible>{Ok(self.next())}
            fn try_size_hint(&self)->Result<(usize,Option<usize>),Infallible>{Ok(self.size_hint())}
        }
        impl<$($g)*> TryDoubleEndedIterator for $ty {fn try_next_back(&mut self)->Result<Option<Self::Item>,Infallible>{Ok(self.next_back())}}
        impl<$($g)*> TryFusedIterator for $ty {}
    }
}
macro_rules! exact {
    ([$($g:tt)*] $ty:ty)=>{impl<$($g)*> TryExactSizeIterator for $ty {fn try_len(&self)->Result<usize,Infallible>{Ok(self.len())}}}
}
fallible_iter!(['a,K,V] Iter<'a,K,V>);
exact!(['a,K,V] Iter<'a,K,V>);
fallible_iter!(['a,K,V] IterMut<'a,K,V>);
exact!(['a,K,V] IterMut<'a,K,V>);
fallible_iter!(['a,K,V] Keys<'a,K,V>);
exact!(['a,K,V] Keys<'a,K,V>);
fallible_iter!(['a,K,V] Values<'a,K,V>);
exact!(['a,K,V] Values<'a,K,V>);
fallible_iter!(['a,K,V] ValuesMut<'a,K,V>);
exact!(['a,K,V] ValuesMut<'a,K,V>);
fallible_iter!(['a,K,V] Range<'a,K,V>);
fallible_iter!(['a,K,V] RangeMut<'a,K,V>);
fallible_iter!([K,V,P:AllocationPolicy] IntoIter<K,V,P>);
exact!([K,V,P:AllocationPolicy] IntoIter<K,V,P>);
fallible_iter!([K,V,P:AllocationPolicy] IntoKeys<K,V,P>);
exact!([K,V,P:AllocationPolicy] IntoKeys<K,V,P>);
fallible_iter!([K,V,P:AllocationPolicy] IntoValues<K,V,P>);
exact!([K,V,P:AllocationPolicy] IntoValues<K,V,P>);

#[cfg(kani)]
mod drop_frontier_proofs {
    use super::*;
    #[kani::proof]
    #[kani::unwind(3)]
    fn no_drop_leaf_cleanup_consumes_each_valid_owned_prefix_once() {
        let count: usize = kani::any();
        let first: usize = kani::any();
        kani::assume(0 < count && count <= super::super::CAP && first < count);
        if let Ok(root) = node::allocate::<u8, u8, _>(0, &Unmetered) {
            let data = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            // SAFETY: a fresh leaf, initialized exactly through its admitted prefix.
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr(), node::key(root, 0), count);
                core::ptr::copy_nonoverlapping(data.as_ptr(), node::val(root, 0), count);
                node::set_len(root, count);
            }
            let mut map = BTreeMap::new();
            map.root = Some(root);
            map.len = Checked::new(count);
            let mut owner = map.into_iter();
            // The consumed prefix owns no resources. The remaining interval is
            // exact and lies in this single live leaf, including both endpoints.
            owner.state.front = Some(Cursor::new(root, first, 0));
            owner.state.remaining = Checked::new(count - first);
            assert!(owner.drop_next());
            assert!(owner.len() == 0 && owner.state.front.is_none() && owner.state.back.is_none());
            assert!(!owner.drop_next());
        }
    }
}
