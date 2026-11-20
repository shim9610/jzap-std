//! A checked snapshot over stable fixed-stride pages.

use super::{ByteQuery, KeyPosition, StridedFormat, StridedNode};
use crate::{Error, vec::Vec};

/// An immutable B+ snapshot over borrowed strided pages.
///
/// Child identifiers are indices into the page slice in any order. Page bytes
/// and descriptors are borrowed. Keys and child identifiers must decode
/// consistently for as long as the snapshot is in use.
pub struct StridedTree<'pages, 'bytes, F> {
    nodes: &'pages [StridedNode<'bytes, F>],
    root: Option<usize>,
    height: usize,
}

struct Visit<'a> {
    id: usize,
    height: usize,
    minimum: Option<&'a [u8]>,
    ceiling: Option<&'a [u8]>,
}

impl<'pages, 'bytes, F: StridedFormat<Id = usize>> StridedTree<'pages, 'bytes, F> {
    /// Validate the entire tree and borrow its descriptors.
    ///
    /// Pass no root and height zero for an empty page slice. All nonempty
    /// snapshots must have a root and exactly one path to every page. The
    /// temporary validation allocations are released before returning.
    pub fn try_new(
        nodes: &'pages [StridedNode<'bytes, F>],
        root: Option<usize>,
        height: usize,
    ) -> Result<Self, Error> {
        let tree = Self {
            nodes,
            root,
            height,
        };
        match root {
            Some(id) if height < usize::BITS as usize => tree.validate(id)?,
            None if nodes.is_empty() && height == 0 => {}
            _ => return Err(Error::InvalidStructure),
        }
        Ok(tree)
    }

    fn validate(&self, root: usize) -> Result<(), Error> {
        let mut seen = Vec::try_with_capacity(self.nodes.len())?;
        for _ in self.nodes {
            seen.try_push(false).map_err(|rejected| rejected.error)?;
        }
        let mut pending = Vec::new();
        enqueue(
            &mut pending,
            seen.as_mut_slice(),
            Visit {
                id: root,
                height: self.height,
                minimum: None,
                ceiling: None,
            },
        )?;
        while let Some(visit) = pending.pop() {
            let node = self.nodes.get(visit.id).ok_or(Error::InvalidStructure)?;
            if !node.fits(visit.height, visit.minimum, visit.ceiling) {
                return Err(Error::InvalidStructure);
            }
            if let Some(height) = visit.height.checked_sub(1) {
                let mut minimum = node.first;
                for index in 0..node.count {
                    let next = index.checked_add(1).ok_or(Error::InvalidStructure)?;
                    let ceiling = node.key_if_present(next)?;
                    enqueue(
                        &mut pending,
                        seen.as_mut_slice(),
                        Visit {
                            id: F::child(node.payload(index)?)?,
                            height,
                            minimum: Some(minimum),
                            ceiling: ceiling.or(visit.ceiling),
                        },
                    )?;
                    if let Some(next_minimum) = ceiling {
                        minimum = next_minimum;
                    }
                }
            }
        }
        if seen.as_slice().iter().any(|visited| !visited) {
            return Err(Error::InvalidStructure);
        }
        Ok(())
    }

    /// Find the value in the original leaf page without copying it or
    /// allocating storage. Tree structure is checked by the constructor.
    #[inline]
    pub fn get(&self, query: &[u8]) -> Result<Option<&'bytes [u8]>, Error> {
        let Some(mut id) = self.root else {
            return Ok(None);
        };
        let query = ByteQuery::new(query);
        let mut remaining = self.height;
        while let Some(height) = remaining.checked_sub(1) {
            remaining = height;
            let node = self.nodes.get(id).ok_or(Error::InvalidStructure)?;
            let index = match node.search(&query)? {
                KeyPosition::Found(index, _) => index,
                KeyPosition::Gap(upper) => {
                    let Some(index) = upper.checked_sub(1) else {
                        return Ok(None);
                    };
                    index
                }
            };
            id = F::child(node.payload(index)?)?;
        }
        let node = self.nodes.get(id).ok_or(Error::InvalidStructure)?;
        match node.search(&query)? {
            KeyPosition::Found(index, _) => node.payload(index).map(Some),
            KeyPosition::Gap(_) => Ok(None),
        }
    }
}

fn enqueue<'a>(
    pending: &mut Vec<Visit<'a>>,
    seen: &mut [bool],
    visit: Visit<'a>,
) -> Result<(), Error> {
    let claimed = seen.get_mut(visit.id).ok_or(Error::InvalidStructure)?;
    if *claimed {
        return Err(Error::InvalidStructure);
    }
    pending.try_push(visit).map_err(|rejected| rejected.error)?;
    *claimed = true;
    Ok(())
}

