# Internal unsafe verification: Plan A

The [Vec optimization checkpoint](vec-optimization.md) records subsequent source
changes and the renewed complete certificate. This page retains the preceding
checkpoint and the agreed verification boundaries.

Checkpoint date: 2026-11-04, Korea Standard Time. Baseline commit:
`535b3583c1d84cb4d0814cbed7665dce107b0a86`, plus the current working tree.
Product source SHA-256, excluding proof-only files:
`06481ca65645f07bf59570d8e60af09760cf037037da9ec0f4619e6a34cdcbe2`.

Plan A reuses compiler, Miri, sanitizer and failure-injection evidence for the
properties they check. Additional formal work establishes numeric owner/credit
closure, loop bounds and the conditions of retained internal guards. It does
not introduce a new requirement to formalize all Rust aliasing or arbitrary
payload implementations.

## Recorded result

**The fresh combined gate passes for this checkpoint.** F1–F3 are connected
under the verification boundaries below; there are no open obligations in this
numeric/source-guard certificate.

| Final check | Result |
| --- | --- |
| Source connections | 455 checked; zero open |
| Native Verus composition and actual-call queries | 400 verified, zero errors |
| Specialized 32-bit numeric composition | 190 verified, zero errors |
| Real-source controls | Seven semantic verification failures and seven compiled-source binding rejections, all detected |
| Fresh focused Kani | 18 memory/state harnesses, three native bridge laws and one UTF-8 adapter harness pass |
| Link profiles | Release, fat-LTO and overflow-checked release pass directly; debug's 19 originating guards and three forwarders are connected to the source certificate |
| Complete gate | Pass, with fresh prerequisites and all four profiles |

The counts include reused mathematical kernels and 181 generated actual-call
queries; they are not 590 new independent ownership proofs. The final receipt is
`target/verification/performance-recheck/panic-certificate/summary.json`, with verification-input
SHA-256 `8375e1ce1c0fcb5c9f8fdc18c455c9a942e315a779ce446b54c05e87d8692f7d`.
Development runs and reused generated inputs do not qualify as complete
certificates.

## What was implemented

| Workstream | Implementation |
| --- | --- |
| F1: owner/accounting closure | Source-derived construction, reservation, shrink, conversion, flattening and publication checks; allocation-credit creation, refusal/cancellation, replacement, transfer and release; frames preserve other live credits |
| F2: loop and operation composition | Actual retain/dedup/extract/fill/staging loops; complete initialized resize, initial/rest buffer collection, hinted/unhinted splice completion and tail restoration; prefix/array and vectored-byte bounds reuse existing source-generated kernels |
| F3: source-to-guard connection | Actual offset/index/slice/copy arguments generate solver queries; pinned core predicates, typed-layout/alignment conditions and the all-input native UTF-8 adapter proof connect those queries to retained fixture calls |
| Completion gate | `verify_panic_paths.py` uses the new source certificate by default; successful completion requires fresh models, source bindings, arithmetic policy, the full four-profile link matrix and all controls |

The 455 source obligations comprise 85 state publications, 131 invariant-bearing
constructor fields, 30 accounting calls, 17 allocation calls, four private
capacity-helper calls, seven allocation-guard constructors and 181 unsafe memory
boundaries. Immutable error/value records are not owner-state constructors.
Tuple guards are checked explicitly; collecting named field assignments alone
would miss their saved pointer, layout and policy.

These counts are coverage of the defined projection, not a percentage of all
Rust memory-safety properties. New or changed publications, private callers,
allocation policies/layouts and unsupported unsafe operations fail the binding
check. In particular, inserting a new intrinsic inside an existing unsafe block
cannot inherit that block's certificate.

### F1: source-connected numeric state closure

The source bindings close `len <= cap <= limit`, checked byte capacity and the
corresponding credit transitions. Private capacity helpers have checked caller
preconditions. Sealed policies, their getter bodies and the relevant metadata
getters are validated so a policy or count cannot change interpretation between
reservation, publication and release.

