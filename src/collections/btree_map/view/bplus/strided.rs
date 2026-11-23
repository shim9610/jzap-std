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
    /// Offset of the first eight key bytes within the encoded key field.
    ///
    /// If decoded keys start at this offset in their fields, the view can
    /// compare a validated range of key bytes before decoding the full key.
    /// Differently encoded pages can still use the first eight bytes when
    /// they match every decoded key's prefix.
    const WORD_PREFIX_OFFSET: Option<usize> = None;

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
    prefix: Prefix,
    format: PhantomData<F>,
}

#[derive(Clone, Copy)]
enum Prefix {
    None,
    Short,
    Word,
    SharedShort,
    SharedWord,
}

enum KeyPosition<'a> {
    Found(usize, &'a [u8]),
    Gap(usize),
}

#[inline]
fn compare_bytes(left: &[u8], right: &[u8]) -> Ordering {
    ByteQuery::new(right).compare(left)
}

struct ByteQuery<'a> {
    bytes: &'a [u8],
    prefix: Option<(u64, &'a [u8])>,
}

impl<'a> ByteQuery<'a> {
    #[inline]
    fn new(bytes: &'a [u8]) -> Self {
        let prefix = bytes
            .split_first_chunk::<8>()
            .map(|(word, tail)| (u64::from_be_bytes(*word), tail));
        Self { bytes, prefix }
    }

    #[inline]
    fn compare(&self, key: &[u8]) -> Ordering {
        if let (Some((right, right_tail)), Some((left, left_tail))) =
            (self.prefix, key.split_first_chunk::<8>())
        {
            u64::from_be_bytes(*left)
                .cmp(&right)
                .then_with(|| left_tail.cmp(right_tail))
        } else {
            key.cmp(self.bytes)
        }
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
        let first_entry = entries.next();
        let first = first_entry
            .map(|entry| Self::read_key(entry, &layout))
            .transpose()?
            .unwrap_or_default();
        let mut last = first;
        let mut shortest = first.len();
        let mut contiguous = false;
        let mut words = false;
        if let (Some(entry), Some(offset)) = (first_entry, F::WORD_PREFIX_OFFSET) {
            let field = entry
                .get(layout.key.clone())
                .ok_or(Error::InvalidStructure)?;
            contiguous = Self::inline_matches(field, first, offset, 0);
            words = contiguous || Self::word_matches(field, first, offset);
        }
        for entry in entries {
            let field = entry
                .get(layout.key.clone())
                .ok_or(Error::InvalidStructure)?;
            let next = F::key(field)?;
            if compare_bytes(last, next).is_ge() {
                return Err(Error::InvalidStructure);
            }
            if let Some(offset) = F::WORD_PREFIX_OFFSET {
                contiguous = contiguous && Self::inline_matches(field, next, offset, 0);
                words = words && (contiguous || Self::word_matches(field, next, offset));
                shortest = shortest.min(next.len());
            }
            last = next;
        }
        // In a sorted node, equal endpoint prefixes also belong to
        // all intervening keys.
        let prefix = if contiguous
            && shortest >= 11
            && first.first_chunk::<8>() == last.first_chunk::<8>()
        {
            if shortest >= 16 {
                Prefix::SharedWord
            } else {
                Prefix::SharedShort
            }
        } else if words && shortest >= 8 {
            Prefix::Word
        } else if contiguous && shortest >= 3 {
            Prefix::Short
        } else {
            Prefix::None
        };
        Ok(Self {
            bytes,
            count,
            layout,
            first,
            last,
            leaf,
            prefix,
            format: PhantomData,
        })
    }

    #[inline]
    fn word_matches(field: &[u8], key: &[u8], offset: usize) -> bool {
        if let Some(prefix) = key.first_chunk::<8>() {
            field
                .get(offset..)
                .and_then(|inline| inline.first_chunk::<8>())
                == Some(prefix)
        } else {
            false
        }
    }

    #[inline]
    fn inline_matches(field: &[u8], key: &[u8], offset: usize, minimum: usize) -> bool {
        key.len() >= minimum
            && field
                .get(offset..)
                .and_then(|inline| inline.get(..key.len()))
                .is_some_and(|inline| core::ptr::eq(inline, key))
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
    fn field(&self, index: usize, range: Range<usize>) -> Result<&'a [u8], Error> {
        let stride = self.layout.stride.get();
        let offset = index.checked_mul(stride).ok_or(Error::InvalidStructure)?;
        let end = offset.checked_add(stride).ok_or(Error::InvalidStructure)?;
        self.bytes
            .get(offset..end)
            .and_then(|record| record.get(range))
            .ok_or(Error::InvalidStructure)
    }

