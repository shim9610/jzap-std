//! Checked B+ point lookup over keys stored partly in the node.
//!
//! Every visited node is validated in full: each adjacent key pair, the
//! fences and the search comparisons must be decided before a result is
//! returned. A comparison the stored parts cannot decide yields a request.
//! The caller attaches the requested key to its node buffer and provides the
//! node again, so every answer comes from one pass whose comparisons were all
//! decided.

use super::super::LookupStep;
use super::Contents;
use crate::Error;
use core::cmp::Ordering;

/// A key as stored in a node: complete, or only partly present.
///
/// Implementations must satisfy these laws, which the lookup's exactness
/// relies on. A completion is a complete key that the stored key or fence
/// may represent.
///
/// - **Consistency.** A `Some(o)` from any comparison holds for every
///   completion of both sides.
/// - **Resolved keys.** `full` returns the key's completion.
/// - **Fences.** `fence` represents the same completions as the key, and
///   `fence_admits(fence, full)` is true exactly when `full` is one of them.
/// - **Determinism.** The same arguments give the same results.
pub trait PartialKey {
    /// The complete key, e.g. `[u8]`.
    type Full: ?Sized + Ord;
    /// A self-contained copy kept as a fence after the node buffer is reused.
    type Fence: Copy;

    /// Order two stored keys. `None`: the stored parts cannot decide.
    fn cmp_stored(&self, other: &Self) -> Option<Ordering>;
    /// Order this stored key against a complete key.
    fn cmp_full(&self, full: &Self::Full) -> Option<Ordering>;
    /// The complete key, when it is inline or attached.
    fn full(&self) -> Option<&Self::Full>;

    fn fence(&self) -> Self::Fence;
    /// Order this stored key against a fence.
    fn cmp_fence(&self, fence: &Self::Fence) -> Option<Ordering>;
    /// Order a fence against a complete key.
    fn fence_cmp_full(fence: &Self::Fence, full: &Self::Full) -> Option<Ordering>;
    /// Whether `full` is a completion of the fence.
    fn fence_admits(fence: &Self::Fence, full: &Self::Full) -> bool;
}

/// A byte-string key stored as up to `P` inline bytes and its full length.
///
/// A key of at most `P` bytes is complete inline. A longer key keeps its
/// first `P` bytes, and its full bytes can be attached by the caller.
#[derive(Clone, Copy, Debug)]
pub struct PrefixKey<'a, const P: usize> {
    prefix: [u8; P],
    len: usize,
    full: Option<&'a [u8]>,
}

/// The inline part of a [`PrefixKey`], kept by the lookup as a fence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefixFence<const P: usize> {
    prefix: [u8; P],
    len: usize,
}

/// The stored bytes of a key whose full length is `len`.
#[inline]
fn inline<const P: usize>(prefix: &[u8; P], len: usize) -> &[u8] {
    prefix.get(..len.min(P)).unwrap_or(prefix.as_slice())
}

/// Order two keys from their inline bytes and full lengths.
#[inline]
fn cmp_parts<const P: usize>(a: &[u8], a_len: usize, b: &[u8], b_len: usize) -> Option<Ordering> {
    let shared = a.len().min(b.len());
    let order = a.get(..shared).cmp(&b.get(..shared));
    if order.is_ne() {
        return Some(order);
    }
    match (a_len <= P, b_len <= P) {
        (true, true) => Some(a_len.cmp(&b_len)),
        // A complete key equal to the start of a longer key orders first.
        (true, false) => Some(Ordering::Less),
        (false, true) => Some(Ordering::Greater),
        (false, false) => None,
    }
}

/// Order a key, given its inline bytes and full length, against a full key.
#[inline]
fn cmp_part_full<const P: usize>(a: &[u8], a_len: usize, full: &[u8]) -> Option<Ordering> {
    if a_len <= P {
        return Some(a.cmp(full));
    }
    let shared = a.len().min(full.len());
    let order = a.get(..shared).cmp(&full.get(..shared));
    if order.is_ne() {
        return Some(order);
    }
    // The full key is a proper prefix of every completion.
    (full.len() <= P).then_some(Ordering::Greater)
}

