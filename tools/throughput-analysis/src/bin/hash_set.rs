//! Equal-work HashSet lifecycles and algebra; genuine default builders.
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
    collections::{HashSet, hash_map::RandomState},
};
use ordered_corpus::{Case, corpus};
use std::{collections::HashSet as StdSet, hint::black_box, time::Instant};
const OPS: [&str; 30] = [
    "build",
    "get",
    "replace",
    "remove_cycle",
    "iter",
    "clone",
    "retain",
    "extract",
    "drain",
    "into_iter",
    "reserve",
    "shrink",
    "union",
    "intersection",
    "difference",
    "symmetric_difference",
    "is_subset",
    "is_disjoint",
    "default_new",
    "owned_union",
    "owned_intersection",
    "owned_difference",
    "owned_symmetric_difference",
    "intersection_disjoint",
    "intersection_skew",
    "is_disjoint_true",
    "is_subset_true",
    "phase_clone",
    "phase_grow",
    "phase_shrink",
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
trait Owner<'a>: Sized {
    fn new(c: &'a Context) -> Self;
    fn len(&self) -> usize;
    fn insert(&mut self, key: u64) -> bool;
    fn get(&self, key: u64) -> Option<u64>;
    fn replace(&mut self, key: u64) -> Option<u64>;
    fn remove(&mut self, key: u64) -> bool;
    fn sum(&self) -> u64;
    fn copy(&self) -> Self;
    fn retain(&mut self, pivot: u64);
    fn extract(&mut self, pivot: u64) -> u64;
    fn drain(&mut self) -> u64;
    fn consume(self) -> u64;
    fn reserve(&mut self, n: usize);
    fn shrink(&mut self);
    fn algebra(&self, other: &Self, op: usize) -> u64;
    fn owned_algebra(&self, other: &Self, op: usize) -> Self;
    fn default_new(c: &'a Context) -> u64 {
        let value = Self::new(c);
        black_box(&value);
        black_box(value.len()) as u64
    }
}
impl<'a> Owner<'a> for StdSet<u64> {
    fn new(_: &'a Context) -> Self {
        Self::new()
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn insert(&mut self, key: u64) -> bool {
        self.insert(key)
    }
    fn get(&self, key: u64) -> Option<u64> {
        self.get(&key).copied()
    }
    fn replace(&mut self, key: u64) -> Option<u64> {
        self.replace(key)
    }
    fn remove(&mut self, key: u64) -> bool {
        self.remove(&key)
    }
    fn sum(&self) -> u64 {
        self.iter().fold(0, |sum, key| sum.wrapping_add(*key))
    }
    fn copy(&self) -> Self {
        self.clone()
    }
    fn retain(&mut self, pivot: u64) {
        self.retain(|key| !(*key ^ pivot).is_multiple_of(3))
    }
    fn extract(&mut self, pivot: u64) -> u64 {
        self.extract_if(|key| (*key ^ pivot).is_multiple_of(3))
            .fold(0, |sum, key| sum.wrapping_add(key))
    }
    fn drain(&mut self) -> u64 {
        self.drain().fold(0, |sum, key| sum.wrapping_add(key))
    }
    fn consume(self) -> u64 {
        self.into_iter().fold(0, |sum, key| sum.wrapping_add(key))
    }
    fn reserve(&mut self, n: usize) {
        self.reserve(n)
    }
    fn shrink(&mut self) {
        self.shrink_to_fit()
    }
    #[inline(always)]
    fn algebra(&self, other: &Self, op: usize) -> u64 {
        match op {
            12 => self.union(other).fold(0, |sum, key| sum.wrapping_add(*key)),
            13 | 23 | 24 => self
                .intersection(other)
                .fold(0, |sum, key| sum.wrapping_add(*key)),
            14 => self
                .difference(other)
                .fold(0, |sum, key| sum.wrapping_add(*key)),
            15 => self
                .symmetric_difference(other)
                .fold(0, |sum, key| sum.wrapping_add(*key)),
            16 | 26 => u64::from(self.is_subset(other)),
            17 | 25 => u64::from(self.is_disjoint(other)),
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    fn owned_algebra(&self, other: &Self, op: usize) -> Self {
        match op {
            19 => self | other,
            20 => self & other,
            21 => self - other,
            22 => self ^ other,
            _ => unreachable!(),
        }
    }
}
impl<'a, P: Policy<'a>> Owner<'a> for HashSet<u64, RandomState, P> {
    fn new(c: &'a Context) -> Self {
        Self::try_new_in(P::choose(c)).unwrap()
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn insert(&mut self, key: u64) -> bool {
        self.try_insert(key).unwrap()
    }
    fn get(&self, key: u64) -> Option<u64> {
        self.get(&key).copied()
    }
    fn replace(&mut self, key: u64) -> Option<u64> {
        self.try_replace(key).unwrap()
    }
    fn remove(&mut self, key: u64) -> bool {
        self.remove(&key)
    }
    fn sum(&self) -> u64 {
        self.iter().fold(0, |sum, key| sum.wrapping_add(*key))
    }
    fn copy(&self) -> Self {
        self.try_clone().unwrap()
    }
    fn retain(&mut self, pivot: u64) {
        self.try_retain(|key| Ok::<_, ()>(!(*key ^ pivot).is_multiple_of(3)))
            .unwrap()
    }
    fn extract(&mut self, pivot: u64) -> u64 {
        let mut iter = self.try_extract_if(|key| Ok::<_, ()>((*key ^ pivot).is_multiple_of(3)));
        let mut sum = 0u64;
        while let Some(key) = iter.try_next().unwrap() {
            sum = sum.wrapping_add(key);
        }
        sum
    }
    fn drain(&mut self) -> u64 {
        self.drain().fold(0, |sum, key| sum.wrapping_add(key))
    }
    fn consume(self) -> u64 {
        self.into_iter().fold(0, |sum, key| sum.wrapping_add(key))
    }
    fn reserve(&mut self, n: usize) {
        self.try_reserve(n).unwrap()
    }
    fn shrink(&mut self) {
        self.try_shrink_to_fit().unwrap()
    }
    #[inline(always)]
    fn algebra(&self, other: &Self, op: usize) -> u64 {
        match op {
            12 => self.union(other).fold(0, |sum, key| sum.wrapping_add(*key)),
            13 | 23 | 24 => self
                .intersection(other)
                .fold(0, |sum, key| sum.wrapping_add(*key)),
            14 => self
                .difference(other)
                .fold(0, |sum, key| sum.wrapping_add(*key)),
            15 => self
                .symmetric_difference(other)
                .fold(0, |sum, key| sum.wrapping_add(*key)),
            16 | 26 => u64::from(self.is_subset(other)),
            17 | 25 => u64::from(self.is_disjoint(other)),
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    fn owned_algebra(&self, other: &Self, op: usize) -> Self {
        match op {
            19 => self.try_union(other).unwrap(),
            20 => self.try_intersection(other).unwrap(),
            21 => self.try_difference(other).unwrap(),
            22 => self.try_symmetric_difference(other).unwrap(),
            _ => unreachable!(),
        }
    }
}
fn populate<'a, T: Owner<'a>>(case: &Case, c: &'a Context, op: usize) -> (T, T) {
    let mut left = T::new(c);
    let mut right = T::new(c);
    for &key in &case.keys {
        left.insert(key);
    }
    for (index, &key) in case.keys.iter().enumerate() {
        match op {
            23 | 25 => {
                right.insert(key | 1);
            }
            24 if index >= 3 => {}
            24 => {
                right.insert(key);
            }
            26 => {
                right.insert(key);
                right.insert(key | 1);
            }
            _ => {
                right.insert(if key.is_multiple_of(4) { key } else { key | 1 });
            }
        }
    }
    (left, right)
}
#[inline(never)]
fn epoch<'a, T: Owner<'a>, const OP: usize>(
    cases: &[Case],
    c: &'a Context,
    sources: &mut [(T, T)],
    n: u64,
) -> u64 {
    let mut sum = 0u64;
    for _ in 0..n {
        for (case, (left, right)) in cases.iter().zip(sources.iter_mut()) {
            let left = black_box(left);
            let right = black_box(right);
            let value = match OP {
                0 => {
                    let mut m = T::new(c);
                    for key in &case.keys {
                        black_box(m.insert(black_box(*key)));
                    }
                    black_box(&m);
                    m.len() as u64
                }
                1 => {
                    let mut s = 0u64;
                    for key in &case.keys {
                        s = s.wrapping_add(black_box(left.get(black_box(*key))).unwrap_or(0));
                        s = s.wrapping_add(black_box(left.get(black_box(key | 1))).unwrap_or(0));
                    }
                    s
                }
                2 => {
                    let mut s = 0u64;
                    for key in &case.keys {
                        s = s.wrapping_add(black_box(left.replace(black_box(*key))).unwrap_or(0));
                    }
                    s
                }
                3 => {
                    let mut s = 0u64;
                    for key in &case.keys {
                        s = s.wrapping_add(u64::from(black_box(left.remove(black_box(*key)))));
                        black_box(left.insert(black_box(*key)));
                    }
                    s
                }
                4 => left.sum(),
                12..=17 | 23..=26 => left.algebra(right, OP),
                18 => T::default_new(c),
                19..=22 => {
                    let result = left.owned_algebra(right, OP);
                    black_box(&result);
                    result.len() as u64
                }
                _ => {
                    let mut m = left.copy();
                    let value = match OP {
                        6 => {
                            m.retain(black_box(case.pivot));
                            m.len() as u64
                        }
                        7 => m.extract(black_box(case.pivot)),
                        8 => m.drain(),
                        9 => {
                            sum = sum.wrapping_add(black_box(m.consume()));
                            continue;
                        }
                        10 => {
                            m.reserve(case.keys.len() + 1);
                            m.len() as u64
                        }
                        11 => {
                            m.reserve(case.keys.len() + 1);
                            m.shrink();
                            m.len() as u64
                        }
                        _ => m.len() as u64,
                    };
                    black_box(&m);
                    value
                }
            };
            sum = sum.wrapping_add(black_box(value));
        }
    }
    sum
}
fn resize_phase<'a, T: Owner<'a>, const OP: usize>(
    cases: &[Case],
    sources: &[(T, T)],
    epochs: u64,
) -> (f64, u64) {
    let mut work = Vec::with_capacity(cases.len());
    let mut elapsed = std::time::Duration::ZERO;
    let mut checksum = 0u64;
    for _ in 0..epochs {
        if OP != 27 {
            work.extend(sources.iter().map(|(left, _)| left.copy()));
            if OP == 29 {
                for (set, case) in work.iter_mut().zip(cases) {
                    set.reserve(case.keys.len() + 1);
                }
            }
        }
        let start = Instant::now();
        match OP {
            27 => work.extend(sources.iter().map(|(left, _)| black_box(left).copy())),
            28 => {
                for (set, case) in work.iter_mut().zip(cases) {
                    black_box(set).reserve(case.keys.len() + 1);
                }
            }
            29 => {
                for set in &mut work {
                    black_box(set).shrink();
                }
            }
            _ => unreachable!(),
        }
        elapsed += start.elapsed();
        for set in &work {
            checksum = checksum.wrapping_add(black_box(set.sum()));
        }
        work.clear();
    }
    (elapsed.as_secs_f64(), checksum)
}

fn run<'a, T: Owner<'a>, const OP: usize>(cases: &[Case], c: &'a Context, n: u64) -> (f64, u64) {
    let mut sources = cases
        .iter()
        .map(|case| populate::<T>(case, c, OP))
        .collect::<Vec<_>>();
    if OP >= 27 {
        black_box(resize_phase::<T, OP>(cases, &sources, 2));
        return resize_phase::<T, OP>(cases, &sources, n);
    }
    black_box(epoch::<T, OP>(cases, c, &mut sources, 2));
    let start = Instant::now();
    let checksum = epoch::<T, OP>(cases, c, &mut sources, n);
    let elapsed = start.elapsed().as_secs_f64();
    black_box(sources);
    (elapsed, checksum)
}
macro_rules! dispatch {
    ($op:expr, $label:expr, $cases:expr, $c:expr, $n:expr) => {
        match $label {
            "std" | "std_control" => run::<StdSet<u64>, $op>($cases, $c, $n),
            "unlimited" => run::<HashSet<u64>, $op>($cases, $c, $n),
            "local" => run::<HashSet<u64, RandomState, Budget<'_>>, $op>($cases, $c, $n),
            "concurrent" => {
                run::<HashSet<u64, RandomState, ConcurrentBudget<'_>>, $op>($cases, $c, $n)
            }
            _ => panic!("unknown label"),
        }
    };
}
#[cfg(feature = "btree-memory")]
fn memory_for<'a, T: Owner<'a>>(label: &str, cases: &[Case], c: &'a Context, seed: u64) {
    for (ci, case) in cases.iter().enumerate() {
        for op in [0, 5, 6, 7, 8, 9, 10, 11, 19, 20, 21, 22] {
            memory::start();
            let (mut left, right) = if op >= 19 {
                populate::<T>(case, c, op)
            } else {
                let mut m = T::new(c);
                for &key in &case.keys {
                    m.insert(key);
                }
                (m, T::new(c))
            };
            let before = memory::snapshot();
            let mut extra = None;
            match op {
                5 => extra = Some(left.copy()),
                6 => left.retain(case.pivot),
                7 => {
                    black_box(left.extract(case.pivot));
                }
                8 => {
                    black_box(left.drain());
                }
                9 => {
                    let old = std::mem::replace(&mut left, T::new(c));
                    black_box(old.consume());
                }
                10 => left.reserve(case.keys.len() + 1),
                11 => {
                    left.reserve(case.keys.len() + 1);
                    left.shrink();
                }
                19..=22 => extra = Some(left.owned_algebra(&right, op)),
                _ => {}
            }
            let held = memory::snapshot();
            drop((left, right, extra));
            let end = memory::stop();
            assert_eq!((end.live, end.actual), (0, 0));
            assert_eq!(
                (c.local.occupied(), c.local.reserved(), c.concurrent.used()),
                (0, 0, 0)
            );
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
                    .map(|op| format!("hash_set_{op}"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            return;
        }
        Some("describe") => {
            println!(
                "{{\"operation_unit\":\"get counts two lookups per key; replace counts one existing-key replacement; remove_cycle counts one remove+insert pair. Other rows count a whole workload, including clone for destructive batches; reserve is clone+growth and shrink is clone+growth+shrink. Owned algebra includes result construction/drop. The phase tasks time only clone, reserve, or shrink, with preparation and drop outside the timed interval. Sources and two warmup epochs are outside timing.\",\"inputs\":\"64 shuffled seeded u64 corpora, size 0..4096 (small: 0..32). Equal-size right sets overlap by about half; disjoint tasks use odd keys, skew intersection has at most three right keys, true subset has a right superset. Both implementations use genuine default builders; owned std operators create a default builder while product owned algebra clones the left builder/policy. OS-selected hash placements vary.\"}}"
            );
            return;
        }
        #[cfg(feature = "btree-memory")]
        Some("memory") => {
            black_box(StdSet::<u64>::new());
            black_box(HashSet::<u64>::try_new().unwrap());
            for seed in [617, 6203, 96011] {
                let cases = corpus(seed, false);
                let c = Context::new();
                memory_for::<StdSet<u64>>("std", &cases, &c, seed);
                memory_for::<HashSet<u64>>("unlimited", &cases, &c, seed);
                memory_for::<HashSet<u64, RandomState, Budget<'_>>>("local", &cases, &c, seed);
                memory_for::<HashSet<u64, RandomState, ConcurrentBudget<'_>>>(
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
    let op = OPS
        .iter()
        .position(|s| Some(*s) == args[2].strip_prefix("hash_set_"))
        .unwrap();
    let seed = args[3].parse().unwrap();
    let n = args[4].parse::<u64>().unwrap();
    let cases = corpus(seed, args[5] == "small");
    let c = Context::new();
    macro_rules! choose { ($($i:literal),*) => { match op { $($i=>dispatch!($i,label.as_str(),&cases,&c,n),)* _=>unreachable!() } }; }
    let (elapsed, checksum) = choose!(
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29
    );
    assert_eq!(
        (c.local.occupied(), c.local.reserved(), c.concurrent.used()),
        (0, 0, 0)
    );
    let mut digest = 0xcbf29ce484222325u64;
    for case in &cases {
        for key in &case.keys {
            digest = (digest ^ *key).wrapping_mul(0x100000001b3);
        }
    }
    let elements = cases.iter().map(|case| case.keys.len() as u64).sum::<u64>();
    let calls = if op == 1 {
        elements * 2
    } else if op == 2 || op == 3 {
        elements
    } else {
        64
    };
    println!(
        "{{\"epochs\":{n},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"checksum\":{checksum},\"elapsed_seconds\":{elapsed}}}",
        n * calls,
        if op == 18 { 0 } else { n * elements },
        cases.iter().map(|c| c.keys.len()).min().unwrap(),
        cases.iter().map(|c| c.keys.len()).max().unwrap()
    );
}
