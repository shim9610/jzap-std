//! Borrowed node slices. Storage origin and persistence are the caller's concern.
use super::node::{self, Link};
use crate::Checked;
use crate::Error;
use core::marker::PhantomData;
use core::{borrow::Borrow, ops::Bound};

pub mod bplus;

pub struct NodeView<'a, K, V> {
    pub(super) node: Link<K, V>,
    pub(super) height: usize,
    pub(super) marker: PhantomData<&'a (K, V)>,
}
impl<K, V> Copy for NodeView<'_, K, V> {}
impl<K, V> Clone for NodeView<'_, K, V> {
    fn clone(&self) -> Self {
        *self
    }
}
// SAFETY: the view only borrows immutable node slices held live by the map.
unsafe impl<K: Sync, V: Sync> Send for NodeView<'_, K, V> {}
// SAFETY: shared access cannot mutate the borrowed tree or policy.
unsafe impl<K: Sync, V: Sync> Sync for NodeView<'_, K, V> {}
impl<'a, K, V> NodeView<'a, K, V> {
    pub fn height(self) -> usize {
        self.height
    }
    pub fn keys(self) -> &'a [K] {
        // SAFETY: view borrows the map, precisely len initialized key slots.
        unsafe { core::slice::from_raw_parts(node::key(self.node, 0), node::len(self.node)) }
    }
    pub fn values(self) -> &'a [V] {
        // SAFETY: same live node borrow, precisely len initialized value slots.
        unsafe { core::slice::from_raw_parts(node::val(self.node, 0), node::len(self.node)) }
    }
    pub fn child(self, index: usize) -> Result<Option<Self>, Error> {
        if self.height == 0 {
            return if index == 0 {
                Ok(None)
            } else {
                Err(Error::Bounds)
            };
        }
        // SAFETY: map-borrowed live node header; public arbitrary index is checked.
        let n = unsafe { node::len(self.node) };
        if index > n {
            return Err(Error::Bounds);
        }
        // SAFETY: internal kind and initialized edge bound were established above.
        Ok(Some(Self {
            node: unsafe { node::child(self.node, index) },
            height: super::sub(self.height, 1),
            marker: PhantomData,
        }))
    }
}

/// Caller-owned, typed node slices. Child identifiers describe edges rather
/// than addresses. Construction validates local shape and strict ordering.
pub struct NodeSlice<'a, K, V, I = usize> {
    keys: &'a [K],
    values: &'a [V],
    children: Option<&'a [I]>,
}

fn search_index<K: Borrow<Q>, Q: ?Sized + Ord>(
    keys: &[K],
    query: &Q,
) -> Result<Result<usize, usize>, Error> {
    let mut low = 0;
    let mut high = keys.len();
    loop {
        if low == high {
            return Ok(Err(low));
        }
        let middle = super::geometry::middle(low, high);
        let key = keys.get(middle).ok_or(Error::InvalidStructure)?;
        match key.borrow().cmp(query) {
            core::cmp::Ordering::Equal => return Ok(Ok(middle)),
            core::cmp::Ordering::Less => low = super::geometry::advance(low, high, middle, true).0,
            core::cmp::Ordering::Greater => {
                high = super::geometry::advance(low, high, middle, false).1
            }
        }
    }
}

