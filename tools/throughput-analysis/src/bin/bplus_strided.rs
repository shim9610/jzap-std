//! Compare complete inline keys using Varlit's fixed-stride reuse-page geometry.
use jzap::{
    Error,
    collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node, StridedFormat, StridedLayout, StridedNode},
    },
};
use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::BTreeMap,
    hint::black_box,
    num::NonZeroUsize,
    time::{Duration, Instant},
};

const PAGE: usize = 4096;
const HEADER: usize = 32;
const KEY: usize = 28;
const LEAF_PAYLOAD: usize = 52;
const BRANCH_PAYLOAD: usize = 24;
const LEAF_STRIDE: usize = KEY + LEAF_PAYLOAD;
const BRANCH_STRIDE: usize = KEY + BRANCH_PAYLOAD;
const LEAF_MAX: usize = (PAGE - HEADER) / LEAF_STRIDE;
const BRANCH_MAX: usize = (PAGE - HEADER) / BRANCH_STRIDE;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct InlineKey {
    bytes: [u8; 16],
    len: usize,
}

impl Borrow<[u8]> for InlineKey {
    #[inline]
    fn borrow(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl Ord for InlineKey {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        <Self as Borrow<[u8]>>::borrow(self).cmp(other.borrow())
    }
}

impl PartialOrd for InlineKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct Format;

impl StridedFormat for Format {
    type Fence = InlineKey;
    type Id = usize;

    #[inline]
    fn key(field: &[u8]) -> Result<&[u8], Error> {
        let len = u32::from_le_bytes(
            field
                .get(8..12)
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::InvalidStructure)?,
        );
        let len = usize::try_from(len).map_err(|_| Error::InvalidStructure)?;
        let prefix = field.get(12..28).ok_or(Error::InvalidStructure)?;
        prefix
            .get(..len)
            .filter(|k| !k.is_empty())
            .ok_or(Error::InvalidStructure)
    }

    #[inline]
    fn copy_fence(key: &[u8]) -> Result<InlineKey, Error> {
        let mut bytes = [0; 16];
        bytes
            .get_mut(..key.len())
            .ok_or(Error::InvalidStructure)?
            .copy_from_slice(key);
        Ok(InlineKey {
            bytes,
            len: key.len(),
        })
    }

    #[inline]
    fn child(field: &[u8]) -> Result<usize, Error> {
        let bytes = field.first_chunk().ok_or(Error::InvalidStructure)?;
        let offset =
            usize::try_from(u64::from_le_bytes(*bytes)).map_err(|_| Error::InvalidStructure)?;
        Ok(offset / PAGE)
    }
}

type Page = [u8; PAGE];
type Answer = Result<Option<u8>, Error>;
type Read = fn(&Fixture, &[u8]) -> Answer;

struct Fixture {
    pages: Vec<Page>,
    oracle: BTreeMap<InlineKey, u8>,
}

fn layout(stride: usize) -> StridedLayout {
    StridedLayout {
        stride: NonZeroUsize::new(stride).unwrap(),
        key: 0..KEY,
        payload: KEY..stride,
    }
}

fn key_at(n: usize) -> InlineKey {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&(n as u64).to_be_bytes());
    bytes[8..].fill(37);
    InlineKey {
        bytes,
        len: 8 + (n / 2 % 9),
    }
}

fn write_key(entry: &mut [u8], key: InlineKey) {
    entry[8..12].copy_from_slice(&(key.len as u32).to_le_bytes());
    entry[12..12 + key.len].copy_from_slice(key.borrow());
}

fn begin_page(height: u8, count: usize) -> Page {
    let mut page = [0; PAGE];
    page[..8].copy_from_slice(b"VIXPAGE3");
    page[8] = height;
    page[10..12].copy_from_slice(&(count as u16).to_le_bytes());
    page
}

