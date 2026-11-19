# Verification and reproduction

Start with the selectable local runner. Its default is a quick check; full
reproduction is an explicit choice. It never starts remote CI or benchmarks.
The [Splice batch report](vec-splice-batch.md#verification) records the latest
four-profile source/link certificate. Historical reports identify their
original baselines.

```sh
bash scripts/verify.sh --list
bash scripts/verify.sh --check runner
bash scripts/verify.sh --area btree-set --check tests --check miri --plan
bash scripts/verify.sh --area btree-set --check api --check tests
bash scripts/verify.sh --area hash-set --check api --check tests
bash scripts/verify.sh --area hash-set --check kani
bash scripts/verify.sh --area binary-heap --check api --check tests
bash scripts/verify.sh --area binary-heap --check verus --check models
bash scripts/verify.sh --area deque --check api --check tests
bash scripts/verify.sh --area deque --check verus --check kani
bash scripts/verify.sh --area deque --profile memory
bash scripts/verify.sh --area scratch-slice --check tests --check verus
bash scripts/verify.sh --area scratch-slice --check models
bash scripts/verify.sh --area scratch-slice --profile memory
bash scripts/verify.sh --area vec --check miri --check miri-tree
bash scripts/verify.sh --area arithmetic --check verus
bash scripts/verify.sh --area vec --check verus
bash scripts/verify.sh --area vec --check direct-source
bash scripts/verify.sh --area budget --check verus
bash scripts/verify.sh --area budget --check direct-source
bash scripts/verify.sh --area shared --check loom
bash scripts/verify.sh --area shared --check small-counts
bash scripts/verify.sh --check sbom
bash scripts/verify.sh --check inventory
bash scripts/verify.sh --area btree --check models
bash scripts/verify.sh --check link
bash scripts/verify.sh --profile full --plan
bash scripts/verify.sh --profile full
```

Repeat `--check` to select independent checks; it overrides `--profile`.
The runner rebuilds verification-module bytecode caches in checked-hash mode:
same-size edits within one timestamp tick are validated against source contents.
Verification rejects Python's `-O`/`PYTHONOPTIMIZE` mode because its assertions are
part of the checker logic. The runner regressions exercise both conditions.
`--area` selects test targets. Whole-library checks such as Clippy and link
roots remain identified as global checks. A combined certificate requires
`--area all`. `--keep-going` records independent failures and continues;
`--timeout SECONDS` bounds each command. A missing tool, failure, timeout,
cancellation or unrun job never counts as a pass. `--plan` executes nothing.

The common runner replaces the separate String, shared-owner and B-tree shell
drivers. Their reduced-count tests, deep-tree fuzz seeds, ledger dependency
audit and software bill of materials (SBOM) generation remain available here.
For B-tree fuzzing, `--check fuzz` creates the 12/144/1,200-entry seed corpus
under the selected output directory and retains the 16,384-byte input limit.
`--check sbom` writes CycloneDX JSON to that directory.

Each run creates a fresh timestamped directory under `target/verification/replay/`.
`--output target/verification/replay/my-check` selects a fresh named directory.
The incrementally written `summary.json` lists exact commands, environment
overrides, status, exit code, elapsed time, evidence kind and log paths. Existing
receipts are not overwritten. The receipt records the Git revision, dirty state
and input fingerprint; a source change during the run invalidates its overall
pass. A passing partial receipt covers only its selected
checks; it does not certify the entire product.

`--check inventory` independently parses the current Rust source with the
verification-only `syn` tool. It prints counts by site kind and saves every
file, function and location in `inventory.log` (JSON). It needs neither Verus
nor reviewed source fingerprints. These are syntactic unsafe/metadata sites,
not independent theorem counts or the certificate's routed obligations.

## Snapshot regression checks

Export the committed tree to start a repository with its own history.
Use the destination directory for the initial commit, with the Git identity
you want to publish.

```sh
snapshot_dir=$(mktemp -d)
git archive --format=tar HEAD^{tree} | tar -xf - -C "$snapshot_dir"
git -C "$snapshot_dir" init --initial-branch=main
git -C "$snapshot_dir" add .
git -C "$snapshot_dir" commit -m 'Initial jzap-std snapshot'
```

Install the applicable [tools](#tool-requirements) and run the maintained
checks from the snapshot checkout. Keep its initial commit as the source
baseline for subsequent performance comparisons.

```sh
cargo test --locked --all-features
bash scripts/verify.sh --area vec --check tests --check release --check overflow
bash scripts/verify.sh --area all --profile full --check certificate
```

The Vec benchmarks cover ExtractIf and Splice with equal input, work and
results across implementations. The initial run uses `HEAD` from a clean
snapshot checkout for the identical-source baseline, while std provides
the performance reference. Later runs pass a previously accepted commit in
the new repository as `--baseline` and use the current checkout as the
candidate. Refer to the [timing instructions](../../tools/throughput-analysis/README.md#snapshot-vec-timing)
for commands and how to read the controls and comparisons.

Historical reports keep their original revision identifiers and measurements.
Their pinned experiments require the original source and toolchain. A snapshot
contains the current source and the retained summaries.

## What each form of evidence establishes

| Evidence | Actual input and guarantee | Boundary |
| --- | --- | --- |
| Direct Verus | Original primitive integer implementations, bounded arithmetic, shared-byte admission, clone/table/heap/deque/scratch/slice geometry, and `Checked` representation plus Compaction/Extraction methods | Declared arithmetic and collection-state preconditions still need caller proofs. Atomic schedules and allocation ownership are separate. |
| Kani | Actual compiled Rust harnesses, including numeric and heap operations | Each harness's inputs, preconditions and unwind bounds apply. |
| Projected Verus models | Remaining arithmetic, prefix, caller/credit and domain models, source correspondence and negative controls | The Python translation and correspondence checks remain trusted for these remaining models. The migration table identifies replaced groups. |
| Miri / ASan / fuzzing | Executed product memory paths and differential scenarios | Executed cases and explored schedules, not every possible input. |
| Compiler and link checks | Type/lifetime boundaries and concrete retained termination edges | Selected targets, generic instantiations and call graph; retained guards need their source/caller evidence. |

`--check verus` runs the direct path. The maintained Rust entry files in
`tools/direct-verus/` include product modules by `#[path]`; Python only launches
Verus and records its JSON. No Rust function body is extracted, substituted or
generated. `cfg_attr(verus_keep_ghost, verus_spec(...))` attaches specifications
to those same functions. Ordinary stable Cargo builds erase the proof attributes
and ghost code and acquire no Verus/vstd runtime dependency.

`--check direct-source` runs the migrated foundation groups. Select one with
`python3 scripts/verify_source.py --group state --mutations` (or `arithmetic`, `slot`, `clone`, `budget`, `vec-geometry`, `vec-conversion`, `layout`, `vec-queries`).
The state check reads `src/lib.rs` itself; maintained adapters call the product
methods for the pending caller models. They do not copy method bodies. The
[migration table](../direct-source-migration.md) lists completed and pending groups.

All direct targets use `--no-cheating`, retaining lifetime and erasure checks.
Their optional source mutations include wrong arithmetic, a reversed budget
guard, a newly inserted panic, incorrect table-size/mirror/probe operations and
heap parent/child/selection calculations;
the runner rejects compiler errors as proof
counterexamples. Reproduce one target without the other:

```sh
JZAP_VERUS=/path/to/verus python3 scripts/verify_direct_verus.py --target bounded --mutations
JZAP_VERUS=/path/to/verus python3 scripts/verify_direct_verus.py --target integer --mutations
JZAP_VERUS=/path/to/verus python3 scripts/verify_direct_verus.py --target budget-admit --mutations
JZAP_VERUS=/path/to/verus python3 scripts/verify_direct_verus.py --target hash-map --mutations
JZAP_VERUS=/path/to/verus python3 scripts/verify_direct_verus.py --target binary-heap --mutations
```

The same product-source proof can also run without Python:

```sh
export JZAP_VERUS=/path/to/verus
"$JZAP_VERUS" tools/direct-verus/budget-admit.rs \
  --crate-type lib --crate-name direct_product --edition=2024 \
  --cfg 'feature="alloc"' --no-cheating --output-json
```

Use `tools/direct-verus/bounded.rs` for the arithmetic target. The Python
launcher adds receipts and optional negative controls to these same commands.

The annotated product crate, including the original state methods, can be
checked directly without Python or caller projections:

```sh
"$JZAP_VERUS" src/lib.rs --crate-type lib --crate-name jzap_direct \
  --edition=2024 --cfg 'feature="alloc"' --no-cheating --output-json
```

This checks the annotated items. Unannotated product functions are outside that
command's direct proof scope.

Existing projected ownership, B-tree and private core-guard models remain
available through `--check models`. Their source-binding and mutation checks
reduce drift; they do not remove the translation gap. A combined source/link
receipt composes these different forms of evidence and is not a direct Verus
proof of every Rust function. A source-obligation count counts routed source
sites, often sharing contracts; a fingerprint identifies inputs. Neither number
is a measure of proof strength.

Direct verification of the remaining pointer code requires explicit memory
ownership specifications and supported pointer operations in Verus. Merely
including an unannotated Rust module does not prove it. We retain the existing
model and executable checks during migration; we do not replace a Python
projection with an assumed or unchecked Rust function. `btree_core.py` also
inspects pinned **std** private implementation code, which these product
annotations cannot directly verify.

After `--check certificate`, the generated `source-obligations.json` lists every
counted source site and its contract/disposition. Its row count can be recomputed:

```sh
python3 - <<'PY'
import collections, json
from pathlib import Path
run = Path('target/verification/replay/my-check/certificate')
rows = json.loads((run / 'source-obligations.json').read_text())
print('source sites:', len(rows))
print('dispositions:', dict(collections.Counter(r['disposition'] for r in rows)))
print('distinct contracts:', len({r['contract'] for r in rows}))
PY
```

## Reports

Git retains current result summaries, interpretation, reproduction commands, and
source commit identifiers. Raw benchmark samples, logs, coverage dumps, assembly,
LLVM IR, and generated proof/mutation inputs are disposable local outputs under
`target/verification/`. Verification strength does not depend on committing those
outputs. Earlier source states belong in Git history, not `before/` directories.

| Report | Scope |
| --- | --- |
| [VecDeque guide and results](../vec-deque-design.md) | Stable API counterparts, ring/ownership contracts, direct source checks and selective reproduction |
| [VecDeque timing](vec-deque-timing.csv) | 234 matched layout/seed rows, 1,170 long intervals; all 39 predeclared conditions |
| [VecDeque iteration alignment](vec-deque-iteration-alignment.csv) | 120 long intervals comparing default/32-byte options with matched counts, two layouts and small/mixed inputs |
| [Copy layout control](vec-deque-copy-layout.csv) | Same-kernel, fixed-address 256-byte copies with four scratch offsets; maintained C reproduction |
| [Scratch timing records](scratch-slice-timings.csv) | 480 matched intervals of at least one second; three seeds, two layouts and complete operation counts |
| [Scratch code generation](scratch-slice-codegen.csv) | 81 matched intervals comparing original, common 32-byte alignment and revised source |
| [Scratch allocation records](scratch-slice-memory.csv) | 40 isolated policy/width/length observations with requested and allocator-usable peaks |
| [Scratch copy layout](scratch-slice-copy-layout.csv) | 12 one-second-or-longer recordings of the shared controlled temporary-copy kernel |
| [VecDeque allocation aggregates](vec-deque-memory.csv) | 27,648 observations summarized into 108 policy/task rows |
| [BinaryHeap guide and results](../binary-heap-design.md) | Stable API counterparts, returned errors, ownership rules, source checks, std comparisons and selective reproduction |
| [BinaryHeap timing](binary-heap-timing.csv) | 360 selected matched variant/seed groups / 1,800 long intervals; complete current default/aligned matrix |
| [BinaryHeap control screen](binary-heap-control-screen.csv) | 12 initial groups replaced only for identical-std drift; complete replacements are in the current timing file |
| [BinaryHeap append comparison](binary-heap-append-comparison.csv) | 24 matched groups / 120 intervals comparing original and storage-reuse implementations |
| [BinaryHeap allocation aggregates](binary-heap-memory.csv) | 23,040 observations summarized into 90 policy/task rows |
| [Arc headroom experiment](arc-headroom.md) | Strong-clone locked-increment prototype; bounded tests, actual saturation counterexample and performance decision; product unchanged |
| [Arc headroom timing aggregates](arc-headroom-benchmarks.csv) | 48 means from 200 long intervals, direct CAS comparisons and an unstable read-heavy repeat |
| [Arc application workflows](arc-scenarios.md) | 19 workflow conditions, 475 long intervals, paired uncertainty, actual allocation costs and suitability judgments |
| [Arc workflow timing aggregates](arc-scenario-benchmarks.csv) | 76 primary/follow-up means, matched controls, process CPU/RSS and bootstrap intervals |
| [Arc workflow allocation aggregates](arc-scenario-memory.csv) | 28 requested/usable held and peak observations, including owned-domain bootstrap |
| [Owning budget verification](owned-budgets.md) | Lifetime-free stored policies, bootstrap accounting, actual failure/concurrency paths, and that campaign’s combined certificate |
| [Owning budget benchmark aggregates](owned-budget-benchmarks.csv) | 64 means from 240 intervals; paired borrowed/owning/std construction, hot handles, COW and slices |
| [Policy benchmark aggregates](policy-benchmarks.csv) | 142 means from 531 intervals; source baseline, initial migration and ledger-capture fix |
| [Policy ownership verification](policy-duplication.md) | Fallible policy duplication, non-Copy ownership/refund oracles, renewed source-to-link certificate and before/after/std timings |
| [BTreeSet guide and current verification](../btree-set-design.md) | Stable functionality, actual Rust kernels, source/link and memory boundaries, std time/allocation results and reproduction |
| [BTreeSet timing aggregates](btree-set-benchmarks.csv) | 128 policy/control means from 480 valid long intervals, preserving totals, call counts and seed spread |
| [BTreeSet allocation aggregates](btree-set-memory.csv) | 13,824 observations summarized into 54 policy/operation rows |
| [HashSet guide](../hash-set-design.md) | Stable API contracts, selective checks, code generation and std timing/allocation reproduction |
| [HashSet timing records](hash-set-branch-optimization.csv) | Compact seed records with call counts and matched std/control seconds; independently recomputable means |
| [HashSet unequal-size controls](hash-set-disjoint-shapes.csv) | Shared-set current/prior-expression and matching-hasher controls for skewed/variable sizes and mixed argument orientation |
| [HashSet allocation aggregates](hash-set-memory.csv) | 9,216 observations summarized into 36 policy/operation rows |
| [BTreeMap verification](btree.md) | Ordered map and borrowed node-slice checkpoint; source/guard checks and std comparisons |
| [BTreeMap benchmark aggregates](btree-benchmarks.csv) | 80 means from 300 valid intervals; all sixteen operations and four small-input conditions |
| [BTreeMap alignment aggregates](btree-alignment.csv) | 48 means from 180 valid intervals, preserving initial controls and independent repeats |
| [BTreeMap allocation aggregates](btree-memory.csv) | 12,288 observations summarized into 48 policy/operation rows |
| [Shared ownership verification](shared.md) | Rc/Arc/Weak, concurrent budgets, source-connected proofs, concurrency and std comparisons |
| [Shared benchmark aggregates](shared-benchmarks.csv) | 144 means from 540 valid intervals; counter, value and dynamic-slice workloads |
| [Shared integration rechecks](shared-integration-benchmarks.csv) | 26 means from 96 Box/Vec/String intervals after accounting integration |
| [Shared independent repeats](shared-rechecks.csv) | Identical-std control and default/32-byte-aligned String repeats, preserving initial results |
| [Shared allocation aggregates](shared-memory.csv) | Requested/allocator-observed held and peak bytes, calls and layouts |
| [String implementation and verification](string.md) | Stable API coverage, UTF-8 editing, source/guard checks and equal-work performance |
| [String benchmark aggregates](string-benchmarks.csv) | 75 policy/control means from 270 valid intervals; totals, call counts and seed ranges |
| [String allocation aggregates](string-memory.csv) | Requested and actual allocator observations, including handle sizes |
| [Vec optimization checkpoint](vec-optimization.md) | November 4 baseline for primitive copies, dedup, the source/link certificate and optional alignment |
| [ExtractIf default-build timing](vec-extract-inline.md) | Normal inlining, assembly and adjusted std comparisons for 12 conditions |
| [Splice batch verification and timing](vec-splice-batch.md) | Four-profile source/link certificate, remaining small-input performance gaps and pinned reproduction |
| [Vec proof migration status](vec-projection-removal.md) | Original-source diagnostic checks and the remaining nine Vec caller projections |
| [Vec benchmark aggregates](vec-benchmarks.csv) | 185 policy/control means with totals, call counts and seed ranges; raw samples excluded |
| [Long-interval operation means](throughput-averages.md) | Standard mean reporting from validated one-second-or-longer repeats; operation-weighted averages and per-policy tables |
| [Randomized total-time comparison](throughput-long.md) | November 4 one-second-or-longer recordings, equal randomized work, total seconds, policy/flag comparisons and remaining overhead |
| [Box and Vec performance recheck](performance-recheck.md) | Preceding fixed-input comparisons, collection fix, alignment options, allocation measurements and remaining Vec failures |
| [Internal unsafe verification: Plan A](unsafe-proof-plan.md) | Verification plan, model assumptions and division of memory-tool checks |
| [Wrapping arithmetic and memory](wrapping-memory.md) | All internal modular call families, arbitrary input bounds, source-linked byte extents, actual heap probes, and overrun controls |
| [Iteration verification](iteration.md) | Executed checks, generated-code equivalence, splice timing and memory results |
| [Fallible iteration and consumer checks](../iteration.md) | Open iterator traits, implementation contracts, and consumer final-link checks with negative controls |
| [Arithmetic policy](../arithmetic-policy.md) | Checked operators, bounded explicit wrapping, type-based lint controls, and current link/performance decisions |
| [Checked metadata](../checked-metadata.md) | Transparent integers and closed capacity/cursor/compaction states; verification scope and std comparison |
| [Fallible arithmetic](../arithmetic.md) | Five checked operators, ten primitive methods, all integer widths, link and codegen controls |
| [Internal panic-path certificate](panic-paths.md) | Fresh source-to-link gate, retained debug guard origins, controls and verification boundaries |
| [Vec report](vec/README.md) | Element limits, storage/editing tests, proof boundaries, and performance |
| [Box checkpoint](box-completion/README.md) | API coverage, executed checks, and provisional performance |
| [Box result summary](box-completion/summary.json) | Counts, toolchain, and source commit |
| [Memory proof boundaries](memory-proof/README.md) | Kani limitations, Verus supplementation, and trust boundaries |
| [1MiB clone analysis](box-str-clone-cause/README.md) | Actual execution path and measured uncertainty |
| [Benchmark methodology](../benchmarks.md) | Conditions and the additional reference-fixture prerequisite |

## Tool requirements

Only the tools needed by the selected checks are required. The `full` profile
replays safety/functional verification; performance campaigns remain separate.
The reference environment is Linux x86-64 with Python 3.12.

Checks use Cargo's offline mode after dependencies are fetched. On a fresh clone,
fetch the locked sources needed by the chosen checks first:

```sh
cargo fetch --locked
# Additional verification workspaces, when selected:
cargo fetch --locked --manifest-path tools/panic-audit/Cargo.toml
cargo fetch --locked --manifest-path tools/shared-loom/Cargo.toml
cargo fetch --locked --manifest-path tools/consumer-audit/Cargo.toml
cargo fetch --locked --manifest-path fuzz/Cargo.toml
```

| Checks | Requirements |
| --- | --- |
| Native tests / API / Clippy | Rust 1.95.0, Cargo, rust-src for the pinned API inventory, locked dependencies |
| Miri / ASan | nightly-2026-10-30; Miri and rust-src components; i686 target support for the 32-bit check |
| Coverage / fuzz | cargo-llvm-cov, llvm-tools-preview, cargo-fuzz and a C/C++ compiler |
| Direct Verus / models | Verus `0.2026.11.02.8bf68ee`, its bundled Z3/vstd, and its required Rust 1.99.1 toolchain; set `JZAP_VERUS` outside the managed environment |
| Kani | kani-verifier 0.68.0 with its setup completed; Z3 for arithmetic harnesses |
| Link checks / certificate | Linux ELF tools: C compiler, objdump and nm, plus the relevant formal tools above |
| Target builds | wasm32-unknown-unknown and thumbv7em-none-eabi Rust targets |
| Dependency audit | cargo-audit with an advisory database, cargo-deny and locked dependency sources |
| SBOM | cargo-cyclonedx |

The pinned Linux x86-64 Verus installer verifies the release archive checksum
and installs its required compiler. Pinned core-source checks also need the
normal Cargo compiler's Rust sources:

```sh
python3 scripts/setup_verus.py
rustup component add rust-src --toolchain 1.95.0
```

The installer prints `JZAP_VERUS` for installations outside the managed path.
Source-scanner readers build their locked Rust executables on demand. Partial
checks do not require a previous full run to populate `target/panic-inventory`.

Verus attribute syntax follows its [pinned official example](https://github.com/verus-lang/verus/blob/8bf68ee741df833ea5d0fe6a2255126fdea87e47/examples/syntax_attr.rs).
The Verus compiler and normal Cargo compiler have different versions; direct
proof means they read the same product function bodies, not that their compiler
implementations are identical. The runner records Verus's actual version and
toolchain in each direct result. Normal builds retain the declared Rust 1.95 MSRV.

The root crate's tests do not require the separate reference benchmark fixture.
Recorded tools include Rust 1.95, nightly-2026-10-30, Kani 0.68.0, and Verus
0.2026.11.02.8bf68ee. Install the relevant targets and tools before running:

The arithmetic Kani harnesses use Z3; put `z3` on `PATH`. The arithmetic runner
also searches beside the executable selected by `JZAP_VERUS`.

```sh
cargo test --locked --all-features
cargo +nightly-2026-10-30 miri test --locked --all-features --test foundation --test box_complete
cargo kani
JZAP_VERUS=/path/to/verus python3 scripts/verify_panic_paths.py
JZAP_VERUS=/path/to/verus python3 scripts/verify_owner_protocol.py --mutations
python3 scripts/check_unsafe_inventory.py
python3 scripts/verify_thread_bounds.py --mutations
python3 scripts/check_memory_controls.py
JZAP_VERUS=/path/to/verus python3 scripts/verify_direct_verus.py --target init-prefix --target buffer --target local-budget
JZAP_VERUS=/path/to/verus python3 scripts/verify_source.py --group budget --group init-prefix --mutations
python3 scripts/mutation_check.py
```

`bash scripts/verify.sh` defaults to the quick profile. Timing comparisons run
separately using the maintained [benchmark instructions](../benchmarks.md),
after compilation and proof jobs finish; they are not part of the replay runner.
`bash scripts/verify_extended.sh` records enhanced Kani outcomes and continues
independent checks. Nonzero, unsupported, and timeout outcomes remain distinct
from successful proofs. These scripts write to `target/verification/`.

## Continuous integration

The GitHub Actions workflow uses Rust 1.95.0 on Linux, macOS, and Windows. Each
runner checks the core-only build and runs tests with default and all features.
The Linux runner also checks formatting, runs Clippy on the core-only and
all-features library builds, and builds API documentation. Clippy and rustdoc
warnings fail their checks.

A separate CodeQL workflow scans Rust source on pull requests targeting `main`
and pushes to `main`. It uploads analysis results to GitHub Code Scanning.
[CodeQL workflow](../../.github/workflows/codeql.yml)

Use these exact check names for GitHub rulesets under **Require status checks to
pass**. Select **GitHub Actions** as the source.

| Required check name | What it runs |
| --- | --- |
| `Rust (ubuntu-24.04)` | Native build, tests, formatting, Clippy, and rustdoc |
| `Rust (macos-15)` | Native build and tests |
| `Rust (windows-2025)` | Native build and tests |
| `Miri` | `unsafe_paths`, `vec`, `shared`, and `scratch_slice` with strict provenance on x86-64 and i686, plus Tree Borrows on x86-64 |
| `Verus` | The direct product-source proof suite and its negative controls |
| `Kani` | The foundation, arithmetic, Vec, and budget harness groups, including the local-budget proof and its negative control |
| `Full certificate` | Composes the source proofs, negative controls, four Linux x86-64 final-link profiles, and B+ consumer checks |

The verification checks run on Ubuntu. Broader Miri and Kani runs, sanitizers,
and fuzzing are available through the local commands above.
[CI workflow](../../.github/workflows/ci.yml)

The **Source certificate** workflow runs the certificate stages in parallel on
pull requests targeting `main` and pushes to `main`. The B+ stage checks the
initialized prefix of the borrowed lookup's ancestor stack, reviewed debug
panic callers, allocation refusals, two checked release link profiles, five
Kani harnesses, and four B+ integration tests under Miri on x86-64 and i686.

The `Full certificate` job checks each stage's evidence against the checked-out
commit, input fingerprint, and Actions run attempt before matching retained
panic callers to proved guards. It publishes `summary.json` and
`source-obligations.json` as the `full-certificate` artifact. Select **Re-run
all jobs** to retry so every stage produces evidence in the new attempt.
[Source certificate workflow](../../.github/workflows/certificate.yml)
