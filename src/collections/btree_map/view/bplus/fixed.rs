//! Snapshots with fixed-width records borrowed from the caller.

use super::{Node, SMALL_NODE_KEYS};
use crate::{
    Error,
    arithmetic::bounded::{wrapping_add, wrapping_sub},
    vec::Vec,
};
use core::{borrow::Borrow, hint::select_unpredictable, mem::MaybeUninit};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
mod range;
pub use range::Range;

/// An immutable, prevalidated B+ tree with exactly `N` keys in each node.
///
/// Leaves have IDs starting at zero. Branches follow the leaves, with children
/// preceding their parent and the root last. Both slices may be empty.
/// Branch keys are the minimum keys of their children.
///
/// The ordering of the borrowed keys must stay stable while the view is in use.
/// The view borrows both pools, so values can be returned without copying them.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct FixedTree<'a, K, V, const N: usize> {
    leaves: &'a [([K; N], [V; N])],
    branches: &'a [([K; N], [usize; N])],
    height: usize,
    root: Option<usize>,
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
struct Summary<'a, K> {
    minimum: &'a K,
    maximum: &'a K,
    height: usize,
    has_parent: bool,
}

// The lookup reads the pools without checks. Its safety rests on the shape
// facts below, which `assemble` establishes and the type invariant carries to
// every `FixedTree` value. Verified directly on this source with Verus.
#[cfg(verus_keep_ghost)]
verus! {

// The height of record `id`: zero for a leaf, one more than its first child.
pub open spec fn height<K, const N: usize>(
    leaves: int,
    branches: Seq<([K; N], [usize; N])>,
    id: int,
) -> int
    decreases id,
{
    if id < leaves || id - leaves >= branches.len() || N == 0 {
        0
    } else {
        let child = branches[id - leaves].1@[0] as int;
        if 0 <= child < id { height(leaves, branches, child) + 1 } else { 0 }
    }
}

// Branches below `upto` name only earlier records, all one level lower.
pub open spec fn shaped<K, const N: usize>(
    leaves: int,
    branches: Seq<([K; N], [usize; N])>,
    upto: int,
) -> bool {
    &&& N > 0
    &&& forall|j: int, s: int| 0 <= j < upto && 0 <= s < N ==> {
        &&& (#[trigger] branches[j].1@[s]) < leaves + j
        &&& height(leaves, branches, branches[j].1@[s] as int)
            == height(leaves, branches, leaves + j) - 1
    }
}

impl<'a, K, V, const N: usize> FixedTree<'a, K, V, N> {
    #[verifier::type_invariant]
    spec fn shape(self) -> bool {
        match self.root {
            None => self.height == 0,
            Some(root) => {
                &&& root as int == self.leaves@.len() + self.branches@.len() - 1
                &&& shaped(self.leaves@.len() as int, self.branches@, self.branches@.len() as int)
                &&& height(self.leaves@.len() as int, self.branches@, root as int)
                    == self.height as int
            }
        }
    }
}

// The first `written` slots hold the summaries of records `0..written`.
spec fn summaries_ok<'a, K, const N: usize>(
    slots: Seq<MaybeUninit<Summary<'a, K>>>,
    written: int,
    leaves: int,
    branches: Seq<([K; N], [usize; N])>,
) -> bool {
    &&& 0 <= written <= slots.len()
    &&& forall|i: int| 0 <= i < written ==> {
        &&& (#[trigger] slots[i]).as_option() is Some
        &&& slots[i].as_option().unwrap().height == height(leaves, branches, i)
    }
}

proof fn height_nonnegative<K, const N: usize>(
    leaves: int,
    branches: Seq<([K; N], [usize; N])>,
    id: int,
)
    ensures height(leaves, branches, id) >= 0,
    decreases id,
{
    if id >= leaves && id - leaves < branches.len() && N > 0 {
        let child = branches[id - leaves].1@[0] as int;
        if 0 <= child < id {
            height_nonnegative(leaves, branches, child);
        }
    }
}

// A record of positive height is a branch, and each child is one level lower.
proof fn descend<K, const N: usize>(
    leaves: int,
    branches: Seq<([K; N], [usize; N])>,
    id: int,
    slot: int,
)
    requires
        shaped(leaves, branches, branches.len() as int),
        0 <= leaves,
        0 <= id < leaves + branches.len(),
        height(leaves, branches, id) > 0,
        0 <= slot < N,
    ensures
        leaves <= id,
        (branches[id - leaves].1@[slot] as int) < id,
        height(leaves, branches, branches[id - leaves].1@[slot] as int)
            == height(leaves, branches, id) - 1,
{
    reveal_with_fuel(height, 2);
    let j = id - leaves;
    assert(0 <= j);
    assert(branches[j].1@[slot] < leaves + j);
}

// A record of height zero is a leaf.
proof fn zero_height_is_leaf<K, const N: usize>(
    leaves: int,
    branches: Seq<([K; N], [usize; N])>,
    id: int,
)
    requires
        shaped(leaves, branches, branches.len() as int),
        0 <= leaves,
        0 <= id < leaves + branches.len(),
        height(leaves, branches, id) == 0,
    ensures id < leaves,
{
    if id >= leaves {
        let j = id - leaves;
        let child = branches[j].1@[0] as int;
        assert(child < leaves + j);
        height_nonnegative(leaves, branches, child);
    }
}

// Accepting branch `j` with children of height `child_height` extends the shape.
proof fn extend_shape<K, const N: usize>(
    leaves: int,
    branches: Seq<([K; N], [usize; N])>,
    j: int,
    child_height: int,
)
    requires
        N > 0,
        0 <= leaves,
        0 <= j < branches.len(),
        shaped(leaves, branches, j),
        forall|s: int| 0 <= s < N ==> {
            &&& (#[trigger] branches[j].1@[s] as int) < leaves + j
            &&& height(leaves, branches, branches[j].1@[s] as int) == child_height
        },
    ensures
        shaped(leaves, branches, j + 1),
        height(leaves, branches, leaves + j) == child_height + 1,
{
    reveal_with_fuel(height, 2);
    assert(branches[j].1@[0] < leaves + j);
    assert forall|jj: int, s: int| 0 <= jj < j + 1 && 0 <= s < N implies {
        &&& (#[trigger] branches[jj].1@[s]) < leaves + jj
        &&& height(leaves, branches, branches[jj].1@[s] as int)
            == height(leaves, branches, leaves + jj) - 1
    } by {}
}

} // verus!

impl<'a, K: Ord, V, const N: usize> FixedTree<'a, K, V, N> {
    /// Validate both pools and borrow their records.
    ///
    /// Returns an error for zero width, inconsistent structure, or failure to
    /// allocate temporary validation summaries. Those summaries are dropped
    /// before this method returns. The input buffers are only borrowed.
    pub fn try_new(
        leaves: &'a [([K; N], [V; N])],
        branches: &'a [([K; N], [usize; N])],
    ) -> Result<Self, Error> {
        if N == 0 {
            return Err(Error::InvalidStructure);
        }
        let count = leaves
            .len()
            .checked_add(branches.len())
            .ok_or(Error::InvalidStructure)?;
        if count == 0 {
            return Ok(Self::assemble(
                leaves,
                branches,
                &mut [],
                leaf_bounds,
                branch_minimum,
                fences_fail,
            )?);
        }
        let mut summaries = Vec::try_with_capacity(count)?;
        // Summaries hold only references and integers, so the vector never
        // drops them; it keeps length zero and only provides the storage.
        Self::assemble(
            leaves,
            branches,
            summaries.spare_capacity_mut(),
            leaf_bounds,
            branch_minimum,
            fences_fail,
        )
    }

    /// Find a value in the validated snapshot. Lookup does not allocate.
    ///
    /// For keys that keep the documented stable ordering, the result is the
    /// value stored with an equal key. An inconsistent `Ord` can make the
    /// result unspecified, but the lookup still only reads the borrowed pools.
    /// A tree accepted by [`try_new`](Self::try_new) never makes this method
    /// return an error.
    #[inline(always)]
    pub fn get<Q: ?Sized + Ord>(&self, query: &Q) -> Result<Option<&'a V>, Error>
    where
        K: Borrow<Q>,
    {
        Ok(self.find(
            &|key: &K| key.borrow() > query,
            &|key: &K| key.borrow() <= query,
            &|key: &K| key.borrow().cmp(query).is_gt(),
            &|key: &K| key.borrow().cmp(query).is_eq(),
            &|key: &K| key.borrow().cmp(query).is_lt(),
        ))
    }
}

// The structural core: validation and lookup over the pools, with every key
// comparison supplied by the wrappers above.
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl<'a, K, V, const N: usize> FixedTree<'a, K, V, N> {
    // Validate into `slots` and build the tree. Key checks arrive as closures;
    // every structural fact the lookup relies on is decided here.
    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires
            forall|k: &'a [K; N], v: &'a [V; N]| leaf.requires((k, v)),
            forall|k: &'a [K; N], c: &[usize; N]| branch.requires((k, c)),
            forall|l: Option<&K>, u: Option<&K>, a: &K, b: &K| fences.requires((l, u, a, b)),
    ))]
    #[inline(always)]
    fn assemble<A, B, F>(
        leaves: &'a [([K; N], [V; N])],
        branches: &'a [([K; N], [usize; N])],
        slots: &mut [MaybeUninit<Summary<'a, K>>],
        leaf: A,
        branch: B,
        fences: F,
    ) -> Result<Self, Error>
    where
        A: Copy + Fn(&'a [K; N], &'a [V; N]) -> Result<(), Error>,
        B: Copy + Fn(&'a [K; N], &[usize; N]) -> Result<&'a K, Error>,
        F: Copy + Fn(Option<&K>, Option<&K>, &K, &K) -> bool,
    {
        let count = match leaves.len().checked_add(branches.len()) {
            Some(count) => count,
            None => return Err(Error::InvalidStructure),
        };
        if count == 0 {
            return Ok(Self {
                leaves,
                branches,
                height: 0,
                root: None,
            });
        }
        if N == 0 || slots.len() < count {
            return Err(Error::InvalidStructure);
        }
        let height = summarize(leaves, branches, slots, leaf, branch, fences)?;
        Ok(Self {
            leaves,
            branches,
            height,
            root: Some(wrapping_sub(count, 1)),
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires
            forall|k: &K| above.requires((k,)),
            forall|k: &K| admits.requires((k,)),
            forall|k: &K| greater.requires((k,)),
            forall|k: &K| equal.requires((k,)),
            forall|k: &K| less.requires((k,)),
    ))]
    #[inline(always)]
    fn find<G, L, X, E, Y>(
        &self,
        above: &G,
        admits: &L,
        greater: &X,
        equal: &E,
        less: &Y,
    ) -> Option<&'a V>
    where
        G: Fn(&K) -> bool,
        L: Fn(&K) -> bool,
        X: Fn(&K) -> bool,
        E: Fn(&K) -> bool,
        Y: Fn(&K) -> bool,
    {
        #[cfg(verus_keep_ghost)]
        proof! {
            use_type_invariant(self);
        }
        let Some(mut id) = self.root else {
            return None;
        };
        let mut remaining = self.height;
        if remaining != 0 {
            #[cfg(verus_keep_ghost)]
            proof! {
                descend::<K, N>(self.leaves@.len() as int, self.branches@, id as int, 0);
            }
            // SAFETY: a root of nonzero height is a validated branch.
            let (keys, _) = unsafe {
                self.branches
                    .get_unchecked(wrapping_sub(id, self.leaves.len()))
            };
            // Separators are the minima of their children, so with a stable
            // order only the root can rule out a query below every key.
            match keys.first() {
                Some(key) => {
                    if above(key) {
                        return None;
                    }
                }
                None => return None,
            }
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                shaped(self.leaves@.len() as int, self.branches@, self.branches@.len() as int),
                0 <= id < self.leaves@.len() + self.branches@.len(),
                height(self.leaves@.len() as int, self.branches@, id as int) == remaining as int,
                forall|k: &K| admits.requires((k,)),
            decreases remaining,
        ))]
        while remaining != 0 {
            #[cfg(verus_keep_ghost)]
            proof! {
                descend::<K, N>(self.leaves@.len() as int, self.branches@, id as int, 0);
            }
            remaining = wrapping_sub(remaining, 1);
            // SAFETY: `id` has height `remaining + 1`, so it is a validated
            // branch, and its children all have height `remaining`.
            let (keys, children) = unsafe {
                self.branches
                    .get_unchecked(wrapping_sub(id, self.leaves.len()))
            };
            // Conditional selection at every width: on the pinned Linux x86-64
            // CPU the width-eight branch search took 32.50 ns/lookup against
            // 19.64 ns with selection (docs/verification/linux-bottlenecks.md).
            let slot = floor_slot(keys, admits);
            #[cfg(verus_keep_ghost)]
            proof! {
                descend::<K, N>(self.leaves@.len() as int, self.branches@, id as int, slot as int);
            }
            // SAFETY: `floor_slot` returns a slot below `N` for every order.
            id = unsafe { *children.get_unchecked(slot) };
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            zero_height_is_leaf::<K, N>(self.leaves@.len() as int, self.branches@, id as int);
        }
        // SAFETY: a node of height zero is a validated leaf.
        let (keys, values) = unsafe { self.leaves.get_unchecked(id) };
        let index = if N <= SMALL_NODE_KEYS {
            leaf_slot(keys, greater, equal)
        } else {
            leaf_rank(keys, less, equal)
        };
        match index {
            // SAFETY: both searches return only slots of existing keys, and a
            // leaf holds `N` keys and `N` values.
            Some(i) => Some(unsafe { values.get_unchecked(i) }),
            None => None,
        }
    }
}

