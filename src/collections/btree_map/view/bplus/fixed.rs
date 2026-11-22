//! Snapshots with fixed-width records borrowed from the caller.

use super::{Node, SMALL_NODE_KEYS, leaf_index};
use crate::{Error, vec::Vec};
use core::{borrow::Borrow, hint::select_unpredictable};
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
pub struct FixedTree<'a, K, V, const N: usize> {
    leaves: &'a [([K; N], [V; N])],
    branches: &'a [([K; N], [usize; N])],
    height: usize,
    root: Option<usize>,
}

struct Summary<'a, K> {
    minimum: &'a K,
    maximum: &'a K,
    height: usize,
    has_parent: bool,
}

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
        let Some(root) = count.checked_sub(1) else {
            return Ok(Self {
                leaves,
                branches,
                height: 0,
                root: None,
            });
        };

        let mut summaries = Vec::try_with_capacity(count)?;
        for (keys, values) in leaves {
            Node::<_, _, usize>::leaf(keys, values)?;
            let summary = Summary {
                minimum: keys.first().ok_or(Error::InvalidStructure)?,
                maximum: keys.last().ok_or(Error::InvalidStructure)?,
                height: 0,
                has_parent: false,
            };
            summaries
                .try_push(summary)
                .map_err(|rejected| rejected.error)?;
        }
        for (keys, children) in branches {
            let summary = summarize_branch(keys, children, summaries.as_mut_slice())?;
            summaries
                .try_push(summary)
                .map_err(|rejected| rejected.error)?;
        }
        // Every earlier node must have exactly one parent. A backward edge
        // cannot name the last record, so that record is the unique root.
        let non_root = summaries
            .as_slice()
            .get(..root)
            .ok_or(Error::InvalidStructure)?;
        if non_root.iter().any(|node| !node.has_parent) {
            return Err(Error::InvalidStructure);
        }
        let height = summaries.last().ok_or(Error::InvalidStructure)?.height;
        Ok(Self {
            leaves,
            branches,
            height,
            root: Some(root),
        })
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
        let Some(mut id) = self.root else {
            return Ok(None);
        };
        let mut remaining = self.height;
        if remaining != 0 {
            // SAFETY: a root of nonzero height is a validated branch.
            let (keys, _) = unsafe { self.branch_unchecked(id) };
            // Separators are the minima of their children, so with a stable
            // order only the root can rule out a query below every key.
            if keys.first().is_none_or(|key| key.borrow() > query) {
                return Ok(None);
            }
        }
        while let Some(next_height) = remaining.checked_sub(1) {
            remaining = next_height;
            // SAFETY: `id` has height `remaining + 1`, so it is a validated
            // branch, and its children all have height `remaining`.
            let (keys, children) = unsafe { self.branch_unchecked(id) };
            // Conditional selection at every width: on the pinned Linux x86-64
            // CPU the width-eight branch search took 32.50 ns/lookup against
            // 19.64 ns with selection (docs/verification/linux-bottlenecks.md).
            let slot = floor_slot(keys, query);
            // SAFETY: `floor_slot` returns a slot below `N` for every order.
            id = unsafe { *children.as_ptr().add(slot) };
        }
        // SAFETY: a node of height zero is a validated leaf.
        let (keys, values) = unsafe { &*self.leaves.as_ptr().add(id) };
        let index = if N <= SMALL_NODE_KEYS {
            keys.binary_search_by(|key| key.borrow().cmp(query)).ok()
        } else {
            leaf_index(keys, query)
        };
        // SAFETY: both searches return only indices of existing keys, and a
        // leaf holds `N` keys and `N` values.
        Ok(index.map(|i| unsafe { &*values.as_ptr().add(i) }))
    }

    /// Borrow the branch record with node ID `id`.
    ///
    /// # Safety
    ///
    /// `id` must be the root of a tree of nonzero height, or a child reached
    /// from a branch of height at least two. `try_new` guarantees that such a
    /// node lies after the leaves and before the end of the branch pool.
    ///
    /// The unchecked reads use `as_ptr().add()` rather than `get_unchecked`:
    /// Kani 0.68 with CBMC 6.11 reported a wrong value for a pointer that
    /// `get_unchecked` derived from a pool returned by `try_new`, while Miri and
    /// native runs read the right record. Both forms perform the same loads;
    /// the x86-64 release lookups have the same instruction counts, with
    /// identical code at width eight and a different block order at widths
    /// 16 and 32.
    ///
    /// The offset uses the library's bounded subtraction, whose non-wrapping
    /// contract is registered for this call in the wrapping-memory coverage.
    #[inline(always)]
    unsafe fn branch_unchecked(&self, id: usize) -> &'a ([K; N], [usize; N]) {
        // SAFETY: the caller guarantees leaves.len() <= id < leaves.len() +
        // branches.len(), so the subtraction does not wrap and the record lies
        // inside the borrowed pool.
        unsafe {
            &*self
                .branches
                .as_ptr()
                .add(crate::arithmetic::bounded::wrapping_sub(
                    id,
                    self.leaves.len(),
                ))
        }
    }
}

