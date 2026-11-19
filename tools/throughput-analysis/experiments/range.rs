#![feature(slice_range)]
use std::{hint::black_box, ops::Bound, time::Instant};
extern crate jzap_new;
extern crate jzap_old;
#[derive(Clone, Copy)]
struct Input {
    a: Bound<usize>,
    b: Bound<usize>,
    n: usize,
}
#[inline(never)]
fn old(x: Input) -> (usize, usize) {
    match jzap_old::proof_codegen_range((x.a, x.b), x.n) {
        Ok(v) => v,
        Err(_) => (usize::MAX, usize::MAX),
    }
}
#[inline(never)]
fn new(x: Input) -> (usize, usize) {
    match jzap_new::proof_codegen_range((x.a, x.b), x.n) {
        Ok(v) => v,
        Err(_) => (usize::MAX, usize::MAX),
    }
}
#[inline(never)]
fn std_range(x: Input) -> (usize, usize) {
    let r = std::slice::range((x.a, x.b), ..x.n);
    (r.start, r.end)
}
fn rng(s: &mut u64) -> usize {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s as usize
}
fn measure(f: fn(Input) -> (usize, usize), data: &[Input], batches: usize) -> (f64, u64, usize) {
    let start = Instant::now();
    let mut ops = 0u64;
    let mut sum = 0usize;
    for _ in 0..batches {
        for x in data {
            let (a, b) = black_box(f)(black_box(*x));
            sum = sum.wrapping_add(a ^ b);
            ops += 1;
        }
    }
    {
        let elapsed = start.elapsed().as_secs_f64();
        assert!(elapsed >= 1.0);
        (elapsed, ops, black_box(sum))
    }
}
fn main() {
    let batches = std::env::var("JZAP_RANGE_BATCHES")
        .map(|s| s.parse::<usize>().expect("integer batches"))
        .unwrap_or(156250);
    assert!(batches > 0);
    println!("seed,case,implementation,seconds,operations,checksum");
    let methods: [(&str, fn(Input) -> (usize, usize)); 3] =
        [("std", std_range), ("old", old), ("new", new)];
    for seed in 1u64..=3 {
        let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
        for case in 0..10usize {
            let mut data = Vec::with_capacity(4096);
            for _ in 0..4096 {
                let n = 1024 + rng(&mut state) % 4096;
                let a = 1 + rng(&mut state) % (n / 2 - 1);
                let b = n / 2 + 1 + rng(&mut state) % (n - n / 2 - 1);
                let mode = if case == 9 { rng(&mut state) % 9 } else { case };
                data.push(Input {
                    a: match mode / 3 {
                        0 => Bound::Included(a),
                        1 => Bound::Excluded(a - 1),
                        _ => Bound::Unbounded,
                    },
                    b: match mode % 3 {
                        0 => Bound::Included(b - 1),
                        1 => Bound::Excluded(b),
                        _ => Bound::Unbounded,
                    },
                    n,
                });
            }
            for x in &data {
                assert_eq!(old(*x), new(*x));
                assert_eq!(std_range(*x), new(*x));
            }
            for j in 0..3 {
                let idx = (j + seed as usize + case) % 3;
                let (name, f) = methods[idx];
                let (s, o, c) = measure(f, &data, batches);
                println!("{seed},{case},{name},{s:.9},{o},{c}");
            }
        }
    }
}
