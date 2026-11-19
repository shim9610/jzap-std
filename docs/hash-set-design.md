# Fallible HashSet

The set owns an existing `HashMap<T, (), S, P>` and retains its table
geometry, entropy, byte policies and Copy-clone capabilities. No product
dependency is added.

The branch-based `is_disjoint` optimization is implemented in source `fcfc517`.
Functionality, runtime memory and fresh source/guard verification pass within
the scopes below. All 33 unmetered timing means are within the observed std
+15% criterion; optional policy costs remain separate.

## References and scope

The API inventory uses Rust 1.95.0 (`59807616e`), whose `hash/set.rs` SHA-256 is
`73705da3a147f9d764e4c09160d1f17705be0abf0208660fd1a0a96214905ec4`.
The nightly-2026-10-30 reference (`92b5b8c87`) has SHA-256
`485cf4323c9205d54348162fb7862f8174cfe90679795681b74a3581a064fd0b`.
Both keep Entry and `get_or_insert*` unstable. They are not included in stable
support. The cached hashbrown 0.16.1 set implementation is a reference for
single-search replacement and choosing the smaller side for intersection and
the filtered part of union. Its storage implementation remains outside product
dependencies; the existing local map supplies storage. The pinned inventory
accounts for all 30 distinct stable inherent method names and explicit
counterparts for allocating traits.

## Contracts

| Operation | Explicit behavior |
| --- | --- |
| Default construction | `try_new`, `try_with_capacity`, `TryDefault`; entropy errors are returned, without a fixed-seed fallback |
| Explicit builder/policy | `with_hasher`, `with_hasher_in` and fallible capacity variants; all existing borrowed/owned local/concurrent policies |
| Insert | `try_insert` returns whether a new object was stored; a duplicate retains the stored object and destroys the incoming duplicate |
| Replace | `try_replace` returns the previous equal object; an existing match does not allocate or change capacity; failed insertion returns the incoming owner |
| Lookup/removal | Borrowed heterogeneous `get`, `contains`, `take`, `remove`; stored keys are not exposed mutably |
| Growth/shrink | `try_reserve`, `try_shrink_to`, `try_shrink_to_fit`; allocation/budget refusal preserves contents, capacity and accounting |
| Retain/extraction | `Result` callbacks; earlier removals remain on error, the current value is retained, and extraction consumes that visit; dropping extraction retains unvisited objects |
| Borrowed algebra | Union, intersection, difference, symmetric difference, subset/superset/disjoint; no allocation |
| Owned algebra | `try_union`, `try_intersection`, `try_difference`, `try_symmetric_difference`; clone the left builder/policy and publish a private result; failure preserves both inputs |
| Clone | `TryClone`, transactional `try_clone_from`, target-policy `TryCloneIn`; partial clones clean only their private prefix |
| Construction/extension | Explicit array/fallible-source construction and `try_extend`/`try_extend_copied`; errors retain the successful destination prefix |
| Iteration | Borrowing, consuming and draining exact/fused standard iterators and fallible mirrors; algebra iterators are fused with conservative size hints |
| Traits | Equality across builders/policies, debug formatting and inferred thread/lifetime bounds; allocating implicit `Default`, `Clone`, collection/extension and set operators are replaced by explicit methods |

Explicit keys via `RandomState::from_seed`, or a caller-selected builder, work
without `std`. Default OS entropy currently uses the existing Linux `std`
implementation; unsupported sources return `Entropy`. Allocator and returning
Hash/Eq/clone/iterator/destructor callbacks retain the documented execution
contract. The internal map remains private.

The control array includes at least one byte per bucket plus 16 mirror bytes.
The admitted layout bounds each table's bucket count by `isize::MAX - 16`;
length is no greater than that count. Even the sum of two set lengths is below
`usize::MAX`. The standard iterator count bridge therefore remains exact when
using a total saturating increment: saturation is unreachable for these valid
outputs. The all-word Kani kernel checks this bound and increment, while the
linked fixtures check that the inherited addition-panic guard is gone.

## Verification scope

