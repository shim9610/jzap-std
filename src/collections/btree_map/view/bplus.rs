//! Checked point lookup over caller-owned B+ tree nodes with complete keys.

use super::{LookupStep, search_index};
use crate::Error;
use core::borrow::Borrow;

enum Contents<'a, V, I> {
    Leaf(&'a [V]),
    Branch(&'a [I]),
}

/// A borrowed node whose shape and strict key order have been checked.
pub struct Node<'a, K, V, I = usize> {
    keys: &'a [K],
    contents: Contents<'a, V, I>,
}

impl<'a, K: Ord, V, I> Node<'a, K, V, I> {
    /// Each key identifies a value. An empty leaf can serve as the root.
    pub fn leaf(keys: &'a [K], values: &'a [V]) -> Result<Self, Error> {
        Self::new(keys, values.len(), Contents::Leaf(values))
    }

    /// Each separator is the minimum key of its corresponding child.
    pub fn branch(minima: &'a [K], children: &'a [I]) -> Result<Self, Error> {
        if minima.is_empty() {
            return Err(Error::InvalidStructure);
        }
        Self::new(minima, children.len(), Contents::Branch(children))
    }

    fn new(keys: &'a [K], count: usize, contents: Contents<'a, V, I>) -> Result<Self, Error> {
        if keys.len() != count || keys.iter().zip(keys.iter().skip(1)).any(|(a, b)| a >= b) {
            return Err(Error::InvalidStructure);
        }
        Ok(Self { keys, contents })
    }
}

/// Validates the visited path of a point lookup. A rejected node leaves the
/// lookup ready to accept a replacement for the same identifier.
///
/// Separator keys are copied into the lookup. Keys with borrows must keep
/// their referents alive, while self-contained keys allow the node buffer to
/// be reused after each `NeedNode`.
pub struct Lookup<'q, K, I, Q: ?Sized = K> {
    query: &'q Q,
    pending: Option<I>,
    height: usize,
    depth: usize,
    path: [I; usize::BITS as usize],
    minimum: Option<K>,
    ceiling: Option<K>,
}

impl<'q, K: Copy + Ord + Borrow<Q>, I: Copy + Eq, Q: ?Sized + Ord> Lookup<'q, K, I, Q> {
    /// Start at `root`. A leaf has height zero.
    pub fn new(root: I, height: usize, query: &'q Q) -> Result<Self, Error> {
        if height >= usize::BITS as usize {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            query,
            pending: Some(root),
            height,
            depth: 0,
            path: [root; usize::BITS as usize],
            minimum: None,
            ceiling: None,
        })
    }

    pub fn requested_node(&self) -> Option<I> {
        self.pending
    }

    /// Validate and search one node. A `Found` result borrows the supplied leaf.
    #[inline(always)]
    pub fn provide<'a, V>(
        &mut self,
        id: I,
        node: &Node<'a, K, V, I>,
    ) -> Result<LookupStep<'a, K, V, I>, Error> {
        if self.pending != Some(id)
            || (self.height == 0) != matches!(node.contents, Contents::Leaf(_))
            || self
                .minimum
                .is_some_and(|min| node.keys.first() != Some(&min))
            || self
                .ceiling
                .is_some_and(|max| node.keys.last().is_some_and(|last| *last >= max))
        {
            return Err(Error::InvalidStructure);
        }

        let position = search_index(node.keys, self.query)?;
        let result = match node.contents {
            Contents::Leaf(values) => match position {
                Ok(index) => {
                    let (key, value) = node
                        .keys
                        .get(index)
                        .zip(values.get(index))
                        .ok_or(Error::InvalidStructure)?;
                    LookupStep::Found(key, value)
                }
                Err(_) => LookupStep::Absent,
            },
            Contents::Branch(children) => {
                let index = match position {
                    Ok(index) => Some(index),
                    Err(index) => index.checked_sub(1),
                };
                if let Some(index) = index {
                    return self.descend(id, index, node.keys, children);
                }
                LookupStep::Absent
            }
        };
        self.pending = None;
        Ok(result)
    }

    #[inline(always)]
    fn descend<'a, V>(
        &mut self,
        id: I,
        index: usize,
        minima: &[K],
        children: &[I],
    ) -> Result<LookupStep<'a, K, V, I>, Error> {
        let child = *children.get(index).ok_or(Error::InvalidStructure)?;
        let minimum = *minima.get(index).ok_or(Error::InvalidStructure)?;
        let depth = self.depth.checked_add(1).ok_or(Error::InvalidStructure)?;
        let height = self.height.checked_sub(1).ok_or(Error::InvalidStructure)?;
        let history = self.path.get(..self.depth).ok_or(Error::InvalidStructure)?;
        if child == id || history.contains(&child) {
            return Err(Error::InvalidStructure);
        }
        let next = index.checked_add(1).ok_or(Error::InvalidStructure)?;
        let slot = self
            .path
            .get_mut(self.depth)
            .ok_or(Error::InvalidStructure)?;
        *slot = id;
        self.depth = depth;
        self.height = height;
        self.minimum = Some(minimum);
        self.ceiling = minima.get(next).copied().or(self.ceiling);
        self.pending = Some(child);
        Ok(LookupStep::NeedNode(child))
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(9)]
    fn branch_routes_to_the_last_admitted_minimum() {
        let minima: [u8; 4] = kani::any();
        let query: u8 = kani::any();
        let children = [1, 2, 3, 4];
        let Ok(node) = Node::<_, (), _>::branch(&minima, &children) else {
            return;
        };
        let expected = minima
            .iter()
            .zip(children)
            .filter(|(key, _)| **key <= query)
            .last();
        let mut lookup = Lookup::new(0, 1, &query).unwrap();
        let step = lookup.provide(0, &node).unwrap();
        assert!(match expected {
            Some((_, child)) => step == LookupStep::NeedNode(child),
            None => step == LookupStep::Absent,
        });
    }
}
