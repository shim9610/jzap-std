//! Long-interval equal-work ordered-map workloads. All input generation and
//! source-map preparation occur outside the one continuous measured interval.
#[cfg(feature = "btree-memory")]
#[path = "../string_memory.rs"]
mod memory;
use jzap::{
    TryClone, TryIterator,
    budget::{
        AllocationPolicy, Budget, ConcurrentBudget, ConcurrentMemoryBudget, MemoryBudget, Unmetered,
    },
    collections::BTreeMap,
};
use std::{collections::BTreeMap as StdMap, hint::black_box, time::Instant};
const OPS: [&str; 16] = [
    "build",
    "get",
    "replace",
    "iter",
    "range",
    "entry",
    "clone",
    "remove",
    "retain",
    "extract",
    "split",
    "append",
    "append_disjoint",
    "pop_all",
    "into_iter",
    "remove_cycle",
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
        Budget::Limited(&c.local)
    }
}
impl<'a> Policy<'a> for ConcurrentBudget<'a> {
    fn choose(c: &'a Context) -> Self {
        Self(&c.concurrent)
    }
}
#[path = "../ordered_corpus.rs"]
mod ordered_corpus;
use ordered_corpus::{Case, corpus};
trait Owner<'a>: Sized {
    fn new(c: &'a Context) -> Self;
    fn insert(&mut self, k: u64, v: u64);
    fn get(&self, k: u64) -> Option<u64>;
    fn checksum(&self) -> u64;
    fn range_sum(&self, p: u64) -> u64;
    fn entry(&mut self, k: u64);
    fn copy(&self) -> Self;
    fn remove(&mut self, k: u64);
    fn retain(&mut self, p: u64);
    fn extract(&mut self, p: u64);
    fn split(&mut self, p: u64) -> Self;
    fn append(&mut self, other: &mut Self);
    fn pop(&mut self) -> Option<(u64, u64)>;
    fn consume(self) -> u64;
}
impl<'a> Owner<'a> for StdMap<u64, u64> {
    fn new(_: &'a Context) -> Self {
        Self::new()
    }
    fn insert(&mut self, k: u64, v: u64) {
        black_box(StdMap::insert(self, k, v));
    }
    fn get(&self, k: u64) -> Option<u64> {
        StdMap::get(self, &k).copied()
    }
    fn checksum(&self) -> u64 {
        self.iter().fold(0u64, |a, (k, v)| a.wrapping_add(k ^ v))
    }
    fn range_sum(&self, p: u64) -> u64 {
        self.range(p..)
            .fold(0u64, |a, (k, v)| a.wrapping_add(k ^ v))
    }
    fn entry(&mut self, k: u64) {
        *self.entry(k).or_insert(7) ^= 3;
    }
    fn copy(&self) -> Self {
        self.clone()
    }
    fn remove(&mut self, k: u64) {
        black_box(StdMap::remove(self, &k));
    }
    fn retain(&mut self, p: u64) {
        StdMap::retain(self, |k, v| {
            *v ^= 1;
            !(*k ^ p).is_multiple_of(3)
        });
    }
    fn extract(&mut self, p: u64) {
        for pair in self.extract_if(.., |k, v| {
            *v ^= 1;
            (*k ^ p).is_multiple_of(3)
        }) {
            black_box(pair);
        }
    }
    fn split(&mut self, p: u64) -> Self {
        self.split_off(&p)
    }
    fn append(&mut self, b: &mut Self) {
        StdMap::append(self, b);
    }
    fn pop(&mut self) -> Option<(u64, u64)> {
        self.pop_first()
    }
    fn consume(self) -> u64 {
        self.into_iter()
            .fold(0u64, |a, (k, v)| a.wrapping_add(k ^ v))
    }
}
impl<'a, P: Policy<'a>> Owner<'a> for BTreeMap<u64, u64, P> {
    fn new(c: &'a Context) -> Self {
        Self::new_in(P::choose(c))
    }
    fn insert(&mut self, k: u64, v: u64) {
        black_box(self.try_insert(k, v).unwrap());
    }
    fn get(&self, k: u64) -> Option<u64> {
        BTreeMap::get(self, &k).copied()
    }
    fn checksum(&self) -> u64 {
        self.iter().fold(0u64, |a, (k, v)| a.wrapping_add(k ^ v))
    }
    fn range_sum(&self, p: u64) -> u64 {
        self.try_range(p..)
            .unwrap()
            .fold(0u64, |a, (k, v)| a.wrapping_add(k ^ v))
    }
    fn entry(&mut self, k: u64) {
        *self.entry(k).try_or_insert(7).unwrap() ^= 3;
    }
    fn copy(&self) -> Self {
        self.try_clone().unwrap()
    }
    fn remove(&mut self, k: u64) {
        black_box(BTreeMap::remove(self, &k));
    }
    fn retain(&mut self, p: u64) {
        self.try_retain(|k, v| {
            *v ^= 1;
            Ok::<_, ()>(!(*k ^ p).is_multiple_of(3))
        })
        .unwrap();
    }
    fn extract(&mut self, p: u64) {
        let mut i = self
            .try_extract_if(.., |k, v| {
                *v ^= 1;
                Ok::<_, ()>((*k ^ p).is_multiple_of(3))
            })
            .unwrap();
        while let Some(pair) = i.try_next().unwrap() {
            black_box(pair);
        }
    }
    fn split(&mut self, p: u64) -> Self {
        self.try_split_off(&p).unwrap()
    }
    fn append(&mut self, b: &mut Self) {
        self.try_append(b).unwrap();
    }
    fn pop(&mut self) -> Option<(u64, u64)> {
        self.pop_first()
    }
    fn consume(self) -> u64 {
        self.into_iter()
            .fold(0u64, |a, (k, v)| a.wrapping_add(k ^ v))
    }
}
fn prepared<'a, T: Owner<'a>>(cases: &[Case], context: &'a Context) -> Vec<T> {
    cases
        .iter()
        .map(|c| {
            let mut m = T::new(context);
            for k in &c.keys {
                m.insert(*k, *k + 1);
            }
            m
        })
        .collect()
}
fn epochs<'a, T: Owner<'a>, const OP: usize>(
    cases: &[Case],
    context: &'a Context,
    sources: &mut [T],
    others: &[T],
    epochs: u64,
) {
    for _ in 0..epochs {
        for (index, (case, source)) in cases.iter().zip(sources.iter_mut()).enumerate() {
            if OP == 0 {
                let mut m = T::new(context);
                for k in &case.keys {
                    m.insert(black_box(*k), *k + 1);
                }
                black_box(m);
            } else if OP == 1 {
                for k in &case.keys {
                    black_box(source.get(black_box(*k)));
                    black_box(source.get(black_box(*k | 1)));
                }
            } else if OP == 2 {
                for k in &case.keys {
                    source.insert(black_box(*k), black_box(*k ^ 7));
                }
            } else if OP == 3 {
                black_box(source.checksum());
            } else if OP == 4 {
                black_box(source.range_sum(black_box(case.pivot)));
            } else if OP == 5 {
                for k in &case.keys {
                    source.entry(black_box(*k));
                }
            } else if OP == 15 {
                for k in &case.keys {
                    source.remove(black_box(*k));
                    source.insert(black_box(*k), *k + 1);
                }
            } else {
                let mut m = source.copy();
                if OP == 7 {
                    for k in &case.keys {
                        m.remove(black_box(*k));
                    }
                } else if OP == 8 {
                    m.retain(black_box(case.pivot));
                } else if OP == 9 {
                    m.extract(black_box(case.pivot));
                } else if OP == 10 {
                    black_box(m.split(black_box(case.pivot)));
                } else if OP == 11 {
                    let mut b = source.copy();
                    b.insert(case.pivot | 1, 13);
                    m.append(&mut b);
                    black_box(b);
                } else if OP == 12 {
                    let mut b = others[index].copy();
                    m.append(&mut b);
                    black_box(b);
                } else if OP == 13 {
                    while let Some(pair) = m.pop() {
                        black_box(pair);
                    }
                } else if OP == 14 {
                    black_box(m.consume());
                    continue;
                }
                black_box(m);
            }
        }
    }
}
fn run<'a, T: Owner<'a>, const OP: usize>(cases: &[Case], context: &'a Context, n: u64) -> f64 {
    let mut sources = prepared::<T>(cases, context);
    let others = if OP == 12 {
        cases
            .iter()
            .map(|case| {
                let mut m = T::new(context);
                for k in &case.keys {
                    m.insert(*k | 1, *k + 2);
                }
                m
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    epochs::<T, OP>(cases, context, &mut sources, &others, 2);
    let started = Instant::now();
    epochs::<T, OP>(cases, context, &mut sources, &others, n);
    let elapsed = started.elapsed().as_secs_f64();
    black_box(&sources);
    elapsed
}
macro_rules! dispatch {
    ($op:expr,$label:expr,$cases:expr,$context:expr,$n:expr) => {
        match $label {
            "std" => run::<StdMap<u64, u64>, $op>($cases, $context, $n),
            "unlimited" => run::<BTreeMap<u64, u64>, $op>($cases, $context, $n),
            "local" => run::<BTreeMap<u64, u64, Budget<'_>>, $op>($cases, $context, $n),
            "concurrent" => {
                run::<BTreeMap<u64, u64, ConcurrentBudget<'_>>, $op>($cases, $context, $n)
            }
            _ => panic!("unknown label"),
        }
    };
}
#[cfg(feature = "btree-memory")]
fn memory_for<'a, T: Owner<'a>>(label: &str, cases: &[Case], context: &'a Context, seed: u64) {
    for (ci, case) in cases.iter().enumerate() {
        for op in OPS {
            memory::start();
            let mut m = T::new(context);
            for k in &case.keys {
                m.insert(*k, *k + 1);
            }
            let mut extra = None;
            let before = memory::snapshot();
            match op {
                "get" => {
                    for k in &case.keys {
                        black_box(m.get(*k));
                        black_box(m.get(*k | 1));
                    }
                }
                "iter" => {
                    black_box(m.checksum());
                }
                "range" => {
                    black_box(m.range_sum(case.pivot));
                }
                "replace" => {
                    for k in &case.keys {
                        m.insert(*k, *k + 1);
                    }
                }
                "entry" => {
                    for k in &case.keys {
                        m.entry(*k);
                    }
                }
                "remove_cycle" => {
                    for k in &case.keys {
                        m.remove(*k);
                        m.insert(*k, *k + 1);
                    }
                }
                "pop_all" => {
                    while let Some(pair) = m.pop() {
                        black_box(pair);
                    }
                }
                "into_iter" => {
                    let owned = std::mem::replace(&mut m, T::new(context));
                    black_box(owned.consume());
                }
                "clone" => extra = Some(m.copy()),
                "remove" => {
                    for k in &case.keys {
                        m.remove(*k);
                    }
                }
                "retain" => m.retain(case.pivot),
                "extract" => m.extract(case.pivot),
                "split" => extra = Some(m.split(case.pivot)),
                "append" => {
                    let mut b = m.copy();
                    b.insert(case.pivot | 1, 13);
                    m.append(&mut b);
                    extra = Some(b);
                }
                "append_disjoint" => {
                    let mut b = T::new(context);
                    for k in &case.keys {
                        b.insert(*k | 1, *k + 2);
                    }
                    m.append(&mut b);
                    extra = Some(b);
                }
                "build" => {}
                _ => unreachable!(),
            }
            let held = memory::snapshot();
            if matches!(op, "get" | "iter" | "range" | "replace" | "entry") {
                assert_eq!(held.calls, before.calls);
            }
            drop((m, extra));
            let final_ = memory::stop();
            assert_eq!((final_.live, final_.actual), (0, 0));
            println!(
                "{{\"label\":\"{label}\",\"seed\":{seed},\"case\":{ci},\"operation\":\"{op}\",\"size\":{},\"calls\":{},\"held\":{},\"peak\":{},\"actual_held\":{},\"actual_peak\":{},\"handle\":{}}}",
                case.keys.len(),
                held.calls,
                held.live,
                held.peak,
                held.actual,
                held.actual_peak,
                std::mem::size_of::<T>()
            );
        }
    }
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|s| s == "tasks") {
        println!("{}", OPS.map(|x| format!("btree_{x}")).join(","));
        return;
    }
    if args.get(1).is_some_and(|s| s == "describe") {
        println!(
            "{{\"operation_unit\":\"get/replace/entry count individual key calls; remove_cycle counts one remove+reinsert pair; other operations count complete map workload cases; build includes construction/drop; iter/range scan prepared maps; clone/remove/retain/extract/split/pop_all/into_iter include source cloning and output destruction; append cases include both source clones\",\"inputs\":\"64 pregenerated shuffled u64 key cases; mixed logarithmic sizes up to 4096, small up to 32; same keys/pivots/order per seed across labels; append includes duplicate keys plus one new key; append_disjoint uses separately prepared odd keys\"}}"
        );
        return;
    }
    #[cfg(feature = "btree-memory")]
    if args.get(1).is_some_and(|s| s == "memory") {
        for seed in [617, 6203, 96011] {
            let cases = corpus(seed, false);
            let c = Context::new();
            memory_for::<StdMap<u64, u64>>("std", &cases, &c, seed);
            memory_for::<BTreeMap<u64, u64>>("unlimited", &cases, &c, seed);
            memory_for::<BTreeMap<u64, u64, Budget<'_>>>("local", &cases, &c, seed);
            memory_for::<BTreeMap<u64, u64, ConcurrentBudget<'_>>>("concurrent", &cases, &c, seed);
        }
        return;
    }
    assert_eq!(args.len(), 6);
    let label = args[1].as_str();
    let op = OPS
        .iter()
        .position(|x| format!("btree_{x}") == args[2])
        .unwrap();
    let seed = args[3].parse().unwrap();
    let n = args[4].parse().unwrap();
    let cases = corpus(seed, args[5] == "small");
    let c = Context::new();
    let elapsed = match op {
        0 => dispatch!(0, label, &cases, &c, n),
        1 => dispatch!(1, label, &cases, &c, n),
        2 => dispatch!(2, label, &cases, &c, n),
        3 => dispatch!(3, label, &cases, &c, n),
        4 => dispatch!(4, label, &cases, &c, n),
        5 => dispatch!(5, label, &cases, &c, n),
        6 => dispatch!(6, label, &cases, &c, n),
        7 => dispatch!(7, label, &cases, &c, n),
        8 => dispatch!(8, label, &cases, &c, n),
        9 => dispatch!(9, label, &cases, &c, n),
        10 => dispatch!(10, label, &cases, &c, n),
        11 => dispatch!(11, label, &cases, &c, n),
        12 => dispatch!(12, label, &cases, &c, n),
        13 => dispatch!(13, label, &cases, &c, n),
        14 => dispatch!(14, label, &cases, &c, n),
        15 => dispatch!(15, label, &cases, &c, n),
        _ => unreachable!(),
    };
    assert_eq!(
        (c.local.occupied(), c.local.reserved(), c.concurrent.used()),
        (0, 0, 0)
    );
    let mut digest = 0xcbf29ce484222325u64;
    for case in &cases {
        for k in case.keys.iter().chain([&case.pivot]) {
            digest = (digest ^ *k).wrapping_mul(0x100000001b3);
        }
    }
    let operations = n * match op {
        1 => 2 * cases.iter().map(|c| c.keys.len() as u64).sum::<u64>(),
        2 | 5 | 15 => cases.iter().map(|c| c.keys.len() as u64).sum::<u64>(),
        _ => 64,
    };
    println!(
        "{{\"epochs\":{n},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"elapsed_seconds\":{elapsed}}}",
        operations,
        n * cases.iter().map(|c| c.keys.len() as u64).sum::<u64>(),
        cases.iter().map(|c| c.keys.len()).min().unwrap(),
        cases.iter().map(|c| c.keys.len()).max().unwrap()
    );
}
