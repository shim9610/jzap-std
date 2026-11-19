//! Equal-work long-interval shared-ownership benchmarks, including hot handles.
#[cfg(feature = "shared-memory")]
#[path = "../string_memory.rs"]
mod memory;
use jzap::{
    TryClone,
    budget::{
        Budget, ConcurrentBudget, ConcurrentMemoryBudget, MemoryBudget, OwnedBudget,
        OwnedConcurrentBudget, Unmetered,
    },
    shared::{SharedPolicy, Strong, Weak, count::Counter},
};
use std::{hint::black_box, time::Instant};
const OPS: [&str; 15] = [
    "construct",
    "clone",
    "weak_clone",
    "downgrade",
    "upgrade",
    "get_mut",
    "unwrap",
    "cow_shared",
    "cow_weak",
    "cyclic",
    "slice",
    "cow_unique",
    "cyclic_infallible",
    "slice_clone",
    "slice_cow",
];
#[derive(Clone, Copy, Debug)]
struct Data([u64; 8]);
impl TryClone for Data {
    fn try_clone(&self) -> Result<Self, jzap::Error> {
        Ok(*self)
    }
}
struct Context {
    local: MemoryBudget,
    concurrent: ConcurrentMemoryBudget,
    owned_local: OwnedBudget,
    owned_concurrent: OwnedConcurrentBudget,
}
impl Context {
    fn new() -> Self {
        Self {
            local: MemoryBudget::new(usize::MAX),
            concurrent: ConcurrentMemoryBudget::new(usize::MAX),
            owned_local: OwnedBudget::try_new(usize::MAX).unwrap(),
            owned_concurrent: OwnedConcurrentBudget::try_new(usize::MAX).unwrap(),
        }
    }
    fn empty(&self) {
        assert_eq!(
            (
                self.local.occupied(),
                self.local.reserved(),
                self.concurrent.used(),
                self.owned_local.occupied(),
                self.owned_local.reserved(),
                self.owned_concurrent.used(),
            ),
            (0, 0, 0, 0, 0, 0)
        );
    }
}
trait Policy<'a>: SharedPolicy {
    fn select(context: &'a Context) -> Self;
}
impl<'a> Policy<'a> for Unmetered {
    fn select(_: &'a Context) -> Self {
        Self
    }
}
impl<'a> Policy<'a> for Budget<'a> {
    fn select(c: &'a Context) -> Self {
        Budget::Limited(&c.local)
    }
}
impl<'a> Policy<'a> for ConcurrentBudget<'a> {
    fn select(c: &'a Context) -> Self {
        Self(&c.concurrent)
    }
}
impl<'a> Policy<'a> for OwnedBudget {
    fn select(c: &'a Context) -> Self {
        c.owned_local.try_clone().unwrap()
    }
}
impl<'a> Policy<'a> for OwnedConcurrentBudget {
    fn select(c: &'a Context) -> Self {
        c.owned_concurrent.try_clone().unwrap()
    }
}
trait Owner<'a>: Sized {
    type Weak;
    fn make(data: Data, context: &'a Context) -> Self;
    fn copy(&self) -> Self;
    fn downgrade(&self) -> Self::Weak;
    fn weak_copy(w: &Self::Weak) -> Self::Weak;
    fn upgrade(w: &Self::Weak) -> Option<Self>;
    fn read(&self) -> u64;
    fn unique(&mut self) -> Option<&mut Data>;
    fn unwrap(self) -> Data;
    fn cow(&mut self) -> &mut Data;
    fn cyclic(data: Data, context: &'a Context) -> Self;
    fn cyclic_infallible(data: Data, context: &'a Context) -> Self;
}
macro_rules! standard {
    ($strong:path,$weak:path) => {
        impl<'a> Owner<'a> for $strong {
            type Weak = $weak;
            #[inline(always)]
            fn make(d: Data, _: &'a Context) -> Self {
                Self::new(d)
            }
            #[inline(always)]
            fn copy(&self) -> Self {
                self.clone()
            }
            #[inline(always)]
            fn downgrade(&self) -> Self::Weak {
                Self::downgrade(self)
            }
            #[inline(always)]
            fn weak_copy(w: &Self::Weak) -> Self::Weak {
                w.clone()
            }
            #[inline(always)]
            fn upgrade(w: &Self::Weak) -> Option<Self> {
                w.upgrade()
            }
            #[inline(always)]
            fn read(&self) -> u64 {
                self.0[0]
            }
            #[inline(always)]
            fn unique(&mut self) -> Option<&mut Data> {
                Self::get_mut(self)
            }
            #[inline(always)]
            fn unwrap(self) -> Data {
                Self::try_unwrap(self).unwrap()
            }
            #[inline(always)]
            fn cow(&mut self) -> &mut Data {
                Self::make_mut(self)
            }
            #[inline(always)]
            fn cyclic_infallible(d: Data, c: &'a Context) -> Self {
                <Self as Owner>::cyclic(d, c)
            }
            #[inline(always)]
            fn cyclic(d: Data, _: &'a Context) -> Self {
                Self::new_cyclic(|w| {
                    black_box(w.upgrade());
                    d
                })
            }
        }
    };
}
standard!(std::rc::Rc<Data>, std::rc::Weak<Data>);
standard!(std::sync::Arc<Data>, std::sync::Weak<Data>);
impl<'a, C: Counter, P: Policy<'a>> Owner<'a> for Strong<Data, C, P> {
    type Weak = Weak<Data, C, P>;
    #[inline(always)]
    fn make(d: Data, c: &'a Context) -> Self {
        Self::try_new_in(d, P::select(c)).unwrap()
    }
    #[inline(always)]
    fn copy(&self) -> Self {
        Self::try_clone(self).unwrap()
    }
    #[inline(always)]
    fn downgrade(&self) -> Self::Weak {
        Self::try_downgrade(self).unwrap()
    }
    #[inline(always)]
    fn weak_copy(w: &Self::Weak) -> Self::Weak {
        w.try_clone().unwrap()
    }
    #[inline(always)]
    fn upgrade(w: &Self::Weak) -> Option<Self> {
        w.try_upgrade().unwrap()
    }
    #[inline(always)]
    fn read(&self) -> u64 {
        self.0[0]
    }
    #[inline(always)]
    fn unique(&mut self) -> Option<&mut Data> {
        Self::get_mut(self)
    }
    #[inline(always)]
    fn unwrap(self) -> Data {
        Self::try_unwrap(self).unwrap()
    }
    #[inline(always)]
    fn cow(&mut self) -> &mut Data {
        Self::try_make_mut(self).unwrap()
    }
    #[inline(always)]
    fn cyclic_infallible(d: Data, c: &'a Context) -> Self {
        Self::try_new_cyclic_in(P::select(c), |w| {
            black_box(w.try_upgrade().unwrap());
            Ok::<_, core::convert::Infallible>(d)
        })
        .unwrap()
    }
    #[inline(always)]
    fn cyclic(d: Data, c: &'a Context) -> Self {
        Self::try_new_cyclic_in(P::select(c), |w| {
            black_box(w.try_upgrade().unwrap());
            Ok::<_, ()>(d)
        })
        .unwrap()
    }
}
// Slice construction has its own monomorphized helper, preserving actual DSTs.
trait SliceOwner<'a>: Owner<'a> {
    type Slice: AsRef<[u64]>;
    fn make_slice(source: &[u64], c: &'a Context) -> Self::Slice;
    fn make_cloned_slice(source: &[u64], c: &'a Context) -> Self::Slice;
    fn clone_slice(source: &Self::Slice) -> Self::Slice;
    fn cow_slice(source: &mut Self::Slice) -> &mut [u64];
}
impl<'a> SliceOwner<'a> for std::rc::Rc<Data> {
    type Slice = std::rc::Rc<[u64]>;
    #[inline(always)]
    fn make_slice(s: &[u64], _: &'a Context) -> Self::Slice {
        s.into()
    }
    #[inline(always)]
    fn make_cloned_slice(s: &[u64], c: &'a Context) -> Self::Slice {
        Self::make_slice(s, c)
    }
    #[inline(always)]
    fn clone_slice(s: &Self::Slice) -> Self::Slice {
        s.clone()
    }
    #[inline(always)]
    fn cow_slice(s: &mut Self::Slice) -> &mut [u64] {
        std::rc::Rc::make_mut(s)
    }
}
impl<'a> SliceOwner<'a> for std::sync::Arc<Data> {
    type Slice = std::sync::Arc<[u64]>;
    #[inline(always)]
    fn make_slice(s: &[u64], _: &'a Context) -> Self::Slice {
        s.into()
    }
    #[inline(always)]
    fn make_cloned_slice(s: &[u64], c: &'a Context) -> Self::Slice {
        Self::make_slice(s, c)
    }
    #[inline(always)]
    fn clone_slice(s: &Self::Slice) -> Self::Slice {
        s.clone()
    }
    #[inline(always)]
    fn cow_slice(s: &mut Self::Slice) -> &mut [u64] {
        std::sync::Arc::make_mut(s)
    }
}
impl<'a, C: Counter, P: Policy<'a>> SliceOwner<'a> for Strong<Data, C, P> {
    type Slice = Strong<[u64], C, P>;
    #[inline(always)]
    fn make_slice(s: &[u64], c: &'a Context) -> Self::Slice {
        Strong::<[u64], C, P>::try_copy_from_slice_in(s, P::select(c)).unwrap()
    }
    #[inline(always)]
    fn make_cloned_slice(s: &[u64], c: &'a Context) -> Self::Slice {
        Strong::<[u64], C, P>::try_from_slice_in(s, P::select(c)).unwrap()
    }
    #[inline(always)]
    fn clone_slice(s: &Self::Slice) -> Self::Slice {
        Strong::try_clone(s).unwrap()
    }
    #[inline(always)]
    fn cow_slice(s: &mut Self::Slice) -> &mut [u64] {
        Strong::try_make_mut(s).unwrap()
    }
}
struct Case {
    data: Data,
    source: Vec<u64>,
}
fn next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut n = *seed;
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
fn corpus(seed: u64, small: bool) -> Vec<Case> {
    let mut rng = seed;
    let mut out = Vec::new();
    let max = if small { 32 } else { 4096usize };
    for i in 0..64 {
        let n = match i {
            0 => 0,
            1 => 1,
            2 => max,
            _ => {
                let e = next(&mut rng) as u32 % (max.ilog2() + 1);
                ((1usize << e) + (next(&mut rng) as usize % (1usize << e))).min(max)
            }
        };
        let data = Data(std::array::from_fn(|_| next(&mut rng)));
        let source = (0..n).map(|_| next(&mut rng)).collect();
        out.push(Case { data, source });
    }
    for i in (1..out.len()).rev() {
        let j = next(&mut rng) as usize % (i + 1);
        out.swap(i, j);
    }
    out
}
#[inline(never)]
fn epochs<'a, T: SliceOwner<'a>, const OP: usize>(
    cases: &[Case],
    sources: &mut [T],
    weaks: &[T::Weak],
    c: &'a Context,
    n: u64,
) {
    for _ in 0..n {
        for (index, (case, source)) in cases.iter().zip(sources.iter_mut()).enumerate() {
            match OP {
                0 => {
                    black_box(T::make(black_box(case.data), c));
                }
                1 => {
                    black_box(source.copy());
                }
                2 => {
                    black_box(T::weak_copy(black_box(&weaks[index])));
                }
                3 => {
                    black_box(source.downgrade());
                }
                4 => {
                    black_box(T::upgrade(black_box(&weaks[index])).unwrap());
                }
                5 => {
                    black_box(source.unique().unwrap());
                }
                6 => {
                    black_box(T::make(black_box(case.data), c).unwrap());
                }
                7 => {
                    let mut a = source.copy();
                    black_box(a.cow());
                    black_box(a);
                }
                8 => {
                    let mut a = T::make(black_box(case.data), c);
                    let w = a.downgrade();
                    black_box(a.cow());
                    black_box(a);
                    black_box(w);
                }
                9 => {
                    black_box(T::cyclic(black_box(case.data), c));
                }
                10 => {
                    black_box(T::make_slice(black_box(&case.source), c));
                }
                11 => {
                    black_box(source.cow());
                }
                12 => {
                    black_box(T::cyclic_infallible(black_box(case.data), c));
                }
                13 => {
                    black_box(T::make_cloned_slice(black_box(&case.source), c));
                }
                14 => {
                    let a = T::make_slice(black_box(&case.source), c);
                    let mut b = T::clone_slice(&a);
                    black_box(T::cow_slice(&mut b));
                    black_box((a, b));
                }
                _ => unreachable!(),
            }
        }
    }
}
fn run<'a, T: SliceOwner<'a>, const OP: usize>(cases: &[Case], c: &'a Context, n: u64) -> f64 {
    let mut sources: Vec<T> = cases.iter().map(|r| T::make(r.data, c)).collect();
    for (s, r) in sources.iter().zip(cases) {
        assert_eq!(s.read(), r.data.0[0]);
    }
    for (source, case) in sources.iter().zip(cases) {
        let a = source.copy();
        assert_eq!(a.read(), case.data.0[0]);
        drop(a);
        let w = source.downgrade();
        let q = T::weak_copy(&w);
        assert_eq!(T::upgrade(&q).unwrap().read(), case.data.0[0]);
        drop((w, q));
        if OP == 9 {
            assert_eq!(T::cyclic(case.data, c).unwrap().0, case.data.0);
        }
        if OP == 12 {
            assert_eq!(T::cyclic_infallible(case.data, c).unwrap().0, case.data.0);
        }
        if OP == 6 {
            assert_eq!(T::make(case.data, c).unwrap().0, case.data.0);
        }
        if OP == 7 {
            let mut copy = source.copy();
            copy.cow().0[0] ^= 1;
            assert_eq!(source.read(), case.data.0[0]);
        }
        if OP == 8 {
            let mut a = T::make(case.data, c);
            let w = a.downgrade();
            black_box(a.cow());
            assert!(T::upgrade(&w).is_none());
        }
        if OP == 13 || OP == 14 {
            assert_eq!(
                T::make_cloned_slice(&case.source, c).as_ref(),
                case.source.as_slice()
            );
        }
        if OP == 14 {
            let a = T::make_slice(&case.source, c);
            let mut b = T::clone_slice(&a);
            if let Some(first) = T::cow_slice(&mut b).first_mut() {
                *first ^= 1;
            }
            assert_eq!(a.as_ref(), case.source.as_slice());
            assert_eq!(b.as_ref().len(), case.source.len());
        }
        if OP == 10 {
            assert_eq!(
                T::make_slice(&case.source, c).as_ref(),
                case.source.as_slice()
            );
        }
    }
    let weaks = if OP == 2 || OP == 4 {
        sources.iter().map(|s| s.downgrade()).collect()
    } else {
        Vec::new()
    };
    epochs::<T, OP>(cases, &mut sources, &weaks, c, 2);
    let start = Instant::now();
    epochs::<T, OP>(cases, &mut sources, &weaks, c, n);
    let seconds = start.elapsed().as_secs_f64();
    drop(weaks);
    drop(sources);
    c.empty();
    seconds
}
macro_rules! dispatch {
    ($op:expr,$family:expr,$label:expr,$cases:expr,$context:expr,$n:expr) => {{
        match ($family, $label) {
            ("rc", "std") => run::<std::rc::Rc<Data>, $op>($cases, $context, $n),
            ("arc", "std") => run::<std::sync::Arc<Data>, $op>($cases, $context, $n),
            ("rc", "unlimited") => run::<jzap::rc::Rc<Data>, $op>($cases, $context, $n),
            ("arc", "unlimited") => run::<jzap::sync::Arc<Data>, $op>($cases, $context, $n),
            ("rc", "local") => {
                #[cfg(not(feature = "owned-budgets"))]
                {
                    run::<jzap::rc::Rc<Data, Budget<'_>>, $op>($cases, $context, $n)
                }
                #[cfg(feature = "owned-budgets")]
                {
                    run::<jzap::rc::Rc<Data, OwnedBudget>, $op>($cases, $context, $n)
                }
            }
            ("arc", "local") => {
                #[cfg(not(feature = "owned-budgets"))]
                {
                    run::<jzap::sync::Arc<Data, Budget<'_>>, $op>($cases, $context, $n)
                }
                #[cfg(feature = "owned-budgets")]
                {
                    run::<jzap::sync::Arc<Data, OwnedBudget>, $op>($cases, $context, $n)
                }
            }
            ("rc", "concurrent") => {
                #[cfg(not(feature = "owned-budgets"))]
                {
                    run::<jzap::rc::Rc<Data, ConcurrentBudget<'_>>, $op>($cases, $context, $n)
                }
                #[cfg(feature = "owned-budgets")]
                {
                    run::<jzap::rc::Rc<Data, OwnedConcurrentBudget>, $op>($cases, $context, $n)
                }
            }
            ("arc", "concurrent") => {
                #[cfg(not(feature = "owned-budgets"))]
                {
                    run::<jzap::sync::Arc<Data, ConcurrentBudget<'_>>, $op>($cases, $context, $n)
                }
                #[cfg(feature = "owned-budgets")]
                {
                    run::<jzap::sync::Arc<Data, OwnedConcurrentBudget>, $op>($cases, $context, $n)
                }
            }
            _ => panic!("invalid family/label"),
        }
    }};
}
#[cfg(feature = "shared-memory")]
fn memory_rows<'a, T: SliceOwner<'a>, const OP: usize>(
    cases: &[Case],
    c: &'a Context,
    label: &str,
    family: &str,
) {
    for (index, case) in cases.iter().enumerate() {
        memory::start();
        let held = match OP {
            0 => {
                let a = T::make(case.data, c);
                let held = memory::snapshot();
                drop(a);
                held
            }
            1 => {
                let a = T::make(case.data, c);
                let b = a.copy();
                let held = memory::snapshot();
                drop((a, b));
                held
            }
            2 => {
                let a = T::make(case.data, c);
                let w = a.downgrade();
                let q = T::weak_copy(&w);
                let held = memory::snapshot();
                drop((a, w, q));
                held
            }
            3 => {
                let a = T::make(case.data, c);
                let w = a.downgrade();
                let held = memory::snapshot();
                drop((a, w));
                held
            }
            4 => {
                let a = T::make(case.data, c);
                let w = a.downgrade();
                let b = T::upgrade(&w).unwrap();
                let held = memory::snapshot();
                drop((a, w, b));
                held
            }
            5 => {
                let mut a = T::make(case.data, c);
                black_box(a.unique().unwrap());
                let held = memory::snapshot();
                drop(a);
                held
            }
            6 => {
                let a = T::make(case.data, c);
                black_box(a.unwrap());
                memory::snapshot()
            }
            7 => {
                let a = T::make(case.data, c);
                let mut b = a.copy();
                black_box(b.cow());
                let held = memory::snapshot();
                drop((a, b));
                held
            }
            8 => {
                let mut a = T::make(case.data, c);
                let w = a.downgrade();
                black_box(a.cow());
                let held = memory::snapshot();
                drop((a, w));
                held
            }
            9 => {
                let a = T::cyclic(case.data, c);
                let held = memory::snapshot();
                drop(a);
                held
            }
            10 => {
                let a = T::make_slice(&case.source, c);
                let held = memory::snapshot();
                drop(a);
                held
            }
            13 => {
                let a = T::make_cloned_slice(&case.source, c);
                let held = memory::snapshot();
                drop(a);
                held
            }
            14 => {
                let a = T::make_slice(&case.source, c);
                let mut b = T::clone_slice(&a);
                black_box(T::cow_slice(&mut b));
                let held = memory::snapshot();
                drop((a, b));
                held
            }
            12 => {
                let a = T::cyclic_infallible(case.data, c);
                let held = memory::snapshot();
                drop(a);
                held
            }
            11 => {
                let mut a = T::make(case.data, c);
                black_box(a.cow());
                let held = memory::snapshot();
                drop(a);
                held
            }
            _ => unreachable!(),
        };
        let end = memory::stop();
        assert_eq!((end.live, end.actual), (0, 0));
        c.empty();
        let input_bytes = if [10, 13, 14].contains(&OP) {
            case.source.len() * 8
        } else {
            std::mem::size_of::<Data>()
        };
        let handle_bytes = if [10, 13, 14].contains(&OP) {
            std::mem::size_of::<T::Slice>()
        } else {
            std::mem::size_of::<T>()
        };
        println!(
            "{{\"task\":\"{family}_{}\",\"label\":\"{label}\",\"case\":{index},\"input_bytes\":{input_bytes},\"calls\":{},\"held_bytes\":{},\"peak_bytes\":{},\"held_actual\":{},\"peak_actual\":{},\"handle_bytes\":{handle_bytes}}}",
            OPS[OP], held.calls, held.live, held.peak, held.actual, held.actual_peak
        );
    }
}
#[cfg(feature = "shared-memory")]
fn layout_memory() {
    #[repr(align(256))]
    struct Aligned;
    fn record<T, O>(name: &str, family: &str, label: &str, make: impl FnOnce() -> O) {
        memory::start();
        let owner = make();
        black_box(&owner);
        let held = memory::snapshot();
        drop(owner);
        let end = memory::stop();
        assert_eq!((end.live, end.actual), (0, 0));
        println!(
            "{{\"task\":\"{family}_layout_{name}\",\"label\":\"{label}\",\"case\":0,\"input_bytes\":{},\"calls\":{},\"held_bytes\":{},\"peak_bytes\":{},\"held_actual\":{},\"peak_actual\":{},\"handle_bytes\":{}}}",
            std::mem::size_of::<T>(),
            held.calls,
            held.live,
            held.peak,
            held.actual,
            held.actual_peak,
            std::mem::size_of::<O>()
        );
    }
    let c = Context::new();
    macro_rules! shapes {
        ($($name:literal => $ty:ty = $value:expr),* $(,)?) => {$({
            let make = || -> $ty { $value };
            record::<$ty,_>($name, "rc", "std", || std::rc::Rc::new(make()));
            record::<$ty,_>($name, "arc", "std", || std::sync::Arc::new(make()));
            record::<$ty,_>($name, "rc", "unlimited", || jzap::rc::Rc::try_new(make()).ok().unwrap());
            record::<$ty,_>($name, "arc", "unlimited", || jzap::sync::Arc::try_new(make()).ok().unwrap());
            record::<$ty,_>($name, "rc", "local", || jzap::rc::Rc::try_new_in(make(), Budget::Limited(&c.local)).ok().unwrap());
            record::<$ty,_>($name, "arc", "local", || jzap::sync::Arc::try_new_in(make(), Budget::Limited(&c.local)).ok().unwrap());
            record::<$ty,_>($name, "rc", "concurrent", || jzap::rc::Rc::try_new_in(make(), ConcurrentBudget(&c.concurrent)).ok().unwrap());
            record::<$ty,_>($name, "arc", "concurrent", || jzap::sync::Arc::try_new_in(make(), ConcurrentBudget(&c.concurrent)).ok().unwrap());
        })*};
    }
    shapes!("unit" => () = (), "u8" => u8 = 1, "u64" => u64 = 1, "u128" => u128 = 1, "bytes64" => [u8;64] = [1;64], "align256" => Aligned = Aligned);
    c.empty();
}
#[cfg(feature = "shared-memory")]
fn all_memory() {
    let cases = corpus(617, false);
    let c = Context::new();
    macro_rules! operation {
        ($op:literal) => {{
            memory_rows::<std::rc::Rc<Data>, $op>(&cases, &c, "std", "rc");
            memory_rows::<std::sync::Arc<Data>, $op>(&cases, &c, "std", "arc");
            memory_rows::<jzap::rc::Rc<Data>, $op>(&cases, &c, "unlimited", "rc");
            memory_rows::<jzap::sync::Arc<Data>, $op>(&cases, &c, "unlimited", "arc");
            memory_rows::<jzap::rc::Rc<Data, Budget<'_>>, $op>(&cases, &c, "local", "rc");
            memory_rows::<jzap::sync::Arc<Data, Budget<'_>>, $op>(&cases, &c, "local", "arc");
            memory_rows::<jzap::rc::Rc<Data, ConcurrentBudget<'_>>, $op>(
                &cases,
                &c,
                "concurrent",
                "rc",
            );
            memory_rows::<jzap::sync::Arc<Data, ConcurrentBudget<'_>>, $op>(
                &cases,
                &c,
                "concurrent",
                "arc",
            );
        }};
    }
    operation!(0);
    operation!(1);
    operation!(2);
    operation!(3);
    operation!(4);
    operation!(5);
    operation!(6);
    operation!(7);
    operation!(8);
    operation!(9);
    operation!(10);
    operation!(11);
    operation!(12);
    operation!(13);
    operation!(14);
    layout_memory();
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    #[cfg(feature = "shared-memory")]
    if args.len() == 2 && args[1] == "memory" {
        all_memory();
        return;
    }
    if args.len() == 2 && args[1] == "describe" {
        println!(
            "{{\"operation_unit\":\"one complete API call; construct/unwrap/COW/cyclic/slice include their stated allocation lifecycle, hot handles reuse prepared owners\",\"inputs\":\"64 shuffled cases per seed; 64-byte payload for handle/value operations, slice, slice_clone and slice_cow lengths 0/1/4096 and logarithmic random sizes (short maximum 32); cyclic uses Result<Data, ()> and cyclic_infallible uses Result<Data, Infallible>; handle elements count one credit operation, slice_cow includes source allocation/copy plus shared COW (two copies); other value elements count eight u64 words\"}}"
        );
        return;
    }
    if args.len() == 2 && args[1] == "tasks" {
        println!(
            "{}",
            ["rc", "arc"]
                .iter()
                .flat_map(|f| OPS.map(|op| format!("{f}_{op}")))
                .collect::<Vec<_>>()
                .join(",")
        );
        return;
    }
    assert_eq!(args.len(), 6);
    let label = &args[1];
    let (family, operation) = args[2].split_once('_').unwrap();
    let op = OPS.iter().position(|x| *x == operation).unwrap();
    let seed = args[3].parse().unwrap();
    let n: u64 = args[4].parse().unwrap();
    let small = args[5] == "small";
    assert!(small || args[5] == "mixed");
    let cases = corpus(seed, small);
    let c = Context::new();
    let elapsed = match op {
        0 => dispatch!(0, family, label.as_str(), &cases, &c, n),
        1 => dispatch!(1, family, label.as_str(), &cases, &c, n),
        2 => dispatch!(2, family, label.as_str(), &cases, &c, n),
        3 => dispatch!(3, family, label.as_str(), &cases, &c, n),
        4 => dispatch!(4, family, label.as_str(), &cases, &c, n),
        5 => dispatch!(5, family, label.as_str(), &cases, &c, n),
        6 => dispatch!(6, family, label.as_str(), &cases, &c, n),
        7 => dispatch!(7, family, label.as_str(), &cases, &c, n),
        8 => dispatch!(8, family, label.as_str(), &cases, &c, n),
        9 => dispatch!(9, family, label.as_str(), &cases, &c, n),
        10 => dispatch!(10, family, label.as_str(), &cases, &c, n),
        11 => dispatch!(11, family, label.as_str(), &cases, &c, n),
        12 => dispatch!(12, family, label.as_str(), &cases, &c, n),
        13 => dispatch!(13, family, label.as_str(), &cases, &c, n),
        14 => dispatch!(14, family, label.as_str(), &cases, &c, n),
        _ => unreachable!(),
    };
    let elements = if [10, 13, 14].contains(&op) {
        cases.iter().map(|r| r.source.len() as u64).sum::<u64>() * n * if op == 14 { 2 } else { 1 }
    } else if [1, 2, 3, 4, 5, 11].contains(&op) {
        n * 64
    } else {
        n * 64 * 8
    };
    let (minimum, maximum) = if [10, 13, 14].contains(&op) {
        (
            cases.iter().map(|r| r.source.len()).min().unwrap(),
            cases.iter().map(|r| r.source.len()).max().unwrap(),
        )
    } else if [1, 2, 3, 4, 5, 11].contains(&op) {
        (1, 1)
    } else {
        (8, 8)
    };
    let mut digest = 0xcbf29ce484222325u64;
    for case in &cases {
        for b in case.data.0.iter().chain(&case.source) {
            digest = (digest ^ b).wrapping_mul(0x100000001b3);
        }
    }
    println!(
        "{{\"epochs\":{n},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"elapsed_seconds\":{elapsed}}}",
        n * 64,
        elements,
        minimum,
        maximum
    );
}