fn build(total: usize, leaf_width: usize, branch_width: usize) -> Fixture {
    let mut pages = Vec::new();
    let mut oracle = BTreeMap::new();
    let mut layer = Vec::new();
    let mut start = 0;
    while start < total {
        let count = (total - start).min(leaf_width - pages.len() % (leaf_width / 8 + 1));
        let mut page = begin_page(0, count);
        for (i, entry) in page[HEADER..]
            .chunks_exact_mut(LEAF_STRIDE)
            .take(count)
            .enumerate()
        {
            let key = key_at((start + i) * 2);
            let value = ((start + i) % 251 + 1) as u8;
            write_key(entry, key);
            entry[KEY..].fill(value);
            oracle.insert(key, value);
        }
        layer.push((key_at(start * 2), pages.len()));
        pages.push(page);
        start += count;
    }
    let mut height = 0;
    while layer.len() > 1 {
        height += 1;
        let mut parents = Vec::new();
        for chunk in layer.chunks(branch_width) {
            let mut page = begin_page(height, chunk.len());
            for (entry, &(key, child)) in page[HEADER..].chunks_exact_mut(BRANCH_STRIDE).zip(chunk)
            {
                write_key(entry, key);
                entry[KEY..KEY + 8].copy_from_slice(&((child * PAGE) as u64).to_le_bytes());
            }
            parents.push((chunk[0].0, pages.len()));
            pages.push(page);
        }
        layer = parents;
    }
    Fixture { pages, oracle }
}

#[inline]
fn entry_count(page: &Page) -> usize {
    usize::from(u16::from_le_bytes([page[10], page[11]]))
}

#[inline(never)]
fn direct(fixture: &Fixture, query: &[u8]) -> Answer {
    let (root, depth) = root(fixture)?;
    let mut lookup = Lookup::new(root, depth, query)?;
    let mut id = root;
    loop {
        let page = fixture.pages.get(id).ok_or(Error::InvalidStructure)?;
        let count = entry_count(page);
        let raw = if page[8] == 0 {
            StridedNode::<Format>::leaf(&page[HEADER..], count, layout(LEAF_STRIDE))?
        } else {
            StridedNode::<Format>::branch(&page[HEADER..], count, layout(BRANCH_STRIDE))?
        };
        match lookup.provide_strided(id, &raw)? {
            LookupStep::NeedNode(child) => id = child,
            LookupStep::Found(_, value) => return Ok(value.last().copied()),
            LookupStep::Absent => return Ok(None),
        }
    }
}

#[inline]
fn root(fixture: &Fixture) -> Result<(usize, usize), Error> {
    let root = fixture
        .pages
        .len()
        .checked_sub(1)
        .ok_or(Error::InvalidStructure)?;
    let page = fixture.pages.last().ok_or(Error::InvalidStructure)?;
    Ok((root, usize::from(page[8])))
}

#[inline(never)]
fn decoded<const COPY_VALUES: bool>(fixture: &Fixture, query: &[u8]) -> Answer {
    let (root, depth) = root(fixture)?;
    let mut lookup = Lookup::new(root, depth, query)?;
    let mut id = root;
    let mut keys = [InlineKey::default(); BRANCH_MAX];
    let mut children = [0; BRANCH_MAX];
    let mut values = [[0; LEAF_PAYLOAD]; LEAF_MAX];
    let mut refs: [&[u8]; LEAF_MAX] = [&[]; LEAF_MAX];
    loop {
        let page = fixture.pages.get(id).ok_or(Error::InvalidStructure)?;
        let count = entry_count(page);
        let step = if page[8] == 0 {
            let keys = keys.get_mut(..count).ok_or(Error::InvalidStructure)?;
            let entries = page[HEADER..].chunks_exact(LEAF_STRIDE).take(count);
            if COPY_VALUES {
                let values = values.get_mut(..count).ok_or(Error::InvalidStructure)?;
                for ((key, value), entry) in keys.iter_mut().zip(values.iter_mut()).zip(entries) {
                    *key = Format::copy_fence(Format::key(&entry[..KEY])?)?;
                    value.copy_from_slice(&entry[KEY..]);
                }
                match lookup.provide(id, &Node::leaf(keys, values)?)? {
                    LookupStep::Found(_, value) => return Ok(value.last().copied()),
                    LookupStep::Absent => return Ok(None),
                    LookupStep::NeedNode(_) => return Err(Error::InvalidStructure),
                }
            } else {
                let refs = refs.get_mut(..count).ok_or(Error::InvalidStructure)?;
                for ((key, value), entry) in keys.iter_mut().zip(refs.iter_mut()).zip(entries) {
                    *key = Format::copy_fence(Format::key(&entry[..KEY])?)?;
                    *value = &entry[KEY..];
                }
                match lookup.provide(id, &Node::leaf(keys, refs)?)? {
                    LookupStep::Found(_, value) => return Ok(value.last().copied()),
                    LookupStep::Absent => return Ok(None),
                    LookupStep::NeedNode(_) => return Err(Error::InvalidStructure),
                }
            }
        } else {
            let keys = keys.get_mut(..count).ok_or(Error::InvalidStructure)?;
            let children = children.get_mut(..count).ok_or(Error::InvalidStructure)?;
            let entries = page[HEADER..].chunks_exact(BRANCH_STRIDE).take(count);
            for ((key, child), entry) in keys.iter_mut().zip(children.iter_mut()).zip(entries) {
                *key = Format::copy_fence(Format::key(&entry[..KEY])?)?;
                *child = Format::child(&entry[KEY..])?;
            }
            lookup.provide(id, &Node::<_, (), _>::branch(keys, children)?)?
        };
        match step {
            LookupStep::NeedNode(child) => id = child,
            LookupStep::Absent => return Ok(None),
            LookupStep::Found(_, _) => return Err(Error::InvalidStructure),
        }
    }
}