#[inline]
fn admits(a: &[u8], a_len: usize, full: &[u8]) -> bool {
    full.len() == a_len && full.get(..a.len()) == Some(a)
}

impl<'a, const P: usize> PrefixKey<'a, P> {
    /// A key of `len` bytes whose first `min(len, P)` bytes are in `prefix`.
    /// Bytes of `prefix` beyond `len` are ignored.
    pub fn new(prefix: [u8; P], len: usize) -> Self {
        Self {
            prefix,
            len,
            full: None,
        }
    }

    /// A key with its full bytes attached.
    pub fn from_full(full: &'a [u8]) -> Self {
        let mut prefix = [0; P];
        for (slot, byte) in prefix.iter_mut().zip(full) {
            *slot = *byte;
        }
        Self {
            prefix,
            len: full.len(),
            full: Some(full),
        }
    }

    /// Attach the key's full bytes. They must have the stored length and
    /// start with the stored prefix.
    pub fn attach(&mut self, full: &'a [u8]) -> Result<(), Error> {
        if !admits(self.inline(), self.len, full) {
            return Err(Error::InvalidStructure);
        }
        self.full = Some(full);
        Ok(())
    }

    /// The full length of the key.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the key is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The stored bytes: the whole key when it has at most `P` bytes.
    pub fn inline(&self) -> &[u8] {
        inline(&self.prefix, self.len)
    }
}

impl<const P: usize> PartialKey for PrefixKey<'_, P> {
    type Full = [u8];
    type Fence = PrefixFence<P>;

    fn cmp_stored(&self, other: &Self) -> Option<Ordering> {
        cmp_parts::<P>(self.inline(), self.len, other.inline(), other.len)
    }

    fn cmp_full(&self, full: &[u8]) -> Option<Ordering> {
        cmp_part_full::<P>(self.inline(), self.len, full)
    }

    fn full(&self) -> Option<&[u8]> {
        self.full
            .or_else(|| (self.len <= P).then(|| inline(&self.prefix, self.len)))
    }

    fn fence(&self) -> PrefixFence<P> {
        PrefixFence {
            prefix: self.prefix,
            len: self.len,
        }
    }

    fn cmp_fence(&self, fence: &PrefixFence<P>) -> Option<Ordering> {
        cmp_parts::<P>(
            self.inline(),
            self.len,
            inline(&fence.prefix, fence.len),
            fence.len,
        )
    }

    fn fence_cmp_full(fence: &PrefixFence<P>, full: &[u8]) -> Option<Ordering> {
        cmp_part_full::<P>(inline(&fence.prefix, fence.len), fence.len, full)
    }

    fn fence_admits(fence: &PrefixFence<P>, full: &[u8]) -> bool {
        admits(inline(&fence.prefix, fence.len), fence.len, full)
    }
}

/// A borrowed node of partial keys whose local shape has been checked. Key
/// order is checked by [`PartialLookup`], because it may need resolution.
pub struct PartialNode<'a, K, V, I = usize> {
    keys: &'a [K],
    contents: Contents<'a, V, I>,
}

impl<'a, K, V, I> PartialNode<'a, K, V, I> {
    /// Each key identifies a value. An empty leaf can serve as the root.
    pub fn leaf(keys: &'a [K], values: &'a [V]) -> Result<Self, Error> {
        if keys.len() != values.len() {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            keys,
            contents: Contents::Leaf(values),
        })
    }

    /// Each separator is the minimum key of its corresponding child.
    pub fn branch(minima: &'a [K], children: &'a [I]) -> Result<Self, Error> {
        if minima.is_empty() || minima.len() != children.len() {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            keys: minima,
            contents: Contents::Branch(children),
        })
    }

    pub fn keys(&self) -> &'a [K] {
        self.keys
    }
}

/// A fence and the node and separator index it was copied from.
#[derive(Clone, Copy)]
struct Fence<T, I> {
    key: T,
    node: I,
    index: usize,
}

