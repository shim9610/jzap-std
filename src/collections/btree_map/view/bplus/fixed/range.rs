//! Ordered scans of an already validated snapshot.

use super::FixedTree;
use crate::{Error, TryIterator};
use core::{borrow::Borrow, ops::Bound};

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
    leaf: Option<&'a ([K; N], [V; N])>,
    index: usize,
    limit: usize,
}

impl<'a, K: Ord, V, const N: usize> FixedTree<'a, K, V, N> {
    /// Open an ordered cursor with inclusive, exclusive, or open endpoints.
    ///
    /// Returns [`Error::Bounds`] if the start exceeds the end or both exclude
    /// the same key. An empty tree or range yields no items.
    #[inline]
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
            leaf: None,
            index: 0,
            limit: 0,
        };
        if let Some(root) = self.root {
            let mut id = root;
            let mut height = self.height;
            while let Some(next) = height.checked_sub(1) {
                height = next;
                let (keys, children) = cursor.branch(id)?;
                let slot = match start {
                    Bound::Included(query) | Bound::Excluded(query) => {
                        super::child_index::<false, _, _>(keys, query)?.unwrap_or(0)
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
            cursor.open_leaf(id)?;
            let (keys, _) = cursor.leaf.ok_or(Error::InvalidStructure)?;
            cursor.index = match start {
                Bound::Included(query) => prefix_len(keys, query, false)?,
                Bound::Excluded(query) => prefix_len(keys, query, true)?,
                Bound::Unbounded => 0,
            };
        }
        Ok(cursor)
    }
}

// Count the keys below the bound, optionally including a matching key.
#[inline]
fn prefix_len<K: Borrow<Q>, Q: ?Sized + Ord>(
    keys: &[K],
    query: &Q,
    inclusive: bool,
) -> Result<usize, Error> {
    let Some(index) = super::child_index::<false, _, _>(keys, query)? else {
        return Ok(0);
    };
    if inclusive || keys.get(index).ok_or(Error::InvalidStructure)?.borrow() < query {
        index.checked_add(1).ok_or(Error::InvalidStructure)
    } else {
        Ok(index)
    }
}

impl<'a, K: Borrow<Q>, V, Q: ?Sized + Ord, const N: usize> Range<'_, 'a, '_, K, V, Q, N> {
    #[inline]
    fn branch(&self, id: usize) -> Result<&'a ([K; N], [usize; N]), Error> {
        let index = id
            .checked_sub(self.tree.leaves.len())
            .ok_or(Error::InvalidStructure)?;
        self.tree.branches.get(index).ok_or(Error::InvalidStructure)
    }

    #[inline]
    fn open_leaf(&mut self, id: usize) -> Result<(), Error> {
        let leaf = self.tree.leaves.get(id).ok_or(Error::InvalidStructure)?;
        let keys = &leaf.0;
        self.limit = match self.end {
            Bound::Included(query) if keys.last().is_some_and(|k| k.borrow() > query) => {
                prefix_len(keys, query, true)?
            }
            Bound::Excluded(query) if keys.last().is_some_and(|k| k.borrow() >= query) => {
                prefix_len(keys, query, false)?
            }
            _ => N,
        };
        self.leaf = Some(leaf);
        self.index = 0;
        Ok(())
    }

    // Most leaf changes stay within the cached parent. Changing parents
    // decodes the logical rank from the root, in separator order.
    fn advance_leaf(&mut self) -> Result<(), Error> {
        self.leaf = None;
        let rank = self.rank.checked_add(1).ok_or(Error::InvalidStructure)?;
        let count = self.tree.leaves.len();
        if rank >= count {
            return Ok(());
        }
        self.rank = rank;
        let slot = rank.checked_rem(N).ok_or(Error::InvalidStructure)?;
        if slot != 0 {
            let children = self.parent.ok_or(Error::InvalidStructure)?;
            return self.open_leaf(*children.get(slot).ok_or(Error::InvalidStructure)?);
        }
        let mut id = self.tree.root.ok_or(Error::InvalidStructure)?;
        let mut relative = rank;
        let mut width = count;
        let mut height = self.tree.height;
        while let Some(next) = height.checked_sub(1) {
            height = next;
            width = width.checked_div(N).ok_or(Error::InvalidStructure)?;
            let slot = relative.checked_div(width).ok_or(Error::InvalidStructure)?;
            relative = relative.checked_rem(width).ok_or(Error::InvalidStructure)?;
            let (_, children) = self.branch(id)?;
            self.parent = Some(children);
            id = *children.get(slot).ok_or(Error::InvalidStructure)?;
        }
        self.open_leaf(id)
    }
}

impl<'a, K: Borrow<Q> + 'a, V: 'a, Q: ?Sized + Ord, const N: usize> TryIterator
    for Range<'_, 'a, '_, K, V, Q, N>
{
    type Item = (&'a K, &'a V);
    type Error = Error;

    #[inline(always)]
    fn try_next(&mut self) -> Result<Option<Self::Item>, Error> {
        if self.index >= self.limit {
            if self.leaf.is_none() || self.limit < N {
                self.leaf = None;
                return Ok(None);
            }
            self.advance_leaf()?;
        }
        let Some((keys, values)) = self.leaf else {
            return Ok(None);
        };
        if self.index >= self.limit {
            self.leaf = None;
            return Ok(None);
        }
        let key = keys.get(self.index).ok_or(Error::InvalidStructure)?;
        let value = values.get(self.index).ok_or(Error::InvalidStructure)?;
        self.index = self.index.checked_add(1).ok_or(Error::InvalidStructure)?;
        Ok(Some((key, value)))
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
