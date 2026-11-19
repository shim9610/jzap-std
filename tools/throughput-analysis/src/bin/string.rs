//! Long-interval String/std comparisons. Generation and reservation stay outside timing.
#[cfg(feature = "string-memory")]
#[path = "../string_memory.rs"]
mod memory;
use jzap::{
    TryClone,
    budget::{AllocationPolicy, Budget, MemoryBudget, Unmetered},
    string::String as JString,
    vec::{LengthLimit, LengthPolicy},
};
use std::{fmt::Write, hint::black_box, time::Instant};
const TASKS: [&str; 11] = [
    "string_construct",
    "string_reserved",
    "string_clone",
    "string_replace",
    "string_retain",
    "string_pop",
    "string_format",
    "string_within",
    "string_drain",
    "string_utf16",
    "string_lossy",
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
}
struct Case {
    text: String,
    units: Vec<u16>,
    invalid: Vec<u8>,
    at: usize,
    end: usize,
    limit: usize,
}
fn corpus(seed: u64, small: bool) -> Vec<Case> {
    let mut rng = Random(seed);
    let mut cases = Vec::new();
    let max: usize = if small { 32 } else { 16384 };
    for i in 0..64 {
        let n = match i {
            0 => 0,
            1 => 1,
            2 => max,
            _ => {
                let exp = rng.next() as u32 % (max.ilog2() + 1);
                ((1usize << exp) + (rng.next() as usize % (1usize << exp))).min(max)
            }
        };
        let mut text = String::new();
        for _ in 0..n {
            let chars = if i % 2 == 0 {
                ['a', 'x', 'b', 'c']
            } else {
                ['a', 'é', '한', '😀']
            };
            text.push(chars[rng.next() as usize % 4]);
        }
        let boundaries: Vec<_> = text
            .char_indices()
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let a = boundaries[rng.next() as usize % boundaries.len()];
        let b = boundaries[rng.next() as usize % boundaries.len()];
        let limit = text.len() * 3 + 128;
        let units = text.encode_utf16().collect();
        let mut invalid = text.as_bytes().to_vec();
        invalid.push(0xff);
        cases.push(Case {
            units,
            invalid,
            text,
            at: a.min(b),
            end: a.max(b),
            limit,
        });
    }
    for i in (1..cases.len()).rev() {
        let j = rng.next() as usize % (i + 1);
        cases.swap(i, j);
    }
    cases
}
trait Owner: Sized {
    fn text(&self) -> &str;
    fn append(&mut self, s: &str);
    fn reserve(&mut self, n: usize);
    fn clear(&mut self);
    fn copy(&self) -> Self;
    fn converted<const OP: usize>(&self, c: &Case) -> Self;
    fn edit<const OP: usize>(&mut self, c: &Case);
}
impl Owner for String {
    #[inline(always)]
    fn text(&self) -> &str {
        self
    }
    #[inline(always)]
    fn append(&mut self, s: &str) {
        self.push_str(s)
    }
    #[inline(always)]
    fn reserve(&mut self, n: usize) {
        self.reserve_exact(n)
    }
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
    #[inline(always)]
    fn copy(&self) -> Self {
        self.clone()
    }
    #[inline(always)]
    fn converted<const OP: usize>(&self, c: &Case) -> Self {
        if OP == 9 {
            String::from_utf16(black_box(&c.units)).unwrap()
        } else {
            String::from_utf8_lossy(black_box(&c.invalid)).into_owned()
        }
    }
    #[inline(always)]
    fn edit<const OP: usize>(&mut self, c: &Case) {
        match OP {
            3 => self.replace_range(c.at..c.end, black_box("aé한😀")),
            4 => self.retain(|ch| ch != 'x' && ch != '한'),
            5 => {
                while let Some(ch) = self.pop() {
                    black_box(ch);
                }
            }
            6 => write!(
                self,
                "{}:{}:{}",
                black_box(c.text.as_str()),
                black_box(c.at),
                black_box('한')
            )
            .unwrap(),
            7 => self.extend_from_within(c.at..c.end),
            8 => {
                for ch in self.drain(c.at..c.end) {
                    black_box(ch);
                }
            }
            _ => (),
        }
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> Owner for JString<P, L> {
    #[inline(always)]
    fn text(&self) -> &str {
        self.as_str()
    }
    #[inline(always)]
    fn append(&mut self, s: &str) {
        self.try_push_str(s).unwrap()
    }
    #[inline(always)]
    fn reserve(&mut self, n: usize) {
        self.try_reserve_exact(n).unwrap()
    }
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
    #[inline(always)]
    fn copy(&self) -> Self {
        self.try_clone().unwrap()
    }
    #[inline(always)]
    fn converted<const OP: usize>(&self, c: &Case) -> Self {
        if OP == 9 {
            Self::try_from_utf16_in(
                black_box(&c.units),
                self.allocation_policy().try_clone().unwrap(),
                self.length_policy(),
            )
            .unwrap()
        } else {
            match Self::try_from_utf8_lossy_in(
                black_box(&c.invalid),
                self.allocation_policy().try_clone().unwrap(),
                self.length_policy(),
            )
            .unwrap()
            {
                jzap::string::Utf8Lossy::Owned(text) => text,
                _ => unreachable!("corpus has an invalid trailing byte"),
            }
        }
    }
    #[inline(always)]
    fn edit<const OP: usize>(&mut self, c: &Case) {
        match OP {
            3 => self
                .try_replace_range(c.at..c.end, black_box("aé한😀"))
                .unwrap(),
            4 => self.retain(|ch| ch != 'x' && ch != '한'),
            5 => {
                while let Some(ch) = self.pop() {
                    black_box(ch);
                }
            }
            6 => self
                .try_write_fmt(format_args!(
                    "{}:{}:{}",
                    black_box(c.text.as_str()),
                    black_box(c.at),
                    black_box('한')
                ))
                .unwrap(),
            7 => self.try_extend_from_within(c.at..c.end).unwrap(),
            8 => {
                for ch in self.try_drain(c.at..c.end).unwrap() {
                    black_box(ch);
                }
            }
            _ => (),
        }
    }
}
trait Make<'a>: Owner {
    fn make(c: &Case, b: &'a MemoryBudget) -> Self;
}
impl<'a> Make<'a> for String {
    fn make(_: &Case, _: &'a MemoryBudget) -> Self {
        Self::new()
    }
}
impl<'a> Make<'a> for JString {
    fn make(_: &Case, _: &'a MemoryBudget) -> Self {
        Self::new()
    }
}
impl<'a> Make<'a> for JString<Unmetered, LengthLimit> {
    fn make(c: &Case, _: &'a MemoryBudget) -> Self {
        JString::with_limit(c.limit)
    }
}
impl<'a> Make<'a> for JString<Budget<'a>> {
    fn make(_: &Case, b: &'a MemoryBudget) -> Self {
        JString::new_in(Budget::Limited(b))
    }
}
impl<'a> Make<'a> for JString<Budget<'a>, LengthLimit> {
    fn make(c: &Case, b: &'a MemoryBudget) -> Self {
        JString::with_limit_in(c.limit, Budget::Limited(b))
    }
}
fn epochs<'a, T: Make<'a>, const OP: usize>(
    cases: &[Case],
    sources: &mut [T],
    budget: &'a MemoryBudget,
    count: u64,
) {
    for _ in 0..count {
        for (c, source) in cases.iter().zip(sources.iter_mut()) {
            if OP == 1 {
                source.clear();
                source.append(black_box(&c.text));
                black_box(source.text());
            } else {
                let mut value = if OP == 0 || OP == 6 || OP >= 9 {
                    T::make(c, budget)
                } else {
                    source.copy()
                };
                if OP >= 9 {
                    value = value.converted::<OP>(c);
                } else if OP == 0 {
                    value.append(black_box(&c.text));
                } else {
                    value.edit::<OP>(c);
                }
                black_box(value.text());
                drop(value);
            }
        }
    }
}
fn run<'a, T: Make<'a>, const OP: usize>(
    cases: &[Case],
    budget: &'a MemoryBudget,
    count: u64,
) -> f64 {
    let mut sources: Vec<T> = cases
        .iter()
        .map(|c| {
            let mut s = T::make(c, budget);
            s.reserve(c.text.len());
            s.append(&c.text);
            s
        })
        .collect();
    // Independently compare each operation's full result before timing.
    for (c, s) in cases.iter().zip(sources.iter()) {
        let mut ours = if OP == 0 || OP == 6 || OP >= 9 {
            T::make(c, budget)
        } else {
            s.copy()
        };
        let mut model = if OP == 0 || OP == 6 || OP >= 9 {
            String::new()
        } else {
            c.text.clone()
        };
        if OP >= 9 {
            ours = ours.converted::<OP>(c);
            model = model.converted::<OP>(c);
        } else if OP == 0 || OP == 1 {
            ours.clear();
            ours.append(&c.text);
            model.clear();
            model.push_str(&c.text);
        } else {
            ours.edit::<OP>(c);
            model.edit::<OP>(c);
        }
        assert_eq!(ours.text(), model);
    }
    epochs::<T, OP>(cases, &mut sources, budget, 2);
    let start = Instant::now();
    epochs::<T, OP>(cases, &mut sources, budget, count);
    let elapsed = start.elapsed().as_secs_f64();
    drop(sources);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
    elapsed
}
macro_rules! dispatch {
    ($op:expr,$label:expr,$cases:expr,$b:expr,$n:expr) => {
        match $label {
            "std" => run::<String, $op>($cases, $b, $n),
            "unlimited" => run::<JString, $op>($cases, $b, $n),
            "limited" => run::<JString<Unmetered, LengthLimit>, $op>($cases, $b, $n),
            "metered" => run::<JString<Budget<'_>>, $op>($cases, $b, $n),
            "limited_metered" => run::<JString<Budget<'_>, LengthLimit>, $op>($cases, $b, $n),
            _ => panic!("unknown label"),
        }
    };
}
#[cfg(feature = "string-memory")]
fn memory_rows<'a, T: Make<'a>, const OP: usize>(
    cases: &[Case],
    budget: &'a MemoryBudget,
    label: &str,
) {
    for (index, c) in cases.iter().enumerate() {
        memory::start();
        let mut value = T::make(c, budget);
        if OP != 0 && OP != 6 && OP < 9 {
            value.reserve(c.text.len());
            value.append(&c.text);
        }
        if OP >= 9 {
            value = value.converted::<OP>(c);
        } else if OP == 0 {
            value.append(&c.text);
        } else if OP == 1 {
            value.clear();
            value.append(&c.text);
        } else {
            value.edit::<OP>(c);
        }
        let held = memory::snapshot();
        drop(value);
        let done = memory::stop();
        assert_eq!((done.live, done.actual), (0, 0));
        println!(
            "{{\"label\":\"{label}\",\"task\":\"{}\",\"case\":{index},\"input_bytes\":{},\"calls\":{},\"held_bytes\":{},\"peak_bytes\":{},\"held_actual\":{},\"peak_actual\":{},\"handle_bytes\":{}}}",
            TASKS[OP],
            c.text.len(),
            held.calls,
            held.live,
            held.peak,
            held.actual,
            held.actual_peak,
            std::mem::size_of::<T>()
        );
    }
}
#[cfg(feature = "string-memory")]
fn all_memory() {
    let cases = corpus(617, false);
    let budget = MemoryBudget::new(usize::MAX);
    macro_rules! labels {
        ($op:literal) => {
            memory_rows::<String, $op>(&cases, &budget, "std");
            memory_rows::<JString, $op>(&cases, &budget, "unlimited");
            memory_rows::<JString<Unmetered, LengthLimit>, $op>(&cases, &budget, "limited");
            memory_rows::<JString<Budget<'_>>, $op>(&cases, &budget, "metered");
            memory_rows::<JString<Budget<'_>, LengthLimit>, $op>(
                &cases,
                &budget,
                "limited_metered",
            );
        };
    }
    labels!(0);
    labels!(1);
    labels!(2);
    labels!(3);
    labels!(4);
    labels!(5);
    labels!(6);
    labels!(7);
    labels!(8);
    labels!(9);
    labels!(10);
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    #[cfg(feature = "string-memory")]
    if args.len() == 2 && args[1] == "memory" {
        all_memory();
        return;
    }
    if args.len() == 2 && args[1] == "tasks" {
        println!("{}", TASKS.join(","));
        return;
    }
    assert_eq!(args.len(), 6);
    let label = args[1].as_str();
    let task = TASKS.iter().position(|&x| x == args[2]).unwrap();
    let seed = args[3].parse().unwrap();
    let count: u64 = args[4].parse().unwrap();
    let small = args[5] == "small";
    assert!(small || args[5] == "mixed");
    let cases = corpus(seed, small);
    let budget = MemoryBudget::new(usize::MAX);
    let mut digest = 0xcbf29ce484222325u64;
    for c in &cases {
        for v in c
            .text
            .bytes()
            .map(u64::from)
            .chain([c.at as u64, c.end as u64, c.limit as u64])
        {
            digest = (digest ^ v).wrapping_mul(0x100000001b3);
        }
    }
    macro_rules! invoke{($($op:literal),*)=>{match task{$($op=>dispatch!($op,label,&cases,&budget,count),)*_=>unreachable!()}}}
    let elapsed = invoke!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
    let elements: usize = cases.iter().map(|c| c.text.len()).sum();
    println!(
        "{{\"elapsed_seconds\":{elapsed:.9},\"epochs\":{count},\"operations\":{},\"elements\":{},\"corpus_digest\":\"{digest:016x}\",\"min_size\":{},\"max_size\":{},\"cases\":64}}",
        count * 64,
        count * elements as u64,
        cases.iter().map(|c| c.text.len()).min().unwrap(),
        cases.iter().map(|c| c.text.len()).max().unwrap()
    );
}
