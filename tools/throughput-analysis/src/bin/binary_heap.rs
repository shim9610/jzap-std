//! Equal-work heap workloads; timing and allocator observation use separate builds.
#[cfg(feature = "btree-memory")]
#[path = "../string_memory.rs"]
mod memory;
#[path = "../ordered_corpus.rs"]
mod ordered_corpus;
use jzap::{
    TryClone,
    budget::{
        AllocationPolicy, Budget, ConcurrentBudget, ConcurrentMemoryBudget, MemoryBudget, Unmetered,
    },
    collections::BinaryHeap,
};
use std::{collections::BinaryHeap as Std, fmt::Debug, hint::black_box, time::Instant};
const OPS: [&str; 15] = [
    "push",
    "push_reserved",
    "push_ascending",
    "pop_all",
    "from_vec",
    "sort",
    "append_small",
    "append_large",
    "retain",
    "peek_mut",
    "clone",
    "iter",
    "into_iter",
    "drain",
    "extend",
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
trait Item: Copy + Ord + TryClone + Debug {
    fn make(x: u64) -> Self;
    fn key(self) -> u64;
}
impl Item for u64 {
    fn make(x: u64) -> Self {
        x
    }
    fn key(self) -> u64 {
        self
    }
}
impl Item for [u64; 32] {
    fn make(x: u64) -> Self {
        let mut v = [0; 32];
        v[0] = x;
        for i in 1..32 {
            v[i] = x.wrapping_mul(i as u64 + 617)
        }
        v
    }
    fn key(self) -> u64 {
        self[0]
    }
}
trait Owner<'a, T: Item>: Sized {
    fn empty(c: &'a Context, cap: usize) -> Self;
    fn from_values(c: &'a Context, values: &[T]) -> Self;
    fn copy(&self) -> Self;
    fn push(&mut self, v: T);
    fn pop(&mut self) -> Option<T>;
    fn append(&mut self, b: &mut Self);
    fn retain(&mut self, pivot: u64);
    fn edit(&mut self, v: T);
    fn checksum(&self) -> u64;
    fn consume(self) -> u64;
    fn sort(self) -> u64;
    fn drain(&mut self) -> u64;
    fn extend(&mut self, values: &[T]);
}
fn sum<T: Item>(values: impl Iterator<Item = T>) -> u64 {
    values.fold(0, |a, v| a.wrapping_add(v.key()))
}
// Deliberately unknown-length sources on both sides. Known-length construction
// has its own from_vec workload; neither side is given an asymmetric size hint.
struct Unknown<I>(I);
impl<I: Iterator> Iterator for Unknown<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
impl<'a, T: Item> Owner<'a, T> for Std<T> {
    fn empty(_: &'a Context, cap: usize) -> Self {
        Self::with_capacity(cap)
    }
    fn from_values(_: &'a Context, v: &[T]) -> Self {
        Self::from(v.to_vec())
    }
    fn copy(&self) -> Self {
        self.clone()
    }
    fn push(&mut self, v: T) {
        self.push(v)
    }
    fn pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn append(&mut self, b: &mut Self) {
        self.append(b)
    }
    fn retain(&mut self, p: u64) {
        self.retain(|v| v.key() & 3 != p & 3)
    }
    fn edit(&mut self, v: T) {
        if let Some(mut p) = self.peek_mut() {
            *p = v
        }
    }
    fn checksum(&self) -> u64 {
        sum(self.iter().copied())
    }
    fn consume(self) -> u64 {
        sum(self.into_iter())
    }
    fn sort(self) -> u64 {
        sum(self.into_sorted_vec().into_iter())
    }
    fn drain(&mut self) -> u64 {
        sum(self.drain())
    }
    fn extend(&mut self, v: &[T]) {
        Extend::extend(self, Unknown(v.iter().copied()))
    }
}
impl<'a, T: Item, P: Policy<'a>> Owner<'a, T> for BinaryHeap<T, P> {
    fn empty(c: &'a Context, cap: usize) -> Self {
        Self::try_with_capacity_in(cap, P::choose(c)).unwrap()
    }
    fn from_values(c: &'a Context, v: &[T]) -> Self {
        let mut data = jzap::vec::Vec::try_with_capacity_in(v.len(), P::choose(c)).unwrap();
        data.try_extend_copy(v).unwrap();
        Self::from_vec(data)
    }
    fn copy(&self) -> Self {
        self.try_clone().unwrap()
    }
    fn push(&mut self, v: T) {
        self.try_push(v).unwrap()
    }
    fn pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn append(&mut self, b: &mut Self) {
        self.try_append(b).unwrap()
    }
    fn retain(&mut self, p: u64) {
        self.try_retain(|v| Ok::<_, ()>(v.key() & 3 != p & 3))
            .unwrap()
    }
    fn edit(&mut self, v: T) {
        if let Some(mut p) = self.peek_mut() {
            *p = v
        }
    }
    fn checksum(&self) -> u64 {
        sum(self.iter().copied())
    }
    fn consume(self) -> u64 {
        sum(self.into_iter())
    }
    fn sort(self) -> u64 {
        sum(self.into_sorted_vec().into_iter())
    }
    fn drain(&mut self) -> u64 {
        sum(self.drain())
    }
    fn extend(&mut self, v: &[T]) {
        self.try_extend(jzap::iter::from_std(Unknown(v.iter().copied())))
            .unwrap()
    }
}
struct Case<T> {
    values: Vec<T>,
    ascending: Vec<T>,
    pivot: u64,
}
fn cases<T: Item>(seed: u64, small: bool) -> Vec<Case<T>> {
    ordered_corpus::corpus(seed, small)
        .into_iter()
        .map(|c| {
            let values = c.keys.into_iter().map(T::make).collect::<Vec<_>>();
            let mut ascending = values.clone();
            ascending.sort();
            Case {
                values,
                ascending,
                pivot: c.pivot,
            }
        })
        .collect()
}
fn action<'a, T: Item, H: Owner<'a, T>, const OP: usize>(
    case: &Case<T>,
    base: &H,
    other: &H,
    c: &'a Context,
) -> u64 {
    match OP {
        0..=2 => {
            let mut h = H::empty(c, if OP == 1 { case.values.len() } else { 0 });
            for v in if OP == 2 {
                &case.ascending
            } else {
                &case.values
            } {
                h.push(*black_box(v))
            }
            black_box(h).consume()
        }
        3 => {
            let mut h = base.copy();
            let mut s = 0u64;
            while let Some(v) = h.pop() {
                s = s.wrapping_add(black_box(v.key()));
            }
            s
        }
        4 => H::from_values(c, black_box(&case.values)).consume(),
        5 => base.copy().sort(),
        6 | 7 => {
            let (mut a, mut b) = if OP == 6 {
                (base.copy(), other.copy())
            } else {
                (other.copy(), base.copy())
            };
            a.append(&mut b);
            black_box(b);
            a.consume()
        }
        8 => {
            let mut h = base.copy();
            h.retain(black_box(case.pivot));
            h.consume()
        }
        9 => {
            let mut h = base.copy();
            for i in 0..case.values.len() {
                h.edit(black_box(T::make(case.pivot.wrapping_add(i as u64))));
            }
            h.consume()
        }
        10 => base.copy().consume(),
        11 => base.checksum(),
        12 => base.copy().consume(),
        13 => {
            let mut h = base.copy();
            h.drain()
        }
        14 => {
            let mut h = H::empty(c, 0);
            h.extend(black_box(&case.values));
            h.consume()
        }
        _ => unreachable!(),
    }
}
fn measure<'a, T: Item, H: Owner<'a, T>, const OP: usize>(
    cases: &[Case<T>],
    c: &'a Context,
    n: u64,
) -> (f64, u64) {
    let prepared = cases
        .iter()
        .map(|case| {
            (
                H::from_values(c, &case.values),
                H::from_values(c, &case.values[..case.values.len().min(8)]),
            )
        })
        .collect::<Vec<_>>();
    let mut checksum = 0u64;
    for _ in 0..2 {
        for (case, (a, b)) in cases.iter().zip(&prepared) {
            black_box(action::<T, H, OP>(case, a, b, c));
        }
    }
    let start = Instant::now();
    for _ in 0..n {
        for (case, (a, b)) in cases.iter().zip(&prepared) {
            checksum = checksum.wrapping_add(black_box(action::<T, H, OP>(case, a, b, c)));
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    black_box(prepared);
    (elapsed, checksum)
}
fn dispatch<T: Item, const OP: usize>(
    label: &str,
    cases: &[Case<T>],
    c: &Context,
    n: u64,
) -> (f64, u64) {
    match label {
        "std" => measure::<T, Std<T>, OP>(cases, c, n),
        "unlimited" => measure::<T, BinaryHeap<T>, OP>(cases, c, n),
        "local" => measure::<T, BinaryHeap<T, Budget<'_>>, OP>(cases, c, n),
        "concurrent" => measure::<T, BinaryHeap<T, ConcurrentBudget<'_>>, OP>(cases, c, n),
        _ => unreachable!(),
    }
}
#[cfg(feature = "btree-memory")]
fn memory_for<'a, T: Item, H: Owner<'a, T>>(
    label: &str,
    cases: &[Case<T>],
    c: &'a Context,
    seed: u64,
    only: Option<usize>,
) {
    for (ci, case) in cases.iter().enumerate() {
        if only.is_some_and(|n| n != ci) {
            continue;
        }
        for (op, name) in OPS.iter().enumerate() {
            memory::start();
            let mut a = H::empty(c, 0);
            let mut b = H::empty(c, 0);
            if !matches!(op, 0..=2 | 4 | 14) {
                a = H::from_values(c, &case.values);
            }
            if matches!(op, 6 | 7) {
                b = H::from_values(c, &case.values[..case.values.len().min(8)]);
                if op == 7 {
                    std::mem::swap(&mut a, &mut b)
                }
            }
            let mut extra = None;
            match op {
                0..=2 => {
                    if op == 1 {
                        a = H::empty(c, case.values.len());
                    }
                    for v in if op == 2 {
                        &case.ascending
                    } else {
                        &case.values
                    } {
                        a.push(*v)
                    }
                }
                3 => {
                    while let Some(v) = a.pop() {
                        black_box(v);
                    }
                }
                4 => a = H::from_values(c, &case.values),
                5 => {
                    let owned = std::mem::replace(&mut a, H::empty(c, 0));
                    black_box(owned.sort());
                }
                6 | 7 => a.append(&mut b),
                8 => a.retain(case.pivot),
                9 => a.edit(T::make(case.pivot)),
                10 => extra = Some(a.copy()),
                11 => {
                    black_box(a.checksum());
                }
                12 => {
                    let owned = std::mem::replace(&mut a, H::empty(c, 0));
                    black_box(owned.consume());
                }
                13 => {
                    black_box(a.drain());
                }
                14 => a.extend(&case.values),
                _ => unreachable!(),
            }
            let held = memory::snapshot();
            drop((a, b, extra));
            let final_ = memory::stop();
            assert_eq!((final_.live, final_.actual), (0, 0));
            println!(
                "{{\"label\":\"{label}\",\"seed\":{seed},\"case\":{ci},\"operation\":\"{name}_{}\",\"size\":{},\"calls\":{},\"held\":{},\"peak\":{},\"actual_held\":{},\"actual_peak\":{},\"handle\":{},\"width\":{}}}",
                std::mem::size_of::<T>(),
                case.values.len(),
                held.calls,
                held.live,
                held.peak,
                held.actual,
                held.actual_peak,
                std::mem::size_of::<H>(),
                std::mem::size_of::<T>()
            );
        }
    }
}
fn run<T: Item>(label: &str, op: usize, seed: u64, n: u64, small: bool) {
    let cases = cases::<T>(seed, small);
    let c = Context::new();
    macro_rules! go{($($i:literal),*)=>{match op{$($i=>dispatch::<T,$i>(label,&cases,&c,n),)*_=>unreachable!()}};}
    let (elapsed, checksum) = go!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14);
    assert_eq!(
        (c.local.occupied(), c.local.reserved(), c.concurrent.used()),
        (0, 0, 0)
    );
    let mut digest = 0xcbf29ce484222325u64;
    for c in &cases {
        for v in &c.values {
            digest = (digest ^ v.key()).wrapping_mul(0x100000001b3);
        }
        digest = (digest ^ c.pivot).wrapping_mul(0x100000001b3);
    }
    println!(
        "{{\"epochs\":{n},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"checksum\":{checksum},\"elapsed_seconds\":{elapsed}}}",
        n * 64,
        n * cases.iter().map(|c| c.values.len() as u64).sum::<u64>(),
        cases.iter().map(|c| c.values.len()).min().unwrap(),
        cases.iter().map(|c| c.values.len()).max().unwrap()
    );
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|s| s == "tasks") {
        println!(
            "{}",
            [8, 256]
                .into_iter()
                .flat_map(|w| OPS.map(move |s| format!("binary_heap_{s}_{w}")))
                .collect::<Vec<_>>()
                .join(",")
        );
        return;
    }
    if args.get(1).is_some_and(|s| s == "describe") {
        println!(
            "{{\"operation_unit\":\"One complete 64-corpus workload per epoch; operations counts heap workloads. Destructive operations include source cloning, output drop and checksum; iter uses prepared heaps. peek_mut replaces the root once per input element; clone and into_iter both include full clone plus consuming scan. from_vec includes input copy; sort includes clone plus in-place heap sort.\",\"inputs\":\"64 deterministic shuffled corpora, zero/one/maximum and log-uniform sizes up to 4096, small up to 32; u64 and 256-byte array values; three seeds. push_ascending uses sorted ascending copies. append compares both receiver/source size relations.\"}}"
        );
        return;
    }
    #[cfg(feature = "btree-memory")]
    if args.get(1).is_some_and(|s| s == "memory") {
        // Optional label/seed/case/width isolate allocator history for a case.
        let wanted = args.get(2).map(String::as_str);
        let only_seed = args.get(3).map(|s| s.parse::<u64>().unwrap());
        let only_case = args.get(4).map(|s| s.parse::<usize>().unwrap());
        let only_width = args.get(5).map(|s| s.parse::<usize>().unwrap());
        for seed in [617, 6203, 96011] {
            if only_seed.is_some_and(|n| n != seed) {
                continue;
            }
            let c = Context::new();
            macro_rules! observe {
                ($t:ty) => {{
                    if only_width.is_none_or(|w| w == std::mem::size_of::<$t>()) {
                        let cases = cases::<$t>(seed, false);
                        if wanted.is_none_or(|s| s == "std") {
                            memory_for::<$t, Std<$t>>("std", &cases, &c, seed, only_case);
                        }
                        if wanted.is_none_or(|s| s == "unlimited") {
                            memory_for::<$t, BinaryHeap<$t>>(
                                "unlimited",
                                &cases,
                                &c,
                                seed,
                                only_case,
                            );
                        }
                        if wanted.is_none_or(|s| s == "local") {
                            memory_for::<$t, BinaryHeap<$t, Budget<'_>>>(
                                "local", &cases, &c, seed, only_case,
                            );
                        }
                        if wanted.is_none_or(|s| s == "concurrent") {
                            memory_for::<$t, BinaryHeap<$t, ConcurrentBudget<'_>>>(
                                "concurrent",
                                &cases,
                                &c,
                                seed,
                                only_case,
                            );
                        }
                    }
                }};
            }
            observe!(u64);
            observe!([u64; 32]);
        }
        return;
    }
    assert_eq!(args.len(), 6);
    let task = args[2].strip_prefix("binary_heap_").unwrap();
    let (name, width) = task.rsplit_once('_').unwrap();
    let op = OPS.iter().position(|s| *s == name).unwrap();
    let seed = args[3].parse().unwrap();
    let n = args[4].parse().unwrap();
    match width {
        "8" => run::<u64>(&args[1], op, seed, n, args[5] == "small"),
        "256" => run::<[u64; 32]>(&args[1], op, seed, n, args[5] == "small"),
        _ => unreachable!(),
    }
}
