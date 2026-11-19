# Arc strong-clone headroom experiment

This is the historical isolated experiment at `8332575`. The subsequent
[integrated protocol and verification](arc-headroom-integration.md) are separate
results. The generator now obtains that historical input from Git explicitly.

This experiment asks whether replacing the owned strong-handle clone
compare-and-swap (CAS) loop
with a locked increment and rollback materially improves the measured workflows.
The generated headroom library is not a supported product implementation.

`tools/arc-counter-experiment/prepare.py` generates two disposable source trees
under ignored `target/verification/arc-counter-experiment/`. The baseline is the
unchanged product. The candidate changes `Strong::try_clone` to call a new
counter operation: local counters delegate to existing admission; atomic
counters increment, inspect the new count's guard bit, and undo a refused credit.
Weak clone, downgrade, upgrade, uniqueness, retirement and release keep their
existing operations. Owning concurrent policies use the new strong-clone path
when acquiring another ledger owner.

The atomic method is eleven lines; the trait default and one call-site change
bring the experimental product-source delta to roughly twenty lines. No counter
field, control-block layout, allocator or public owning type is added.

## Boundaries fixed before timing

Performance covers the existing state8 1/2/4-worker conditions, state128 with
four workers, and 64-byte request creation/fanout with four workers. Baseline,
candidate and std use identical inputs, work, checksums and affinity. Three seeds
produce paired intervals of at least one second, using the existing driver.
The +15% std criterion and ±10% identical-std repeat check remain unchanged.
Compilation and memory/model checks finish before timing.

These workflows keep each counter in a small range, far below the guard bit.
Native and Miri exercise real heap ownership in that domain; Loom checks bounded
interleavings and saturation rollback with its reduced count maximum.

Two explicit controls document why general product safety is still open:

1. At the real word's saturation boundary, the post-increment value has the same
   high bit as retired payloads. An unchanged Weak upgrade can classify that
   temporary live value as dead before rollback.
2. In a four-bit model, seven live credits followed by ten increments paused
   before checks/rollback produce `8,9,...,15,0,1`. After consuming the entire
   headroom, the guard test can accept again. A general proof needs an explicit
   bound on simultaneously outstanding increments, or a different protocol.

Passing tests that expose these controls does not close these defects. The
experiment measures a low-count fast path without asserting its validity for
all safe Arc callers or changing the product's existing certificate.

### Count conservation and the legacy encoding

Zero remains the logical death condition. In the live, non-wrapping domain,
write the raw count as `live credits + provisional increments`. Rolling back a
failed increment with atomic subtraction removes only that provisional credit;
other threads' increments and releases are preserved. For example, seven live
credits plus one provisional increment gives eight; two releases give six;
rolling back the provisional increment leaves the correct five live credits.

The failed Weak test exposes a representation mismatch. The existing metered
implementation reuses the strong-counter word after its zero transition to
store a tagged allocation size for the eventual Weak refund. Its upgrade code
therefore treats above-limit words as retired metadata. Retaining that test
while permitting provisional overflow words causes the false None result.
It does not establish that increment-and-subtract corrupts other owners' credits.
An integrated design must distinguish zero/death from temporary saturation and
place or encode the refund metadata accordingly.

## Results

The main campaign contains **150 valid intervals**, three seeds (617, 6203,
96011), five workflows, two separately compiled libraries and five policy/control
labels. A separate five-seed read-heavy repeat adds 50 intervals. Every selected
interval is at least **1.062 seconds**. The [48 aggregate rows](arc-headroom-benchmarks.csv)
retain both campaigns, all labels, matching std means, counts, seed spread and
paired bootstrap intervals. Means are summed seconds divided by summed work;
multi-worker ns/transaction express inverse aggregate throughput.

The table below uses owning concurrent policies. Each library has its own
matched std control; the CSV retains both. The direct change compares equal
work between CAS and headroom builds, without assuming their code layouts
are identical.

| Workflow | Workers | CAS ns/transaction | Headroom ns/transaction | Direct change | CAS vs its std | Headroom vs its std |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |

| arc_app_state8_t1 | 1 | 9.295 | 9.790 | +5.3% | -0.1% | +1.3% |
| arc_app_state8_t2 | 2 | 20.625 | 15.333 | -25.7% | +41.9% | +5.5% |
| arc_app_state8_t4 | 4 | 32.550 | 19.757 | -39.3% | +52.4% | -14.8% |
| arc_app_state128_t4 | 4 | 37.365 | 38.865 | +4.0% | -0.2% | +2.9% |
| arc_app_request64_t4 | 4 | 44.156 | 46.456 | +5.2% | +241.5% | +271.7% |

The short contended owning-handle paths improve **25.7% with two workers** and
**39.3% with four**, compared directly with CAS. The corresponding unmetered
improvements are **28.6%** and **19.9%**. Single-worker results remain close to
std; the owning variant's direct +0.495 ns difference is retained rather than
presented as a universal speedup.

Shared-budget request creation does not improve: 44.156 → 46.456 ns with owning
policies, and 34.953 → 36.453 ns with borrowed policies. Without a budget, that
same request workflow improves from 15.106 to 11.952 ns (−20.9%). Removing
strong-clone CAS does not remove the concurrent budget reservation CAS/refund,
which all workers still direct at one shared ledger.

### Read-heavy uncertainty

The initial owning read-heavy point changes from 37.365 to 38.865 ns; the
unmetered candidate mean is +17.3% against its std with a very wide paired
interval (−9.0% to +63.5%). This prompted an independent five-seed repeat,
whose owning means are 43.916/42.626 ns. The candidate's identical-std repeat
mean differs by **+10.4%**, exceeding the predefined ±10% gate. That repeat is
retained as unstable, and the read-heavy condition is **inconclusive**.

