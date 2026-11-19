//! Ordered scans of an already validated snapshot.

use super::FixedTree;
use crate::{Error, TryIterator};
use core::{borrow::Borrow, iter::Zip, ops::Bound, slice::Iter};

/// Forward range cursor over a validated [`FixedTree`].
///
/// Each item borrows a key and value from its original leaf. The cursor owns
/// its traversal state and does not allocate. Bounds and keys must keep their
/// ordering while the cursor is used.
pub struct Range<'t, 'a, 'q, K, V, Q: ?Sized, const N: usize> {
    tree: &'t FixedTree<'a, K, V, N>,
    end: Bound<&'q Q>,
    // Leaf rank in separator order, independent of the caller's node IDs.
    rank: usize,
    parent: Option<&'a [usize; N]>,
    remaining: Zip<Iter<'a, K>, Iter<'a, V>>,
    more_leaves: bool,
}

impl<'a, K: Ord, V, const N: usize> FixedTree<'a, K, V, N> {
    /// Open an ordered cursor with inclusive, exclusive, or open endpoints.
    ///
    /// Returns [`Error::Bounds`] if the start exceeds the end or both exclude
    /// the same key. An empty tree or range yields no items.
    #[inline(always)]
    pub fn range<'q, Q: ?Sized + Ord>(
        &self,
        start: Bound<&'q Q>,
        end: Bound<&'q Q>,
    ) -> Result<Range<'_, 'a, 'q, K, V, Q, N>, Error>
    where
        K: Borrow<Q>,
    {
        match (start, end) {
            (Bound::Excluded(lower), Bound::Excluded(upper)) if lower >= upper => {
                return Err(Error::Bounds);
            }
            (
                Bound::Included(lower) | Bound::Excluded(lower),
                Bound::Included(upper) | Bound::Excluded(upper),
            ) if lower > upper => {
                return Err(Error::Bounds);
            }
            _ => {}
        }

        let mut cursor = Range {
            tree: self,
            end,
            rank: 0,
            parent: None,
            remaining: [].iter().zip([].iter()),
            more_leaves: false,
        };
        if let Some(root) = self.root {
            let mut id = root;
            let mut height = self.height;
            while let Some(next) = height.checked_sub(1) {
                height = next;
                let (keys, children) = branch(self, id)?;
                let slot = match start {
                    Bound::Included(query) | Bound::Excluded(query) => {
                        prefix_len(keys, query, true).saturating_sub(1)
                    }
                    Bound::Unbounded => 0,
                };
                cursor.rank = cursor
                    .rank
                    .checked_mul(N)
                    .and_then(|rank| rank.checked_add(slot))
                    .ok_or(Error::InvalidStructure)?;
                cursor.parent = Some(children);
                id = *children.get(slot).ok_or(Error::InvalidStructure)?;
            }
            cursor.open_leaf(id, start)?;
        }
        Ok(cursor)
    }
}

// Count the keys below the bound, optionally including a matching key.
#[inline]
fn prefix_len<K: Borrow<Q>, Q: ?Sized + Ord>(keys: &[K], query: &Q, inclusive: bool) -> usize {
    keys.partition_point(|key| match key.borrow().cmp(query) {
        core::cmp::Ordering::Less => true,
        core::cmp::Ordering::Equal => inclusive,
        core::cmp::Ordering::Greater => false,
    })
}

#[inline]
fn branch<'a, K, V, const N: usize>(
    tree: &FixedTree<'a, K, V, N>,
    id: usize,
) -> Result<&'a ([K; N], [usize; N]), Error> {
    let index = id
        .checked_sub(tree.leaves.len())
        .ok_or(Error::InvalidStructure)?;
    tree.branches.get(index).ok_or(Error::InvalidStructure)
}