The wrapper adds no unsafe block. Equal-key replacement adds one occupied-slot
write inside the existing map; it is checked as actual Rust by Kani and through
native/i686/Tree Borrows Miri, sanitization, failure injection and differential
scenarios. Borrowed algebra uses the map's finite exact iterators. Its original
generic filter/chain bodies are checked by Kani over arbitrary byte
triples and prefix lengths; all-word hint/count bounds are checked separately.
The complete set membership/algebra behavior is exercised by differential,
property and fuzz scenarios. Source review seals invalidate changed glue; they
are not semantic proofs. No Python projection of a positive set body is used.
Existing direct-source Verus geometry remains part of the map prerequisite.
Deliberate defects are rejected, and final set consumer roots are included
in release/LTO/overflow-checked/debug source/link composition.

The selective verification runner exposes `--area hash-set`. Benchmarks use
the existing >=1-second equal-work, three-seed protocol with genuine std default
builders, repeated identical-std controls and operation-weighted means. The
unmetered comparison uses the observed +15% criterion; optional local/concurrent
ledger costs are shown separately. Requested/allocator-usable held/peak bytes
are checked independently. Raw logs, generated inputs and assembly stay in
ignored `target/`; only compact results and reproduction commands enter Git.

## Verification checkpoint

The fresh combined run regenerated prerequisites without model reuse and kept
its product/proof inputs unchanged throughout. Product Rust is `fcfc517`.
Both Map and Set native suites pass with and without the std feature. The
whole-crate test and documentation-example suite passes 320 tests.

| Evidence | Checked scope | Result |
| --- | --- | --- |
| API and native scenarios | 30 pinned stable set names; 18 set and 23 map scenarios, including properties, failures, ownership, algebra and unique references | Pass |
| Actual Rust Kani and negative controls | Occupied replacement on a four-slot table, all-word hint/count bounds and generic filter/chain bodies over arbitrary byte triples/prefixes | Three kernels pass; three Kani counterexamples and two runtime regressions reject all five compiled defects |
| Inherited map proofs | Seven actual Rust map/scan kernels, direct-source table-size/mirror/probe Verus and explicit phase/caller models | Pass |
| Runtime memory | Native/i686 strict-provenance Miri, native Tree Borrows, AddressSanitizer | Pass |
| State and compiler boundaries | 10,000 fuzz runs, differential/property scenarios, coverage and lifetime/Send/Sync/Copy-capability probes with weakening controls | Pass |
| Build and dependencies | Alloc-only/core, wasm/thumb, no-global-OOM, release/overflow checks, strict lints/rustdoc, security/license audit and software bill of materials | Pass |
| Source and final linking | Fresh release/LTO/overflow-checked/debug composition; eight deliberate exit controls | Zero open source conditions or unproved linked exit edges; all controls detected |

The bare link gate reports retained guards. The combined check attaches their
source/caller proofs; it does not equate a retained symbol with a reachable
library failure. The final inventory is:

| Profile | Originating guards | Forwarding calls | Unproved edges |
| --- | ---: | ---: | ---: |
| Release | 3 | 0 | 0 |
| Release with LTO | 6 | 0 | 0 |
| Overflow-checked release | 3 | 0 | 0 |
| Debug | 58 | 5 | 0 |

These are checked Linux x86-64 consumer roots and source contracts, including
inherited numeric/state projections. Actual Rust kernels, direct Verus bodies,
reviewed correspondence and executed provenance/initialization/drop scenarios
remain distinct forms of evidence. Allocator, compiler/core, public-unsafe and
returning user-callback contracts remain the execution boundary.

## Selective reproduction

Use the pinned Rust 1.95 toolchain for native builds. Memory checks use
nightly-2026-10-30; the actual-code proofs use Kani 0.68.0 and Verus
0.2026.11.02.8bf68ee. The runner records the selected commands, versions, status
and source identity, and refuses to label a partial selection a full certificate.

```sh
bash scripts/verify.sh --area hash-set --profile quick
bash scripts/verify.sh --area hash-set --check api --check release --check overflow
bash scripts/verify.sh --area hash-set --check miri --check miri-32 --check miri-tree --check asan
bash scripts/verify.sh --area hash-set --check fuzz --check coverage --check bounds
bash scripts/verify.sh --area hash-set --check builds --check docs
python3 scripts/verify_hash_set.py --kani --mutations
bash scripts/verify.sh --area hash-map --check verus
bash scripts/verify.sh --area all --check certificate
```

