# Arc application-workflow evaluation

This evaluation measures complete ownership workflows using std and the public
jzap API. It supplements the isolated constructor/clone/COW measurements. It is
an executable application-kernel model, not an HTTP, GUI frame or storage-engine
end-to-end benchmark. Workload frequencies below are explicit scenario inputs,
not claimed production frequency estimates.

## Workloads and criteria fixed before measurement

| Workflow | Work per transaction | Conditions |
| --- | --- | --- |
| Long-lived shared state | Clone a shared 64KiB immutable state, perform indexed reads, release the handle | 8 reads with 1/2/4 workers; 128 reads with 1/4 workers |
| Request object with fanout | Allocate a payload, duplicate four handles, read through each, release all handles | 64-byte and 4KiB payloads; 1/4 workers sharing an application budget |
| Request-local budget | Same 64-byte request object; construct and retire the budget domain inside each request | 1/4 workers; borrowed budget uses a fresh stack ledger, owned uses a fresh heap ledger |
| Versioned page update | Retain an old version, COW a page, change one word, read the new version, retire the old version | 4KiB page with 1 or 128 reads per update; 64KiB page with 128 reads |
| Locked shared state | Clone an Arc, lock the same Mutex, update a word, unlock and release | 1/4 workers; mutation uses a lock rather than COW |

The initial campaign has sixteen conditions. Three follow-up conditions cover
the uniquely owned 4KiB COW fast path and 1/4-worker 4KiB request construction
through the existing `try_init_in` API. The constructor comparison tests whether
moving construction after admission removes an observed extra 4KiB stack copy;
it retains the original by-value measurements and the same +15% criterion.
Read indices and payload words are generated from
the seed before timing. Thread workers are prepared before timing, start behind
a barrier, and complete fixed work before the timer stops. Each worker is pinned
to its own selected CPU and warms up with `max(2, epochs / 20)` epochs. The
initial unpinned campaign exceeded the predefined identical-std drift criterion
in four-worker runs and was interrupted; it is not an acceptance result.
Worker creation and
joining are excluded. All workers' work is included in wall time; CPU time and
the process RSS high-water mark are also recorded. Threaded runs use an explicit
four-CPU affinity set, rather than the older driver's single-CPU default.

The comparisons are std, repeated identical std, unmetered jzap, borrowed
concurrent budget and owning concurrent budget. Five independent seeded runs
use equal transaction counts and inputs. Every accepted interval must last at
least one second. Means use summed seconds divided by summed transactions.
The existing +15% time and allocation criteria remain the comparison thresholds.
Large identical-std drift triggers a repeat before a stable judgment is made.
Ratios and fixed ns/transaction costs are reported together.

Memory probes separately record requested and allocator-usable held/peak bytes,
allocation counts and final cleanup. They include owning-domain bootstrap
storage. A one-object and sixteen-object in-flight working set exposes whether
the once-per-domain allocation is amortized. Raw logs and generated data remain
under ignored `target/verification/arc-scenarios/`.

## Source basis

The model follows observed ownership patterns without importing consumer code
or adding product dependencies:

- Varlit checkpoint `ba541cc3082cecdf7f385d06897a58a3889e1294`:
  `crates/varlit-core/src/snapshot.rs` clones an Arc-backed file;
  `matrix_generation.rs` caches Arc-backed pages;
  `matrix_directory.rs::prepare_cache_update` separates retained 4KiB pages
  with `Arc::make_mut` before authoritative writes.

## Decision

The measured cost depends on the ownership workflow. Long-lived state with
substantial reads and shared page COW can be practical. High-frequency cloning
of the same Arc and high-rate allocation through one global concurrent budget
remain material bottlenecks. The earlier approximately equal isolated clone
measurements were single-threaded and do not establish equivalence under
contention.

