//! Timing only: default system allocator, no allocation instrumentation.
use jzap::{
    TryClone,
    boxed::Box as JBox,
    budget::{AllocationPolicy, Budget, MemoryBudget, Unmetered},
};
use std::{hint::black_box, time::Instant};

// Diagnostic control: std allocation with the same 16-byte handle, no accounting.
#[derive(Clone)]
struct StdHandle<'a> {
    value: Box<u64>,
    budget: &'a MemoryBudget,
}

fn measure<B, const PER_ITEM: bool>(
    n: usize,
    mut new: impl FnMut(u64) -> B,
    mut clone: impl FnMut(&B) -> B,
) -> [u128; 3] {
    // Observe each completed phase: making every returned aggregate opaque forces
    // a stack round trip for wide handles. PER_ITEM preserves that diagnostic.
    let t = Instant::now();
    let mut source = Vec::with_capacity(n);
    for i in 0..n {
        let value = new(black_box(i as u64));
        source.push(if PER_ITEM { black_box(value) } else { value });
    }
    if !PER_ITEM {
        black_box(&source);
    }
    let create = t.elapsed().as_nanos();
    let t = Instant::now();
    let mut target = Vec::with_capacity(n);
    for value in &source {
        let value = clone(black_box(value));
        target.push(if PER_ITEM { black_box(value) } else { value });
    }
    if !PER_ITEM {
        black_box(&target);
    }
    let cloned = t.elapsed().as_nanos();
    let t = Instant::now();
    black_box(&target);
    drop(target);
    drop(source);
    [create, cloned, t.elapsed().as_nanos()]
}
fn measure_jzap<P: AllocationPolicy, const PER_ITEM: bool>(n: usize, policy: P) -> [u128; 3] {
    measure::<_, PER_ITEM>(
        n,
        |x| JBox::try_new_in(x, policy.try_clone().unwrap()).ok().unwrap(),
        |x| x.try_clone().unwrap(),
    )
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args[1] == "sizes" {
        println!(
            "{{\"std_box\":{},\"jzap_box\":{},\"jzap_metered_box\":{},\"varlit_result\":{},\"jzap_new_result\":{},\"jzap_clone_result\":{}}}",
            size_of::<Box<u64>>(),
            size_of::<JBox<u64>>(),
            size_of::<JBox<u64, Budget<'_>>>(),
            size_of::<varlit_core::Result<Box<u64>>>(),
            size_of::<Result<JBox<u64>, jzap::Rejected<u64>>>(),
            size_of::<Result<JBox<u64>, jzap::Error>>()
        );
        return;
    }
    let implementation = &args[1];
    let n: usize = args[2].parse().unwrap();
    let barriers = args.get(3).map(String::as_str).unwrap_or("phase");
    assert!(matches!(barriers, "item" | "phase"));
    let budget = MemoryBudget::new(usize::MAX);
    // Two fixed warmup cycles; retain all thirty subsequent cycles.
    let mut samples = [[0u128; 3]; 30];
    for cycle in 0..32 {
        let sample = if barriers == "phase" {
            sample::<false>(implementation, n, &budget)
        } else {
            sample::<true>(implementation, n, &budget)
        };
        assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
        if cycle >= 2 {
            samples[cycle - 2] = sample;
        }
    }
    println!(
        "{{\"implementation\":\"{implementation}\",\"n\":{n},\"barriers\":\"{barriers}\",\"samples_ns\":{samples:?}}}"
    );
}

fn sample<const PER_ITEM: bool>(
    implementation: &str,
    n: usize,
    budget: &MemoryBudget,
) -> [u128; 3] {
    match implementation {
        "std" => measure::<_, PER_ITEM>(n, Box::new, |x| x.clone()),
        "std_handle16" => measure::<_, PER_ITEM>(
            n,
            |x| StdHandle {
                value: Box::new(x),
                budget,
            },
            |x| StdHandle {
                value: x.value.clone(),
                budget: x.budget,
            },
        ),
        "varlit" => measure::<_, PER_ITEM>(
            n,
            |x| varlit_core::try_box(x).unwrap(),
            |x| varlit_core::TryClone::try_clone(x).unwrap(),
        ),
        "unmetered" => measure_jzap::<_, PER_ITEM>(n, Unmetered),
        "metered" => measure_jzap::<_, PER_ITEM>(n, Budget::Limited(budget)),
        _ => panic!("unknown implementation"),
    }
}

// Instrumented only in the test executable; timings retain the system allocator.
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../tests/common/mod.rs"]
mod tracking;
#[cfg(test)]
#[global_allocator]
static ALLOCATOR: tracking::FaultAllocator = tracking::FaultAllocator;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrier_modes_preserve_real_allocations_and_budget_cleanup() {
        for n in [0, 1, 37, 10_000] {
            for label in ["std", "std_handle16", "unmetered", "metered", "varlit"] {
                let budget = MemoryBudget::new(usize::MAX);
                let (_, item) = tracking::track(usize::MAX, || sample::<true>(label, n, &budget));
                let (_, phase) = tracking::track(usize::MAX, || sample::<false>(label, n, &budget));
                let handle = if matches!(label, "metered" | "std_handle16") {
                    16
                } else {
                    8
                };
                let expected = (
                    if n == 0 { 0 } else { 2 * n + 2 },
                    0,
                    (2 * n * (8 + handle)) as isize,
                );
                assert_eq!((item.calls, item.live, item.peak), expected);
                assert_eq!((phase.calls, phase.live, phase.peak), expected);
                assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
            }
        }
    }

    fn check_values<const PER_ITEM: bool>() {
        let budget = MemoryBudget::new(37 * 16);
        let mut created = 0;
        let mut cloned = 0;
        measure::<_, PER_ITEM>(
            37,
            |value| {
                assert_eq!(value, created);
                created += 1;
                JBox::try_new_in(value, Budget::Limited(&budget))
                    .ok()
                    .unwrap()
            },
            |value| {
                assert_eq!(**value, cloned);
                cloned += 1;
                let copy = value.try_clone().unwrap();
                assert_eq!(*copy, **value);
                copy
            },
        );
        assert_eq!((created, cloned), (37, 37));
        assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
    }

    #[test]
    fn barrier_modes_preserve_values_and_operation_counts() {
        check_values::<true>();
        check_values::<false>();
    }
}
