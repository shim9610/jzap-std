//! Matched point lookups over resident B+ nodes and a specialized Rust index.
#[cfg(feature = "bplus-memory")]
#[path = "../string_memory.rs"]
mod memory;
#[cfg(feature = "bplus-pool")]
#[path = "../bplus_pool.rs"]
mod pool;

use brie_tree::{BTree, nonmax::NonMaxU64};
use jzap::{
    Error,
    collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node},
    },
};
#[cfg(feature = "bplus-pool")]
use jzap::{collections::btree_map::view::bplus::FixedTree, vec::Vec as FallibleVec};
use std::{collections::BTreeMap, hint::black_box, time::Instant};

type WordNode<'a, I> = Node<'a, u64, u64, I>;
type Answer = Result<Option<u64>, Error>;

struct Page<I> {
    keys: Vec<u64>,
    values: Vec<u64>,
    children: Vec<I>,
}

impl<I> Page<I> {
    fn view(&self) -> Result<WordNode<'_, I>, Error> {
        if self.children.is_empty() {
            Node::leaf(&self.keys, &self.values)
        } else {
            Node::branch(&self.keys, &self.children)
        }
    }
}

#[cfg(feature = "bplus-pool")]
impl Page<usize> {
    fn borrowed(&self) -> pool::Page<'_> {
        pool::Page {
            keys: &self.keys,
            values: &self.values,
            children: &self.children,
        }
    }
}

fn make_pages<I: TryFrom<usize>>(fanout: usize, height: u32) -> Vec<Page<I>> {
    let leaves = fanout.pow(height);
    let mut pages = Vec::new();
    for leaf in 0..leaves {
        let keys: Vec<_> = (leaf * fanout..(leaf + 1) * fanout)
            .map(|i| i as u64 * 2 + 2)
            .collect();
        let values = keys.iter().map(|key| key * 3).collect();
        pages.push(Page {
            keys,
            values,
            children: Vec::new(),
        });
    }
    let mut previous = 0..leaves;
    while previous.len() > 1 {
        let start = pages.len();
        for first in previous.step_by(fanout) {
            let ids = first..first + fanout;
            let keys = ids.clone().map(|id| pages[id].keys[0]).collect();
            let children = ids
                .map(|id| I::try_from(id).ok().expect("node ID fits"))
                .collect();
            pages.push(Page {
                keys,
                values: Vec::new(),
                children,
            });
        }
        previous = start..pages.len();
    }
    pages
}