The shared `read<8192>` function has the same 61 instructions in both emitted
assemblies after debug-label normalization. No new per-element read loop was
introduced. The data do not isolate the observed variation to a particular
hardware stall or code-layout effect, and no improvement is claimed for this
condition. Paired intervals resample whole seeded recordings 10,000 times;
they are not request-latency percentiles.

## Code generation

The actual public clone wrapper changes from admission checks plus a
`lock cmpxchg` retry loop to:

```asm
movq (%rdi), %rax
lock incq (%rax)
js failure
retq
failure:
lock decq (%rax)
# propagate RefCount; this benchmark wrapper unwraps unexpected failures
```

The source changes only strong-handle acquisition, including ownership of an
owning budget ledger. Existing weak admission, uniqueness, publication,
retirement and release operations keep their previous protocols. A rollback
uses a release atomic decrement and performs no allocation. The generated
library contains no additional object or counter fields.

## Verification and product decision

| Check | Result and scope |
| --- | --- |
| Native existing integration tests | 227 pass in the generated candidate |
| Experiment checks | Two numeric/concurrent checks pass; two issue oracles expose the known limits |
| Tree Borrows Miri, strict provenance | 41 existing shared/owned-ledger scenarios plus the four experiment checks pass |
| Loom positive cases | 12 existing cases and two bounded admission/rollback cases pass |
| Real Arc saturation counterexample | Passes against baseline CAS; fails against the candidate with `temporary overflow incorrectly reported a live Arc as dead` |
| Workflow preflight | 152 processes per variant, 304 total; values/checksums and work agree with std |
| Clippy | Candidate library passes with warnings denied |
| Reporter controls | Eight tests pass, including isolation of repeated-std gates between variants |

The expected Loom failure is a functional defect, not a successful safety proof.
All seven reduced-limit strong owners remain live; a concurrent refused clone
leaves a temporary count of eight, and an unchanged Weak upgrade returns None.
The counterexample uses real Arc handles and ordinary safe operations.

The prototype demonstrates useful performance potential for short, contended
strong-handle cloning. It is **not eligible for product promotion** while the
transient-count classification and general pending-increment bound remain open.
The product `src/`, proof scripts and runtime dependencies are unchanged. Its
existing certificate input fingerprint remains
`5c2cabf887db443fe15d14f862728d7412755d53f5d1ca23abf8b2cc29ef8665`.

The unchanged and experimental product-source SHA-256 values (relative path,
NUL, contents) are:

- CAS: `1f3964def2efe478e0668514404bcb3093a5fc875a533755c8dbc535b3250444`
- Headroom: `3b98d1d9147f6b7575a7792a5c7e53ddad2fcbc314540bc5da0de027eafad592`

## Reproduction

Rust 1.95.0/LLVM 22.1.2, release optimization, one codegen unit, `panic=abort`,
no loop-alignment flags, AMD EPYC 9V74, CPUs 0–3 and the container's four-core
quota match the preceding workflow method. Build and finish checks before timing.
`prepare.py` refuses to overwrite an existing experiment directory; use a fresh
ignored output directory when repeating setup.

```sh
python3 tools/arc-counter-experiment/prepare.py
cargo test --locked --offline --all-features --manifest-path target/verification/arc-counter-experiment/headroom/Cargo.toml --tests
MIRIFLAGS='-Zmiri-tree-borrows -Zmiri-strict-provenance' cargo +nightly-2026-10-30 miri test --locked --offline --all-features --manifest-path target/verification/arc-counter-experiment/headroom/Cargo.toml --test shared --test owned_budget --test headroom_experiment
RUSTFLAGS='--cfg jzap_loom --cfg jzap_small_counts' cargo test --locked --offline --manifest-path target/verification/arc-counter-experiment/headroom/tools/shared-loom/Cargo.toml --test concurrent
```

Run `--test headroom_negative` through the same Loom command separately: baseline
CAS must pass and headroom must fail at the stated assertion. Do not count the
candidate's nonzero result as successful verification.

```sh
cargo build --locked --offline --release --manifest-path target/verification/arc-counter-experiment/cas/tools/throughput-analysis/Cargo.toml --bin arc_scenarios --target-dir target/verification/arc-counter-experiment/cas-build
cargo build --locked --offline --release --manifest-path target/verification/arc-counter-experiment/headroom/tools/throughput-analysis/Cargo.toml --bin arc_scenarios --target-dir target/verification/arc-counter-experiment/headroom-build
python3 tools/throughput-analysis/compare.py --binary cas="$PWD/target/verification/arc-counter-experiment/cas-build/release/arc_scenarios" --binary headroom="$PWD/target/verification/arc-counter-experiment/headroom-build/release/arc_scenarios" --tasks arc_app_state8_t1,arc_app_state8_t2,arc_app_state8_t4,arc_app_state128_t4,arc_app_request64_t4 --small-tasks '' --small-variants '' --cpus 0,1,2,3 --output target/verification/arc-counter-experiment/performance
python3 tools/throughput-analysis/scenario_report.py --input target/verification/arc-counter-experiment/performance
```

Repeat only `arc_app_state128_t4` with seeds `617,6203,96011,17291,70183` and a
separate output directory to renew the uncertainty check. Preserve both sets.
Direct change is `headroom total seconds / CAS total seconds - 1` for matched
work. Generated source trees, binaries, logs, LLVM/assembly and raw recordings
remain ignored. Git retains the small generator, tests and compact aggregates.