    #[inline]
    fn key(&self, index: usize) -> Result<&'a [u8], Error> {
        F::key(self.field(index, self.layout.key.clone())?)
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
        self.field(index, self.layout.payload.clone())
    }

    #[inline(always)]
    fn search(&self, query: &ByteQuery<'_>) -> Result<KeyPosition<'a>, Error> {
        if let Some(offset) = F::WORD_PREFIX_OFFSET {
            match self.prefix {
                Prefix::None => {}
                Prefix::Short => return self.search_prefix::<3>(query.bytes, offset, 0),
                Prefix::Word => {
                    if let Some((word, tail)) = query.prefix {
                        return self.search_word(word, tail, offset);
                    }
                }
                Prefix::SharedShort | Prefix::SharedWord => {
                    let first = self
                        .first
                        .first_chunk::<8>()
                        .ok_or(Error::InvalidStructure)?;
                    let Some((word, tail)) = query.prefix else {
                        return Ok(KeyPosition::Gap(if query.bytes < first {
                            0
                        } else {
                            self.count
                        }));
                    };
                    match word.cmp(&u64::from_be_bytes(*first)) {
                        Ordering::Less => return Ok(KeyPosition::Gap(0)),
                        Ordering::Greater => return Ok(KeyPosition::Gap(self.count)),
                        Ordering::Equal => {
                            let start = offset.checked_add(8).ok_or(Error::InvalidStructure)?;
                            return if matches!(self.prefix, Prefix::SharedWord) {
                                self.search_prefix::<8>(tail, start, 8)
                            } else {
                                self.search_prefix::<3>(tail, start, 8)
                            };
                        }
                    }
                }
            }
        }
        self.search_by(self.count, |index| {
            let key = self.key(index)?;
            Ok((query.compare(key), key))
        })
    }

    #[inline(always)]
    fn search_word(&self, word: u64, tail: &[u8], offset: usize) -> Result<KeyPosition<'a>, Error> {
        self.search_by(self.count, |index| {
            let field = self.field(index, self.layout.key.clone())?;
            let prefix = field
                .get(offset..)
                .and_then(|bytes| bytes.first_chunk::<8>())
                .ok_or(Error::InvalidStructure)?;
            let order = u64::from_be_bytes(*prefix).cmp(&word);
            if order != Ordering::Equal {
                // An unequal prefix cannot produce a key in the result.
                return Ok((order, &[]));
            }
            let key = F::key(field)?;
            let suffix = key.get(8..).ok_or(Error::InvalidStructure)?;
            Ok((suffix.cmp(tail), key))
        })
    }

    #[inline(always)]
    fn prefix_at<const N: usize>(&self, index: usize, start: usize) -> Result<u64, Error> {
        // Construction validated count * stride and that this prefix fits
        // in every record. The search only supplies indices below count.
        let record = index
            .checked_mul(self.layout.stride.get())
            .ok_or(Error::InvalidStructure)?;
        let start = record.checked_add(start).ok_or(Error::InvalidStructure)?;
        let bytes = self.bytes.get(start..).ok_or(Error::InvalidStructure)?;
        let prefix = bytes.first_chunk::<N>().ok_or(Error::InvalidStructure)?;
        Ok(prefix_word(prefix))
    }

    #[inline(always)]
    fn search_prefix<const N: usize>(
        &self,
        query: &[u8],
        offset: usize,
        skip: usize,
    ) -> Result<KeyPosition<'a>, Error> {
        let Some((query_prefix, query_tail)) = query.split_first_chunk::<N>() else {
            return self.search_suffix(self.count, skip, query);
        };
        let query_prefix = prefix_word(query_prefix);
        let start = self
            .layout
            .key
            .start
            .checked_add(offset)
            .ok_or(Error::InvalidStructure)?;
        // Search for the rightmost prefix <= the query. Here count is
        // nonzero, and base + size never exceeds count, so each midpoint
        // and the last base stay in the record range.
        let mut base = 0usize;
        let mut size = self.count;
        while size > 1 {
            let half = size / 2;
            let middle = base.checked_add(half).ok_or(Error::InvalidStructure)?;
            base = if self.prefix_at::<N>(middle, start)? <= query_prefix {
                middle
            } else {
                base
            };
            size = size.checked_sub(half).ok_or(Error::InvalidStructure)?;
        }
        let after = |index: usize| {
            index
                .checked_add(1)
                .map(KeyPosition::Gap)
                .ok_or(Error::InvalidStructure)
        };
        match self.prefix_at::<N>(base, start)?.cmp(&query_prefix) {
            Ordering::Less => after(base),
            Ordering::Greater => Ok(KeyPosition::Gap(0)),
            Ordering::Equal => {
                let end = skip.checked_add(N).ok_or(Error::InvalidStructure)?;
                let key = self.key(base)?;
                let suffix = key.get(end..).ok_or(Error::InvalidStructure)?;
                match suffix.cmp(query_tail) {
                    Ordering::Less => after(base),
                    Ordering::Equal => Ok(KeyPosition::Found(base, key)),
                    // An earlier record may have the same packed prefix.
                    Ordering::Greater => self.search_suffix(base, skip, query),
                }
            }
        }
    }

    #[inline(never)]
    fn search_suffix(
        &self,
        upper: usize,
        skip: usize,
        query: &[u8],
    ) -> Result<KeyPosition<'a>, Error> {
        self.search_by(upper, |index| {
            let key = self.key(index)?;
            let tail = key.get(skip..).ok_or(Error::InvalidStructure)?;
            Ok((tail.cmp(query), key))
        })
    }

    #[inline(always)]
    fn search_by(
        &self,
        mut high: usize,
        mut compare: impl FnMut(usize) -> Result<(Ordering, &'a [u8]), Error>,
    ) -> Result<KeyPosition<'a>, Error> {
        let mut low = 0usize;
        while low < high {
            let middle = low.midpoint(high);
            match compare(middle)? {
                (Ordering::Less, _) => {
                    low = middle.checked_add(1).ok_or(Error::InvalidStructure)?
                }
                (Ordering::Greater, _) => high = middle,
                (Ordering::Equal, key) => return Ok(KeyPosition::Found(middle, key)),
            }
        }
        Ok(KeyPosition::Gap(low))
    }
}

