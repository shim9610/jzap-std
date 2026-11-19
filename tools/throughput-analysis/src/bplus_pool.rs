//! Fixed-width, immutable B+ pools used by the performance baseline.

use super::{Answer, Page};
use core::{borrow::Borrow, hint::select_unpredictable};
use jzap::Error;

// Search kernels used by the prototype, derived from the checked view at 5313656.
const SMALL_NODE_KEYS: usize = 8;

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
            let advance = select_unpredictable(key.borrow() <= query, half, 0);
            base = base.wrapping_add(advance);
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

pub(super) struct Resident<'a> {
    pages: &'a [Page<usize>],
    views: Vec<ResidentNode<'a>>,
    root: usize,
    height: usize,
}

struct ResidentNode<'a> {
    keys: &'a [u64],
    contents: ResidentContents<'a>,
}

enum ResidentContents<'a> {
    Leaf(&'a [u64]),
    Branch(&'a [usize]),
}

impl<'a> Resident<'a> {
    pub(super) fn check(pages: &'a [Page<usize>], height: usize) -> Result<Self, Error> {
        let mut summaries: Vec<(usize, u64)> = Vec::with_capacity(pages.len());
        let mut views = Vec::with_capacity(pages.len());
        for (id, page) in pages.iter().enumerate() {
            page.view()?;
            let last_key = *page.keys.last().ok_or(Error::InvalidStructure)?;
            let contents;
            let summary;
            if page.children.is_empty() {
                summary = (0usize, last_key);
                contents = ResidentContents::Leaf(&page.values);
            } else {
                let first = page.children.first().ok_or(Error::InvalidStructure)?;
                let (first_height, _) = *summaries.get(*first).ok_or(Error::InvalidStructure)?;
                let mut max_key = last_key;
                for (slot, &child) in page.children.iter().enumerate() {
                    let (child_height, child_max) =
                        *summaries.get(child).ok_or(Error::InvalidStructure)?;
                    if child >= id
                        || child_height != first_height
                        || pages.get(child).and_then(|page| page.keys.first())
                            != page.keys.get(slot)
                        || page
                            .keys
                            .get(slot + 1)
                            .is_some_and(|ceiling| child_max >= *ceiling)
                    {
                        return Err(Error::InvalidStructure);
                    }
                    max_key = child_max;
                }
                summary = (
                    first_height.checked_add(1).ok_or(Error::InvalidStructure)?,
                    max_key,
                );
                contents = ResidentContents::Branch(&page.children);
            }
            summaries.push(summary);
            views.push(ResidentNode {
                keys: &page.keys,
                contents,
            });
        }
        let (actual_height, _) = *summaries.last().ok_or(Error::InvalidStructure)?;
        if actual_height != height {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            pages,
            views,
            root: pages.len().checked_sub(1).ok_or(Error::InvalidStructure)?,
            height,
        })
    }

    #[inline(always)]
    pub(super) fn get(&self, query: u64) -> Answer {
        let mut id = self.root;
        for _ in 0..self.height {
            let node = self.views.get(id).ok_or(Error::InvalidStructure)?;
            let ResidentContents::Branch(children) = node.contents else {
                return Err(Error::InvalidStructure);
            };
            let next = if node.keys.len() <= SMALL_NODE_KEYS {
                child_index::<false, _, _>(node.keys, &query)?
            } else {
                child_index::<true, _, _>(node.keys, &query)?
            };
            let Some(next) = next else {
                return Ok(None);
            };
            id = *children.get(next).ok_or(Error::InvalidStructure)?;
        }
        let node = self.views.get(id).ok_or(Error::InvalidStructure)?;
        let ResidentContents::Leaf(values) = node.contents else {
            return Err(Error::InvalidStructure);
        };
        let index = if node.keys.len() <= SMALL_NODE_KEYS {
            node.keys.binary_search(&query).ok()
        } else {
            leaf_index(node.keys, &query)
        };
        index
            .map(|index| values.get(index).copied().ok_or(Error::InvalidStructure))
            .transpose()
    }
}

// Compare separate allocations with inline node records while preserving the
// same fixed node width, numeric IDs, child selection, and slice checks.
pub(super) struct Pools<Keys, Children> {
    branches: Vec<(Keys, Children)>,
    leaves: Vec<(Keys, Keys)>,
    root: usize,
    height: usize,
}

impl<Keys, Children> Pools<Keys, Children> {
    #[inline(always)]
    pub(super) fn get<const N: usize>(&self, query: u64) -> Answer
    where
        Keys: Borrow<[u64; N]>,
        Children: Borrow<[usize; N]>,
    {
        let mut id = self.root;
        for _ in 0..self.height {
            let branch_id = id
                .checked_sub(self.leaves.len())
                .ok_or(Error::InvalidStructure)?;
            let (keys, children) = self
                .branches
                .get(branch_id)
                .ok_or(Error::InvalidStructure)?;
            let keys: &[u64; N] = keys.borrow();
            let next = if N <= SMALL_NODE_KEYS {
                child_index::<false, _, _>(keys, &query)?
            } else {
                child_index::<true, _, _>(keys, &query)?
            };
            let Some(next) = next else {
                return Ok(None);
            };
            let children: &[usize; N] = children.borrow();
            id = *children.get(next).ok_or(Error::InvalidStructure)?;
        }
        let (keys, values) = self.leaves.get(id).ok_or(Error::InvalidStructure)?;
        let keys: &[u64; N] = keys.borrow();
        let values: &[u64; N] = values.borrow();
        let index = if N <= SMALL_NODE_KEYS {
            keys.binary_search(&query).ok()
        } else {
            leaf_index(keys, &query)
        };
        index
            .map(|i| values.get(i).copied().ok_or(Error::InvalidStructure))
            .transpose()
    }
}

