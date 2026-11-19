use jzap::{TryClone, collections::HashSet as Set, collections::hash_map::RandomState};
use std::{collections::HashSet as StdSet, hint::black_box, time::Instant};
type Matched = StdSet<u64, RandomState>;
struct Case {
    a: Set<u64>,
    b: Set<u64>,
    sa: StdSet<u64>,
    sb: StdSet<u64>,
    ma: Matched,
    mb: Matched,
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn current(a: &Set<u64>, b: &Set<u64>) -> bool {
    a.is_disjoint(b)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn min_all(a: &Set<u64>, b: &Set<u64>) -> bool {
    let (a, b) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    a.iter().all(|k| !b.contains(k))
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn split_all(a: &Set<u64>, b: &Set<u64>) -> bool {
    if a.len() <= b.len() {
        a.iter().all(|k| !b.contains(k))
    } else {
        b.iter().all(|k| !a.contains(k))
    }
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn lookup(set: &Set<u64>, key: &u64) -> bool {
    set.contains(key)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn boundary_all(a: &Set<u64>, b: &Set<u64>) -> bool {
    if a.len() <= b.len() {
        a.iter().all(|k| !lookup(b, k))
    } else {
        b.iter().all(|k| !lookup(a, k))
    }
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn native(a: &StdSet<u64>, b: &StdSet<u64>) -> bool {
    a.is_disjoint(b)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn matched(a: &Matched, b: &Matched) -> bool {
    a.is_disjoint(b)
}
fn rand(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn cases(n: usize, overlap: bool, mut seed: u64, shape: &str) -> Vec<Case> {
    (0..64)
        .map(|case| {
            let right_size = match shape {
                "equal" => n,
                "skew" => n.min(3),
                "vary" => rand(&mut seed) as usize % (n + 1),
                _ => panic!("unknown shape"),
            };
            let reverse = shape != "equal" && rand(&mut seed) & 1 != 0;
            let mut a = Set::try_new().unwrap();
            let mut b = Set::try_new().unwrap();
            let mut sa = StdSet::new();
            let mut sb = StdSet::new();
            let mut ma = Matched::with_hasher(a.hasher().try_clone().unwrap());
            let mut mb = Matched::with_hasher(b.hasher().try_clone().unwrap());
            let mut keys = (0..n as u64)
                .map(|k| ((case * 1_000_000 + k) * 2, overlap && k % 2 == 0))
                .collect::<Vec<_>>();
            for i in (1..n).rev() {
                let j = rand(&mut seed) as usize % (i + 1);
                keys.swap(i, j)
            }
            for (index, (key, shared)) in keys.into_iter().enumerate() {
                let right = if shared { key } else { key | 1 };
                a.try_insert(key).unwrap();
                sa.insert(key);
                ma.insert(key);
                if index < right_size {
                    b.try_insert(right).unwrap();
                    sb.insert(right);
                    mb.insert(right);
                }
            }
            if reverse {
                std::mem::swap(&mut a, &mut b);
                std::mem::swap(&mut sa, &mut sb);
                std::mem::swap(&mut ma, &mut mb);
            }
            assert_eq!(
                a.iter().copied().collect::<Vec<_>>(),
                ma.iter().copied().collect::<Vec<_>>()
            );
            assert_eq!(
                b.iter().copied().collect::<Vec<_>>(),
                mb.iter().copied().collect::<Vec<_>>()
            );
            Case {
                a,
                b,
                sa,
                sb,
                ma,
                mb,
            }
        })
        .collect()
}
#[inline(never)]
fn epoch<const V: usize>(cases: &[Case], epochs: u64) -> u64 {
    let mut sum = 0;
    for _ in 0..epochs {
        for c in cases {
            let c = black_box(c);
            let result = match V {
                0 => native(black_box(&c.sa), black_box(&c.sb)),
                1 => current(black_box(&c.a), black_box(&c.b)),
                2 => min_all(black_box(&c.a), black_box(&c.b)),
                3 => split_all(black_box(&c.a), black_box(&c.b)),
                4 => boundary_all(black_box(&c.a), black_box(&c.b)),
                5 => matched(black_box(&c.ma), black_box(&c.mb)),
                _ => unreachable!(),
            };
            sum += u64::from(black_box(result));
        }
    }
    sum
}
fn run(label: &str, cases: &[Case], epochs: u64) -> (f64, u64) {
    let start = Instant::now();
    let checksum = match label {
        "std" | "std_control" => epoch::<0>(cases, epochs),
        "current" => epoch::<1>(cases, epochs),
        "min_all" => epoch::<2>(cases, epochs),
        "split_all" => epoch::<3>(cases, epochs),
        "boundary_all" => epoch::<4>(cases, epochs),
        "matched_std" => epoch::<5>(cases, epochs),
        _ => panic!(),
    };
    (start.elapsed().as_secs_f64(), checksum)
}
fn visits<'a>(iter: impl Iterator<Item = &'a u64>, mut contains: impl FnMut(&u64) -> bool) -> u64 {
    let mut count = 0;
    for k in iter {
        count += 1;
        if contains(k) {
            break;
        }
    }
    count
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let n = args[1].parse::<usize>().unwrap();
    let overlap = args[2] == "overlap";
    let seed = args[3].parse::<u64>().unwrap();
    let shape = args.get(4).map(String::as_str).unwrap_or("equal");
    let cases = cases(n, overlap, seed, shape);
    let our_visits = cases
        .iter()
        .map(|c| {
            if c.a.len() <= c.b.len() {
                visits(c.a.iter(), |k| c.b.contains(k))
            } else {
                visits(c.b.iter(), |k| c.a.contains(k))
            }
        })
        .sum::<u64>();
    let std_visits = cases
        .iter()
        .map(|c| {
            if c.sa.len() <= c.sb.len() {
                visits(c.sa.iter(), |k| c.sb.contains(k))
            } else {
                visits(c.sb.iter(), |k| c.sa.contains(k))
            }
        })
        .sum::<u64>();
    let matched_visits = cases
        .iter()
        .map(|c| {
            if c.ma.len() <= c.mb.len() {
                visits(c.ma.iter(), |k| c.mb.contains(k))
            } else {
                visits(c.mb.iter(), |k| c.ma.contains(k))
            }
        })
        .sum::<u64>();
    assert_eq!(our_visits, matched_visits);
    let defaults = vec![
        "std",
        "current",
        "min_all",
        "split_all",
        "boundary_all",
        "matched_std",
        "std_control",
    ];
    let labels: Vec<&str> = args
        .get(5)
        .map(|s| s.split(',').collect())
        .unwrap_or(defaults);
    assert!(labels.contains(&"std") && labels.contains(&"std_control"));
    let mut common = 1;
    for label in &labels {
        black_box(run(label, &cases, 2));
        let mut epochs = 4;
        loop {
            let (t, _) = run(label, &cases, epochs);
            if t >= 0.05 {
                common = common.max((epochs as f64 * 1.25 / t).ceil() as u64);
                break;
            }
            epochs *= ((0.055 / t).ceil() as u64).clamp(2, 1024);
        }
    }
    let expected = cases
        .iter()
        .map(|c| {
            let value = native(&c.sa, &c.sb);
            assert_eq!(current(&c.a, &c.b), value);
            assert_eq!(min_all(&c.a, &c.b), value);
            assert_eq!(matched(&c.ma, &c.mb), value);
            u64::from(value)
        })
        .sum::<u64>();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let mut shortest = f64::INFINITY;
        for i in 0..labels.len() {
            let label = labels[(i + (seed as usize % labels.len())) % labels.len()];
            let (t, sum) = run(label, &cases, common);
            assert_eq!(sum, expected * common);
            shortest = shortest.min(t);
            println!(
                "{{\"size\":{n},\"shape\":\"{shape}\",\"kind\":\"{}\",\"seed\":{seed},\"label\":\"{label}\",\"calls\":{},\"seconds\":{t},\"attempt\":{attempt},\"our_visits\":{our_visits},\"std_visits\":{std_visits},\"matched_visits\":{matched_visits}}}",
                if overlap { "overlap" } else { "true" },
                common * 64
            );
        }
        if shortest >= 1. {
            println!("{{\"complete\":true,\"attempt\":{attempt}}}");
            break;
        }
        common = (common as f64 * (1.4_f64.max(1.25 / shortest))).ceil() as u64;
    }
}
