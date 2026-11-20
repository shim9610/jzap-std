//! Checked fixed-stride byte views for complete B+ keys.

use super::{Lookup, LookupStep};
use crate::Error;
use core::{borrow::Borrow, cmp::Ordering, marker::PhantomData, num::NonZeroUsize, ops::Range};

mod tree;
pub use tree::StridedTree;

/// Entry layout relative to the beginning of each record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StridedLayout {
    pub stride: NonZeroUsize,
    pub key: Range<usize>,
    pub payload: Range<usize>,
}

/// Interprets the key and child fields of a fixed-stride page.
///
/// Implementations must consistently return the complete key in lexical byte
/// order. The key field may include metadata such as its inline length.
/// Decoding the same child field must always produce the same identifier.
/// The fence representation must preserve those bytes and their ordering.
/// For a reusable page buffer, the fence must own its bytes.
pub trait StridedFormat {
    type Fence: Copy + Ord + Borrow<[u8]>;
    type Id: Copy + Eq;

    fn key(field: &[u8]) -> Result<&[u8], Error>;
    fn copy_fence(key: &[u8]) -> Result<Self::Fence, Error>;
    fn child(field: &[u8]) -> Result<Self::Id, Error>;
}

/// A borrowed byte region with checked record bounds and strictly ordered keys.
///
/// Pass the page's entry region after its header. Unused bytes after the last
/// entry are ignored. The caller supplies the count and decides how to check
/// page identity, checksums, and payload semantics.
pub struct StridedNode<'a, F> {
    bytes: &'a [u8],
    count: usize,
    layout: StridedLayout,
    first: &'a [u8],
    last: &'a [u8],
    leaf: bool,
    format: PhantomData<F>,
}

enum KeyPosition<'a> {
    Found(usize, &'a [u8]),
    Gap(usize),
}

#[inline]
fn compare_bytes(left: &[u8], right: &[u8]) -> Ordering {
    if let (Some((l, l_tail)), Some((r, r_tail))) = (
        left.split_first_chunk::<8>(),
        right.split_first_chunk::<8>(),
    ) {
        u64::from_be_bytes(*l)
            .cmp(&u64::from_be_bytes(*r))
            .then_with(|| l_tail.cmp(r_tail))
    } else {
        left.cmp(right)
    }
}

impl<'a, F: StridedFormat> StridedNode<'a, F> {
    pub fn leaf(bytes: &'a [u8], count: usize, layout: StridedLayout) -> Result<Self, Error> {
        Self::new(bytes, count, layout, true)
    }

    pub fn branch(bytes: &'a [u8], count: usize, layout: StridedLayout) -> Result<Self, Error> {
        if count == 0 {
            return Err(Error::InvalidStructure);
        }
        Self::new(bytes, count, layout, false)
    }

    #[inline]
    fn new(
        bytes: &'a [u8],
        count: usize,
        layout: StridedLayout,
        leaf: bool,
    ) -> Result<Self, Error> {
        let stride = layout.stride.get();
        if layout.key.start > layout.key.end
            || layout.payload.start > layout.payload.end
            || layout.key.end > stride
            || layout.payload.end > stride
            || (!layout.key.is_empty()
                && !layout.payload.is_empty()
                && layout.key.start < layout.payload.end
                && layout.payload.start < layout.key.end)
        {
            return Err(Error::InvalidStructure);
        }
        let span = count.checked_mul(stride).ok_or(Error::InvalidStructure)?;
        let bytes = bytes.get(..span).ok_or(Error::InvalidStructure)?;
        let mut entries = bytes.chunks_exact(stride);
        let first = entries
            .next()
            .map(|entry| Self::read_key(entry, &layout))
            .transpose()?
            .unwrap_or_default();
        let mut last = first;
        for entry in entries {
            let next = Self::read_key(entry, &layout)?;
            if compare_bytes(last, next).is_ge() {
                return Err(Error::InvalidStructure);
            }
            last = next;
        }
        Ok(Self {
            bytes,
            count,
            layout,
            first,
            last,
            leaf,
            format: PhantomData,
        })
    }

