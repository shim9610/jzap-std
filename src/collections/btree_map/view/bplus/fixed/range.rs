//! Ordered scans of an already validated snapshot.

use super::FixedTree;
use crate::{Error, TryIterator};
use core::{borrow::Borrow, cmp::Ordering, ops::Bound};

const MAX_BRANCH_DEPTH: usize = usize::BITS as usize;

#[derive(Clone, Copy)]
struct Frame {
    branch: usize,
    next: usize,
}

/// Forward range cursor over a validated [`FixedTree`].
///
/// Each item borrows a key and value from its original leaf. The cursor owns
/// its traversal state and does not allocate. Bounds and keys must keep their
/// ordering while the cursor is used.
pub struct Range<'t, 'a, 'q, K, V, Q: ?Sized, const N: usize> {
    tree: &'t FixedTree<'a, K, V, N>,
    end: Bound<&'q Q>,
    frames: [Frame; MAX_BRANCH_DEPTH],
    depth: usize,
    leaf: Option<usize>,
    index: usize,
}

impl<'a, K: Ord, V, const N: usize> FixedTree<'a, K, V, N> {
    /// Open an ordered cursor with inclusive, exclusive, or open endpoints.
    ///
    /// Returns [`Error::Bounds`] if the start exceeds the end or both exclude
    /// the same key. An empty tree or range yields no items.
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
            frames: [Frame { branch: 0, next: 0 }; MAX_BRANCH_DEPTH],
            depth: 0,
            leaf: None,
            index: 0,
        };
        if let Some(root) = self.root {
            cursor.descend(root, self.height, start)?;
        }
        Ok(cursor)
    }
}

impl<K: Borrow<Q>, V, Q: ?Sized + Ord, const N: usize> Range<'_, '_, '_, K, V, Q, N> {
    fn descend(&mut self, mut id: usize, mut height: usize, start: Bound<&Q>) -> Result<(), Error> {
        while let Some(next_height) = height.checked_sub(1) {
            height = next_height;
            let branch = id
                .checked_sub(self.tree.leaves.len())
                .ok_or(Error::InvalidStructure)?;
            let (keys, children) = self
                .tree
                .branches
                .get(branch)
                .ok_or(Error::InvalidStructure)?;
            let index = match start {
                Bound::Included(query) | Bound::Excluded(query) => {
                    super::child_index::<false, _, _>(keys, query)?.unwrap_or(0)
                }
                Bound::Unbounded => 0,
            };
            id = *children.get(index).ok_or(Error::InvalidStructure)?;
            // At width one there are no siblings, even for a very deep tree.
            // Otherwise h levels require at least 2^h distinct leaves.
            if N > 1 {
                let frame = self
                    .frames
                    .get_mut(self.depth)
                    .ok_or(Error::InvalidStructure)?;
                *frame = Frame {
                    branch,
                    next: index.checked_add(1).ok_or(Error::InvalidStructure)?,
                };
                self.depth = self.depth.checked_add(1).ok_or(Error::InvalidStructure)?;
            }
        }
        let (keys, _) = self.tree.leaves.get(id).ok_or(Error::InvalidStructure)?;
        let mut index = 0;
        while let Some(key) = keys.get(index) {
            let before_start = match start {
                Bound::Included(query) => key.borrow().cmp(query) == Ordering::Less,
                Bound::Excluded(query) => key.borrow().cmp(query) != Ordering::Greater,
                Bound::Unbounded => false,
            };
            if !before_start {
                break;
            }
            index = index.checked_add(1).ok_or(Error::InvalidStructure)?;
        }
        self.leaf = Some(id);
        self.index = index;
        Ok(())
    }

    fn advance_leaf(&mut self) -> Result<(), Error> {
        self.leaf = None;
        while let Some(level) = self.depth.checked_sub(1) {
            let frame = self.frames.get_mut(level).ok_or(Error::InvalidStructure)?;
            let (_, children) = self
                .tree
                .branches
                .get(frame.branch)
                .ok_or(Error::InvalidStructure)?;
            if let Some(&id) = children.get(frame.next) {
                frame.next = frame.next.checked_add(1).ok_or(Error::InvalidStructure)?;
                let height = self
                    .tree
                    .height
                    .checked_sub(self.depth)
                    .ok_or(Error::InvalidStructure)?;
                return self.descend(id, height, Bound::Unbounded);
            }
            self.depth = level;
        }
        Ok(())
    }
}

impl<'a, K: Borrow<Q> + 'a, V: 'a, Q: ?Sized + Ord, const N: usize> TryIterator
    for Range<'_, 'a, '_, K, V, Q, N>
{
    type Item = (&'a K, &'a V);
    type Error = Error;

    fn try_next(&mut self) -> Result<Option<Self::Item>, Error> {
        if self.leaf.is_some() && self.index == N {
            self.advance_leaf()?;
        }
        let Some(id) = self.leaf else { return Ok(None) };
        let (keys, values) = self.tree.leaves.get(id).ok_or(Error::InvalidStructure)?;
        let key = keys.get(self.index).ok_or(Error::InvalidStructure)?;
        let past_end = match self.end {
            Bound::Included(query) => key.borrow().cmp(query) == Ordering::Greater,
            Bound::Excluded(query) => key.borrow().cmp(query) != Ordering::Less,
            Bound::Unbounded => false,
        };
        if past_end {
            self.leaf = None;
            return Ok(None);
        }
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
