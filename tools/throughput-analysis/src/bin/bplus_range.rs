use brie_tree::{BTree, nonmax::NonMaxU64};
use jzap::{TryIterator, collections::btree_map::view::bplus::FixedTree};
use std::{
    hint::black_box,
    ops::Bound::{Excluded, Included},
    time::Instant,
};

fn measure<F: Fn(u64, u64) -> (u64, u64) + ?Sized>(
    scan: &F,
    queries: &[(u64, u64)],
    batches: u64,
) -> (f64, u64, u64) {
    let start = Instant::now();
    let (mut count, mut sum) = (0u64, 0u64);
    for _ in 0..batches {
        for &(lo, hi) in queries {
            let (n, s) = black_box(scan(black_box(lo), black_box(hi)));
            count += n;
            sum = sum.wrapping_add(s);
        }
    }
    (
        start.elapsed().as_secs_f64(),
        black_box(count),
        black_box(sum),
    )
}

fn run<const N: usize>(seed: u64, height: u32) {
    let leaf_count = N.pow(height);
    let leaves: Vec<_> = (0..leaf_count)
        .map(|id| {
            let keys = std::array::from_fn(|i| (id * N + i) as u64 * 2 + 2);
            (keys, keys.map(|k| k * 3))
        })
        .collect::<Vec<([u64; N], [u64; N])>>();
    let mut branches: Vec<([u64; N], [usize; N])> = Vec::new();
    let (mut first, mut width) = (0, leaf_count);
    while width > 1 {
        let next = leaf_count + branches.len();
        for id in (first..first + width).step_by(N) {
            let children = std::array::from_fn(|k| id + k);
            let keys = children.map(|c| {
                if c < leaf_count {
                    leaves[c].0[0]
                } else {
                    branches[c - leaf_count].0[0]
                }
            });
            branches.push((keys, children));
        }
        first = next;
        width /= N;
    }
    let fixed = FixedTree::try_new(&leaves, &branches).unwrap();
    let mut brie = BTree::new();
    for (keys, values) in &leaves {
        for (&k, &v) in keys.iter().zip(values) {
            brie.insert(NonMaxU64::new(k).unwrap(), v);
        }
    }
    let fixed = black_box(&fixed);
    let brie = black_box(&brie);
    let scan_jzap = |lo: u64, hi: u64| {
        let mut cursor = fixed.range(Included(&lo), Excluded(&hi)).unwrap();
        let (mut n, mut sum) = (0, 0u64);
        while let Some((&k, &v)) = cursor.try_next().unwrap() {
            n += 1;
            sum = sum.wrapping_add(black_box(k ^ v));
        }
        (n, sum)
    };
    let scan_brie = |lo: u64, hi: u64| {
        let lo = NonMaxU64::new(lo).unwrap();
        let hi = NonMaxU64::new(hi).unwrap();
        let (mut n, mut sum) = (0, 0u64);
        for (k, &v) in brie.range(lo..hi) {
            n += 1;
            sum = sum.wrapping_add(black_box(k.get() ^ v));
        }
        (n, sum)
    };
    for (case, k) in [0, 1, 32, 256].into_iter().enumerate() {
        let mut rng = seed;
        let queries: Vec<_> = (0..4096)
            .map(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                let rank = rng % ((leaf_count * N) as u64 - k.max(1) + 1);
                let lo = rank * 2 + 2;
                if k == 0 {
                    (lo - 1, lo)
                } else {
                    (lo, lo + 2 * k)
                }
            })
            .collect();
        for &(lo, hi) in &queries {
            let mut j = fixed.range(Included(&lo), Excluded(&hi)).unwrap();
            let mut b = brie.range(NonMaxU64::new(lo).unwrap()..NonMaxU64::new(hi).unwrap());
            let first = (lo + 1) & !1;
            for t in 0..k {
                let key = first + 2 * t;
                assert_eq!(
                    j.try_next().unwrap().map(|(&x, &y)| (x, y)),
                    Some((key, key * 3))
                );
                assert_eq!(b.next().map(|(x, &y)| (x.get(), y)), Some((key, key * 3)));
            }
            assert!(j.try_next().unwrap().is_none() && b.next().is_none());
        }
        let methods: [(&str, &dyn Fn(u64, u64) -> (u64, u64)); 3] = [
            ("brie", &scan_brie),
            ("jzap", &scan_jzap),
            ("brie_control", &scan_brie),
        ];
        let mut min = f64::INFINITY;
        for (_, f) in methods {
            min = min.min(measure(f, &queries, 12).0);
        }
        let batches = (1.24 * 12.0 / min).ceil() as u64;
        let start = (seed as usize + case) % 3;
        let mut counts = None;
        for i in 0..3 {
            let offset = if seed % 2 == 0 { 3 - i } else { i };
            let (name, scan) = methods[(start + offset) % 3];
            let (seconds, items, checksum) = measure(scan, &queries, batches);
            assert!(
                seconds >= 1.0,
                "short interval {N}/{height}/{k}/{seed}/{name}/{seconds}"
            );
            assert_eq!(items, k * batches * queries.len() as u64);
            assert_eq!(*counts.get_or_insert((items, checksum)), (items, checksum));
            println!(
                "{seed},{N},{height},{k},{name},{seconds:.9},{},{items},{checksum}",
                batches * queries.len() as u64
            );
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().unwrap().parse().unwrap();
    println!("seed,fanout,height,items_per_range,impl,seconds,ranges,items,checksum");
    match args.next().as_deref() {
        Some("8") => run::<8>(seed, 2),
        Some("32") => run::<32>(seed, 2),
        Some("16") => run::<16>(seed, 3),
        _ => panic!("shape"),
    }
}