#[inline(always)]
fn leaf_bounds<'a, K: Ord, V, const N: usize>(
    keys: &'a [K; N],
    values: &'a [V; N],
) -> Result<(), Error> {
    Node::<_, _, usize>::leaf(keys, values).map(|_| ())
}

#[inline(always)]
fn branch_minimum<'a, K: Ord, const N: usize>(
    keys: &'a [K; N],
    children: &[usize; N],
) -> Result<&'a K, Error> {
    Node::<_, (), _>::branch(keys, children)?;
    keys.first().ok_or(Error::InvalidStructure)
}

// Reject a child whose minimum is not its separator or whose maximum reaches
// the next separator.
#[inline(always)]
fn fences_fail<K: Ord>(lower: Option<&K>, upper: Option<&K>, minimum: &K, maximum: &K) -> bool {
    lower != Some(minimum) || upper.is_some_and(|bound| maximum >= bound)
}

// Select the last child whose minimum does not exceed the query, defaulting to
// child zero. The base slot only ever takes checked middle positions, so the
// result is below `N` for every order whenever `N` is nonzero.
#[cfg_attr(verus_keep_ghost, verus_spec(slot =>
    requires N > 0, forall|k: &K| admits.requires((k,)),
    ensures slot < N,
))]
#[inline(always)]
fn floor_slot<K, L: Fn(&K) -> bool, const N: usize>(keys: &[K; N], admits: &L) -> usize {
    let mut base = 0usize;
    let mut remaining = N;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant N > 0, 1 <= remaining, base + remaining <= N,
            forall|k: &K| admits.requires((k,)),
        decreases remaining,
    ))]
    while remaining > 1 {
        let half = remaining / 2;
        let Some(middle) = base.checked_add(half) else {
            break;
        };
        let Some(key) = keys.get(middle) else {
            break;
        };
        base = select_unpredictable(admits(key), middle, base);
        let Some(rest) = remaining.checked_sub(half) else {
            break;
        };
        remaining = rest;
    }
    base
}