#[cfg(kani)]
mod search_proofs {
    use super::*;
    #[kani::proof]
    #[kani::unwind(10)]
    fn validated_slice_search_matches_std_on_every_small_query() {
        let count = kani::any::<u8>() as usize;
        let query: u8 = kani::any();
        kani::assume(count <= 8);
        let keys = [2u8, 4, 6, 8, 10, 12, 14, 16];
        let values = [0u8; 8];
        let view = NodeSlice::<_, _, usize>::new(&keys[..count], &values[..count], None).unwrap();
        assert!(search_index(view.keys(), &query) == Ok(keys[..count].binary_search(&query)));
    }
}
impl<K, V, I> Copy for NodeSlice<'_, K, V, I> {}
impl<K, V, I> Clone for NodeSlice<'_, K, V, I> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, K: Ord, V, I> NodeSlice<'a, K, V, I> {
    pub fn new(keys: &'a [K], values: &'a [V], children: Option<&'a [I]>) -> Result<Self, Error> {
        if keys.len() != values.len() || keys.iter().zip(keys.iter().skip(1)).any(|(a, b)| a >= b) {
            return Err(Error::InvalidStructure);
        }
        if let Some(edges) = children {
            let count = keys.len().checked_add(1).ok_or(Error::InvalidStructure)?;
            if keys.is_empty() || edges.len() != count {
                return Err(Error::InvalidStructure);
            }
        }
        Ok(Self {
            keys,
            values,
            children,
        })
    }
}
impl<'a, K, V, I> NodeSlice<'a, K, V, I> {
    pub fn keys(self) -> &'a [K] {
        self.keys
    }
    pub fn values(self) -> &'a [V] {
        self.values
    }
    pub fn children(self) -> Option<&'a [I]> {
        self.children
    }
    /// One locally validated lookup step. Only Found borrows this node's data;
    /// NeedNode carries a copied identifier. A caller can therefore consume the
    /// step, release/reuse its buffer and supply the next slice. Cross-node fence
    /// validation is available through Lookup when separator borrows are retained.
    pub fn lookup<Q: ?Sized + Ord>(self, query: &Q) -> Result<LookupStep<'a, K, V, I>, Error>
    where
        K: Borrow<Q>,
        I: Copy,
    {
        match search_index(self.keys, query)? {
            Ok(i) => {
                let (k, v) = self
                    .keys
                    .get(i)
                    .zip(self.values.get(i))
                    .ok_or(Error::InvalidStructure)?;
                Ok(LookupStep::Found(k, v))
            }
            Err(i) => match self.children {
                Some(children) => Ok(LookupStep::NeedNode(
                    *children.get(i).ok_or(Error::InvalidStructure)?,
                )),
                None => Ok(LookupStep::Absent),
            },
        }
    }
    fn check(self, height: usize, low: Option<&K>, high: Option<&K>) -> Result<(), Error>
    where
        K: Ord,
    {
        if (height == 0) != self.children.is_none()
            || self
                .keys
                .iter()
                .any(|k| low.is_some_and(|l| k <= l) || high.is_some_and(|h| k >= h))
        {
            return Err(Error::InvalidStructure);
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum LookupStep<'a, K, V, I> {
    NeedNode(I),
    Found(&'a K, &'a V),
    Absent,
}
/// A lookup continuation, borrowing only supplied slices and separator keys.
/// Rejected nodes do not advance the continuation and can be replaced/retried.
pub struct Lookup<'q, 'a, K, V, I: Copy + Eq, Q: ?Sized = K> {
    key: &'q Q,
    pending: Option<I>,
    height: Checked<usize>,
    depth: Checked<usize>,
    path: [Option<I>; usize::BITS as usize],
    low: Option<&'a K>,
    high: Option<&'a K>,
    marker: PhantomData<&'a V>,
}
impl<'q, 'a, K: Ord + Borrow<Q>, V, I: Copy + Eq, Q: ?Sized + Ord> Lookup<'q, 'a, K, V, I, Q> {
    pub fn new(root: I, height: usize, key: &'q Q) -> Result<Self, Error> {
        if height >= usize::BITS as usize {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            key,
            pending: Some(root),
            height: Checked::new(height),
            depth: Checked::new(0),
            path: [None; usize::BITS as usize],
            low: None,
            high: None,
            marker: PhantomData,
        })
    }
    pub fn requested_node(&self) -> Option<I> {
        self.pending
    }
    pub fn provide(
        &mut self,
        id: I,
        node: NodeSlice<'a, K, V, I>,
    ) -> Result<LookupStep<'a, K, V, I>, Error> {
        if self.pending != Some(id)
            || self
                .path
                .iter()
                .take(self.depth.get())
                .flatten()
                .any(|x| *x == id)
        {
            return Err(Error::InvalidStructure);
        }
        node.check(self.height.get(), self.low, self.high)?;
        let found = search_index(node.keys, self.key)?;
        match found {
            Ok(i) => {
                let (k, v) = node
                    .keys
                    .get(i)
                    .zip(node.values.get(i))
                    .ok_or(Error::InvalidStructure)?;
                self.pending = None;
                Ok(LookupStep::Found(k, v))
            }
            Err(i) => {
                let Some(children) = node.children else {
                    self.pending = None;
                    return Ok(LookupStep::Absent);
                };
                let child = *children.get(i).ok_or(Error::InvalidStructure)?;
                if child == id
                    || self
                        .path
                        .iter()
                        .take(self.depth.get())
                        .flatten()
                        .any(|x| *x == child)
                {
                    return Err(Error::InvalidStructure);
                }
                let slot = self
                    .path
                    .get_mut(self.depth.get())
                    .ok_or(Error::InvalidStructure)?;
                *slot = Some(id);
                self.depth = Checked::new(super::add(self.depth.get(), 1));
                self.height = Checked::new(super::sub(self.height.get(), 1));
                if i > 0 {
                    self.low = node.keys.get(super::sub(i, 1));
                }
                if i < node.keys.len() {
                    self.high = node.keys.get(i);
                }
                self.pending = Some(child);
                Ok(LookupStep::NeedNode(child))
            }
        }
    }
}