fn jzap_lookup<const VALIDATE: bool, I: Copy + Eq + TryFrom<usize>>(
    pages: &[Page<I>],
    views: &[WordNode<'_, I>],
    height: usize,
    query: u64,
) -> Answer
where
    usize: TryFrom<I>,
{
    let root_index = pages.len().checked_sub(1).ok_or(Error::InvalidStructure)?;
    let root = I::try_from(root_index).map_err(|_| Error::InvalidStructure)?;
    let mut lookup = Lookup::new(root, height, &query)?;
    let mut id = root;
    loop {
        let index = usize::try_from(id).map_err(|_| Error::InvalidStructure)?;
        let constructed;
        let node = if VALIDATE {
            constructed = pages.get(index).ok_or(Error::InvalidStructure)?.view()?;
            &constructed
        } else {
            views.get(index).ok_or(Error::InvalidStructure)?
        };
        match lookup.provide(id, node)? {
            LookupStep::Found(_, value) => return Ok(Some(*value)),
            LookupStep::Absent => return Ok(None),
            LookupStep::NeedNode(child) => id = child,
        }
    }
}

fn reference_lookup<const VALIDATE: bool>(
    pages: &[Page<usize>],
    mut height: usize,
    query: u64,
) -> Answer {
    let mut id = pages.len().checked_sub(1).ok_or(Error::InvalidStructure)?;
    let mut path = [id; usize::BITS as usize];
    let mut depth = 0usize;
    let mut minimum = None;
    let mut ceiling = None;
    if height >= path.len() {
        return Err(Error::InvalidStructure);
    }
    loop {
        let page = pages.get(id).ok_or(Error::InvalidStructure)?;
        let leaf = page.children.is_empty();
        if VALIDATE {
            let expected = if leaf {
                page.values.len()
            } else {
                page.children.len()
            };
            if page.keys.len() != expected
                || (!leaf && page.keys.is_empty())
                || page
                    .keys
                    .iter()
                    .zip(page.keys.iter().skip(1))
                    .any(|(a, b)| a >= b)
            {
                return Err(Error::InvalidStructure);
            }
        }
        if (height == 0) != leaf
            || minimum.is_some_and(|m| page.keys.first() != Some(&m))
            || ceiling.is_some_and(|c| page.keys.last().is_some_and(|key| *key >= c))
        {
            return Err(Error::InvalidStructure);
        }
        let rank = page.keys.binary_search(&query);
        if leaf {
            return match rank {
                Ok(i) => Ok(Some(*page.values.get(i).ok_or(Error::InvalidStructure)?)),
                Err(_) => Ok(None),
            };
        }
        let index = match rank {
            Ok(i) => i,
            Err(0) => return Ok(None),
            Err(i) => i - 1,
        };
        let next = *page.children.get(index).ok_or(Error::InvalidStructure)?;
        if next == id
            || path
                .get(..depth)
                .ok_or(Error::InvalidStructure)?
                .contains(&next)
        {
            return Err(Error::InvalidStructure);
        }
        *path.get_mut(depth).ok_or(Error::InvalidStructure)? = id;
        depth += 1;
        height -= 1;
        minimum = Some(*page.keys.get(index).ok_or(Error::InvalidStructure)?);
        ceiling = page.keys.get(index + 1).copied().or(ceiling);
        id = next;
    }
}

fn measure(
    get: &dyn Fn(u64) -> Answer,
    queries: &[u64],
    batches: usize,
) -> Result<(f64, u64), Error> {
    let start = Instant::now();
    let mut sum = 0u64;
    for _ in 0..batches {
        for &query in queries {
            sum = sum.wrapping_add(black_box(get)(black_box(query))?.unwrap_or(0));
        }
    }
    Ok((start.elapsed().as_secs_f64(), black_box(sum)))
}

fn run<const N: usize>(seed: u64, fanout: usize, height: u32) -> Result<(), Error> {
    let pages = make_pages::<usize>(fanout, height);
    #[cfg(feature = "bplus-pool")]
    if std::env::var_os("BPLUS_PREPARE").is_some() {
        let entries: Vec<_> = pages[..fanout.pow(height)]
            .iter()
            .flat_map(|page| page.keys.iter().copied().zip(page.values.iter().copied()))
            .collect();
        return measure_preparation::<N>(seed, height, &entries);
    }
    let views: Vec<_> = pages.iter().map(Page::view).collect::<Result<_, _>>()?;
    let pages_u32 = make_pages::<u32>(fanout, height);
    let views_u32: Vec<_> = pages_u32.iter().map(Page::view).collect::<Result<_, _>>()?;
    let mut std = BTreeMap::new();
    let mut brie = BTree::new();
    for page in &pages[..fanout.pow(height)] {
        for (&key, &value) in page.keys.iter().zip(&page.values) {
            std.insert(key, value);
            brie.insert(NonMaxU64::new(key).ok_or(Error::InvalidStructure)?, value);
        }
    }
    let mut random = seed;
    let queries: Vec<_> = (0..4096)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            random % (2 * std.len() as u64 + 4)
        })
        .collect();
    let height = height as usize;
    #[cfg(feature = "bplus-pool")]
    let borrowed_pages: Vec<_> = pages.iter().map(Page::borrowed).collect();
    #[cfg(feature = "bplus-pool")]
    let resident = pool::Resident::check(&borrowed_pages, height)?;
    #[cfg(feature = "bplus-pool")]
    let split = resident.split::<N>(fanout.pow(height as u32))?;
    #[cfg(feature = "bplus-pool")]
    let packed = pool::pack_pools(&split)?;
    #[cfg(feature = "bplus-pool")]
    let fixed = FixedTree::try_new(packed.leaves(), packed.branches())?;
    let methods: &[(&str, &dyn Fn(u64) -> Answer)] = &[
        ("jzap", &|q| {
            jzap_lookup::<false, _>(&pages, &views, height, q)
        }),
        ("checked_ref", &|q| {
            reference_lookup::<false>(&pages, height, q)
        }),
        ("jzap_validate", &|q| {
            jzap_lookup::<true, _>(&pages, &views, height, q)
        }),
        ("checked_ref_validate", &|q| {
            reference_lookup::<true>(&pages, height, q)
        }),
        ("brie", &|q| {
            Ok(brie
                .get(NonMaxU64::new(q).ok_or(Error::InvalidStructure)?)
                .copied())
        }),
        ("std", &|q| Ok(std.get(&q).copied())),
        ("jzap_u32", &|q| {
            jzap_lookup::<false, _>(&pages_u32, &views_u32, height, q)
        }),
        #[cfg(feature = "bplus-pool")]
        ("resident", &|q| resident.get(q)),
        #[cfg(feature = "bplus-pool")]
        ("split", &|q| split.get::<N>(q)),
        #[cfg(feature = "bplus-pool")]
        ("packed", &|q| packed.get::<N>(q)),
        #[cfg(feature = "bplus-pool")]
        ("fixed", &|q| fixed.get(&q).map(|value| value.copied())),
    ];
    for query in queries.iter().copied() {
        for &(name, method) in methods {
            if method(query)? != std.get(&query).copied() {
                eprintln!("lookup mismatch for {name}: {query}");
                return Err(Error::InvalidStructure);
            }
        }
    }
    #[cfg(feature = "bplus-memory")]
    for &(name, method) in methods {
        memory::start();
        let result = measure(method, &queries, 1);
        let stats = memory::stop();
        result?;
        println!(
            "{seed},{fanout},{height},{name},{},{},{},{},{}",
            stats.calls, stats.peak, stats.live, stats.actual_peak, stats.actual
        );
    }
    #[cfg(not(feature = "bplus-memory"))]
    {
        let requested = std::env::var(format!("BPLUS_BATCHES_{fanout}_{height}"))
            .ok()
            .map(|value| value.parse::<std::num::NonZeroUsize>())
            .transpose()
            .map_err(|_| Error::InvalidStructure)?;
        let batches = if let Some(requested) = requested {
            requested.get()
        } else {
            let pilot = 256;
            let mut fastest = f64::INFINITY;
            for &(_, method) in methods {
                fastest = fastest.min(measure(method, &queries, pilot)?.0);
            }
            (pilot as f64 * 1.5 / fastest).ceil() as usize
        };
        let operations = batches
            .checked_mul(queries.len())
            .ok_or(Error::InvalidStructure)?;
        let start = ((seed as usize % methods.len()) + fanout) % methods.len();
        for i in 0..methods.len() {
            let offset = if seed % 2 == 0 { methods.len() - i } else { i };
            let (name, method) = methods[(start + offset) % methods.len()];
            #[cfg(feature = "bplus-pool")]
            if !matches!(
                name,
                "jzap" | "brie" | "resident" | "split" | "packed" | "fixed"
            ) || (std::env::var_os("BPLUS_FIXED_ONLY").is_some()
                && !matches!(name, "jzap" | "brie" | "packed" | "fixed"))
            {
                continue;
            }
            let (elapsed, sum) = measure(method, &queries, batches)?;
            if elapsed < 1.0 {
                eprintln!("short interval for {name}");
                return Err(Error::InvalidStructure);
            }
            println!("{seed},{fanout},{height},{name},{elapsed:.9},{operations},{sum}");
        }
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    let seed = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<std::num::NonZeroU64>().ok())
        .ok_or(Error::InvalidStructure)?
        .get();
    eprintln!(
        "lookup_state_bytes={} lookup_u32_state_bytes={}",
        std::mem::size_of::<Lookup<'_, u64, usize>>(),
        std::mem::size_of::<Lookup<'_, u64, u32>>()
    );
    #[cfg(feature = "bplus-memory")]
    println!(
        "seed,fanout,height,implementation,alloc_calls,requested_peak,requested_final,actual_peak,actual_final"
    );
    #[cfg(feature = "bplus-pool")]
    if std::env::var_os("BPLUS_PREPARE").is_some() {
        println!("seed,fanout,height,implementation,seconds,operations,checksum");
        run::<8>(seed, 8, 2)?;
        run::<32>(seed, 32, 2)?;
        return run::<16>(seed, 16, 3);
    }
    #[cfg(not(feature = "bplus-memory"))]
    println!("seed,fanout,height,implementation,seconds,operations,checksum");
    #[cfg(feature = "bplus-pool")]
    {
        run::<8>(seed, 8, 2)?;
        run::<32>(seed, 32, 2)?;
        run::<16>(seed, 16, 3)?;
    }
    #[cfg(not(feature = "bplus-pool"))]
    for (fanout, height) in [(8, 2), (32, 2), (16, 3)] {
        run::<0>(seed, fanout, height)?;
    }
    Ok(())
}

