//! Partial-key B+ point lookups against the complete-key lookup and std.
//!
//! The tree uses varlit's disk index shape (50 leaf and 78 branch entries)
//! with 34-byte keys stored as a 16-byte inline prefix. Two key families:
//! `distinct` keys whose prefixes nearly always differ, and `shared` keys
//! whose first 26 bytes are common, so that every stored comparison needs the
//! full key. Every method is checked against `std::collections::BTreeMap`
//! before timing. Each interval lasts at least one second.
use jzap::{
    Error,
    collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node, PartialLookup, PartialNode, PartialStep, PrefixKey},
    },
};
use std::{cell::Cell, collections::BTreeMap, hint::black_box, time::Instant};

const P: usize = 16;
const LEAF: usize = 50;
const BRANCH: usize = 78;
const KEY: usize = 34;

type Answer = Result<Option<u64>, Error>;

struct Page<'k> {
    full: Vec<&'k [u8]>,
    values: Vec<u64>,
    children: Vec<usize>,
    attached: Vec<PrefixKey<'k, P>>,
    stored: Vec<PrefixKey<'k, P>>,
}

fn stored(full: &[u8]) -> PrefixKey<'_, P> {
    let mut prefix = [0; P];
    for (slot, byte) in prefix.iter_mut().zip(full) {
        *slot = *byte;
    }
    PrefixKey::new(prefix, full.len())
}

fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Sorted unique keys of the family, and absent keys of the same shape.
fn keys(family: &str, size: usize, seed: u64) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let make = |i: u64, state: &mut u64| -> Vec<u8> {
        let mut key = Vec::with_capacity(KEY);
        if family == "shared" {
            key.extend_from_slice(&[b'k'; KEY - 8]);
            key.extend_from_slice(&i.to_be_bytes());
        } else {
            while key.len() < KEY {
                key.extend_from_slice(&next(state).to_be_bytes());
            }
            key.truncate(KEY);
        }
        key
    };
    let mut state = seed;
    let mut present: Vec<Vec<u8>> = (0..size as u64)
        .map(|i| make(i * 2 + 2, &mut state))
        .collect();
    present.sort();
    present.dedup();
    let absent = (0..size as u64)
        .map(|i| make(i * 2 + 3, &mut state))
        .collect();
    (present, absent)
}

