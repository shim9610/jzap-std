//! Public Box collection: exact, empty and partial lower hints at equal input work.
use std::hint::black_box;
#[cfg(not(jzap_memory_probe))]
use std::time::Instant;
extern crate jzap_new;
extern crate jzap_old;
#[derive(Clone, Copy)]
struct Input {
    seed: u64,
    length: usize,
}
type Method = fn(Input) -> (usize, u64);
struct Source<const MODE: u8> {
    position: usize,
    input: Input,
}
impl<const MODE: u8> Iterator for Source<MODE> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        if self.position == self.input.length {
            return None;
        }
        let value = black_box(self.input.seed.wrapping_add(self.position as u64));
        self.position += 1;
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.input.length - self.position;
        match MODE {
            0 => (remaining, Some(remaining)),
            1 => (0, None),
            _ => (remaining / 2, Some(remaining)),
        }
    }
}
#[cfg(jzap_memory_probe)]
#[path = "../src/string_memory.rs"]
mod memory;
#[cfg(jzap_memory_probe)]
mod observation {
    use super::*;
    use std::cell::Cell;
    thread_local! {static HELD:Cell<memory::Stats>=Cell::new(memory::Stats::default());}
    pub fn hold() {
        HELD.with(|x| x.set(memory::snapshot()));
    }
    pub fn run() {
        let cases: [[Method; 3]; 3] = [
            [std_exact, old_exact, new_exact],
            [std_empty, old_empty, new_empty],
            [std_partial, old_partial, new_partial],
        ];
        println!(
            "seed,mode,elements,implementation,calls,held,peak,usable_held,usable_peak,after,usable_after"
        );
        for seed in 1u64..=3 {
            for (mode, methods) in cases.iter().enumerate() {
                for length in [8, 9, 15, 16, 512, 513, 1023, 1024] {
                    for (label, &method) in methods.iter().enumerate() {
                        let input = Input {
                            seed: seed.wrapping_mul(0x9e3779b97f4a7c15),
                            length,
                        };
                        memory::start();
                        let reply = method(input);
                        let held = HELD.with(Cell::get);
                        let end = memory::stop();
                        assert_eq!(reply, (length, input.seed.wrapping_add(length as u64 - 1)));
                        assert_eq!(held.live, length * 8);
                        assert!(end.live == 0 && end.actual == 0);
                        println!(
                            "{seed},{mode},{length},{},{},{},{},{},{},{},{}",
                            ["std", "old", "new"][label],
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
}
macro_rules! product {
    ($name:ident,$lib:ident,$mode:expr) => {
        #[inline(never)]
        fn $name(input: Input) -> (usize, u64) {
            let result =
                $lib::boxed::Box::<[u64]>::try_from_iter(Source::<$mode> { position: 0, input })
                    .unwrap();
            #[cfg(jzap_memory_probe)]
            observation::hold();
            black_box(&result);
            (result.len(), result.as_slice()[black_box(input.length - 1)])
        }
    };
}
macro_rules! standard {
    ($name:ident,$mode:expr) => {
        #[inline(never)]
        fn $name(input: Input) -> (usize, u64) {
            let result = Source::<$mode> { position: 0, input }.collect::<Box<[u64]>>();
            #[cfg(jzap_memory_probe)]
            observation::hold();
            black_box(&result);
            (result.len(), result[black_box(input.length - 1)])
        }
    };
}
product!(old_exact, jzap_old, 0);
product!(new_exact, jzap_new, 0);
standard!(std_exact, 0);
product!(old_empty, jzap_old, 1);
product!(new_empty, jzap_new, 1);
standard!(std_empty, 1);
product!(old_partial, jzap_old, 2);
product!(new_partial, jzap_new, 2);
standard!(std_partial, 2);
#[cfg(not(jzap_memory_probe))]
fn rng(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
#[cfg(not(jzap_memory_probe))]
fn measure(method: Method, data: &[Input], batches: usize) -> (f64, u64, u64) {
    let start = Instant::now();
    let mut sum = 0u64;
    let mut calls = 0u64;
    for _ in 0..batches {
        for &x in data {
            let (len, last) = black_box(method)(black_box(x));
            sum = sum.wrapping_add(last ^ len as u64);
            calls += 1;
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    assert!(
        seconds >= 1.0,
        "Increase JZAP_RANGE_BATCHES for the whole comparison"
    );
    (seconds, calls, black_box(sum))
}
#[cfg(jzap_memory_probe)]
fn main() {
    observation::run();
}
#[cfg(not(jzap_memory_probe))]
fn main() {
    let batches = std::env::var("JZAP_RANGE_BATCHES")
        .map(|x| x.parse::<usize>().unwrap())
        .unwrap_or(50000);
    let selected = std::env::var("JZAP_RANGE_SEED")
        .map(|x| x.parse::<u64>().unwrap())
        .unwrap_or(0);
    assert!(selected <= 3);
    let cases: [[Method; 3]; 3] = [
        [std_exact, old_exact, new_exact],
        [std_empty, old_empty, new_empty],
        [std_partial, old_partial, new_partial],
    ];
    println!("seed,case,implementation,seconds,operations,checksum");
    for seed in 1u64..=3 {
        if selected != 0 && selected != seed {
            continue;
        }
        let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
        for case in 0..6 {
            let bound = if case < 3 { 16 } else { 1024 };
            let data: Vec<_> = (0..4096)
                .map(|_| Input {
                    seed: rng(&mut state),
                    length: bound / 2 + (rng(&mut state) as usize) % (bound / 2 + 1),
                })
                .collect();
            let methods = cases[case % 3];
            for &x in &data {
                assert_eq!(methods[0](x), methods[1](x));
                assert_eq!(methods[0](x), methods[2](x));
            }
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