/// The outcome of one comparison within a pass.
enum Decision {
    Order(Ordering),
    /// The key at this index of the current node must be resolved.
    Key(usize),
    /// The fence must be resolved.
    Fence,
}

#[inline]
fn order_keys<K: PartialKey>(a: &K, left: usize, b: &K, right: usize) -> Decision {
    if let Some(order) = a.cmp_stored(b) {
        return Decision::Order(order);
    }
    match (a.full(), b.full()) {
        (Some(a), Some(b)) => Decision::Order(a.cmp(b)),
        (Some(a), None) => b
            .cmp_full(a)
            .map_or(Decision::Key(right), |order| Decision::Order(order.reverse())),
        (None, Some(b)) => a.cmp_full(b).map_or(Decision::Key(left), Decision::Order),
        (None, None) => Decision::Key(left),
    }
}

#[inline]
fn order_fence<K: PartialKey>(
    key: &K,
    index: usize,
    fence: &K::Fence,
    full: Option<&K::Full>,
) -> Decision {
    if let Some(order) = key.cmp_fence(fence) {
        return Decision::Order(order);
    }
    match (key.full(), full) {
        (Some(key), Some(fence)) => Decision::Order(key.cmp(fence)),
        (Some(key), None) => K::fence_cmp_full(fence, key)
            .map_or(Decision::Fence, |order| Decision::Order(order.reverse())),
        (None, Some(fence)) => key.cmp_full(fence).map_or(Decision::Key(index), Decision::Order),
        (None, None) => Decision::Key(index),
    }
}

#[inline]
fn order_query<K: PartialKey>(key: &K, index: usize, query: &K::Full) -> Decision {
    match key.cmp_full(query).or_else(|| key.full().map(|full| full.cmp(query))) {
        Some(order) => Decision::Order(order),
        None => Decision::Key(index),
    }
}

/// Validates the visited path of a point lookup over partial keys.
///
/// The lookup is generic over the full key and fence types, so each node may
/// use keys that borrow different storage. Requests and refusals leave the
/// same node requested.
pub struct PartialLookup<'q, F: ?Sized, T, I> {
    query: &'q F,
    pending: Option<I>,
    height: usize,
    depth: usize,
    path: [Option<I>; usize::BITS as usize],
    minimum: Option<Fence<T, I>>,
    ceiling: Option<Fence<T, I>>,
}

/// A step of one pass, before the lookup state is updated.
enum Pass<'a, K, V, I> {
    Step(LookupStep<'a, K, V, I>),
    Descend(usize, I),
}