Logical allocation keys identify live credits, not integer pointer addresses.
The ledger proves that a matching owned charge is at most total occupancy, that
reservation/commit/cancel/release do not overflow and that another owner's
transition preserves the current entitlement. Actual unique ownership and
callback borrowing use the agreed Rust/compiler/Miri boundary.

### F2: source-connected loop and operation closure

The projections preserve the actual checked counters, branches and publications.
They prove bounds for arbitrary machine-word lengths by induction, without
finite loop unrolling. Callback success, end, returned error and missing slot
initialization are distinguished where applicable. Successful reservations
establish the capacity used by subsequent accesses; an increased capacity after
refusal is not confused with committed length.

Splice proofs cover the initial hole, hidden retained tail, hinted growth,
staging collection, final two transfers and Drop restoration after success or
error. Buffer collection checks the private reserved-end/capacity bounds in both phases, including
unknown-length progress and machine-word exhaustion. Existing Slot/array-prefix
proofs and actual memory tests supply their separate initialization/cleanup
properties.

### F3: remaining internal guard calls

The raw debug link still retains 22 calls. Nineteen originate in guards; three
forward panic handling. The certificate attaches proof evidence to their caller
conditions rather than claiming that retained symbols disappeared.

| Originating guard family | Calls |
| --- | ---: |
| Layout validity/alignment | 5 |
| Non-null pointers/unchecked offset addition | 3 |
| Slice construction/unchecked indexing | 4 |
| Copy/write spans/nonoverlap | 6 |
| UTF-8 output capacity | 1 |
| **Total originating calls** | **19** |

The alignment-argument proof permits an unaligned tested address: a packed field
can produce `false` and an explicit layout error. Only the alignment argument
must be a nonzero power of two for that check to avoid panic. Offset queries also
check the eventual read/write extent; a permitted one-past pointer does not
permit reading a value there.

Unknown originating edges, unsupported unsafe operations, missing bindings,
changed verification inputs, mismatched generated proof inputs, incomplete
profile matrices and failed controls prevent successful completion. A raw link
check and a source-to-guard certificate are different checks: the former still
reports the retained debug symbols.

## Other evidence and performance

| Evidence | Result and scope |
| --- | --- |
| Normal and overflow-checked release tests | 154 pass in each configuration, including 21 doctests |
| Miri / Tree Borrows with strict provenance / ASan | 119 distinct tests per mode: 108 existing plus 11 targeted; executed scenarios, not universal provenance proofs |
| Six Send/Sync implementations | 24 positive/negative compiler cases, six weakened-bound controls and real transfer/shared-read tests |
| Actual memory controls | Overlapping spare storage and uninitialized slot access are rejected by strict Miri with UB diagnostics |
| Unsafe census/macros | 181 blocks, 18 overlapping unsafe function signatures and six unsafe impls; seven relevant macro blocks matched to actual test expansions |
| Existing full Kani checkpoint | 55 default-feature harnesses passed; the certificate freshly reruns the focused prerequisites instead of relabeling the earlier unsupported whole-loop attempts |

The memory suites are reused for the identical product source graph above. Their
complete 118-test run was followed by rerunning the final 11-test target after
adding formatting-error persistence; repeated cases are counted once.

Product code and runtime dependencies did not change during this verification
extension. The rebuilt Vec benchmark remains byte-identical, SHA-256:
`dfaefbf62f29d0899b8440a536dbd05a6f37487bcb750d51792d2f909fecb03a`.
This establishes unchanged generated code for that fixture, not a new timing
measurement or universal std performance equivalence.

## Verification boundaries

The certificate concerns numeric overflow-to-memory conditions and internal
guard unreachability in the checked source projections and Linux x86_64 link
fixtures. Specialized 32-bit word models cover numeric transitions; native
pointer/core-predicate queries use the native target.

Trusted boundaries include Rust's type/layout and safe-borrow contracts, the
compiler/core/verifier, a valid failure-returning allocator and the documented
public-unsafe/callback contracts. The source projection/checking tools are also
part of the verification trust base; they are not a verified Rust compiler.
Callbacks are not certified by merely implementing a fallible trait. Their
returned errors and our state/cleanup are checked; arbitrary callback exit,
unwinding behavior and provenance are not silently proved by scalar models.

