//! Public initialization and array-clone comparisons against a pinned revision and std.
use std::{hint::black_box, time::Instant};
extern crate jzap_new;
extern crate jzap_old;
type Method = fn(u64) -> u64;
struct Value(u64);
impl Clone for Value {
    fn clone(&self) -> Self {
        Self(black_box(self.0))
    }
}
impl jzap_old::TryClone for Value {
    fn try_clone(&self) -> Result<Self, jzap_old::Error> {
        Ok(Self(black_box(self.0)))
    }
}
impl jzap_new::TryClone for Value {
    fn try_clone(&self) -> Result<Self, jzap_new::Error> {
        Ok(Self(black_box(self.0)))
    }
}
macro_rules! product {
    ($clone:ident,$array:ident,$slice:ident,$lib:ident,$n:expr) => {
        #[inline(never)]
        fn $clone(seed: u64) -> u64 {
            let source: [Value; $n] = std::array::from_fn(|i| Value(seed.wrapping_add(i as u64)));
            let result = $lib::TryClone::try_clone(black_box(&source)).unwrap();
            black_box(&result);
            result[black_box($n - 1)].0
        }
        #[inline(never)]
        fn $array(seed: u64) -> u64 {
            let result = $lib::boxed::Box::<[u64; $n]>::try_init_array_in(
                $lib::budget::Unmetered,
                |i, slot| {
                    slot.write(black_box(seed.wrapping_add(i as u64)))
                        .map_err(|e| e.error)
                },
            )
            .unwrap();
            black_box(&result);
            result[black_box($n - 1)]
        }
        #[inline(never)]
        fn $slice(seed: u64) -> u64 {
            let result = $lib::boxed::Box::<[u64]>::try_init_slice(black_box($n), |i, slot| {
                slot.write(black_box(seed.wrapping_add(i as u64)))
                    .map_err(|e| e.error)
            })
            .unwrap();
            black_box(&result);
            result.as_slice()[black_box($n - 1)]
        }
    };
}
macro_rules! standard {
    ($clone:ident,$array:ident,$slice:ident,$n:expr) => {
        #[inline(never)]
        fn $clone(seed: u64) -> u64 {
            let source: [Value; $n] = std::array::from_fn(|i| Value(seed.wrapping_add(i as u64)));
            let result = black_box(&source).clone();
            black_box(&result);
            result[black_box($n - 1)].0
        }
        #[inline(never)]
        fn $array(seed: u64) -> u64 {
            let mut result = Box::<[u64; $n]>::new_uninit();
            let ptr = result.as_mut_ptr().cast::<u64>();
            for i in 0..$n {
                // SAFETY: exact admitted array storage, one write to each element.
                unsafe {
                    ptr.add(i).write(black_box(seed.wrapping_add(i as u64)));
                }
            }
            // SAFETY: all elements initialized above.
            let result = unsafe { result.assume_init() };
            black_box(&result);
            result[black_box($n - 1)]
        }
        #[inline(never)]
        fn $slice(seed: u64) -> u64 {
            let n = black_box($n);
            let mut result = Box::<[u64]>::new_uninit_slice(n);
            for (i, slot) in result.iter_mut().enumerate() {
                slot.write(black_box(seed.wrapping_add(i as u64)));
            }
            // SAFETY: all n elements initialized above.
            let result = unsafe { result.assume_init() };
            black_box(&result);
            result[black_box($n - 1)]
        }
    };
}
product!(old_c16, old_a16, old_s16, jzap_old, 16);
product!(new_c16, new_a16, new_s16, jzap_new, 16);
product!(old_c1024, old_a1024, old_s1024, jzap_old, 1024);
product!(new_c1024, new_a1024, new_s1024, jzap_new, 1024);
standard!(std_c16, std_a16, std_s16, 16);
standard!(std_c1024, std_a1024, std_s1024, 1024);
fn measure(method: Method, data: &[u64], batches: usize) -> (f64, u64, u64) {
    let start = Instant::now();
    let mut sum = 0u64;
    let mut calls = 0u64;
    for _ in 0..batches {
        for &x in data {
            sum = sum.wrapping_add(black_box(method)(black_box(x)));
            calls += 1;
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    assert!(
        elapsed >= 1.0,
        "Increase JZAP_RANGE_BATCHES for the entire comparison"
    );
    (elapsed, calls, black_box(sum))
}
fn main() {
    let batches = std::env::var("JZAP_RANGE_BATCHES")
        .map(|x| x.parse::<usize>().unwrap())
        .unwrap_or(50000);
    let cases: [[Method; 3]; 6] = [
        [std_c16, old_c16, new_c16],
        [std_a16, old_a16, new_a16],
        [std_s16, old_s16, new_s16],
        [std_c1024, old_c1024, new_c1024],
        [std_a1024, old_a1024, new_a1024],
        [std_s1024, old_s1024, new_s1024],
    ];
    println!("seed,case,implementation,seconds,operations,checksum");
    let selected_seed = std::env::var("JZAP_RANGE_SEED")
        .map(|x| x.parse::<u64>().unwrap())
        .unwrap_or(0);
    assert!(selected_seed <= 3);
    for seed in 1u64..=3 {
        if selected_seed != 0 && selected_seed != seed {
            continue;
        }
        let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
        let data: Vec<_> = (0..4096)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            })
            .collect();
        for (case, methods) in cases.iter().enumerate() {
            for &x in &data {
                assert_eq!(methods[0](x), methods[1](x));
                assert_eq!(methods[0](x), methods[2](x));
            }
            // The large cases perform proportionately fewer calls; all three
            // implementations still receive identical work in each condition.
            let count = if case < 3 { batches } else { batches / 32 };
            assert!(count > 0);
            for j in 0..3 {
                let index = (j + seed as usize + case) % 3;
                let (seconds, operations, checksum) = measure(methods[index], &data, count);
                println!(
                    "{seed},{case},{},{seconds:.9},{operations},{checksum}",
                    ["std", "old", "new"][index]
                );
            }
        }
    }
}