impl<'q, F: ?Sized + Ord, T: Copy, I: Copy + Eq> PartialLookup<'q, F, T, I> {
    /// Start at `root`. A leaf has height zero.
    pub fn new(root: I, height: usize, query: &'q F) -> Result<Self, Error> {
        if height >= usize::BITS as usize {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            query,
            pending: Some(root),
            height,
            depth: 0,
            path: [None; usize::BITS as usize],
            minimum: None,
            ceiling: None,
        })
    }

    pub fn requested_node(&self) -> Option<I> {
        self.pending
    }

    /// The node and separator index that the minimum and ceiling fences of
    /// the requested node were copied from.
    pub fn fence_origins(&self) -> (Option<(I, usize)>, Option<(I, usize)>) {
        (
            self.minimum.map(|fence| (fence.node, fence.index)),
            self.ceiling.map(|fence| (fence.node, fence.index)),
        )
    }

    /// Validate and search one node.
    ///
    /// Returns `NeedKey(index)` when the key at `index` must be attached and
    /// the node provided again, and `NeedFence` when the fences' full keys
    /// must be passed with [`provide_fenced`](Self::provide_fenced).
    pub fn provide<'a, K, V>(
        &mut self,
        id: I,
        node: &PartialNode<'a, K, V, I>,
    ) -> Result<LookupStep<'a, K, V, I>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        self.provide_fenced(id, node, None, None)
    }

    /// [`provide`](Self::provide) with the full keys of the current fences.
    /// A full key passed for a fence that does not exist, or that the fence
    /// does not admit, is refused.
    pub fn provide_fenced<'a, K, V>(
        &mut self,
        id: I,
        node: &PartialNode<'a, K, V, I>,
        minimum: Option<&F>,
        ceiling: Option<&F>,
    ) -> Result<LookupStep<'a, K, V, I>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        match self.pass(id, node, minimum, ceiling)? {
            Pass::Step(step) => {
                if matches!(step, LookupStep::Found(..) | LookupStep::Absent) {
                    self.pending = None;
                }
                Ok(step)
            }
            Pass::Descend(index, child) => self.descend(id, index, node.keys, child),
        }
    }

    fn pass<'a, K, V>(
        &self,
        id: I,
        node: &PartialNode<'a, K, V, I>,
        minimum: Option<&F>,
        ceiling: Option<&F>,
    ) -> Result<Pass<'a, K, V, I>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        if self.pending != Some(id) || (self.height == 0) != matches!(node.contents, Contents::Leaf(_))
        {
            return Err(Error::InvalidStructure);
        }
        for (fence, full) in [(&self.minimum, minimum), (&self.ceiling, ceiling)] {
            match (fence, full) {
                (Some(fence), Some(full)) if !K::fence_admits(&fence.key, full) => {
                    return Err(Error::InvalidStructure);
                }
                (None, Some(_)) => return Err(Error::InvalidStructure),
                _ => {}
            }
        }
        let keys = node.keys;
        let mut fences = (false, false);
        if let Some(fence) = &self.minimum {
            let first = keys.first().ok_or(Error::InvalidStructure)?;
            match order_fence(first, 0, &fence.key, minimum) {
                Decision::Order(Ordering::Equal) => {}
                Decision::Order(_) => return Err(Error::InvalidStructure),
                Decision::Key(index) => return Ok(Pass::Step(LookupStep::NeedKey(index))),
                Decision::Fence => fences.0 = true,
            }
        }
        let mut left = 0usize;
        for pair in keys.windows(2) {
            let (Some(a), Some(b)) = (pair.first(), pair.last()) else {
                return Err(Error::InvalidStructure);
            };
            let right = left.checked_add(1).ok_or(Error::InvalidStructure)?;
            match order_keys(a, left, b, right) {
                Decision::Order(Ordering::Less) => {}
                Decision::Order(_) | Decision::Fence => return Err(Error::InvalidStructure),
                Decision::Key(index) => return Ok(Pass::Step(LookupStep::NeedKey(index))),
            }
            left = right;
        }
        if let (Some(fence), Some(last)) = (&self.ceiling, keys.last()) {
            match order_fence(last, left, &fence.key, ceiling) {
                Decision::Order(Ordering::Less) => {}
                Decision::Order(_) => return Err(Error::InvalidStructure),
                Decision::Key(index) => return Ok(Pass::Step(LookupStep::NeedKey(index))),
                Decision::Fence => fences.1 = true,
            }
        }

        // Binary search over the validated keys: `low` keys order below the
        // query, and keys at `high` and above order above it.
        let mut low = 0usize;
        let mut high = keys.len();
        let mut equal = None;
        while low < high {
            let span = high.checked_sub(low).ok_or(Error::InvalidStructure)?;
            let middle = low.checked_add(span / 2).ok_or(Error::InvalidStructure)?;
            let key = keys.get(middle).ok_or(Error::InvalidStructure)?;
            match order_query(key, middle, self.query) {
                Decision::Order(Ordering::Less) => {
                    low = middle.checked_add(1).ok_or(Error::InvalidStructure)?;
                }
                Decision::Order(Ordering::Greater) => high = middle,
                Decision::Order(Ordering::Equal) => {
                    equal = Some(middle);
                    break;
                }
                Decision::Key(index) => return Ok(Pass::Step(LookupStep::NeedKey(index))),
                Decision::Fence => return Err(Error::InvalidStructure),
            }
        }
        if fences.0 || fences.1 {
            return Ok(Pass::Step(LookupStep::NeedFence {
                minimum: fences.0,
                ceiling: fences.1,
            }));
        }
        Ok(match node.contents {
            Contents::Leaf(values) => match equal {
                Some(index) => {
                    let (key, value) = keys
                        .get(index)
                        .zip(values.get(index))
                        .ok_or(Error::InvalidStructure)?;
                    Pass::Step(LookupStep::Found(key, value))
                }
                None => Pass::Step(LookupStep::Absent),
            },
            Contents::Branch(children) => {
                // The last separator at or below the query.
                let index = match equal {
                    Some(index) => index,
                    None => match low.checked_sub(1) {
                        Some(index) => index,
                        None => return Ok(Pass::Step(LookupStep::Absent)),
                    },
                };
                let child = *children.get(index).ok_or(Error::InvalidStructure)?;
                Pass::Descend(index, child)
            }
        })
    }

    fn descend<'a, K, V>(
        &mut self,
        id: I,
        index: usize,
        minima: &[K],
        child: I,
    ) -> Result<LookupStep<'a, K, V, I>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        let depth = self.depth.checked_add(1).ok_or(Error::InvalidStructure)?;
        let height = self.height.checked_sub(1).ok_or(Error::InvalidStructure)?;
        let next = index.checked_add(1).ok_or(Error::InvalidStructure)?;
        let minimum = minima.get(index).ok_or(Error::InvalidStructure)?;
        let (slot, history) = self
            .path
            .get_mut(..=self.depth)
            .and_then(|path| path.split_last_mut())
            .ok_or(Error::InvalidStructure)?;
        if child == id || history.contains(&Some(child)) {
            return Err(Error::InvalidStructure);
        }
        *slot = Some(id);
        self.depth = depth;
        self.height = height;
        self.minimum = Some(Fence {
            key: minimum.fence(),
            node: id,
            index,
        });
        if let Some(separator) = minima.get(next) {
            self.ceiling = Some(Fence {
                key: separator.fence(),
                node: id,
                index: next,
            });
        }
        self.pending = Some(child);
        Ok(LookupStep::NeedNode(child))
    }
}

