# Integrated Arc headroom protocol

This work replaces strong-clone CAS with an atomic increment, guard-bit test and atomic
subtraction on refusal on native Linux x86-64. Other native targets retain CAS. Weak
upgrades retain CAS against zero. This report separates the integrated protocol from the
[historical speed experiment](arc-headroom.md). The integrated verification matrix below
passes, including the sole implicit Weak retirement optimization. The final throughput
campaign is reported below; historical experimental timings are not substituted for this
source.

## Live counts, refusal and retirement

Zero is the logical end of strong ownership. While live, the raw strong word is `S + P`:
successful strong credits plus rejected increments awaiting rollback. Each rejected
attempt subtracts exactly one from the current atomic word. It never restores a saved
count, so intervening increments and releases survive. The attempt borrows a strong
owner until rollback; that owner pins the payload.

The strong word still stores the checked allocation size after logical zero, so eventual
Weak destruction can refund the original requested bytes. An independent high bit in the
weak word now validates that metadata. A high strong word with no retired flag is live
saturation: upgrade returns `Error::RefCount`, rather than `Ok(None)`. Retirement
publishes the weak flag before the tagged size. Cyclic publication clears the old size
through zero, clears the flag, then release-publishes one initialized strong owner.
Slow-path acquire reads and rechecks distinguish these publications from provisional
counts. Normal low-count lookup does not execute this slow path.

Atomic weak admission reserves one count below the ordinary strong maximum, keeping
`retired flag | count` distinct from the all-ones uniqueness lock. Local Rc admission is
unchanged. No handle field, header field, allocation, product dependency or
user-supplied task quota is added.

## Native execution bound and trusted platform boundary

Increment/test/subtract requires a bound on outstanding increments; testing a guard bit
alone cannot prove it. The numeric proof uses `0 <= P < MAX_COUNT` and proves `S + P + 1
<= usize::MAX`, accepted-credit bounds, rollback conservation and that a reservation
pins an owner. A deliberate mutation that removes the pending bound must fail
verification.

The native fast path is restricted to **Linux x86-64 with 64-bit pointers and the
four/five-level native paging ABI**. The platform refinement uses:

- Distinct executing native tasks have positive `pid_t` IDs, bounded by
  `i32::MAX`. Linux's ordinary configured ceiling is lower.
- Re-entered or suspended native activations require distinct nonempty valid
  signal/continuation storage. Across one native address space, that storage
  fits below the `2^56`-byte user-address ceiling. An unreturned native
  activation cannot discard and reuse its continuation storage while retaining
  a pending increment.
- The clone kernel executes no allocator, callback, destructor or voluntary
  context switch between increment and admission/rollback. A signal nesting
  frame is counted as suspended storage, rather than assuming signals never
  re-enter. Ordinary running attempts are counted by task IDs.

Thus the conservative bound is `P < 2^56 + 2^31`, far below `2^63 - 1`. Kani checks the
actual machine-word additions under these platform bounds; Verus checks the credit
arithmetic and invariants. These tools **do not verify the Linux kernel, the native ABI
implementation or arbitrary foreign context-switching assembly**. Those are explicit
trusted platform/valid-caller boundaries, alongside the existing compiler, allocator and
public-unsafe boundaries. Foreign runtimes must preserve valid native activation
storage; the numeric theorem must not be presented as proving arbitrary synthesized
execution contexts. Other target triples keep bounded CAS admission.

