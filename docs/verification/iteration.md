# Fallible iteration verification

This page retains the iteration checkpoint. The
[Vec checkpoint](vec-optimization.md) records the subsequent source changes and
November 4 certificate and measurements.

Date: 2026-11-04. Baseline commit:
`535b3583c1d84cb4d0814cbed7665dce107b0a86`, with the current arithmetic, metadata,
and iteration changes. The ignored verification reports identify the actual
working sources by hash. See the [API contracts and consumer guide](../iteration.md).

| Check | Result and scope |
| --- | --- |
| External implementation | 13 integration tests pass; custom sources need no standard Iterator or private marker |
| Error propagation | Conversion, hint, traversal, processing, UTF-8 collection, and splice insertion errors retain their source channel |
| Ownership and cleanup | Suffix rollback, retained-tail restoration, iterator resumption, partial Drop counts, allocation refusal, and budget refunds pass |
| Miri | 82 tests pass across Box, Vec, and iteration; the 13 iteration tests also pass Tree Borrows with strict provenance |
| Address sanitizer | Same 82 tests pass |
| Kani | Five selected full-input state harnesses pass: take transitions/hints, nth early exit, fold error distinction, and the UTF-8 cursor |
| Numeric source projections | 22 closed-state contracts, 43 numeric contracts, nine budget-ledger contracts, and the updated bounded-buffer proof pass |
| Deliberate defects | Both new runtime mutations are rejected; all 12 maintained runtime mutations, six foundation proof mutations, five numeric mutations, and eight Vec proof mutations are detected |
| Consumer final links | Checked release and checked fat-LTO accept the good implementation; overflow, indexing, and abort controls are detected in both configurations |
| Product final links | Concrete Box/Vec fixtures pass release, fat-LTO, and overflow-checked release |
| Build checks | Debug and overflow-checked release tests, Clippy, core-only, WebAssembly, Cortex-M, and rustdoc pass |

The trait and container tests cover the implementation contracts. The final-link
check covers retained direct edges of concrete roots. Neither certifies arbitrary
user implementations, indirect exits, or a whole-program return guarantee.

## Generated code and performance

The matched `std_iteration` and `jzap_iteration` wrappers consume the same fallible
input and perform the same wrapping reduction. Rust 1.95 emits the same 26
instructions for both, with no calls or saved registers. The maintained assembly
check compares their complete bodies after normalizing local labels. This is
evidence for these concrete functions, not every possible adapter/consumer.
All 35 Box assembly wrappers and 100 specialized Vec benchmark functions pass
their maintained code-generation checks.

The splice benchmark measures clone, replacement, and Drop together. Thirty
independent timing processes per case compare three splice input forms, three
sizes, and four jzap policies directly with std. Of 36 product comparisons,
34 pass, one is inconclusive, and one fails; all nine std controls pass.
All 3,510 memory assertions across 270 cases pass, including requested/usable
peak and retained allocation, allocation counts, and handle layouts.

| Exact-hint splice, element limit plus budget | std ns | jzap ns | Ratio |
| --- | ---: | ---: | ---: |
| 32 elements | 46.10 | 56.42 | 1.2240 |
| 4,096 elements | 1,041.35 | 1,076.17 | 1.0334 |
| 131,072 elements | 41,628.56 | 42,213.19 | 1.0140 |

The 32-element combined-policy case already failed the preceding arithmetic
checkpoint (ratio 1.2279); its current 95% ratio interval is 1.1851–1.2663.
Generated policy code retains element-limit checks and budget reservation,
commitment, and release around allocation/cleanup. The benchmark also includes
clone-copy and allocator work; individual time contributions are not isolated.
This remains an existing performance acceptance failure. The matched iteration
wrappers above isolate the new trait's error channel and have identical code.

The final rebuild produces the same timed Vec image:
`dfaefbf62f29d0899b8440a536dbd05a6f37487bcb750d51792d2f909fecb03a`.
Raw measurements remain in `target/verification/iteration/splice-performance/`.
No global std performance equivalence is claimed.

## Reproduction

```sh
cargo test --locked --all-features
CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true cargo test --locked --all-features --release
python3 scripts/check_consumer_link.py --source tools/consumer-audit/cases/checked.rs
cargo +nightly-2026-10-30 miri test --locked --all-features --test iteration --test box_complete --test vec
python3 scripts/verify_panic_state.py --mutations
python3 scripts/mutation_check.py
python3 scripts/analyze_box_codegen.py
python3 scripts/analyze_vec_codegen.py
```

Use `bash scripts/verify.sh --area vec --check tests` for the iteration tests
and `python3 scripts/check_consumer_link.py` for consumer roots. The dedicated
`scripts/verify_vec.py` also includes the four iterator Kani harnesses. The
separate UTF-8 cursor harness uses the existing loop-contract configuration.
Timing/memory binaries must be freshly built before running
`scripts/compare_vec.py`; see its options and the [benchmark guide](../benchmarks.md).
