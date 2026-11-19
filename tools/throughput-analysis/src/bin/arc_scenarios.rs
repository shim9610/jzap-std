//! Application ownership kernels. Timing contains no input generation or clock reads.
#[cfg(feature = "shared-memory")]
#[path = "../string_memory.rs"]
mod memory;
use jzap::{
    TryClone,
    budget::{ConcurrentBudget, ConcurrentMemoryBudget, OwnedConcurrentBudget, Unmetered},
};
use std::{
    hint::black_box,
    sync::{Barrier, Mutex},
    time::Instant,
};

#[derive(Clone)]
struct Payload<const N: usize>([u64; N]);
impl<const N: usize> TryClone for Payload<N> {
    fn try_clone(&self) -> Result<Self, jzap::Error> {
        Ok(self.clone())
    }
}
trait Owner<T>: AsRef<T> + Send + Sync {
    fn duplicate(&self) -> Self;
    fn edit(&mut self) -> &mut T
    where
        T: Clone + TryClone;
}
impl<T: Send + Sync> Owner<T> for std::sync::Arc<T> {
    #[inline]
    fn duplicate(&self) -> Self {
        self.clone()
    }
    #[inline]
    fn edit(&mut self) -> &mut T
    where
        T: Clone + TryClone,
    {
        Self::make_mut(self)
    }
}
impl<T: Send + Sync, P: jzap::shared::SharedPolicy + Send + Sync> Owner<T>
    for jzap::sync::Arc<T, P>
{
    #[inline]
    fn duplicate(&self) -> Self {
        Self::try_clone(self).unwrap()
    }
    #[inline]
    fn edit(&mut self) -> &mut T
    where
        T: Clone + TryClone,
    {
        Self::try_make_mut(self).unwrap()
    }
}
trait Family: Sync + Sized {
    type Handle<T: Send + Sync>: Owner<T>;
    fn make<T: Send + Sync>(&self, value: T) -> Self::Handle<T>;
    fn make_from<const N: usize>(
        &self,
        source: &Payload<N>,
        case: &Case,
    ) -> Self::Handle<Payload<N>> {
        let mut value = source.clone();
        value.0[0] ^= case.delta;
        self.make(value)
    }
    fn fresh_request<const N: usize>(&self, source: &Payload<N>, case: &Case) -> u64 {
        request(self, source, case)
    }
}
struct Standard;
impl Family for Standard {
    type Handle<T: Send + Sync> = std::sync::Arc<T>;
    #[inline]
    fn make<T: Send + Sync>(&self, value: T) -> Self::Handle<T> {
        std::sync::Arc::new(value)
    }
}
struct Ours<P>(P);
impl<P: jzap::shared::SharedPolicy + Send + Sync> Family for Ours<P> {
    type Handle<T: Send + Sync> = jzap::sync::Arc<T, P>;
    #[inline]
    fn make<T: Send + Sync>(&self, value: T) -> Self::Handle<T> {
        jzap::sync::Arc::try_new_in(value, self.0.try_clone().unwrap())
            .map_err(|r| r.error)
            .unwrap()
    }
    #[inline]
    fn make_from<const N: usize>(
        &self,
        source: &Payload<N>,
        case: &Case,
    ) -> Self::Handle<Payload<N>> {
        initialized(source, case, self.0.try_clone().unwrap())
    }
}
struct Borrowed<'a>(ConcurrentBudget<'a>);
impl<'a> Family for Borrowed<'a> {
    type Handle<T: Send + Sync> = jzap::sync::Arc<T, ConcurrentBudget<'a>>;
    #[inline]
    fn make<T: Send + Sync>(&self, value: T) -> Self::Handle<T> {
        jzap::sync::Arc::try_new_in(value, self.0)
            .map_err(|r| r.error)
            .unwrap()
    }
    #[inline]
    fn make_from<const N: usize>(
        &self,
        source: &Payload<N>,
        case: &Case,
    ) -> Self::Handle<Payload<N>> {
        initialized(source, case, self.0)
    }
    #[inline]
    fn fresh_request<const N: usize>(&self, source: &Payload<N>, case: &Case) -> u64 {
        let ledger = ConcurrentMemoryBudget::new(usize::MAX);
        request(&Borrowed(ConcurrentBudget(&ledger)), source, case)
    }
}
struct Owned(OwnedConcurrentBudget);
impl Family for Owned {
    type Handle<T: Send + Sync> = jzap::sync::Arc<T, OwnedConcurrentBudget>;
    #[inline]
    fn make<T: Send + Sync>(&self, value: T) -> Self::Handle<T> {
        jzap::sync::Arc::try_new_in(value, self.0.try_clone().unwrap())
            .map_err(|r| r.error)
            .unwrap()
    }
    #[inline]
    fn make_from<const N: usize>(
        &self,
        source: &Payload<N>,
        case: &Case,
    ) -> Self::Handle<Payload<N>> {
        initialized(source, case, self.0.try_clone().unwrap())
    }
    #[inline]
    fn fresh_request<const N: usize>(&self, source: &Payload<N>, case: &Case) -> u64 {
        let fresh = Owned(OwnedConcurrentBudget::try_new(usize::MAX).unwrap());
        request(&fresh, source, case)
    }
}

