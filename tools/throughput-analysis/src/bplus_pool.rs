//! Fixed-width, immutable B+ pools used by the performance baseline.

use core::{borrow::Borrow, hint::select_unpredictable};
use jzap::{Error, collections::btree_map::view::bplus::Node, vec::Vec};

type Answer = Result<Option<u64>, Error>;

pub(super) struct Page<'a> {
    pub(super) keys: &'a [u64],
    pub(super) values: &'a [u64],
    pub(super) children: &'a [usize],
}

impl Page<'_> {
    fn check(&self) -> Result<(), Error> {
        if self.children.is_empty() {
            Node::<_, _, usize>::leaf(self.keys, self.values)?;
        } else {
            Node::<_, u64, _>::branch(self.keys, self.children)?;
        }
        Ok(())
    }
}

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

pub(super) struct Resident<'a> {
    views: Vec<ResidentNode<'a>>,
    root: usize,
    height: usize,
}

struct ResidentNode<'a> {
    keys: &'a [u64],
    contents: ResidentContents<'a>,
    height: usize,
    max_key: u64,
}

enum ResidentContents<'a> {
    Leaf(&'a [u64]),
    Branch(&'a [usize]),
}

// Validate a branch against the completed prefix before publishing its summary.
fn summarize_branch(page: &Page<'_>, prefix: &[ResidentNode<'_>]) -> Result<(usize, u64), Error> {
    let first = *page.children.first().ok_or(Error::InvalidStructure)?;
    let first = prefix.get(first).ok_or(Error::InvalidStructure)?;
    let mut max_key = first.max_key;
    let mut minima = page.keys.iter();
    let mut minimum = minima.next();
    for &child in page.children {
        let child = prefix.get(child).ok_or(Error::InvalidStructure)?;
        let ceiling = minima.next();
        if child.height != first.height
            || child.keys.first() != minimum
            || ceiling.is_some_and(|ceiling| child.max_key >= *ceiling)
        {
            return Err(Error::InvalidStructure);
        }
        minimum = ceiling;
        max_key = child.max_key;
    }
    let height = first.height.checked_add(1).ok_or(Error::InvalidStructure)?;
    Ok((height, max_key))
}

fn validate_next<'a>(
    page: &Page<'a>,
    prefix: &[ResidentNode<'a>],
) -> Result<ResidentNode<'a>, Error> {
    page.check()?;
    let ((height, max_key), contents) = if page.children.is_empty() {
        let maximum = *page.keys.last().ok_or(Error::InvalidStructure)?;
        ((0, maximum), ResidentContents::Leaf(page.values))
    } else {
        (
            summarize_branch(page, prefix)?,
            ResidentContents::Branch(page.children),
        )
    };
    Ok(ResidentNode {
        keys: page.keys,
        contents,
        height,
        max_key,
    })
}

