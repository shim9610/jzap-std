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
                let ceiling = if upper < node.count {
                    Some(Self::strided_fence::<F>(node.key(upper)?)?)
                } else {
                    None
                };
                self.descend(id, child, minimum, ceiling)?;
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

    pub(super) struct Bytes;

    impl StridedFormat for Bytes {
        type Fence = [u8; 1];
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
        let keys = [[first], [second]];
        let values = [[a], [b]];
        let raw = StridedNode::<Bytes>::leaf(&page, 2, layout());
        let typed = super::super::Node::<_, _, u8>::leaf(&keys, &values);
        assert_eq!(raw.is_ok(), typed.is_ok());
        if let (Ok(raw), Ok(typed)) = (raw, typed) {
            let query = [query];
            let mut lookup = Lookup::new(0u8, 0, query.as_slice()).unwrap();
            let expected = lookup.provide(0, &typed).unwrap();
            let mut actual = Lookup::new(0u8, 0, query.as_slice()).unwrap();
            let found = actual.provide_strided(0, &raw).unwrap();
            match (expected, found) {
                (LookupStep::Found(k, v), LookupStep::Found(rk, rv)) => {
                    assert_eq!(k.as_slice(), rk);
                    assert_eq!(v.as_slice(), rv);
                }
                (LookupStep::Absent, LookupStep::Absent) => {}
                _ => panic!("raw and typed lookup disagree"),
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn branch_matches_typed_routing_and_fences() {
        let [first, second, child_minimum, child_last, query]: [u8; 5] = kani::any();
        kani::assume(first < second && child_minimum < child_last);
        let page = [0, first, 1, 0, second, 2];
        let raw = StridedNode::<Bytes>::branch(&page, 2, layout()).unwrap();
        let keys = [[first], [second]];
        let typed = super::super::Node::<_, (), u8>::branch(&keys, &[1, 2]).unwrap();
        let query = [query];
        let mut expected = Lookup::new(0u8, 1, query.as_slice()).unwrap();
        let mut actual = Lookup::new(0u8, 1, query.as_slice()).unwrap();
        let expected_id = match expected.provide(0, &typed).unwrap() {
            LookupStep::Absent => None,
            LookupStep::NeedNode(id) => Some(id),
            LookupStep::Found(..) => panic!("branch returned a value"),
        };
        assert_eq!(actual.requested_node(), Some(0));
        let result = actual.provide_strided(0, &raw);
        assert_eq!(
            result,
            Ok(expected_id.map_or(LookupStep::Absent, LookupStep::NeedNode))
        );
        assert_eq!(actual.requested_node(), expected_id);
        if let Some(id) = expected_id {
            let page = [0, child_minimum, 7, 0, child_last, 9];
            let raw_leaf = StridedNode::<Bytes>::leaf(&page, 2, layout()).unwrap();
            let keys = [[child_minimum], [child_last]];
            let values = [[7], [9]];
            let typed_leaf = super::super::Node::<_, _, u8>::leaf(&keys, &values).unwrap();
            let expected = expected.provide(id, &typed_leaf).map(|step| match step {
                LookupStep::Found(key, value) => {
                    LookupStep::Found(key.as_slice(), value.as_slice())
                }
                LookupStep::Absent => LookupStep::Absent,
                LookupStep::NeedNode(child) => LookupStep::NeedNode(child),
            });
            assert_eq!(actual.provide_strided(id, &raw_leaf), expected);
            if expected.is_err() {
                assert_eq!(actual.requested_node(), Some(id));
            }
        }
    }
}
