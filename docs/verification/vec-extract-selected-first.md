# Selected-first ExtractIf verification

The 2026-11-16 follow-up implements std's decision order: return a selected
element first, and inspect the compaction gap only when retaining an element.
The candidate is preserved in Git at
`92cccf8`.
The requested follow-up reapplies the same candidate at `f92e94f` in
PR 5. Fresh global
verification passes without model or prerequisite reuse. Later
[repeated cloud timing](vec-extract-repeated-timing.md) confirms all-selected
gains but substantial retained-prefix regressions. The candidate remains active
in the Draft PR and is not accepted as a general default optimization.
The accepted Splice optimization remains in place.

## Control flow and generated code

The baseline is `6d172996618879cdece75a182d0c8d22fda82819`. Its ExtractIf driver
tests `read == write` on every `next()` entry, then chooses one of two scans.
The experiment uses one scan:

```text
while read < end:
    decision = predicate(read)
    if decision is an error: propagate it without advancing
    if decision selects the element:
        advance read and return the selected index
    if write != read:
        move the retained element into the hole
    advance both read and write
```

The predicate is called once per visited element. The public iterator still
stops after a returned error, and its existing Drop restores the unvisited
suffix and length. Function preconditions and postconditions are unchanged;
the single loop maintains the common cursor invariant instead of separate
equal-cursor and gapped-cursor invariants. No allocation, unsafe block, public
API, platform branch or compiler option is added.

The comparison uses the existing dynamic-selector `u64` clone/edit/observe/drop
consumer in `tools/throughput-analysis/experiments/extraction.rs`, Rust 1.95.0 /
LLVM 22.1.2, and `-O -C panic=abort -C codegen-units=1`. Product builds enable
`alloc` and use unmetered, unlimited Vec. The callback returns
`Result<bool, Infallible>`; error discrimination is eliminated in this consumer.
The extra baseline instructions therefore cannot be attributed to checking a
possible callback error here.

Counts include successful repeated-path instructions and conditional branches,
including control-flow transitions across basic blocks. They exclude entry,
exit, allocation and cleanup. Prefix and gap rows use the same parity predicate;
they are not separate constant-predicate builds.

| Repeated path | Baseline instructions / branches | Candidate instructions / branches | std instructions / branches |
| --- | ---: | ---: | ---: |
| All selected, after the first result | 11 / 3 | 5 / 1 | 5 / 1 |
| Parity predicate: retain before the first hole | 5 / 2 | 10 / 3 | 8 / 2 |
| Parity predicate: retain with an existing hole | 8 / 2 | 11 / 3 | 7 / 2 |
| Parity predicate: select with an existing hole | 18 / 5 | 7 / 2 | 11 / 3 |

The consumer shrinks from 776 to 520 bytes; std is 539 bytes. The candidate's
all-selected repeated path is a load, cursor increment, output observation
store, end comparison and backedge. Phase and workload-selection tests leave
that repeated path under the default compiler settings.

Retained paths have a different tradeoff: the merged loop checks the gap,
maintains both cursors and returns through shared control flow. The baseline
keeps the prefix and gapped scans separate. std instead stores a deletion
count and derives the hole position. Matching std's decision order alone does
not reproduce that representation or all of its emitted paths.

