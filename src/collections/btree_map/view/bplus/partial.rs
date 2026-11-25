//! Checked B+ point lookup over keys stored partly in the node.
//!
//! Every visited node is validated in full: each adjacent key pair, the
//! fences and the search comparisons must be decided before a result is
//! returned. A comparison the stored parts cannot decide yields a request.
//! The caller attaches the requested key to its node buffer and provides the
//! node again, so every answer comes from one pass whose comparisons were all
//! decided.

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
/// Comparisons read the inline bytes first and an attached key's bytes after
/// the prefix only when the prefixes are equal, so keys with distinct
/// prefixes never touch the attached bytes.
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

/// Byte-lexicographic order of two short byte strings, eight bytes at a time
/// as big-endian words, so that inline prefixes compare without a `memcmp`
/// call. Big-endian word order is the byte order of the same eight bytes.
#[inline(always)]
fn cmp_short(a: &[u8], b: &[u8]) -> Ordering {
    let (mut a, mut b) = (a, b);
    while let (Some((x, a_rest)), Some((y, b_rest))) =
        (a.split_first_chunk::<8>(), b.split_first_chunk::<8>())
    {
        let order = u64::from_be_bytes(*x).cmp(&u64::from_be_bytes(*y));
        if order.is_ne() {
            return order;
        }
        (a, b) = (a_rest, b_rest);
    }
    for (x, y) in a.iter().zip(b) {
        let order = x.cmp(y);
        if order.is_ne() {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

/// Byte order of two whole inline prefixes, as big-endian words. `P` is a
/// constant, so the comparison has no loop or length logic.
#[inline(always)]
fn cmp_prefixes<const P: usize>(a: &[u8; P], b: &[u8; P]) -> Ordering {
    let (mut a, mut b) = (a.as_slice(), b.as_slice());
    while let (Some((x, a_rest)), Some((y, b_rest))) =
        (a.split_first_chunk::<8>(), b.split_first_chunk::<8>())
    {
        let order = u64::from_be_bytes(*x).cmp(&u64::from_be_bytes(*y));
        if order.is_ne() {
            return order;
        }
        (a, b) = (a_rest, b_rest);
    }
    a.cmp(b)
}

/// Order two keys from their inline bytes and full lengths.
#[inline]
fn cmp_parts<const P: usize>(a: &[u8], a_len: usize, b: &[u8], b_len: usize) -> Option<Ordering> {
    let shared = a.len().min(b.len());
    let order = cmp_short(
        a.get(..shared).unwrap_or(&[]),
        b.get(..shared).unwrap_or(&[]),
    );
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
        return Some(cmp_short(a, full));
    }
    let shared = a.len().min(full.len());
    let order = cmp_short(
        a.get(..shared).unwrap_or(&[]),
        full.get(..shared).unwrap_or(&[]),
    );
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

    // Inlined into the pass: as an out-of-line call per comparison, the
    // word-wise prefix comparison doubled the pass's stores.
    #[inline(always)]
    fn cmp_stored(&self, other: &Self) -> Option<Ordering> {
        if self.len > P && other.len > P {
            // Both keys are longer than `P`: whole prefixes, then attached tails.
            let order = cmp_prefixes(&self.prefix, &other.prefix);
            if order.is_ne() {
                return Some(order);
            }
            let (a, b) = (self.full?, other.full?);
            return Some(a.get(P..).cmp(&b.get(P..)));
        }
        cmp_parts::<P>(self.inline(), self.len, other.inline(), other.len).or_else(|| {
            // Both keys are longer than `P` with equal prefixes. Attached
            // keys start with their prefixes, so their tails decide.
            let (a, b) = (self.full?, other.full?);
            Some(a.get(P..).cmp(&b.get(P..)))
        })
    }

    #[inline(always)]
    fn cmp_full(&self, full: &[u8]) -> Option<Ordering> {
        if let (true, Some(head)) = (self.len > P, full.first_chunk::<P>()) {
            // Both are longer than `P` bytes or `full` has exactly `P`.
            let order = cmp_prefixes(&self.prefix, head);
            if order.is_ne() {
                return Some(order);
            }
            return match self.full {
                Some(attached) => Some(attached.get(P..).cmp(&full.get(P..))),
                // `full` equals this key's prefix and is shorter.
                None => (full.len() == P).then_some(Ordering::Greater),
            };
        }
        cmp_part_full::<P>(self.inline(), self.len, full).or_else(|| {
            // This key is longer than `P` and `full` shares its prefix.
            Some(self.full?.get(P..).cmp(&full.get(P..)))
        })
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

/// A step of a [`PartialLookup`].
///
/// Complete-key lookups use [`LookupStep`](super::super::LookupStep); this
/// separate type adds the two requests without changing that type's layout.
#[derive(Debug, PartialEq, Eq)]
pub enum PartialStep<'a, K, V, I> {
    NeedNode(I),
    /// Attach the full key at this index of the current node, then provide
    /// the node again.
    NeedKey(usize),
    /// Provide the current node again with the full keys of the fences that
    /// [`PartialLookup::fence_origins`] reports.
    NeedFence,
    Found(&'a K, &'a V),
    Absent,
}

/// A borrowed node of partial keys for [`PartialLookup::visit`], with
/// exclusive access to the keys so that they can be attached during the
/// visit. Construction checks the same local shape as [`PartialNode`].
pub struct PartialNodeMut<'a, K, V, I = usize> {
    keys: &'a mut [K],
    contents: Contents<'a, V, I>,
}

impl<'a, K, V, I> PartialNodeMut<'a, K, V, I> {
    /// Each key identifies a value. An empty leaf can serve as the root.
    pub fn leaf(keys: &'a mut [K], values: &'a [V]) -> Result<Self, Error> {
        if keys.len() != values.len() {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            keys,
            contents: Contents::Leaf(values),
        })
    }

    /// Each separator is the minimum key of its corresponding child.
    pub fn branch(minima: &'a mut [K], children: &'a [I]) -> Result<Self, Error> {
        if minima.is_empty() || minima.len() != children.len() {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            keys: minima,
            contents: Contents::Branch(children),
        })
    }
}

/// What a [`PartialVisit`] needs next.
#[derive(Debug, PartialEq, Eq)]
pub enum VisitStep {
    /// Attach the key at this index with [`PartialVisit::attach_with`].
    NeedKey(usize),
    /// Run again with the full keys of the fences that
    /// [`PartialLookup::fence_origins`] reports.
    NeedFence,
    /// The node is answered; call [`PartialVisit::finish`].
    Done,
}

/// One node of a [`PartialLookup`], visited with exclusive access to its keys.
///
/// Keys are attached only through [`attach_with`](Self::attach_with), which
/// accepts a change only if it leaves the key's stored parts as they were.
/// Every comparison already decided therefore stays decided, and each
/// [`run`](Self::run) continues where the previous one stopped: a node of
/// `n` keys that needs `r` keys costs one pass of `O(n)` comparisons and `r`
/// resumptions, instead of `r + 1` passes. The lookup changes only in
/// [`finish`](Self::finish); dropping a visit leaves the node requested.
pub struct PartialVisit<'l, 'q, 'a, F: ?Sized, T, I, K, V> {
    lookup: &'l mut PartialLookup<'q, F, T, I>,
    id: I,
    keys: &'a mut [K],
    contents: Contents<'a, V, I>,
    progress: Progress,
    outcome: Option<Outcome<I>>,
    /// Set when an attachment changed a key's stored parts or failed.
    refused: bool,
}