| Use | Owning concurrent observation | Assessment against the existing +15% time criterion |
| --- | --- | --- |
| Long-lived state, 128 indexed reads/request | −2.1% with one worker, −1.4% with four | Within; paired intervals also remain within +15% |
| Very short state access, same Arc cloned by 2/4 workers | +44.5% / +28.5% | Above; unsuitable as an equal-speed replacement for this hot path |
| Shared 4KiB page COW | +9.1% with one read; +12.7% with 128 reads | First condition within; second is borderline (upper interval +15.9%) |
| Shared 64KiB page COW, 128 reads/update | +4.3% | Within |
| Unique 4KiB page edit/read | 7.759 → 9.789 ns, +26.2% | Percentage criterion exceeded; approximately 2.03 ns fixed cost without allocation or copying |
| Locked shared state | +0.8% with one worker, +12.9% with four | One worker within; four workers borderline (upper interval +17.5%) |
| Per-request 64-byte owner, shared budget, 1/4 workers | +50.1% / +313.7% | Above; high-rate small-object creation needs improvement |
| Per-request 64-byte owner and fresh budget, 1/4 workers | +86.2% / +73.3% | Above; private ledgers reduce contention but add a domain allocation |
| Per-request 4KiB owner, shared budget, by-value construction | +68.9% / +126.1% | Above; this caller retains an extra payload copy |
| Same 4KiB request using `try_init_in` | +10.9% / +51.5% | One worker borderline (upper interval +15.5%); four workers above |

These assessments concern the specified application kernels. The multi-worker
ns/transaction values are inverse aggregate throughput, not individual request
latency or a server response-time percentile.

## Results

The primary campaign at `bf763049` contains 400 valid intervals; the three
follow-up conditions at `a9aa4a1` contain 75. Every interval is at least
1.003 seconds. The [76 aggregate rows](arc-scenario-benchmarks.csv) preserve both
campaigns, all product/control labels, totals, operation counts, seed spread,
process CPU time and RSS ranges. Identical-std mean differences stay between
−9.0% and +6.0%, within the predeclared repeat threshold. The initial unpinned
campaign is excluded from these totals.

The intervals in the tables use 10,000 deterministic paired seed bootstrap
resamples (seed 17291) of the elapsed-time ratio. They describe variation across
five seeded process runs; they are not per-request latency intervals. Means
below +15% whose upper interval crosses +15% remain borderline.

The machine exposes AMD EPYC 9V74 CPUs. Workers use CPUs 0–3, one per worker;
the container CPU quota is four cores. Rust 1.95.0/LLVM 22.1.2 builds use release
optimization, one codegen unit and `panic=abort`, without loop-alignment flags.
Compilation, code generation and proofs finish before timing starts. Process
CPU time includes setup and warmup; RSS is the child's Linux `VmHWM`, read after
timing. Neither replaces the separate allocation observations.



### Primary workflow means

| Workflow | Workers | std ns/transaction | Unmetered | Borrowed concurrent | Owning concurrent | Owning change | Paired bootstrap 95% | std repeat change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| arc_app_cow4096_r1 | 1 | 77.899 | 79.644 | 84.847 | 85.008 | +9.1% | +6.4% to +11.4% | +3.0% |
| arc_app_cow4096_r128 | 1 | 776.177 | 864.818 | 881.126 | 874.635 | +12.7% | +8.9% to +15.9% | -1.2% |
| arc_app_cow65536_r128 | 1 | 1939.109 | 2047.166 | 2049.699 | 2022.841 | +4.3% | +1.2% to +7.6% | +4.2% |
| arc_app_domain64_t1 | 1 | 34.670 | 41.681 | 48.858 | 64.568 | +86.2% | +79.3% to +92.8% | +3.0% |
| arc_app_domain64_t4 | 4 | 13.199 | 14.536 | 16.347 | 22.872 | +73.3% | +59.0% to +91.3% | +0.1% |
| arc_app_mutex_t1 | 1 | 9.909 | 10.052 | 9.784 | 9.990 | +0.8% | -1.1% to +2.9% | +6.0% |
| arc_app_mutex_t4 | 4 | 52.154 | 59.899 | 53.617 | 58.906 | +12.9% | +8.4% to +17.5% | -2.3% |
| arc_app_request4096_t1 | 1 | 101.398 | 108.413 | 158.763 | 171.259 | +68.9% | +52.7% to +82.2% | -1.5% |
| arc_app_request4096_t4 | 4 | 37.154 | 38.009 | 76.694 | 84.005 | +126.1% | +112.6% to +144.4% | +1.5% |
| arc_app_request64_t1 | 1 | 35.756 | 38.737 | 49.771 | 53.656 | +50.1% | +44.5% to +56.9% | -1.0% |
| arc_app_request64_t4 | 4 | 13.554 | 15.139 | 41.849 | 56.069 | +313.7% | +267.1% to +349.4% | -4.3% |
| arc_app_state128_t1 | 1 | 81.914 | 83.000 | 85.395 | 80.209 | -2.1% | -5.9% to +1.5% | +1.9% |
| arc_app_state128_t4 | 4 | 39.542 | 37.816 | 38.936 | 38.969 | -1.4% | -5.8% to +4.2% | -6.9% |
| arc_app_state8_t1 | 1 | 10.645 | 10.880 | 11.088 | 11.080 | +4.1% | -0.5% to +8.9% | -3.9% |
| arc_app_state8_t2 | 2 | 14.263 | 21.357 | 20.956 | 20.608 | +44.5% | +30.1% to +57.6% | +0.1% |
| arc_app_state8_t4 | 4 | 19.712 | 27.174 | 23.848 | 25.331 | +28.5% | +17.3% to +40.3% | -9.0% |