The original assembly trial used instruction counts without an elapsed-time
verdict. The later [twelve-process comparison](vec-extract-repeated-timing.md)
measures both its gains and regressions; it does not introduce a new CPU-model
prediction. Historical single-loop timing trials remain in
[the migration report](vec-edit-source-performance.md#alternatives-evaluated).

## Focused validation

| Check on the candidate | Result and scope |
| --- | --- |
| Native Vec tests | All 41 pass in debug, release and overflow-checked release |
| Miri | All 41 pass with native strict provenance, i686 strict provenance and Tree Borrows |
| AddressSanitizer | All 41 pass with `detect_leaks=0`; the restricted execution-tool sandbox prevents LeakSanitizer in this run. The [later leak follow-up](vec-extract-leaks.md) checks the current runtime outside it |
| Direct source proofs | Verus in both word domains and actual Rust Kani pass for arbitrary admitted word-sized cursors; the actual iterator/std content comparison covers every byte input, range, Boolean reply and error ordinal through length three |
| Source-defect controls | Three Verus defects and three native Rust defects are detected |
| Vec libFuzzer | 10,000 runs complete |
| Panic-abort tests | Two compatible extraction tests and the explicit callback-error test pass in isolated processes |
| Allocation observation | All 36 baseline/candidate/std triples match calls, requested and usable held/peak bytes, cleanup and handle size |
| Formatting | `cargo fmt --all -- --check` passes |

The prefix-progress Verus mutation originally targeted the removed prefix
loop. It is replaced by a gap-condition inversion, which violates the physical
move's admission precondition and is rejected by the verifier. Both retained
and selected retirement defects remain checked. The native selected-retirement
mutation's indentation is updated for the merged loop; source-routing and
error-stop controls remain enabled. No proof check or input domain is removed.

The initial direct runner stopped at the obsolete mutation anchor; the final
focused replay passes after that update. An initial broad `panic=abort` test
selection also includes a test that deliberately panics and expects
`catch_unwind`, so that subprocess terminates as expected. That test passes in
the unwind and memory suites; the abort-compatible replay tests returned errors
separately. The failed attempts are not counted as passes.

Allocation observations cover six shapes at lengths 0, 1, 8, 32, 512 and 4096,
for 108 isolated observations. Every observation ends with zero held bytes.
The fresh global source/link result is recorded below. Dedicated-machine timing
has not run.

## Global verification follow-up

The verified source checkpoint is `f92e94f`; its runtime source is identical to
the focused-tested `92cccf8`. Proof-scope descriptions identify the single scan
and conditional compaction. The fresh source/link certificate passes with
unchanged source inputs and no model or prerequisite reuse:

| Global check | Result |
| --- | --- |
| Direct foundation/Vec | 20 groups pass; all 55 source defects are detected |
| Standalone direct Verus | 30 targets pass; all 120 source controls are detected |
| Domain replays | Shared ownership including Loom, String, BTreeMap/Set, HashMap/Set, BinaryHeap, Deque, Scratch/slice, owner composition, UTF-8 and arithmetic policy pass |
| Source closure | All 84 state writes, 130 constructor fields and 178 unsafe callers are bound; no open source obligations |
| Final link | Release, link-time optimization, overflow-checked release and debug complete; no unproved link edges; all eight intentional exit controls are detected |

The certificate attaches 89 originating guard calls and 12 forwarding calls to
source contracts. Retained binary guards are covered by those contracts rather
than assumed absent. Its scope is numeric overflow-to-memory and internal guard
unreachability under the recorded contracts in Linux x86_64 fixtures. Direct
Rust/Verus kernels and reviewed source projections remain distinct evidence.
Valid allocator, safe ownership, public unsafe and returning-callback contracts
remain explicit; alias, initialization and drop scenarios are checked separately
by the focused memory and compiler suites. The raw certificate is under ignored
`target/verification/extract-selected-first-global/certificate/`.

The whole-library follow-up passes all 433 native/doc cases in each of debug,
release and overflow-checked release. Whole-library and core-only Clippy,
rustdoc, formatting, verification-runner tests, unsafe census/routing and
core-only, wasm32, thumb and no-global-OOM builds also pass with unchanged
source inputs. Compiler ownership/thread boundary checks pass.

Vec coverage executes all six functions and 90 of 93 lines in
`vec/compaction_driver.rs`. The recorded branch summary is 21 of 26 sides for
the full shared controller file. Each of the three source conditions in the
changed ExtractIf scan executes both outcomes across the test instantiations.
Coverage measures executed paths and remains distinct from all-input proofs.

## Reproduction

The candidate commit contains the executable change and updated mutation
anchors. Use the verified `f92e94f` snapshot for the global replay; the assembly
comparison below uses the identical runtime from `92cccf8`. Generated
sources and raw output belong under ignored `target/verification/`.

From the repository root, the following rebuilds the assembly comparison
without running timing code:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](README.md#snapshot-regression-checks).

```sh
extract_trial_root=target/verification/extract-selected-first-replay
mkdir -p "$extract_trial_root/base" "$extract_trial_root/candidate"
git archive 6d172996618879cdece75a182d0c8d22fda82819 src |
  tar -x -C "$extract_trial_root/base"
git archive 92cccf876d5f5a6dd52450cfdb8c107e896e03df src |
  tar -x -C "$extract_trial_root/candidate"
rustc --edition=2024 -O -C panic=abort -C codegen-units=1 \
  --crate-name jzap_old --crate-type rlib --cfg 'feature="alloc"' \
  "$extract_trial_root/base/src/lib.rs" -o "$extract_trial_root/libjzap_old.rlib"
rustc --edition=2024 -O -C panic=abort -C codegen-units=1 \
  --crate-name jzap_new --crate-type rlib --cfg 'feature="alloc"' \
  "$extract_trial_root/candidate/src/lib.rs" -o "$extract_trial_root/libjzap_new.rlib"
rustc --edition=2024 -O -C panic=abort -C codegen-units=1 \
  --crate-name extract_std_shape --emit=asm,link \
  --extern "jzap_old=$extract_trial_root/libjzap_old.rlib" \
  --extern "jzap_new=$extract_trial_root/libjzap_new.rlib" \
  tools/throughput-analysis/experiments/extraction.rs -o "$extract_trial_root/consumer"
```

On the candidate snapshot, verification commands are:

```sh
python3 scripts/verify_source.py --group vec-extraction \
  --group vec-extraction-values --mutations \
  --output target/verification/extract-selected-first-source
XDG_CACHE_HOME=/workspace/.cache ASAN_OPTIONS=detect_leaks=0 \
  bash scripts/verify.sh --area vec --test-target vec --profile memory \
  --output target/verification/extract-selected-first-memory
ASAN_OPTIONS=detect_leaks=0 bash scripts/verify.sh --area vec --check fuzz \
  --output target/verification/extract-selected-first-fuzz
python3 scripts/source_certificate.py \
  --output target/verification/extract-selected-first-certificate
```
