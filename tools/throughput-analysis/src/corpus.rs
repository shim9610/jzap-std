//! Deterministic input generation, always outside the measured interval.
pub const CASES: usize = 64;
pub const TASKS: [&str; 31] = [
    "grow",
    "reserved",
    "copy",
    "clone",
    "retain",
    "dedup",
    "drain",
    "extract",
    "splice",
    "within",
    "splice_unknown",
    "splice_no_hint",
    "extract_none",
    "extract_all",
    "insert_remove",
    "resize",
    "split_off",
    "append",
    "shrink",
    "pop",
    "box_u64_create_drop",
    "box_u64_clone_drop",
    "box_u64_clone_from",
    "box_slice_create_drop",
    "box_slice_clone_drop",
    "box_slice_clone_from",
    "box_str_create_drop",
    "box_str_clone_drop",
    "box_str_clone_from",
    "box_zeroed_create_drop",
    "box_collect_create_drop",
];

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[derive(Clone, Debug)]
pub struct Case {
    pub input: Vec<u64>,
    pub text: String,
    pub replacement: Vec<u64>,
    pub start: usize,
    pub end: usize,
    pub index: usize,
    pub resize: usize,
    pub threshold: u64,
    pub value: u64,
    pub limit: usize,
}
impl Case {
    pub fn len(&self, task: usize) -> usize {
        if task < 20 {
            self.input.len()
        } else if task < 23 {
            1
        } else {
            self.text.len()
        }
    }
}

pub struct Corpus {
    pub cases: Vec<Case>,
    pub digest: u64,
    pub elements: usize,
    pub min: usize,
    pub max: usize,
}
pub fn generate(seed: u64, task: usize, profile: &str) -> Corpus {
    assert!(profile == "mixed" || profile == "small");
    let mut random = Random(seed ^ (task as u64).wrapping_mul(0x517cc1b727220a95));
    let maximum: usize = if profile == "small" {
        64
    } else if task < 20 {
        131_072
    } else {
        1_048_576
    };
    let bits = maximum.ilog2() as usize;
    let mut cases = Vec::with_capacity(CASES);
    for i in 0..CASES {
        let n = if (20..23).contains(&task) {
            1
        } else {
            match i {
                0 => 0,
                1 => 1,
                2 => maximum,
                _ if profile == "small" => random.below(maximum + 1),
                _ => {
                    let exponent = random.below(bits + 1);
                    let lower = 1usize << exponent;
                    (lower + random.below(lower)).min(maximum)
                }
            }
        };
        let mut input = Vec::new();
        let mut text = String::new();
        if task < 20 {
            input.reserve_exact(n);
            let mut group = random.next() as u32 as u64;
            let mut left = 0;
            for _ in 0..n {
                if task == 5 {
                    if left == 0 {
                        group = random.next() as u32 as u64;
                        left = 1 + random.below(8);
                    }
                    left -= 1;
                    input.push(group);
                } else {
                    input.push(random.next() as u32 as u64);
                }
            }
        } else if task >= 23 {
            text.reserve_exact(n);
            for _ in 0..n {
                text.push((b'a' + random.below(26) as u8) as char);
            }
        }
        let a = random.below(n + 1);
        let b = random.below(n + 1);
        let replacement = if matches!(task, 8 | 10 | 11) {
            let count = random.below(n * 2 + 3);
            (0..count).map(|_| random.next()).collect()
        } else {
            Vec::new()
        };
        cases.push(Case {
            input,
            text,
            replacement,
            start: a.min(b),
            end: a.max(b),
            index: random.below(n + 1),
            resize: random.below(n * 2 + 9),
            threshold: match task {
                12 => 0,
                13 => u64::MAX,
                _ => random.next() as u32 as u64,
            },
            value: random.next(),
            limit: n * 3 + 32,
        });
    }
    // The generated call order is shared too. Do not time a random generator.
    for i in (1..cases.len()).rev() {
        let j = random.below(i + 1);
        cases.swap(i, j);
    }
    let mut digest = 0xcbf29ce484222325u64;
    let mut hash = |value: u64| {
        digest = (digest ^ value).wrapping_mul(0x100000001b3);
    };
    for c in &cases {
        for v in [c.len(task), c.start, c.end, c.index, c.resize, c.limit] {
            hash(v as u64);
        }
        hash(c.threshold);
        hash(c.value);
        for x in &c.input {
            hash(*x);
        }
        for x in &c.replacement {
            hash(*x);
        }
        for x in c.text.bytes() {
            hash(x as u64);
        }
    }
    let elements = cases.iter().map(|c| c.len(task)).sum();
    let min = cases.iter().map(|c| c.len(task)).min().unwrap();
    let max = cases.iter().map(|c| c.len(task)).max().unwrap();
    Corpus {
        cases,
        digest,
        elements,
        min,
        max,
    }
}