// The position of the key equal to the query, with the iteration count and
// conditional selection of `slice::binary_search_by` in Rust 1.95. Every slot
// read is a checked middle position, so a result is below `N` for every order.
#[cfg_attr(verus_keep_ghost, verus_spec(slot =>
    requires forall|k: &K| greater.requires((k,)), forall|k: &K| equal.requires((k,)),
    ensures slot matches Some(i) ==> i < N,
))]
#[inline(always)]
fn leaf_slot<K, X: Fn(&K) -> bool, E: Fn(&K) -> bool, const N: usize>(
    keys: &[K; N],
    greater: &X,
    equal: &E,
) -> Option<usize> {
    let mut base = 0usize;
    let mut size = N;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant base + size <= N || N == 0, size <= N, base < N || N == 0,
            forall|k: &K| greater.requires((k,)), forall|k: &K| equal.requires((k,)),
        decreases size,
    ))]
    while size > 1 {
        let half = size / 2;
        let Some(middle) = base.checked_add(half) else {
            break;
        };
        let Some(key) = keys.get(middle) else {
            break;
        };
        base = select_unpredictable(greater(key), base, middle);
        let Some(rest) = size.checked_sub(half) else {
            break;
        };
        size = rest;
    }
    match keys.get(base) {
        Some(key) => {
            if equal(key) {
                Some(base)
            } else {
                None
            }
        }
        None => None,
    }
}

