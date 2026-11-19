//! Public unit-owner length update; std reference includes the same logical bound.
use std::{hint::black_box, time::Instant};
extern crate jzap_new;
extern crate jzap_old;
#[derive(Clone, Copy)]
struct Input {
    limit: usize,
    requested: usize,
}
type Method = fn(Input) -> (bool, usize);
macro_rules! product {
    ($name:ident,$lib:ident) => {
        #[inline(never)]
        fn $name(x: Input) -> (bool, usize) {
            let mut owner = $lib::vec::Vec::<()>::with_limit(x.limit);
            // SAFETY: every admitted unit value is valid without storage/resources.
            let reply = unsafe { owner.try_set_len(x.requested) };
            (reply.is_ok(), owner.len())
        }
    };
}
product!(old, jzap_old);
product!(new, jzap_new);
#[inline(never)]
fn standard(x: Input) -> (bool, usize) {
    let mut owner = Vec::<()>::new();
    if x.requested > x.limit {
        return (false, owner.len());
    }
    // SAFETY: units own no bytes/resources, and std's capacity is usize::MAX.
    unsafe {
        owner.set_len(x.requested);
    }
    (true, owner.len())
}
fn rng(x: &mut u64) -> usize {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x as usize
}
fn measure(f: Method, data: &[Input], batches: usize) -> (f64, u64, usize) {
    let start = Instant::now();
    let mut sum = 0usize;
    let mut calls = 0u64;
    for _ in 0..batches {
        for &x in data {
            let (ok, len) = black_box(f)(black_box(x));
            sum = sum.wrapping_add(len ^ ok as usize);
            calls += 1;
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    assert!(
        seconds >= 1.0,
        "Increase JZAP_RANGE_BATCHES for the complete comparison"
    );
    (seconds, calls, black_box(sum))
}
fn main() {
    let batches = std::env::var("JZAP_RANGE_BATCHES")
        .map(|x| x.parse::<usize>().unwrap())
        .unwrap_or(250000);
    let selected = std::env::var("JZAP_RANGE_SEED")
        .map(|x| x.parse::<u64>().unwrap())
        .unwrap_or(0);
    assert!(selected <= 3);
    println!("seed,case,implementation,seconds,operations,checksum");
    let methods: [Method; 3] = [standard, old, new];
    for seed in 1u64..=3 {
        if selected != 0 && selected != seed {
            continue;
        }
        let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
        for case in 0..3 {
            let data: Vec<_> = (0..4096)
                .map(|_| {
                    let limit = rng(&mut state) % 4096;
                    let success = case == 0 || (case == 2 && rng(&mut state) % 2 == 0);
                    let requested = if success {
                        rng(&mut state) % (limit + 1)
                    } else {
                        limit + 1 + rng(&mut state) % 4096
                    };
                    Input { limit, requested }
                })
                .collect();
            for &x in &data {
                assert_eq!(methods[0](x), methods[1](x));
                assert_eq!(methods[0](x), methods[2](x));
            }
            for j in 0..3 {
                let index = (j + seed as usize + case) % 3;
                let (seconds, operations, checksum) = measure(methods[index], &data, batches);
                println!(
                    "{seed},{case},{},{seconds:.9},{operations},{checksum}",
                    ["std", "old", "new"][index]
                );
            }
        }
    }
}