#[cfg(kani)]
mod proofs {
    use super::super::{Lookup, Node};
    use super::*;

    const P: usize = 2;
    const MAX: usize = 4;

    /// An arbitrary byte string of at most `MAX` bytes.
    fn bytes() -> ([u8; MAX], usize) {
        let len = usize::from(kani::any::<u8>());
        kani::assume(len <= MAX);
        (kani::any(), len)
    }

    fn stored<'a>(full: &[u8]) -> PrefixKey<'a, P> {
        let mut prefix = [0; P];
        for (slot, byte) in prefix.iter_mut().zip(full) {
            *slot = *byte;
        }
        PrefixKey::new(prefix, full.len())
    }

    fn own(full: &[u8]) -> ([u8; MAX], usize) {
        let mut bytes = [0; MAX];
        for (slot, byte) in bytes.iter_mut().zip(full) {
            *slot = *byte;
        }
        (bytes, full.len())
    }

    fn agrees(decided: Option<Ordering>, actual: Ordering) -> bool {
        decided.is_none_or(|order| order == actual)
    }

    /// Every decided comparison of two stored keys holds for their
    /// completions. Both completions are arbitrary, so this covers every
    /// completion of the stored parts.
    #[kani::proof]
    #[kani::unwind(6)]
    fn prefix_key_comparisons_hold_for_every_completion() {
        let (a, a_len) = bytes();
        let (b, b_len) = bytes();
        let (a, b) = (&a[..a_len], &b[..b_len]);
        let (stored_a, stored_b) = (stored(a), stored(b));
        let actual = a.cmp(b);
        assert!(agrees(stored_a.cmp_stored(&stored_b), actual));
        assert!(agrees(stored_a.cmp_full(b), actual));
        assert!(agrees(stored_a.cmp_fence(&stored_b.fence()), actual));
        assert!(agrees(PrefixKey::<P>::fence_cmp_full(&stored_a.fence(), b), actual));
        // Complete keys are always decided.
        if a_len <= P && b_len <= P {
            assert!(stored_a.cmp_stored(&stored_b) == Some(actual));
        }
        if a_len <= P {
            assert!(stored_a.full() == Some(a));
            assert!(stored_a.cmp_full(b) == Some(actual));
        } else {
            assert!(stored_a.full().is_none());
        }
        let admitted = b_len == a_len && b[..a_len.min(P)] == a[..a_len.min(P)];
        assert!(PrefixKey::<P>::fence_admits(&stored_a.fence(), b) == admitted);
        let mut attached = stored_a;
        assert!(attached.attach(b).is_ok() == admitted);
        if admitted {
            assert!(attached.full() == Some(b));
        }
        assert!(PrefixKey::from_full(a).full() == Some(a));
        assert!(PrefixKey::from_full(a).fence() == stored_a.fence());
    }