Pinned platform references: Linux **v6.18** [task
limits](https://github.com/torvalds/linux/blob/v6.18/include/linux/threads.h), [PID
allocation and bounded
sysctl](https://github.com/torvalds/linux/blob/v6.18/kernel/pid.c), [x86-64 address
limits](https://github.com/torvalds/linux/blob/v6.18/arch/x86/include/asm/page_64_types.h),
[actual task-size
selection](https://github.com/torvalds/linux/blob/v6.18/arch/x86/include/asm/page_64.h),
and [native signal
frames](https://github.com/torvalds/linux/blob/v6.18/arch/x86/kernel/signal_64.c). The
reduced-count Loom model uses at most three running threads and guard 8; it verifies
that bounded domain rather than deriving the native platform limit.

## Verification status

The maintained entry point is `bash scripts/verify.sh --area shared`; select
checks using `--check` or the memory profile. New checks include five
numeric/representation regressions, six actual-implementation Loom tests, all-word and
reduced-count Kani kernels, native/u32 Verus credit models and compiled negative
controls. The AST census now records the sealed Counter trait, rejecting new default
bodies that would otherwise escape implementation-body bindings. Raw logs, generated
models and assembly remain ignored under target.

| Check | Integrated result |
| --- | --- |
| Debug, release and overflow-checked release | 232 runtime tests and 26 doctests pass in each configuration |
| ASan | The same 232 runtime cases and 26 doctests pass; rustc and rustdoc both receive the sanitizer flag |
| Miri | 46 shared/owned-budget/guard cases pass on x86-64, i686 and strict Tree Borrows; 136 existing Box/Vec/String/iterator memory cases also pass |
| Actual Rust Kani | 20 shared kernels, six reduced-count checks and two concurrent-budget kernels pass |
| Verus | 35 native and 35 u32 checks pass; the Linux execution-bound contract applies to the 64-bit target |
| Actual-implementation Loom | 12 existing and six new scenarios pass |
| Negative controls | 23 detected: two solver mutations, 12 compiled source-boundary mutations and nine executed Loom mutations |
| Fuzzing | 10,000 shared-ownership runs complete |
| Compiler boundaries | 108 cases and 12 weakened-bound controls pass; core-only, wasm/thumb and no-global-OOM builds pass |
| Counter coverage | 35/35 functions, 190/203 lines and 46/53 branch outcomes in the native selected-suite report; race retries use Loom and the CAS fallback also uses i686 Miri/Kani |
| Whole-library source/link certificate | Pass: 880 source obligations, zero open obligations, zero unproved link edges; four profiles, 62 originating guards, five forwarders and eight detected exit controls |

The certificate input SHA-256 is
`8fb88454cd88d9aadbc766c93cde9112c49913e47e725bd55823bf44980ec06f`. It matches the
integrated source and verification inputs. Retained originating guards have
source-connected unreachability proofs under the stated contracts; the retained linker
diagnostic alone is not treated as proof or as a new library panic path.

### Sole implicit Weak retirement

After an initialized payload's final strong transition, an acquired weak count of one
denotes the sole private implicit weak. Prior strong releases and the final acquire
transition publish earlier Weak creations. With no public weak and no live strong owner,
no valid weak-creation source remains. This phase permits a release atomic store of
`RETIRED | 1`; a count above one keeps the release `fetch_or`, preserving concurrent
clones/releases. Cyclic construction is a different phase: its construction Weak remains
publicly borrowable and keeps the existing publication protocol.

An isolated, equal-work three-seed comparison measured owning request
construction/fanout/destruction at 52.502 → 48.136 ns with one worker and 52.427 →
47.330 ns with four. Each interval lasts at least one second. The changed source removes
one locked RMW per exclusive retirement; it does not remove the shared quota reservation
CAS or refund. Borrowed-policy four-worker means did not improve in this campaign, so a
universal timing gain is not claimed. Whole-program layout variation and quota
contention remain visible in the controls and are assessed again in the final campaign.

The verification includes an all-word retirement-credit Kani proof, a reduced- count
guard calibration, an explicit weak-creator phase theorem and negative controls for
overwriting a surviving weak and omitting the final acquire. The new race fixture
observes the retained charge before touching the Weak, allowing a deliberately broken
variant to fail without a secondary use-after- free during test cleanup.

Loom mutation packages use separate target directories and check their resolved source
paths. Sharing the product target initially reused an unmutated cached binary; the
negative-control gate rejected that result. Fresh isolated builds now detect missing
rollback, reversed admission, live/retired aliasing, missing retirement publication and
weakened cyclic publication. Custom assertion messages are accepted only as failures at
the actual test oracle, while compiler errors and model-limit failures remain excluded.

## Reproduction

The recording host is Linux x86-64, AMD EPYC 9V74, with four pinned workers and
glibc allocation. Rust is 1.95.0 (`59807616e1fa2540724bfbac14d7976d7e4a3860`),
LLVM 22.1.2. Both libraries use default release optimization and no forced loop
alignment. The CAS baseline source is pinned at `8332575`; the integrated source
is in this report's commit. Use fresh ignored output directories for a replay.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
. /workspace/.devtools/activate.sh
bash scripts/verify.sh --area shared --check api --check tests --check small-counts --check loom
bash scripts/verify.sh --area shared --profile memory
cargo build --locked --offline --release \
  --manifest-path tools/throughput-analysis/Cargo.toml --bin arc_scenarios
mkdir -p target/verification/arc-cas-source
git archive 8332575ef5bd7aa2a28daa1fa46d22491771598a | \
  tar -x -C target/verification/arc-cas-source
cargo build --locked --offline --release \
  --manifest-path target/verification/arc-cas-source/tools/throughput-analysis/Cargo.toml \
  --target-dir target/verification/arc-cas-build --bin arc_scenarios
JZAP_ARC_CORE=arc_app_state8_t1,arc_app_state8_t2,arc_app_state8_t4,arc_app_state128_t4,arc_app_request64_t4
JZAP_ARC_REST=arc_app_state128_t1,arc_app_request64_t1,arc_app_request4096_t1,arc_app_request4096_t4,arc_app_domain64_t1,arc_app_domain64_t4,arc_app_cow4096_r1,arc_app_cow4096_r128,arc_app_cow65536_r128,arc_app_mutex_t1,arc_app_mutex_t4,arc_app_unique4096_r1,arc_app_init4096_t1,arc_app_init4096_t4
JZAP_ARC_REPEAT_SEEDS=100003,100019,100043,100049,100057,100069,100103,100109,100129,100151,100153,100169,100183,100189,100193
python3 tools/throughput-analysis/compare.py \
  --binary integrated="$PWD/tools/throughput-analysis/target/release/arc_scenarios" \
  --binary cas="$PWD/target/verification/arc-cas-build/release/arc_scenarios" \
  --tasks "$JZAP_ARC_CORE" --seeds 617,6203,96011,17291,70183 \
  --small-tasks '' --cpus 0,1,2,3 --output target/verification/arc-replay-core
python3 tools/throughput-analysis/compare.py \
  --binary integrated="$PWD/tools/throughput-analysis/target/release/arc_scenarios" \
  --tasks "$JZAP_ARC_REST" --seeds 617,6203,96011 \
  --small-tasks '' --cpus 0,1,2,3 --output target/verification/arc-replay-rest
python3 tools/throughput-analysis/compare.py \
  --binary integrated="$PWD/tools/throughput-analysis/target/release/arc_scenarios" \
  --binary cas="$PWD/target/verification/arc-cas-build/release/arc_scenarios" \
  --tasks arc_app_state8_t4 --seeds "$JZAP_ARC_REPEAT_SEEDS" \
  --small-tasks '' --cpus 0,1,2,3 --output target/verification/arc-replay-repeat
python3 tools/throughput-analysis/scenario_report.py \
  --input target/verification/arc-replay-core
python3 tools/throughput-analysis/scenario_report.py \
  --input target/verification/arc-replay-rest
python3 tools/throughput-analysis/scenario_report.py \
  --input target/verification/arc-replay-repeat
```

Timing starts after all CPU-intensive verification/build jobs finish. Every accepted
timing interval must last at least one second; the same seeded corpus, operation count
and checksums are required for std and each policy. The existing +15% speed gate and
±10% repeated-std drift gate remain unchanged. Owning and borrowed shared-budget
allocation costs are assessed separately from strong handle cloning; a faster clone does
not remove ledger reservation contention.

## Integrated code generation and memory

The maintained `std_arc_handle_clone` and `jzap_arc_handle_clone` wrappers in
`tools/throughput-analysis/src/lib.rs` emit the following normal paths under Rust 1.95.0
/ LLVM 22.1.2, default release settings:

```asm
# std                                # jzap
movq (%rdi), %rax                     movq (%rdi), %rax
lock incq (%rax)                      lock incq (%rax)
jle std_failure                       js jzap_failure
retq                                  retq
```

There is no CAS retry on successful strong cloning in this target. The guard conditions
differ: jzap refuses the increment crossing its admitted maximum. Its failure path
atomically decrements before propagating `Error::RefCount`. These wrappers explicitly
unwrap, so their cold failure code then invokes the caller's chosen unwrap handler; that
is separate from library error return. The ordinary std failure path terminates. Equal
instruction counts do not by themselves establish equal elapsed times under contention.

Inspection of the actual linked workload binary also confirms that retirement loads the
weak word at header offset 8 and compares it with one. The exclusive branch writes the
retired flag/count with an ordinary aligned atomic store; the shared branch still
executes `lock or`. Both then publish the tagged layout size. No per-element loop is
introduced. The observed request-lifecycle difference is a per-retirement cost, not a
new payload-size-proportional scan.

The same disassembly identifies the remaining quota costs. Borrowed concurrent policies
reserve against a shared CAS counter and refund with `lock sub`. Owning policies also
acquire/release a ledger owner with locked increments and decrements. Those ledger words
share a cache line on this target. These operations remain in high-rate construction,
while cloning an existing payload handle updates only that payload's strong word.
Removing the strong CAS retry does not remove quota contention.

The integrated allocation probe repeats all 28 workflow observations. Every probe
returns requested live bytes to zero. The flag uses an existing counter bit, so
requested/usable held and peak bytes match the previous representation:

| 64-byte objects sharing one domain | Policy | Requested held/peak bytes | Allocator usable held/peak bytes | Allocations |
| --- | --- | ---: | ---: | ---: |
| 1 | std / unmetered | 80 | 88 | 1 |
| 1 | borrowed concurrent | 88 | 88 | 1 |
| 1 | owning concurrent, including bootstrap | 120 | 128 | 2 |
| 16 | std / unmetered | 1,280 | 1,408 | 16 |
| 16 | borrowed concurrent | 1,408 | 1,408 | 16 |
| 16 | owning concurrent, including bootstrap | 1,440 | 1,448 | 17 |

The allocator is glibc on the measured Linux x86-64 host. Usable bytes measure allocator
rounding, rather than RSS. Memory probes use a separate instrumented binary; timings use
the ordinary release binary. The 32-byte requested (40-byte usable) owning-domain
bootstrap is outside the payload quota and occurs once per domain.

## Final performance results

The final campaign contains **610 valid intervals**, each at least one second: 250 for
five core conditions with five seeds and both CAS/integrated libraries, 210 for the
remaining fourteen conditions with three seeds, and 150 for one fixed larger repeat. A
separate 60-interval retirement comparison isolates the exclusive-retirement change. No
build, model-checking or verification job runs during timing. Workers use CPUs 0–3 and
every paired group has equal work, inputs and result checksums.

The initial integrated four-worker state8 cohort failed the predefined identical-std
repeat check (−14.5%). It remains in the CSV with `selected_for_summary=false`. One
predeclared larger cohort used fifteen new seeds; its integrated/CAS std repeats differ
by −3.1%/+0.8%, within the unchanged ±10% gate. The larger cohort is the primary
comparison for that condition. No further repeat was needed; every other final condition
passed the same gate.

[All 104 timing aggregates](arc-headroom-integration-benchmarks.csv) retain both the
rejected initial cohort and the selected results. [Sixteen retirement
aggregates](arc-retirement-benchmarks.csv) and [28 memory
observations](arc-headroom-integration-memory.csv) retain the separate cause analysis
and allocator measurements. Means use summed seconds divided by summed calls. Paired 95%
intervals resample complete seeded recordings 10,000 times; they are not request-latency
percentiles. Multi-worker ns/transaction are inverse aggregate throughput.

### Comparison scope

Std Arc does not reserve or refund a shared byte budget. The unmetered comparison
measures the corresponding ownership workload with explicit failure handling in
jzap. Borrowed and owning budget configurations add byte-limit enforcement;
the owning configuration also retains the ledger's lifetime. Their std-relative
ratios describe the total cost of those additional features, rather than the
efficiency of two equivalent budget implementations. This campaign contains no
std-based implementation of the same byte-budget contract.

For the same four-worker, 64-byte request/fanout workload, the selected five-seed
records give:

| Configuration | ns/transaction | Versus unbudgeted std | Shared byte-budget work |
| --- | ---: | ---: | --- |
| Std Arc | 14.011 | Baseline | None |
| jzap Unmetered | 12.659 | -9.6% | None |
| jzap borrowed concurrent budget | 37.587 | +168.3% | Reservation CAS and atomic refund |
| jzap owning concurrent budget | 48.674 | +247.4% | Reservation/refund plus ledger-owner acquisition/release |

Each row includes construction, fanout and final release. These are inverse
aggregate-throughput figures, not individual request latencies. The confidence
intervals and repeated-std control remain in the linked CSV. The 48.674 ns result
is a budget-enabled lifecycle cost; existing-handle cloning performs no new byte
reservation and is measured separately below. The default-policy speed criterion
and optional-feature costs remain separate, allowing callers to assess the
accounting guarantees against their workload's time and memory requirements.

### Strong ownership workflows

| Workflow | Workers | Std ns/transaction | Integrated owning ns/transaction | Versus std | Paired 95% interval | Previous CAS ns/transaction |
| --- | ---: | ---: | ---: | ---: | --- | ---: |
| Clone + 8 reads | 1 | 9.959 | 9.452 | -5.1% | -12.9% to +1.1% | 9.908 |
| Clone + 8 reads | 2 | 14.142 | 13.824 | -2.2% | -6.0% to +2.2% | 18.640 |
| Clone + 8 reads | 4 | 20.008 | 19.848 | -0.8% | -5.9% to +4.2% | 28.073 |
| Clone + 128 reads | 4 | 38.084 | 38.346 | +0.7% | -5.1% to +4.7% | 39.396 |

The owning two-/four-worker short-state means improve 25.8%/29.3% directly against the
prior CAS implementation. The four owning strong-state comparisons in this table have
paired intervals within the +15% criterion. The ordinary clone has the same successful
instruction count as std; eliminating expected-value CAS retries is the structural
change. This does not assert a universal speedup for every Arc operation or target.

### Additional application shapes

| Workflow | Std ns/transaction | Integrated owning ns/transaction | Versus std | Paired 95% interval |
| --- | ---: | ---: | ---: | --- |
| Shared 4KiB COW, one read | 75.226 | 80.873 | +7.5% | +4.8% to +9.6% |
| Shared 4KiB COW, 128 reads | 711.768 | 746.458 | +4.9% | +1.7% to +6.8% |
| Shared 64KiB COW, 128 reads | 1866.706 | 1988.224 | +6.5% | +0.8% to +16.8% |
| Locked shared state, four workers | 52.144 | 52.270 | +0.2% | -3.4% to +7.3% |
| Unique 4KiB COW, one read | 8.319 | 9.090 | +9.3% | -3.4% to +18.8% |
| Shared-quota 64-byte request/fanout, four workers | 14.011 | 48.674 | +247.4% | +208.5% to +283.4% |
| Shared-quota in-place 4KiB request, four workers | 34.760 | 58.444 | +68.1% | +28.9% to +147.2% |

Unmetered point means for all nineteen selected conditions are within +15% of std; the
64KiB COW and four-worker by-value 4KiB request intervals cross that threshold and
remain uncertain. Owning 64KiB shared COW and unique COW are also borderline by
interval. There is no universal +15% performance pass across configurations.

High-rate quota-managed creation has the additional feature costs shown above.
The disassembly identifies
shared reservation CAS/refund and owning-ledger increment/decrement, rather than a new
per-element scan. In-place 4KiB creation reduces the owning by-value means from 154.294
to 115.178 ns with one worker and 81.784 to 58.444 ns with four, removing the previously
identified extra stack copy. Quota contention remains. Creating a fresh owning domain
per request also adds its management allocation and is not equivalent to sharing one
application-wide quota.

The memory representation is unchanged by the counter optimization. The final probe
repeats all 28 observations exactly, including zero final live bytes.