// The first key not below the query, found as `slice::partition_point` does in
// Rust 1.95, and its slot when that key equals the query.
#[cfg_attr(verus_keep_ghost, verus_spec(slot =>
    requires forall|k: &K| less.requires((k,)), forall|k: &K| equal.requires((k,)),
    ensures slot matches Some(i) ==> i < N,
))]
#[inline(always)]
fn leaf_rank<K, Y: Fn(&K) -> bool, E: Fn(&K) -> bool, const N: usize>(
    keys: &[K; N],
    less: &Y,
    equal: &E,
) -> Option<usize> {
    let mut base = 0usize;
    let mut size = N;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant base + size <= N || N == 0, size <= N, base < N || N == 0,
            forall|k: &K| less.requires((k,)), forall|k: &K| equal.requires((k,)),
        decreases size,
    ))]
    while size > 1 {
        let half = size / 2;
        let Some(middle) = base.checked_add(half) else {
            break;
        };
        let Some(key) = keys.get(middle) else {
            break;
        };
        base = select_unpredictable(!less(key), base, middle);
        let Some(rest) = size.checked_sub(half) else {
            break;
        };
        size = rest;
    }
    let index = match keys.get(base) {
        Some(key) => wrapping_add(base, less(key) as usize),
        None => return None,
    };
    match keys.get(index) {
        Some(key) => {
            if equal(key) {
                Some(index)
            } else {
                None
            }
        }
        None => None,
    }
}

