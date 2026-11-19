//! Validated NodeSlice lookup versus the previous revision and std slice search.
use std::{hint::black_box, time::Instant};
extern crate jzap_new;
extern crate jzap_old;
macro_rules! lookup {
    ($lib:ident, $view:expr, $query:expr) => {
        match $view.lookup($query).unwrap() {
            $lib::collections::btree_map::view::LookupStep::Found(_, value) => Some(*value),
            $lib::collections::btree_map::view::LookupStep::Absent => None,
            _ => unreachable!(),
        }
    };
}
fn rng(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}
fn measure<Q>(f: &dyn Fn(&Q) -> Option<u64>, data: &[Q], batches: usize) -> (f64, u64, u64) {
    let start = Instant::now();
    let mut sum = 0u64;
    for _ in 0..batches {
        for x in data {
            sum = sum.wrapping_add(black_box(f)(black_box(x)).unwrap_or(u64::MAX));
        }
    }
    (
        start.elapsed().as_secs_f64(),
        (data.len() * batches) as u64,
        black_box(sum),
    )
}
fn run<Q>(
    methods: [&dyn Fn(&Q) -> Option<u64>; 3],
    data: &[Q],
    seed: u64,
    case: usize,
    fixed: Option<usize>,
) {
    for x in data {
        assert_eq!(methods[0](x), methods[1](x));
        assert_eq!(methods[0](x), methods[2](x));
    }
    // Exclude calibration, then freeze equal work for all implementations.
    let batches = fixed.unwrap_or_else(|| {
        let mut pilot = 128;
        loop {
            let fastest = methods
                .iter()
                .map(|&f| measure(f, data, pilot).0)
                .fold(f64::INFINITY, f64::min);
            if fastest >= 0.05 {
                break ((pilot as f64) * 2.0 / fastest).ceil() as usize;
            }
            pilot *= 2;
        }
    });
    for j in 0..3 {
        let index = (j + seed as usize + case) % 3;
        let (seconds, operations, checksum) = measure(methods[index], data, batches);
        assert!(
            seconds >= 1.0,
            "Increase JZAP_RANGE_BATCHES for the complete comparison"
        );
        println!(
            "{seed},{case},{},{seconds:.9},{operations},{checksum}",
            ["std", "old", "new"][index]
        );
    }
}
fn main() {
    let fixed = std::env::var("JZAP_RANGE_BATCHES")
        .ok()
        .map(|x| x.parse::<usize>().unwrap());
    let seed: u64 = std::env::var("JZAP_RANGE_SEED").unwrap().parse().unwrap();
    assert!((1..=3).contains(&seed));
    let mut state = seed.wrapping_mul(0x9e3779b97f4a7c15);
    println!("seed,case,implementation,seconds,operations,checksum");
    for (shape, n) in [11usize, 64, 4096].into_iter().enumerate() {
        let words: Vec<u64> = (0..n as u64).map(|i| i * 2).collect();
        let strings: Vec<String> = words.iter().map(|i| format!("{i:016x}")).collect();
        let values = words.clone();
        let queries: Vec<u64> = (0..4096)
            .map(|_| rng(&mut state) % (n as u64 * 2 + 2))
            .collect();
        let texts: Vec<String> = queries.iter().map(|i| format!("{i:016x}")).collect();
        let old_word = jzap_old::collections::btree_map::view::NodeSlice::<_, _, usize>::new(
            &words, &values, None,
        )
        .unwrap();
        let new_word = jzap_new::collections::btree_map::view::NodeSlice::<_, _, usize>::new(
            &words, &values, None,
        )
        .unwrap();
        let old_text = jzap_old::collections::btree_map::view::NodeSlice::<_, _, usize>::new(
            &strings, &values, None,
        )
        .unwrap();
        let new_text = jzap_new::collections::btree_map::view::NodeSlice::<_, _, usize>::new(
            &strings, &values, None,
        )
        .unwrap();
        // Views are captured once. The timed corpus contains only queries, so
        // implementation-specific fields never change the corpus cache footprint.
        let standard_word = |q: &u64| words.binary_search(q).ok().map(|i| values[i]);
        let previous_word = |q: &u64| lookup!(jzap_old, old_word, q);
        let current_word = |q: &u64| lookup!(jzap_new, new_word, q);
        run(
            [&standard_word, &previous_word, &current_word],
            &queries,
            seed,
            shape * 2,
            fixed,
        );
        let standard_text = |q: &String| {
            strings
                .binary_search_by(|s| s.as_str().cmp(q.as_str()))
                .ok()
                .map(|i| values[i])
        };
        let previous_text = |q: &String| lookup!(jzap_old, old_text, q.as_str());
        let current_text = |q: &String| lookup!(jzap_new, new_text, q.as_str());
        run(
            [&standard_text, &previous_text, &current_text],
            &texts,
            seed,
            shape * 2 + 1,
            fixed,
        );
    }
}