#[derive(Clone, Copy)]
enum Kind {
    State(usize),
    Request,
    Fresh,
    Cow(usize),
    Locked,
    UniqueCow(usize),
    InitRequest,
}
#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    kind: Kind,
    workers: usize,
    bytes: usize,
}
const SCENARIOS: [Scenario; 19] = [
    Scenario {
        name: "arc_app_state8_t1",
        kind: Kind::State(8),
        workers: 1,
        bytes: 65536,
    },
    Scenario {
        name: "arc_app_state8_t2",
        kind: Kind::State(8),
        workers: 2,
        bytes: 65536,
    },
    Scenario {
        name: "arc_app_state8_t4",
        kind: Kind::State(8),
        workers: 4,
        bytes: 65536,
    },
    Scenario {
        name: "arc_app_state128_t1",
        kind: Kind::State(128),
        workers: 1,
        bytes: 65536,
    },
    Scenario {
        name: "arc_app_state128_t4",
        kind: Kind::State(128),
        workers: 4,
        bytes: 65536,
    },
    Scenario {
        name: "arc_app_request64_t1",
        kind: Kind::Request,
        workers: 1,
        bytes: 64,
    },
    Scenario {
        name: "arc_app_request64_t4",
        kind: Kind::Request,
        workers: 4,
        bytes: 64,
    },
    Scenario {
        name: "arc_app_request4096_t1",
        kind: Kind::Request,
        workers: 1,
        bytes: 4096,
    },
    Scenario {
        name: "arc_app_request4096_t4",
        kind: Kind::Request,
        workers: 4,
        bytes: 4096,
    },
    Scenario {
        name: "arc_app_domain64_t1",
        kind: Kind::Fresh,
        workers: 1,
        bytes: 64,
    },
    Scenario {
        name: "arc_app_domain64_t4",
        kind: Kind::Fresh,
        workers: 4,
        bytes: 64,
    },
    Scenario {
        name: "arc_app_cow4096_r1",
        kind: Kind::Cow(1),
        workers: 1,
        bytes: 4096,
    },
    Scenario {
        name: "arc_app_cow4096_r128",
        kind: Kind::Cow(128),
        workers: 1,
        bytes: 4096,
    },
    Scenario {
        name: "arc_app_cow65536_r128",
        kind: Kind::Cow(128),
        workers: 1,
        bytes: 65536,
    },
    Scenario {
        name: "arc_app_mutex_t1",
        kind: Kind::Locked,
        workers: 1,
        bytes: 16,
    },
    Scenario {
        name: "arc_app_mutex_t4",
        kind: Kind::Locked,
        workers: 4,
        bytes: 16,
    },
    Scenario {
        name: "arc_app_unique4096_r1",
        kind: Kind::UniqueCow(1),
        workers: 1,
        bytes: 4096,
    },
    Scenario {
        name: "arc_app_init4096_t1",
        kind: Kind::InitRequest,
        workers: 1,
        bytes: 4096,
    },
    Scenario {
        name: "arc_app_init4096_t4",
        kind: Kind::InitRequest,
        workers: 4,
        bytes: 4096,
    },
];
struct Case {
    indices: [usize; 128],
    delta: u64,
}
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
fn inputs<const N: usize>(seed: u64) -> (Payload<N>, Vec<Case>, u64) {
    let mut rng = seed | 1;
    let payload = Payload(std::array::from_fn(|_| next(&mut rng)));
    let cases = (0..64)
        .map(|_| Case {
            indices: std::array::from_fn(|_| next(&mut rng) as usize),
            delta: next(&mut rng),
        })
        .collect::<Vec<_>>();
    let mut digest = 0u64;
    for v in &payload.0 {
        digest = digest.rotate_left(7) ^ v;
    }
    for c in &cases {
        digest = digest.rotate_left(7) ^ c.delta;
        for &v in &c.indices {
            digest = digest.rotate_left(7) ^ v as u64;
        }
    }
    (payload, cases, digest)
}
#[inline(never)]
fn read<const N: usize>(payload: &Payload<N>, case: &Case, probes: usize) -> u64 {
    let payload = black_box(payload);
    let mut sum = case.delta;
    for &i in &case.indices[..probes] {
        sum = sum.wrapping_add(payload.0[i & (N - 1)]).rotate_left(3);
    }
    sum
}
#[inline]
fn request<F: Family, const N: usize>(family: &F, source: &Payload<N>, case: &Case) -> u64 {
    let mut value = source.clone();
    value.0[0] ^= case.delta;
    let root = black_box(family.make(value));
    let handles = std::array::from_fn::<_, 4, _>(|_| root.duplicate());
    let mut sum = 0u64;
    for h in &handles {
        sum = sum.wrapping_add(read(h.as_ref(), case, 8));
    }
    black_box(handles);
    sum
}
#[inline]
fn initialized<P: jzap::shared::SharedPolicy, const N: usize>(
    source: &Payload<N>,
    case: &Case,
    policy: P,
) -> jzap::sync::Arc<Payload<N>, P> {
    jzap::sync::Arc::try_init_in(policy, |slot: &mut jzap::init::InitSlot<'_, Payload<N>>| {
        let mut value = source.clone();
        value.0[0] ^= case.delta;
        slot.write(value).map_err(|r| r.error)
    })
    .unwrap()
}
#[inline]
fn request_init<F: Family, const N: usize>(family: &F, source: &Payload<N>, case: &Case) -> u64 {
    let root = black_box(family.make_from(source, case));
    let handles = std::array::from_fn::<_, 4, _>(|_| root.duplicate());
    let mut sum = 0u64;
    for h in &handles {
        sum = sum.wrapping_add(read(h.as_ref(), case, 8));
    }
    black_box(handles);
    sum
}
#[inline(never)]
fn epochs<F: Family, const N: usize>(
    family: &F,
    root: &mut F::Handle<Payload<N>>,
    source: &Payload<N>,
    cases: &[Case],
    kind: Kind,
    count: u64,
) -> u64 {
    let mut sum = 0u64;
    for _ in 0..count {
        for case in cases {
            let v = match kind {
                Kind::State(probes) => {
                    let view = black_box(root.duplicate());
                    read(view.as_ref(), case, probes)
                }
                Kind::Request => request(family, source, case),
                Kind::Fresh => family.fresh_request(source, case),
                Kind::InitRequest => request_init(family, source, case),
                Kind::Cow(readers) | Kind::UniqueCow(readers) => {
                    let old = if matches!(kind, Kind::Cow(_)) {
                        Some(black_box(root.duplicate()))
                    } else {
                        None
                    };
                    let at = case.indices[0] & (N - 1);
                    root.edit().0[at] ^= case.delta;
                    let mut result = old.as_ref().map(|v| read(v.as_ref(), case, 8)).unwrap_or(0);
                    for _ in 0..readers {
                        let view = black_box(root.duplicate());
                        result = result.wrapping_add(read(view.as_ref(), black_box(case), 8));
                    }
                    result
                }
                Kind::Locked => unreachable!(),
            };
            sum = sum.wrapping_add(v);
        }
    }
    black_box(sum)
}
fn allowed_cpus() -> Vec<usize> {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status
        .lines()
        .find(|s| s.starts_with("Cpus_allowed_list:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .split(',')
        .flat_map(|part| {
            let bounds: Vec<usize> = part.split('-').map(|n| n.parse().unwrap()).collect();
            bounds[0]..=*bounds.last().unwrap()
        })
        .collect()
}
fn pin_worker(cpu: usize) {
    unsafe extern "C" {
        fn sched_setaffinity(pid: i32, size: usize, mask: *const u64) -> i32;
    }
    let mut mask = [0u64; 16];
    mask[cpu / 64] |= 1u64 << (cpu % 64);
    // Linux pid 0 selects this calling thread. The initialized mask is live for
    // the call, has the supplied extent, and names an inherited allowed CPU.
    assert_eq!(
        unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) },
        0
    );
}
fn warmup(count: u64) -> u64 {
    2.max(count / 20)
}
fn run<F: Family, const N: usize>(family: &F, s: Scenario, seed: u64, count: u64) {
    let (payload, cases, digest) = inputs::<N>(seed);
    let mut global = Some(family.make(payload.clone()));
    let ready_gate = Barrier::new(s.workers + 1);
    let start_gate = Barrier::new(s.workers + 1);
    let end_gate = Barrier::new(s.workers + 1);
    let cpus = allowed_cpus();
    assert!(
        cpus.len() >= s.workers,
        "worker count exceeds selected CPU set"
    );
    let (seconds, checksum) = std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for &cpu in cpus.iter().take(s.workers) {
            let mut root = global.as_ref().unwrap().duplicate();
            let (source, cases, ready, start, end) =
                (&payload, &cases, &ready_gate, &start_gate, &end_gate);
            workers.push(scope.spawn(move || {
                pin_worker(cpu);
                epochs(family, &mut root, source, cases, s.kind, warmup(count));
                ready.wait();
                start.wait();
                let checksum = epochs(family, &mut root, source, cases, s.kind, count);
                end.wait();
                checksum
            }));
        }
        if matches!(s.kind, Kind::Cow(_) | Kind::UniqueCow(_)) {
            drop(global.take());
        }
        ready_gate.wait();
        let now = Instant::now();
        start_gate.wait();
        end_gate.wait();
        let seconds = now.elapsed().as_secs_f64();
        let checksum = workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .fold(0u64, u64::wrapping_add);
        (seconds, checksum)
    });
    drop(global);
    output(s, seed, count, seconds, checksum, digest);
}
fn locked<F: Family>(family: &F, s: Scenario, seed: u64, count: u64) {
    let (_, cases, digest) = inputs::<8>(seed);
    let global = family.make(Mutex::new(0u64));
    let ready_gate = Barrier::new(s.workers + 1);
    let start_gate = Barrier::new(s.workers + 1);
    let end_gate = Barrier::new(s.workers + 1);
    let cpus = allowed_cpus();
    assert!(
        cpus.len() >= s.workers,
        "worker count exceeds selected CPU set"
    );
    std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for &cpu in cpus.iter().take(s.workers) {
            let root = global.duplicate();
            let (cases, ready, start, end) = (&cases, &ready_gate, &start_gate, &end_gate);
            workers.push(scope.spawn(move || {
                pin_worker(cpu);
                let work = |n| {
                    for _ in 0..n {
                        for c in cases {
                            let view = black_box(root.duplicate());
                            let mut guard = view.as_ref().lock().unwrap();
                            *guard = guard.wrapping_add(c.delta);
                            black_box(*guard);
                        }
                    }
                };
                work(warmup(count));
                ready.wait();
                start.wait();
                work(count);
                end.wait();
            }));
        }
        ready_gate.wait();
        let now = Instant::now();
        start_gate.wait();
        end_gate.wait();
        let seconds = now.elapsed().as_secs_f64();
        for w in workers {
            w.join().unwrap();
        }
        let checksum = *global.as_ref().lock().unwrap();
        let per_epoch = cases.iter().fold(0u64, |n, c| n.wrapping_add(c.delta));
        assert_eq!(
            checksum,
            per_epoch.wrapping_mul((count + warmup(count)) * s.workers as u64)
        );
        output(s, seed, count, seconds, checksum, digest);
    });
}
fn output(s: Scenario, _: u64, count: u64, seconds: f64, checksum: u64, digest: u64) {
    let calls = count * 64 * s.workers as u64;
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let peak_rss: usize = status
        .lines()
        .find(|s| s.starts_with("VmHWM:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    println!(
        "{{\"elapsed_seconds\":{seconds:.9},\"epochs\":{count},\"operations\":{calls},\"elements\":{},\"corpus_digest\":\"{digest:016x}\",\"min_size\":{},\"max_size\":{},\"cases\":64,\"checksum\":{checksum},\"workers\":{},\"peak_rss_kib\":{peak_rss}}}",
        calls * s.bytes as u64,
        s.bytes,
        s.bytes,
        s.workers
    );
}
fn execute<F: Family>(family: &F, s: Scenario, seed: u64, count: u64) {
    if matches!(s.kind, Kind::Locked) {
        locked(family, s, seed, count);
    } else {
        match s.bytes {
            64 => run::<_, 8>(family, s, seed, count),
            4096 => run::<_, 512>(family, s, seed, count),
            65536 => run::<_, 8192>(family, s, seed, count),
            _ => unreachable!(),
        }
    }
}
#[cfg(feature = "shared-memory")]
fn memory_work<F: Family, const N: usize>(family: &F, count: usize, cow: bool) -> memory::Stats {
    let payload = Payload([13u64; N]);
    let mut objects: [Option<F::Handle<Payload<N>>>; 16] = std::array::from_fn(|_| None);
    for slot in &mut objects[..count] {
        *slot = Some(family.make(payload.clone()));
    }
    let old = if cow {
        let root = objects[0].as_mut().unwrap();
        let old = root.duplicate();
        root.edit().0[0] = 29;
        assert_eq!(old.as_ref().0[0], 13);
        assert_eq!(root.as_ref().0[0], 29);
        Some(old)
    } else {
        None
    };
    let held = memory::snapshot();
    drop(old);
    drop(objects);
    held
}
#[cfg(feature = "shared-memory")]
fn memory_cases() {
    for &(name, bytes, count, cow) in &[
        ("objects64_1", 64, 1, false),
        ("objects64_16", 64, 16, false),
        ("objects4096_1", 4096, 1, false),
        ("objects4096_16", 4096, 16, false),
        ("state65536_1", 65536, 1, false),
        ("cow4096", 4096, 1, true),
        ("cow65536", 65536, 1, true),
    ] {
        for label in ["std", "unlimited", "borrowed", "owned"] {
            let ledger = ConcurrentMemoryBudget::new(usize::MAX);
            memory::start();
            macro_rules! measure {
                ($family:expr) => {{
                    let family = $family;
                    match bytes {
                        64 => memory_work::<_, 8>(&family, count, cow),
                        4096 => memory_work::<_, 512>(&family, count, cow),
                        65536 => memory_work::<_, 8192>(&family, count, cow),
                        _ => unreachable!(),
                    }
                }};
            }
            let held = match label {
                "std" => measure!(Standard),
                "unlimited" => measure!(Ours(Unmetered)),
                "borrowed" => measure!(Borrowed(ConcurrentBudget(&ledger))),
                "owned" => measure!(Owned(OwnedConcurrentBudget::try_new(usize::MAX).unwrap())),
                _ => unreachable!(),
            };
            let end = memory::stop();
            assert_eq!((end.live, end.actual, ledger.used()), (0, 0, 0));
            println!(
                "{{\"scenario\":\"{name}\",\"label\":\"{label}\",\"objects\":{count},\"payload_bytes\":{bytes},\"requested_held\":{},\"requested_peak\":{},\"usable_held\":{},\"usable_peak\":{},\"allocations\":{},\"final_bytes\":0}}",
                held.live, end.peak, held.actual, end.actual_peak, end.calls
            );
        }
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    #[cfg(feature = "shared-memory")]
    if args.len() == 2 && args[1] == "memory" {
        memory_cases();
        return;
    }
    if args.len() == 2 && args[1] == "tasks" {
        println!("{}", SCENARIOS.map(|s| s.name).join(","));
        return;
    }
    if args.len() == 2 && args[1] == "describe" {
        println!(
            "{{\"operation_unit\":\"one request, locked update, or COW update/read cycle; all worker transactions counted\",\"inputs\":\"64 seeded index/delta cases and seeded payload; five input seeds; 1/2/4 workers; no network/disk I/O\",\"scenarios\":[{}]}}",
            SCENARIOS
                .iter()
                .map(|s| {
                    let reads = match s.kind {
                        Kind::State(n) | Kind::Cow(n) | Kind::UniqueCow(n) => n,
                        _ => 0,
                    };
                    format!(
                        "{{\"task\":\"{}\",\"workers\":{},\"payload_bytes\":{},\"reads\":{reads}}}",
                        s.name, s.workers, s.bytes
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        );
        return;
    }
    assert_eq!(args.len(), 6);
    let s = *SCENARIOS.iter().find(|s| s.name == args[2]).unwrap();
    let seed = args[3].parse().unwrap();
    let count = args[4].parse().unwrap();
    let ledger = ConcurrentMemoryBudget::new(usize::MAX);
    match args[1].as_str() {
        "std" => execute(&Standard, s, seed, count),
        "unlimited" => execute(&Ours(Unmetered), s, seed, count),
        "borrowed" => execute(&Borrowed(ConcurrentBudget(&ledger)), s, seed, count),
        "owned" => {
            let family = Owned(OwnedConcurrentBudget::try_new(usize::MAX).unwrap());
            execute(&family, s, seed, count);
            assert_eq!(family.0.used(), 0);
        }
        _ => unreachable!(),
    }
    assert_eq!(ledger.used(), 0);
}
