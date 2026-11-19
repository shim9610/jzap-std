//! Public ZST conversion diagnostic: no allocator or descriptor generation in timing.
use std::{hint::black_box, time::Instant};
extern crate jzap_new;
extern crate jzap_old;
#[derive(Clone, Copy)]
struct Input {
    length: usize,
    limit: usize,
}
type Reply = (bool, usize);
type Method = fn(Input) -> Reply;
macro_rules! product {
    ($name:ident, $library:ident, $width:expr, $limited:expr) => {
        #[inline(never)]
        fn $name(x: Input) -> Reply {
            if $limited {
                let mut value = $library::vec::Vec::<[(); $width]>::with_limit(x.limit);
                // SAFETY: unit arrays require no bytes/resources; prepared
                // lengths fit this zero-sized capacity.
                unsafe {
                    value.try_set_len(x.length).unwrap();
                }
                match value.try_into_flattened() {
                    Ok(value) => (true, value.len()),
                    Err(_) => (false, 0),
                }
            } else {
                let mut value = $library::vec::Vec::<[(); $width]>::new();
                // SAFETY: unit arrays require no bytes/resources.
                unsafe {
                    value.try_set_len(x.length).unwrap();
                }
                match value.try_into_flattened() {
                    Ok(value) => (true, value.len()),
                    Err(_) => (false, 0),
                }
            }
        }
    };
}
macro_rules! standard {
    ($name:ident, $width:expr) => {
        #[inline(never)]
        fn $name(x: Input) -> Reply {
            let mut value = Vec::<[(); $width]>::new();
            // SAFETY: unit arrays require no storage and have no destructor.
            unsafe {
                value.set_len(x.length);
            }
            (true, value.into_flattened().len())
        }
    };
}
product!(old3, jzap_old, 3, false);
product!(new3, jzap_new, 3, false);
product!(old3_limited, jzap_old, 3, true);
product!(new3_limited, jzap_new, 3, true);
product!(old_max, jzap_old, usize::MAX, false);
product!(new_max, jzap_new, usize::MAX, false);
product!(old_max_limited, jzap_old, usize::MAX, true);
product!(new_max_limited, jzap_new, usize::MAX, true);
standard!(std3, 3);
standard!(std_max, usize::MAX);
fn rng(state: &mut u64) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state as usize
}
fn measure(method: Method, data: &[Input], batches: usize) -> (f64, u64, usize) {
    let begin = Instant::now();
    let mut calls = 0u64;
    let mut sum = 0usize;
    for _ in 0..batches {
        for item in data {
            let (ok, length) = black_box(method)(black_box(*item));
            sum = sum.wrapping_add(length ^ ok as usize);
            calls += 1;
        }
    }
    let elapsed = begin.elapsed().as_secs_f64();
    assert!(elapsed >= 1.0, "Increase JZAP_RANGE_BATCHES");
    (elapsed, calls, black_box(sum))
}
fn main() {
    let batches = std::env::var("JZAP_RANGE_BATCHES")
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(156250);
    assert!(batches > 0);
    println!("seed,case,implementation,seconds,operations,checksum");
    let cases: [[Method; 3]; 4] = [
        [std3, old3, new3],
        [std3, old3_limited, new3_limited],
        [std_max, old_max, new_max],
        [std_max, old_max_limited, new_max_limited],
    ];
    for seed in 1u64..=3 {
        let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
        for (case, methods) in cases.iter().enumerate() {
            let data: Vec<_> = (0..4096)
                .map(|_| {
                    let length = if case < 2 {
                        rng(&mut state) % 4096
                    } else {
                        rng(&mut state) % 2
                    };
                    Input {
                        length,
                        limit: if case < 2 {
                            3 * length + rng(&mut state) % 4096
                        } else {
                            usize::MAX
                        },
                    }
                })
                .collect();
            for x in &data {
                assert_eq!(methods[0](*x), methods[1](*x));
                assert_eq!(methods[0](*x), methods[2](*x));
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