    #[inline]
    fn read_key(entry: &'a [u8], layout: &StridedLayout) -> Result<&'a [u8], Error> {
        F::key(
            entry
                .get(layout.key.clone())
                .ok_or(Error::InvalidStructure)?,
        )
    }

    #[inline]
    fn entry(&self, index: usize) -> Result<&'a [u8], Error> {
        let stride = self.layout.stride.get();
        let offset = index.checked_mul(stride).ok_or(Error::InvalidStructure)?;
        let end = offset.checked_add(stride).ok_or(Error::InvalidStructure)?;
        self.bytes.get(offset..end).ok_or(Error::InvalidStructure)
    }

    #[inline]
    fn key(&self, index: usize) -> Result<&'a [u8], Error> {
        Self::read_key(self.entry(index)?, &self.layout)
    }

    #[inline]
    fn fits(&self, height: usize, minimum: Option<&[u8]>, ceiling: Option<&[u8]>) -> bool {
        (height == 0) == self.leaf
            && minimum.is_none_or(|min| self.count != 0 && self.first == min)
            && ceiling.is_none_or(|max| self.count == 0 || compare_bytes(self.last, max).is_lt())
    }

    #[inline]
    fn key_if_present(&self, index: usize) -> Result<Option<&'a [u8]>, Error> {
        (index < self.count).then(|| self.key(index)).transpose()
    }

    #[inline]
    fn payload(&self, index: usize) -> Result<&'a [u8], Error> {
        self.entry(index)?
            .get(self.layout.payload.clone())
            .ok_or(Error::InvalidStructure)
    }

    #[inline(always)]
    fn search(&self, query: &[u8]) -> Result<KeyPosition<'a>, Error> {
        let mut low = 0usize;
        let mut high = self.count;
        while low < high {
            let middle = low.midpoint(high);
            let key = self.key(middle)?;
            match compare_bytes(key, query) {
                Ordering::Less => {
                    low = middle.checked_add(1).ok_or(Error::InvalidStructure)?;
                }
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(KeyPosition::Found(middle, key)),
            }
        }
        Ok(KeyPosition::Gap(low))
    }
}

impl<K: Copy + Ord + Borrow<[u8]>, I: Copy + Eq> Lookup<'_, K, I, [u8]> {
    /// Search a checked byte view. A match borrows the original key and value
    /// fields. Only the selected child field is decoded. Errors leave the
    /// pending request available for retry.
    #[inline]
    pub fn provide_strided<'a, F: StridedFormat<Fence = K, Id = I>>(
        &mut self,
        id: I,
        node: &StridedNode<'a, F>,
    ) -> Result<LookupStep<'a, [u8], [u8], I>, Error> {
        if self.pending != Some(id)
            || !node.fits(
                self.height,
                self.minimum.as_ref().map(Borrow::borrow),
                self.ceiling.as_ref().map(Borrow::borrow),
            )
        {
            return Err(Error::InvalidStructure);
        }

        let position = node.search(self.query)?;
        if node.leaf {
            if let KeyPosition::Found(index, key) = position {
                let value = node.payload(index)?;
                self.pending = None;
                return Ok(LookupStep::Found(key, value));
            }
        } else {
            let selected = match position {
                KeyPosition::Found(index, key) => Some((index, key)),
                KeyPosition::Gap(upper) => upper
                    .checked_sub(1)
                    .map(|index| node.key(index).map(|key| (index, key)))
                    .transpose()?,
            };
            if let Some((index, key)) = selected {
                let child = F::child(node.payload(index)?)?;
                let minimum = Self::strided_fence::<F>(key)?;
                let first = [minimum];
                let pair;
                let upper = index.checked_add(1).ok_or(Error::InvalidStructure)?;
                let minima = if let Some(next) = node.key_if_present(upper)? {
                    pair = [minimum, Self::strided_fence::<F>(next)?];
                    pair.as_slice()
                } else {
                    first.as_slice()
                };
                // The selector has already chosen a child and its fences.
                self.descend::<()>(id, 0, minima, &[child])?;
                return Ok(LookupStep::NeedNode(child));
            }
        }
        self.pending = None;
        Ok(LookupStep::Absent)
    }

    #[inline]
    fn strided_fence<F: StridedFormat<Fence = K>>(key: &[u8]) -> Result<K, Error> {
        let fence = F::copy_fence(key)?;
        if fence.borrow() != key {
            return Err(Error::InvalidStructure);
        }
        Ok(fence)
    }
}

