//! Checked fixed-stride byte views for complete B+ keys.

use super::{Lookup, LookupStep};
use crate::Error;
use core::{borrow::Borrow, marker::PhantomData, num::NonZeroUsize, ops::Range};

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
            if last >= next {
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
    fn payload(&self, index: usize) -> Result<&'a [u8], Error> {
        self.entry(index)?
            .get(self.layout.payload.clone())
            .ok_or(Error::InvalidStructure)
    }

    #[inline]
    fn partition(&self, query: &[u8], inclusive: bool) -> Result<usize, Error> {
        let mut low = 0usize;
        let mut high = self.count;
        while low < high {
            let middle = low.midpoint(high);
            let order = self.key(middle)?.cmp(query);
            if order.is_lt() || (inclusive && order.is_eq()) {
                low = middle.checked_add(1).ok_or(Error::InvalidStructure)?;
            } else {
                high = middle;
            }
        }
        Ok(low)
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
            || (self.height == 0) != node.leaf
            || self
                .minimum
                .is_some_and(|min| node.count == 0 || node.first != min.borrow())
            || self
                .ceiling
                .is_some_and(|max| node.count != 0 && node.last >= max.borrow())
        {
            return Err(Error::InvalidStructure);
        }

        let result = if node.leaf {
            let index = node.partition(self.query, false)?;
            if index < node.count {
                let key = node.key(index)?;
                if key == self.query {
                    let value = node.payload(index)?;
                    self.pending = None;
                    return Ok(LookupStep::Found(key, value));
                }
            }
            LookupStep::Absent
        } else {
            let upper = node.partition(self.query, true)?;
            if let Some(index) = upper.checked_sub(1) {
                let child = F::child(node.payload(index)?)?;
                let minimum = Self::strided_fence::<F>(node.key(index)?)?;
                let first = [minimum];
                let pair;
                let minima = if upper < node.count {
                    pair = [minimum, Self::strided_fence::<F>(node.key(upper)?)?];
                    pair.as_slice()
                } else {
                    first.as_slice()
                };
                // The byte view has already selected the child, so pass the
                // selected minimum and optional next fence to the shared descent.
                self.descend::<()>(id, 0, minima, &[child])?;
                return Ok(LookupStep::NeedNode(child));
            }
            LookupStep::Absent
        };
        self.pending = None;
        Ok(result)
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
}
