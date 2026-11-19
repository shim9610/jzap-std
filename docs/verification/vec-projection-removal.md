# Vec projection-removal work

Baseline runtime: `63975d7` (the normal-inlining checkpoint `656c330`).
On 18 November 2026, the user paused further tool experiments and accepted
retaining the existing projections for this checkpoint. No Crux fork or tool
patch is scheduled. Direct-source migration remains incomplete; the Crux enum
boundary checks below are preserved as partial results.

The user requires projection removal only where execution remains unchanged.
Do not restructure product loops, move checks/publications, add error/early-return
paths or change normal-build inlining to accommodate the verifier. Verification
must adapt to the original execution code. Keep existing projections whenever
an adequate original-source replacement is not available under that constraint.

The runtime refactors introduced by `82e7293`, `c8a0275` and `4c4cdd6`, plus the
uncommitted vectored controller, are withdrawn after the user's scope correction.
Their loop factoring changed generated code and performance; semantic-equivalence
arguments did not authorize those changes. All product files and Cargo.toml now
match `63975d7` byte for byte after normalizing one proof-only module-visibility
declaration. Normal builds retain the original private declaration. No product
runtime body, field layout, condition, update order, error path or inline
attribute change remains. This restoration preserves the earlier authorized
ExtractIf optimization and leak-check checkpoint.

The withdrawn candidates' logs and measurements are historical experiments,
not current-source certificates or current performance results. Current source
identity and cancellation records are under `target/verification/scope-correction/`.

Current-source routing audit starts with ten generated caller projections:
nine Vec callers and the shared nested-array adapter. The count describes
source sites, not proof strength. Separate owner/reservation/outer-growth
models also remain; eliminating these ten sites alone does not complete Vec.

| Unit | Generated caller sites | Status |
| --- | ---: | --- |
| Extension and target-budget clone callbacks | 3 | Pending; original execution retained; earlier refactored-controller proof is withdrawn |
| Clone from within the same owner | 2 | Pending |
| Vectored byte writes | 3 | Pending |
| Staged splice transfers | 1 | Pending |
| Shared nested-array slice adapter | 1 → 0 | Direct original count + typed-storage composition and Rust checks pass; routing integrated |
| Owner, reservation and outer collection composition | Separate models | Pending |

The original Vec loops and their existing projections remain. A probe or an
independent bounded typed check is not sufficient to retire their complete
32/64-bit numeric contracts. No source-body rewrite is an accepted workaround.

Completion requires original-source contracts and real Rust harnesses with
compiled negative controls, preserved 32/64-bit arithmetic coverage, removal of
the replaced positive-body generation and current caller/link integration.
Python may enumerate/audit sites and run tools; it must not generate a copied
positive executable proof body. Canonical external Rust/core/allocator
contracts remain explicit trust boundaries.

Raw audit/proof/code-generation outputs stay ignored under
`target/verification/vec-source-migration/` (the initial routing census is in
`target/verification/vec-projection-audit/`). No new global certificate is claimed.

## First completed unit: nested-array caller

The original `flatten_count` is composed with the canonical typed-array ABI:
array byte size is element count times element byte size, with element alignment
preserved. The returned flat count establishes both source and destination
extents. No new precondition assumes the result fits. Empty arrays, zero-sized
elements, mismatched lengths and checked multiplication refusal remain covered.
The adapter is maintained Rust composition, not a translated product body.

| Check | Result |
| --- | --- |
| Direct Verus `clone` / `array-caller` | 2 / 328 verified, zero errors, Either32Or64 |
| Actual Rust nested-array harnesses | 2 pass; arbitrary word lengths at widths 0, 1, 3 and usize::MAX, plus nonzero payload/mismatch checks |
| Compiled controls | Reversed length comparison, wrapping multiplication, shifted array width and shifted storage offset all rejected by their checker/solver |
| Fresh caller routing audit | 178 sites; generated caller projections 10 → 9; one direct array-layout composition |
| Historical array-checkpoint owner replay | Passed with **reused memory/pointer prerequisites**; not rerun as a new global certificate after the scope correction |
| Runtime body | Unchanged; geometry visibility differs only under the verifier cfg |

Reproduce the focused direct checks with:

```sh
python3 scripts/verify_source.py --group clone --mutations \
  --output target/verification/vec-source-migration/array-source
```

Raw receipts: `array-source/summary.json`, `array-routing/bindings.json` and
`array-owner-replay.log` under the work directory above. The owner receipt is
`target/verification/owner-protocol/summary.json` and explicitly records reuse.

## Historical clone probes

The actual extension, Vec target-budget clone and owning-iterator target clone
were tried over arbitrary admitted word-sized metadata and normalized success
or returned clone errors. They do **not** pass yet. A missing length-policy
frame was identified; after adding that proof-only frame, Kani still rejects
its generated loop write-set bookkeeping. Inspecting the transformed program
shows inferred writes through loop-local temporary references that are not yet
initialized at loop entry. This is not a verified product-memory defect.

Reusing the existing physical slot leaf did not fix the instrumentation error;
that executable refactor was reverted. A canonical zero-byte core-write model
also failed and was not adopted. An owner-length-only explicit write set failed
assignability checks for omitted temporaries/slot state and was not adopted.
No pointer, bounds, UB or unwinding check was disabled, and no all-word input
range was replaced by a small finite range. Unsuccessful harnesses remain raw
probes, outside the registered passing source groups. These Kani experiments
do not establish the cause of the original Verus caller projections.

