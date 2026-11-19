//! Long-interval homogeneous collection workloads. No RNG or clock in the inner loop.
mod boxed;
mod corpus;
mod vector;
use corpus::{Corpus, TASKS};
use jzap::{
    budget::{Budget, MemoryBudget, Unmetered},
    vec::{LengthLimit, Unlimited, Vec as JVec},
};
use std::time::Instant;
use vector::NewOwner;
#[cfg(test)]
use vector::Owner;

fn output(corpus: &Corpus, count: u64, elapsed: f64) {
    println!(
        "{{\"elapsed_seconds\":{elapsed:.9},\"epochs\":{count},\"operations\":{},\"elements\":{},\"corpus_digest\":\"{:016x}\",\"min_size\":{},\"max_size\":{},\"cases\":{}}}",
        count * corpus.cases.len() as u64,
        count * corpus.elements as u64,
        corpus.digest,
        corpus.min,
        corpus.max,
        corpus.cases.len()
    );
}
fn run_vec<'a, T: NewOwner<'a>, const OP: usize>(
    corpus: &Corpus,
    count: u64,
    budget: &'a MemoryBudget,
) {
    let mut sources: Vec<T> = corpus
        .cases
        .iter()
        .map(|c| {
            let mut source = T::new(c, budget);
            source.reserve(c.input.len());
            source.copy(&c.input);
            source
        })
        .collect();
    vector::epochs::<T, OP>(&corpus.cases, &mut sources, budget, 2);
    let start = Instant::now();
    vector::epochs::<T, OP>(&corpus.cases, &mut sources, budget, count);
    let elapsed = start.elapsed().as_secs_f64();
    for (source, c) in sources.iter().zip(&corpus.cases) {
        assert_eq!(source.slice(), c.input);
    }
    drop(sources);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
    output(corpus, count, elapsed);
}
fn run_boxes<B: boxed::Boxes, const OP: usize>(boxes: B, corpus: &Corpus, count: u64) {
    let mut sources = boxed::prepare(&boxes, &corpus.cases);
    boxed::epochs::<B, OP>(&boxes, &corpus.cases, &mut sources, 2);
    let start = Instant::now();
    boxed::epochs::<B, OP>(&boxes, &corpus.cases, &mut sources, count);
    let elapsed = start.elapsed().as_secs_f64();
    drop(sources);
    output(corpus, count, elapsed);
}
macro_rules! vector_task {
    ($op:expr, $label:expr, $corpus:expr, $count:expr, $budget:expr) => {
        match $label {
            "std" => run_vec::<Vec<u64>, $op>($corpus, $count, $budget),
            "unlimited" => run_vec::<JVec<u64>, $op>($corpus, $count, $budget),
            "limited" => {
                run_vec::<JVec<u64, Unmetered, LengthLimit>, $op>($corpus, $count, $budget)
            }
            "metered" => run_vec::<JVec<u64, Budget<'_>, Unlimited>, $op>($corpus, $count, $budget),
            "limited_metered" => {
                run_vec::<JVec<u64, Budget<'_>, LengthLimit>, $op>($corpus, $count, $budget)
            }
            _ => panic!("unknown label"),
        }
    };
}
macro_rules! box_task {
    ($op:expr, $label:expr, $corpus:expr, $count:expr, $budget:expr) => {
        match $label {
            "std" => run_boxes::<_, $op>(boxed::Standard, $corpus, $count),
            "unlimited" => run_boxes::<_, $op>(boxed::Ours(Unmetered), $corpus, $count),
            "metered" => {
                run_boxes::<_, $op>(boxed::Ours(Budget::Limited($budget)), $corpus, $count)
            }
            _ => panic!("unknown Box label"),
        }
    };
}
macro_rules! dispatch {
    ($task:expr, $label:expr, $corpus:expr, $count:expr, $budget:expr; $($vec:literal),*; $($boxed:literal),*) => {
        match $task {
            $($vec => vector_task!($vec, $label, $corpus, $count, $budget),)*
            $($boxed => box_task!($boxed, $label, $corpus, $count, $budget),)*
            _ => unreachable!(),
        }
    };
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 2 && args[1] == "tasks" {
        println!("{}", TASKS.join(","));
        return;
    }
    assert_eq!(args.len(), 6, "label task seed epochs mixed|small");
    let label = args[1].as_str();
    let task = TASKS
        .iter()
        .position(|x| *x == args[2])
        .expect("unknown task");
    let corpus = corpus::generate(args[3].parse().unwrap(), task, &args[5]);
    let count = args[4].parse().unwrap();
    let budget = MemoryBudget::new(usize::MAX);
    dispatch!(task, label, &corpus, count, &budget;
        0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19;
        20,21,22,23,24,25,26,27,28,29,30);
    assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn compare<'a, T: NewOwner<'a>, const OP: usize>(corpus: &Corpus, budget: &'a MemoryBudget) {
        for c in &corpus.cases {
            let mut standard = c.input.clone();
            let mut ours = T::new(c, budget);
            ours.copy(&c.input);
            let expected = standard.edit::<OP, true>(c);
            let actual = ours.edit::<OP, true>(c);
            assert_eq!(ours.slice(), standard);
            assert_eq!(actual, expected);
        }
    }
    #[test]
    fn random_edits_match_std_including_removed_order_for_every_policy() {
        macro_rules! check { ($($op:literal),*) => { $(
            for seed in [617, 6203, 96011] {
                let corpus = corpus::generate(seed, $op, "small");
                let budget = MemoryBudget::new(usize::MAX);
                compare::<JVec<u64>, $op>(&corpus, &budget);
                compare::<JVec<u64, Unmetered, LengthLimit>, $op>(&corpus, &budget);
                compare::<JVec<u64, Budget<'_>>, $op>(&corpus, &budget);
                compare::<JVec<u64, Budget<'_>, LengthLimit>, $op>(&corpus, &budget);
                assert_eq!((budget.occupied(), budget.reserved()), (0,0));
            }
        )* }; }
        check!(4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19);
    }
    #[test]
    fn generated_box_operations_match_values_lengths_and_zero_initialization() {
        for seed in [617, 6203, 96011] {
            let corpus = corpus::generate(seed, 23, "small");
            let budget = MemoryBudget::new(usize::MAX);
            boxed::verify(&boxed::Standard, &corpus.cases);
            boxed::verify(&boxed::Ours(Unmetered), &corpus.cases);
            boxed::verify(&boxed::Ours(Budget::Limited(&budget)), &corpus.cases);
            assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
        }
    }
    #[test]
    fn corpus_is_replayable_and_contains_boundaries() {
        for profile in ["small", "mixed"] {
            for task in 0..TASKS.len() {
                let a = corpus::generate(617, task, profile);
                let b = corpus::generate(617, task, profile);
                assert_eq!(a.digest, b.digest);
                assert_eq!(a.cases.len(), 64);
                if !(20..23).contains(&task) {
                    assert_eq!(a.min, 0);
                }
                assert_ne!(a.digest, corpus::generate(6203, task, profile).digest);
            }
        }
    }
}