// Select the last child whose minimum does not exceed the query, defaulting to
// child zero. The base slot only ever takes checked middle positions, so the
// result is below `N` for every order whenever `N` is nonzero.
#[inline(always)]
fn floor_slot<K: Borrow<Q>, Q: ?Sized + Ord, const N: usize>(keys: &[K; N], query: &Q) -> usize {
    let mut base = 0usize;
    let mut remaining = N;
    while remaining > 1 {
        let half = remaining / 2;
        let Some(middle) = base.checked_add(half) else {
            break;
        };
        let Some(key) = keys.get(middle) else {
            break;
        };
        base = select_unpredictable(key.borrow() <= query, middle, base);
        let Some(rest) = remaining.checked_sub(half) else {
            break;
        };
        remaining = rest;
    }
    base
}

fn summarize_branch<'a, K: Ord, const N: usize>(
    keys: &'a [K; N],
    children: &[usize; N],
    prefix: &mut [Summary<'a, K>],
) -> Result<Summary<'a, K>, Error> {
    Node::<_, (), _>::branch(keys, children)?;
    let minimum = keys.first().ok_or(Error::InvalidStructure)?;
    let mut bounds = keys.iter();
    let mut lower = bounds.next();
    let mut height = None;
    let mut maximum = minimum;
    for &id in children {
        let child = prefix.get_mut(id).ok_or(Error::InvalidStructure)?;
        let upper = bounds.next();
        if child.has_parent
            || height.is_some_and(|expected| expected != child.height)
            || lower != Some(child.minimum)
            || upper.is_some_and(|bound| child.maximum >= bound)
        {
            return Err(Error::InvalidStructure);
        }
        child.has_parent = true;
        height = Some(child.height);
        maximum = child.maximum;
        lower = upper;
    }
    let height = height
        .and_then(|value| value.checked_add(1))
        .ok_or(Error::InvalidStructure)?;
    Ok(Summary {
        minimum,
        maximum,
        height,
        has_parent: false,
    })
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
        assert!(floor_slot(&chaos::<N>(), &Chaos(0)) < N);
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
            leaf_index(keys, query)
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
            Summary {
                minimum: &2,
                maximum: &first_maximum,
                height: left_height,
                has_parent: used,
            },
            Summary {
                minimum: &right_minimum,
                maximum: &12,
                height: right_height,
                has_parent: false,
            },
        ];
        let expected = !used
            && first_maximum < 7
            && right_minimum == 7
            && left_height == right_height
            && left_height != usize::MAX;
        let result = summarize_branch(&minima, &[0, 1], &mut prefix);
        assert!(result.is_ok() == expected);
        if let Ok(summary) = result {
            assert!(*summary.minimum == 2 && *summary.maximum == 12);
            assert!(summary.height == left_height + 1 && !summary.has_parent);
            assert!(prefix.iter().all(|child| child.has_parent));
        }
    }
}
