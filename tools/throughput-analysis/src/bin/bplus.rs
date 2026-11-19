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

fn measure(get: &dyn Fn(u64) -> Answer, queries: &[u64], batches: usize) -> (f64, u64) {
    let start = Instant::now();
    let mut sum = 0u64;
    for _ in 0..batches {
        for &query in queries {
            sum = sum.wrapping_add(black_box(get)(black_box(query)).unwrap().unwrap_or(0));
        }
    }
    (start.elapsed().as_secs_f64(), black_box(sum))
}

fn run<const N: usize>(seed: u64, fanout: usize, height: u32) {
    let pages = make_pages::<usize>(fanout, height);
    let views: Vec<_> = pages.iter().map(|page| page.view().unwrap()).collect();
    let pages_u32 = make_pages::<u32>(fanout, height);
    let views_u32: Vec<_> = pages_u32.iter().map(|page| page.view().unwrap()).collect();
    let mut std = BTreeMap::new();
    let mut brie = BTree::new();
    for page in &pages[..fanout.pow(height)] {
        for (&key, &value) in page.keys.iter().zip(&page.values) {
            std.insert(key, value);
            brie.insert(NonMaxU64::new(key).unwrap(), value);
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
    let resident = pool::Resident::check(&pages, height).unwrap();
    #[cfg(feature = "bplus-pool")]
    let split = resident.split::<N>(fanout.pow(height as u32)).unwrap();
    #[cfg(feature = "bplus-pool")]
    let packed = pool::pack_pools(&split);
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
            Ok(brie.get(NonMaxU64::new(q).unwrap()).copied())
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
    ];
    for query in queries.iter().copied() {
        for &(name, method) in methods {
            assert_eq!(
                method(query).unwrap(),
                std.get(&query).copied(),
                "{name}: {query}"
            );
        }
    }
    #[cfg(feature = "bplus-memory")]
    for &(name, method) in methods {
        memory::start();
        measure(method, &queries, 1);
        let stats = memory::stop();
        println!(
            "{seed},{fanout},{height},{name},{},{},{},{},{}",
            stats.calls, stats.peak, stats.live, stats.actual_peak, stats.actual
        );
    }
    #[cfg(not(feature = "bplus-memory"))]
    {
        let batches = std::env::var(format!("BPLUS_BATCHES_{fanout}_{height}"))
            .ok()
            .map(|value| value.parse::<usize>().expect("positive batch count"))
            .unwrap_or_else(|| {
                let pilot = 256;
                let fastest = methods
                    .iter()
                    .map(|(_, method)| measure(*method, &queries, pilot).0)
                    .fold(f64::INFINITY, f64::min);
                (pilot as f64 * 1.5 / fastest).ceil() as usize
            });
        let start = (seed as usize + fanout) % methods.len();
        for i in 0..methods.len() {
            let offset = if seed % 2 == 0 { methods.len() - i } else { i };
            let (name, method) = methods[(start + offset) % methods.len()];
            #[cfg(feature = "bplus-pool")]
            if !matches!(name, "jzap" | "brie" | "resident" | "split" | "packed") {
                continue;
            }
            let (elapsed, sum) = measure(method, &queries, batches);
            assert!(elapsed >= 1.0, "short interval for {name}");
            println!(
                "{seed},{fanout},{height},{name},{elapsed:.9},{},{sum}",
                batches * queries.len()
            );
        }
    }
}

fn main() {
    let seed = std::env::args()
        .nth(1)
        .expect("positive seed")
        .parse::<u64>()
        .unwrap();
    assert_ne!(seed, 0);
    eprintln!(
        "lookup_state_bytes={} lookup_u32_state_bytes={}",
        std::mem::size_of::<Lookup<'_, u64, usize>>(),
        std::mem::size_of::<Lookup<'_, u64, u32>>()
    );
    #[cfg(feature = "bplus-memory")]
    println!(
        "seed,fanout,height,implementation,alloc_calls,requested_peak,requested_final,actual_peak,actual_final"
    );
    #[cfg(not(feature = "bplus-memory"))]
    println!("seed,fanout,height,implementation,seconds,operations,checksum");
    #[cfg(feature = "bplus-pool")]
    {
        run::<8>(seed, 8, 2);
        run::<32>(seed, 32, 2);
        run::<16>(seed, 16, 3);
    }
    #[cfg(not(feature = "bplus-pool"))]
    for (fanout, height) in [(8, 2), (32, 2), (16, 3)] {
        run::<0>(seed, fanout, height);
    }
}
