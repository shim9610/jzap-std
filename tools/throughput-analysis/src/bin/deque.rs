//! Equal-work ring workloads; timing and allocator observation use separate builds.
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
    collections::VecDeque,
};
use std::{collections::VecDeque as Std, fmt::Debug, hint::black_box, time::Instant};
const OPS: [&str; 18] = [
    "push_back",
    "push_front",
    "fifo",
    "pop_all",
    "iter",
    "index",
    "insert_remove",
    "rotate",
    "contiguous",
    "shrink",
    "append",
    "split",
    "clone",
    "retain",
    "drain",
    "resize",
    "extend",
    "clone_from",
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
    fn from_values(c: &'a Context, v: &[T]) -> Self;
    fn copy(&self) -> Self;
    fn copy_from(&mut self, source: &Self);
    fn len(&self) -> usize;
    fn push(&mut self, v: T);
    fn front_push(&mut self, v: T);
    fn pop(&mut self) -> Option<T>;
    fn back_pop(&mut self) -> Option<T>;
    fn rotate(&mut self, n: usize);
    fn insert_remove(&mut self, index: usize, v: T) -> T;
    fn contiguous(&mut self) -> u64;
    fn shrink(&mut self);
    fn append(&mut self, other: &mut Self);
    fn split(&mut self, n: usize) -> Self;
    fn retain(&mut self, p: u64);
    fn checksum(&self) -> u64;
    fn index_sum(&self) -> u64;
    fn consume(self) -> u64;
    fn drain(&mut self) -> u64;
    fn resize(&mut self, n: usize, v: T);
    fn extend(&mut self, v: &[T]);
}
fn sum<T: Item>(values: impl Iterator<Item = T>) -> u64 {
    values.fold(0, |a, v| a.wrapping_add(v.key()))
}
impl<'a, T: Item> Owner<'a, T> for Std<T> {
    fn empty(_: &'a Context, cap: usize) -> Self {
        Self::with_capacity(cap)
    }
    fn from_values(_: &'a Context, v: &[T]) -> Self {
        let mut d = Self::from(v.to_vec());
        if !d.is_empty() {
            d.rotate_left(v.len() / 3)
        }
        d
    }
    fn copy(&self) -> Self {
        self.clone()
    }
    fn copy_from(&mut self, source: &Self) {
        self.clone_from(source)
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn push(&mut self, v: T) {
        self.push_back(v)
    }
    fn front_push(&mut self, v: T) {
        self.push_front(v)
    }
    fn pop(&mut self) -> Option<T> {
        self.pop_front()
    }
    fn back_pop(&mut self) -> Option<T> {
        self.pop_back()
    }
    fn rotate(&mut self, n: usize) {
        self.rotate_left(n)
    }
    fn insert_remove(&mut self, i: usize, v: T) -> T {
        self.insert(i, v);
        self.remove(i).unwrap()
    }
    fn contiguous(&mut self) -> u64 {
        sum(self.make_contiguous().iter().copied())
    }
    fn shrink(&mut self) {
        self.shrink_to_fit()
    }
    fn append(&mut self, b: &mut Self) {
        self.append(b)
    }
    fn split(&mut self, n: usize) -> Self {
        self.split_off(n)
    }
    fn retain(&mut self, p: u64) {
        self.retain(|v| v.key() & 3 != p & 3)
    }
    fn checksum(&self) -> u64 {
        sum(self.iter().copied())
    }
    fn index_sum(&self) -> u64 {
        (0..self.len()).fold(0, |a, i| a.wrapping_add(self.get(i).unwrap().key()))
    }
    fn consume(self) -> u64 {
        if cfg!(feature = "deque-borrowed-consume") {
            sum(self.iter().copied())
        } else {
            sum(self.into_iter())
        }
    }
    fn drain(&mut self) -> u64 {
        let len = self.len();
        sum(self.drain(len / 4..len * 3 / 4))
    }
    fn resize(&mut self, n: usize, v: T) {
        self.resize(n, v)
    }
    fn extend(&mut self, v: &[T]) {
        Extend::extend(self, v.iter().copied())
    }
}
impl<'a, T: Item, P: Policy<'a>> Owner<'a, T> for VecDeque<T, P> {
    fn empty(c: &'a Context, cap: usize) -> Self {
        Self::try_with_capacity_in(cap, P::choose(c)).unwrap()
    }
    fn from_values(c: &'a Context, v: &[T]) -> Self {
        let mut d = Self::empty(c, v.len());
        d.try_extend_copy(v).unwrap();
        if !d.is_empty() {
            d.rotate_left(v.len() / 3).unwrap()
        }
        d
    }
    fn copy(&self) -> Self {
        self.try_clone().unwrap()
    }
    fn copy_from(&mut self, source: &Self) {
        self.try_clone_from(source).unwrap()
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn push(&mut self, v: T) {
        self.try_push_back(v).unwrap()
    }
    fn front_push(&mut self, v: T) {
        self.try_push_front(v).unwrap()
    }
    fn pop(&mut self) -> Option<T> {
        self.pop_front()
    }
    fn back_pop(&mut self) -> Option<T> {
        self.pop_back()
    }
    fn rotate(&mut self, n: usize) {
        self.rotate_left(n).unwrap()
    }
    fn insert_remove(&mut self, i: usize, v: T) -> T {
        self.try_insert(i, v).unwrap();
        self.remove(i).unwrap()
    }
    fn contiguous(&mut self) -> u64 {
        sum(self.make_contiguous().iter().copied())
    }
    fn shrink(&mut self) {
        self.try_shrink_to_fit().unwrap()
    }
    fn append(&mut self, b: &mut Self) {
        self.try_append(b).unwrap()
    }
    fn split(&mut self, n: usize) -> Self {
        self.try_split_off(n).unwrap()
    }
    fn retain(&mut self, p: u64) {
        self.try_retain(|v| Ok::<_, ()>(v.key() & 3 != p & 3))
            .unwrap()
    }
    fn checksum(&self) -> u64 {
        sum(self.iter().copied())
    }
    fn index_sum(&self) -> u64 {
        (0..self.len()).fold(0, |a, i| a.wrapping_add(self.get(i).unwrap().key()))
    }
    fn consume(self) -> u64 {
        if cfg!(feature = "deque-borrowed-consume") {
            sum(self.iter().copied())
        } else {
            sum(self.into_iter())
        }
    }
    fn drain(&mut self) -> u64 {
        let len = self.len();
        sum(self.drain(len / 4..len * 3 / 4).unwrap())
    }
    fn resize(&mut self, n: usize, v: T) {
        self.try_resize(n, &v).unwrap()
    }
    fn extend(&mut self, v: &[T]) {
        self.try_extend_copy(v).unwrap()
    }
}
struct Case<T> {
    values: Vec<T>,
    pivot: u64,
}
fn cases<T: Item>(seed: u64, small: bool) -> Vec<Case<T>> {
    ordered_corpus::corpus(seed, small)
        .into_iter()
        .map(|c| {
            let values = c.keys.into_iter().map(T::make).collect::<Vec<_>>();
            Case {
                values,
                pivot: c.pivot,
            }
        })
        .collect()
}
#[cfg_attr(feature = "deque-outline-action", inline(never))]
fn action<'a, T: Item, H: Owner<'a, T>, const OP: usize>(
    case: &Case<T>,
    base: &H,
    other: &H,
    c: &'a Context,
) -> u64 {
    if OP <= 1 || OP == 16 {
        let mut d = H::empty(c, 0);
        if OP == 16 {
            d.extend(&case.values)
        } else {
            for v in &case.values {
                if OP == 0 {
                    d.push(*black_box(v))
                } else {
                    d.front_push(*black_box(v))
                }
            }
        }
        return black_box(d).consume();
    }
    if OP == 4 {
        return base.checksum();
    }
    if OP == 5 {
        return base.index_sum();
    }
    if OP == 12 {
        return black_box(base.copy()).consume();
    }
    let mut d = base.copy();
    let len = d.len();
    if len != 0 {
        d.rotate(len / 3)
    }
    match OP {
        2 => {
            for _ in 0..len {
                if let Some(v) = d.pop() {
                    d.push(black_box(v))
                }
            }
        }
        3 => {
            let mut out = 0u64;
            while let Some(v) = d.back_pop() {
                out = out.wrapping_add(black_box(v).key())
            }
            return out;
        }
        6 => {
            black_box(d.insert_remove(len / 2, T::make(case.pivot)));
        }
        7 => {
            let n = len / 3;
            d.rotate(n);
            d.rotate(len - n)
        }
        8 => return d.contiguous(),
        9 => {
            for _ in 0..len / 2 {
                black_box(d.pop());
            }
            d.shrink()
        }
        10 => d.append(&mut other.copy()),
        11 => {
            let tail = d.split(len / 2);
            return d.consume().wrapping_add(tail.consume());
        }
        13 => d.retain(case.pivot),
        14 => {
            let removed = d.drain();
            return removed.wrapping_add(d.consume());
        }
        15 => d.resize(len * 2, T::make(case.pivot)),
        _ => unreachable!(),
    }
    black_box(d).consume()
}
fn measure<'a, T: Item, H: Owner<'a, T>, const OP: usize>(
    cases: &[Case<T>],
    c: &'a Context,
    n: u64,
) -> (f64, u64) {
    if OP == 17 {
        return measure_clone_from::<T, H>(cases, c, n);
    }
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
#[inline(never)]
fn measure_clone_from<'a, T: Item, H: Owner<'a, T>>(
    cases: &[Case<T>],
    c: &'a Context,
    n: u64,
) -> (f64, u64) {
    let mut prepared = cases
        .iter()
        .map(|case| {
            (
                H::from_values(c, &case.values),
                H::from_values(c, &case.values),
            )
        })
        .collect::<Vec<_>>();
    for _ in 0..2 {
        for (source, target) in &mut prepared {
            target.copy_from(source);
            black_box(target.checksum());
        }
    }
    let mut checksum = 0u64;
    let start = Instant::now();
    for _ in 0..n {
        for (source, target) in &mut prepared {
            target.copy_from(source);
            checksum = checksum.wrapping_add(black_box(target.checksum()));
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
        "unlimited" => measure::<T, VecDeque<T>, OP>(cases, c, n),
        "local" => measure::<T, VecDeque<T, Budget<'_>>, OP>(cases, c, n),
        "concurrent" => measure::<T, VecDeque<T, ConcurrentBudget<'_>>, OP>(cases, c, n),
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
            let mut d = if op <= 1 || op == 16 {
                H::empty(c, 0)
            } else {
                H::from_values(c, &case.values)
            };
            let mut extra = None;
            let n = d.len();
            match op {
                0 | 1 => {
                    for v in &case.values {
                        if op == 0 {
                            d.push(*v)
                        } else {
                            d.front_push(*v)
                        }
                    }
                }
                2 => {
                    for _ in 0..n {
                        if let Some(v) = d.pop() {
                            d.push(v)
                        }
                    }
                }
                3 => {
                    while let Some(v) = d.back_pop() {
                        black_box(v);
                    }
                }
                4 => {
                    black_box(d.checksum());
                }
                5 => {
                    black_box(d.index_sum());
                }
                6 => {
                    black_box(d.insert_remove(n / 2, T::make(case.pivot)));
                }
                7 => {
                    d.rotate(n / 3);
                    d.rotate(n - n / 3)
                }
                8 => {
                    black_box(d.contiguous());
                }
                9 => {
                    for _ in 0..n / 2 {
                        black_box(d.pop());
                    }
                    d.shrink()
                }
                10 => {
                    let mut b = H::from_values(c, &case.values[..case.values.len().min(8)]);
                    d.append(&mut b);
                    extra = Some(b)
                }
                11 => extra = Some(d.split(n / 2)),
                12 => extra = Some(d.copy()),
                13 => d.retain(case.pivot),
                14 => {
                    black_box(d.drain());
                }
                15 => d.resize(n * 2, T::make(case.pivot)),
                16 => d.extend(&case.values),
                17 => {
                    let source = H::from_values(c, &case.values);
                    d.copy_from(&source);
                    extra = Some(source);
                }
                _ => unreachable!(),
            }
            let held = memory::snapshot();
            drop((d, extra));
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
    let (elapsed, checksum) = go!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17);
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
                .flat_map(|w| OPS.map(move |s| format!("deque_{s}_{w}")))
                .collect::<Vec<_>>()
                .join(",")
        );
        return;
    }
    if args.get(1).is_some_and(|s| s == "describe") {
        let scan = if cfg!(feature = "deque-borrowed-consume") {
            "borrowed"
        } else {
            "owning"
        };
        let boundary = if cfg!(feature = "deque-outline-action") {
            "outlined"
        } else {
            "natural"
        };
        println!(
            "{{\"operation_unit\":\"One whole deque workload, with source clone/drop/checksum for destructive operations; iter and index use prepared wrapped rings; clone_from copies between prepared equal-capacity owners then checksums the destination. FIFO performs one pop-front/push-back per input element. rotate performs left/right restoration, insert_remove performs one middle insertion/removal.\",\"inputs\":\"64 deterministic random corpora, zero/one/4096 and log-uniform lengths; small capped at 32; u64 and 256-byte values, caller-selected input seeds. Input generation is outside timing.\",\"consume_scan\":\"{scan}\",\"action_boundary\":\"{boundary}\"}}"
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
                            memory_for::<$t, VecDeque<$t>>(
                                "unlimited",
                                &cases,
                                &c,
                                seed,
                                only_case,
                            );
                        }
                        if wanted.is_none_or(|s| s == "local") {
                            memory_for::<$t, VecDeque<$t, Budget<'_>>>(
                                "local", &cases, &c, seed, only_case,
                            );
                        }
                        if wanted.is_none_or(|s| s == "concurrent") {
                            memory_for::<$t, VecDeque<$t, ConcurrentBudget<'_>>>(
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
    let task = args[2].strip_prefix("deque_").unwrap();
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
