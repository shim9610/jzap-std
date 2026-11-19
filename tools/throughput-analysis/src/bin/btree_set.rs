//! Equal-work ordered-set lifecycles and algebra, with one long timed interval.
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
    collections::BTreeSet,
};
use ordered_corpus::{Case, corpus};
use std::{collections::BTreeSet as StdSet, hint::black_box, time::Instant};
const OPS: [&str; 27] = [
    "build",
    "get",
    "replace",
    "iter",
    "range",
    "clone",
    "remove",
    "retain",
    "extract",
    "split",
    "append",
    "pop_all",
    "into_iter",
    "remove_cycle",
    "union",
    "intersection",
    "difference",
    "symmetric_difference",
    "subset",
    "intersection_skew",
    "difference_skew",
    "intersection_disjoint",
    "union_disjoint",
    "owned_union",
    "owned_intersection",
    "owned_difference",
    "owned_symmetric_difference",
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
    fn insert(&mut self, k: u64) -> bool;
    fn replace(&mut self, k: u64) -> Option<u64>;
    fn get(&self, k: u64) -> Option<u64>;
    fn checksum(&self) -> u64;
    fn range_sum(&self, p: u64) -> u64;
    fn copy(&self) -> Self;
    fn take(&mut self, k: u64) -> Option<u64>;
    fn retain(&mut self, p: u64);
    fn extract(&mut self, p: u64) -> u64;
    fn split(&mut self, p: u64) -> Self;
    fn append(&mut self, b: &mut Self);
    fn pop(&mut self) -> Option<u64>;
    fn len(&self) -> usize;
    fn consume(self) -> u64;
    fn algebra(&self, b: &Self, kind: usize) -> u64;
    fn owned_algebra(&self, b: &Self, kind: usize) -> Self;
}
fn sum<'a>(i: impl Iterator<Item = &'a u64>) -> u64 {
    i.fold(0, |a, k| a.wrapping_add(*k))
}
impl<'a> Owner<'a> for StdSet<u64> {
    fn new(_: &'a Context) -> Self {
        Self::new()
    }
    fn insert(&mut self, k: u64) -> bool {
        StdSet::insert(self, k)
    }
    fn replace(&mut self, k: u64) -> Option<u64> {
        StdSet::replace(self, k)
    }
    fn get(&self, k: u64) -> Option<u64> {
        StdSet::get(self, &k).copied()
    }
    fn checksum(&self) -> u64 {
        sum(self.iter())
    }
    fn range_sum(&self, p: u64) -> u64 {
        sum(self.range(p..))
    }
    fn copy(&self) -> Self {
        self.clone()
    }
    fn take(&mut self, k: u64) -> Option<u64> {
        StdSet::take(self, &k)
    }
    fn retain(&mut self, p: u64) {
        StdSet::retain(self, |k| !(*k ^ p).is_multiple_of(3))
    }
    fn extract(&mut self, p: u64) -> u64 {
        self.extract_if(.., |k| (*k ^ p).is_multiple_of(3))
            .fold(0u64, u64::wrapping_add)
    }
    fn split(&mut self, p: u64) -> Self {
        self.split_off(&p)
    }
    fn append(&mut self, b: &mut Self) {
        StdSet::append(self, b)
    }
    fn pop(&mut self) -> Option<u64> {
        self.pop_first()
    }
    fn len(&self) -> usize {
        StdSet::len(self)
    }
    fn consume(self) -> u64 {
        self.into_iter().fold(0, u64::wrapping_add)
    }
    fn algebra(&self, b: &Self, kind: usize) -> u64 {
        match kind {
            0 => sum(self.union(b)),
            1 => sum(self.intersection(b)),
            2 => sum(self.difference(b)),
            3 => sum(self.symmetric_difference(b)),
            _ => u64::from(self.is_subset(b)),
        }
    }
    fn owned_algebra(&self, b: &Self, kind: usize) -> Self {
        match kind {
            0 => self | b,
            1 => self & b,
            2 => self - b,
            _ => self ^ b,
        }
    }
}
impl<'a, P: Policy<'a>> Owner<'a> for BTreeSet<u64, P> {
    fn new(c: &'a Context) -> Self {
        Self::new_in(P::choose(c))
    }
    fn insert(&mut self, k: u64) -> bool {
        self.try_insert(k).unwrap()
    }
    fn replace(&mut self, k: u64) -> Option<u64> {
        self.try_replace(k).unwrap()
    }
    fn get(&self, k: u64) -> Option<u64> {
        BTreeSet::get(self, &k).copied()
    }
    fn checksum(&self) -> u64 {
        sum(self.iter())
    }
    fn range_sum(&self, p: u64) -> u64 {
        sum(self.try_range(p..).unwrap())
    }
    fn copy(&self) -> Self {
        self.try_clone().unwrap()
    }
    fn take(&mut self, k: u64) -> Option<u64> {
        BTreeSet::take(self, &k)
    }
    fn retain(&mut self, p: u64) {
        self.try_retain(|k| Ok::<_, ()>(!(*k ^ p).is_multiple_of(3)))
            .unwrap()
    }
    fn extract(&mut self, p: u64) -> u64 {
        let mut it = self
            .try_extract_if(.., |k| Ok::<_, ()>((*k ^ p).is_multiple_of(3)))
            .unwrap();
        let mut n = 0u64;
        while let Some(k) = it.try_next().unwrap() {
            n = n.wrapping_add(k);
        }
        n
    }
    fn split(&mut self, p: u64) -> Self {
        self.try_split_off(&p).unwrap()
    }
    fn append(&mut self, b: &mut Self) {
        self.try_append(b).unwrap()
    }
    fn pop(&mut self) -> Option<u64> {
        self.pop_first()
    }
    fn len(&self) -> usize {
        BTreeSet::len(self)
    }
    fn consume(self) -> u64 {
        self.into_iter().fold(0, u64::wrapping_add)
    }
    fn algebra(&self, b: &Self, kind: usize) -> u64 {
        match kind {
            0 => sum(self.union(b)),
            1 => sum(self.intersection(b)),
            2 => sum(self.difference(b)),
            3 => sum(self.symmetric_difference(b)),
            _ => u64::from(self.is_subset(b)),
        }
    }
    fn owned_algebra(&self, b: &Self, kind: usize) -> Self {
        match kind {
            0 => self.try_union(b),
            1 => self.try_intersection(b),
            2 => self.try_difference(b),
            _ => self.try_symmetric_difference(b),
        }
        .unwrap()
    }
}
fn prepared<'a, T: Owner<'a>, const OP: usize>(
    cases: &[Case],
    c: &'a Context,
    other: bool,
) -> Vec<T> {
    cases
        .iter()
        .map(|case| {
            let mut set = T::new(c);
            let take = if !other && matches!(OP, 19 | 20) {
                8
            } else {
                usize::MAX
            };
            for k in case.keys.iter().take(take) {
                let key = if !other {
                    *k
                } else if matches!(OP, 21 | 22) {
                    *k + 1_000_000
                } else if matches!(OP, 18..=20) {
                    *k
                } else if k % 3 == 0 {
                    *k | 1
                } else {
                    *k
                };
                set.insert(key);
                if other && OP == 18 {
                    set.insert(*k | 1);
                }
            }
            set
        })
        .collect()
}
fn epochs<'a, T: Owner<'a>, const OP: usize>(
    cases: &[Case],
    c: &'a Context,
    sources: &mut [T],
    others: &[T],
    n: u64,
) -> u64 {
    let mut checksum = 0u64;
    for _ in 0..n {
        for (index, (case, source)) in cases.iter().zip(sources.iter_mut()).enumerate() {
            let result = if OP == 0 {
                let mut s = T::new(c);
                for k in &case.keys {
                    black_box(s.insert(black_box(*k)));
                }
                black_box(s.len()) as u64
            } else if OP == 1 {
                let mut n = 0u64;
                for k in &case.keys {
                    n = n.wrapping_add(black_box(source.get(black_box(*k))).unwrap_or(0));
                    n = n.wrapping_add(black_box(source.get(black_box(*k | 1))).unwrap_or(0));
                }
                n
            } else if OP == 2 {
                let mut n = 0u64;
                for k in &case.keys {
                    n = n.wrapping_add(black_box(source.replace(black_box(*k))).unwrap_or(0));
                }
                n
            } else if OP == 3 {
                source.checksum()
            } else if OP == 4 {
                source.range_sum(black_box(case.pivot))
            } else if OP == 13 {
                let mut n = 0u64;
                for k in &case.keys {
                    n = n.wrapping_add(black_box(source.take(black_box(*k))).unwrap_or(0));
                    black_box(source.insert(black_box(*k)));
                }
                n
            } else if (14..=22).contains(&OP) {
                let kind = match OP {
                    14 | 22 => 0,
                    15 | 19 | 21 => 1,
                    16 | 20 => 2,
                    17 => 3,
                    _ => 4,
                };
                source.algebra(&others[index], kind)
            } else if OP >= 23 {
                let s = source.owned_algebra(&others[index], OP - 23);
                black_box(s.len()) as u64
            } else {
                let mut s = source.copy();
                let mut extra = 0u64;
                if OP == 6 {
                    for k in &case.keys {
                        extra = extra.wrapping_add(black_box(s.take(black_box(*k))).unwrap_or(0));
                    }
                } else if OP == 7 {
                    s.retain(black_box(case.pivot));
                } else if OP == 8 {
                    extra = s.extract(black_box(case.pivot));
                } else if OP == 9 {
                    extra = black_box(s.split(black_box(case.pivot)).len()) as u64;
                } else if OP == 10 {
                    let mut b = others[index].copy();
                    s.append(&mut b);
                    extra = b.len() as u64;
                } else if OP == 11 {
                    while let Some(k) = s.pop() {
                        extra = extra.wrapping_add(black_box(k));
                    }
                } else if OP == 12 {
                    checksum = checksum.wrapping_add(black_box(s.consume()));
                    continue;
                }
                extra.wrapping_add(black_box(s.len()) as u64)
            };
            checksum = checksum.wrapping_add(black_box(result));
        }
    }
    checksum
}
fn run<'a, T: Owner<'a>, const OP: usize>(cases: &[Case], c: &'a Context, n: u64) -> (f64, u64) {
    let mut sources = prepared::<T, OP>(cases, c, false);
    let others = if OP == 10 || OP >= 14 {
        prepared::<T, OP>(cases, c, true)
    } else {
        Vec::new()
    };
    black_box(epochs::<T, OP>(cases, c, &mut sources, &others, 2));
    let started = Instant::now();
    let checksum = epochs::<T, OP>(cases, c, &mut sources, &others, n);
    let elapsed = started.elapsed().as_secs_f64();
    black_box(&sources);
    (elapsed, checksum)
}
macro_rules! dispatch {
    ($op:expr,$label:expr,$cases:expr,$c:expr,$n:expr) => {
        match $label {
            "std" | "std_control" => run::<StdSet<u64>, $op>($cases, $c, $n),
            "unlimited" => run::<BTreeSet<u64>, $op>($cases, $c, $n),
            "local" => run::<BTreeSet<u64, Budget<'_>>, $op>($cases, $c, $n),
            "concurrent" => run::<BTreeSet<u64, ConcurrentBudget<'_>>, $op>($cases, $c, $n),
            _ => panic!("unknown label"),
        }
    };
}
#[cfg(feature = "btree-memory")]
fn memory_for<'a, T: Owner<'a>>(label: &str, cases: &[Case], c: &'a Context, seed: u64) {
    for (ci, case) in cases.iter().enumerate() {
        for op in [
            0, 1, 2, 3, 4, 5, 9, 10, 11, 12, 14, 15, 16, 17, 23, 24, 25, 26,
        ] {
            memory::start();
            let mut a = T::new(c);
            let mut b = T::new(c);
            for k in &case.keys {
                a.insert(*k);
                if op >= 10 {
                    b.insert(if k % 3 == 0 { *k | 1 } else { *k });
                }
            }
            let before = memory::snapshot();
            let mut extra = None;
            match op {
                1 => {
                    for k in &case.keys {
                        black_box(a.get(*k));
                    }
                }
                2 => {
                    for k in &case.keys {
                        black_box(a.replace(*k));
                    }
                }
                3 => {
                    black_box(a.checksum());
                }
                4 => {
                    black_box(a.range_sum(case.pivot));
                }
                5 => extra = Some(a.copy()),
                9 => extra = Some(a.split(case.pivot)),
                10 => a.append(&mut b),
                11 => {
                    while let Some(k) = a.pop() {
                        black_box(k);
                    }
                }
                12 => {
                    let owned = std::mem::replace(&mut a, T::new(c));
                    black_box(owned.consume());
                }
                14..=17 => {
                    black_box(a.algebra(&b, op - 14));
                }
                23..=26 => extra = Some(a.owned_algebra(&b, op - 23)),
                _ => {}
            }
            let held = memory::snapshot();
            if matches!(op,1..=4|14..=17) {
                assert_eq!(held.calls, before.calls);
            }
            drop((a, b, extra));
            let final_ = memory::stop();
            assert_eq!((final_.live, final_.actual), (0, 0));
            println!(
                "{{\"label\":\"{label}\",\"seed\":{seed},\"case\":{ci},\"operation\":\"{}\",\"size\":{},\"calls\":{},\"held\":{},\"peak\":{},\"actual_held\":{},\"actual_peak\":{},\"handle\":{}}}",
                OPS[op],
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
        println!("{}", OPS.map(|s| format!("btree_set_{s}")).join(","));
        return;
    }
    if args.get(1).is_some_and(|s| s == "describe") {
        println!(
            "{{\"operation_unit\":\"get counts two lookups per key; replace counts one key; remove_cycle counts one take+reinsert pair; others count one whole set workload including output drop. Read/algebra sources are prepared outside timing; destructive operations include source cloning; owned algebra includes result construction/drop.\",\"inputs\":\"64 shuffled u64 corpora, mixed sizes up to 4096 or small up to 32; normal algebra has partial overlap; subset is positive; skew inputs use at most eight source keys against the full other set; disjoint inputs shift the other set by 1000000.\"}}"
        );
        return;
    }
    #[cfg(feature = "btree-memory")]
    if args.get(1).is_some_and(|s| s == "memory") {
        for seed in [617, 6203, 96011] {
            let cases = corpus(seed, false);
            let c = Context::new();
            memory_for::<StdSet<u64>>("std", &cases, &c, seed);
            memory_for::<BTreeSet<u64>>("unlimited", &cases, &c, seed);
            memory_for::<BTreeSet<u64, Budget<'_>>>("local", &cases, &c, seed);
            memory_for::<BTreeSet<u64, ConcurrentBudget<'_>>>("concurrent", &cases, &c, seed);
        }
        return;
    }
    assert_eq!(args.len(), 6);
    let label = args[1].as_str();
    let op = OPS
        .iter()
        .position(|s| format!("btree_set_{s}") == args[2])
        .unwrap();
    let seed = args[3].parse().unwrap();
    let n = args[4].parse().unwrap();
    let cases = corpus(seed, args[5] == "small");
    let c = Context::new();
    macro_rules! choose{($($i:literal),*)=>{match op{$($i=>dispatch!($i,label,&cases,&c,n),)*_=>unreachable!()}};}
    let (elapsed, checksum) = choose!(
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26
    );
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
    let per_epoch = match op {
        1 => 2 * cases.iter().map(|c| c.keys.len() as u64).sum::<u64>(),
        2 | 13 => cases.iter().map(|c| c.keys.len() as u64).sum::<u64>(),
        _ => 64,
    };
    println!(
        "{{\"epochs\":{n},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"checksum\":{checksum},\"elapsed_seconds\":{elapsed}}}",
        n * per_epoch,
        n * cases.iter().map(|c| c.keys.len() as u64).sum::<u64>(),
        cases.iter().map(|c| c.keys.len()).min().unwrap(),
        cases.iter().map(|c| c.keys.len()).max().unwrap()
    );
}