#[cfg(any(test, kani))]
mod test_format {
    use super::*;

    pub(super) struct Bytes<const LEN: usize = 1>;

    impl<const LEN: usize> StridedFormat for Bytes<LEN> {
        type Fence = [u8; LEN];
        type Id = u8;

        fn key(field: &[u8]) -> Result<&[u8], Error> {
            Ok(field)
        }

        fn copy_fence(key: &[u8]) -> Result<Self::Fence, Error> {
            key.try_into().map_err(|_| Error::InvalidStructure)
        }

        fn child(field: &[u8]) -> Result<Self::Id, Error> {
            field.first().copied().ok_or(Error::InvalidStructure)
        }
    }

    pub(super) fn layout() -> StridedLayout {
        StridedLayout {
            stride: NonZeroUsize::new(3).unwrap(),
            key: 1..2,
            payload: 2..3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_format::*, *};

    #[test]
    fn bplus_strided_lookup_reuses_buffer_and_rejects_damage() {
        let mut page = [0, 10, 1, 0, 20, 2, 0];
        let mut lookup = Lookup::new(0, 1, &[11][..]).unwrap();
        let root = StridedNode::<Bytes>::branch(&page, 2, layout()).unwrap();
        assert_eq!(
            lookup.provide_strided(0, &root),
            Ok(LookupStep::NeedNode(1))
        );
        page = [0, 10, 70, 0, 20, 71, 0];
        let bad_leaf = StridedNode::<Bytes>::leaf(&page, 2, layout()).unwrap();
        assert_eq!(
            lookup.provide_strided(1, &bad_leaf),
            Err(Error::InvalidStructure)
        );
        assert_eq!(lookup.requested_node(), Some(1));

        page = [0, 10, 70, 0, 11, 71, 0];
        let leaf = StridedNode::<Bytes>::leaf(&page, 2, layout()).unwrap();
        assert_eq!(
            lookup.provide_strided(2, &leaf),
            Err(Error::InvalidStructure)
        );
        assert_eq!(lookup.requested_node(), Some(1));
        let LookupStep::Found(key, value) = lookup.provide_strided(1, &leaf).unwrap() else {
            panic!("expected a hit");
        };
        assert!(core::ptr::eq(key, &page[4..5]));
        assert!(core::ptr::eq(value, &page[5..6]));
        assert_eq!(lookup.requested_node(), None);
    }

    #[test]
    fn bplus_strided_rejects_invalid_layout_and_cycles() {
        let page = [0, 10, 1, 0, 10, 1];
        assert!(StridedNode::<Bytes>::leaf(&page, 2, layout()).is_err());
        assert!(StridedNode::<Bytes>::leaf(&page, 3, layout()).is_err());
        assert!(StridedNode::<Bytes>::leaf(&page, usize::MAX, layout()).is_err());
        let mut bad_layout = layout();
        bad_layout.payload = 1..3;
        assert!(StridedNode::<Bytes>::leaf(&page, 1, bad_layout).is_err());
        let root = StridedNode::<Bytes>::branch(&page, 1, layout()).unwrap();
        let mut lookup = Lookup::new(1, 1, &[10][..]).unwrap();
        assert_eq!(
            lookup.provide_strided(1, &root),
            Err(Error::InvalidStructure)
        );
        assert_eq!(lookup.requested_node(), Some(1));
    }

    #[test]
    fn bplus_strided_searches_complete_long_keys() {
        let layout = StridedLayout {
            stride: NonZeroUsize::new(36).unwrap(),
            key: 1..35,
            payload: 35..36,
        };
        let first = [42; 34];
        let mut second = first;
        second[33] = 43;
        let mut page = [0; 72];
        page[1..35].copy_from_slice(&first);
        page[35] = 1;
        page[37..71].copy_from_slice(&second);
        page[71] = 2;

        let mut lookup = Lookup::new(0u8, 1, second.as_slice()).unwrap();
        let branch = StridedNode::<Bytes<34>>::branch(&page, 2, layout.clone()).unwrap();
        assert_eq!(
            lookup.provide_strided(0, &branch),
            Ok(LookupStep::NeedNode(2))
        );
        page[35] = 55;
        let leaf = StridedNode::<Bytes<34>>::leaf(&page[36..], 1, layout).unwrap();
        assert_eq!(
            lookup.provide_strided(2, &leaf),
            Ok(LookupStep::Found(second.as_slice(), &[2][..]))
        );
    }
}

#[cfg(kani)]
mod proofs {
    use super::{test_format::*, *};