The last command rebuilds prerequisites and the four final-link profiles.
Development-only reuse switches cannot produce a complete certificate. The set
source checker verifies an exact file domain, reviewed safe delegation and the
absence of unsafe set-wrapper operations before the common owner checker defers
to its separate proof domain. New files or modified delegation require review.

## Timing and allocation reproduction

Build both executables and finish verification before recording times. The
timing executable has no allocation observer. Run the observer separately.

```sh
cargo build --locked --offline --release --manifest-path tools/throughput-analysis/Cargo.toml --features hash-set --bin hash_set --target-dir target/verification/hash-set/inline-build
cargo build --locked --offline --release --manifest-path tools/throughput-analysis/Cargo.toml --features hash-set,btree-memory --bin hash_set --target-dir target/verification/hash-set/memory-inline-build
python3 tools/throughput-analysis/test_ordered.py --binary target/verification/hash-set/inline-build/release/hash_set
python3 tools/throughput-analysis/compare.py --binary default=target/verification/hash-set/inline-build/release/hash_set --small-tasks hash_set_build,hash_set_get,hash_set_clone,hash_set_intersection,hash_set_is_disjoint,hash_set_default_new --small-variants default --output target/verification/hash-set/performance-final
python3 tools/throughput-analysis/report.py --input target/verification/hash-set/performance-final
target/verification/hash-set/memory-inline-build/release/hash_set memory >target/verification/hash-set/memory-final.jsonl
python3 tools/throughput-analysis/memory_report.py --input target/verification/hash-set/memory-final.jsonl --output target/verification/hash-set/memory-final.csv --format hash-set --max-ratio 1.15
```

The timing corpus contains 64 shuffled seeded `u64` cases, sizes 0–4,096,
plus a 0–32 profile for six operations. Seeds are 617, 6203 and 96011. Both
implementations use genuine default builders; process-selected hash placements
vary. Equal-sized right sets have roughly half overlap. Additional intersection
cases are disjoint or have at most three right keys; predicates include true
subset and true disjoint cases. Input generation, source preparation and two
warmup epochs are outside the timer. Each selected interval lasts at least one
second; each seed group uses identical inputs, order and call counts. Means are
`sum(seconds) / sum(calls)`, with seed spread retained separately. Repeated
identical-std controls must have a mean ratio within 0.9–1.1.

`get` counts two lookups per source key; `replace` counts one existing-key
replacement; `remove_cycle` counts a remove/insert pair. Other rows count an
entire workload. Destructive batches include source cloning; `reserve` is
clone/growth and `shrink` is clone/growth/shrink. Owned algebra includes result
construction and destruction. Std's owned set operators create a default
builder, while the explicit product operations clone the left builder/policy.
Optional byte-budget costs are reported separately from the unmetered comparison.
`default_new` measures warmed, cached empty construction; cold entropy
initialization and entropy refusal have separate functional checks.

## Recorded allocation results

The renewed observer run on product source `fcfc517` uses the same map
allocation geometry as std for these `u64` workloads. Its aggregate rows are
byte-identical to the pre-optimization allocation checkpoint.
The [36 aggregate rows](verification/hash-set-memory.csv) cover 9,216 observations:
12 operations, 64 cases, three seeds and four implementations/policies. Requested held/peak bytes match in every case, and the maximum matched
allocator-usable held/peak ratio is 1.0000. In the reserve row, mean usable
held/peak ratios are slightly lower at 0.99874/0.99916; other means are 1.0000.
Allocation call counts also match, and all temporary allocations and budget
charges are released at teardown.

The observed Linux x86-64 handles are 48 bytes for std and the unmetered set,
56 bytes for the borrowed local/concurrent budget forms. These fixed handle
bytes are separate from heap ratios. The thread-local default-key cell is outside
the observer; these measurements concern table requests and allocator-usable
heap bytes, not resident-set size. Owning policies have functional checks but
are outside this timing/allocation campaign.

