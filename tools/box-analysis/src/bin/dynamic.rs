use jzap::{
    TryClone,
    boxed::Box as JBox,
    budget::{AllocationPolicy, Budget, MemoryBudget, Unmetered},
};
use std::{hint::black_box, mem::MaybeUninit, time::Instant};
#[cfg(feature = "memory")]
#[path = "../memory.rs"]
mod memory;
#[cfg(not(feature = "memory"))]
mod memory {
    #[derive(Clone, Copy, Default)]
    pub struct Stats {
        pub live: usize,
        pub peak: usize,
        pub total: usize,
        pub calls: usize,
    }
    pub fn snapshot() -> Stats {
        Stats::default()
    }
    pub fn start(_: bool) {}
    pub fn stop() {}
}
fn measure<B>(
    n: usize,
    replace: bool,
    mut new: impl FnMut() -> B,
    mut clone: impl FnMut(&B) -> B,
    mut clone_from: impl FnMut(&mut B, &B),
) -> ([u128; 4], [usize; 7]) {
    let t = Instant::now();
    let mut source = Vec::with_capacity(n);
    for _ in 0..n {
        source.push(new());
    }
    black_box(&source);
    let created = t.elapsed().as_nanos();
    let built = memory::snapshot();
    let t = Instant::now();
    let mut target = Vec::with_capacity(n);
    for value in &source {
        target.push(clone(black_box(value)));
    }
    black_box(&target);
    let cloned = t.elapsed().as_nanos();
    let live = memory::snapshot();
    let t = Instant::now();
    if replace {
        for (destination, source) in target.iter_mut().zip(&source) {
            clone_from(black_box(destination), black_box(source));
        }
    }
    black_box(&target);
    let replaced = t.elapsed().as_nanos();
    let t = Instant::now();
    drop(target);
    drop(source);
    let dropped = t.elapsed().as_nanos();
    let end = memory::snapshot();
    (
        [created, cloned, replaced, dropped],
        [
            built.live,
            live.live,
            end.peak,
            end.live,
            end.total,
            end.calls,
            size_of::<B>(),
        ],
    )
}
fn ours<P: AllocationPolicy>(
    kind: &str,
    input: &[u8],
    text: &str,
    n: usize,
    policy: P,
    replace: bool,
) -> ([u128; 4], [usize; 7]) {
    match kind {
        "slice" => measure(
            n,
            replace,
            || JBox::<[u8], _>::try_copy_from_slice_in(black_box(input), policy.try_clone().unwrap()).unwrap(),
            |v| v.try_clone().unwrap(),
            |destination, source| destination.try_clone_from(source).unwrap(),
        ),
        "str" => measure(
            n,
            replace,
            || JBox::<str, _>::try_from_str_in(black_box(text), policy.try_clone().unwrap()).unwrap(),
            |v| v.try_clone().unwrap(),
            |destination, source| destination.try_clone_from(source).unwrap(),
        ),
        "zeroed" => measure(
            n,
            replace,
            || unsafe {
                JBox::<[MaybeUninit<u8>], _>::assume_init(
                    JBox::<[u8], _>::try_new_zeroed_slice_in(input.len(), policy.try_clone().unwrap()).unwrap(),
                )
            },
            |v| v.try_clone().unwrap(),
            |destination, source| destination.try_clone_from(source).unwrap(),
        ),
        "collect" => measure(
            n,
            replace,
            || JBox::<[u8], _>::try_from_iter_in(black_box(input).iter().copied(), policy.try_clone().unwrap()).unwrap(),
            |v| v.try_clone().unwrap(),
            |destination, source| destination.try_clone_from(source).unwrap(),
        ),
        _ => panic!("unknown kind"),
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let label = &args[1];
    let kind = &args[2];
    let len: usize = args[3].parse().unwrap();
    let mode = args.get(4).map(String::as_str).unwrap_or("time");
    let timing = mode == "time" || mode == "replace";
    let replace = mode != "time";
    assert_eq!(cfg!(feature = "memory"), !timing);
    let input = vec![b'x'; len];
    let text = std::str::from_utf8(&input).unwrap();
    let n = 4.max(1_048_576 / len);
    let budget = MemoryBudget::new(usize::MAX);
    let cycles = if timing { 32 } else { 1 };
    let mut samples = Vec::with_capacity(cycles);
    for cycle in 0..cycles {
        memory::start(mode == "usable");
        let (times, stats) = match label.as_str() {
            "std" | "std_reuse_str" => match kind.as_str() {
                "slice" => measure(
                    n,
                    replace,
                    || Box::<[u8]>::from(black_box(input.as_slice())),
                    |v| v.clone(),
                    |destination, source| destination.clone_from(source),
                ),
                "str" => measure(n, replace, || Box::<str>::from(black_box(text)), |v| v.clone(), |destination, source| {
                    if label == "std_reuse_str" {
                        // Diagnostic control: same UTF-8 payload and length, reuse existing storage.
                        unsafe { destination.as_bytes_mut().copy_from_slice(source.as_bytes()) };
                    } else { destination.clone_from(source) }
                }),
                "zeroed" => measure(
                    n,
                    replace,
                    || unsafe { Box::<[u8]>::new_zeroed_slice(len).assume_init() },
                    |v| v.clone(),
                    |destination, source| destination.clone_from(source),
                ),
                "collect" => measure(
                    n,
                    replace,
                    || {
                        black_box(input.as_slice())
                            .iter()
                            .copied()
                            .collect::<Box<[u8]>>()
                    },
                    |v| v.clone(),
                    |destination, source| destination.clone_from(source),
                ),
                _ => panic!("unknown kind"),
            },
            "unmetered" => ours(kind, &input, text, n, Unmetered, replace),
            "metered" => ours(kind, &input, text, n, Budget::Limited(&budget), replace),
            _ => panic!("unknown implementation"),
        };
        memory::stop();
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
        if timing {
            if cycle >= 2 {
                samples.push(times);
            }
        } else {
            println!(
                "{{\"label\":\"{label}\",\"kind\":\"{kind}\",\"len\":{len},\"n\":{n},\"mode\":\"{mode}\",\"stats\":{stats:?}}}"
            );
        }
    }
    if timing {
        println!(
            "{{\"label\":\"{label}\",\"kind\":\"{kind}\",\"len\":{len},\"n\":{n},\"samples_ns\":{samples:?}}}"
        );
    }
}