    #[kani::proof]
    #[kani::unwind(5)]
    fn arbitrary_layout_stays_within_the_borrowed_page() {
        let bytes: [u8; 5] = kani::any();
        let Some(stride) = NonZeroUsize::new(kani::any()) else {
            return;
        };
        let count = kani::any();
        let layout = StridedLayout {
            stride,
            key: kani::any()..kani::any(),
            payload: kani::any()..kani::any(),
        };
        if let Ok(node) = StridedNode::<Bytes>::leaf(&bytes, count, layout.clone()) {
            assert_eq!(node.count.checked_mul(stride.get()), Some(node.bytes.len()));
            assert!(node.bytes.len() <= bytes.len());
            let index = kani::any();
            if let Ok(field) = node.payload(index) {
                assert!(index < count);
                assert_eq!(field.len(), layout.payload.end - layout.payload.start);
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn lookup_matches_two_complete_keys() {
        let [first, second, query, a, b]: [u8; 5] = kani::any();
        let page = [0, first, a, 0, second, b];
        let raw = StridedNode::<Bytes>::leaf(&page, 2, layout());
        assert_eq!(raw.is_ok(), first < second);
        if let Ok(raw) = raw {
            let query_bytes = [query];
            let mut actual = Lookup::new(0u8, 0, query_bytes.as_slice()).unwrap();
            let found = actual.provide_strided(0, &raw).unwrap();
            match found {
                LookupStep::Found(key, value) => {
                    assert_eq!(key, query_bytes);
                    assert_eq!(
                        value.first().copied(),
                        Some(if query == first { a } else { b })
                    );
                    assert!(query == first || query == second);
                }
                LookupStep::Absent => assert!(query != first && query != second),
                LookupStep::NeedNode(_) => panic!("leaf requested another node"),
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn branch_routes_and_preserves_fences() {
        let [first, second, query]: [u8; 3] = kani::any();
        kani::assume(first < second);
        let page = [0, first, 1, 0, second, 2];
        let raw = StridedNode::<Bytes>::branch(&page, 2, layout()).unwrap();
        let query_bytes = [query];
        let mut lookup = Lookup::new(0u8, 1, query_bytes.as_slice()).unwrap();
        let found = lookup.provide_strided(0, &raw).unwrap();
        if query < first {
            assert_eq!(found, LookupStep::Absent);
            assert_eq!(lookup.requested_node(), None);
        } else if query < second {
            assert_eq!(found, LookupStep::NeedNode(1));
            assert_eq!(lookup.requested_node(), Some(1));
            assert_eq!(lookup.minimum, Some([first]));
            assert_eq!(lookup.ceiling, Some([second]));
        } else {
            assert_eq!(found, LookupStep::NeedNode(2));
            assert_eq!(lookup.requested_node(), Some(2));
            assert_eq!(lookup.minimum, Some([second]));
            assert_eq!(lookup.ceiling, None);
        }
    }

    #[kani::proof]
    #[kani::unwind(11)]
    fn compare_long_byte_keys_matches_lexical_order() {
        let a: [u8; 9] = kani::any();
        let b: [u8; 9] = kani::any();
        let a_len = if kani::any() { 8 } else { 9 };
        let b_len = if kani::any() { 8 } else { 9 };
        let left = &a[..a_len];
        let right = &b[..b_len];
        assert_eq!(compare_bytes(left, right), left.cmp(right));
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn snapshot_lookup_matches_known_arbitrary_page_order() {
        let nodes = tree::test_format::arbitrary_ids();
        let snapshot = StridedTree::try_new(&nodes, Some(0), 1).unwrap();
        let query: u8 = kani::any();
        let expected = match query {
            5 => Some(&[51][..]),
            10 => Some(&[52][..]),
            20 => Some(&[71][..]),
            21 => Some(&[72][..]),
            _ => None,
        };
        assert_eq!(snapshot.get(&[query]), Ok(expected));
    }
}