#[inline(never)]
fn std_lookup(fixture: &Fixture, query: &[u8]) -> Answer {
    Ok(fixture.oracle.get(query).copied())
}

fn measure(
    fixture: &Fixture,
    queries: &[InlineKey],
    read: Read,
    batches: usize,
) -> (Duration, u64) {
    let mut sum = 0u64;
    let start = Instant::now();
    for _ in 0..batches {
        for query in queries {
            sum = sum.wrapping_add(u64::from(
                read(black_box(fixture), black_box(query.borrow()))
                    .unwrap()
                    .unwrap_or(0),
            ));
        }
    }
    (start.elapsed(), black_box(sum))
}

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn run_group(
    fixture: &Fixture,
    queries: &[InlineKey],
    methods: &[(&'static str, Read)],
    round: usize,
    case: usize,
) -> Vec<(&'static str, Duration, u64, usize)> {
    let shortest = methods
        .iter()
        .map(|&(_, read)| measure(fixture, queries, read, 48).0.as_secs_f64())
        .fold(f64::INFINITY, f64::min);
    let mut batches = (48.0 * 1.2 / shortest).ceil() as usize;
    let order = (round + case) % methods.len();
    loop {
        let mut rows = Vec::new();
        for i in 0..methods.len() {
            let index = if round.is_multiple_of(2) {
                (order + methods.len() - i) % methods.len()
            } else {
                (order + i) % methods.len()
            };
            let (name, read) = methods[index];
            let (elapsed, checksum) = measure(fixture, queries, read, batches);
            rows.push((name, elapsed, checksum, queries.len() * batches));
        }
        if rows.iter().any(|row| row.1 < Duration::from_secs(1)) {
            batches = (batches as f64 * 1.25).ceil() as usize;
            continue;
        }
        assert!(rows.iter().all(|row| row.2 == rows[0].2));
        return rows;
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().expect("seed").parse::<u64>().unwrap();
    let round = args.next().expect("run order").parse::<usize>().unwrap();
    let cases = [
        (2048, 27, 40),
        (2048, LEAF_MAX, BRANCH_MAX),
        (181_267, LEAF_MAX, BRANCH_MAX),
    ];
    let decoded_methods: [(&str, Read); 4] = [
        ("decoded_copy", decoded::<true>),
        ("decoded_borrow", decoded::<false>),
        ("direct", direct),
        ("decoded_control", decoded::<true>),
    ];
    let resident_methods: [(&str, Read); 3] = [
        ("std", std_lookup),
        ("std_control", std_lookup),
        ("direct", direct),
    ];
    for (case, (size, leaf, branch)) in cases.into_iter().enumerate() {
        let fixture = build(size, leaf, branch);
        let mut random = seed;
        let queries: Vec<_> = (0..4096)
            .map(|_| key_at(next(&mut random) as usize % (2 * size)))
            .collect();
        for query in &queries {
            let query = query.borrow();
            let expected = std_lookup(&fixture, query).unwrap();
            for (_, read) in decoded_methods {
                assert_eq!(read(&fixture, query).unwrap(), expected);
            }
        }
        let mut groups: [(&str, &[(&'static str, Read)]); 2] =
            [("page", &decoded_methods), ("resident", &resident_methods)];
        if (round + case).is_multiple_of(2) {
            groups.reverse();
        }
        for (group, methods) in groups {
            for (name, elapsed, checksum, operations) in
                run_group(&fixture, &queries, methods, round, case)
            {
                println!(
                    "{seed},{case},{size},{leaf},{branch},{},{group},{name},{:.9},{operations},{checksum}",
                    fixture.pages.last().unwrap()[8],
                    elapsed.as_secs_f64(),
                );
            }
        }
    }
}