## Scope-correction verification

All 137 product source/manifest files match `63975d7` byte for byte after
normalizing only the verifier-only geometry visibility declaration. Normal
builds use the original private module. No execution loop, predicate, update,
error branch, data layout or inline attribute change remains.

Nine representative x86-64 consumers match the baseline assembly: cloned
extension, target-budget clone, initializer, 4 KiB initializer error, extraction,
internal clone, owned resize with the default/custom slot clone, and vectored
writes. Comparison normalizes local block labels and legacy Rust metadata hash
suffixes while retaining full encoded callee paths, instructions, registers,
branches, memory accesses and ordering. This supplements source identity; it is
not a new timing measurement or a performance claim for the withdrawn variants.

The restored source passes 433 all-feature cases, 41 focused Vec cases in both
release and overflow-checked builds, maintained all/core library Clippy,
formatting and core/wasm32/thumbv7em/no-global-OOM compile checks. The retained
array migration is freshly rechecked: two actual Rust harnesses, two original
count functions and 328 array/storage composition functions pass; one native
width control and three Verus/source controls are rejected.

Current evidence is under `target/verification/scope-correction/`:
`runtime-identity.json`, `native-all.log`, `codegen/summary.json`,
`static/summary.json` and `array-source/summary.json`. Canceled owner/panic
campaigns and active receipts from the withdrawn runtime are archived rather
than reused as current evidence. The nine Vec caller projections and separate
outer models remain in place.

## Crux enum boundary and original caller probe

Source checkpoint: `f5b4fcc`. Product runtime and release configuration are
unchanged. The pinned Verus `0.2026.11.02.8bf68ee` rejects the original
`try_resize_with_init` at `.map_err(InitError::Storage)` before generating loop
verification conditions: enum constructors used as function values are not
supported by that translator. A standalone enum example reproduces the error.

The Crux experiment uses the official SAW suite `snapshot-20261008`, Ubuntu
24.04 x86-64 archive, SHA-256
`eda9d3cb25be1315b4cc286913c89f36e5e73aa659ac3f72adabe2ecb42d36a5`,
checked against the GitHub release asset digest. The `crux-mir` component reports version
`0.13.0.0.99`, commit `25d0f3698a96cb8f014911146c02fabdb26f66ff`;
mir-json translates Rust's Mid-level Intermediate Representation (MIR), using
schema 13 and Rust nightly `2026-03-21` (`1.96.0-nightly`,
`ac7f9ec7d`). This is an isolated verification compiler with the suite's
translated core/alloc libraries, not verification of the stable release binary.
Z3 discharges the conditions; pointer, bounds and assertion checks remain enabled.

| Check | Result and scope |
| --- | --- |
| Constructor-as-function boundary | Pass: `Result<u64, Error>::map_err(InitError::<Error>::Storage)` preserves success values and wraps every error kind and payload. This maintained harness exercises core and the original enum declarations; it is not the original Vec body. |
| Original error conversion | Pass: the unmodified `From<Error> for InitError<u8>` implementation, included directly from `src/error.rs`, preserves every error kind and payload. Together the two tests prove 27 conditions, with zero disproved, incomplete or unknown conditions. |
| Compiled semantic controls | Wrong `Initialize` variant and payload replacement with `Error::Bounds` are rejected by assertion counterexamples, not compilation or unsupported-operation failures. |
| Original Vec initializer, unit elements | Blocked at `vec.rs:531`: Crux reports unsupported pointer arithmetic on the dangling zero-sized pointer. A standalone `NonNull::<()>::dangling().as_ptr().add(count)` reproduces the diagnostic. |
| Original Vec initializer, byte elements | Blocked in `alloc::alloc`: the simulator does not recognize `__rust_no_alloc_shim_is_unstable_v2`. A standalone allocation reproduces the diagnostic; `crux-mir` and `crux-mir-comp` both encounter it in the original caller. |

The enum domain covers all eleven `Error` variants. Its `usize` and `i32`
payloads and the success `u64` are symbolic over their complete x86-64 widths;
only the tag is constrained to select a valid variant. The two concrete error
type instantiations above do not establish a theorem over every generic `E` or
32-bit targets. Both original Vec probes start empty, have a fixed element limit
of two, and use arbitrary word-sized requested lengths and callback failure
indices; the byte probe also has a symbolic element value. Neither caller passes.
The reported simulator failures are not established product-memory defects.
No custom intrinsic override or product rewrite was adopted.

The maintained harnesses are in `tools/crux-probe/`. The driver copies harnesses,
resolves source paths, creates Cargo fixture metadata and generates only explicit
negative source variants. It does not extract or generate positive product bodies.
All nine Vec caller projections and their existing word domains remain in place.

After installing the pinned [official SAW suite](https://github.com/GaloisInc/saw-suite/releases/tag/snapshot-20261008)
and its required nightly compiler, reproduce with:

```sh
. /workspace/.devtools/activate.sh
python3 scripts/probe_crux_vec.py \
  --suite /workspace/.devtools/saw-suite-snapshot-20261008/saw-suite \
  --include-callers
```

The full diagnostic command exits 2 and records `partial`: the enum boundary
and two rejection controls pass, while original caller verification remains
blocked. Omit `--include-callers` to run only the passing boundary and its controls.
Receipts and raw outputs are ignored under `target/verification/vec-crux-probe/`;
the maintained replay writes `replay/summary.json`. The source identity check
covers all 138 tracked product source, manifest and lock files.
