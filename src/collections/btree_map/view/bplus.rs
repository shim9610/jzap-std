//! Checked point lookup over caller-owned B+ tree nodes with complete keys.

use super::{LookupStep, search_index};
use crate::Error;
use core::{borrow::Borrow, hint::select_unpredictable, mem::MaybeUninit};

mod fixed;
mod strided;
pub use fixed::{FixedTree, Range};
pub use strided::{StridedFormat, StridedLayout, StridedNode, StridedTree};

const SMALL_NODE_KEYS: usize = 8;

#[cfg(not(windows))]
#[inline(always)]
fn short_keys<K, Q: ?Sized>(query: &Q) -> bool {
    size_of::<K>() <= size_of::<u64>() && size_of_val(query) <= size_of::<u64>()
}

#[cfg(not(windows))]
#[inline(always)]
fn count_admitted_minima<K: Borrow<Q>, Q: ?Sized + Ord>(
    later: &[K],
    query: &Q,
) -> Result<usize, Error> {
    // Read the seven later minima at fixed offsets for width-eight nodes.
    Iterator::try_fold(&mut later.iter(), 0usize, |count, key| {
        count.checked_add(usize::from(key.borrow().cmp(query).is_le()))
    })
    .ok_or(Error::InvalidStructure)
}

#[inline]
fn child_index<const CONDITIONAL: bool, K: Borrow<Q>, Q: ?Sized + Ord>(
    keys: &[K],
    query: &Q,
) -> Result<Option<usize>, Error> {
    if keys.first().is_none_or(|key| key.borrow() > query) {
        return Ok(None);
    }
    // The base is admitted, and base + remaining stays within the key slice.
    let mut base = 0usize;
    let mut remaining = keys.len();
    while remaining > 1 {
        let half = remaining / 2;
        let middle = base.checked_add(half).ok_or(Error::InvalidStructure)?;
        let key = keys.get(middle).ok_or(Error::InvalidStructure)?;
        if CONDITIONAL {
            base = select_unpredictable(key.borrow() <= query, middle, base);
        } else if key.borrow() <= query {
            base = middle;
        }
        remaining = remaining.checked_sub(half).ok_or(Error::InvalidStructure)?;
    }
    Ok(Some(base))
}

#[inline]
fn leaf_index<K: Borrow<Q>, Q: ?Sized + Ord>(keys: &[K], query: &Q) -> Option<usize> {
    let index = keys.partition_point(|key| key.borrow().cmp(query).is_lt());
    keys.get(index)
        .filter(|key| (*key).borrow().cmp(query).is_eq())
        .map(|_| index)
}

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
        if keys.len() != count || !keys.is_sorted_by(|a, b| a < b) {
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
    // The first `depth` entries contain the visited ancestors.
    path: [MaybeUninit<I>; usize::BITS as usize],
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
            path: [MaybeUninit::uninit(); usize::BITS as usize],
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

        let result = match node.contents {
            Contents::Leaf(values) => {
                let index = if node.keys.len() <= SMALL_NODE_KEYS {
                    search_index(node.keys, self.query)?.ok()
                } else {
                    leaf_index(node.keys, self.query)
                };
                if let Some(index) = index {
                    let (key, value) = node
                        .keys
                        .get(index)
                        .zip(values.get(index))
                        .ok_or(Error::InvalidStructure)?;
                    LookupStep::Found(key, value)
                } else {
                    LookupStep::Absent
                }
            }
            Contents::Branch(children) => {
                // Keep the Windows lookup layout after the width-eight timing regression.
                #[cfg(windows)]
                let index = if node.keys.len() <= SMALL_NODE_KEYS {
                    child_index::<false, _, _>(node.keys, self.query)?
                } else {
                    child_index::<true, _, _>(node.keys, self.query)?
                };
                #[cfg(not(windows))]
                let index = if let Some(keys) =
                    node.keys.first_chunk::<SMALL_NODE_KEYS>().filter(|_| {
                        node.keys.len() == SMALL_NODE_KEYS && short_keys::<K, _>(self.query)
                    }) {
                    let (first, later) = keys.split_first().ok_or(Error::InvalidStructure)?;
                    if first.borrow() > self.query {
                        None
                    } else {
                        Some(count_admitted_minima(later, self.query)?)
                    }
                } else if node.keys.len() <= SMALL_NODE_KEYS {
                    child_index::<false, _, _>(node.keys, self.query)?
                } else {
                    child_index::<true, _, _>(node.keys, self.query)?
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
        let (slot, history) = self
            .path
            .get_mut(..=self.depth)
            .and_then(|path| path.split_last_mut())
            .ok_or(Error::InvalidStructure)?;
        if child == id
            || history.iter().any(|ancestor| {
                // SAFETY: The prefix excludes the current slot. Each
                // successful descent writes its slot before increasing depth.
                *unsafe { ancestor.assume_init_ref() } == child
            })
        {
            return Err(Error::InvalidStructure);
        }
        let next = index.checked_add(1).ok_or(Error::InvalidStructure)?;
        slot.write(id);
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
    #[kani::unwind(10)]
    fn branch_routes_to_the_last_admitted_minimum() {
        let minima: [u8; 5] = kani::any();
        let query: u8 = kani::any();
        let count = usize::from(kani::any::<u8>());
        kani::assume(count <= minima.len());
        let minima = &minima[..count];
        let children = &[1, 2, 3, 4, 5][..count];
        let Ok(node) = Node::<_, (), _>::branch(minima, children) else {
            return;
        };
        let expected = minima
            .iter()
            .zip(children.iter().copied())
            .filter(|(key, _)| **key <= query)
            .last();
        assert!(
            child_index::<true, _, _>(minima, &query).unwrap()
                == minima.iter().rposition(|key| *key <= query)
        );
        let mut lookup = Lookup::new(0, 1, &query).unwrap();
        let step = lookup.provide(0, &node).unwrap();
        assert!(match expected {
            Some((_, child)) => step == LookupStep::NeedNode(child),
            None => step == LookupStep::Absent,
        });
    }

    #[cfg(not(windows))]
    #[kani::proof]
    #[kani::unwind(10)]
    fn width_eight_lookup_selects_the_last_admitted_child() {
        let minima: [u8; 8] = kani::any();
        let query: u8 = kani::any();
        let children = [1, 2, 3, 4, 5, 6, 7, 8];
        let Ok(node) = Node::<_, (), _>::branch(&minima, &children) else {
            return;
        };
        let expected = minima
            .iter()
            .rposition(|key| *key <= query)
            .map_or(LookupStep::Absent, |index| {
                LookupStep::NeedNode(children[index])
            });
        let mut lookup = Lookup::new(0, 1, &query).unwrap();
        assert!(lookup.provide(0, &node) == Ok(expected));
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn leaf_lookup_matches_linear_search() {
        let keys: [u8; 5] = kani::any();
        let values = [9, 8, 7, 6, 5];
        let query: u8 = kani::any();
        let count = usize::from(kani::any::<u8>());
        kani::assume(count <= keys.len());
        let Ok(node) = Node::leaf(&keys[..count], &values[..count]) else {
            return;
        };
        let expected = keys[..count]
            .iter()
            .zip(&values)
            .find(|(key, _)| **key == query)
            .map_or(LookupStep::<_, _, usize>::Absent, |(key, value)| {
                LookupStep::Found(key, value)
            });
        assert!(
            leaf_index(&keys[..count], &query)
                == keys[..count].iter().position(|key| *key == query)
        );
        let mut lookup = Lookup::new(0, 0, &query).unwrap();
        assert!(lookup.provide(0, &node) == Ok(expected));
    }
}