impl<'l, 'q, 'a, F, T, I, K, V> PartialVisit<'l, 'q, 'a, F, T, I, K, V>
where
    F: ?Sized + Ord,
    T: Copy + Eq,
    I: Copy + Eq,
    K: PartialKey<Full = F, Fence = T>,
{
    /// Continue the pass over the node, with the full keys of the fences
    /// when they are needed (or in advance).
    pub fn run(&mut self, minimum: Option<&F>, ceiling: Option<&F>) -> Result<VisitStep, Error> {
        if self.refused {
            return Err(Error::InvalidStructure);
        }
        if self.outcome.is_some() {
            return Ok(VisitStep::Done);
        }
        let outcome = self.lookup.pass(
            self.keys,
            &self.contents,
            minimum,
            ceiling,
            &mut self.progress,
        )?;
        Ok(match outcome {
            Outcome::NeedKey(index) => VisitStep::NeedKey(index),
            Outcome::NeedFence => VisitStep::NeedFence,
            done => {
                self.outcome = Some(done);
                VisitStep::Done
            }
        })
    }

    /// Every unresolved key the rest of the pass could request, as
    /// [`PartialLookup::unresolved_keys`] lists them for the whole node.
    pub fn unresolved_keys(&self) -> impl Iterator<Item = usize> + '_ {
        let keys: &[K] = self.keys;
        let last = keys.len().checked_sub(1);
        (0..keys.len()).filter(move |&index| self.lookup.needs_key(keys, last, index))
    }

    /// Attach the key at `index` with `attach`, for example
    /// `|key| key.attach(full)`. The key must be unresolved. The change is
    /// refused, and the visit with it, if `attach` fails, if it changes the
    /// key's stored parts (its fence), or if the resolved key is not a
    /// completion of them.
    pub fn attach_with(
        &mut self,
        index: usize,
        attach: impl FnOnce(&mut K) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if self.refused {
            return Err(Error::InvalidStructure);
        }
        let key = self.keys.get_mut(index).ok_or(Error::Bounds)?;
        if key.full().is_some() {
            return Err(Error::InvalidStructure);
        }
        let stored = key.fence();
        let result = attach(key);
        let consistent =
            key.fence() == stored && key.full().is_none_or(|full| K::fence_admits(&stored, full));
        if result.is_err() || !consistent {
            self.refused = true;
            return result.and(Err(Error::InvalidStructure));
        }
        Ok(())
    }

    /// Apply the answer to the lookup: `Found` and `Absent` end it, and
    /// `NeedNode` requests the child.
    pub fn finish(self) -> Result<PartialStep<'a, K, V, I>, Error> {
        if self.refused {
            return Err(Error::InvalidStructure);
        }
        let outcome = self.outcome.ok_or(Error::InvalidStructure)?;
        let keys: &'a [K] = self.keys;
        self.lookup.apply(self.id, keys, &self.contents, outcome)
    }
}

/// A fence and the node and separator index it was copied from.
#[derive(Clone, Copy)]
struct Fence<T, I> {
    key: T,
    node: I,
    index: usize,
}

/// The node and separator index a fence was copied from.
type Origin<I> = Option<(I, usize)>;

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
        (Some(a), None) => b.cmp_full(a).map_or(Decision::Key(right), |order| {
            Decision::Order(order.reverse())
        }),
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
        (None, Some(fence)) => key
            .cmp_full(fence)
            .map_or(Decision::Key(index), Decision::Order),
        (None, None) => Decision::Key(index),
    }
}

