//! Equal-work default HashMap lifecycles, with separately named fixed-key controls.
#[cfg(feature = "btree-memory")]
#[path = "../string_memory.rs"]
mod memory;
#[path = "../ordered_corpus.rs"]
mod ordered_corpus;
use jzap::{
    TryClone, TryIterator,
    budget::{
        AllocationPolicy, Budget, ConcurrentBudget, ConcurrentMemoryBudget, MemoryBudget, Unmetered,
    },
    collections::HashMap,
    collections::hash_map::RandomState,
};
use ordered_corpus::{Case, corpus};
use std::{
    collections::HashMap as StdMap, hash::BuildHasherDefault, hint::black_box, time::Instant,
};
type Std = StdMap<u64, u64>;
type StdFixed = StdMap<u64, u64, BuildHasherDefault<std::collections::hash_map::DefaultHasher>>;
const OPS: [&str; 16] = [
    "build",
    "get",
    "update",
    "entry",
    "iter",
    "clone",
    "remove_cycle",
    "retain",
    "extract",
    "drain",
    "into_iter",
    "reserve",
    "shrink",
    "default_new",
    "random_state_new",
    "random_state_hash",
];
struct Context {
    local: MemoryBudget,
    concurrent: ConcurrentMemoryBudget,
}
impl Context {
    fn new() -> Self {
        Self {
            local: MemoryBudget::new(usize::MAX),
            concurrent: ConcurrentMemoryBudget::new(usize::MAX),
        }
    }
}
trait Policy<'a>: AllocationPolicy {
    fn choose(c: &'a Context) -> Self;
}
impl<'a> Policy<'a> for Unmetered {
    fn choose(_: &'a Context) -> Self {
        Self
    }
}
impl<'a> Policy<'a> for Budget<'a> {
    fn choose(c: &'a Context) -> Self {
        Self::Limited(&c.local)
    }
}
impl<'a> Policy<'a> for ConcurrentBudget<'a> {
    fn choose(c: &'a Context) -> Self {
        Self(&c.concurrent)
    }
}
trait Owner<'a, const FIXED: bool = false>: Sized {
    fn new(c: &'a Context) -> Self;
    fn insert(&mut self, k: u64, v: u64) -> Option<u64>;
    fn get(&self, k: u64) -> Option<u64>;
    fn entry(&mut self, k: u64) -> u64;
    fn sum(&self) -> u64;
    fn copy(&self) -> Self;
    fn remove(&mut self, k: u64) -> Option<u64>;
    fn retain(&mut self, p: u64);
    fn extract(&mut self, p: u64) -> u64;
    fn drain(&mut self) -> u64;
    fn consume(self) -> u64;
    fn reserve(&mut self, n: usize);
    fn shrink(&mut self);
    fn len(&self) -> usize;
    fn default_new(c: &'a Context) -> u64;
    /// Default key builder only, independent of the map and its policy.
    fn random_state_new() -> u64;
    fn random_state_hash(key: u64) -> u64;
}
impl<'a, S: std::hash::BuildHasher + Clone + Default, const FIXED: bool> Owner<'a, FIXED>
    for StdMap<u64, u64, S>
{
    fn new(_: &'a Context) -> Self {
        Self::default()
    }
    fn insert(&mut self, k: u64, v: u64) -> Option<u64> {
        StdMap::insert(self, k, v)
    }
    fn get(&self, k: u64) -> Option<u64> {
        StdMap::get(self, &k).copied()
    }
    fn entry(&mut self, k: u64) -> u64 {
        *StdMap::entry(self, k).or_insert(k * 3)
    }
    fn sum(&self) -> u64 {
        self.iter().fold(0, |s, (k, v)| s.wrapping_add(k ^ v))
    }
    fn copy(&self) -> Self {
        self.clone()
    }
    fn remove(&mut self, k: u64) -> Option<u64> {
        StdMap::remove(self, &k)
    }
    fn retain(&mut self, p: u64) {
        StdMap::retain(self, |k, _| !(*k ^ p).is_multiple_of(3))
    }
    fn extract(&mut self, p: u64) -> u64 {
        self.extract_if(|k, _| (*k ^ p).is_multiple_of(3))
            .fold(0, |s, (k, v)| s.wrapping_add(k ^ v))
    }
    fn drain(&mut self) -> u64 {
        StdMap::drain(self).fold(0, |s, (k, v)| s.wrapping_add(k ^ v))
    }
    fn consume(self) -> u64 {
        self.into_iter().fold(0, |s, (k, v)| s.wrapping_add(k ^ v))
    }
    fn reserve(&mut self, n: usize) {
        StdMap::reserve(self, n)
    }
    fn shrink(&mut self) {
        self.shrink_to_fit()
    }
    fn len(&self) -> usize {
        StdMap::len(self)
    }
    fn default_new(_: &'a Context) -> u64 {
        let m = StdMap::<u64, u64>::new();
        black_box(&m);
        black_box(m.len()) as u64
    }
    fn random_state_new() -> u64 {
        black_box(&std::hash::RandomState::new());
        1
    }
    fn random_state_hash(key: u64) -> u64 {
        use std::hash::BuildHasher;
        // Keys differ between implementations, so the hash is consumed, not compared.
        black_box(std::hash::RandomState::new().hash_one(key));
        1
    }
}
impl<'a, P: Policy<'a>, const FIXED: bool> Owner<'a, FIXED> for HashMap<u64, u64, RandomState, P> {
    fn new(c: &'a Context) -> Self {
        if FIXED {
            Self::with_hasher_in(RandomState::from_seed(0, 0), P::choose(c))
        } else {
            Self::try_new_in(P::choose(c)).unwrap()
        }
    }
    fn insert(&mut self, k: u64, v: u64) -> Option<u64> {
        self.try_insert(k, v).unwrap()
    }
    fn get(&self, k: u64) -> Option<u64> {
        HashMap::get(self, &k).copied()
    }
    fn entry(&mut self, k: u64) -> u64 {
        *HashMap::entry(self, k).try_or_insert(k * 3).unwrap()
    }
    fn sum(&self) -> u64 {
        self.iter().fold(0, |s, (k, v)| s.wrapping_add(k ^ v))
    }
    fn copy(&self) -> Self {
        self.try_clone().unwrap()
    }
    fn remove(&mut self, k: u64) -> Option<u64> {
        HashMap::remove(self, &k)
    }
    fn retain(&mut self, p: u64) {
        self.try_retain(|k, _| Ok::<_, ()>(!(*k ^ p).is_multiple_of(3)))
            .unwrap()
    }
    fn extract(&mut self, p: u64) -> u64 {
        let mut it =
            self.try_extract_if(|k: &u64, _: &mut u64| Ok::<_, ()>((*k ^ p).is_multiple_of(3)));
        let mut s = 0u64;
        while let Some((k, v)) = it.try_next().unwrap() {
            s = s.wrapping_add(k ^ v)
        }
        s
    }
    fn drain(&mut self) -> u64 {
        HashMap::drain(self).fold(0, |s, (k, v)| s.wrapping_add(k ^ v))
    }
    fn consume(self) -> u64 {
        self.into_iter().fold(0, |s, (k, v)| s.wrapping_add(k ^ v))
    }
    fn reserve(&mut self, n: usize) {
        self.try_reserve(n).unwrap()
    }
    fn shrink(&mut self) {
        self.try_shrink_to_fit().unwrap()
    }
    fn len(&self) -> usize {
        HashMap::len(self)
    }
    fn default_new(c: &'a Context) -> u64 {
        let m = Self::try_new_in(P::choose(c)).unwrap();
        black_box(&m);
        black_box(m.len()) as u64
    }
    fn random_state_new() -> u64 {
        black_box(&RandomState::try_new().unwrap());
        1
    }
    fn random_state_hash(key: u64) -> u64 {
        use std::hash::BuildHasher;
        black_box(RandomState::try_new().unwrap().hash_one(key));
        1
    }
}
fn epoch<'a, T: Owner<'a, FIXED>, const OP: usize, const FIXED: bool>(
    cases: &[Case],
    c: &'a Context,
    sources: &mut [T],
    n: u64,
) -> u64 {
    let mut sum = 0u64;
    for _ in 0..n {
        for (case, source) in cases.iter().zip(sources.iter_mut()) {
            let value = match OP {
                0 => {
                    let mut m = T::new(c);
                    for k in &case.keys {
                        black_box(m.insert(black_box(*k), k * 3));
                    }
                    black_box(m.len()) as u64
                }
                1 => {
                    let mut s = 0u64;
                    for k in &case.keys {
                        s = s.wrapping_add(black_box(source.get(black_box(*k))).unwrap_or(0));
                        s = s.wrapping_add(black_box(source.get(black_box(k | 1))).unwrap_or(0));
                    }
                    s
                }
                2 => {
                    let mut s = 0u64;
                    for k in &case.keys {
                        s = s.wrapping_add(
                            black_box(source.insert(black_box(*k), k * 3)).unwrap_or(0),
                        );
                    }
                    s
                }
                3 => {
                    let mut s = 0u64;
                    for k in &case.keys {
                        s = s.wrapping_add(black_box(source.entry(black_box(*k))));
                    }
                    s
                }
                4 => source.sum(),
                6 => {
                    let mut s = 0u64;
                    for k in &case.keys {
                        s = s.wrapping_add(black_box(source.remove(black_box(*k))).unwrap_or(0));
                        black_box(source.insert(black_box(*k), k * 3));
                    }
                    s
                }
                13 => T::default_new(c),
                14 => T::random_state_new(),
                15 => T::random_state_hash(black_box(case.pivot)),
                _ => {
                    let mut m = source.copy();
                    let s = match OP {
                        7 => {
                            m.retain(black_box(case.pivot));
                            m.len() as u64
                        }
                        8 => m.extract(black_box(case.pivot)),
                        9 => m.drain(),
                        10 => {
                            sum = sum.wrapping_add(black_box(m.consume()));
                            continue;
                        }
                        11 => {
                            m.reserve(case.keys.len() + 1);
                            m.len() as u64
                        }
                        12 => {
                            m.reserve(case.keys.len() + 1);
                            m.shrink();
                            m.len() as u64
                        }
                        _ => m.len() as u64,
                    };
                    black_box(&m);
                    s
                }
            };
            sum = sum.wrapping_add(black_box(value));
        }
    }
    sum
}
fn run<'a, T: Owner<'a, FIXED>, const OP: usize, const FIXED: bool>(
    cases: &[Case],
    c: &'a Context,
    n: u64,
) -> (f64, u64) {
    let mut sources = cases
        .iter()
        .map(|case| {
            let mut m = T::new(c);
            for k in &case.keys {
                m.insert(*k, k * 3);
            }
            m
        })
        .collect::<Vec<_>>();
    black_box(epoch::<T, OP, FIXED>(cases, c, &mut sources, 2));
    let start = Instant::now();
    let sum = epoch::<T, OP, FIXED>(cases, c, &mut sources, n);
    let elapsed = start.elapsed().as_secs_f64();
    black_box(sources);
    (elapsed, sum)
}
macro_rules! dispatch_mode {
    ($op:expr,$label:expr,$cases:expr,$c:expr,$n:expr,$std:ty,$fixed:literal) => {
        match $label {
            "std" | "std_control" => run::<$std, $op, $fixed>($cases, $c, $n),
            "unlimited" => run::<HashMap<u64, u64>, $op, $fixed>($cases, $c, $n),
            "local" => {
                run::<HashMap<u64, u64, RandomState, Budget<'_>>, $op, $fixed>($cases, $c, $n)
            }
            "concurrent" => {
                run::<HashMap<u64, u64, RandomState, ConcurrentBudget<'_>>, $op, $fixed>(
                    $cases, $c, $n,
                )
            }
            _ => panic!("unknown label"),
        }
    };
}
macro_rules! dispatch {
    ($op:expr,$label:expr,$cases:expr,$c:expr,$n:expr,$fixed:expr) => {
        if $fixed {
            dispatch_mode!($op, $label, $cases, $c, $n, StdFixed, true)
        } else {
            dispatch_mode!($op, $label, $cases, $c, $n, Std, false)
        }
    };
}
#[cfg(feature = "btree-memory")]
fn memory_for<'a, T: Owner<'a>>(label: &str, cases: &[Case], c: &'a Context, seed: u64) {
    for (ci, case) in cases.iter().enumerate() {
        for op in [0, 5, 7, 8, 9, 10, 11, 12] {
            memory::start();
            let mut m = T::new(c);
            for k in &case.keys {
                m.insert(*k, k * 3);
            }
            let before = memory::snapshot();
            let mut extra = None;
            match op {
                5 => extra = Some(m.copy()),
                7 => m.retain(case.pivot),
                8 => {
                    black_box(m.extract(case.pivot));
                }
                9 => {
                    black_box(m.drain());
                }
                10 => {
                    let old = std::mem::replace(&mut m, T::new(c));
                    black_box(old.consume());
                }
                11 => m.reserve(case.keys.len() + 1),
                12 => {
                    m.reserve(case.keys.len() + 1);
                    m.shrink();
                }
                _ => {}
            }
            let held = memory::snapshot();
            drop((m, extra));
            let end = memory::stop();
            assert_eq!((end.live, end.actual), (0, 0));
            println!(
                "{{\"label\":\"{label}\",\"seed\":{seed},\"case\":{ci},\"operation\":\"{}\",\"size\":{},\"calls\":{},\"held\":{},\"peak\":{},\"actual_held\":{},\"actual_peak\":{},\"handle\":{},\"before\":{}}}",
                OPS[op],
                case.keys.len(),
                held.calls,
                held.live,
                held.peak,
                held.actual,
                held.actual_peak,
                std::mem::size_of::<T>(),
                before.live
            );
        }
    }
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        Some("tasks") => {
            println!(
                "{}",
                OPS.iter()
                    .map(|op| format!("hash_map_{op}"))
                    .chain(
                        ["build", "get", "clone", "iter"].map(|op| format!("hash_map_fixed_{op}"))
                    )
                    .collect::<Vec<_>>()
                    .join(",")
            );
            return;
        }
        Some("describe") => {
            println!(
                "{{\"operation_unit\":\"get counts two lookups per key; update/entry count one key; remove_cycle counts one remove+insert pair; default_new counts one empty constructor+drop; random_state_new counts one default key builder; random_state_hash counts one builder plus one u64 hash_one; others count a complete workload. Destructive batches include clone preparation; reserve includes clone+growth, shrink includes clone+growth+shrink. Source preparation and corpus generation are outside timing.\",\"inputs\":\"64 seeded shuffled u64 key/value corpora, empty to 4096 keys (small: 32). Primary tasks use genuine std/product default builders, including cached seed acquisition on construction. OS-selected hash keys and placements vary; input corpora and call counts are replayed. Named fixed_* controls use equal zero SipHash13 keys; that std control has a zero-sized builder (32-byte handle versus 48-byte seeded handle).\"}}"
            );
            return;
        }
        #[cfg(feature = "btree-memory")]
        Some("memory") => {
            black_box(Std::new());
            black_box(HashMap::<u64, u64>::try_new().unwrap());
            for seed in [617, 6203, 96011] {
                let cases = corpus(seed, false);
                let c = Context::new();
                memory_for::<Std>("std", &cases, &c, seed);
                memory_for::<HashMap<u64, u64>>("unlimited", &cases, &c, seed);
                memory_for::<HashMap<u64, u64, RandomState, Budget<'_>>>("local", &cases, &c, seed);
                memory_for::<HashMap<u64, u64, RandomState, ConcurrentBudget<'_>>>(
                    "concurrent",
                    &cases,
                    &c,
                    seed,
                );
            }
            return;
        }
        _ => {}
    }
    assert_eq!(args.len(), 6);
    let label = &args[1];
    let name = args[2].strip_prefix("hash_map_").unwrap();
    let (fixed, name) = if let Some(name) = name.strip_prefix("fixed_") {
        (true, name)
    } else {
        (false, name)
    };
    let op = OPS.iter().position(|s| *s == name).unwrap();
    let seed = args[3].parse().unwrap();
    let n = args[4].parse::<u64>().unwrap();
    let cases = corpus(seed, args[5] == "small");
    let c = Context::new();
    macro_rules! choose{($($i:literal),*)=>{match op{$($i=>dispatch!($i,label.as_str(),&cases,&c,n,fixed),)*_=>unreachable!()}};}
    let (elapsed, checksum) = choose!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
    assert_eq!(
        (c.local.occupied(), c.local.reserved(), c.concurrent.used()),
        (0, 0, 0)
    );
    let mut digest = 0xcbf29ce484222325u64;
    for case in &cases {
        for k in &case.keys {
            digest = (digest ^ *k).wrapping_mul(0x100000001b3)
        }
    }
    let elements = cases.iter().map(|case| case.keys.len() as u64).sum::<u64>();
    let per_epoch = if op == 1 {
        elements * 2
    } else if matches!(op, 2 | 3 | 6) {
        elements
    } else {
        64
    };
    println!(
        "{{\"epochs\":{n},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"checksum\":{checksum},\"elapsed_seconds\":{elapsed}}}",
        n * per_epoch,
        if op >= 13 { 0 } else { n * elements },
        cases.iter().map(|c| c.keys.len()).min().unwrap(),
        cases.iter().map(|c| c.keys.len()).max().unwrap()
    );
}