type SplitPools<'a, const N: usize> = Pools<&'a [u64; N], &'a [usize; N]>;
type PackedPools<const N: usize> = Pools<[u64; N], [usize; N]>;

impl<'a> Resident<'a> {
    /// Borrow fixed-width arrays from the same snapshot that passed validation.
    pub(super) fn split<const N: usize>(
        &self,
        leaf_count: usize,
    ) -> Result<SplitPools<'a, N>, Error> {
        let (leaves, branches) = self
            .pages
            .split_at_checked(leaf_count)
            .ok_or(Error::InvalidStructure)?;
        let leaves = leaves
            .iter()
            .map(|page| {
                if !page.children.is_empty() {
                    return Err(Error::InvalidStructure);
                }
                Ok((array(&page.keys)?, array(&page.values)?))
            })
            .collect::<Result<_, Error>>()?;
        let branches = branches
            .iter()
            .map(|page| {
                if page.children.is_empty() {
                    return Err(Error::InvalidStructure);
                }
                Ok((array(&page.keys)?, array(&page.children)?))
            })
            .collect::<Result<_, Error>>()?;
        Ok(Pools {
            branches,
            leaves,
            root: self.root,
            height: self.height,
        })
    }
}

fn array<T, const N: usize>(slice: &[T]) -> Result<&[T; N], Error> {
    slice.try_into().map_err(|_| Error::InvalidStructure)
}

pub(super) fn pack_pools<const N: usize>(split: &SplitPools<'_, N>) -> PackedPools<N> {
    let branches = split
        .branches
        .iter()
        .map(|&(keys, children)| (*keys, *children))
        .collect();
    let leaves = split
        .leaves
        .iter()
        .map(|&(keys, values)| (*keys, *values))
        .collect();
    Pools {
        branches,
        leaves,
        root: split.root,
        height: split.height,
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(10)]
    fn packed_lookup_matches_exact_key_oracle() {
        let packed: PackedPools<2> = Pools {
            branches: vec![([2, 6], [0, 1])],
            leaves: vec![([2, 4], [6, 12]), ([6, 8], [18, 24])],
            root: 2,
            height: 1,
        };
        let query = u64::from(kani::any::<u8>());
        let expected = match query {
            2 | 4 | 6 | 8 => Some(query * 3),
            _ => None,
        };
        assert!(packed.get::<2>(query) == Ok(expected));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::make_pages;
    use std::collections::BTreeMap;

    fn compare<const N: usize>(height: u32) {
        let pages = make_pages::<usize>(N, height);
        let leaf_count = N.pow(height);
        let resident = Resident::check(&pages, height as usize).unwrap();
        let split = resident.split::<N>(leaf_count).unwrap();
        let packed = pack_pools(&split);
        let model: BTreeMap<_, _> = pages[..leaf_count]
            .iter()
            .flat_map(|page| page.keys.iter().copied().zip(page.values.iter().copied()))
            .collect();

        for query in (0..(2 * model.len() as u64 + 4)).chain([u64::MAX]) {
            let expected = Ok(model.get(&query).copied());
            assert_eq!(resident.get(query), expected);
            assert_eq!(split.get::<N>(query), expected);
            assert_eq!(packed.get::<N>(query), expected);
        }

        let words = N * (size_of::<u64>() + size_of::<usize>());
        let expected_stride = words.next_multiple_of(align_of::<([u64; N], [usize; N])>());
        assert_eq!(size_of::<([u64; N], [usize; N])>(), expected_stride);
        assert_eq!(size_of::<([u64; N], [u64; N])>(), N * 2 * size_of::<u64>());
    }

    #[test]
    fn pools_match_the_sorted_map() {
        compare::<2>(0);
        compare::<2>(3);
        compare::<8>(2);
        compare::<9>(2);
        compare::<16>(1);
        compare::<32>(1);
    }

    #[test]
    fn validation_rejects_bad_edges_depth_and_fences() {
        let mut pages = make_pages::<usize>(2, 2);
        assert!(Resident::check(&[], 0).is_err());
        assert!(Resident::check(&pages, 1).is_err());

        pages[4].children[0] = 4;
        assert!(Resident::check(&pages, 2).is_err());
        pages[4].children[0] = 5;
        assert!(Resident::check(&pages, 2).is_err());
        pages[4].children[0] = usize::MAX;
        assert!(Resident::check(&pages, 2).is_err());
        pages[4].children[0] = 0;

        pages[6].children[1] = 2;
        assert!(Resident::check(&pages, 2).is_err());
        pages[6].children[1] = 5;

        pages[4].keys[1] += 1;
        assert!(Resident::check(&pages, 2).is_err());
        pages[4].keys[1] -= 1;

        let ceiling = pages[2].keys[0];
        let old_maximum = core::mem::replace(&mut pages[1].keys[1], ceiling);
        assert!(Resident::check(&pages, 2).is_err());
        pages[1].keys[1] = old_maximum;
        assert!(Resident::check(&pages, 2).is_ok());
    }

    #[test]
    fn fixed_width_and_pool_boundary_are_checked() {
        let pages = make_pages::<usize>(2, 2);
        let resident = Resident::check(&pages, 2).unwrap();
        assert!(resident.split::<3>(4).is_err());
        assert!(resident.split::<2>(3).is_err());
        assert!(resident.split::<2>(5).is_err());
        assert!(resident.split::<2>(usize::MAX).is_err());
        assert!(resident.split::<2>(4).is_ok());
    }
}