#[inline]
fn order_query<K: PartialKey>(key: &K, index: usize, query: &K::Full) -> Decision {
    match key
        .cmp_full(query)
        .or_else(|| key.full().map(|full| full.cmp(query)))
    {
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

/// The outcome of one pass, before the lookup state is updated.
enum Outcome<I> {
    NeedKey(usize),
    NeedFence,
    Found(usize),
    Absent,
    Descend(usize, I),
}

/// How far a pass over one node got. Every completed check stays valid as
/// long as no key's stored parts and no resolved key change, which
/// `PartialVisit` enforces; `provide` starts from scratch on every call.
#[derive(Clone, Copy, Default)]
struct Progress {
    minimum: Check,
    /// Adjacent pairs `(i, i + 1)` with `i` below this are ordered.
    pairs: usize,
    ceiling: Check,
    /// Search bounds once the search has started, and an equal key.
    search: Option<(usize, usize)>,
    equal: Option<usize>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Check {
    #[default]
    Unchecked,
    Passed,
    /// Decided only with the fence's full key.
    Fence,
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
    pub fn fence_origins(&self) -> (Origin<I>, Origin<I>) {
        (
            self.minimum.map(|fence| (fence.node, fence.index)),
            self.ceiling.map(|fence| (fence.node, fence.index)),
        )
    }

    /// Every unresolved key of `node` that a pass over it could request, in
    /// index order.
    ///
    /// Attaching all of them leaves no `NeedKey` for this node: a pass can
    /// then only answer, refuse or request the fences. Whether a key is
    /// listed depends on that key, its neighbours' stored parts, the fences
    /// and the query, and not on whether other keys are attached, so the
    /// caller can collect the indexes, then attach them in any order. Like
    /// the lookup, this keeps no state; a node supplied later is validated
    /// in full again.
    pub fn unresolved_keys<'n, K, V>(
        &'n self,
        node: &'n PartialNode<'_, K, V, I>,
    ) -> impl Iterator<Item = usize> + 'n
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        let keys = node.keys;
        let last = keys.len().checked_sub(1);
        (0..keys.len()).filter(move |&index| self.needs_key(keys, last, index))
    }

    /// Visit the requested node with exclusive access to its keys; see
    /// [`PartialVisit`]. A wrong identifier or node kind is refused and
    /// leaves the lookup unchanged.
    pub fn visit<'l, 'a, K, V>(
        &'l mut self,
        id: I,
        node: PartialNodeMut<'a, K, V, I>,
    ) -> Result<PartialVisit<'l, 'q, 'a, F, T, I, K, V>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        self.check(id, &node.contents)?;
        Ok(PartialVisit {
            lookup: self,
            id,
            keys: node.keys,
            contents: node.contents,
            progress: Progress::default(),
            outcome: None,
            refused: false,
        })
    }

    /// Whether the pass could request the key at `index`. Lists both
    /// unresolved keys of every adjacent pair the stored parts cannot
    /// decide, and each key whose fence or query comparison would request it
    /// when no fence key is passed, which requests at least as much as with
    /// fence keys.
    fn needs_key<K>(&self, keys: &[K], last: Option<usize>, index: usize) -> bool
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        let Some(key) = keys.get(index) else {
            return false;
        };
        if key.full().is_some() {
            return false;
        }
        let left = index
            .checked_sub(1)
            .and_then(|left| keys.get(left))
            .is_some_and(|left| left.cmp_stored(key).is_none());
        let right = index
            .checked_add(1)
            .and_then(|right| keys.get(right))
            .is_some_and(|right| key.cmp_stored(right).is_none());
        let minimum = index == 0
            && self
                .minimum
                .as_ref()
                .is_some_and(|fence| key.cmp_fence(&fence.key).is_none());
        let ceiling = Some(index) == last
            && self
                .ceiling
                .as_ref()
                .is_some_and(|fence| key.cmp_fence(&fence.key).is_none());
        left || right || minimum || ceiling || key.cmp_full(self.query).is_none()
    }

    /// Validate and search one node.
    ///
    /// Returns `NeedKey(index)` when the key at `index` must be attached and
    /// the node provided again, and `NeedFence` when the full keys of the
    /// fences reported by [`fence_origins`](Self::fence_origins) must be
    /// passed with [`provide_fenced`](Self::provide_fenced).
    pub fn provide<'a, K, V>(
        &mut self,
        id: I,
        node: &PartialNode<'a, K, V, I>,
    ) -> Result<PartialStep<'a, K, V, I>, Error>
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
    ) -> Result<PartialStep<'a, K, V, I>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        self.check(id, &node.contents)?;
        let mut progress = Progress::default();
        let outcome = self.pass(node.keys, &node.contents, minimum, ceiling, &mut progress)?;
        self.apply(id, node.keys, &node.contents, outcome)
    }

    /// The node must be the requested one, of the kind its height needs.
    fn check<V>(&self, id: I, contents: &Contents<'_, V, I>) -> Result<(), Error> {
        if self.pending != Some(id) || (self.height == 0) != matches!(contents, Contents::Leaf(_)) {
            return Err(Error::InvalidStructure);
        }
        Ok(())
    }

    /// Turn an outcome into a step, updating the lookup for an answer or a
    /// descent. Requests leave it unchanged.
    fn apply<'a, K, V>(
        &mut self,
        id: I,
        keys: &'a [K],
        contents: &Contents<'a, V, I>,
        outcome: Outcome<I>,
    ) -> Result<PartialStep<'a, K, V, I>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        Ok(match outcome {
            Outcome::NeedKey(index) => PartialStep::NeedKey(index),
            Outcome::NeedFence => PartialStep::NeedFence,
            Outcome::Found(index) => {
                let Contents::Leaf(values) = contents else {
                    return Err(Error::InvalidStructure);
                };
                let values: &'a [V] = values;
                let (key, value) = keys
                    .get(index)
                    .zip(values.get(index))
                    .ok_or(Error::InvalidStructure)?;
                self.pending = None;
                PartialStep::Found(key, value)
            }
            Outcome::Absent => {
                self.pending = None;
                PartialStep::Absent
            }
            Outcome::Descend(index, child) => return self.descend(id, index, keys, child),
        })
    }

    /// One pass over a node, continuing from `progress`. Returns at the
    /// first key it needs, or with the answer once every comparison is
    /// decided; fence comparisons that need the fence's full key are
    /// reported after everything else.
    fn pass<K, V>(
        &self,
        keys: &[K],
        contents: &Contents<'_, V, I>,
        minimum: Option<&F>,
        ceiling: Option<&F>,
        progress: &mut Progress,
    ) -> Result<Outcome<I>, Error>
    where
        K: PartialKey<Full = F, Fence = T>,
    {
        for (fence, full) in [(&self.minimum, minimum), (&self.ceiling, ceiling)] {
            match (fence, full) {
                (Some(fence), Some(full)) if !K::fence_admits(&fence.key, full) => {
                    return Err(Error::InvalidStructure);
                }
                (None, Some(_)) => return Err(Error::InvalidStructure),
                _ => {}
            }
        }
        if let Some(fence) = &self.minimum
            && progress.minimum != Check::Passed
        {
            let first = keys.first().ok_or(Error::InvalidStructure)?;
            progress.minimum = match order_fence(first, 0, &fence.key, minimum) {
                Decision::Order(Ordering::Equal) => Check::Passed,
                Decision::Order(_) => return Err(Error::InvalidStructure),
                Decision::Key(index) => return Ok(Outcome::NeedKey(index)),
                Decision::Fence => Check::Fence,
            };
        }
        let mut left = progress.pairs;
        for pair in keys.get(left..).unwrap_or(&[]).windows(2) {
            let (Some(a), Some(b)) = (pair.first(), pair.last()) else {
                return Err(Error::InvalidStructure);
            };
            let right = left.checked_add(1).ok_or(Error::InvalidStructure)?;
            match order_keys(a, left, b, right) {
                Decision::Order(Ordering::Less) => {}
                Decision::Order(_) | Decision::Fence => return Err(Error::InvalidStructure),
                Decision::Key(index) => {
                    progress.pairs = left;
                    return Ok(Outcome::NeedKey(index));
                }
            }
            left = right;
        }
        progress.pairs = left;
        if let (Some(fence), Some(last)) = (&self.ceiling, keys.last())
            && progress.ceiling != Check::Passed
        {
            progress.ceiling = match order_fence(last, left, &fence.key, ceiling) {
                Decision::Order(Ordering::Less) => Check::Passed,
                Decision::Order(_) => return Err(Error::InvalidStructure),
                Decision::Key(index) => return Ok(Outcome::NeedKey(index)),
                Decision::Fence => Check::Fence,
            };
        }

        // Binary search over the validated keys: `low` keys order below the
        // query, and keys at `high` and above order above it.
        let (mut low, mut high) = progress.search.unwrap_or((0, keys.len()));
        while progress.equal.is_none() && low < high {
            let span = high.checked_sub(low).ok_or(Error::InvalidStructure)?;
            let middle = low.checked_add(span / 2).ok_or(Error::InvalidStructure)?;
            let key = keys.get(middle).ok_or(Error::InvalidStructure)?;
            match order_query(key, middle, self.query) {
                Decision::Order(Ordering::Less) => {
                    low = middle.checked_add(1).ok_or(Error::InvalidStructure)?;
                }
                Decision::Order(Ordering::Greater) => high = middle,
                Decision::Order(Ordering::Equal) => progress.equal = Some(middle),
                Decision::Key(index) => {
                    progress.search = Some((low, high));
                    return Ok(Outcome::NeedKey(index));
                }
                Decision::Fence => return Err(Error::InvalidStructure),
            }
        }
        progress.search = Some((low, high));
        if progress.minimum == Check::Fence || progress.ceiling == Check::Fence {
            return Ok(Outcome::NeedFence);
        }
        Ok(match contents {
            Contents::Leaf(_) => progress.equal.map_or(Outcome::Absent, Outcome::Found),
            Contents::Branch(children) => {
                // The last separator at or below the query.
                let index = match progress.equal {
                    Some(index) => index,
                    None => match low.checked_sub(1) {
                        Some(index) => index,
                        None => return Ok(Outcome::Absent),
                    },
                };
                Outcome::Descend(index, *children.get(index).ok_or(Error::InvalidStructure)?)
            }
        })
    }

    fn descend<'a, K, V>(
        &mut self,
        id: I,
        index: usize,
        minima: &[K],
        child: I,
    ) -> Result<PartialStep<'a, K, V, I>, Error>
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
        Ok(PartialStep::NeedNode(child))
    }
}

#[cfg(kani)]
mod proofs {
    use super::super::super::LookupStep;
    use super::super::{Lookup, Node};
    use super::*;

    const P: usize = 2;
    const MAX: usize = 4;
    // The lookup harnesses use one inline byte and keys of up to two bytes:
    // keys longer than the prefix and pairs with equal prefixes remain.
    const SMALL_P: usize = 1;
    const SMALL: usize = 2;

