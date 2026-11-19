//! Prepared fixed-buffer and checked-slice workloads; setup is outside timing.
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
    scratch::{Scratch, ScratchBuf},
    slice::{Slice, SliceMut},
};
use std::{hint::black_box, mem::MaybeUninit, time::Instant};
const OPS: [&str; 10] = [
    "push_pop",
    "extend",
    "resize",
    "init",
    "clone_from",
    "borrowed_push_pop",
    "borrowed_extend",
    "get",
    "range",
    "copy_within",
];
trait Item: Copy + TryClone + PartialEq {
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
        [x; 32]
    }
    fn key(self) -> u64 {
        self[0]
    }
}
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
trait Fixed<T: Item> {
    fn slice(&self) -> &[T];
    fn slice_mut(&mut self) -> &mut [T];
    fn clear(&mut self);
    fn push(&mut self, v: T);
    fn pop(&mut self) -> Option<T>;
    fn extend(&mut self, v: &[T]);
    fn resize(&mut self, n: usize, v: T);
    fn initialize(&mut self, n: usize, v: T);
    fn copy_from(&mut self, other: &Self);
}
impl<T: Item> Fixed<T> for Vec<T> {
    fn slice(&self) -> &[T] {
        self
    }
    fn slice_mut(&mut self) -> &mut [T] {
        self
    }
    fn clear(&mut self) {
        self.clear()
    }
    fn push(&mut self, v: T) {
        self.push(v)
    }
    fn pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn extend(&mut self, v: &[T]) {
        self.extend_from_slice(v)
    }
    fn resize(&mut self, n: usize, v: T) {
        self.resize(n, v)
    }
    fn initialize(&mut self, n: usize, v: T) {
        self.resize_with(n, || v)
    }
    fn copy_from(&mut self, other: &Self) {
        self.clone_from(other)
    }
}
impl<T: Item, P: AllocationPolicy> Fixed<T> for ScratchBuf<T, P> {
    fn slice(&self) -> &[T] {
        self.as_slice()
    }
    fn slice_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
    fn clear(&mut self) {
        self.clear()
    }
    fn push(&mut self, v: T) {
        self.try_push(v).ok().unwrap();
    }
    fn pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn extend(&mut self, v: &[T]) {
        self.try_extend_copy(v).unwrap()
    }
    fn resize(&mut self, n: usize, v: T) {
        self.try_resize(n, &v).unwrap()
    }
    fn initialize(&mut self, n: usize, v: T) {
        self.try_resize_with_init(n, |_, s| s.write(v).map_err(|r| r.error))
            .unwrap()
    }
    fn copy_from(&mut self, other: &Self) {
        self.try_clone_from(other).unwrap()
    }
}
impl<T: Item> Fixed<T> for Scratch<'_, T> {
    fn slice(&self) -> &[T] {
        self.as_slice()
    }
    fn slice_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
    fn clear(&mut self) {
        self.clear()
    }
    fn push(&mut self, v: T) {
        self.try_push(v).ok().unwrap();
    }
    fn pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn extend(&mut self, v: &[T]) {
        self.try_extend_copy(v).unwrap()
    }
    fn resize(&mut self, n: usize, v: T) {
        self.try_resize(n, &v).unwrap()
    }
    fn initialize(&mut self, n: usize, v: T) {
        self.try_resize_with_init(n, |_, s| s.write(v).map_err(|r| r.error))
            .unwrap()
    }
    fn copy_from(&mut self, _: &Self) {
        unreachable!("borrowed scratch has no owner clone-from")
    }
}
#[inline(always)]
fn action<T: Item, O: Fixed<T>, const OP: usize, const CHECKED: bool>(
    owner: &mut O,
    source: &O,
    input: &[T],
    pivot: usize,
) -> u64 {
    let n = input.len();
    let value = input.first().copied().unwrap_or_else(|| T::make(0));
    match OP {
        0 | 5 => {
            owner.clear();
            for v in black_box(input) {
                owner.push(*v)
            }
            black_box(owner.slice());
            let mut sum = 0u64;
            while let Some(v) = owner.pop() {
                sum = sum.wrapping_add(v.key())
            }
            return sum;
        }
        1 | 6 => {
            owner.clear();
            owner.extend(black_box(input));
        }
        2 => {
            owner.clear();
            owner.resize(n, black_box(value));
        }
        3 => {
            owner.clear();
            owner.initialize(n, black_box(value));
        }
        4 => {
            owner.copy_from(black_box(source));
        }
        7 => {
            let view = Slice::new(black_box(owner.slice()));
            let mut sum = 0u64;
            for i in 0..n {
                let at = black_box(i);
                let v = if CHECKED {
                    view.get(at).unwrap()
                } else {
                    view.as_slice().get(at).unwrap()
                };
                sum = sum.wrapping_add(v.key());
            }
            return sum;
        }
        8 => {
            let start = black_box(pivot % (n + 1));
            let end = black_box(n);
            let s = if CHECKED {
                Slice::new(black_box(owner.slice()))
                    .range(start..end)
                    .unwrap()
                    .as_slice()
            } else {
                black_box(owner.slice()).get(start..end).unwrap()
            };
            black_box(s);
            return s.len() as u64;
        }
        9 => {
            let count = black_box(n / 2);
            let at = black_box(n - count);
            if CHECKED {
                SliceMut::new(black_box(owner.slice_mut()))
                    .copy_within(..count, at)
                    .unwrap()
            } else {
                black_box(owner.slice_mut()).copy_within(..count, at)
            };
        }
        _ => unreachable!(),
    }
    let s = black_box(owner.slice());
    (s.len() as u64)
        .wrapping_add(s.first().copied().map(Item::key).unwrap_or(0))
        .wrapping_add(s.last().copied().map(Item::key).unwrap_or(0))
}
fn measure<T: Item, O: Fixed<T>, const OP: usize, const CHECKED: bool>(
    owners: &mut [O],
    sources: &[O],
    inputs: &[Vec<T>],
    pivots: &[usize],
    epochs: u64,
) -> (f64, u64) {
    let mut sum = 0u64;
    for _ in 0..2 {
        for (i, owner) in owners.iter_mut().enumerate() {
            sum = sum.wrapping_add(action::<T, O, OP, CHECKED>(
                owner,
                &sources[i],
                &inputs[i],
                pivots[i],
            ));
        }
    }
    black_box(sum);
    sum = 0;
    let start = Instant::now();
    for _ in 0..epochs {
        for (i, owner) in owners.iter_mut().enumerate() {
            sum = sum.wrapping_add(action::<T, O, OP, CHECKED>(
                black_box(owner),
                &sources[i],
                &inputs[i],
                pivots[i],
            ));
        }
    }
    (start.elapsed().as_secs_f64(), black_box(sum))
}
fn owning<T: Item, P: AllocationPolicy, const OP: usize>(
    policy: impl Fn() -> P,
    inputs: &[Vec<T>],
    pivots: &[usize],
    epochs: u64,
) -> (f64, u64) {
    let mut owners = inputs
        .iter()
        .map(|v| {
            let mut s = ScratchBuf::try_with_capacity_in(v.len(), policy()).unwrap();
            s.try_extend_copy(v).unwrap();
            s
        })
        .collect::<Vec<_>>();
    let sources = inputs
        .iter()
        .map(|v| {
            let mut s = ScratchBuf::try_with_capacity_in(v.len(), policy()).unwrap();
            s.try_extend_copy(v).unwrap();
            s
        })
        .collect::<Vec<_>>();
    measure::<T, _, OP, true>(&mut owners, &sources, inputs, pivots, epochs)
}
fn dispatch<T: Item, const OP: usize>(
    label: &str,
    ctx: &Context,
    inputs: &[Vec<T>],
    pivots: &[usize],
    epochs: u64,
) -> (f64, u64) {
    if label == "std" || label == "std_control" {
        let mut owners = inputs.to_vec();
        let sources = inputs.to_vec();
        return measure::<T, _, OP, false>(&mut owners, &sources, inputs, pivots, epochs);
    }
    if OP == 5 || OP == 6 {
        let mut backing = inputs
            .iter()
            .map(|v| vec![MaybeUninit::uninit(); v.len()])
            .collect::<Vec<_>>();
        let mut other = inputs
            .iter()
            .map(|v| vec![MaybeUninit::uninit(); v.len()])
            .collect::<Vec<_>>();
        let mut owners = backing
            .iter_mut()
            .zip(inputs)
            .map(|(b, v)| {
                let mut s = Scratch::new(b);
                s.try_extend_copy(v).unwrap();
                s
            })
            .collect::<Vec<_>>();
        let sources = other
            .iter_mut()
            .zip(inputs)
            .map(|(b, v)| {
                let mut s = Scratch::new(b);
                s.try_extend_copy(v).unwrap();
                s
            })
            .collect::<Vec<_>>();
        return measure::<T, _, OP, true>(&mut owners, &sources, inputs, pivots, epochs);
    }
    match label {
        "unlimited" => owning::<T, _, OP>(|| Unmetered, inputs, pivots, epochs),
        "local" => owning::<T, _, OP>(|| Budget::Limited(&ctx.local), inputs, pivots, epochs),
        "concurrent" => {
            owning::<T, _, OP>(|| ConcurrentBudget(&ctx.concurrent), inputs, pivots, epochs)
        }
        _ => unreachable!(),
    }
}
fn run<T: Item>(label: &str, op: usize, seed: u64, epochs: u64, small: bool) {
    let cases = ordered_corpus::corpus(seed, small);
    let inputs = cases
        .iter()
        .map(|c| c.keys.iter().copied().map(T::make).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let pivots = cases.iter().map(|c| c.pivot as usize).collect::<Vec<_>>();
    let ctx = Context::new();
    macro_rules! go {($($n:literal),*)=>{match op{$($n=>dispatch::<T,$n>(label,&ctx,&inputs,&pivots,epochs),)*_=>unreachable!()}};}
    let (elapsed, checksum) = go!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9);
    assert_eq!(
        (
            ctx.local.occupied(),
            ctx.local.reserved(),
            ctx.concurrent.used()
        ),
        (0, 0, 0)
    );
    let mut digest = 0xcbf29ce484222325u64;
    for c in &cases {
        for v in &c.keys {
            digest = (digest ^ v).wrapping_mul(0x100000001b3);
        }
        digest = (digest ^ c.pivot).wrapping_mul(0x100000001b3);
    }
    println!(
        "{{\"epochs\":{epochs},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"checksum\":{checksum},\"elapsed_seconds\":{elapsed}}}",
        epochs * 64,
        epochs * cases.iter().map(|c| c.keys.len() as u64).sum::<u64>(),
        cases.iter().map(|c| c.keys.len()).min().unwrap(),
        cases.iter().map(|c| c.keys.len()).max().unwrap()
    );
}
#[cfg(feature = "btree-memory")]
fn memory_run<T: Item>(wanted: Option<&str>, only_length: Option<usize>) {
    for label in ["std", "unlimited", "local", "concurrent"] {
        if wanted.is_some_and(|s| s != label) {
            continue;
        }
        for n in [0, 1, 4, 32, 4096] {
            if only_length.is_some_and(|length| length != n) {
                continue;
            }
            let ctx = Context::new();
            let input = vec![T::make(9); n];
            // Separate concrete paths avoid instrumentation in the timing binary.
            match label {
                "std" => memory_owner::<T, _>(label, n, &input, || Vec::with_capacity(n)),
                "unlimited" => memory_owner::<T, _>(label, n, &input, || {
                    ScratchBuf::try_with_capacity(n).unwrap()
                }),
                "local" => memory_owner::<T, _>(label, n, &input, || {
                    ScratchBuf::try_with_capacity_in(n, Budget::Limited(&ctx.local)).unwrap()
                }),
                "concurrent" => memory_owner::<T, _>(label, n, &input, || {
                    ScratchBuf::try_with_capacity_in(n, ConcurrentBudget(&ctx.concurrent)).unwrap()
                }),
                _ => unreachable!(),
            }
        }
    }
    let mut bytes = [0u8; 4096];
    memory::start();
    let mut s = Scratch::from_bytes(&mut bytes);
    s.try_extend_copy(&[7; 4096]).unwrap();
    s.clear();
    let held = memory::snapshot();
    drop(s);
    let final_ = memory::stop();
    assert_eq!((held.calls, held.peak, final_.live), (0, 0, 0));
    println!(
        "{{\"borrowed_calls\":0,\"borrowed_peak\":0,\"borrowed_handle\":{},\"view_handle\":{},\"view_mut_handle\":{}}}",
        std::mem::size_of::<Scratch<'_, u8>>(),
        std::mem::size_of::<Slice<'_, u8>>(),
        std::mem::size_of::<SliceMut<'_, u8>>()
    );
}
#[cfg(feature = "btree-memory")]
fn memory_owner<T: Item, O: Fixed<T>>(
    label: &str,
    n: usize,
    input: &[T],
    create: impl FnOnce() -> O,
) {
    memory::start();
    let mut owner = create();
    owner.extend(input);
    let initial = memory::snapshot();
    for _ in 0..4 {
        owner.clear();
        owner.extend(input);
        owner.clear();
        owner.resize(n, T::make(2));
        owner.clear();
        owner.initialize(n, T::make(3));
    }
    let held = memory::snapshot();
    let handle = std::mem::size_of_val(&owner);
    drop(owner);
    let final_ = memory::stop();
    assert_eq!((final_.live, final_.actual), (0, 0));
    println!(
        "{{\"label\":\"{label}\",\"width\":{},\"length\":{n},\"calls\":{},\"operation_calls\":{},\"held\":{},\"peak\":{},\"usable\":{},\"usable_peak\":{},\"handle\":{handle}}}",
        std::mem::size_of::<T>(),
        held.calls,
        held.calls - initial.calls,
        held.live,
        held.peak,
        held.actual,
        held.actual_peak
    );
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args[1] == "tasks" {
        println!(
            "{}",
            [8, 256]
                .into_iter()
                .flat_map(|w| OPS.map(move |s| format!("scratch_{s}_{w}")))
                .collect::<Vec<_>>()
                .join(",")
        );
        return;
    }
    if args[1] == "describe" {
        println!(
            "{{\"operation_unit\":\"One prepared fixed-buffer refill, push-all/pop-all, resize or slot initialization, clone-from, checked whole get scan, range view, or half-buffer overlapping copy. Sources, destination allocations and random input generation are outside timing. Warmup runs twice. Writes are black-boxed; constant-time checksum uses length/first/last except pop and get scan.\",\"inputs\":\"64 deterministic random corpora, zero/one/4096 and log-uniform sizes; u64 and 256-byte values; three caller-selected seeds. std and all labels receive identical corpora and counts. Borrowed scratch uses prepared std Vec as an equal-byte initialized storage baseline.\"}}"
        );
        return;
    }
    #[cfg(feature = "btree-memory")]
    if args[1] == "memory" {
        let label = args.get(2).map(String::as_str);
        let width = args.get(3).map(|s| s.parse::<usize>().unwrap());
        let length = args.get(4).map(|s| s.parse::<usize>().unwrap());
        if width.is_none_or(|w| w == 8) {
            memory_run::<u64>(label, length);
        }
        if width.is_none_or(|w| w == 256) {
            memory_run::<[u64; 32]>(label, length);
        }
        return;
    }
    assert_eq!(args.len(), 6);
    let (name, width) = args[2]
        .strip_prefix("scratch_")
        .unwrap()
        .rsplit_once('_')
        .unwrap();
    let op = OPS.iter().position(|s| *s == name).unwrap();
    let seed = args[3].parse().unwrap();
    let epochs = args[4].parse().unwrap();
    let small = args[5] == "small";
    match width {
        "8" => run::<u64>(&args[1], op, seed, epochs, small),
        "256" => run::<[u64; 32]>(&args[1], op, seed, epochs, small),
        _ => unreachable!(),
    }
}