    /// A leaf lookup, resolving each requested key with its completion,
    /// returns what the complete-key lookup returns over the completions.
    #[kani::proof]
    #[kani::unwind(6)]
    fn partial_leaf_matches_complete_lookup() {
        let count = usize::from(kani::any::<u8>());
        kani::assume(count <= 3);
        let fulls: [([u8; MAX], usize); 3] = [bytes(), bytes(), bytes()];
        let full = |i: usize| &fulls[i].0[..fulls[i].1];
        let completed = [full(0), full(1), full(2)];
        let mut keys = [stored(full(0)), stored(full(1)), stored(full(2))];
        for (i, key) in keys.iter_mut().enumerate() {
            if kani::any() {
                key.attach(completed[i]).unwrap();
            }
        }
        let values = [10u8, 11, 12];
        let (query, query_len) = bytes();
        let query = &query[..query_len];

        let mut lookup = PartialLookup::new(0usize, 0, query).unwrap();
        let mut requests = 0;
        let result = loop {
            let node = PartialNode::leaf(&keys[..count], &values[..count]).unwrap();
            let requested = match lookup.provide(0, &node) {
                Ok(LookupStep::Found(key, value)) => break Ok(Some((key.full().map(own), *value))),
                Ok(LookupStep::Absent) => break Ok(None),
                Ok(LookupStep::NeedKey(index)) => index,
                Ok(_) => panic!("leaf returned another step"),
                Err(error) => break Err(error),
            };
            // Only an unresolved key is requested, so each key at most once.
            assert!(requested < count && keys[requested].full().is_none());
            assert!(lookup.requested_node() == Some(0));
            requests += 1;
            assert!(requests <= count);
            keys[requested].attach(completed[requested]).unwrap();
        };

        let mut complete = Lookup::new(0usize, 0, query).unwrap();
        let expected = Node::leaf(&completed[..count], &values[..count]).and_then(|node| {
            complete.provide(0, &node).map(|step| match step {
                LookupStep::Found(key, value) => Some((Some(own(key)), *value)),
                LookupStep::Absent => None,
                _ => panic!("leaf returned another step"),
            })
        });
        assert!(result == expected);
    }

    /// A root branch over two leaves, with arbitrary attachments and fences
    /// passed on request or in advance, returns what the complete-key lookup
    /// returns over the completions.
    #[kani::proof]
    #[kani::unwind(9)]
    fn partial_two_levels_match_complete_lookup() {
        let fulls: [([u8; MAX], usize); 6] =
            [bytes(), bytes(), bytes(), bytes(), bytes(), bytes()];
        let completed: [&[u8]; 6] = core::array::from_fn(|i| &fulls[i].0[..fulls[i].1]);
        let mut keys: [PrefixKey<'_, P>; 6] = core::array::from_fn(|i| stored(completed[i]));
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                key.attach(full).unwrap();
            }
        }
        let counts = [usize::from(kani::any::<u8>()), usize::from(kani::any::<u8>())];
        kani::assume(counts[0] <= 2 && counts[1] <= 2);
        let ranges = [0..2, 2..2 + counts[0], 4..4 + counts[1]];
        let children = [1usize, 2];
        let values = [0u8, 1, 2, 3, 4, 5];
        let (query, query_len) = bytes();
        let query = &query[..query_len];
        let fence_full = |origin: Option<(usize, usize)>| {
            origin.map(|(node, index)| {
                assert!(node == 0);
                completed[index]
            })
        };

