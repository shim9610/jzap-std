//! Phase controls for Vec extraction. Diagnostic timings do not replace the
//! fully inlined lifecycle acceptance benchmark in main.rs.
use jzap::{TryClone, vec::Vec as JVec};
use std::{hint::black_box, time::Instant};

#[inline(never)]
#[unsafe(no_mangle)]
pub fn std_extract_phase(v: &mut Vec<u64>, start: usize, end: usize, threshold: u64) {
    for x in v.extract_if(start..end, |x| *x < threshold) {
        black_box(x);
    }
    black_box(v.as_slice());
}
#[inline(never)]
#[unsafe(no_mangle)]
pub fn jzap_extract_phase(v: &mut JVec<u64>, start: usize, end: usize, threshold: u64) {
    for x in v
        .try_extract_if(start..end, |x| Ok::<_, ()>(*x < threshold))
        .unwrap()
    {
        black_box(x.unwrap());
    }
    black_box(v.as_slice());
}
trait Owner: Sized {
    fn source(input: &[u64]) -> Self;
    fn clone_owner(&self) -> Self;
    fn extract(&mut self, start: usize, end: usize, threshold: u64);
    fn slice(&self) -> &[u64];
}
impl Owner for Vec<u64> {
    fn source(input: &[u64]) -> Self {
        input.to_vec()
    }
    #[inline(always)]
    fn clone_owner(&self) -> Self {
        self.clone()
    }
    #[inline(always)]
    fn extract(&mut self, start: usize, end: usize, threshold: u64) {
        std_extract_phase(self, start, end, threshold);
    }
    fn slice(&self) -> &[u64] {
        self.as_slice()
    }
}
impl Owner for JVec<u64> {
    fn source(input: &[u64]) -> Self {
        Self::try_from_slice(input).unwrap()
    }
    #[inline(always)]
    fn clone_owner(&self) -> Self {
        self.try_clone().unwrap()
    }
    #[inline(always)]
    fn extract(&mut self, start: usize, end: usize, threshold: u64) {
        jzap_extract_phase(self, start, end, threshold);
    }
    fn slice(&self) -> &[u64] {
        self.as_slice()
    }
}
#[inline(never)]
fn run<T: Owner>(label: &str, mode: &str, n: usize, percent: usize, position: &str) {
    assert!(percent <= 100);
    let input: Vec<_> = (0..n as u64).collect();
    let source = T::source(&input);
    let (start, end) = match position {
        "middle" => (n / 4, n * 3 / 4),
        "full" => (0, n),
        "tail" => (n / 2, n),
        _ => panic!("position"),
    };
    let threshold = if percent == 100 {
        n
    } else if percent == 0 {
        0
    } else {
        start + (end - start) * percent / 100
    } as u64;
    let expected: Vec<_> = input
        .iter()
        .enumerate()
        .filter_map(|(i, x)| (!(start..end).contains(&i) || *x >= threshold).then_some(*x))
        .collect();
    // Pool is capped near 1MiB of payload. Setup/drop stay outside the timed
    // extraction phase; this has a different cache regime from hot reuse.
    let reps = (131072 / n.max(1)).clamp(4, 4096);
    let mut pool: Vec<T> = Vec::with_capacity(reps);
    let mut samples = Vec::with_capacity(30);
    for cycle in 0..32 {
        pool.clear();
        if matches!(mode, "extract" | "drop" | "control") {
            for _ in 0..reps {
                pool.push(source.clone_owner());
            }
        }
        let time = Instant::now();
        match mode {
            "lifecycle" => {
                for _ in 0..reps {
                    let mut v = black_box(&source).clone_owner();
                    v.extract(start, end, threshold);
                    drop(v);
                }
            }
            "clone" => {
                for _ in 0..reps {
                    pool.push(black_box(&source).clone_owner());
                }
            }
            "extract" => {
                for v in &mut pool {
                    v.extract(start, end, threshold);
                }
            }
            "drop" => {
                for v in pool.drain(..) {
                    drop(black_box(v));
                }
            }
            "control" => {
                for v in &pool {
                    black_box(v.slice());
                }
            }
            _ => panic!("phase"),
        }
        let elapsed = time.elapsed().as_nanos() as u64;
        // Functional agreement is checked outside the timer, not inferred from speed.
        if mode == "extract" {
            for v in &pool {
                assert_eq!(v.slice(), expected);
            }
        }
        if mode == "clone" {
            for v in &pool {
                assert_eq!(v.slice(), input);
            }
        }
        assert_eq!(source.slice(), input);
        if cycle >= 2 {
            samples.push(elapsed);
        }
    }
    println!(
        "{{\"label\":\"{label}\",\"phase\":\"{mode}\",\"n\":{n},\"percent\":{percent},\"position\":\"{position}\",\"repetitions\":{reps},\"samples_ns\":{samples:?}}}"
    );
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let label = &args[1];
    let mode = &args[2];
    let n = args[3].parse().unwrap();
    let percent = args.get(4).map(|v| v.parse().unwrap()).unwrap_or(100);
    let position = args.get(5).map(String::as_str).unwrap_or("middle");
    match label.as_str() {
        "std" => run::<Vec<u64>>(label, mode, n, percent, position),
        "jzap" => run::<JVec<u64>>(label, mode, n, percent, position),
        _ => panic!("label"),
    }
}
