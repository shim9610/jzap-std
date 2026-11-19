use jzap::{
    TryClone,
    budget::{Budget, MemoryBudget, Unmetered},
    vec::{LengthLimit, Unlimited, Vec as V},
};
use std::{hint::black_box, time::Instant};
#[cfg(feature = "memory")]
mod tracking;
struct NoHint<I>(I);
impl<I: Iterator> Iterator for NoHint<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
trait Owner<'a>: Sized {
    fn new(n: usize, b: &'a MemoryBudget) -> Self;
    fn reserve(&mut self, n: usize);
    fn push(&mut self, x: u64);
    fn copy(&mut self, x: &[u64]);
    fn clone_owner(&self) -> Self;
    fn clear(&mut self);
    fn slice(&self) -> &[u64];
    fn edit<const OP: u8>(&mut self);
}
impl<'a> Owner<'a> for Vec<u64> {
    #[inline(always)]
    fn edit<const OP: u8>(&mut self) {
        let n = self.len();
        match OP {
            4 => self.retain(|x| *x % 2 == 0),
            5 => self.dedup_by_key(|x| *x / 3),
            6 => drop(self.drain(n / 4..n / 2)),
            7 | 12 | 13 => {
                for x in self.extract_if(n / 4..n * 3 / 4, |x| match OP {
                    12 => *x >= n as u64,
                    13 => *x < n as u64,
                    _ => *x % 2 == 0,
                }) {
                    black_box(x);
                }
            }
            8 => drop(self.splice(n / 4..n / 2, (0..n / 2).map(|x| x as u64))),
            10 => drop(self.splice(n / 4..n / 2, (0..n / 2).filter(|_| true).map(|x| x as u64))),
            11 => drop(self.splice(n / 4..n / 2, NoHint((0..n / 2).map(|x| x as u64)))),
            9 => self.extend_from_within(n / 4..n * 3 / 4),
            14 => {
                self.insert(n / 2, 17);
                black_box(self.remove(n / 2));
            }
            15 => self.resize(n * 2, 17),
            16 => {
                let tail = self.split_off(n / 2);
                black_box(&tail);
                drop(tail);
            }
            17 => {
                let mut other = self.clone();
                self.append(&mut other);
                black_box(&other);
            }
            18 => {
                self.truncate(n / 2);
                self.shrink_to_fit();
            }
            19 => {
                while let Some(x) = self.pop() {
                    black_box(x);
                }
            }
            _ => panic!("edit operation"),
        }
    }
    #[inline(always)]
    fn new(_: usize, _: &'a MemoryBudget) -> Self {
        Self::new()
    }
    #[inline(always)]
    fn reserve(&mut self, n: usize) {
        self.reserve_exact(n)
    }
    #[inline(always)]
    fn push(&mut self, x: u64) {
        self.push(x)
    }
    #[inline(always)]
    fn copy(&mut self, x: &[u64]) {
        self.extend_from_slice(x)
    }
    #[inline(always)]
    fn clone_owner(&self) -> Self {
        self.clone()
    }
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
    #[inline(always)]
    fn slice(&self) -> &[u64] {
        self.as_slice()
    }
}
macro_rules! owner {
    ($ty:ty,$ctor:expr) => {
        impl<'a> Owner<'a> for $ty {
            #[inline(always)]
            fn edit<const OP: u8>(&mut self) {
                let n = self.len();
                match OP {
                    4 => self.retain(|x| *x % 2 == 0),
                    5 => self.dedup_by_key(|x| *x / 3),
                    6 => drop(self.try_drain(n / 4..n / 2).unwrap()),
                    7 | 12 | 13 => {
                        for x in self
                            .try_extract_if(n / 4..n * 3 / 4, |x| {
                                Ok::<_, ()>(match OP {
                                    12 => *x >= n as u64,
                                    13 => *x < n as u64,
                                    _ => *x % 2 == 0,
                                })
                            })
                            .unwrap()
                        {
                            black_box(x.unwrap());
                        }
                    }
                    8 => self
                        .try_splice(n / 4..n / 2, (0..n / 2).map(|x| x as u64))
                        .unwrap()
                        .finish()
                        .unwrap(),
                    10 => self
                        .try_splice(n / 4..n / 2, (0..n / 2).filter(|_| true).map(|x| x as u64))
                        .unwrap()
                        .finish()
                        .unwrap(),
                    11 => self
                        .try_splice(n / 4..n / 2, NoHint((0..n / 2).map(|x| x as u64)))
                        .unwrap()
                        .finish()
                        .unwrap(),
                    9 => self.try_extend_from_within(n / 4..n * 3 / 4).unwrap(),
                    14 => {
                        self.try_insert(n / 2, 17).unwrap();
                        black_box(self.remove(n / 2).unwrap());
                    }
                    15 => self.try_resize(n * 2, &17).unwrap(),
                    16 => {
                        let tail = self.try_split_off(n / 2).unwrap();
                        black_box(&tail);
                        drop(tail);
                    }
                    17 => {
                        let mut other = self.try_clone().unwrap();
                        self.try_append(&mut other).unwrap();
                        black_box(&other);
                    }
                    18 => {
                        self.truncate(n / 2);
                        self.try_shrink_to_fit().unwrap();
                    }
                    19 => {
                        while let Some(x) = self.pop() {
                            black_box(x);
                        }
                    }
                    _ => panic!("edit operation"),
                }
            }
            #[inline(always)]
            fn new(n: usize, b: &'a MemoryBudget) -> Self {
                ($ctor)(n, b)
            }
            #[inline(always)]
            fn reserve(&mut self, n: usize) {
                self.try_reserve_exact(n).unwrap()
            }
            #[inline(always)]
            fn push(&mut self, x: u64) {
                self.try_push(x).unwrap()
            }
            #[inline(always)]
            fn copy(&mut self, x: &[u64]) {
                self.try_extend_copy(x).unwrap()
            }
            #[inline(always)]
            fn clone_owner(&self) -> Self {
                self.try_clone().unwrap()
            }
            #[inline(always)]
            fn clear(&mut self) {
                self.clear()
            }
            #[inline(always)]
            fn slice(&self) -> &[u64] {
                self.as_slice()
            }
        }
    };
}
owner!(V<u64>, |_: usize, _: &'a MemoryBudget| V::new());
owner!(V<u64,Unmetered,LengthLimit>,|n:usize,_:&'a MemoryBudget|V::with_limit(n));
owner!(V<u64,Budget<'a>,Unlimited>,|_:usize,b:&'a MemoryBudget|V::new_in(Budget::Limited(b)));
owner!(V<u64,Budget<'a>,LengthLimit>,|n:usize,b:&'a MemoryBudget|V::<u64,_>::with_limit_in(n,Budget::Limited(b)));

#[inline(never)]
fn run<'a, T: Owner<'a>, const OP: u8>(label: &str, n: usize, budget: &'a MemoryBudget) {
    let op = match OP {
        0 => "grow",
        1 => "reserved",
        2 => "copy",
        3 => "clone",
        4 => "retain",
        5 => "dedup",
        6 => "drain",
        7 => "extract",
        8 => "splice",
        9 => "within",
        10 => "splice_unknown",
        11 => "splice_no_hint",
        12 => "extract_none",
        13 => "extract_all",
        14 => "insert_remove",
        15 => "resize",
        16 => "split_off",
        17 => "append",
        18 => "shrink",
        19 => "pop",
        _ => unreachable!(),
    };
    let input: Vec<u64> = (0..n as u64).collect();
    let mut source = T::new(n.saturating_mul(2).max(1), budget);
    source.reserve(n);
    source.copy(&input);
    let reps = (262144 / n.max(1)).clamp(4, 8192);
    let mut samples = Vec::with_capacity(30);
    for cycle in 0..32 {
        let t = Instant::now();
        for _ in 0..reps {
            match OP {
                0 => {
                    let mut v = T::new(black_box(n), budget);
                    for i in 0..black_box(n) {
                        v.push(black_box(i as u64));
                    }
                    black_box(&v);
                    drop(v);
                }
                1 => {
                    source.clear();
                    for i in 0..black_box(n) {
                        source.push(black_box(i as u64));
                    }
                    black_box(&source);
                }
                2 => {
                    let mut v = T::new(black_box(n), budget);
                    v.copy(black_box(&input));
                    black_box(&v);
                    drop(v);
                }
                3 => {
                    let v = black_box(&source).clone_owner();
                    black_box(&v);
                    drop(v);
                }
                4..=19 => {
                    let mut v = black_box(&source).clone_owner();
                    v.edit::<OP>();
                    black_box(v.slice());
                    drop(v);
                }
                _ => panic!("unknown operation"),
            }
        }
        let elapsed = t.elapsed().as_nanos() as u64;
        if cycle >= 2 {
            samples.push(elapsed);
        }
    }
    assert_eq!(source.slice(), input);
    println!(
        "{{\"label\":\"{label}\",\"op\":\"{op}\",\"n\":{n},\"repetitions\":{reps},\"handle\":{},\"samples_ns\":{samples:?}}}",
        std::mem::size_of::<T>()
    );
}
#[cfg(feature = "memory")]
fn memory<'a, T: Owner<'a>>(
    label: &str,
    n: usize,
    budget: &'a MemoryBudget,
    moving: bool,
    op: &str,
) {
    // Input construction is outside the measured allocator interval, as in
    // timing(). Keep its storage alive until after tracking stops.
    let input: std::vec::Vec<u64> = if op == "copy" {
        (0..n).map(|i| i as u64).collect()
    } else {
        std::vec::Vec::new()
    };
    tracking::start(moving);
    let mut v = T::new(n.saturating_mul(2).max(1), budget);
    if op == "copy" {
        v.copy(&input);
    } else {
        for i in 0..n {
            v.push(i as u64);
        }
    }
    match op {
        "grow" | "copy" => {}
        "retain" => v.edit::<4>(),
        "dedup" => v.edit::<5>(),
        "drain" => v.edit::<6>(),
        "extract" => v.edit::<7>(),
        "splice" => v.edit::<8>(),
        "within" => v.edit::<9>(),
        "splice_unknown" => v.edit::<10>(),
        "splice_no_hint" => v.edit::<11>(),
        "extract_none" => v.edit::<12>(),
        "extract_all" => v.edit::<13>(),
        "insert_remove" => v.edit::<14>(),
        "resize" => v.edit::<15>(),
        "split_off" => v.edit::<16>(),
        "append" => v.edit::<17>(),
        "shrink" => v.edit::<18>(),
        "pop" => v.edit::<19>(),
        _ => panic!("memory operation"),
    }
    let build = tracking::snapshot();
    let clone = v.clone_owner();
    let cloned = tracking::snapshot();
    drop(clone);
    drop(v);
    let end = tracking::stop();
    println!(
        "{{\"label\":\"{label}\",\"n\":{n},\"handle\":{},\"build\":{build:?},\"cloned\":{cloned:?},\"end\":{end:?}}}",
        std::mem::size_of::<T>()
    );
    assert_eq!(end[0], 0);
    assert_eq!(end[2], 0);
}
fn main() {
    let a: Vec<_> = std::env::args().collect();
    let label = a.get(1).expect("label");
    let op = a.get(2).expect("operation");
    let n: usize = a.get(3).expect("n").parse().unwrap();
    let budget = MemoryBudget::new(usize::MAX);
    macro_rules! dispatch {
        ($ty:ty) => {{
            #[cfg(feature = "memory")]
            {
                let moving = op.starts_with("memory-moving");
                let edit = op
                    .strip_prefix("memory-moving-")
                    .or_else(|| op.strip_prefix("memory-"))
                    .filter(|x| *x != "moving")
                    .unwrap_or("grow");
                memory::<$ty>(label, n, &budget, moving, edit);
            }
            #[cfg(not(feature = "memory"))]
            match op.as_str() {
                "grow" => run::<$ty, 0>(label, n, &budget),
                "reserved" => run::<$ty, 1>(label, n, &budget),
                "copy" => run::<$ty, 2>(label, n, &budget),
                "clone" => run::<$ty, 3>(label, n, &budget),
                "retain" => run::<$ty, 4>(label, n, &budget),
                "dedup" => run::<$ty, 5>(label, n, &budget),
                "drain" => run::<$ty, 6>(label, n, &budget),
                "extract" => run::<$ty, 7>(label, n, &budget),
                "splice" => run::<$ty, 8>(label, n, &budget),
                "within" => run::<$ty, 9>(label, n, &budget),
                "splice_unknown" => run::<$ty, 10>(label, n, &budget),
                "splice_no_hint" => run::<$ty, 11>(label, n, &budget),
                "extract_none" => run::<$ty, 12>(label, n, &budget),
                "extract_all" => run::<$ty, 13>(label, n, &budget),
                "insert_remove" => run::<$ty, 14>(label, n, &budget),
                "resize" => run::<$ty, 15>(label, n, &budget),
                "split_off" => run::<$ty, 16>(label, n, &budget),
                "append" => run::<$ty, 17>(label, n, &budget),
                "shrink" => run::<$ty, 18>(label, n, &budget),
                "pop" => run::<$ty, 19>(label, n, &budget),
                _ => panic!("operation"),
            }
        }};
    }
    match label.as_str() {
        "std" => dispatch!(Vec<u64>),
        "unlimited" => dispatch!(V<u64>),
        "limited" => dispatch!(V<u64,Unmetered,LengthLimit>),
        "metered" => dispatch!(V<u64,Budget<'_>>),
        "limited_metered" => dispatch!(V<u64,Budget<'_>,LengthLimit>),
        _ => panic!("label"),
    }
    assert_eq!(budget.occupied(), 0);
    assert_eq!(budget.reserved(), 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare<'a, T: Owner<'a>, const OP: u8>(n: usize, b: &'a MemoryBudget) {
        let input: Vec<_> = (0..n as u64).collect();
        let mut expected = input.clone();
        let mut actual = T::new((n * 2).max(1), b);
        actual.copy(&input);
        expected.edit::<OP>();
        actual.edit::<OP>();
        assert_eq!(actual.slice(), expected.as_slice());
    }

    #[test]
    fn additional_workloads_match_std_for_every_policy() {
        let budget = MemoryBudget::new(usize::MAX);
        for n in [0, 1, 3, 8, 32, 4096] {
            macro_rules! check {
                ($op:literal) => {
                    compare::<V<u64>, $op>(n, &budget);
                    compare::<V<u64, Unmetered, LengthLimit>, $op>(n, &budget);
                    compare::<V<u64, Budget<'_>>, $op>(n, &budget);
                    compare::<V<u64, Budget<'_>, LengthLimit>, $op>(n, &budget);
                };
            }
            check!(14);
            check!(15);
            check!(16);
            check!(17);
            check!(18);
            check!(19);
            assert_eq!(budget.occupied(), 0);
            assert_eq!(budget.reserved(), 0);
        }
    }
}
