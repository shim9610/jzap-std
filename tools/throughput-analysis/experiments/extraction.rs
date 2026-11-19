//! Equal extraction/splice workloads with exact-capacity clone/edit/drop.
extern crate jzap_new;
extern crate jzap_old;
use std::hint::black_box;
#[cfg(not(any(jzap_memory_probe, jzap_entry_probe, jzap_stat_probe)))]
use std::time::Instant;
#[cfg(jzap_memory_probe)]
#[path = "../src/string_memory.rs"]
mod memory;
#[cfg(not(jzap_entry_probe))]
type Edit = fn(&[u64], usize) -> (usize, u64);
#[cfg(jzap_entry_probe)]
std::thread_local! {
    static EXPECTED: std::cell::RefCell<Option<Vec<u64>>> = const {std::cell::RefCell::new(None)};
}
fn finish(values: &[u64]) -> (usize, u64) {
    #[cfg(jzap_entry_probe)]
    EXPECTED.with(|expected| {
        let mut expected = expected.borrow_mut();
        if let Some(reference) = expected.as_ref() {
            assert_eq!(values, reference.as_slice());
        } else {
            *expected = Some(values.to_vec());
        }
    });
    (
        values.len(),
        values.first().copied().unwrap_or(0) ^ values.last().copied().unwrap_or(0),
    )
}
#[cfg(jzap_splice_fill_probe)]
fn splice_shape(n: usize, kind: usize) -> (usize, usize, usize) {
    let start = n / 4;
    let end = n * 3 / 4;
    let hole = end - start;
    let replacement = match kind {
        0 => 0,
        1 => hole / 2,
        2 => hole,
        5 => n * 4 + 1,
        _ => hole + hole / 2 + 1,
    };
    (start, end, replacement)
}
#[cfg(jzap_splice_fill_probe)]
fn replacement_value(i: usize) -> u64 {
    (i as u64).wrapping_mul(17) ^ 0x91e10da5c79e7b1d
}
// Preserve values while selecting an honest exact or zero lower length hint.
#[cfg(jzap_splice_fill_probe)]
#[derive(Clone)]
struct Hinted<I> {
    inner: I,
    unknown: bool,
}
#[cfg(jzap_splice_fill_probe)]
impl<I: Iterator> Iterator for Hinted<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.inner.size_hint();
        (if self.unknown { 0 } else { lower }, upper)
    }
}
#[cfg(jzap_splice_fill_probe)]
fn replacement(count: usize, kind: usize) -> impl Iterator<Item = u64> + Clone {
    Hinted {
        inner: (0..count).map(replacement_value),
        unknown: kind >= 4,
    }
}
#[inline(never)]
fn standard(input: &[u64], kind: usize) -> (usize, u64) {
    let mut v = input.to_vec();
    #[cfg(jzap_splice_fill_probe)]
    {
        let (start, end, count) = splice_shape(v.len(), kind);
        let mut splice = v.splice(start..end, replacement(count, kind));
        for value in splice.by_ref() {
            black_box(value);
        }
        drop(splice);
    }
    #[cfg(not(jzap_splice_fill_probe))]
    {
        let predicate = |v: &mut u64| match kind {
            0 => false,
            1 => true,
            _ => *v & 1 == 0,
        };
        let mut iter = v.extract_if(.., predicate);
        if kind == 3 {
            black_box(iter.next());
        } else {
            for value in iter.by_ref() {
                black_box(value);
            }
        }
        drop(iter);
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
            #[cfg(jzap_splice_fill_probe)]
            {
                let (start, end, count) = splice_shape(v.len(), kind);
                let mut splice = v.try_splice(start..end, replacement(count, kind)).unwrap();
                for value in splice.by_ref() {
                    black_box(value);
                }
                splice.finish().unwrap();
            }
            #[cfg(not(jzap_splice_fill_probe))]
            {
                let predicate = |v: &mut u64| {
                    Ok::<_, core::convert::Infallible>(match kind {
                        0 => false,
                        1 => true,
                        _ => *v & 1 == 0,
                    })
                };
                let mut iter = v.try_extract_if(.., predicate).unwrap();
                if kind == 3 {
                    black_box(iter.next().map(Result::unwrap));
                } else {
                    for value in iter.by_ref() {
                        black_box(value.unwrap());
                    }
                }
                drop(iter);
            }
            #[cfg(jzap_memory_probe)]
            observation::hold(std::mem::size_of_val(&v));
            black_box(finish(black_box(v.as_slice())))
        }
    };
}
product!(previous, jzap_old);
product!(current, jzap_new);
#[cfg(not(any(jzap_memory_probe, jzap_entry_probe)))]
fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
#[cfg(not(any(jzap_memory_probe, jzap_entry_probe, jzap_stat_probe)))]
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
#[cfg(not(any(jzap_memory_probe, jzap_entry_probe, jzap_stat_probe)))]
fn main() {
    let seed: u64 = std::env::var("JZAP_RANGE_SEED").unwrap().parse().unwrap();
    let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
    let methods: [Edit; 3] = [standard, previous, current];
    println!("seed,case,implementation,seconds,operations,checksum");
    let cases = 12;
    for case in 0..cases {
        let kind = case / 2;
        let corpus: Vec<Vec<u64>> = (0..64)
            .map(|_| {
                let n = if case % 2 == 0 {
                    if cfg!(jzap_splice_fill_probe) && kind == 5 {
                        1 + random(&mut state) as usize % 32
                    } else {
                        random(&mut state) as usize % 33
                    }
                } else {
                    256 + random(&mut state) as usize % 3841
                };

                #[cfg_attr(jzap_splice_fill_probe, allow(unused_mut))]
                let mut values: Vec<u64> = (0..n).map(|_| random(&mut state)).collect();
                #[cfg(not(jzap_splice_fill_probe))]
                if kind >= 4 {
                    for value in &mut values {
                        *value |= 1;
                    }
                }
                #[cfg(not(jzap_splice_fill_probe))]
                if kind == 5 {
                    if let Some(last) = values.last_mut() {
                        *last &= !1;
                    }
                }
                values
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

#[cfg(jzap_stat_probe)]
#[path = "extraction_stats.rs"]
mod statistical;
#[cfg(jzap_stat_probe)]
fn main() {
    statistical::main();
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
        let patterns = 6;
        for kind in 0..patterns {
            for length in [0usize, 1, 8, 32, 512, 4096] {
                #[cfg_attr(jzap_splice_fill_probe, allow(unused_mut))]
                let mut input: Vec<u64> = (0..length)
                    .map(|i| if kind == 3 { (i / 3) as u64 } else { i as u64 })
                    .collect();
                #[cfg(not(jzap_splice_fill_probe))]
                if kind >= 4 {
                    for value in &mut input {
                        *value |= 1;
                    }
                }
                #[cfg(not(jzap_splice_fill_probe))]
                if kind == 5 {
                    if let Some(last) = input.last_mut() {
                        *last &= !1;
                    }
                }
                let mut expected = None;
                for (i, method) in methods.iter().enumerate() {
                    memory::start();
                    let result = method(&input, kind);
                    let (held, handle) = HELD.with(Cell::get);
                    let end = memory::stop();
                    assert!(expected.is_none_or(|v| v == result));
                    expected = Some(result);
                    #[cfg(not(jzap_splice_fill_probe))]
                    {
                        assert_eq!(held.live, length * 8);
                        assert_eq!(end.peak, length * 8);
                        assert_eq!(end.calls, usize::from(length != 0));
                    }
                    assert!(held.live >= result.0 * core::mem::size_of::<u64>());
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

// This mode links disposable entry-instrumented libraries, never timing builds.
#[cfg(all(jzap_entry_probe, jzap_splice_fill_probe))]
fn main() {
    use std::sync::atomic::Ordering;
    println!("kind,length,implementation,stage_entries");
    for kind in 0..6 {
        for length in [0usize, 1, 8, 32, 512, 4096] {
            let input: Vec<u64> = (0..length).map(|i| i as u64).collect();
            EXPECTED.with(|expected| *expected.borrow_mut() = None);
            let reference = standard(&input, kind);
            jzap_old::STAGE_ENTRIES.store(0, Ordering::Relaxed);
            assert_eq!(previous(&input, kind), reference);
            println!(
                "{kind},{length},old,{}",
                jzap_old::STAGE_ENTRIES.load(Ordering::Relaxed)
            );
            jzap_new::STAGE_ENTRIES.store(0, Ordering::Relaxed);
            assert_eq!(current(&input, kind), reference);
            println!(
                "{kind},{length},new,{}",
                jzap_new::STAGE_ENTRIES.load(Ordering::Relaxed)
            );
        }
    }
}