/// Pages bottom-up; returns (pages, root, height).
fn build<'k>(present: &'k [Vec<u8>]) -> (Vec<Page<'k>>, usize, usize) {
    let mut pages = Vec::new();
    let mut level: Vec<(usize, &'k [u8])> = Vec::new();
    for (chunk_index, chunk) in present.chunks(LEAF).enumerate() {
        let full: Vec<&[u8]> = chunk.iter().map(Vec::as_slice).collect();
        level.push((pages.len(), full[0]));
        pages.push(Page {
            attached: full.iter().map(|key| PrefixKey::from_full(key)).collect(),
            stored: full.iter().map(|key| stored(key)).collect(),
            values: (0..chunk.len())
                .map(|i| (chunk_index * LEAF + i) as u64 + 1)
                .collect(),
            children: Vec::new(),
            full,
        });
    }
    let mut height = 0;
    while level.len() > 1 {
        height += 1;
        let mut upper = Vec::new();
        for chunk in level.chunks(BRANCH) {
            let full: Vec<&[u8]> = chunk.iter().map(|(_, key)| *key).collect();
            upper.push((pages.len(), full[0]));
            pages.push(Page {
                attached: full.iter().map(|key| PrefixKey::from_full(key)).collect(),
                stored: full.iter().map(|key| stored(key)).collect(),
                values: Vec::new(),
                children: chunk.iter().map(|(id, _)| *id).collect(),
                full,
            });
        }
        level = upper;
    }
    (pages, level[0].0, height)
}

fn complete<const VALIDATE: bool>(
    pages: &[Page<'_>],
    views: &[Node<'_, &[u8], u64, usize>],
    root: usize,
    height: usize,
    query: &[u8],
) -> Answer {
    let mut lookup = Lookup::new(root, height, query)?;
    let mut id = root;
    loop {
        let constructed;
        let node = if VALIDATE {
            let page = pages.get(id).ok_or(Error::InvalidStructure)?;
            constructed = if page.children.is_empty() {
                Node::leaf(&page.full, &page.values)?
            } else {
                Node::branch(&page.full, &page.children)?
            };
            &constructed
        } else {
            views.get(id).ok_or(Error::InvalidStructure)?
        };
        match lookup.provide(id, node)? {
            LookupStep::Found(_, value) => return Ok(Some(*value)),
            LookupStep::Absent => return Ok(None),
            LookupStep::NeedNode(child) => id = child,
        }
    }
}

fn partial_attached(pages: &[Page<'_>], root: usize, height: usize, query: &[u8]) -> Answer {
    let mut lookup = PartialLookup::new(root, height, query)?;
    let mut id = root;
    loop {
        let page = pages.get(id).ok_or(Error::InvalidStructure)?;
        let node = if page.children.is_empty() {
            PartialNode::leaf(&page.attached, &page.values)?
        } else {
            PartialNode::branch(&page.attached, &page.children)?
        };
        // Attached separators keep only their prefix in the fence, so the
        // fence keys are passed in advance.
        let (minimum, ceiling) = lookup.fence_origins();
        let fence = |origin: Option<(usize, usize)>| -> Result<Option<&[u8]>, Error> {
            origin
                .map(|(node, index)| {
                    pages
                        .get(node)
                        .and_then(|page| page.full.get(index))
                        .copied()
                        .ok_or(Error::InvalidStructure)
                })
                .transpose()
        };
        match lookup.provide_fenced(id, &node, fence(minimum)?, fence(ceiling)?)? {
            PartialStep::Found(_, value) => return Ok(Some(*value)),
            PartialStep::Absent => return Ok(None),
            PartialStep::NeedNode(child) => id = child,
            PartialStep::NeedKey(_) | PartialStep::NeedFence => {
                return Err(Error::InvalidStructure);
            }
        }
    }
}

/// Copies each page's prefix-only keys into a stack buffer, as a caller
/// decoding a page would, and passes the fences on request. Without
/// `LISTED`, it attaches each requested key; with `LISTED`, it first attaches
/// every key `unresolved_keys` lists, in one round per node. `counts` records
/// attached keys, fence requests and resolution rounds.
fn partial_requests<const LISTED: bool>(
    pages: &[Page<'_>],
    root: usize,
    height: usize,
    query: &[u8],
    counts: &Cell<(u64, u64, u64)>,
) -> Answer {
    let mut lookup = PartialLookup::new(root, height, query)?;
    let mut id = root;
    let mut buffer = [PrefixKey::<P>::new([0; P], 0); BRANCH];
    loop {
        let page = pages.get(id).ok_or(Error::InvalidStructure)?;
        let keys = buffer
            .get_mut(..page.stored.len())
            .ok_or(Error::InvalidStructure)?;
        keys.copy_from_slice(&page.stored);
        if LISTED {
            let mut listed = [0usize; BRANCH];
            let mut count = 0usize;
            {
                let node = if page.children.is_empty() {
                    PartialNode::leaf(&*keys, &page.values)?
                } else {
                    PartialNode::branch(&*keys, &page.children)?
                };
                for index in lookup.unresolved_keys(&node) {
                    *listed.get_mut(count).ok_or(Error::InvalidStructure)? = index;
                    count += 1;
                }
            }
            for &index in listed.get(..count).ok_or(Error::InvalidStructure)? {
                let full = page.full.get(index).ok_or(Error::InvalidStructure)?;
                keys.get_mut(index)
                    .ok_or(Error::InvalidStructure)?
                    .attach(full)?;
            }
            if count > 0 {
                let (attached, fences, rounds) = counts.get();
                counts.set((attached + count as u64, fences, rounds + 1));
            }
        }
        let mut fenced = false;
        let step = loop {
            let node = if page.children.is_empty() {
                PartialNode::leaf(keys, &page.values)?
            } else {
                PartialNode::branch(keys, &page.children)?
            };
            let step = if fenced {
                let (minimum, ceiling) = lookup.fence_origins();
                let fence = |origin: Option<(usize, usize)>| -> Result<Option<&[u8]>, Error> {
                    origin
                        .map(|(node, index)| {
                            pages
                                .get(node)
                                .and_then(|page| page.full.get(index))
                                .copied()
                                .ok_or(Error::InvalidStructure)
                        })
                        .transpose()
                };
                lookup.provide_fenced(id, &node, fence(minimum)?, fence(ceiling)?)?
            } else {
                lookup.provide(id, &node)?
            };
            match step {
                PartialStep::NeedKey(index) => {
                    if LISTED {
                        return Err(Error::InvalidStructure);
                    }
                    let (attached, fences, rounds) = counts.get();
                    counts.set((attached + 1, fences, rounds + 1));
                    let full = page.full.get(index).ok_or(Error::InvalidStructure)?;
                    keys.get_mut(index)
                        .ok_or(Error::InvalidStructure)?
                        .attach(full)?;
                }
                PartialStep::NeedFence => {
                    let (attached, fences, rounds) = counts.get();
                    counts.set((attached, fences + 1, rounds));
                    fenced = true;
                }
                PartialStep::Found(_, value) => break Some(Ok(Some(*value))),
                PartialStep::Absent => break Some(Ok(None)),
                PartialStep::NeedNode(child) => {
                    id = child;
                    break None;
                }
            }
        };
        if let Some(answer) = step {
            return answer;
        }
    }
}

fn measure(
    get: &dyn Fn(&[u8]) -> Answer,
    queries: &[Vec<u8>],
    batches: usize,
) -> Result<(f64, u64), Error> {
    let start = Instant::now();
    let mut sum = 0u64;
    for _ in 0..batches {
        for query in queries {
            sum = sum.wrapping_add(black_box(get)(black_box(query))?.unwrap_or(0));
        }
    }
    Ok((start.elapsed().as_secs_f64(), black_box(sum)))
}

fn run(seed: u64, family: &str, size: usize) -> Result<(), Error> {
    let (present, absent) = keys(family, size, seed);
    let (pages, root, height) = build(&present);
    let views: Vec<_> = pages
        .iter()
        .map(|page| {
            if page.children.is_empty() {
                Node::leaf(&page.full, &page.values)
            } else {
                Node::branch(&page.full, &page.children)
            }
        })
        .collect::<Result<_, _>>()?;
    let std: BTreeMap<Vec<u8>, u64> = pages
        .iter()
        .filter(|page| page.children.is_empty())
        .flat_map(|page| {
            page.full
                .iter()
                .map(|key| key.to_vec())
                .zip(page.values.iter().copied())
        })
        .collect();
    let mut state = seed ^ 0x5151;
    let queries: Vec<Vec<u8>> = (0..4096)
        .map(|_| {
            let pick = next(&mut state) as usize;
            if pick % 2 == 0 {
                present[pick / 2 % present.len()].clone()
            } else {
                absent[pick / 2 % absent.len()].clone()
            }
        })
        .collect();
    let counts = Cell::new((0u64, 0u64, 0u64));
    let listed_counts = Cell::new((0u64, 0u64, 0u64));
    let methods: &[(&str, &dyn Fn(&[u8]) -> Answer)] = &[
        ("complete", &|q| {
            complete::<false>(&pages, &views, root, height, q)
        }),
        ("complete_validate", &|q| {
            complete::<true>(&pages, &views, root, height, q)
        }),
        ("partial_attached", &|q| {
            partial_attached(&pages, root, height, q)
        }),
        ("partial_requests", &|q| {
            partial_requests::<false>(&pages, root, height, q, &counts)
        }),
        ("partial_listed", &|q| {
            partial_requests::<true>(&pages, root, height, q, &listed_counts)
        }),
        ("std", &|q| Ok(std.get(q).copied())),
    ];
    for query in &queries {
        for &(name, method) in methods {
            if method(query)? != std.get(query.as_slice()).copied() {
                eprintln!("lookup mismatch for {name}: {query:?}");
                return Err(Error::InvalidStructure);
            }
        }
    }
    let lookups = queries.len() as f64;
    for (name, counts) in [
        ("partial_requests", &counts),
        ("partial_listed", &listed_counts),
    ] {
        let (attached, fences, rounds) = counts.get();
        eprintln!(
            "{seed},{family},{size},height={height},{name},attached_keys_per_lookup={:.3},key_rounds_per_lookup={:.3},fence_requests_per_lookup={:.3}",
            attached as f64 / lookups,
            rounds as f64 / lookups,
            fences as f64 / lookups
        );
    }
    let batches = match std::env::var(format!("BPLUS_PARTIAL_BATCHES_{family}_{size}")) {
        Ok(value) => value.parse().map_err(|_| Error::InvalidStructure)?,
        Err(_) => {
            let pilot = 64;
            let mut fastest = f64::INFINITY;
            for &(_, method) in methods {
                fastest = fastest.min(measure(method, &queries, pilot)?.0);
            }
            (pilot as f64 * 1.5 / fastest).ceil() as usize
        }
    };
    let operations = batches * queries.len();
    for i in 0..methods.len() {
        let offset = if seed % 2 == 0 {
            methods.len() - 1 - i
        } else {
            i
        };
        let (name, method) = methods[(offset + seed as usize) % methods.len()];
        if selected(name).is_none() {
            continue;
        }
        let (elapsed, sum) = measure(method, &queries, batches)?;
        // Instruction counting under Valgrind only; never a timing record.
        if elapsed < 1.0 && std::env::var_os("BPLUS_PARTIAL_INSTRUCTIONS").is_none() {
            eprintln!("short interval for {name}");
            return Err(Error::InvalidStructure);
        }
        println!("{seed},{family},{size},{name},{elapsed:.9},{operations},{sum}");
    }
    Ok(())
}

/// `BPLUS_PARTIAL_ONLY` (comma-separated names) limits the methods timed;
/// every method is still checked against std.
fn selected(name: &str) -> Option<()> {
    match std::env::var("BPLUS_PARTIAL_ONLY") {
        Ok(only) => only.split(',').any(|item| item == name).then_some(()),
        Err(_) => Some(()),
    }
}

fn main() -> Result<(), Error> {
    let seed = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<std::num::NonZeroU64>().ok())
        .ok_or(Error::InvalidStructure)?
        .get();
    println!("seed,family,size,implementation,seconds,operations,checksum");
    let families =
        std::env::var("BPLUS_PARTIAL_FAMILIES").unwrap_or_else(|_| "distinct,shared".into());
    let sizes = std::env::var("BPLUS_PARTIAL_SIZES").unwrap_or_else(|_| "2048,181267".into());
    for family in families.split(',') {
        for size in sizes.split(',') {
            run(
                seed,
                family,
                size.parse().map_err(|_| Error::InvalidStructure)?,
            )?;
        }
    }
    Ok(())
}