    /// An arbitrary byte string of at most `M` bytes.
    fn bytes<const M: usize>() -> ([u8; M], usize) {
        let len = usize::from(kani::any::<u8>());
        kani::assume(len <= M);
        (kani::any(), len)
    }

    fn stored<'a, const Q: usize>(full: &[u8]) -> PrefixKey<'a, Q> {
        let mut prefix = [0; Q];
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

    /// Byte order, as the replacement of the word-wise comparisons in the
    /// lookup harnesses. `short_comparison_is_byte_order` and
    /// `prefix_comparison_is_byte_order` prove the product functions equal
    /// to it, so harnesses with these stubs compose with those lemmas.
    fn cmp_short_by_bytes(a: &[u8], b: &[u8]) -> Ordering {
        a.cmp(b)
    }

    fn cmp_prefixes_by_bytes<const Q: usize>(a: &[u8; Q], b: &[u8; Q]) -> Ordering {
        a.cmp(b)
    }

    fn agrees(decided: Option<Ordering>, actual: Ordering) -> bool {
        decided.is_none_or(|order| order == actual)
    }

    /// The word-wise comparison of short byte strings is byte order, for
    /// every pair of strings of up to 17 bytes: two whole words and a
    /// remainder byte, with unequal lengths.
    #[kani::proof]
    #[kani::unwind(19)]
    fn short_comparison_is_byte_order() {
        let (a, a_len) = bytes::<17>();
        let (b, b_len) = bytes::<17>();
        let (a, b) = (&a[..a_len], &b[..b_len]);
        assert!(cmp_short(a, b) == a.cmp(b));
    }

    /// Whole inline prefixes of 17 bytes compare in byte order: two words
    /// and a remainder byte.
    #[kani::proof]
    #[kani::unwind(19)]
    fn prefix_comparison_is_byte_order() {
        let a: [u8; 17] = kani::any();
        let b: [u8; 17] = kani::any();
        assert!(cmp_prefixes(&a, &b) == a.cmp(&b));
    }

    /// The comparison laws with an 8-byte prefix, where the whole-prefix
    /// paths compare one word and attached keys compare their tails: keys of
    /// up to 9 bytes, stored and attached. The remainder bytes of longer
    /// prefixes are covered by `prefix_comparison_is_byte_order`.
    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(11)]
    fn eight_byte_prefix_comparisons_hold_for_every_completion() {
        let (a, a_len) = bytes::<9>();
        let (b, b_len) = bytes::<9>();
        let (a, b) = (&a[..a_len], &b[..b_len]);
        let actual = a.cmp(b);
        let (stored_a, stored_b) = (stored::<8>(a), stored::<8>(b));
        let (attached_a, attached_b) = (PrefixKey::<8>::from_full(a), PrefixKey::<8>::from_full(b));
        assert!(agrees(stored_a.cmp_stored(&stored_b), actual));
        assert!(agrees(stored_a.cmp_full(b), actual));
        assert!(agrees(attached_a.cmp_stored(&stored_b), actual));
        assert!(agrees(stored_a.cmp_stored(&attached_b), actual));
        assert!(attached_a.cmp_stored(&attached_b) == Some(actual));
        assert!(attached_a.cmp_full(b) == Some(actual));
        if a_len <= 8 {
            assert!(stored_a.cmp_full(b) == Some(actual));
        }
    }

