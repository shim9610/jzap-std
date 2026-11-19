# Enforced arithmetic modes

This page records the arithmetic-policy checkpoint. The
[November 4 Vec report](verification/vec-optimization.md) records the
follow-up certificate; the [verification index](verification/README.md)
links to subsequent checks.

The remaining primitive arithmetic operators in Box/Vec metadata, initialization
and budget accounting have been replaced. Potentially panicking primitive
operators are denied by Clippy, with type-pair exceptions only for the library's
Checked/Checked and Capacity/Checked operators. Those operators return Result.

## Implementation

Fallible initialization and resize calculate their next state before calling user
code or transferring ownership. Arithmetic refusal returns an explicit error.
Budget reservation also uses checked arithmetic before changing accounting.

Infallible queries and cleanup use explicitly named private wrapping primitives
in `src/arithmetic/bounded.rs`. They carry a **non-overflow proof obligation**:
Kani asserts `checked_add/sub/mul(...).is_some()` before evaluating the wrapping
operation. These are assertions, not assumptions. Source-linked Verus contracts
project the calls to checked mathematical arithmetic, so a possible overflow or
underflow fails verification. The runners validate the actual helper bodies
before applying that projection. Wrapping does not waive caller or ownership
conditions. Their current source connections are described in the
[Plan A certificate](verification/unsafe-proof-plan.md).

`Checked::new(a + b)` and `.get() + b` are no longer accepted as substitutes for
checked arithmetic. Four deliberate bypass probes fail the type-based lint gate.
The maintained verification drivers include that gate. Applications retain their
own choice of arithmetic and Result handling; no consumer link hook is installed.

## Link results

Linux x86-64, Rust 1.95 / LLVM 22, expanded concrete Box/Vec fixture:

| Configuration | Previous retained calls | Current retained calls | Strict link |
| --- | ---: | ---: | --- |
| Release | 0 | 0 | Pass |
| Fat-LTO release | 0 | 0 | Pass |
| Overflow-checked release | 198 | 0 | Pass |
| Debug | 67 | 22 | Retained; source certificate supplements the raw check |

**There are zero arithmetic-overflow panic calls in all four fixtures.** Four
previous debug calls came from the fixture's own `n + 7`; that calculation now
also uses Checked. The 22 remaining debug calls comprise 19 core precondition
checks and three panic-forwarding calls. The checks cover pointer/slice/layout
requirements, alignment and UTF-8 output capacity. They are not arithmetic-mode
failures. The current [combined gate](verification/panic-paths.md) verifies the
source/caller conditions of these residual checks.

Each profile's product/diagnostic execution passes 5,612 cases, including 5,482
allocation/budget refusals. Eight deliberately forbidden exit controls are detected.
These results concern the concrete fixture, not every possible monomorphization.

## Verification

| Check | Result |
| --- | --- |
| Type-based arithmetic gate | Product passes; four native-arithmetic bypasses rejected |
| Actual Rust Kani suite | 39 default-feature harnesses pass, including bounded-helper precondition assertions |
| Accounting Kani | Two all-word valid-state/owned-charge harnesses pass |
| UTF-8 cursor Kani | All valid cursor/scalar inputs pass |
| Closed numeric states | 22 Verus contracts; all three semantic mutants detected |
| Numeric transitions / credit ledger | 43 / nine Verus results; all five numeric mutants detected |
| Vec kernels | Seven contracts; all eight mutants detected |
| Foundation mutations | All six detected |
| Runtime mutations | All ten detected, including accounting and partial cleanup |
| Normal / overflow-checked release tests | Pass |
| Memory safety | Foundation/Box/Vec: 90 tests each in Miri, Tree Borrows with strict provenance, and ASan |
| Portability | Core, Wasm, thumb and nightly no-global-OOM compilation pass |
| Fuzzing | 10,000 Vec runs, maximum input length 128 |
| Code generation | Box 33 wrappers and Vec 100 specialized benchmark functions checked |

The default-feature Kani suite and the accounting/UTF-8 runs use separate feature
configurations; they are not described as one combined invocation. Numeric proofs
do not certify arbitrary payload pointer ownership or callback behavior.

## Speed and memory

Both rebuilt Vec timing executables are **byte-identical to the preceding
checkpoint**, despite removal of checked/debug arithmetic panic paths:

| Timing image | SHA-256 |
| --- | --- |
| Default alignment | `cbf59533399ee056d4b585b71791f476de3b7a7a409cdf81d570d5ef4d871d0a` |
| Symmetric align32 | `ab9e064f1ffd6c0abb135ad74866865aa8d7cd911f40465a4c71c89c0b821bd4` |

Thus this correction adds no instructions to those concrete optimized programs.
It does not establish universal instruction equality for arbitrary element types.
The existing assembly/placement diagnosis in the [metadata checkpoint](checked-metadata.md)
still applies to these exact images.

Fresh measurements use 20 workloads, three lengths, four product policies and
30 independent processes per condition; compilation and verification finish first.

| Alignment | Pass / fail / inconclusive | Identical std controls |
| --- | --- | --- |
| Default | 219 / 19 / 2 | 60/60 pass |
| Symmetric 32-byte | 215 / 17 / 8 | 60/60 pass |

The different verdict counts between repetitions of the same executables reflect
sampling variation, not an implementation change. Both complete timing gates
remain unsuccessful. No global alignment option is enabled by default.
All 21,060 allocator/handle assertions across 1,620 cases pass, including an
allocator that moves reallocations. Default Vec remains 24 bytes on this target.

## Reproduction

Use the pinned tools listed in the [verification index](verification/README.md).

```sh
python3 scripts/verify_arithmetic_policy.py
cargo test --locked --all-features
cargo kani --lib -j 2 --output-format=terse
python3 scripts/verify_state_types.py
python3 scripts/verify_panic_state.py --mutations
python3 scripts/verify_source.py --group budget --group init-prefix --mutations
python3 scripts/mutation_check.py
python3 scripts/check_link_exits.py
```

The last command still returns nonzero for the 22 retained debug calls. Run
`python3 scripts/verify_panic_paths.py` for the combined source/guard certificate.
Source hashes, commands, raw outputs and generated inputs stay under ignored
`target/verification/`; Git retains reusable checks and this compact report.