impl<'a> Resident<'a> {
    pub(super) fn check(pages: &[Page<'a>], height: usize) -> Result<Self, Error> {
        let mut views = Vec::try_with_capacity(pages.len())?;
        for page in pages {
            // `views` contains one complete record for each preceding page.
            let node = validate_next(page, views.as_slice())?;
            views.try_push(node).map_err(|rejected| rejected.error)?;
        }
        let root = views.len().checked_sub(1).ok_or(Error::InvalidStructure)?;
        if views.last().ok_or(Error::InvalidStructure)?.height != height {
            return Err(Error::InvalidStructure);
        }
        Ok(Self {
            views,
            root,
            height,
        })
    }

    #[inline(always)]
    pub(super) fn get(&self, query: u64) -> Answer {
        let mut id = self.root;
        let mut remaining = self.height;
        while let Some(next_height) = remaining.checked_sub(1) {
            remaining = next_height;
            let node = self
                .views
                .as_slice()
                .get(id)
                .ok_or(Error::InvalidStructure)?;
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
        let node = self
            .views
            .as_slice()
            .get(id)
            .ok_or(Error::InvalidStructure)?;
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
    pub(super) fn leaves(&self) -> &[(Keys, Keys)] {
        self.leaves.as_slice()
    }

    pub(super) fn branches(&self) -> &[(Keys, Children)] {
        self.branches.as_slice()
    }

    #[inline(always)]
    pub(super) fn get<const N: usize>(&self, query: u64) -> Answer
    where
        Keys: Borrow<[u64; N]>,
        Children: Borrow<[usize; N]>,
    {
        lookup::<N, _, _>(
            self.branches.as_slice(),
            self.leaves.as_slice(),
            self.root,
            self.height,
            query,
        )
    }
}

#[inline(always)]
fn lookup<const N: usize, Keys: Borrow<[u64; N]>, Children: Borrow<[usize; N]>>(
    branches: &[(Keys, Children)],
    leaves: &[(Keys, Keys)],
    root: usize,
    height: usize,
    query: u64,
) -> Answer {
    let mut id = root;
    let mut remaining = height;
    while let Some(next_height) = remaining.checked_sub(1) {
        remaining = next_height;
        let branch_id = id
            .checked_sub(leaves.len())
            .ok_or(Error::InvalidStructure)?;
        let (keys, children) = branches.get(branch_id).ok_or(Error::InvalidStructure)?;
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
    let (keys, values) = leaves.get(id).ok_or(Error::InvalidStructure)?;
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

type SplitPools<'a, const N: usize> = Pools<&'a [u64; N], &'a [usize; N]>;
type PackedPools<const N: usize> = Pools<[u64; N], [usize; N]>;

impl<'a> Resident<'a> {
    /// Borrow fixed-width arrays from the same snapshot that passed validation.
    pub(super) fn split<const N: usize>(
        &self,
        leaf_count: usize,
    ) -> Result<SplitPools<'a, N>, Error> {
        let branch_count = self
            .views
            .len()
            .checked_sub(leaf_count)
            .ok_or(Error::InvalidStructure)?;
        let mut leaf_pool = Vec::try_with_capacity(leaf_count)?;
        let mut branch_pool = Vec::try_with_capacity(branch_count)?;
        for node in self.views.iter() {
            if leaf_pool.len() < leaf_count {
                let ResidentContents::Leaf(values) = node.contents else {
                    return Err(Error::InvalidStructure);
                };
                leaf_pool
                    .try_push((array(node.keys)?, array(values)?))
                    .map_err(|rejected| rejected.error)?;
            } else {
                let ResidentContents::Branch(children) = node.contents else {
                    return Err(Error::InvalidStructure);
                };
                branch_pool
                    .try_push((array(node.keys)?, array(children)?))
                    .map_err(|rejected| rejected.error)?;
            }
        }
        Ok(Pools {
            branches: branch_pool,
            leaves: leaf_pool,
            root: self.root,
            height: self.height,
        })
    }
}

fn array<T, const N: usize>(slice: &[T]) -> Result<&[T; N], Error> {
    slice.try_into().map_err(|_| Error::InvalidStructure)
}

pub(super) fn pack_pools<const N: usize>(
    split: &SplitPools<'_, N>,
) -> Result<PackedPools<N>, Error> {
    let mut branches = Vec::try_with_capacity(split.branches.len())?;
    for &(keys, children) in split.branches.iter() {
        branches
            .try_push((*keys, *children))
            .map_err(|rejected| rejected.error)?;
    }
    let mut leaves = Vec::try_with_capacity(split.leaves.len())?;
    for &(keys, values) in split.leaves.iter() {
        leaves
            .try_push((*keys, *values))
            .map_err(|rejected| rejected.error)?;
    }
    Ok(Pools {
        branches,
        leaves,
        root: split.root,
        height: split.height,
    })
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(5)]
    fn resident_publishes_only_a_complete_backward_prefix() {
        let child: usize = kani::any();
        let keys = [2];
        let children = [child];
        let pages = [
            Page {
                keys: &keys,
                values: &keys,
                children: &[],
            },
            Page {
                keys: &keys,
                values: &[],
                children: &children,
            },
        ];
        if let Ok(resident) = Resident::check(&pages, 1) {
            assert!(child == 0);
            assert!(resident.views.len() == pages.len());
            assert!(resident.root == 1);
        }
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn packed_branch_validation_rejects_invalid_child() {
        let left = [2, 4];
        let right = [6, 8];
        let minima = [2, 6];
        let child: usize = kani::any();
        let children = [0, child];
        let branch = Page {
            keys: &minima,
            values: &[],
            children: &children,
        };
        let prefix = [
            ResidentNode {
                keys: &left,
                contents: ResidentContents::Leaf(&left),
                height: 0,
                max_key: 4,
            },
            ResidentNode {
                keys: &right,
                contents: ResidentContents::Leaf(&right),
                height: kani::any(),
                max_key: kani::any(),
            },
        ];
        let expected = if child == 1 && prefix[1].height == 0 {
            Ok((1, prefix[1].max_key))
        } else {
            Err(Error::InvalidStructure)
        };
        let checked = validate_next(&branch, &prefix).map(|node| (node.height, node.max_key));
        assert!(checked == expected);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn packed_lookup_matches_exact_key_oracle() {
        let query = u64::from(kani::any::<u8>());
        let expected = match query {
            2 | 4 | 6 | 8 => Some(query * 3),
            _ => None,
        };
        let branches = [([2, 6], [0, 1])];
        let leaves = [([2, 4], [6, 12]), ([6, 8], [18, 24])];
        assert!(lookup::<2, _, _>(&branches, &leaves, 2, 1, query) == Ok(expected));
    }

    fn check_lookup_step<const N: usize>() {
        let branches = [(kani::any::<[u64; N]>(), kani::any::<[usize; N]>())];
        let leaves = [(kani::any::<[u64; N]>(), kani::any::<[u64; N]>())];
        let root = kani::any();
        let height = usize::from(kani::any::<bool>());
        let query = kani::any();
        let _ = lookup::<N, _, _>(&branches, &leaves, root, height, query);
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn packed_lookup_step_width_8() {
        check_lookup_step::<8>();
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn packed_lookup_step_width_16() {
        check_lookup_step::<16>();
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn packed_lookup_step_width_32() {
        check_lookup_step::<32>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::make_pages;
    use std::collections::BTreeMap;

    fn borrowed(pages: &[crate::Page<usize>]) -> std::vec::Vec<Page<'_>> {
        pages.iter().map(crate::Page::borrowed).collect()
    }

    fn compare<const N: usize>(height: u32) {
        let pages = make_pages::<usize>(N, height);
        let leaf_count = N.pow(height);
        let borrowed = borrowed(&pages);
        let resident = Resident::check(&borrowed, height as usize).unwrap();
        let split = resident.split::<N>(leaf_count).unwrap();
        let packed = pack_pools(&split).unwrap();
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
        let valid = |pages: &[crate::Page<usize>], height| {
            Resident::check(&borrowed(pages), height).is_ok()
        };
        assert!(Resident::check(&[], 0).is_err());
        assert!(!valid(&pages, 1));

        pages[4].children[0] = 4;
        assert!(!valid(&pages, 2));
        pages[4].children[0] = 5;
        assert!(!valid(&pages, 2));
        pages[4].children[0] = usize::MAX;
        assert!(!valid(&pages, 2));
        pages[4].children[0] = 0;

        pages[6].children[1] = 2;
        assert!(!valid(&pages, 2));
        pages[6].children[1] = 5;

        pages[4].keys[1] += 1;
        assert!(!valid(&pages, 2));
        pages[4].keys[1] -= 1;

        let ceiling = pages[2].keys[0];
        let old_maximum = core::mem::replace(&mut pages[1].keys[1], ceiling);
        assert!(!valid(&pages, 2));
        pages[1].keys[1] = old_maximum;
        assert!(valid(&pages, 2));
    }

    #[test]
    fn fixed_width_and_pool_boundary_are_checked() {
        let pages = make_pages::<usize>(2, 2);
        let borrowed = borrowed(&pages);
        let resident = Resident::check(&borrowed, 2).unwrap();
        assert!(resident.split::<3>(4).is_err());
        assert!(resident.split::<2>(3).is_err());
        assert!(resident.split::<2>(5).is_err());
        assert!(resident.split::<2>(usize::MAX).is_err());
        assert!(resident.split::<2>(4).is_ok());
    }
}