        let mut lookup = PartialLookup::new(0usize, 1, query).unwrap();
        let mut fenced = kani::any::<bool>();
        let mut steps = 0;
        let result = loop {
            steps += 1;
            assert!(steps <= 10);
            let id = lookup.requested_node().unwrap();
            let range = ranges[id].clone();
            let start = range.start;
            let node = if id == 0 {
                PartialNode::branch(&keys[range], &children).unwrap()
            } else {
                PartialNode::leaf(&keys[range.clone()], &values[range]).unwrap()
            };
            let (minimum, ceiling) = lookup.fence_origins();
            let step = if fenced {
                lookup.provide_fenced(id, &node, fence_full(minimum), fence_full(ceiling))
            } else {
                lookup.provide(id, &node)
            };
            let requested = match step {
                Ok(LookupStep::Found(key, value)) => break Ok(Some((key.full().map(own), *value))),
                Ok(LookupStep::Absent) => break Ok(None),
                Ok(LookupStep::NeedNode(child)) => {
                    assert!(id == 0 && lookup.requested_node() == Some(child));
                    fenced = kani::any();
                    continue;
                }
                Ok(LookupStep::NeedKey(index)) => start + index,
                Ok(LookupStep::NeedFence { minimum: need_min, ceiling: need_ceiling }) => {
                    assert!(!fenced && id != 0);
                    assert!(need_min <= minimum.is_some() && need_ceiling <= ceiling.is_some());
                    fenced = true;
                    continue;
                }
                Err(error) => break Err(error),
            };
            assert!(keys[requested].full().is_none());
            keys[requested].attach(completed[requested]).unwrap();
        };

        let mut complete = Lookup::new(0usize, 1, query).unwrap();
        let expected = Node::<_, u8, _>::branch(&completed[..2], &children).and_then(|root| {
            match complete.provide(0, &root)? {
                LookupStep::Absent => Ok(None),
                LookupStep::NeedNode(child) => {
                    let range = ranges[child].clone();
                    let leaf = Node::leaf(&completed[range.clone()], &values[range])?;
                    Ok(match complete.provide(child, &leaf)? {
                        LookupStep::Found(key, value) => Some((Some(own(key)), *value)),
                        LookupStep::Absent => None,
                        _ => panic!("leaf returned another step"),
                    })
                }
                _ => panic!("root returned another step"),
            }
        });
        assert!(result == expected);
    }

    /// Refused and requesting calls leave the lookup unchanged.
    #[kani::proof]
    #[kani::unwind(6)]
    fn requests_and_refusals_keep_the_lookup_state() {
        let fulls: [([u8; MAX], usize); 2] = [bytes(), bytes()];
        let keys = [stored(&fulls[0].0[..fulls[0].1]), stored(&fulls[1].0[..fulls[1].1])];
        let children = [1usize, 2];
        let (query, query_len) = bytes();
        let query = &query[..query_len];
        let node = PartialNode::<_, (), _>::branch(&keys, &children).unwrap();
        let mut lookup = PartialLookup::new(0usize, 1, query).unwrap();
        let id: usize = kani::any();
        let fence: Option<&[u8]> = if kani::any() { Some(query) } else { None };
        match lookup.provide_fenced(id, &node, fence, None) {
            Ok(LookupStep::NeedNode(child)) => {
                assert!(id == 0 && fence.is_none());
                assert!(lookup.requested_node() == Some(child));
            }
            Ok(LookupStep::Absent) => assert!(id == 0 && lookup.requested_node().is_none()),
            Ok(LookupStep::NeedKey(_)) | Err(_) => {
                assert!(lookup.requested_node() == Some(0));
                assert!(lookup.fence_origins() == (None, None));
            }
            _ => panic!("root returned another step"),
        }
    }
}