#[inline(always)]
fn prefix_word<const N: usize>(bytes: &[u8; N]) -> u64 {
    if let Some(word) = bytes.first_chunk::<8>() {
        u64::from_be_bytes(*word)
    } else if let Some([a, b, c]) = bytes.first_chunk::<3>() {
        u64::from(u32::from_be_bytes([0, *a, *b, *c]))
    } else {
        0
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

        let position = node.search(&ByteQuery::new(self.query))?;
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

    pub(super) struct Bytes<const LEN: usize = 1, const HINT: bool = false>;

    impl<const LEN: usize, const HINT: bool> StridedFormat for Bytes<LEN, HINT> {
        const WORD_PREFIX_OFFSET: Option<usize> = if HINT { Some(0) } else { None };
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

#[cfg(any(test, kani))]
struct Encoded<const OFFSET: usize>;

#[cfg(any(test, kani))]
impl<const OFFSET: usize> StridedFormat for Encoded<OFFSET> {
    const WORD_PREFIX_OFFSET: Option<usize> = Some(OFFSET);
    type Fence = [u8; 9];
    type Id = usize;

    fn key(field: &[u8]) -> Result<&[u8], Error> {
        let (&length, bytes) = field.split_first().ok_or(Error::InvalidStructure)?;
        bytes
            .get(..usize::from(length))
            .ok_or(Error::InvalidStructure)
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

#[cfg(any(test, kani))]
fn encoded_layout() -> StridedLayout {
    StridedLayout {
        stride: NonZeroUsize::new(11).unwrap(),
        key: 0..10,
        payload: 10..11,
    }
}

#[cfg(any(test, kani))]
const ENCODED_PAGE: [u8; 22] = [
    9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 51, 9, 1, 2, 3, 4, 5, 6, 7, 8, 10, 52,
];

#[cfg(any(test, kani))]
fn shared_prefix_page() -> ([u8; 28], StridedLayout) {
    (
        [
            11, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0, 51, 12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11,
            10, 52,
        ],
        StridedLayout {
            stride: NonZeroUsize::new(14).unwrap(),
            key: 0..13,
            payload: 13..14,
        },
    )
}

#[cfg(any(test, kani))]
fn check_prefix_lookup(
    node: &StridedNode<'_, Encoded<1>>,
    fallback: &StridedNode<'_, Encoded<2>>,
    query: &[u8],
) {
    let query = ByteQuery::new(query);
    match (
        node.search(&query).unwrap(),
        fallback.search(&query).unwrap(),
    ) {
        (KeyPosition::Found(i, key), KeyPosition::Found(j, other)) => {
            assert_eq!(i, j);
            assert_eq!(key, other);
        }
        (KeyPosition::Gap(i), KeyPosition::Gap(j)) => assert_eq!(i, j),
        _ => panic!("validated prefix changed the search result"),
    }
}

#[cfg(test)]
mod tests {
    use super::{test_format::*, *};

    #[test]
    fn bplus_strided_shared_prefix_handles_shorter_keys_and_collisions() {
        let (page, layout) = shared_prefix_page();
        let node = StridedNode::<Encoded<1>>::leaf(&page, 2, layout.clone()).unwrap();
        let fallback = StridedNode::<Encoded<2>>::leaf(&page, 2, layout).unwrap();
        assert!(matches!(node.prefix, Prefix::SharedShort));
        assert!(matches!(fallback.prefix, Prefix::None));
        let mut query = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0];
        for len in 0..=query.len() {
            check_prefix_lookup(&node, &fallback, &query[..len]);
        }
        for byte in u8::MIN..=u8::MAX {
            query[11] = byte;
            check_prefix_lookup(&node, &fallback, &query);
        }
    }

    #[test]
    fn bplus_strided_uses_only_validated_prefixes() {
        let node = StridedNode::<Encoded<1>>::leaf(&ENCODED_PAGE, 2, encoded_layout()).unwrap();
        let fallback = StridedNode::<Encoded<2>>::leaf(&ENCODED_PAGE, 2, encoded_layout()).unwrap();
        assert!(matches!(node.prefix, Prefix::Word));
        assert!(matches!(fallback.prefix, Prefix::None));
        let query = [1, 2, 3, 4, 5, 6, 7, 8, 10];
        assert!(matches!(
            node.search(&ByteQuery::new(&query)),
            Ok(KeyPosition::Found(1, _))
        ));
        assert!(matches!(
            fallback.search(&ByteQuery::new(&query)),
            Ok(KeyPosition::Found(1, _))
        ));
        let mut short = ENCODED_PAGE;
        short[0] = 7;
        let mixed = StridedNode::<Encoded<1>>::leaf(&short, 2, encoded_layout()).unwrap();
        assert!(matches!(mixed.prefix, Prefix::Short));
        assert!(matches!(
            mixed.search(&ByteQuery::new(&query[..7])),
            Ok(KeyPosition::Found(0, _))
        ));
    }

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
        let branch = StridedNode::<Bytes<34, true>>::branch(&page, 2, layout.clone()).unwrap();
        assert!(matches!(branch.prefix, Prefix::SharedWord));
        assert_eq!(
            lookup.provide_strided(0, &branch),
            Ok(LookupStep::NeedNode(2))
        );
        page[35] = 55;
        let leaf = StridedNode::<Bytes<34, true>>::leaf(&page[36..], 1, layout).unwrap();
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
                _ => panic!("leaf returned another step"),
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
    #[kani::unwind(5)]
    fn snapshot_lookup_matches_known_arbitrary_page_order() {
        let nodes = tree::test_format::arbitrary_ids();
        let snapshot = tree::test_format::known_snapshot(&nodes);
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

    #[kani::proof]
    #[kani::unwind(5)]
    fn snapshot_enqueue_claims_only_unvisited_slots() {
        tree::test_format::check_enqueue(kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(11)]
    fn validated_word_hint_preserves_full_key_search() {
        let node = StridedNode::<Encoded<1>>::leaf(&ENCODED_PAGE, 2, encoded_layout()).unwrap();
        let fallback = StridedNode::<Encoded<2>>::leaf(&ENCODED_PAGE, 2, encoded_layout()).unwrap();
        assert!(matches!(node.prefix, Prefix::Word) && matches!(fallback.prefix, Prefix::None));
        let mut query = [1, 2, 3, 4, 5, 6, 7, 8, 0];
        // Cover a different common-word prefix and the entire shared suffix domain.
        query[0] = if kani::any() { 1 } else { kani::any() };
        query[8] = kani::any();
        check_prefix_lookup(&node, &fallback, &query);
    }

    #[kani::proof]
    #[kani::unwind(13)]
    fn shared_prefix_preserves_complete_key_and_gap() {
        let (page, layout) = shared_prefix_page();
        let node = StridedNode::<Encoded<1>>::leaf(&page, 2, layout.clone()).unwrap();
        let fallback = StridedNode::<Encoded<2>>::leaf(&page, 2, layout).unwrap();
        assert!(matches!(node.prefix, Prefix::SharedShort));
        assert!(matches!(fallback.prefix, Prefix::None));
        let mut query = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0];
        query[0] = if kani::any() { 1 } else { kani::any() };
        query[11] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= query.len());
        check_prefix_lookup(&node, &fallback, &query[..len]);
    }

    #[kani::proof]
    #[kani::unwind(11)]
    fn short_prefix_preserves_complete_key_and_gap() {
        let mut page = ENCODED_PAGE;
        page[0] = 7;
        let node = StridedNode::<Encoded<1>>::leaf(&page, 2, encoded_layout()).unwrap();
        let fallback = StridedNode::<Encoded<2>>::leaf(&page, 2, encoded_layout()).unwrap();
        assert!(matches!(node.prefix, Prefix::Short));
        assert!(matches!(fallback.prefix, Prefix::None));
        let mut query = [1, 2, 3, 4, 5, 6, 7, 8, 0];
        query[0] = if kani::any() { 1 } else { kani::any() };
        query[8] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= query.len());
        check_prefix_lookup(&node, &fallback, &query[..len]);
    }
}
