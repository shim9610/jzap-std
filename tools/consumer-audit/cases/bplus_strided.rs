// Strict optimized link root for strided branch and leaf lookup.
use core::{hint::black_box, num::NonZeroUsize};
use jzap::{
    Error,
    collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, StridedFormat, StridedLayout, StridedNode},
    },
};

struct ByteFormat;

impl StridedFormat for ByteFormat {
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

pub fn run(seed: usize) -> Result<(), Error> {
    let layout = StridedLayout {
        stride: NonZeroUsize::new(3).ok_or(Error::InvalidStructure)?,
        key: 1..2,
        payload: 2..3,
    };
    let query = [(seed as u8) % 15];
    let mut lookup = Lookup::new(0, 1, query.as_slice())?;
    let branch = black_box([0, 1, 1, 0, 8, 2]);
    let root = StridedNode::<ByteFormat>::branch(&branch, 2, layout.clone())?;
    let child = match lookup.provide_strided(0, &root)? {
        LookupStep::Absent if query == [0] => return Ok(()),
        LookupStep::NeedNode(child) => child,
        _ => return Err(Error::InvalidStructure),
    };
    let entries = if child == 1 {
        black_box([0, 1, 17, 0, 6, 23])
    } else {
        black_box([0, 8, 31, 0, 9, 47])
    };
    let leaf = StridedNode::<ByteFormat>::leaf(&entries, 2, layout)?;
    let expected = match query {
        [1] => Some(17),
        [6] => Some(23),
        [8] => Some(31),
        [9] => Some(47),
        _ => None,
    };
    match lookup.provide_strided(child, &leaf)? {
        LookupStep::Found(key, value) if key == query && value.first().copied() == expected => {
            Ok(())
        }
        LookupStep::Absent if expected.is_none() => Ok(()),
        _ => Err(Error::InvalidStructure),
    }
}
