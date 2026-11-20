// Strict optimized link root for strided branch and leaf lookup.
use core::{hint::black_box, num::NonZeroUsize};
use jzap::{
    Error,
    collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, StridedFormat, StridedLayout, StridedNode, StridedTree},
    },
};

struct ByteFormat<const WIDTH: usize = 1>;

impl<const WIDTH: usize> StridedFormat for ByteFormat<WIDTH> {
    const WORD_PREFIX_OFFSET: Option<usize> = Some(0);
    type Fence = [u8; WIDTH];
    type Id = usize;

    fn key(field: &[u8]) -> Result<&[u8], Error> {
        Ok(field)
    }

    fn copy_fence(key: &[u8]) -> Result<Self::Fence, Error> {
        key.try_into().map_err(|_| Error::InvalidStructure)
    }

    fn child(field: &[u8]) -> Result<Self::Id, Error> {
        field
            .first()
            .copied()
            .map(usize::from)
            .ok_or(Error::InvalidStructure)
    }
}

fn check_word_prefix(seed: usize) -> Result<(), Error> {
    let page = black_box([0, 1, 2, 3, 4, 5, 6, 7, 8, 42, 1, 1, 2, 3, 4, 5, 6, 7, 8, 43]);
    let layout = StridedLayout {
        stride: NonZeroUsize::new(10).ok_or(Error::InvalidStructure)?,
        key: 0..9,
        payload: 9..10,
    };
    let nodes = [StridedNode::<ByteFormat<9>>::leaf(&page, 2, layout)?];
    let snapshot = StridedTree::try_new(&nodes, Some(0), 0)?;
    let query = [(seed as u8) % 3, 1, 2, 3, 4, 5, 6, 7, 8];
    let expected = match query.first() {
        Some(0) => Some(42),
        Some(1) => Some(43),
        _ => None,
    };
    if snapshot
        .get(&query)?
        .and_then(|value| value.first().copied())
        != expected
    {
        return Err(Error::InvalidStructure);
    }
    Ok(())
}

pub fn run(seed: usize) -> Result<(), Error> {
    check_word_prefix(seed)?;
    let layout = StridedLayout {
        stride: NonZeroUsize::new(3).ok_or(Error::InvalidStructure)?,
        key: 1..2,
        payload: 2..3,
    };
    let query = [(seed as u8) % 15];
    let expected = match query {
        [1] => Some(17),
        [6] => Some(23),
        [8] => Some(31),
        [9] => Some(47),
        _ => None,
    };
    let mut lookup = Lookup::new(0, 1, query.as_slice())?;
    let left = black_box([0, 1, 17, 0, 6, 23]);
    let right = black_box([0, 8, 31, 0, 9, 47]);
    let branch = black_box([0, 1, 1, 0, 8, 2]);
    let root = StridedNode::<ByteFormat>::branch(&branch, 2, layout.clone())?;
    let nodes = [
        root,
        StridedNode::leaf(&left, 2, layout.clone())?,
        StridedNode::leaf(&right, 2, layout)?,
    ];
    let snapshot = StridedTree::try_new(&nodes, Some(0), 1)?;
    if snapshot
        .get(&query)?
        .and_then(|value| value.first().copied())
        != expected
    {
        return Err(Error::InvalidStructure);
    }
    let root = nodes.first().ok_or(Error::InvalidStructure)?;
    let child = match lookup.provide_strided(0, root)? {
        LookupStep::Absent if query == [0] => return Ok(()),
        LookupStep::NeedNode(child) => child,
        _ => return Err(Error::InvalidStructure),
    };
    let leaf = nodes.get(child).ok_or(Error::InvalidStructure)?;
    match lookup.provide_strided(child, leaf)? {
        LookupStep::Found(key, value) if key == query && value.first().copied() == expected => {
            Ok(())
        }
        LookupStep::Absent if expected.is_none() => Ok(()),
        _ => Err(Error::InvalidStructure),
    }
}