impl<'a, K: Borrow<Q>, V, Q: ?Sized + Ord, const N: usize> Range<'_, 'a, '_, K, V, Q, N> {
    #[inline(always)]
    fn open_leaf(&mut self, id: usize, start: Bound<&Q>) -> Result<(), Error> {
        let leaf = self.tree.leaves.get(id).ok_or(Error::InvalidStructure)?;
        let keys = &leaf.0;
        let first = match start {
            Bound::Included(query) => prefix_len(keys, query, false),
            Bound::Excluded(query) => prefix_len(keys, query, true),
            Bound::Unbounded => 0,
        };
        let limit = match self.end {
            Bound::Included(query) if keys.last().is_some_and(|k| k.borrow() > query) => {
                prefix_len(keys, query, true)
            }
            Bound::Excluded(query) if keys.last().is_some_and(|k| k.borrow() >= query) => {
                prefix_len(keys, query, false)
            }
            _ => N,
        };
        let keys = keys.get(first..limit).ok_or(Error::InvalidStructure)?;
        let values = leaf.1.get(first..limit).ok_or(Error::InvalidStructure)?;
        self.remaining = keys.iter().zip(values.iter());
        self.more_leaves = limit == N;
        Ok(())
    }

    // Most leaf changes stay within the cached parent. Changing parents
    // decodes the logical rank from the root, in separator order.
    #[inline(always)]
    fn advance_leaf(&mut self) -> Result<(), Error> {
        self.more_leaves = false;
        let rank = self.rank.checked_add(1).ok_or(Error::InvalidStructure)?;
        let count = self.tree.leaves.len();
        if rank >= count {
            return Ok(());
        }
        self.rank = rank;
        let slot = rank.checked_rem(N).ok_or(Error::InvalidStructure)?;
        if slot != 0 {
            let children = self.parent.ok_or(Error::InvalidStructure)?;
            return self.open_leaf(
                *children.get(slot).ok_or(Error::InvalidStructure)?,
                Bound::Unbounded,
            );
        }
        let (id, parent) = next_parent(self.tree, rank)?;
        self.parent = Some(parent);
        self.open_leaf(id, Bound::Unbounded)
    }
}

#[cold]
#[inline(never)]
fn next_parent<'a, K, V, const N: usize>(
    tree: &FixedTree<'a, K, V, N>,
    rank: usize,
) -> Result<(usize, &'a [usize; N]), Error> {
    let mut id = tree.root.ok_or(Error::InvalidStructure)?;
    let mut relative = rank;
    let mut width = tree.leaves.len();
    let mut height = tree.height;
    while let Some(next) = height.checked_sub(1) {
        height = next;
        width = width.checked_div(N).ok_or(Error::InvalidStructure)?;
        let slot = relative.checked_div(width).ok_or(Error::InvalidStructure)?;
        relative = relative.checked_rem(width).ok_or(Error::InvalidStructure)?;
        let (_, children) = branch(tree, id)?;
        id = *children.get(slot).ok_or(Error::InvalidStructure)?;
        if height == 0 {
            return Ok((id, children));
        }
    }
    Err(Error::InvalidStructure)
}

impl<'a, K: Borrow<Q> + 'a, V: 'a, Q: ?Sized + Ord, const N: usize> TryIterator
    for Range<'_, 'a, '_, K, V, Q, N>
{
    type Item = (&'a K, &'a V);
    type Error = Error;

    #[inline(always)]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Error> {
        if let Some(item) = self.remaining.next() {
            return Ok(Some(item));
        }
        if !self.more_leaves {
            return Ok(None);
        }
        self.advance_leaf()?;
        Ok(self.remaining.next())
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(5)]
    fn range_first_item_matches_sorted_keys_with_arbitrary_byte_bounds() {
        let leaves = [([5_u8, 7], [50, 70]), ([2, 3], [20, 30])];
        let branches = [([2, 5], [1, 0])];
        let tree = FixedTree::try_new(&leaves, &branches).unwrap();
        let start: u8 = kani::any();
        let end: u8 = kani::any();
        let cursor = tree.range(Bound::Excluded(&start), Bound::Included(&end));
        if start > end {
            assert!(matches!(cursor, Err(Error::Bounds)));
            return;
        }
        let expected = [2u8, 3, 5, 7]
            .into_iter()
            .find(|&key| start < key && key <= end);
        let mut cursor = cursor.unwrap();
        assert_eq!(
            cursor
                .try_next()
                .unwrap()
                .map(|(key, value)| (*key, *value)),
            expected.map(|key| (key, u16::from(key) * 10))
        );
    }
}