// Bulk layout of the same sorted pairs supplied to brie-tree's insertion API.
#[cfg(feature = "bplus-pool")]
fn prepare_fixed<const N: usize>(entries: &[(u64, u64)]) -> Result<Option<u64>, Error> {
    let leaf_count = entries.len() / N;
    let mut leaves = FallibleVec::try_with_capacity(leaf_count)?;
    for entries in entries.chunks_exact(N) {
        let keys = std::array::from_fn(|i| entries[i].0);
        let values = std::array::from_fn(|i| entries[i].1);
        leaves
            .try_push((keys, values))
            .map_err(|rejected| rejected.error)?;
    }
    let mut branches = FallibleVec::try_with_capacity((leaf_count - 1) / (N - 1))?;
    let mut level_start = 0;
    let mut width = leaf_count;
    while width > 1 {
        let next_start = leaf_count + branches.len();
        for first in (level_start..level_start + width).step_by(N) {
            let children = std::array::from_fn(|i| first + i);
            let keys = std::array::from_fn(|i| {
                let child = children[i];
                if child < leaf_count {
                    leaves.as_slice()[child].0[0]
                } else {
                    let record: &([u64; N], [usize; N]) = &branches.as_slice()[child - leaf_count];
                    record.0[0]
                }
            });
            branches
                .try_push((keys, children))
                .map_err(|rejected| rejected.error)?;
        }
        level_start = next_start;
        width /= N;
    }
    let tree = FixedTree::try_new(leaves.as_slice(), branches.as_slice())?;
    black_box(&tree)
        .get(&black_box(2u64))
        .map(|item| item.copied())
}