#[cfg(any(test, kani))]
pub(in super::super) mod test_format {
    use super::*;
    use crate::collections::btree_map::view::bplus::StridedLayout;
    use core::num::NonZeroUsize;

    pub(in super::super) struct Index;

    impl StridedFormat for Index {
        type Fence = [u8; 1];
        type Id = usize;

        fn key(field: &[u8]) -> Result<&[u8], Error> {
            Ok(field)
        }

        fn copy_fence(key: &[u8]) -> Result<Self::Fence, Error> {
            key.try_into().map_err(|_| Error::InvalidStructure)
        }

        fn child(field: &[u8]) -> Result<usize, Error> {
            field
                .first()
                .copied()
                .map(usize::from)
                .ok_or(Error::InvalidStructure)
        }
    }

    fn layout() -> StridedLayout {
        StridedLayout {
            stride: NonZeroUsize::new(2).unwrap(),
            key: 0..1,
            payload: 1..2,
        }
    }

    pub(super) fn leaf(bytes: &[u8], count: usize) -> StridedNode<'_, Index> {
        StridedNode::leaf(bytes, count, layout()).unwrap()
    }

    pub(super) fn branch(bytes: &[u8], count: usize) -> StridedNode<'_, Index> {
        StridedNode::branch(bytes, count, layout()).unwrap()
    }

    pub(in super::super) fn arbitrary_ids() -> [StridedNode<'static, Index>; 3] {
        [
            branch(&[5, 2, 20, 1], 2),
            leaf(&[20, 71, 21, 72], 2),
            leaf(&[5, 51, 10, 52], 2),
        ]
    }

    #[cfg(kani)]
    pub(in super::super) fn known_snapshot<'p>(
        nodes: &'p [StridedNode<'static, Index>; 3],
    ) -> StridedTree<'p, 'static, Index> {
        // Fixed valid fixture. The native and Miri scenarios exercise its
        // public constructor; this harness isolates the read-side invariant.
        StridedTree {
            nodes,
            root: Some(0),
            height: 1,
        }
    }

    #[cfg(kani)]
    pub(in super::super) fn check_enqueue(id: usize, first_seen: bool) {
        let mut seen = [first_seen, true];
        let mut pending = Vec::try_with_capacity(1).unwrap();
        let visit = Visit {
            id,
            height: 0,
            minimum: None,
            ceiling: None,
        };
        let result = enqueue(&mut pending, &mut seen, visit);
        if id == 0 && !first_seen {
            assert_eq!(result, Ok(()));
            assert_eq!(seen, [true, true]);
            assert_eq!(pending.pop().map(|row| row.id), Some(id));
        } else {
            assert_eq!(result, Err(Error::InvalidStructure));
            assert_eq!(seen, [first_seen, true]);
            assert!(pending.pop().is_none());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_format::*, *};

    #[test]
    fn bplus_strided_snapshot_queries_arbitrary_page_order() {
        let nodes = arbitrary_ids();
        let tree = StridedTree::try_new(&nodes, Some(0), 1).unwrap();
        for query in 0u8..=u8::MAX {
            let expected = match query {
                5 => Some(&[51][..]),
                10 => Some(&[52][..]),
                20 => Some(&[71][..]),
                21 => Some(&[72][..]),
                _ => None,
            };
            assert_eq!(tree.get(&[query]), Ok(expected));
        }
        assert_eq!(
            StridedTree::<Index>::try_new(&[], None, 0)
                .unwrap()
                .get(&[]),
            Ok(None)
        );
    }

    #[test]
    fn bplus_strided_snapshot_rejects_invalid_topology_and_inherited_ceiling() {
        let nodes = [
            branch(&[1, 1, 10, 2], 2),
            branch(&[1, 3], 1),
            branch(&[10, 4], 1),
            leaf(&[1, 55, 10, 77], 2),
            leaf(&[10, 66], 1),
        ];
        assert!(StridedTree::try_new(&nodes, Some(0), 2).is_err());
        let invalid = [
            [branch(&[5, 0], 1), leaf(&[5, 51], 1)],        // self edge
            [branch(&[5, 1, 20, 1], 2), leaf(&[5, 51], 1)], // reused child
            [branch(&[5, 2], 1), leaf(&[5, 51], 1)],        // unknown child
        ];
        for nodes in invalid {
            assert!(StridedTree::try_new(&nodes, Some(0), 1).is_err());
        }
        let orphan = [branch(&[5, 1], 1), leaf(&[5, 51], 1), leaf(&[20, 77], 1)];
        assert!(StridedTree::try_new(&orphan, Some(0), 1).is_err());
        assert!(StridedTree::try_new(&orphan[..2], Some(0), 2).is_err());
    }
}
