//! Snapshots with fixed-width records borrowed from the caller.

use super::{Node, SMALL_NODE_KEYS, child_index, leaf_index};
use crate::{Error, vec::Vec};
use core::borrow::Borrow;
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
    #[inline(always)]
    pub fn get<Q: ?Sized + Ord>(&self, query: &Q) -> Result<Option<&'a V>, Error>
    where
        K: Borrow<Q>,
    {
        let Some(mut id) = self.root else {
            return Ok(None);
        };
        let mut remaining = self.height;
        while let Some(next_height) = remaining.checked_sub(1) {
            remaining = next_height;
            let branch_id = id
                .checked_sub(self.leaves.len())
                .ok_or(Error::InvalidStructure)?;
            let (keys, children) = self
                .branches
                .get(branch_id)
                .ok_or(Error::InvalidStructure)?;
            // Conditional selection at every width: on the pinned Linux x86-64
            // CPU the width-eight branch search took 32.50 ns/lookup against
            // 19.64 ns with selection (docs/verification/linux-bottlenecks.md).
            let next = child_index::<true, _, _>(keys, query)?;
            let Some(next) = next else { return Ok(None) };
            id = *children.get(next).ok_or(Error::InvalidStructure)?;
        }
        let (keys, values) = self.leaves.get(id).ok_or(Error::InvalidStructure)?;
        let index = if N <= SMALL_NODE_KEYS {
            keys.binary_search_by(|key| key.borrow().cmp(query)).ok()
        } else {
            leaf_index(keys, query)
        };
        index
            .map(|i| values.get(i).ok_or(Error::InvalidStructure))
            .transpose()
    }
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