#[cfg(feature = "bplus-pool")]
fn measure_preparation<const N: usize>(
    seed: u64,
    height: u32,
    entries: &[(u64, u64)],
) -> Result<(), Error> {
    let fixed = || prepare_fixed::<N>(black_box(entries));
    let brie = || {
        let mut tree = BTree::new();
        {
            let mut cursor = tree.cursor_mut();
            for &(key, value) in black_box(entries) {
                cursor.insert_before(NonMaxU64::new(key).ok_or(Error::InvalidStructure)?, value);
                cursor.next();
            }
        }
        Ok(black_box(&tree)
            .get(NonMaxU64::new(2).ok_or(Error::InvalidStructure)?)
            .copied())
    };
    let run = |method: &dyn Fn() -> Answer, batches| -> Result<_, Error> {
        let start = Instant::now();
        let mut checksum = 0u64;
        for _ in 0..batches {
            checksum = checksum.wrapping_add(black_box(method)()?.ok_or(Error::InvalidStructure)?);
        }
        Ok((start.elapsed().as_secs_f64(), black_box(checksum)))
    };
    let methods: [(&str, &dyn Fn() -> Answer); 2] = [("fixed", &fixed), ("brie", &brie)];
    let pilot = 256;
    let fastest = run(&fixed, pilot)?.0.min(run(&brie, pilot)?.0);
    let mut batches = (pilot as f64 * 1.35 / fastest).ceil() as usize;
    loop {
        let first = seed as usize % methods.len();
        let second = (first + 1) % methods.len();
        let first_run = run(methods[first].1, batches)?;
        let second_run = run(methods[second].1, batches)?;
        let shortest = first_run.0.min(second_run.0);
        if shortest < 1.0 {
            batches = (batches as f64 * 1.35 / shortest).ceil() as usize;
            continue;
        }
        for ((name, _), (seconds, checksum)) in
            [(methods[first], first_run), (methods[second], second_run)]
        {
            if checksum != 6 * batches as u64 {
                return Err(Error::InvalidStructure);
            }
            println!("{seed},{N},{height},{name},{seconds:.9},{batches},{checksum}");
        }
        return Ok(());
    }
}
