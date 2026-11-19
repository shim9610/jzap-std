//! Equal random-input clone/edit/drop batches; includes allocation in every case.
extern crate jzap_new;
extern crate jzap_old;
use std::hint::black_box;
#[cfg(not(jzap_memory_probe))]
use std::time::Instant;
#[cfg(jzap_memory_probe)]
#[path = "../src/string_memory.rs"]
mod memory;
type Edit = fn(&[u64], usize) -> (usize, u64);
fn finish(values: &[u64]) -> (usize, u64) {
    (
        values.len(),
        values.first().copied().unwrap_or(0) ^ values.last().copied().unwrap_or(0),
    )
}
#[inline(never)]
fn standard(input: &[u64], kind: usize) -> (usize, u64) {
    let mut v = input.to_vec();
    match kind {
        0 => v.retain(|v| *v != u64::MAX),
        1 => v.retain(|v| *v & 1 == 0),
        _ => v.dedup(),
    }
    #[cfg(jzap_memory_probe)]
    observation::hold(std::mem::size_of_val(&v));
    black_box(finish(black_box(&v)))
}
macro_rules! product {
    ($name:ident, $lib:ident) => {
        #[inline(never)]
        fn $name(input: &[u64], kind: usize) -> (usize, u64) {
            let mut v = $lib::vec::Vec::<u64>::try_with_capacity(input.len()).unwrap();
            v.try_extend_copy(input).unwrap();
            match kind {
                0 => v.retain(|v| *v != u64::MAX),
                1 => v.retain(|v| *v & 1 == 0),
                _ => v.dedup(),
            }
            #[cfg(jzap_memory_probe)]
            observation::hold(std::mem::size_of_val(&v));
            black_box(finish(black_box(v.as_slice())))
        }
    };
}
product!(previous, jzap_old);
product!(current, jzap_new);
#[cfg(not(jzap_memory_probe))]
fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
#[cfg(not(jzap_memory_probe))]
fn sample(method: Edit, corpus: &[Vec<u64>], kind: usize, epochs: usize) -> (f64, u64) {
    let start = Instant::now();
    let mut checksum = 0u64;
    for _ in 0..epochs {
        for input in corpus {
            let (n, endpoints) = method(black_box(input), black_box(kind));
            checksum = checksum.wrapping_add(endpoints.wrapping_add(n as u64));
        }
    }
    (start.elapsed().as_secs_f64(), black_box(checksum))
}
#[cfg(not(jzap_memory_probe))]
fn main() {
    let seed: u64 = std::env::var("JZAP_RANGE_SEED").unwrap().parse().unwrap();
    let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
    let methods: [Edit; 3] = [standard, previous, current];
    println!("seed,case,implementation,seconds,operations,checksum");
    for case in 0..8 {
        let kind = case / 2;
        let corpus: Vec<Vec<u64>> = (0..64)
            .map(|_| {
                let n = if case % 2 == 0 {
                    random(&mut state) as usize % 33
                } else {
                    256 + random(&mut state) as usize % 3841
                };
                let mut last = random(&mut state);
                (0..n)
                    .map(|_| {
                        if kind != 3 || random(&mut state) & 3 == 0 {
                            last = random(&mut state);
                        }
                        last
                    })
                    .collect()
            })
            .collect();
        for input in &corpus {
            assert_eq!(previous(input, kind), standard(input, kind));
            assert_eq!(current(input, kind), standard(input, kind));
        }
        let mut epochs = 1;
        loop {
            let fastest = methods
                .iter()
                .map(|m| sample(*m, &corpus, kind, epochs).0)
                .fold(f64::INFINITY, f64::min);
            if fastest >= 0.05 {
                epochs = ((epochs as f64 * 2.0 / fastest).ceil() as usize).max(epochs);
                break;
            }
            epochs *= 2;
        }
        let mut order = [0, 1, 2];
        for i in (1..3).rev() {
            order.swap(i, random(&mut state) as usize % (i + 1));
        }
        let mut expected = None;
        for i in order {
            let (seconds, checksum) = sample(methods[i], &corpus, kind, epochs);
            assert!(
                seconds >= 1.0,
                "Short interval: increase calibration target and repeat all conditions"
            );
            assert!(expected.is_none_or(|x| x == checksum));
            expected = Some(checksum);
            println!(
                "{seed},{case},{},{seconds},{},{checksum}",
                ["std", "old", "new"][i],
                epochs * 64
            );
        }
    }
}

#[cfg(jzap_memory_probe)]
mod observation {
    use super::*;
    use std::cell::Cell;
    thread_local! {static HELD:Cell<(memory::Stats,usize)>=Cell::new((memory::Stats::default(),0));}
    pub fn hold(handle: usize) {
        HELD.with(|v| v.set((memory::snapshot(), handle)));
    }
    pub fn run() {
        println!(
            "kind,length,implementation,calls,held,peak,usable_held,usable_peak,after,usable_after,handle_bytes"
        );
        let methods: [Edit; 3] = [standard, previous, current];
        for kind in 0..4 {
            for length in [0usize, 1, 8, 32, 512, 4096] {
                let input: Vec<u64> = (0..length)
                    .map(|i| if kind == 3 { (i / 3) as u64 } else { i as u64 })
                    .collect();
                let mut expected = None;
                for (i, method) in methods.iter().enumerate() {
                    memory::start();
                    let result = method(&input, kind);
                    let (held, handle) = HELD.with(Cell::get);
                    let end = memory::stop();
                    assert!(expected.is_none_or(|v| v == result));
                    expected = Some(result);
                    assert_eq!(held.live, length * 8);
                    assert_eq!(end.peak, length * 8);
                    assert_eq!(end.calls, usize::from(length != 0));
                    assert_eq!((end.live, end.actual), (0, 0));
                    println!(
                        "{kind},{length},{},{},{},{},{},{},{},{},{handle}",
                        ["std", "old", "new"][i],
                        end.calls,
                        held.live,
                        end.peak,
                        held.actual,
                        end.actual_peak,
                        end.live,
                        end.actual
                    );
                }
            }
        }
    }
}
#[cfg(jzap_memory_probe)]
fn main() {
    observation::run();
}