pub enum Visit<'a, K, V, I> {
    NeedNode(I),
    Item(&'a K, &'a V),
    Done,
}
struct Frame<'a, K, V, I> {
    id: I,
    node: NodeSlice<'a, K, V, I>,
    next: Checked<usize>,
    descend: bool,
    low: Option<&'a K>,
    high: Option<&'a K>,
}
impl<K, V, I: Copy> Copy for Frame<'_, K, V, I> {}
impl<K, V, I: Copy> Clone for Frame<'_, K, V, I> {
    fn clone(&self) -> Self {
        *self
    }
}
/// In-order traversal with a checked, fixed-capacity path. `advance` asks for
/// one missing node; `provide` borrows that node from caller-managed storage.
/// Range bounds prune disjoint subtrees. No storage backend or I/O is involved.
pub struct Traversal<'q, 'a, K, V, I: Copy + Eq, Q: ?Sized = K> {
    frames: [Option<Frame<'a, K, V, I>>; usize::BITS as usize],
    depth: Checked<usize>,
    root_height: Checked<usize>,
    pending: Option<(I, Option<&'a K>, Option<&'a K>)>,
    start: Bound<&'q Q>,
    end: Bound<&'q Q>,
    done: bool,
}
impl<'q, 'a, K: Ord, V, I: Copy + Eq> Traversal<'q, 'a, K, V, I, K> {
    pub fn new(root: I, height: usize) -> Result<Self, Error> {
        Self::try_range(root, height, Bound::Unbounded, Bound::Unbounded)
    }
}
impl<'q, 'a, K: Ord + Borrow<Q>, V, I: Copy + Eq, Q: ?Sized + Ord> Traversal<'q, 'a, K, V, I, Q> {
    pub fn try_range(
        root: I,
        height: usize,
        start: Bound<&'q Q>,
        end: Bound<&'q Q>,
    ) -> Result<Self, Error> {
        if height >= usize::BITS as usize {
            return Err(Error::InvalidStructure);
        }
        if let (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b)) =
            (start, end)
            && (a > b
                || (a == b && matches!((start, end), (Bound::Excluded(_), Bound::Excluded(_)))))
        {
            return Err(Error::Bounds);
        }
        Ok(Self {
            frames: [None; usize::BITS as usize],
            depth: Checked::new(0),
            root_height: Checked::new(height),
            pending: Some((root, None, None)),
            start,
            end,
            done: false,
        })
    }
    pub fn requested_node(&self) -> Option<I> {
        self.pending.map(|x| x.0)
    }
    pub fn provide(&mut self, id: I, node: NodeSlice<'a, K, V, I>) -> Result<(), Error> {
        let (expected, low, high) = self.pending.ok_or(Error::InvalidStructure)?;
        if expected != id || self.done || self.frames.iter().flatten().any(|f| f.id == id) {
            return Err(Error::InvalidStructure);
        }
        let height = self
            .root_height
            .get()
            .checked_sub(self.depth.get())
            .ok_or(Error::InvalidStructure)?;
        node.check(height, low, high)?;
        let frame = self
            .frames
            .get_mut(self.depth.get())
            .ok_or(Error::InvalidStructure)?;
        *frame = Some(Frame {
            id,
            node,
            next: Checked::new(0),
            descend: true,
            low,
            high,
        });
        self.pending = None;
        Ok(())
    }
    pub fn advance(&mut self) -> Result<Visit<'a, K, V, I>, Error> {
        if self.done {
            return Ok(Visit::Done);
        }
        if let Some((id, _, _)) = self.pending {
            return Ok(Visit::NeedNode(id));
        }
        loop {
            let slot = self
                .frames
                .get_mut(self.depth.get())
                .ok_or(Error::InvalidStructure)?;
            let frame = slot.as_mut().ok_or(Error::InvalidStructure)?;
            let i = frame.next.get();
            if frame.descend
                && let Some(children) = frame.node.children
            {
                let child = *children.get(i).ok_or(Error::InvalidStructure)?;
                let low = if i == 0 {
                    frame.low
                } else {
                    frame.node.keys.get(super::sub(i, 1))
                };
                let high = frame.node.keys.get(i).or(frame.high);
                let skip_low = match (high, self.start) {
                    (Some(h), Bound::Included(s) | Bound::Excluded(s)) => h.borrow() <= s,
                    _ => false,
                };
                let skip_high = match (low, self.end) {
                    (Some(l), Bound::Included(e) | Bound::Excluded(e)) => l.borrow() >= e,
                    _ => false,
                };
                if !skip_low && !skip_high {
                    if self.frames.iter().flatten().any(|f| f.id == child) {
                        return Err(Error::InvalidStructure);
                    }
                    let next_depth = self
                        .depth
                        .get()
                        .checked_add(1)
                        .ok_or(Error::InvalidStructure)?;
                    if next_depth > self.root_height.get() || next_depth >= self.frames.len() {
                        return Err(Error::InvalidStructure);
                    }
                    if let Some(frame) = self
                        .frames
                        .get_mut(self.depth.get())
                        .and_then(Option::as_mut)
                    {
                        frame.descend = false;
                    }
                    self.depth = Checked::new(next_depth);
                    self.pending = Some((child, low, high));
                    return Ok(Visit::NeedNode(child));
                }
                frame.descend = false;
            }
            if let Some((k, v)) = frame.node.keys.get(i).zip(frame.node.values.get(i)) {
                frame.next = Checked::new(i.checked_add(1).ok_or(Error::InvalidStructure)?);
                frame.descend = true;
                let q = k.borrow();
                let before = match self.start {
                    Bound::Unbounded => false,
                    Bound::Included(s) => q < s,
                    Bound::Excluded(s) => q <= s,
                };
                let after = match self.end {
                    Bound::Unbounded => false,
                    Bound::Included(e) => q > e,
                    Bound::Excluded(e) => q >= e,
                };
                if after {
                    self.done = true;
                    return Ok(Visit::Done);
                }
                if !before {
                    return Ok(Visit::Item(k, v));
                }
            } else {
                *slot = None;
                if self.depth.get() == 0 {
                    self.done = true;
                    return Ok(Visit::Done);
                }
                self.depth = Checked::new(super::sub(self.depth.get(), 1));
            }
        }
    }
}
