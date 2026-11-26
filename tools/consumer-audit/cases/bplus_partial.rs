// Strict optimized link root for the partial-key B+ lookup: key requests,
// fence requests and both result kinds over keys that share a prefix.
use core::hint::black_box;
use jzap::{
    Error,
    collections::btree_map::view::bplus::{PartialLookup, PartialNode, PartialStep, PrefixKey},
};

const P: usize = 2;

/// Keys stored as two inline bytes; the full keys stay outside the nodes.
fn stored(full: &[u8]) -> Result<PrefixKey<'_, P>, Error> {
    let first = full.first().copied().unwrap_or(0);
    let second = full.get(1).copied().unwrap_or(0);
    Ok(PrefixKey::new([first, second], full.len()))
}

/// Run one lookup, attaching each requested key and passing the fences on
/// request. `full` resolves (node, index) to the complete key.
fn search(
    full: &[[&[u8]; 2]; 3],
    query: &[u8],
    values: &[[u8; 2]; 3],
) -> Result<Option<u8>, Error> {
    let children = [1usize, 2];
    let mut lookup = PartialLookup::new(0usize, 1, query)?;
    // A root request and three leaf passes at most, plus fences.
    for _ in 0..4 {
        let Some(id) = lookup.requested_node() else {
            return Err(Error::InvalidStructure);
        };
        let keys = full.get(id).ok_or(Error::InvalidStructure)?;
        let mut stored_keys = [stored(keys[0])?, stored(keys[1])?];
        let mut fenced = false;
        for _ in 0..4 {
            let (minimum, ceiling) = lookup.fence_origins();
            let resolve = |origin: Option<(usize, usize)>| -> Result<Option<&[u8]>, Error> {
                origin
                    .map(|(node, index)| {
                        full.get(node)
                            .and_then(|keys| keys.get(index))
                            .copied()
                            .ok_or(Error::InvalidStructure)
                    })
                    .transpose()
            };
            let step = if id == 0 {
                let node = PartialNode::<_, u8, _>::branch(&stored_keys, &children)?;
                match lookup.provide(id, &node)? {
                    PartialStep::NeedNode(_) => break,
                    PartialStep::NeedKey(index) => Some(index),
                    PartialStep::Absent => return Ok(None),
                    _ => return Err(Error::InvalidStructure),
                }
            } else {
                let values = values.get(id).ok_or(Error::InvalidStructure)?;
                let node = PartialNode::leaf(&stored_keys, values)?;
                let step = if fenced {
                    lookup.provide_fenced(id, &node, resolve(minimum)?, resolve(ceiling)?)?
                } else {
                    lookup.provide(id, &node)?
                };
                match step {
                    PartialStep::Found(_, value) => return Ok(Some(*value)),
                    PartialStep::Absent => return Ok(None),
                    PartialStep::NeedKey(index) => Some(index),
                    PartialStep::NeedFence => {
                        fenced = true;
                        None
                    }
                    PartialStep::NeedNode(_) => return Err(Error::InvalidStructure),
                }
            };
            if let Some(index) = step {
                let key = stored_keys.get_mut(index).ok_or(Error::InvalidStructure)?;
                key.attach(keys.get(index).copied().ok_or(Error::InvalidStructure)?)?;
            }
        }
    }
    Err(Error::InvalidStructure)
}

pub fn run(seed: usize) -> Result<(), Error> {
    // Every key is longer than the two inline bytes and starts with "ab".
    let full: [[&[u8]; 2]; 3] = black_box([
        [b"abc", b"abm"],
        [b"abc", b"abd"],
        [b"abm", b"abn"],
    ]);
    let values = black_box([[0, 0], [10, 11], [20, 21]]);
    let queries: [&[u8]; 6] = [b"abc", b"abd", b"abm", b"abn", b"abe", b"aa"];
    let expected = [Some(10), Some(11), Some(20), Some(21), None, None];
    let pick = seed % queries.len();
    let query = queries.get(pick).copied().ok_or(Error::InvalidStructure)?;
    if search(&full, black_box(query), &values)? != expected.get(pick).copied().flatten() {
        return Err(Error::InvalidStructure);
    }
    // A child whose minimum differs from its separator after the prefix.
    let damaged: [[&[u8]; 2]; 3] = black_box([
        [b"abc", b"abm"],
        [b"abd", b"abe"],
        [b"abm", b"abn"],
    ]);
    match search(&damaged, b"abd", &values) {
        Err(Error::InvalidStructure) => Ok(()),
        _ => Err(Error::InvalidStructure),
    }
}