## Recorded timing results

Linux x86-64, AMD EPYC 9V74, Rust 1.95.0 / LLVM 22, release optimization,
one pinned CPU, no forced loop alignment. Product source is `fcfc517`.
The current matrix retains 540 intervals of at least 1.0183 seconds, totaling
693.21 measured seconds across 33 conditions. All 33 unmetered means are within
the observed std +15% criterion. These workload means include input, hash and
code-placement variation; they are not confidence-interval acceptance or
universal performance bounds. Optional policy means remain separate. Borrowed
algebra and predicates perform no library budget reservation or release, so
their policy-row differences are not additional ledger operation costs.

Most conditions use the original three seeds. Mixed `intersection` retains
those three plus nine additional, fixed seeds: 1013, 2027, 4051, 8089, 16193,
32377, 64709, 129403 and 258809. The initial mean was +27.6%, prompting an
unchanged-caller comparison with both immutable product binaries. All 92
workload caller instruction streams are identical after normalizing relocations;
their addresses move by 1,664 bytes and preserve offsets within 64-byte lines.
The added nine-seed after/std mean is +6.0%; the before/std mean is +1.1%.
Including every initial and additional after sample gives the reported
12-seed +11.5%. No original product observation is discarded. These observations
do not isolate hash placement, instruction-cache placement and execution-window
variation into individual cycle costs.

Repeated identical-std mean ratios in the current matrix span 0.92273–1.09822.
All primary and supplemental variant controls pass the 0.9–1.1 criterion.
The [compact CSV](verification/hash-set-branch-optimization.csv) holds 540 seed
rows: 324 current-matrix rows, 54 paired-product rows, 135 candidate-form rows
and 27 supplemental before-control rows. Each row stores calls, elapsed seconds
and its matched std/control seconds. Raw journals, binaries and disassembly
remain outside Git. Earlier tables and their records are available in Git at
checkpoint `eedd4e3`.

### Independent recalculation

This reads only the compact CSV; large raw journals are unnecessary to
recalculate the 33 current unmetered rows and their control ratios.

```python
import csv
from collections import defaultdict

groups = defaultdict(list)
with open("docs/verification/hash-set-branch-optimization.csv") as stream:
    for row in csv.DictReader(stream):
        if row["variant"] == "default" and row["label"] == "unlimited":
            groups[row["profile"], row["task"]].append(row)
for case, rows in sorted(groups.items()):
    calls = sum(int(r["calls"]) for r in rows)
    elapsed = sum(float(r["elapsed_seconds"]) for r in rows)
    standard = sum(float(r["std_elapsed_seconds"]) for r in rows)
    control = sum(float(r["std_control_elapsed_seconds"]) for r in rows)
    print(case, elapsed * 1e9 / calls, standard * 1e9 / calls,
          elapsed / standard, control / standard)
```

### Mixed sizes

