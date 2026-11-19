//! Repeated, paired measurements of the unchanged extraction consumers.
//! Linux diagnostics only; not part of the product library.
use super::{Edit, current, previous, random, standard};
use std::{hint::black_box, io::Write, time::Instant};

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
compile_error!("Statistical timing diagnostics require 64-bit Linux");

#[repr(C)]
struct Timespec {
    seconds: std::os::raw::c_long,
    nanoseconds: std::os::raw::c_long,
}
unsafe extern "C" {
    fn clock_gettime(clock: std::os::raw::c_int, output: *mut Timespec) -> std::os::raw::c_int;
}
fn thread_seconds() -> f64 {
    let mut t = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: CLOCK_THREAD_CPUTIME_ID and writable timespec in Linux's C ABI.
    assert_eq!(unsafe { clock_gettime(3, &mut t) }, 0);
    t.seconds as f64 + t.nanoseconds as f64 * 1e-9
}
fn switches() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let value = |key: &str| {
        s.lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap()
            .trim()
            .parse::<u64>()
            .unwrap()
    };
    (
        value("voluntary_ctxt_switches:"),
        value("nonvoluntary_ctxt_switches:"),
    )
}
struct Sample {
    wall: f64,
    cpu: f64,
    checksum: u64,
    voluntary: u64,
    involuntary: u64,
}
fn measure(method: Edit, corpus: &[Vec<u64>], kind: usize, epochs: usize) -> Sample {
    let before = switches();
    let cpu = thread_seconds();
    let start = Instant::now();
    let mut checksum = 0u64;
    for _ in 0..epochs {
        for input in corpus {
            let (n, endpoints) = method(black_box(input), black_box(kind));
            checksum = checksum.wrapping_add(endpoints.wrapping_add(n as u64));
        }
    }
    let wall = start.elapsed().as_secs_f64();
    let cpu = thread_seconds() - cpu;
    let after = switches();
    Sample {
        wall,
        cpu,
        checksum: black_box(checksum),
        voluntary: after.0 - before.0,
        involuntary: after.1 - before.1,
    }
}
fn corpus(seed: u64, case: usize) -> Vec<Vec<u64>> {
    // Per-case streams keep the corpus identical when case/run order changes.
    let mut state = seed
        .wrapping_mul(0x9e3779b97f4a7c15)
        .wrapping_add((case as u64 + 1).wrapping_mul(0xd1b54a32d192ed03));
    (0..64)
        .map(|_| {
            let n = if case % 2 == 0 {
                random(&mut state) as usize % 33
            } else {
                256 + random(&mut state) as usize % 3841
            };
            let mut v: Vec<u64> = (0..n).map(|_| random(&mut state)).collect();
            if !cfg!(jzap_splice_fill_probe) && case / 2 >= 4 {
                for x in &mut v {
                    *x |= 1;
                }
                if case / 2 == 5 {
                    if let Some(x) = v.last_mut() {
                        *x &= !1;
                    }
                }
            }
            v
        })
        .collect()
}
fn fingerprint(corpus: &[Vec<u64>]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for input in corpus {
        for value in std::iter::once(input.len() as u64).chain(input.iter().copied()) {
            h = (h ^ value).wrapping_mul(0x100000001b3);
        }
    }
    h
}
fn emit(
    block: usize,
    seed: u64,
    case: usize,
    attempt: usize,
    phase: &str,
    label: &str,
    position: usize,
    epochs: usize,
    digest: u64,
    s: &Sample,
) {
    println!(
        "{block},{seed},{case},{attempt},{phase},{label},{position},{epochs},{},{:.9},{:.9},{},{digest:016x},{},{}",
        epochs * 64,
        s.wall,
        s.cpu,
        s.checksum,
        s.voluntary,
        s.involuntary
    );
}
pub fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "seed block target-seconds");
    let seed = args[1].parse::<u64>().unwrap();
    let block = args[2].parse::<usize>().unwrap();
    let target = args[3].parse::<f64>().unwrap();
    assert!(seed > 0 && target >= 1.5 && target.is_finite());
    // The control calls the exact same function address as old.
    let methods: [Edit; 4] = [standard, previous, current, previous];
    let labels = ["std", "old", "new", "old_control"];
    let mut order_state = 0x81e22d9f41235791u64.wrapping_add(block as u64);
    let mut cases: Vec<_> = (0..12).collect();
    for i in (1..cases.len()).rev() {
        cases.swap(i, random(&mut order_state) as usize % (i + 1));
    }
    println!(
        "block,seed,case,attempt,phase,label,position,epochs,operations,wall_seconds,cpu_seconds,checksum,corpus_digest,voluntary_switches,involuntary_switches"
    );
    for case in cases {
        let corpus = corpus(seed, case);
        let digest = fingerprint(&corpus);
        for input in &corpus {
            let expected = standard(input, case / 2);
            assert_eq!(previous(input, case / 2), expected);
            assert_eq!(current(input, case / 2), expected);
        }
        let mut epochs = 1usize;
        loop {
            let samples: Vec<_> = methods[..3]
                .iter()
                .map(|m| measure(*m, &corpus, case / 2, epochs))
                .collect();
            let fastest = samples.iter().map(|s| s.wall).fold(f64::INFINITY, f64::min);
            for (i, s) in samples.iter().enumerate() {
                emit(
                    block,
                    seed,
                    case,
                    0,
                    "calibration",
                    labels[i],
                    i,
                    epochs,
                    digest,
                    s,
                );
            }
            if fastest >= 0.05 {
                epochs = ((epochs as f64 * target * 1.1 / fastest).ceil() as usize).max(epochs);
                break;
            }
            epochs = epochs.checked_mul(2).unwrap();
        }
        let mut order = [0, 1, 2, 3];
        let mut case_order = 0x3916a9f25472b13fu64.wrapping_add(case as u64);
        for i in (1..4).rev() {
            order.swap(i, random(&mut case_order) as usize % (i + 1));
        }
        // Across twelve blocks each implementation occupies each position three times.
        order.rotate_left(block % 4);
        let mut completed = false;
        for attempt in 1..=3 {
            let measured: Vec<_> = order
                .iter()
                .map(|i| measure(methods[*i], &corpus, case / 2, epochs))
                .collect();
            assert!(measured.iter().all(|s| s.checksum == measured[0].checksum));
            let valid = measured.iter().all(|s| s.wall >= 1.0 && s.cpu > 0.0);
            for (position, (i, s)) in order.iter().zip(&measured).enumerate() {
                emit(
                    block,
                    seed,
                    case,
                    attempt,
                    if valid {
                        "measurement"
                    } else {
                        "short-attempt"
                    },
                    labels[*i],
                    position,
                    epochs,
                    digest,
                    s,
                );
            }
            std::io::stdout().flush().unwrap();
            if valid {
                completed = true;
                break;
            }
            epochs = epochs.checked_mul(2).unwrap();
        }
        assert!(
            completed,
            "Three short groups; no accepted result for this condition"
        );
    }
}
