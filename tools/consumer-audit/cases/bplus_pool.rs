// Audits the exact benchmark pool source and the public checked B+ view.
use core::hint::black_box;
use jzap::{
    Error,
    collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node},
    },
    vec::Vec,
};

#[path = "../../throughput-analysis/src/bplus_pool.rs"]
mod pool;

fn check_width<const N: usize>(query: u64) -> Result<(), Error> {
    let mut keys = [[0; N]; N];
    let mut minima = [0; N];
    let mut children = [0; N];
    let mut next_key = 2u64;
    let mut next_child = 0usize;
    let mut limits = minima.iter_mut();
    let mut edges = children.iter_mut();
    for keys in &mut keys {
        *limits.next().ok_or(Error::InvalidStructure)? = next_key;
        *edges.next().ok_or(Error::InvalidStructure)? = next_child;
        next_child = next_child.checked_add(1).ok_or(Error::InvalidStructure)?;
        for key in keys {
            *key = next_key;
            next_key = next_key.checked_add(2).ok_or(Error::InvalidStructure)?;
        }
    }
    let count = N.checked_add(1).ok_or(Error::InvalidStructure)?;
    let mut pages = Vec::try_with_capacity(count)?;
    for keys in &keys {
        pages
            .try_push(pool::Page {
                keys,
                values: keys,
                children: &[],
            })
            .map_err(|rejected| rejected.error)?;
    }
    pages
        .try_push(pool::Page {
            keys: &minima,
            values: &[],
            children: &children,
        })
        .map_err(|rejected| rejected.error)?;

    let resident = pool::Resident::check(pages.as_slice(), 1)?;
    let split = resident.split::<N>(N)?;
    let packed = pool::pack_pools(&split)?;
    let mut lookup = Lookup::new(N, 1, &query)?;
    let branch = Node::<_, u64, _>::branch(&minima, &children)?;
    let expected = match lookup.provide(N, &branch)? {
        LookupStep::Absent => None,
        LookupStep::NeedNode(id) => {
            let keys = keys.get(id).ok_or(Error::InvalidStructure)?;
            let leaf = Node::leaf(keys, keys)?;
            match lookup.provide(id, &leaf)? {
                LookupStep::Found(_, value) => Some(*value),
                LookupStep::Absent => None,
                LookupStep::NeedNode(_) => return Err(Error::InvalidStructure),
            }
        }
        LookupStep::Found(_, _) => return Err(Error::InvalidStructure),
    };
    if resident.get(query)? != expected
        || split.get::<N>(query)? != expected
        || packed.get::<N>(query)? != expected
    {
        return Err(Error::InvalidStructure);
    }
    Ok(())
}

pub fn run(seed: usize) -> Result<(), Error> {
    let query = black_box(seed as u64);
    check_width::<8>(query)?;
    check_width::<16>(query)?;
    check_width::<32>(query)
}