| Operation | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: |
| `build` | 16,016.62 | 17,504.13 | +9.3% | 18,041.31 | 17,070.68 |
| `get` | 10.52 | 11.13 | +5.8% | 11.16 | 11.44 |
| `replace` | 11.28 | 11.08 | -1.7% | 10.94 | 10.84 |
| `remove_cycle` | 26.96 | 29.21 | +8.3% | 29.13 | 29.46 |
| `iter` | 392.32 | 390.53 | -0.5% | 407.15 | 428.18 |
| `clone` | 196.83 | 192.72 | -2.1% | 204.12 | 195.59 |
| `retain` | 1,757.19 | 1,790.90 | +1.9% | 1,779.95 | 1,804.58 |
| `extract` | 1,750.04 | 1,791.01 | +2.3% | 1,952.92 | 1,837.85 |
| `drain` | 706.30 | 624.13 | -11.6% | 622.03 | 637.47 |
| `into_iter` | 725.00 | 630.25 | -13.1% | 615.97 | 603.73 |
| `reserve` | 6,711.54 | 7,106.72 | +5.9% | 7,228.45 | 7,275.42 |
| `shrink` | 13,541.37 | 13,756.34 | +1.6% | 13,873.20 | 13,784.86 |
| `union` | 7,184.47 | 7,185.34 | +0.0% | 7,734.21 | 7,063.27 |
| `intersection` | 5,876.21 | 6,550.83 | +11.5% | 6,333.61 | 5,795.54 |
| `difference` | 6,300.11 | 6,185.83 | -1.8% | 5,928.69 | 6,168.02 |
| `symmetric_difference` | 16,122.70 | 17,612.36 | +9.2% | 17,760.76 | 20,107.43 |
| `is_subset` | 18.64 | 19.32 | +3.6% | 17.71 | 18.53 |
| `is_disjoint` | 19.89 | 19.00 | -4.5% | 17.94 | 20.71 |
| `default_new` | 3.15 | 3.58 | +13.4% | 3.56 | 3.60 |
| `owned_union` | 21,717.78 | 23,751.70 | +9.4% | 25,302.04 | 23,982.99 |
| `owned_intersection` | 17,667.78 | 17,543.79 | -0.7% | 17,111.11 | 17,165.07 |
| `owned_difference` | 17,080.43 | 18,658.59 | +9.2% | 17,580.36 | 17,775.76 |
| `owned_symmetric_difference` | 34,444.44 | 37,596.20 | +9.2% | 37,152.29 | 37,355.48 |
| `intersection_disjoint` | 5,209.58 | 5,573.87 | +7.0% | 5,106.13 | 5,216.65 |
| `intersection_skew` | 32.77 | 31.07 | -5.2% | 31.44 | 31.42 |
| `is_disjoint_true` | 4,984.24 | 5,225.91 | +4.8% | 5,268.88 | 5,362.79 |
| `is_subset_true` | 6,081.28 | 5,826.43 | -4.2% | 6,062.17 | 5,757.43 |

### Small sizes

| Operation | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: |
| `build` | 210.68 | 220.25 | +4.5% | 236.11 | 229.42 |
| `get` | 11.14 | 10.39 | -6.7% | 11.90 | 11.81 |
| `clone` | 19.97 | 18.25 | -8.6% | 20.45 | 22.72 |
| `intersection` | 68.50 | 64.29 | -6.1% | 66.31 | 64.10 |
| `is_disjoint` | 17.26 | 16.99 | -1.6% | 17.78 | 16.87 |
| `default_new` | 3.23 | 3.22 | -0.4% | 3.65 | 3.50 |

## Disjoint scan code generation

The earlier direct Boolean scan removed an outlined resumable-filter boundary.
The current change keeps the size relation explicit in two branches:

```rust
if self.len() <= other.len() {
    self.iter().all(|key| !other.contains(key))
} else {
    other.iter().all(|key| !self.contains(key))
}
```

The former reference-selection form generated two conditional pointer moves,
a repeated length comparison, a selected-length reload and a separate check
for an empty lookup table. In the measured u64 specialization, the branches
expose that a nonempty smaller source implies a nonempty lookup table; LLVM eliminates that redundant
check and reference selection. This is compiler optimization of ordinary safe
control flow, without an added assumption. Scanner end bounds and bounded probe
counters are retained. Per-key hash-state spills still exist. The benchmark's
executable text grows from 612,978 to 614,650 bytes (+0.27%); heap geometry is
unchanged. No padding, forced alignment or new unsafe block is introduced.

### Candidate forms and independent product comparison

Six same-path forms were prepared from baseline `eedd4e3`. `try_for_each` is
byte-identical to the baseline and is not timed again. The five other forms
use 225 selected >=1-second intervals, three seeds, equal work and passing
repeated std controls. The following are each candidate's matched std changes,
separate from the current product matrix above.

| Form | Mixed early exit | Mixed full scan | Small early exit |
| --- | ---: | ---: | ---: |
| Reference selection | -3.7% | +4.6% | +2.1% |
| Early-return for loop | +16.0% | -7.2% | +12.0% |
| Forced inline | +15.1% | +7.1% | +9.8% |
| Direct find | -4.8% | +11.1% | +12.2% |
| Two scan branches | -7.1% | -8.6% | -1.2% |

A separate product before/after/std campaign then uses the same original build
path and public APIs, three seeds and 90 selected >=1-second intervals. All
variant controls pass. Its values differ from the full matrix because they
are separate recordings with their own default hash placements.