### Follow-up means

| Workflow | Workers | std ns/transaction | Unmetered | Borrowed concurrent | Owning concurrent | Owning change | Paired bootstrap 95% | std repeat change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| arc_app_init4096_t1 | 1 | 98.602 | 104.064 | 115.218 | 109.321 | +10.9% | +6.5% to +15.5% | -6.0% |
| arc_app_init4096_t4 | 4 | 39.957 | 39.417 | 55.276 | 60.545 | +51.5% | +26.1% to +76.3% | +0.6% |
| arc_app_unique4096_r1 | 1 | 7.759 | 9.469 | 9.707 | 9.789 | +26.2% | +23.8% to +28.7% | +0.3% |

## Identified costs and improved usage

**Object count contention.** The optimized std clone wrapper uses `lock incq`
and a failure branch. jzap checks the admitted count and uses `lock cmpxchgq`
with a retry back edge; its existing shared-owner credit must remain fallible
before saturation. The actual application binary has the same count kernel.
All jzap policies, including unmetered, show the short shared-state contention
cost, isolating it from owning-budget administration. More work between clones
reduces both the fraction spent on ownership and simultaneous count updates.
Hardware retry/stall counts were not measured, so the timing loss is not
assigned a hardware-counter percentage.

**Concurrent budget admission.** Shared-budget construction updates one domain's
atomic used-byte ledger. Owning policy acquisition additionally updates its Arc
count. Releasing the payload frees its storage, refunds the charge and releases
that policy owner. Private request ledgers remove this cross-request sharing:
for 64-byte requests, the borrowed concurrent result changes from 41.849 to
16.347 ns/transaction with four workers. Owning results change from 56.069 to
22.872 ns. A private ledger enforces its own quota; it does not replace a shared
application-wide cap. The owned private-domain constructor also adds a separate
heap allocation and final free to every request.

**Payload temporaries.** In the original 4KiB request caller, the payload is
copied and one word changed before the fallible constructor. std and unmetered
jzap compile to one payload `memcpy`; budgeted construction retains a 4KiB
stack temporary followed by another heap copy. The existing `try_init_in` API
moves that payload construction after admission. Optimized public-API probes
with the same modified array retain one copy and no 4KiB temporary in this
form. The workflow's owning mean falls from 171.259 to 109.321 ns with one
worker, and from 84.005 to 60.545 ns with four. Separate matched std controls
are retained for the two caller forms. This is an improved use of the existing
API; no product implementation or failure contract was changed.