Miri, ASan, compiler checks and injected failures cover their stated scenarios.
No all-type Rust provenance, aliasing or arbitrary Drop theorem is claimed.
Windows runtime evidence is not supplied by wasm/thumb compilation. These are
the agreed boundaries, not new proof work imposed on already covered properties.

## Reproduction

With the pinned tools in the [verification index](README.md):

```sh
python3 scripts/verify_panic_paths.py
python3 scripts/check_unsafe_inventory.py
python3 scripts/verify_thread_bounds.py --mutations
python3 scripts/check_memory_controls.py
cargo test --locked --all-features
RUSTFLAGS='-C overflow-checks=yes' cargo test --locked --release --all-features
MIRIFLAGS='-Zmiri-tree-borrows -Zmiri-strict-provenance' \
  cargo +nightly-2026-10-30 miri test --locked --all-features \
  --test foundation --test box_complete --test vec --test iteration \
  --test wrapping_memory --test unsafe_paths
```

Use `bash scripts/verify.sh --check certificate` for the complete source/link
composition; default quick checks do not run it. The separate `verify_vec.py`
also includes the certificate step. Timing campaigns run separately.
`verify_owner_protocol.py --reuse-generated` and
`verify_panic_paths.py --reuse-models` are development shortcuts, not final
certificates. `--legacy-kani` explicitly reproduces the earlier whole-loop Kani
instrumentation diagnostics; those failures are not reported as successful Kani
proofs or required again in place of the implemented Verus alternative.

Raw logs, generated models, mutations, binaries and expanded code remain under
ignored `target/verification/`. No CI trigger or product linker restriction was
added. Consumers can still use `unwrap`; the link fixtures are verification tools.

## Existing reference patterns

The contracts use established permission/ownership patterns; they do not require
inventing a new verification logic. Sources inspected for this plan include:

| Reference | Relevant patterns | Limits checked in the source |
| --- | --- | --- |
| [Verus raw-pointer API](https://github.com/verus-lang/verus/blob/main/source/vstd/raw_ptr.rs) | PointsTo permissions, initialization, provenance and metadata; G01–G03 | Trusted vstd model; provenance is abstract, and the file notes unsupported dynamic-trait metadata |
| [Verus doubly linked list](https://github.com/verus-lang/verus/blob/main/examples/doubly_linked.rs) | Tracked ownership maps and invariant-preserving pointer operations; G05–G07 | A linked-list example, not a proof of our contiguous storage or our caller paths |
| [RawVec partial verification](https://github.com/model-checking/verify-rust-std/blob/main/verifast-proofs/alloc/raw_vec/mod.rs/README.md) | Allocation permissions, alignment, zero-size storage, growth and refusal; G01/G05 | Uses `-skip_specless_fns`, `-ignore_unwind_paths`, and `-allow_assume`; documents incomplete aliasing and variance checks |
| [VeriFast mutable-slice examples](https://github.com/verifast/verifast/blob/master/tests/rust/safe_abstraction/slice_mut_inout.rs) | Slice ownership before/after mutation and value swaps; G03/G04/G06 | The [Rust example README](https://github.com/verifast/verifast/blob/master/tests/rust/README.md) explicitly documents incomplete reference/Box aliasing checks |

The former `slot.verus.rs` used vstd PointsTo for initialization-slot
write, borrow, completion and cleanup. It has been replaced by actual-Rust Kani
checks of `InitSlot`; the scopes and commands are recorded in
[the migration guide](../direct-source-migration.md). Further use of the external
patterns requires adapting them to the actual source and running the proofs.
The external examples above were inspected, not executed in our toolchain during
this research step. No product dependency was added.

The name of an example does not establish completion. In particular, Verus's
[`verified_vec.rs`](https://github.com/verus-lang/verus/blob/main/examples/verified_vec.rs)
has a TODO reallocation implementation containing `assume(false)` and additional
assumptions. It is a design reference, not an acceptable completed Vec proof for
this project's gate.