| Condition | Before ns/call | After ns/call | Matched after std ns/call | Before/after change | After/std |
| --- | ---: | ---: | ---: | ---: | ---: |
| Mixed, early exit | 22.07 | 20.31 | 20.13 | -7.9% | +0.9% |
| Mixed, full scan | 5,385.53 | 5,203.21 | 5,260.25 | -3.4% | -1.1% |
| Small, early exit | 19.24 | 15.90 | 17.58 | -17.4% | -9.6% |

### Selective reproduction

Build and verify before recording time. The preparer archives the pinned source
under ignored target storage, changes only the selected method form there and
uses one build path for all variants. It does not project a verification body.
Choose only `base,split` for the focused comparison, or select `--variants base,for,try_each,inline,find,split` to reproduce all forms. Its fixed-size probe also accepts
size, `true` or `overlap`, and seed arguments.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_disjoint.py --source-ref eedd4e358ecfe851a56c241e4be41a9b619c2556 --variants base,split --output target/verification/hash-set/branch-reproduction
python3 tools/throughput-analysis/compare.py --binary before=target/verification/hash-set/branch-reproduction/base-calls --binary after=target/verification/hash-set/branch-reproduction/split-calls --tasks hash_set_is_disjoint,hash_set_is_disjoint_true --small-tasks hash_set_is_disjoint --small-variants before,after --output target/verification/hash-set/branch-reproduction/paired
python3 tools/throughput-analysis/report.py --input target/verification/hash-set/branch-reproduction/paired
```

An explicit `--tasks ''` selects only the requested small-input conditions.
The supplemental intersection command selects `--tasks hash_set_intersection`,
`--small-tasks ''` and `--seeds 1013,2027,4051,8089,16193,32377,64709,129403,258809`.
The predicate-negation control now targets one branch. The native regression
exercises both argument orientations and rejects the compiled defect.

### Unequal sizes and mixed argument directions

A separate named-wrapper probe uses 64 prepared pairs and randomly reverses
argument orientation within each three-seed corpus. Skewed pairs have 512 and
three keys; variable pairs have 512 and 0–512 keys. The current public API and
the previous reference-selection expression share the same product sets.
A matching-hasher std set verifies the same iteration order and visit counts;
the genuine std-default comparison remains separate. Expected results are
computed per case, including empty inputs and skewed pairs without a common key.

All 45 selected intervals last at least one second and their matched std
controls pass (mean ratios 0.93249–1.05450). These isolated-wrapper measurements
are an additional caller context, separate from the primary matrix. The prior
expression is a safe diagnostic implementation, not another runtime API.

| Input | std ns/call | Current API ns/call | Prior expression ns/call | Current/std | Current/prior |
| --- | ---: | ---: | ---: | ---: | ---: |
| Skewed, partial overlap | 25.10 | 23.53 | 26.91 | -6.3% | -12.6% |
| Skewed, disjoint | 38.85 | 32.82 | 33.59 | -15.5% | -2.3% |
| Variable size, disjoint | 2,602.59 | 2,459.45 | 2,401.02 | -5.5% | +2.4% |

Variable-size full scans are 2.4% slower than the prior expression in this
context, while remaining 5.5% faster than the genuine std-default recording.
The change is therefore not reported as a universal improvement. The
[27 compact rows](verification/hash-set-disjoint-shapes.csv) preserve elapsed
seconds, calls, matched std/control times and key-visit counts for recalculation.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_disjoint.py --source-ref fcfc5174be5275c8a1803443455c9e5332b3b732 --variants base --output target/verification/hash-set/shapes-reproduction
taskset -c 0 target/verification/hash-set/shapes-reproduction/probe 512 overlap 617 skew std,current,min_all,matched_std,std_control
```

Select an available CPU, repeat seeds 617/6203/96011, and replace `overlap` with
`true` for disjoint inputs or `skew` with `vary` for the variable-size corpus.
Omitting the shape and labels retains the equal-size probe defaults. The
preparer builds only the requested baseline and the shared-set probe; it never
modifies the product checkout.
