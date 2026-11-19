//! Compare checked Copy against std without allocating inside the operation.
#[path = "../tests/common/mod.rs"]
mod common;
use common::{FaultAllocator, track};
use std::{hint::black_box, time::Instant};
#[global_allocator]
static ALLOC: FaultAllocator = FaultAllocator;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let n: usize = args[2].parse().unwrap();
    let source = vec![17u8; n];
    let mut target = vec![0u8; n];
    let repetitions = (16_000_000 / n).max(20);
    let (ns, stats) = track(1, || {
        let t = Instant::now();
        if args[1] == "std" {
            for _ in 0..repetitions {
                black_box(&mut target).copy_from_slice(black_box(&source));
            }
        } else {
            for _ in 0..repetitions {
                jzap::clone::try_copy_slice(black_box(&source), black_box(&mut target)).unwrap();
            }
        }
        t.elapsed().as_nanos()
    });
    assert_eq!(source, target);
    assert_eq!(stats.calls, 0);
    println!(
        "{{\"n\":{n},\"repetitions\":{repetitions},\"ns\":{ns},\"allocations\":{},\"live\":{},\"peak\":{}}}",
        stats.calls, stats.live, stats.peak
    );
}
