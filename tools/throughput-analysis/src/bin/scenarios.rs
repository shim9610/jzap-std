//! Mixed-collection scenarios: the same logical program written once against
//! std and once against jzap, with equal inputs, call order and checksums.
//! Input generation is outside timing. The input distributions are synthetic
//! and do not represent any application's call frequencies.
use jzap::{
    boxed::Box as JBox,
    budget::{
        AllocationPolicy, Budget, ConcurrentBudget, ConcurrentMemoryBudget, MemoryBudget, Unmetered,
    },
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque},
    string::String as JString,
    vec::{Unlimited, Vec as JVec},
};
use std::{
    cmp::Reverse,
    collections::{
        BTreeMap as StdTree, BTreeSet as StdSet, BinaryHeap as StdHeap, HashMap as StdMap,
        HashSet as StdHashSet, VecDeque as StdDeque,
    },
    hint::black_box,
    time::Instant,
};
const OPS: [&str; 5] = ["wordcount", "scheduler", "index", "graph", "request"];
/// Boxed reply of one request.
struct Reply {
    tokens: u64,
    unique: u64,
    longest: [u64; 5],
    lengths: u64,
}
const BATCH: usize = 8;
/// Boxed per-job state; the payload makes each job one 64-byte allocation.
struct Job {
    priority: u32,
    remaining: u32,
    ticks: u64,
    payload: [u64; 6],
}
const TOP: usize = 10;
struct Context {
    local: MemoryBudget,
    concurrent: ConcurrentMemoryBudget,
}
trait Policy<'a>: AllocationPolicy + Copy {
    fn choose(c: &'a Context) -> Self;
}
impl<'a> Policy<'a> for Unmetered {
    fn choose(_: &'a Context) -> Self {
        Self
    }
}
impl<'a> Policy<'a> for Budget<'a> {
    fn choose(c: &'a Context) -> Self {
        Self::Limited(&c.local)
    }
}
impl<'a> Policy<'a> for ConcurrentBudget<'a> {
    fn choose(c: &'a Context) -> Self {
        Self(&c.concurrent)
    }
}
fn random(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
struct Case {
    /// Space-separated tokens; `tokens` counts them.
    text: String,
    tokens: usize,
    /// (id, priority, quanta) in submission order; ids are distinct.
    jobs: Vec<(u64, u32, u32)>,
    /// Directed edges between node numbers below `tokens`.
    edges: Vec<(u32, u32)>,
}
fn corpus(mut seed: u64, small: bool) -> Vec<Case> {
    // A separate stream keeps the earlier scenarios' inputs unchanged.
    let mut edge_seed = seed ^ 0x9e37_79b9_7f4a_7c15;
    (0..64)
        .map(|c| {
            let n = match c % 16 {
                0 => 0,
                1 => 1,
                2 => {
                    if small {
                        32
                    } else {
                        4096
                    }
                }
                _ => {
                    let e = random(&mut seed) % if small { 6 } else { 13 };
                    (random(&mut seed) as usize % ((1usize << e).max(1))) + 1
                }
            };
            let vocabulary = (n / 4).max(1);
            let words = (0..vocabulary)
                .map(|i| {
                    let len = 3 + (random(&mut seed) % 10) as usize;
                    let mut w = format!("{i:x}");
                    while w.len() < len {
                        w.push((b'a' + (random(&mut seed) % 26) as u8) as char);
                    }
                    w
                })
                .collect::<Vec<_>>();
            let mut text = String::new();
            for i in 0..n {
                // Product of two uniform draws skews use toward low word indices.
                let a = random(&mut seed) as usize % vocabulary;
                let b = random(&mut seed) as usize % vocabulary;
                if i > 0 {
                    text.push(' ');
                }
                text.push_str(&words[a * b / vocabulary]);
            }
            let jobs = (0..n as u64)
                .map(|i| {
                    (
                        i.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1,
                        (random(&mut seed) % 64) as u32,
                        1 + (random(&mut seed) % 4) as u32,
                    )
                })
                .collect();
            let edges = (0..n * 3)
                .map(|_| {
                    (
                        (random(&mut edge_seed) % n as u64) as u32,
                        (random(&mut edge_seed) % n as u64) as u32,
                    )
                })
                .collect();
            Case {
                edges,
                text,
                tokens: n,
                jobs,
            }
        })
        .collect()
}
/// Count tokens in a map with owned string keys, select the TOP most frequent
/// with a bounded heap, sort them and join them into an output string.
fn wordcount_std(case: &Case) -> u64 {
    let mut counts = StdMap::<String, u64>::new();
    for token in case.text.split(' ').filter(|t| !t.is_empty()) {
        if let Some(count) = counts.get_mut(token) {
            *count += 1;
        } else {
            counts.insert(String::from(token), 1);
        }
    }
    let mut heap = StdHeap::<Reverse<(u64, &str)>>::new();
    for (word, count) in counts.iter() {
        heap.push(Reverse((*count, word.as_str())));
        if heap.len() > TOP {
            heap.pop();
        }
    }
    let ranked = heap.into_sorted_vec();
    let mut out = String::new();
    let mut sum = 0u64;
    for Reverse((count, word)) in ranked.as_slice() {
        out.push_str(word);
        out.push(' ');
        sum = sum.wrapping_mul(31).wrapping_add(count * word.len() as u64);
    }
    black_box(&out);
    sum.wrapping_add(out.len() as u64)
        .wrapping_add(counts.len() as u64)
}
fn wordcount_jzap<'a, P: Policy<'a>>(case: &Case, c: &'a Context) -> u64 {
    let mut counts = HashMap::<JString<P>, u64, _, P>::try_new_in(P::choose(c)).unwrap();
    for token in case.text.split(' ').filter(|t| !t.is_empty()) {
        if let Some(count) = counts.get_mut(token) {
            *count += 1;
        } else {
            let key = JString::try_from_str_in(token, P::choose(c), Unlimited).unwrap();
            counts.try_insert(key, 1).ok().unwrap();
        }
    }
    let mut heap = BinaryHeap::<Reverse<(u64, &str)>, P>::new_in(P::choose(c));
    for (word, count) in counts.iter() {
        heap.try_push(Reverse((*count, word.as_str())))
            .ok()
            .unwrap();
        if heap.len() > TOP {
            heap.pop();
        }
    }
    let ranked = heap.into_sorted_vec();
    let mut out = JString::<P>::new_in(P::choose(c));
    let mut sum = 0u64;
    for Reverse((count, word)) in ranked.as_slice() {
        out.try_push_str(word).unwrap();
        out.try_push(' ').unwrap();
        sum = sum.wrapping_mul(31).wrapping_add(count * word.len() as u64);
    }
    black_box(&out);
    sum.wrapping_add(out.as_str().len() as u64)
        .wrapping_add(counts.len() as u64)
}
/// Submit boxed jobs to an id map and a priority heap; repeatedly move the
/// highest-priority ids to a run queue in batches, run one quantum each, and
/// requeue unfinished jobs with a decayed priority or retire finished ones.
fn scheduler_std(case: &Case) -> u64 {
    let mut jobs = StdMap::<u64, Box<Job>>::new();
    let mut ready = StdHeap::<(u32, Reverse<u64>)>::new();
    let mut running = StdDeque::<u64>::new();
    for &(id, priority, quanta) in &case.jobs {
        let job = Box::new(Job {
            priority,
            remaining: quanta,
            ticks: 0,
            payload: [id; 6],
        });
        jobs.insert(id, job);
        ready.push((priority, Reverse(id)));
    }
    let mut sum = 0u64;
    let mut clock = 0u64;
    while !ready.is_empty() {
        while running.len() < BATCH {
            let Some((_, Reverse(id))) = ready.pop() else {
                break;
            };
            running.push_back(id);
        }
        while let Some(id) = running.pop_front() {
            clock += 1;
            let job = jobs.get_mut(&id).unwrap();
            job.remaining -= 1;
            job.ticks += clock;
            if job.remaining > 0 {
                job.priority = job.priority.saturating_sub(1);
                ready.push((job.priority, Reverse(id)));
            } else {
                let done = jobs.remove(&id).unwrap();
                sum = sum
                    .wrapping_mul(31)
                    .wrapping_add(done.ticks ^ done.payload[5]);
            }
        }
    }
    sum.wrapping_add(jobs.len() as u64)
}
fn scheduler_jzap<'a, P: Policy<'a>>(case: &Case, c: &'a Context) -> u64 {
    let mut jobs = HashMap::<u64, JBox<Job, P>, _, P>::try_new_in(P::choose(c)).unwrap();
    let mut ready = BinaryHeap::<(u32, Reverse<u64>), P>::new_in(P::choose(c));
    let mut running = VecDeque::<u64, P>::new_in(P::choose(c));
    for &(id, priority, quanta) in &case.jobs {
        let job = JBox::try_new_in(
            Job {
                priority,
                remaining: quanta,
                ticks: 0,
                payload: [id; 6],
            },
            P::choose(c),
        )
        .ok()
        .unwrap();
        jobs.try_insert(id, job).ok().unwrap();
        ready.try_push((priority, Reverse(id))).ok().unwrap();
    }
    let mut sum = 0u64;
    let mut clock = 0u64;
    while !ready.is_empty() {
        while running.len() < BATCH {
            let Some((_, Reverse(id))) = ready.pop() else {
                break;
            };
            running.try_push_back(id).ok().unwrap();
        }
        while let Some(id) = running.pop_front() {
            clock += 1;
            let job = jobs.get_mut(&id).unwrap();
            job.remaining -= 1;
            job.ticks += clock;
            if job.remaining > 0 {
                job.priority = job.priority.saturating_sub(1);
                ready.try_push((job.priority, Reverse(id))).ok().unwrap();
            } else {
                let done = jobs.remove(&id).unwrap();
                sum = sum
                    .wrapping_mul(31)
                    .wrapping_add(done.ticks ^ done.payload[5]);
            }
        }
    }
    sum.wrapping_add(jobs.len() as u64)
}
/// Build an ordered index and two overlapping tag sets from the job ids, run
/// range queries into a result vector, intersect and unite the tag sets, then
/// remove the intersected keys and scan what remains.
fn index_std(case: &Case) -> u64 {
    let n = case.jobs.len() as u64;
    let width = (u64::MAX / n.max(1)).saturating_mul(8);
    let mut index = StdTree::<u64, u32>::new();
    let mut a = StdSet::<u64>::new();
    let mut b = StdSet::<u64>::new();
    for (i, &(key, priority, quanta)) in case.jobs.iter().enumerate() {
        index.insert(key, i as u32);
        if priority % 3 != 0 {
            a.insert(key);
        }
        if quanta != 1 {
            b.insert(key);
        }
    }
    let mut sum = 0u64;
    let mut hits = Vec::<u32>::new();
    for &(low, _, _) in case.jobs.iter().step_by(16) {
        hits.clear();
        for (_, row) in index.range(low..low.saturating_add(width)) {
            hits.push(*row);
        }
        sum = sum.wrapping_mul(31).wrapping_add(hits.len() as u64);
        sum = sum.wrapping_add(hits.iter().map(|x| u64::from(*x)).sum::<u64>());
    }
    let mut both = Vec::<u64>::new();
    for key in a.intersection(&b) {
        both.push(*key);
    }
    sum = sum.wrapping_add(a.union(&b).count() as u64);
    for key in &both {
        sum = sum.wrapping_add(u64::from(index.remove(key).unwrap()));
    }
    for (key, row) in index.iter() {
        sum = sum.wrapping_mul(31).wrapping_add(key ^ u64::from(*row));
    }
    sum.wrapping_add(both.len() as u64)
}
fn index_jzap<'a, P: Policy<'a>>(case: &Case, c: &'a Context) -> u64 {
    let n = case.jobs.len() as u64;
    let width = (u64::MAX / n.max(1)).saturating_mul(8);
    let mut index = BTreeMap::<u64, u32, P>::new_in(P::choose(c));
    let mut a = BTreeSet::<u64, P>::new_in(P::choose(c));
    let mut b = BTreeSet::<u64, P>::new_in(P::choose(c));
    for (i, &(key, priority, quanta)) in case.jobs.iter().enumerate() {
        index.try_insert(key, i as u32).ok().unwrap();
        if priority % 3 != 0 {
            a.try_insert(key).ok().unwrap();
        }
        if quanta != 1 {
            b.try_insert(key).ok().unwrap();
        }
    }
    let mut sum = 0u64;
    let mut hits = JVec::<u32, P>::new_in(P::choose(c));
    for &(low, _, _) in case.jobs.iter().step_by(16) {
        hits.clear();
        for (_, row) in index.try_range(low..low.saturating_add(width)).unwrap() {
            hits.try_push(*row).ok().unwrap();
        }
        sum = sum.wrapping_mul(31).wrapping_add(hits.len() as u64);
        sum = sum.wrapping_add(hits.as_slice().iter().map(|x| u64::from(*x)).sum::<u64>());
    }
    let mut both = JVec::<u64, P>::new_in(P::choose(c));
    for key in a.intersection(&b) {
        both.try_push(*key).ok().unwrap();
    }
    sum = sum.wrapping_add(a.union(&b).count() as u64);
    for key in both.as_slice() {
        sum = sum.wrapping_add(u64::from(index.remove(key).unwrap()));
    }
    for (key, row) in index.iter() {
        sum = sum.wrapping_mul(31).wrapping_add(key ^ u64::from(*row));
    }
    sum.wrapping_add(both.len() as u64)
}
/// Build per-node adjacency vectors from the edge list, then run a breadth-
/// first search from node 0 with a queue and a visited hash set, recording
/// the visit order in a vector.
fn graph_std(case: &Case) -> u64 {
    let n = case.tokens;
    let mut adjacency = Vec::<Vec<u32>>::new();
    for _ in 0..n {
        adjacency.push(Vec::new());
    }
    for &(from, to) in &case.edges {
        adjacency[from as usize].push(to);
    }
    let mut visited = StdHashSet::<u32>::new();
    let mut queue = StdDeque::<u32>::new();
    let mut order = Vec::<u32>::new();
    if n > 0 {
        visited.insert(0);
        queue.push_back(0);
    }
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for &next in adjacency[node as usize].as_slice() {
            if visited.insert(next) {
                queue.push_back(next);
            }
        }
    }
    let mut sum = visited.len() as u64;
    for node in order.as_slice() {
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(*node));
    }
    sum
}
fn graph_jzap<'a, P: Policy<'a>>(case: &Case, c: &'a Context) -> u64 {
    let n = case.tokens;
    let mut adjacency = JVec::<JVec<u32, P>, P>::new_in(P::choose(c));
    for _ in 0..n {
        adjacency.try_push(JVec::new_in(P::choose(c))).ok().unwrap();
    }
    for &(from, to) in &case.edges {
        adjacency
            .get_mut(from as usize)
            .unwrap()
            .try_push(to)
            .ok()
            .unwrap();
    }
    let mut visited = HashSet::<u32, _, P>::try_new_in(P::choose(c)).unwrap();
    let mut queue = VecDeque::<u32, P>::new_in(P::choose(c));
    let mut order = JVec::<u32, P>::new_in(P::choose(c));
    if n > 0 {
        visited.try_insert(0).ok().unwrap();
        queue.try_push_back(0).ok().unwrap();
    }
    while let Some(node) = queue.pop_front() {
        order.try_push(node).ok().unwrap();
        for &next in adjacency.get(node as usize).unwrap().as_slice() {
            if visited.try_insert(next).ok().unwrap() {
                queue.try_push_back(next).ok().unwrap();
            }
        }
    }
    let mut sum = visited.len() as u64;
    for node in order.as_slice() {
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(*node));
    }
    sum
}
/// One short-lived request touching every collection: copy each token into an
/// owned string, deduplicate through a borrowed-key hash set, count token
/// lengths in an ordered map, pass unique tokens through a queue into a heap,
/// take the five longest and box the reply. Everything is dropped at the end.
fn request_std(case: &Case) -> u64 {
    let mut tokens = Vec::<String>::new();
    for token in case.text.split(' ').filter(|t| !t.is_empty()) {
        tokens.push(String::from(token));
    }
    let mut seen = StdHashSet::<&str>::new();
    let mut lengths = StdTree::<usize, u32>::new();
    let mut queue = StdDeque::<usize>::new();
    for (i, token) in tokens.iter().enumerate() {
        if seen.insert(token.as_str()) {
            queue.push_back(i);
        }
        if let Some(count) = lengths.get_mut(&token.len()) {
            *count += 1;
        } else {
            lengths.insert(token.len(), 1);
        }
    }
    let mut heap = StdHeap::<(usize, Reverse<usize>)>::new();
    while let Some(i) = queue.pop_front() {
        heap.push((tokens[i].len(), Reverse(i)));
    }
    let mut longest = [0u64; 5];
    for slot in &mut longest {
        let Some((len, Reverse(i))) = heap.pop() else {
            break;
        };
        *slot = (len as u64) << 32 | i as u64;
    }
    let reply = Box::new(Reply {
        tokens: tokens.len() as u64,
        unique: seen.len() as u64,
        longest,
        lengths: lengths
            .iter()
            .fold(0, |s, (k, v)| s * 31 + (*k as u64 ^ u64::from(*v))),
    });
    black_box(&reply);
    reply
        .longest
        .iter()
        .fold(reply.tokens ^ reply.unique << 20 ^ reply.lengths, |s, x| {
            s.wrapping_mul(31).wrapping_add(*x)
        })
}
fn request_jzap<'a, P: Policy<'a>>(case: &Case, c: &'a Context) -> u64 {
    let mut tokens = JVec::<JString<P>, P>::new_in(P::choose(c));
    for token in case.text.split(' ').filter(|t| !t.is_empty()) {
        tokens
            .try_push(JString::try_from_str_in(token, P::choose(c), Unlimited).unwrap())
            .ok()
            .unwrap();
    }
    let mut seen = HashSet::<&str, _, P>::try_new_in(P::choose(c)).unwrap();
    let mut lengths = BTreeMap::<usize, u32, P>::new_in(P::choose(c));
    let mut queue = VecDeque::<usize, P>::new_in(P::choose(c));
    for (i, token) in tokens.as_slice().iter().enumerate() {
        if seen.try_insert(token.as_str()).ok().unwrap() {
            queue.try_push_back(i).ok().unwrap();
        }
        let len = token.as_str().len();
        if let Some(count) = lengths.get_mut(&len) {
            *count += 1;
        } else {
            lengths.try_insert(len, 1).ok().unwrap();
        }
    }
    let mut heap = BinaryHeap::<(usize, Reverse<usize>), P>::new_in(P::choose(c));
    while let Some(i) = queue.pop_front() {
        let len = tokens.get(i).unwrap().as_str().len();
        heap.try_push((len, Reverse(i))).ok().unwrap();
    }
    let mut longest = [0u64; 5];
    for slot in &mut longest {
        let Some((len, Reverse(i))) = heap.pop() else {
            break;
        };
        *slot = (len as u64) << 32 | i as u64;
    }
    let reply = JBox::try_new_in(
        Reply {
            tokens: tokens.len() as u64,
            unique: seen.len() as u64,
            longest,
            lengths: lengths
                .iter()
                .fold(0, |s, (k, v)| s * 31 + (*k as u64 ^ u64::from(*v))),
        },
        P::choose(c),
    )
    .ok()
    .unwrap();
    black_box(&reply);
    reply
        .longest
        .iter()
        .fold(reply.tokens ^ reply.unique << 20 ^ reply.lengths, |s, x| {
            s.wrapping_mul(31).wrapping_add(*x)
        })
}
fn timed(cases: &[Case], n: u64, mut one: impl FnMut(&Case) -> u64) -> (f64, u64) {
    let mut epoch = |count: u64| {
        let mut sum = 0u64;
        for _ in 0..count {
            for case in cases {
                sum = sum.wrapping_add(black_box(one(black_box(case))));
            }
        }
        sum
    };
    black_box(epoch(2));
    let start = Instant::now();
    let sum = epoch(n);
    (start.elapsed().as_secs_f64(), sum)
}
fn run(label: &str, op: usize, cases: &[Case], c: &Context, n: u64) -> (f64, u64) {
    match (op, label) {
        (0, "std") => timed(cases, n, wordcount_std),
        (0, "unlimited") => timed(cases, n, |x| wordcount_jzap::<Unmetered>(x, c)),
        (0, "local") => timed(cases, n, |x| wordcount_jzap::<Budget<'_>>(x, c)),
        (0, "concurrent") => timed(cases, n, |x| wordcount_jzap::<ConcurrentBudget<'_>>(x, c)),
        (1, "std") => timed(cases, n, scheduler_std),
        (1, "unlimited") => timed(cases, n, |x| scheduler_jzap::<Unmetered>(x, c)),
        (1, "local") => timed(cases, n, |x| scheduler_jzap::<Budget<'_>>(x, c)),
        (1, "concurrent") => timed(cases, n, |x| scheduler_jzap::<ConcurrentBudget<'_>>(x, c)),
        (2, "std") => timed(cases, n, index_std),
        (2, "unlimited") => timed(cases, n, |x| index_jzap::<Unmetered>(x, c)),
        (2, "local") => timed(cases, n, |x| index_jzap::<Budget<'_>>(x, c)),
        (2, "concurrent") => timed(cases, n, |x| index_jzap::<ConcurrentBudget<'_>>(x, c)),
        (3, "std") => timed(cases, n, graph_std),
        (3, "unlimited") => timed(cases, n, |x| graph_jzap::<Unmetered>(x, c)),
        (3, "local") => timed(cases, n, |x| graph_jzap::<Budget<'_>>(x, c)),
        (3, "concurrent") => timed(cases, n, |x| graph_jzap::<ConcurrentBudget<'_>>(x, c)),
        (4, "std") => timed(cases, n, request_std),
        (4, "unlimited") => timed(cases, n, |x| request_jzap::<Unmetered>(x, c)),
        (4, "local") => timed(cases, n, |x| request_jzap::<Budget<'_>>(x, c)),
        (4, "concurrent") => timed(cases, n, |x| request_jzap::<ConcurrentBudget<'_>>(x, c)),
        _ => unreachable!(),
    }
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|s| s == "tasks") {
        println!("{}", OPS.map(|s| format!("scenario_{s}")).join(","));
        return;
    }
    if args.get(1).is_some_and(|s| s == "describe") {
        println!(
            "{{\"operation_unit\":\"One complete scenario over one input case; an epoch runs all 64 cases. All collections are created and dropped inside the timed scenario.\",\"inputs\":\"64 synthetic cases per seed; sizes zero/one/maximum and log-uniform up to 4096 (small: 32). wordcount: space-separated tokens from a vocabulary of size/4 words of 3..12 bytes, skewed toward low indices; owned string keys in a default-hashed map, bounded top-10 heap, sorted output joined into a string. scheduler: size jobs with distinct ids, priority 0..63 and 1..4 quanta; 64-byte boxed job state in an id map, a priority heap and a run queue filled in batches of 8; unfinished jobs are requeued with priority-1, finished jobs are removed and dropped. index: the same job ids as keys of an ordered map and of two overlapping ordered sets (about 2/3 and 3/4 of the keys); one range query per 16 keys sized for about 8 hits collected into a reused vector; set intersection collected into a vector, union counted; intersected keys removed from the map; remaining entries scanned. graph: size nodes and 3*size random directed edges; per-node adjacency vectors grown by push, breadth-first search from node 0 with a queue, a default-hashed visited set and a visit-order vector. request: the wordcount text; every token copied into an owned string in a vector, deduplicated through a borrowed-key hash set, token lengths counted in an ordered map, unique tokens passed through a queue into a heap, five longest popped, reply boxed; all dropped per request.\"}}"
        );
        return;
    }
    assert_eq!(args.len(), 6);
    let name = args[2].strip_prefix("scenario_").unwrap();
    let op = OPS.iter().position(|s| *s == name).unwrap();
    let seed: u64 = args[3].parse().unwrap();
    let n: u64 = args[4].parse().unwrap();
    let cases = corpus(seed, args[5] == "small");
    let c = Context {
        local: MemoryBudget::new(usize::MAX),
        concurrent: ConcurrentMemoryBudget::new(usize::MAX),
    };
    let (elapsed, checksum) = run(&args[1], op, &cases, &c, n);
    assert_eq!(
        (c.local.occupied(), c.local.reserved(), c.concurrent.used()),
        (0, 0, 0)
    );
    let mut digest = 0xcbf29ce484222325u64;
    for case in &cases {
        for b in case.text.bytes() {
            digest = (digest ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
        for &(from, to) in &case.edges {
            digest = (digest ^ u64::from(from) ^ (u64::from(to) << 32)).wrapping_mul(0x100000001b3);
        }
        for &(id, priority, quanta) in &case.jobs {
            digest = (digest ^ id ^ (u64::from(priority) << 8) ^ u64::from(quanta))
                .wrapping_mul(0x100000001b3);
        }
    }
    println!(
        "{{\"epochs\":{n},\"cases\":64,\"operations\":{},\"elements\":{},\"min_size\":{},\"max_size\":{},\"corpus_digest\":{digest},\"checksum\":{checksum},\"elapsed_seconds\":{elapsed}}}",
        n * 64,
        n * cases.iter().map(|c| c.tokens as u64).sum::<u64>(),
        cases.iter().map(|c| c.tokens).min().unwrap(),
        cases.iter().map(|c| c.tokens).max().unwrap()
    );
}