**Unique COW.** An exclusive page is edited without an allocation or payload
copy. The current fast path prechecks strong/weak counts before the uniqueness
protocol. Its extra loads/branches remain visible in generated code. This
condition adds about 2.03 ns, despite its +26.2% relative figure. Shared COW,
which really copies the retained page, has a different measured cost profile.

The underlying object and budget count changes are per ownership/allocation
lifecycle. The removed stack copy was proportional to payload size. Neither
observation supports assigning one universal slowdown to all Arc use.

## Memory

The [28 allocation observations](arc-scenario-memory.csv) include the domain's
32-byte requested bootstrap allocation and its allocator-usable storage.
All observations end at zero retained requested and usable bytes. The probes
use the Linux System allocator and distinguish these layout counts from RSS.

| Working set | std requested peak | Owned requested peak | std usable peak | Owned usable peak | Usable change |
| --- | ---: | ---: | ---: | ---: | ---: |
| objects64_1 | 80 | 120 | 88 | 128 | +45.45% |
| objects64_16 | 1280 | 1440 | 1408 | 1448 | +2.84% |
| objects4096_1 | 4112 | 4152 | 4120 | 4160 | +0.97% |
| objects4096_16 | 65792 | 65952 | 65920 | 65960 | +0.06% |
| state65536_1 | 65552 | 65592 | 65560 | 65600 | +0.06% |
| cow4096 | 8224 | 8272 | 8240 | 8280 | +0.49% |
| cow65536 | 131104 | 131152 | 131120 | 131160 | +0.03% |

A private owned budget for one 64-byte payload exceeds the +15% memory criterion:
80 → 120 requested bytes and 88 → 128 usable bytes. Sixteen such objects in one
domain use 1,280 → 1,440 requested bytes (+12.5%) and 1,408 → 1,448 usable bytes
(+2.84%). Pages and shared COW peaks remain within +15%. Policy ownership does
not add a separate heap allocation for every shared handle clone.

## Validation and reproduction

The expanded harness checks 152 short workflow processes (19 conditions, two
seeds and four implementations) against std checksums and work counts, including
concurrent locked totals and shared/unique page edits. Seven Python reporting
and negative-control tests pass, and Clippy passes for the timing and allocation
feature builds. Product `src/`, proof scripts and runtime dependencies are
unchanged; the existing 864-obligation source certificate remains the product
receipt. This campaign adds workload coverage and does not claim a new formal
proof.

Build and run the primary sixteen tasks from checkpoint `bf763049`. The current
harness adds the three follow-up tasks; running its complete task list renews
all nineteen conditions. All raw outputs remain under ignored `target/`.

```sh
cargo build --locked --release --manifest-path tools/throughput-analysis/Cargo.toml --bin arc_scenarios --target-dir target/verification/arc-scenarios/timing
cargo build --locked --release --manifest-path tools/throughput-analysis/Cargo.toml --bin arc_scenarios --features shared-memory --target-dir target/verification/arc-scenarios/memory
python3 tools/throughput-analysis/test_arc_scenarios.py --binary target/verification/arc-scenarios/timing/release/arc_scenarios
python3 -m unittest discover -s tools/throughput-analysis -p 'test_*.py'
target/verification/arc-scenarios/memory/release/arc_scenarios memory > target/verification/arc-scenarios/memory.jsonl
python3 tools/throughput-analysis/compare.py --binary app="$PWD/target/verification/arc-scenarios/timing/release/arc_scenarios" --seeds 617,6203,96011,17291,70183 --small-tasks '' --small-variants '' --cpus 0,1,2,3 --output target/verification/arc-scenarios/renewal
python3 tools/throughput-analysis/scenario_report.py --input target/verification/arc-scenarios/renewal --memory target/verification/arc-scenarios/memory.jsonl
```

Emit public clone wrappers using the maintained analysis library, and inspect
the application binary alongside debug LLVM/assembly from the same source:

```sh
cargo rustc --locked --release --manifest-path tools/throughput-analysis/Cargo.toml --lib -- --emit=asm
cargo rustc --locked --release --manifest-path tools/throughput-analysis/Cargo.toml --bin arc_scenarios -- --emit=asm,llvm-ir -C debuginfo=2
```