    /// Every decided comparison of two stored keys holds for their
    /// completions. Both completions are arbitrary, so this covers every
    /// completion of the stored parts.
    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(6)]
    fn prefix_key_comparisons_hold_for_every_completion() {
        let (a, a_len) = bytes::<MAX>();
        let (b, b_len) = bytes::<MAX>();
        let (a, b) = (&a[..a_len], &b[..b_len]);
        let (stored_a, stored_b) = (stored::<P>(a), stored::<P>(b));
        let actual = a.cmp(b);
        assert!(agrees(stored_a.cmp_stored(&stored_b), actual));
        assert!(agrees(stored_a.cmp_full(b), actual));
        assert!(agrees(stored_a.cmp_fence(&stored_b.fence()), actual));
        assert!(agrees(
            PrefixKey::<P>::fence_cmp_full(&stored_a.fence(), b),
            actual
        ));
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
        assert!(PrefixKey::<P>::from_full(a).full() == Some(a));
        // Attached keys: both attached decide exactly; one attached never
        // contradicts the completions.
        let (attached_a, attached_b) = (PrefixKey::<P>::from_full(a), PrefixKey::<P>::from_full(b));
        assert!(attached_a.cmp_stored(&attached_b) == Some(actual));
        assert!(attached_a.cmp_full(b) == Some(actual));
        assert!(agrees(attached_a.cmp_stored(&stored_b), actual));
        assert!(agrees(stored_a.cmp_stored(&attached_b), actual));
        assert!(agrees(attached_a.cmp_fence(&stored_b.fence()), actual));
        assert!(PrefixKey::from_full(a).fence() == stored_a.fence());
    }

    /// A leaf of `N` keys, resolving each requested key with its completion,
    /// gives what the complete-key lookup gives over the completions. The key
    /// count is fixed per harness so that every loop has a concrete bound.
    fn leaf_matches_complete_lookup<const N: usize>() {
        let fulls: [([u8; SMALL], usize); N] = core::array::from_fn(|_| bytes());
        let completed: [&[u8]; N] = core::array::from_fn(|i| &fulls[i].0[..fulls[i].1]);
        let mut keys: [PrefixKey<'_, SMALL_P>; N] = core::array::from_fn(|i| stored(completed[i]));
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                let Ok(()) = key.attach(full) else {
                    panic!("initial attachment refused")
                };
            }
        }
        let values: [u8; N] = core::array::from_fn(|i| i as u8);
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];

        let Ok(mut lookup) = PartialLookup::new(0usize, 0, query) else {
            panic!("partial lookup not created")
        };
        let mut result = None;
        // Each request resolves one more key, so N + 1 passes suffice.
        for _ in 0..=N {
            let Ok(node) = PartialNode::leaf(&keys, &values) else {
                panic!("partial leaf refused")
            };
            let requested = match lookup.provide(0, &node) {
                Ok(PartialStep::Found(key, value)) => {
                    result = Some(Ok(Some((key.full().map(own), *value))));
                    break;
                }
                Ok(PartialStep::Absent) => {
                    result = Some(Ok(None));
                    break;
                }
                Ok(PartialStep::NeedKey(index)) => index,
                Ok(_) => panic!("leaf returned another step"),
                Err(error) => {
                    result = Some(Err(error));
                    break;
                }
            };
            // Only an unresolved key is requested, so each key at most once.
            // The key is selected by a concrete loop: indexing the key array
            // with the symbolic index produced a contradictory counterexample
            // (see docs/bplus-partial-keys.md).
            assert!(requested < N);
            assert!(lookup.requested_node() == Some(0));
            for (i, key) in keys.iter_mut().enumerate() {
                if i == requested {
                    assert!(key.full().is_none());
                    let Ok(()) = key.attach(completed[i]) else {
                        panic!("requested attachment refused")
                    };
                }
            }
        }

        let Ok(mut complete) = Lookup::new(0usize, 0, query) else {
            panic!("complete lookup not created")
        };
        let expected = Node::leaf(&completed, &values).and_then(|node| {
            complete.provide(0, &node).map(|step| match step {
                LookupStep::Found(key, value) => Some((Some(own(key)), *value)),
                LookupStep::Absent => None,
                _ => panic!("leaf returned another step"),
            })
        });
        assert!(result == Some(expected));
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_leaf_of_one_matches_complete_lookup() {
        leaf_matches_complete_lookup::<1>();
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_leaf_of_two_matches_complete_lookup() {
        leaf_matches_complete_lookup::<2>();
    }

    /// Attach the completion of the key at `requested`, selecting it with a
    /// concrete loop rather than a symbolic index.
    fn resolve<'a, const N: usize>(
        keys: &mut [PrefixKey<'a, SMALL_P>; N],
        completed: &[&'a [u8]; N],
        requested: usize,
    ) {
        assert!(requested < N);
        for (i, key) in keys.iter_mut().enumerate() {
            if i == requested {
                assert!(key.full().is_none());
                let Ok(()) = key.attach(completed[i]) else {
                    panic!("requested attachment refused")
                };
            }
        }
    }

    /// The root step over a branch of two separators, with arbitrary
    /// attachments, routes as the complete-key lookup does and records the
    /// separators it descends between as fences.
    fn branch_routes_as_complete_lookup(attached: [bool; 2]) {
        let raw: [([u8; SMALL], usize); 2] = [bytes(), bytes()];
        let completed: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let mut keys: [PrefixKey<'_, SMALL_P>; 2] = [stored(completed[0]), stored(completed[1])];
        for ((key, full), attach) in keys.iter_mut().zip(completed).zip(attached) {
            if attach {
                let Ok(()) = key.attach(full) else {
                    panic!("initial attachment refused")
                };
            }
        }
        let children = [1usize, 2];
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let Ok(mut lookup) = PartialLookup::new(0usize, 1, query) else {
            panic!("partial lookup not created")
        };
        let mut result = None;
        for _ in 0..3 {
            let Ok(node) = PartialNode::<_, u8, _>::branch(&keys, &children) else {
                panic!("branch refused")
            };
            let requested = match lookup.provide(0, &node) {
                Ok(PartialStep::NeedNode(child)) => {
                    result = Some(Ok(Some(child)));
                    break;
                }
                Ok(PartialStep::Absent) => {
                    result = Some(Ok(None));
                    break;
                }
                Ok(PartialStep::NeedKey(index)) => index,
                Ok(_) => panic!("root returned another step"),
                Err(error) => {
                    result = Some(Err(error));
                    break;
                }
            };
            assert!(lookup.requested_node() == Some(0));
            resolve(&mut keys, &completed, requested);
        }
        // The specification that `branch_routes_to_the_last_admitted_minimum`
        // proves for the complete-key lookup: strictly increasing separators,
        // and the last separator at or below the query.
        let expected: Result<Option<usize>, Error> = if completed[0] >= completed[1] {
            Err(Error::InvalidStructure)
        } else if completed[1] <= query {
            Ok(Some(2))
        } else if completed[0] <= query {
            Ok(Some(1))
        } else {
            Ok(None)
        };
        assert!(result == Some(expected));
        // The state the child step starts from: the fences are the
        // separators' fences and record where they were copied from.
        let fence = |fence: Option<Fence<PrefixFence<SMALL_P>, usize>>| {
            fence.map(|f| (f.key, f.node, f.index))
        };
        match expected {
            Ok(Some(1)) => {
                assert!(fence(lookup.minimum) == Some((keys[0].fence(), 0, 0)));
                assert!(fence(lookup.ceiling) == Some((keys[1].fence(), 0, 1)));
            }
            Ok(Some(_)) => {
                assert!(fence(lookup.minimum) == Some((keys[1].fence(), 0, 1)));
                assert!(lookup.ceiling.is_none());
            }
            _ => assert!(lookup.minimum.is_none() && lookup.ceiling.is_none()),
        }
        if let Ok(Some(_)) = expected {
            assert!(lookup.depth == 1 && lookup.height == 0 && lookup.path[0] == Some(0));
        }
    }

    // The attachment choice is fixed per harness to keep each one within
    // memory; together they cover every choice.
    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_branch_routes_with_no_separator_attached() {
        branch_routes_as_complete_lookup([false, false]);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_branch_routes_with_the_first_separator_attached() {
        branch_routes_as_complete_lookup([true, false]);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_branch_routes_with_the_second_separator_attached() {
        branch_routes_as_complete_lookup([false, true]);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_branch_routes_with_both_separators_attached() {
        branch_routes_as_complete_lookup([true, true]);
    }

    /// The child step, from the state `branch_routes_as_complete_lookup`
    /// establishes after a descent: a one-key leaf whose fence comparisons are
    /// resolved on request (`eager` false) or in advance gives what the complete-key lookup
    /// checks for that child: the minimum, the ceiling, then the leaf search.
    fn child_fences_match_complete_lookup(route: usize, eager: bool) {
        let raw: [([u8; SMALL], usize); 3] = [bytes(), bytes(), bytes()];
        let separators: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let leaf_full: [&[u8]; 1] = [&raw[2].0[..raw[2].1]];
        let mut leaf: [PrefixKey<'_, SMALL_P>; 1] = [stored(leaf_full[0])];
        if kani::any() {
            let Ok(()) = leaf[0].attach(leaf_full[0]) else {
                panic!("initial attachment refused")
            };
        }
        let values = [7u8];
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        // Descended through separator `route` of root 0 into child 1.
        let fence = |index: usize| Fence {
            key: stored::<SMALL_P>(separators[index]).fence(),
            node: 0usize,
            index,
        };
        let mut path = [None; usize::BITS as usize];
        path[0] = Some(0usize);
        let mut lookup = PartialLookup {
            query,
            pending: Some(1usize),
            height: 0,
            depth: 1,
            path,
            minimum: Some(fence(route)),
            ceiling: (route == 0).then(|| fence(1)),
        };
        let minimum = Some(separators[route]);
        let ceiling = (route == 0).then_some(separators[1]);

        let mut fenced = eager;
        let mut result = None;
        for _ in 0..3 {
            let Ok(node) = PartialNode::leaf(&leaf, &values) else {
                panic!("leaf refused")
            };
            let step = if fenced {
                lookup.provide_fenced(1, &node, minimum, ceiling)
            } else {
                lookup.provide(1, &node)
            };
            match step {
                Ok(PartialStep::Found(key, value)) => {
                    result = Some(Ok(Some((key.full().map(own), *value))));
                    break;
                }
                Ok(PartialStep::Absent) => {
                    result = Some(Ok(None));
                    break;
                }
                Ok(PartialStep::NeedKey(index)) => {
                    assert!(lookup.requested_node() == Some(1));
                    resolve(&mut leaf, &leaf_full, index);
                }
                Ok(PartialStep::NeedFence) => {
                    assert!(!fenced && lookup.requested_node() == Some(1));
                    fenced = true;
                }
                Ok(_) => panic!("leaf returned another step"),
                Err(error) => {
                    result = Some(Err(error));
                    break;
                }
            }
        }
        // `bplus::Lookup::provide` on a leaf: the first key equals the
        // minimum, the last key is below the ceiling, then an equal key.
        let key = leaf_full[0];
        let expected: Result<Option<(Option<([u8; MAX], usize)>, u8)>, Error> =
            if Some(key) != minimum || ceiling.is_some_and(|ceiling| key >= ceiling) {
                Err(Error::InvalidStructure)
            } else if key == query {
                Ok(Some((Some(own(key)), values[0])))
            } else {
                Ok(None)
            };
        assert!(result == Some(expected));
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_child_with_both_fences_in_advance() {
        child_fences_match_complete_lookup(0, true);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_child_with_minimum_in_advance() {
        child_fences_match_complete_lookup(1, true);
    }

    /// One child call without fence keys, from the same state as
    /// `child_fences_match_complete_lookup`: an answer equals the one with
    /// the fences passed, and a request or refusal leaves the state as it
    /// was. With the in-advance harnesses this covers fences on request.
    fn child_call_without_fences(route: usize) {
        let raw: [([u8; SMALL], usize); 3] = [bytes(), bytes(), bytes()];
        let separators: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let leaf_full: &[u8] = &raw[2].0[..raw[2].1];
        let mut key = stored::<SMALL_P>(leaf_full);
        if kani::any() {
            let Ok(()) = key.attach(leaf_full) else {
                panic!("initial attachment refused")
            };
        }
        let leaf = [key];
        let values = [7u8];
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let fence = |index: usize| Fence {
            key: stored::<SMALL_P>(separators[index]).fence(),
            node: 0usize,
            index,
        };
        let mut path = [None; usize::BITS as usize];
        path[0] = Some(0usize);
        let mut lookup = PartialLookup {
            query,
            pending: Some(1usize),
            height: 0,
            depth: 1,
            path,
            minimum: Some(fence(route)),
            ceiling: (route == 0).then(|| fence(1)),
        };
        let before = (
            lookup.pending,
            lookup.depth,
            lookup.height,
            lookup.fence_origins(),
        );
        let Ok(node) = PartialNode::leaf(&leaf, &values) else {
            panic!("leaf refused")
        };
        let without = lookup.provide(1, &node);
        let unchanged = (
            lookup.pending,
            lookup.depth,
            lookup.height,
            lookup.fence_origins(),
        ) == before;
        let Ok(node) = PartialNode::leaf(&leaf, &values) else {
            panic!("leaf refused")
        };
        let mut fenced = PartialLookup {
            query,
            pending: Some(1usize),
            height: 0,
            depth: 1,
            path,
            minimum: Some(fence(route)),
            ceiling: (route == 0).then(|| fence(1)),
        };
        let with = fenced.provide_fenced(
            1,
            &node,
            Some(separators[route]),
            (route == 0).then_some(separators[1]),
        );
        match without {
            Ok(PartialStep::NeedFence) | Ok(PartialStep::NeedKey(_)) | Err(_) => assert!(unchanged),
            Ok(PartialStep::Found(a, b)) => {
                assert!(
                    matches!(with, Ok(PartialStep::Found(c, d)) if core::ptr::eq(a, c) && core::ptr::eq(b, d))
                )
            }
            Ok(PartialStep::Absent) => assert!(matches!(with, Ok(PartialStep::Absent))),
            Ok(PartialStep::NeedNode(_)) => panic!("leaf returned another step"),
        }
        // Without fences, an error is an error with them too: refusals do
        // not depend on the fence keys being unavailable.
        if without.is_err() {
            assert!(with.is_err());
        }
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_child_call_without_both_fences() {
        child_call_without_fences(0);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn partial_child_call_without_minimum() {
        child_call_without_fences(1);
    }

    /// After every listed key of a leaf of `N` keys is attached, one pass
    /// requests no key.
    fn listed_leaf_keys_leave_no_request<const N: usize>() {
        let raw: [([u8; SMALL], usize); N] = core::array::from_fn(|_| bytes());
        let completed: [&[u8]; N] = core::array::from_fn(|i| &raw[i].0[..raw[i].1]);
        let mut keys: [PrefixKey<'_, SMALL_P>; N] = core::array::from_fn(|i| stored(completed[i]));
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                let Ok(()) = key.attach(full) else {
                    panic!("initial attachment refused")
                };
            }
        }
        let values: [u8; N] = core::array::from_fn(|i| i as u8);
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let Ok(mut lookup) = PartialLookup::new(0usize, 0, query) else {
            panic!("partial lookup not created")
        };
        let Ok(node) = PartialNode::<_, _, usize>::leaf(&keys, &values) else {
            panic!("leaf refused")
        };
        // `unresolved_keys` is `(0..len).filter(needs_key)`. The predicate is
        // evaluated per index because a harness consuming the iterator
        // produced counterexamples that their own concrete playbacks refute.
        let mut selected = [false; N];
        for (index, slot) in selected.iter_mut().enumerate() {
            *slot = lookup.needs_key(node.keys, N.checked_sub(1), index);
        }
        for ((key, full), selected) in keys.iter_mut().zip(completed).zip(selected) {
            if selected {
                assert!(key.full().is_none());
                let Ok(()) = key.attach(full) else {
                    panic!("listed attachment refused")
                };
            }
        }
        let Ok(node) = PartialNode::leaf(&keys, &values) else {
            panic!("leaf refused")
        };
        assert!(!matches!(
            lookup.provide(0, &node),
            Ok(PartialStep::NeedKey(_))
        ));
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn listed_keys_of_a_one_key_leaf_leave_no_request() {
        listed_leaf_keys_leave_no_request::<1>();
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn listed_keys_of_a_two_key_leaf_leave_no_request() {
        listed_leaf_keys_leave_no_request::<2>();
    }

    /// After every listed separator is attached, the root pass requests no key.
    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn listed_separators_leave_no_request() {
        let raw: [([u8; SMALL], usize); 2] = [bytes(), bytes()];
        let completed: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let mut keys: [PrefixKey<'_, SMALL_P>; 2] = [stored(completed[0]), stored(completed[1])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                let Ok(()) = key.attach(full) else {
                    panic!("initial attachment refused")
                };
            }
        }
        let children = [1usize, 2];
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let Ok(mut lookup) = PartialLookup::new(0usize, 1, query) else {
            panic!("partial lookup not created")
        };
        let Ok(node) = PartialNode::<_, u8, _>::branch(&keys, &children) else {
            panic!("branch refused")
        };
        let mut selected = [false; 2];
        for (index, slot) in selected.iter_mut().enumerate() {
            *slot = lookup.needs_key(node.keys, Some(1), index);
        }
        for ((key, full), selected) in keys.iter_mut().zip(completed).zip(selected) {
            if selected {
                let Ok(()) = key.attach(full) else {
                    panic!("listed attachment refused")
                };
            }
        }
        let Ok(node) = PartialNode::<_, u8, _>::branch(&keys, &children) else {
            panic!("branch refused")
        };
        assert!(!matches!(
            lookup.provide(0, &node),
            Ok(PartialStep::NeedKey(_))
        ));
    }

    /// From the state after a descent, attaching the listed key of a one-key
    /// leaf leaves no key request, with or without the fence keys.
    fn listed_child_key_leaves_no_request(route: usize) {
        let raw: [([u8; SMALL], usize); 3] = [bytes(), bytes(), bytes()];
        let separators: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let leaf_full: &[u8] = &raw[2].0[..raw[2].1];
        let mut key = stored::<SMALL_P>(leaf_full);
        if kani::any() {
            let Ok(()) = key.attach(leaf_full) else {
                panic!("initial attachment refused")
            };
        }
        let values = [7u8];
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let fence = |index: usize| Fence {
            key: stored::<SMALL_P>(separators[index]).fence(),
            node: 0usize,
            index,
        };
        let mut path = [None; usize::BITS as usize];
        path[0] = Some(0usize);
        let mut lookup = PartialLookup {
            query,
            pending: Some(1usize),
            height: 0,
            depth: 1,
            path,
            minimum: Some(fence(route)),
            ceiling: (route == 0).then(|| fence(1)),
        };
        let leaf = [key];
        let Ok(node) = PartialNode::leaf(&leaf, &values) else {
            panic!("leaf refused")
        };
        let listed = lookup.unresolved_keys(&node).count();
        assert!(listed <= 1);
        let mut leaf = leaf;
        if listed == 1 {
            let Ok(()) = leaf[0].attach(leaf_full) else {
                panic!("listed attachment refused")
            };
        }
        let Ok(node) = PartialNode::leaf(&leaf, &values) else {
            panic!("leaf refused")
        };
        let step = if kani::any() {
            lookup.provide(1, &node)
        } else {
            lookup.provide_fenced(
                1,
                &node,
                Some(separators[route]),
                (route == 0).then_some(separators[1]),
            )
        };
        assert!(!matches!(step, Ok(PartialStep::NeedKey(_))));
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn listed_child_key_with_both_fences_leaves_no_request() {
        listed_child_key_leaves_no_request(0);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn listed_child_key_with_the_minimum_leaves_no_request() {
        listed_child_key_leaves_no_request(1);
    }

    /// Drive a visit, attaching each requested key's completion through
    /// the visit (selected with a concrete loop), for at most `rounds` runs.
    fn drive_visit<'a, const N: usize>(
        visit: &mut PartialVisit<
            '_,
            '_,
            'a,
            [u8],
            PrefixFence<SMALL_P>,
            usize,
            PrefixKey<'a, SMALL_P>,
            u8,
        >,
        completed: &[&'a [u8]; N],
        rounds: usize,
        fences: (Option<&[u8]>, Option<&[u8]>),
        eager: bool,
    ) -> Result<(), Error> {
        let mut fenced = eager;
        for _ in 0..rounds {
            let (minimum, ceiling) = if fenced { fences } else { (None, None) };
            match visit.run(minimum, ceiling)? {
                VisitStep::Done => return Ok(()),
                VisitStep::NeedFence => {
                    assert!(!fenced);
                    fenced = true;
                }
                VisitStep::NeedKey(requested) => {
                    assert!(requested < N);
                    for (index, full) in completed.iter().enumerate() {
                        if index == requested {
                            let Ok(()) = visit.attach_with(index, |key| key.attach(full)) else {
                                panic!("requested attachment refused")
                            };
                        }
                    }
                }
            }
        }
        panic!("the visit did not finish within its bound")
    }

    /// A visit of a leaf of `N` keys gives the complete-key lookup's result.
    fn visit_leaf_matches_complete_lookup<const N: usize>() {
        let raw: [([u8; SMALL], usize); N] = core::array::from_fn(|_| bytes());
        let completed: [&[u8]; N] = core::array::from_fn(|i| &raw[i].0[..raw[i].1]);
        let mut keys: [PrefixKey<'_, SMALL_P>; N] = core::array::from_fn(|i| stored(completed[i]));
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                let Ok(()) = key.attach(full) else {
                    panic!("initial attachment refused")
                };
            }
        }
        let values: [u8; N] = core::array::from_fn(|i| i as u8);
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let Ok(mut lookup) = PartialLookup::new(0usize, 0, query) else {
            panic!("partial lookup not created")
        };
        let Ok(node) = PartialNodeMut::leaf(&mut keys, &values) else {
            panic!("leaf refused")
        };
        let Ok(mut visit) = lookup.visit(0, node) else {
            panic!("visit refused")
        };
        let result = match drive_visit(&mut visit, &completed, N + 1, (None, None), false) {
            Err(error) => Err(error),
            Ok(()) => match visit.finish() {
                Ok(PartialStep::Found(key, value)) => Ok(Some((key.full().map(own), *value))),
                Ok(PartialStep::Absent) => Ok(None),
                Ok(_) => panic!("leaf finished with another step"),
                Err(error) => Err(error),
            },
        };
        let Ok(mut complete) = Lookup::new(0usize, 0, query) else {
            panic!("complete lookup not created")
        };
        let expected = Node::leaf(&completed, &values).and_then(|node| {
            complete.provide(0, &node).map(|step| match step {
                LookupStep::Found(key, value) => Some((Some(own(key)), *value)),
                LookupStep::Absent => None,
                _ => panic!("leaf returned another step"),
            })
        });
        assert!(result == expected);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn visit_of_a_one_key_leaf_matches_complete_lookup() {
        visit_leaf_matches_complete_lookup::<1>();
    }

    /// A pass resumed after attaching the requested key gives what a pass
    /// from scratch gives over the same keys. `PartialVisit::run` is this
    /// resumed pass and `provide` the pass from scratch, so with the
    /// `provide` harnesses this covers the visit path. `contents` and `query`
    /// fix the node; attachments are arbitrary, and each requested key is
    /// attached with its completion before resuming, for `rounds` steps.
    fn resumed_pass_equals_fresh_pass<'a, const N: usize>(
        lookup: &PartialLookup<'_, [u8], PrefixFence<SMALL_P>, usize>,
        keys: &mut [PrefixKey<'a, SMALL_P>; N],
        completed: &[&'a [u8]; N],
        contents: &Contents<'_, u8, usize>,
        rounds: usize,
    ) {
        let mut progress = Progress::default();
        for _ in 0..rounds {
            let resumed = lookup.pass(keys.as_slice(), contents, None, None, &mut progress);
            let fresh = lookup.pass(
                keys.as_slice(),
                contents,
                None,
                None,
                &mut Progress::default(),
            );
            let same = match (&resumed, &fresh) {
                // Every check the resumed pass skipped is still decided, so
                // both stop at the same key.
                (Ok(Outcome::NeedKey(a)), Ok(Outcome::NeedKey(b))) => a == b && *a < N,
                (Ok(Outcome::NeedFence), Ok(Outcome::NeedFence))
                | (Ok(Outcome::Absent), Ok(Outcome::Absent)) => true,
                (Ok(Outcome::Found(a)), Ok(Outcome::Found(b))) => a == b,
                (Ok(Outcome::Descend(a, x)), Ok(Outcome::Descend(b, y))) => a == b && x == y,
                (Err(a), Err(b)) => a == b,
                _ => false,
            };
            assert!(same);
            let Ok(Outcome::NeedKey(requested)) = resumed else {
                return;
            };
            for (index, (key, full)) in keys.iter_mut().zip(completed).enumerate() {
                if index == requested {
                    assert!(key.full().is_none());
                    let Ok(()) = key.attach(full) else {
                        panic!("requested attachment refused")
                    };
                }
            }
        }
        panic!("the pass did not finish within its bound");
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn resumed_leaf_pass_equals_fresh_pass() {
        let raw: [([u8; SMALL], usize); 2] = [bytes(), bytes()];
        let completed: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let mut keys: [PrefixKey<'_, SMALL_P>; 2] = [stored(completed[0]), stored(completed[1])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                let Ok(()) = key.attach(full) else {
                    panic!("initial attachment refused")
                };
            }
        }
        let values = [1u8, 2];
        let (query, query_len) = bytes::<SMALL>();
        let Ok(lookup) = PartialLookup::new(0usize, 0, &query[..query_len]) else {
            panic!("partial lookup not created")
        };
        resumed_pass_equals_fresh_pass(&lookup, &mut keys, &completed, &Contents::Leaf(&values), 4);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn resumed_branch_pass_equals_fresh_pass() {
        let raw: [([u8; SMALL], usize); 2] = [bytes(), bytes()];
        let completed: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let mut keys: [PrefixKey<'_, SMALL_P>; 2] = [stored(completed[0]), stored(completed[1])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                let Ok(()) = key.attach(full) else {
                    panic!("initial attachment refused")
                };
            }
        }
        let children = [1usize, 2];
        let (query, query_len) = bytes::<SMALL>();
        let Ok(lookup) = PartialLookup::new(0usize, 1, &query[..query_len]) else {
            panic!("partial lookup not created")
        };
        resumed_pass_equals_fresh_pass(
            &lookup,
            &mut keys,
            &completed,
            &Contents::Branch(&children),
            4,
        );
    }

    /// A visit of a one-key child, from the state after a descent, with the
    /// fence keys on request or in advance, gives the complete-key checks.
    fn visit_child_matches_complete_lookup(route: usize, eager: bool) {
        let raw: [([u8; SMALL], usize); 3] = [bytes(), bytes(), bytes()];
        let separators: [&[u8]; 2] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let leaf_full: [&[u8]; 1] = [&raw[2].0[..raw[2].1]];
        let mut leaf: [PrefixKey<'_, SMALL_P>; 1] = [stored(leaf_full[0])];
        if kani::any() {
            let Ok(()) = leaf[0].attach(leaf_full[0]) else {
                panic!("initial attachment refused")
            };
        }
        let values = [7u8];
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let fence = |index: usize| Fence {
            key: stored::<SMALL_P>(separators[index]).fence(),
            node: 0usize,
            index,
        };
        let mut path = [None; usize::BITS as usize];
        path[0] = Some(0usize);
        let mut lookup = PartialLookup {
            query,
            pending: Some(1usize),
            height: 0,
            depth: 1,
            path,
            minimum: Some(fence(route)),
            ceiling: (route == 0).then(|| fence(1)),
        };
        let minimum = Some(separators[route]);
        let ceiling = (route == 0).then_some(separators[1]);
        let Ok(node) = PartialNodeMut::leaf(&mut leaf, &values) else {
            panic!("leaf refused")
        };
        let Ok(mut visit) = lookup.visit(1, node) else {
            panic!("visit refused")
        };
        let result = match drive_visit(&mut visit, &leaf_full, 3, (minimum, ceiling), eager) {
            Err(error) => Err(error),
            Ok(()) => match visit.finish() {
                Ok(PartialStep::Found(key, value)) => Ok(Some((key.full().map(own), *value))),
                Ok(PartialStep::Absent) => Ok(None),
                Ok(_) => panic!("leaf finished with another step"),
                Err(error) => Err(error),
            },
        };
        let key = leaf_full[0];
        let expected: Result<Option<(Option<([u8; MAX], usize)>, u8)>, Error> =
            if Some(key) != minimum || ceiling.is_some_and(|ceiling| key >= ceiling) {
                Err(Error::InvalidStructure)
            } else if key == query {
                Ok(Some((Some(own(key)), values[0])))
            } else {
                Ok(None)
            };
        assert!(result == expected);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn visit_of_a_child_with_both_fences_on_request() {
        visit_child_matches_complete_lookup(0, false);
    }

    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn visit_of_a_child_with_the_minimum_in_advance() {
        visit_child_matches_complete_lookup(1, true);
    }

    /// `attach_with` accepts a change exactly when the stored parts are
    /// unchanged and a resolved key is a completion of them; a refusal
    /// refuses the visit.
    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(5)]
    fn visit_accepts_only_attachments_that_keep_the_stored_parts() {
        let (old, old_len) = bytes::<SMALL>();
        let (new, new_len) = bytes::<SMALL>();
        let (old, new) = (&old[..old_len], &new[..new_len]);
        let mut keys: [PrefixKey<'_, SMALL_P>; 1] = [stored(old)];
        let values = [1u8];
        let query: &[u8] = &[];
        let Ok(mut lookup) = PartialLookup::new(0usize, 0, query) else {
            panic!("partial lookup not created")
        };
        let attach: bool = kani::any();
        let Ok(node) = PartialNodeMut::leaf(&mut keys, &values) else {
            panic!("leaf refused")
        };
        let Ok(mut visit) = lookup.visit(0, node) else {
            panic!("visit refused")
        };
        let unresolved = old_len > SMALL_P;
        let result = visit.attach_with(0, |key| {
            let mut replacement = stored::<SMALL_P>(new);
            if attach {
                replacement.attach(new)?;
            }
            *key = replacement;
            Ok(())
        });
        let same_stored = stored::<SMALL_P>(new).fence() == stored::<SMALL_P>(old).fence();
        // A complete key (length within the prefix) is already resolved.
        assert!(result.is_ok() == (unresolved && same_stored));
        // Attaching a resolved key is refused without refusing the visit;
        // a refused change to an unresolved key refuses the visit.
        if unresolved && result.is_err() {
            assert!(visit.run(None, None) == Err(Error::InvalidStructure));
        }
        drop(visit);
        assert!(lookup.requested_node() == Some(0));
    }

    /// Refused and requesting calls leave the lookup unchanged.
    #[kani::proof]
    #[kani::stub(super::cmp_short, cmp_short_by_bytes)]
    #[kani::stub(super::cmp_prefixes, cmp_prefixes_by_bytes)]
    #[kani::unwind(6)]
    fn requests_and_refusals_keep_the_lookup_state() {
        let fulls: [([u8; SMALL], usize); 2] = [bytes(), bytes()];
        let keys: [PrefixKey<'_, SMALL_P>; 2] = [
            stored(&fulls[0].0[..fulls[0].1]),
            stored(&fulls[1].0[..fulls[1].1]),
        ];
        let children = [1usize, 2];
        let (query, query_len) = bytes::<SMALL>();
        let query = &query[..query_len];
        let node = PartialNode::<_, (), _>::branch(&keys, &children).unwrap();
        let mut lookup = PartialLookup::new(0usize, 1, query).unwrap();
        let id: usize = kani::any();
        let fence: Option<&[u8]> = if kani::any() { Some(query) } else { None };
        match lookup.provide_fenced(id, &node, fence, None) {
            Ok(PartialStep::NeedNode(child)) => {
                assert!(id == 0 && fence.is_none());
                assert!(lookup.requested_node() == Some(child));
            }
            Ok(PartialStep::Absent) => assert!(id == 0 && lookup.requested_node().is_none()),
            Ok(PartialStep::NeedKey(_)) | Err(_) => {
                assert!(lookup.requested_node() == Some(0));
                assert!(lookup.fence_origins() == (None, None));
            }
            _ => panic!("root returned another step"),
        }
    }
}