// Write one summary per record into `slots`, children before parents, and
// return the height of the last record. Slot `i` is written before any read of
// it, and `slots` holds at least `leaves.len() + branches.len()` entries.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        N > 0,
        old(slots)@.len() >= leaves@.len() + branches@.len(),
        leaves@.len() + branches@.len() <= usize::MAX,
        forall|k: &'a [K; N], v: &'a [V; N]| leaf.requires((k, v)),
        forall|k: &'a [K; N], c: &[usize; N]| branch.requires((k, c)),
        forall|l: Option<&K>, u: Option<&K>, a: &K, b: &K| fences.requires((l, u, a, b)),
    ensures
        out matches Ok(h) ==> {
            &&& leaves@.len() + branches@.len() > 0
            &&& shaped(leaves@.len() as int, branches@, branches@.len() as int)
            &&& h as int == height(leaves@.len() as int, branches@,
                leaves@.len() + branches@.len() - 1)
        },
))]
#[inline(always)]
fn summarize<'a, K, V, A, B, F, const N: usize>(
    leaves: &'a [([K; N], [V; N])],
    branches: &'a [([K; N], [usize; N])],
    slots: &mut [MaybeUninit<Summary<'a, K>>],
    leaf: A,
    branch: B,
    fences: F,
) -> Result<usize, Error>
where
    A: Copy + Fn(&'a [K; N], &'a [V; N]) -> Result<(), Error>,
    B: Copy + Fn(&'a [K; N], &[usize; N]) -> Result<&'a K, Error>,
    F: Copy + Fn(Option<&K>, Option<&K>, &K, &K) -> bool,
{
    let mut written = 0usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(iter =>
        invariant
            N > 0,
            written == iter.index@,
            written <= leaves@.len(),
            slots@.len() >= leaves@.len() + branches@.len(),
            leaves@.len() + branches@.len() <= usize::MAX,
            summaries_ok(slots@, written as int, leaves@.len() as int, branches@),
            forall|k: &'a [K; N], v: &'a [V; N]| leaf.requires((k, v)),
    ))]
    for (keys, values) in leaves.iter() {
        leaf(keys, values)?;
        let (Some(minimum), Some(maximum)) = (keys.first(), keys.last()) else {
            return Err(Error::InvalidStructure);
        };
        #[cfg(verus_keep_ghost)]
        let before = verus_exec_expr!(Ghost(slots@));
        // SAFETY: written < leaves.len() <= slots.len().
        unsafe { slots.get_unchecked_mut(written) }.write(Summary {
            minimum,
            maximum,
            height: 0,
            has_parent: false,
        });
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall|i: int| 0 <= i < written + 1 implies {
                &&& (#[trigger] slots@[i]).as_option() is Some
                &&& slots@[i].as_option().unwrap().height
                    == height(leaves@.len() as int, branches@, i)
            } by {
                if i != written { assert(slots@[i] == before@[i]); }
            }
        }
        written = wrapping_add(written, 1);
    }
    let mut j = 0usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            N > 0,
            written == leaves@.len() + j,
            j <= branches@.len(),
            slots@.len() >= leaves@.len() + branches@.len(),
            leaves@.len() + branches@.len() <= usize::MAX,
            summaries_ok(slots@, written as int, leaves@.len() as int, branches@),
            shaped(leaves@.len() as int, branches@, j as int),
            forall|k: &'a [K; N], c: &[usize; N]| branch.requires((k, c)),
            forall|l: Option<&K>, u: Option<&K>, a: &K, b: &K| fences.requires((l, u, a, b)),
        decreases branches@.len() - j,
    ))]
    while j < branches.len() {
        // SAFETY: j < branches.len().
        let (keys, children) = unsafe { branches.get_unchecked(j) };
        // SAFETY: written < leaves.len() + branches.len() <= slots.len(), so
        // the written prefix and the next slot are disjoint parts of `slots`.
        let (prefix, next) = unsafe { slots.split_at_mut_unchecked(written) };
        // SAFETY: the suffix starts at `written` and is not empty.
        summarize_branch(
            keys,
            children,
            prefix,
            unsafe { next.get_unchecked_mut(0) },
            branch,
            fences,
        )?;
        written = wrapping_add(written, 1);
        j = wrapping_add(j, 1);
    }
    let Some(root) = written.checked_sub(1) else {
        return Err(Error::InvalidStructure);
    };
    // Every earlier node must have exactly one parent. A backward edge cannot
    // name the last record, so that record is the unique root.
    // SAFETY: root < written <= slots.len().
    let earlier = unsafe { slots.get_unchecked(..root) };
    #[cfg_attr(verus_keep_ghost, verus_spec(iter =>
        invariant
            iter.seq().len() == earlier@.len(),
            forall|i: int| 0 <= i < iter.seq().len() ==> *(#[trigger] iter.seq()[i]) == earlier@[i],
            forall|i: int| 0 <= i < earlier@.len() ==> (#[trigger] earlier@[i]).as_option() is Some,
    ))]
    for slot in earlier.iter() {
        // SAFETY: every slot below `written` is written.
        if !unsafe { slot.assume_init_ref() }.has_parent {
            return Err(Error::InvalidStructure);
        }
    }
    // SAFETY: root < written.
    Ok(unsafe { slots.get_unchecked(root).assume_init_ref() }.height)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        N > 0,
        forall|i: int| 0 <= i < old(prefix)@.len() ==> (#[trigger] old(prefix)@[i]).as_option() is Some,
        forall|k: &'a [K; N], c: &[usize; N]| branch.requires((k, c)),
        forall|l: Option<&K>, u: Option<&K>, a: &K, b: &K| fences.requires((l, u, a, b)),
    ensures
        final(prefix)@.len() == old(prefix)@.len(),
        forall|i: int| 0 <= i < old(prefix)@.len() ==> {
            &&& (#[trigger] final(prefix)@[i]).as_option() is Some
            &&& final(prefix)@[i].as_option().unwrap().height
                == old(prefix)@[i].as_option().unwrap().height
        },
        out is Ok ==> {
            &&& final(next).as_option() is Some
            &&& forall|s: int| 0 <= s < N ==> {
                &&& (#[trigger] children@[s] as int) < old(prefix)@.len()
                &&& old(prefix)@[children@[s] as int].as_option().unwrap().height
                    == final(next).as_option().unwrap().height - 1
            }
        },
))]
#[inline(never)]
fn summarize_branch<'a, K, B, F, const N: usize>(
    keys: &'a [K; N],
    children: &[usize; N],
    prefix: &mut [MaybeUninit<Summary<'a, K>>],
    next: &mut MaybeUninit<Summary<'a, K>>,
    branch: B,
    fences: F,
) -> Result<(), Error>
where
    B: Copy + Fn(&'a [K; N], &[usize; N]) -> Result<&'a K, Error>,
    F: Copy + Fn(Option<&K>, Option<&K>, &K, &K) -> bool,
{
    let minimum = branch(keys, children)?;
    let mut height = None;
    let mut maximum = minimum;
    let mut s = 0usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            N > 0,
            s <= N,
            prefix@.len() == old(prefix)@.len(),
            forall|i: int| 0 <= i < old(prefix)@.len() ==> {
                &&& (#[trigger] prefix@[i]).as_option() is Some
                &&& prefix@[i].as_option().unwrap().height
                    == old(prefix)@[i].as_option().unwrap().height
            },
            (s == 0) == (height is None),
            height matches Some(x) ==> forall|t: int| 0 <= t < s ==> {
                &&& (#[trigger] children@[t] as int) < old(prefix)@.len()
                &&& old(prefix)@[children@[t] as int].as_option().unwrap().height == x
            },
            forall|l: Option<&K>, u: Option<&K>, a: &K, b: &K| fences.requires((l, u, a, b)),
        decreases N - s,
    ))]
    while s < N {
        // SAFETY: s < N.
        let id = unsafe { *children.get_unchecked(s) };
        if id >= prefix.len() {
            return Err(Error::InvalidStructure);
        }
        // SAFETY: id is below the length of the written prefix.
        let child = unsafe { prefix.get_unchecked_mut(id).assume_init_mut() };
        if child.has_parent
            || match height {
                Some(expected) => expected != child.height,
                None => false,
            }
            || fences(
                keys.get(s),
                keys.get(wrapping_add(s, 1)),
                child.minimum,
                child.maximum,
            )
        {
            return Err(Error::InvalidStructure);
        }
        child.has_parent = true;
        height = Some(child.height);
        maximum = child.maximum;
        s = wrapping_add(s, 1);
    }
    let height = match height {
        Some(value) => match value.checked_add(1) {
            Some(height) => height,
            None => return Err(Error::InvalidStructure),
        },
        None => return Err(Error::InvalidStructure),
    };
    next.write(Summary {
        minimum,
        maximum,
        height,
        has_parent: false,
    });
    Ok(())
}

#[cfg(kani)]
mod proofs {
    use super::*;
    use core::cmp::Ordering;

    // Every comparison returns an arbitrary result, so these harnesses cover
    // every order, including ones that break the stable-`Ord` contract. The
    // unused byte keeps the key sized: CBMC 6.11 aborts on arrays of two or
    // more zero-sized elements.
    struct Chaos(#[allow(dead_code)] u8);

    impl PartialEq for Chaos {
        fn eq(&self, _: &Self) -> bool {
            kani::any()
        }
    }

    impl Eq for Chaos {}

    impl PartialOrd for Chaos {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Chaos {
        fn cmp(&self, _: &Self) -> Ordering {
            match kani::any::<u8>() {
                0 => Ordering::Less,
                1 => Ordering::Equal,
                _ => Ordering::Greater,
            }
        }
    }

    fn chaos<const N: usize>() -> [Chaos; N] {
        [const { Chaos(0) }; N]
    }

    // Accept pools of L leaves and B branches with arbitrary child IDs and
    // check that every accepted tree answers from its own value pool. The
    // record counts stay concrete so the validation summaries have a fixed
    // allocation size; each harness instantiates the shapes of every height.
    fn lookup_reads_only_pool_values<
        const N: usize,
        const L: usize,
        const B: usize,
        const H: usize,
    >() {
        let leaves: [([Chaos; N], [u8; N]); L] = core::array::from_fn(|_| (chaos(), kani::any()));
        let branches: [([Chaos; N], [usize; N]); B] =
            core::array::from_fn(|_| (chaos(), kani::any()));
        // An ID at or past the record count is never in the validated prefix,
        // so try_new rejects it (fixed_tree_rejects_invalid_child_ids covers
        // every machine word). Accepted trees are unchanged by this bound.
        kani::assume(
            branches
                .iter()
                .all(|(_, ids)| ids.iter().all(|&id| id < L + B)),
        );
        let Ok(tree) = FixedTree::try_new(&leaves, &branches) else {
            return;
        };
        kani::cover!(tree.height == H);
        let found = tree.get(&Chaos(0));
        assert!(found.is_ok());
        if let Ok(Some(value)) = found {
            assert!(
                leaves
                    .iter()
                    .any(|(_, values)| values.iter().any(|v| core::ptr::eq(v, value)))
            );
        }
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn fixed_get_reads_only_pool_values_for_any_order_width_one() {
        // Width one admits single-child chains; height four is the longest.
        lookup_reads_only_pool_values::<1, 1, 0, 0>();
        lookup_reads_only_pool_values::<1, 1, 1, 1>();
        lookup_reads_only_pool_values::<1, 1, 4, 4>();
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn fixed_get_reads_only_pool_values_for_any_order_width_two() {
        // Two leaves under a root, and four leaves under two branches.
        lookup_reads_only_pool_values::<2, 2, 1, 1>();
        lookup_reads_only_pool_values::<2, 4, 3, 2>();
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn fixed_get_reads_only_pool_values_for_any_order_width_three() {
        lookup_reads_only_pool_values::<3, 1, 0, 0>();
        lookup_reads_only_pool_values::<3, 3, 1, 1>();
    }

    fn floor_slot_below_width<const N: usize>() {
        assert!(floor_slot(&chaos::<N>(), &|key: &Chaos| *key <= Chaos(0)) < N);
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn floor_slot_stays_below_every_width_for_any_order() {
        floor_slot_below_width::<1>();
        floor_slot_below_width::<2>();
        floor_slot_below_width::<3>();
        floor_slot_below_width::<4>();
        floor_slot_below_width::<5>();
        floor_slot_below_width::<7>();
        floor_slot_below_width::<8>();
        floor_slot_below_width::<9>();
        floor_slot_below_width::<16>();
        floor_slot_below_width::<17>();
        floor_slot_below_width::<32>();
        floor_slot_below_width::<33>();
        floor_slot_below_width::<64>();
    }

    fn leaf_search_reads_only_pool_values<const N: usize>() {
        let leaves = [(chaos::<N>(), kani::any::<[u8; N]>())];
        let Ok(tree) = FixedTree::try_new(&leaves, &[]) else {
            return;
        };
        let found = tree.get(&Chaos(0));
        assert!(found.is_ok());
        if let Ok(Some(value)) = found {
            assert!(leaves[0].1.iter().any(|v| core::ptr::eq(v, value)));
        }
    }

    #[kani::proof]
    #[kani::unwind(18)]
    fn fixed_leaf_search_reads_only_pool_values_for_any_order() {
        // Widths up to eight use the binary search, wider leaves leaf_index.
        leaf_search_reads_only_pool_values::<7>();
        leaf_search_reads_only_pool_values::<8>();
        leaf_search_reads_only_pool_values::<9>();
        leaf_search_reads_only_pool_values::<16>();
    }

    // The checked lookup before the unchecked indexing, kept as the reference.
    fn checked_get<'a, const N: usize>(
        tree: &FixedTree<'a, u8, u8, N>,
        query: &u8,
    ) -> Result<Option<&'a u8>, Error> {
        let Some(mut id) = tree.root else {
            return Ok(None);
        };
        let mut remaining = tree.height;
        while let Some(next_height) = remaining.checked_sub(1) {
            remaining = next_height;
            let branch_id = id
                .checked_sub(tree.leaves.len())
                .ok_or(Error::InvalidStructure)?;
            let (keys, children) = tree
                .branches
                .get(branch_id)
                .ok_or(Error::InvalidStructure)?;
            let next = super::super::child_index::<true, _, _>(keys, query)?;
            let Some(next) = next else { return Ok(None) };
            id = *children.get(next).ok_or(Error::InvalidStructure)?;
        }
        let (keys, values) = tree.leaves.get(id).ok_or(Error::InvalidStructure)?;
        let index = if N <= SMALL_NODE_KEYS {
            keys.binary_search_by(|key| key.cmp(query)).ok()
        } else {
            super::super::leaf_index(keys, query)
        };
        index
            .map(|i| values.get(i).ok_or(Error::InvalidStructure))
            .transpose()
    }

    fn matches_checked_and_linear_lookup<const L: usize, const B: usize, const H: usize>(
        branches: [([u8; 2], [usize; 2]); B],
    ) {
        let leaves: [([u8; 2], [u8; 2]); L] = kani::any();
        kani::assume(
            branches
                .iter()
                .all(|(_, ids)| ids.iter().all(|&id| id < L + B)),
        );
        let Ok(tree) = FixedTree::try_new(&leaves, &branches) else {
            return;
        };
        kani::cover!(tree.height == H);
        let query = kani::any();
        let expected = leaves.iter().find_map(|(keys, values)| {
            keys.iter()
                .zip(values)
                .find_map(|(key, value)| (*key == query).then_some(value))
        });
        let found = tree.get(&query);
        assert!(found == Ok(expected));
        assert!(found == checked_get(&tree, &query));
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn fixed_get_matches_checked_and_linear_lookup_at_height_one() {
        // Arbitrary keys, values and queries over valid child IDs; the
        // memory-safety harnesses cover arbitrary child IDs. A single-leaf
        // instance exhausted CBMC memory; height zero only skips the descent
        // and shares the leaf step checked here.
        let mut root: [([u8; 2], [usize; 2]); 1] = kani::any();
        root[0].1 = [0, 1];
        matches_checked_and_linear_lookup::<2, 1, 1>(root);
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn fixed_get_matches_checked_and_linear_lookup_with_scrambled_ids() {
        let mut branches: [([u8; 2], [usize; 2]); 3] = kani::any();
        branches[0].1 = [3, 1];
        branches[1].1 = [0, 2];
        branches[2].1 = [5, 4];
        matches_checked_and_linear_lookup::<4, 3, 2>(branches);
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn fixed_get_matches_checked_and_linear_lookup_at_height_two() {
        // Arbitrary keys, values and queries in ID order.
        let mut branches: [([u8; 2], [usize; 2]); 3] = kani::any();
        branches[0].1 = [0, 1];
        branches[1].1 = [2, 3];
        branches[2].1 = [4, 5];
        matches_checked_and_linear_lookup::<4, 3, 2>(branches);
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn fixed_tree_matches_linear_lookup() {
        let leaves = [([2u8, 5], [20, 50]), ([7, 12], [70, 120])];
        let branches = [([2, 7], [0, 1])];
        let query = kani::any();
        let tree = FixedTree::try_new(&leaves, &branches).unwrap();
        let expected = match query {
            2 => Some(&20),
            5 => Some(&50),
            7 => Some(&70),
            12 => Some(&120),
            _ => None,
        };
        assert!(tree.get(&query) == Ok(expected));
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn fixed_tree_rejects_invalid_child_ids() {
        let leaves = [([2u8, 5], [20, 50]), ([7, 12], [70, 120])];
        let first: usize = kani::any();
        let second: usize = kani::any();
        let branches = [([2, 7], [first, second])];
        assert!(FixedTree::try_new(&leaves, &branches).is_ok() == (first == 0 && second == 1));
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn fixed_branch_checks_fences_heights_and_unique_parent() {
        let minima = [2u8, 7];
        let first_maximum = kani::any::<u8>();
        let right_minimum = kani::any::<u8>();
        let left_height = kani::any::<usize>();
        let right_height = kani::any::<usize>();
        let used = kani::any();
        let mut prefix = [
            MaybeUninit::new(Summary {
                minimum: &2,
                maximum: &first_maximum,
                height: left_height,
                has_parent: used,
            }),
            MaybeUninit::new(Summary {
                minimum: &right_minimum,
                maximum: &12,
                height: right_height,
                has_parent: false,
            }),
        ];
        let expected = !used
            && first_maximum < 7
            && right_minimum == 7
            && left_height == right_height
            && left_height != usize::MAX;
        let mut next = MaybeUninit::uninit();
        let result = summarize_branch(
            &minima,
            &[0, 1],
            &mut prefix,
            &mut next,
            branch_minimum,
            fences_fail,
        );
        assert!(result.is_ok() == expected);
        if result.is_ok() {
            // SAFETY: a successful check writes the parent summary.
            let summary = unsafe { next.assume_init() };
            assert!(*summary.minimum == 2 && *summary.maximum == 12);
            assert!(summary.height == left_height + 1 && !summary.has_parent);
            // SAFETY: both children were initialized above.
            assert!(
                prefix
                    .iter()
                    .all(|child| unsafe { child.assume_init_ref() }.has_parent)
            );
        }
    }
}
