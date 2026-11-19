# Memory verification: results and proof boundaries

This page records the earlier Box checkpoint. The current
[wrapping-arithmetic follow-up](../wrapping-memory.md) adds Box/Vec byte-extent
proofs, real non-ZST boundary probes, and memory-overrun controls without
promoting those results to whole-source ownership verification.

The later [Plan A source certificate](../unsafe-proof-plan.md) connects owner
credits, full operation flows and actual core-guard arguments. The
[Splice batch report](../vec-splice-batch.md#verification) records the
current four-profile certificate.
The Kani instrumentation limits below remain historical diagnostic evidence.

The Box checkpoint combines bounded implementation checks, source-linked Verus
proofs, runtime memory instrumentation, failure injection, and mutation controls.
A compiler error or unsupported check is never counted as a proof. The current
counts below supersede earlier partial runs retained in the checkpoint history.

## Recorded Box-checkpoint results

| Method | Result | Boundary |
| --- | --- | --- |
| Basic Kani 0.68 | 12 harnesses passed | Finite bounds and instantiated implementation paths in src/proofs.rs |
| Kani with uninit-checks and valid-value-checks | 3 passed, 3 unsupported initialization cases, 4 compiler errors, 2 compilation timeouts | Remaining cases are unproved by these enhanced checks |
| Verus | 19 verified results across seven source-linked models | Trusted vstd, explicit storage models, and extracted control/arithmetic; not whole-program Rust verification |
| Verus mutation controls | Six deliberately broken source invariants rejected | Semantic proof failures, not compilation failures |
| Runtime mutation controls | Four deliberately broken cleanup/accounting paths detected | Executed regression inputs |
| Miri / AddressSanitizer | Passed recorded implementation tests | Executed inputs; neither establishes universal termination |

The [Box checkpoint](../box-completion/README.md) records the tested source
and suite scope. The Miri runs used Stacked Borrows and Tree Borrows with
strict provenance.

## Kani failure investigation

The original compiler failure occurred in `check_values.rs:959`, where scalar-ABI
assumptions encountered a pattern involving a non-null pointer that may carry str
metadata. Formatting paths from `unwrap` and equality assertions in no_std probes
helped isolate it. Replacing formatting assertions with direct equivalent
conditions allowed selected checks to proceed without weakening their properties.
A separate uninitialized-check probe reached an
`initial_target_visitor.rs:107` unwrap failure.

A raw-pointer write probe also exposed validity instrumentation checking the
destination before initialization. The corresponding safe MaybeUninit-write
control and Miri controls distinguished that instrumentation behavior from an
invalid initialized reference. The minimal probes and deliberately invalid-value
controls are retained in [tools/memory-proof/repros](../../../tools/memory-proof/repros).
Product harness counts exclude these probes.

In the final enhanced set, three initialization cases remain unsupported around
union/niche modeling. Dynamic-slice and collection cases reach the
`check_values.rs:959` compiler error; struct initialization reaches
`delayed_ub/mod.rs:75`, and nested-array reuse reaches `ty_layout.rs:191`.
Budget arithmetic and heap-array-slot compilation exceeded 60 seconds. The
algorithmic cause of those timeouts was not established. Neither longer expected
runtime nor a successful basic run is treated as enhanced-check success.

## Verus supplementation

The recorded verifier is release `0.2026.11.02.8bf68ee`. The downloaded archive's
SHA-256 is `f3a0119292df8855e4435da8130bee1873b0e6a60c306accf81a08ea687bba3c`.
Its Rust 1.99.1 toolchain is a verification dependency; the product's declared
minimum remains Rust 1.95.

| Model | Verified results | Property |
| --- | ---: | --- |
| Budget | 5 | Reservation, occupancy, refusal, and release arithmetic |
| Initialization slot | 4 | Recorded writes, ownership transfer, and cleanup state |
| Array clone | 2 | Initialized prefix and ownership disarming |
| Array slot | 2 | Partial initialization and cleanup |
| Dynamic slice | 2 | Initialized prefix and transfer |
| Collection buffer | 3 | Bounded extraction and checked growth |
| Nested-array reuse | 1 | Checked flattened length and preservation on mismatch |

The generator connects extracted product expressions and control conditions to
models, retaining source hashes in `verus-*.json` and generated inputs in
`*.generated.rs` under the ignored local output directory. Slot storage uses the verifier's PointsTo model. Execution uses
`--no-cheating`; the project models do not add assume/admit/external_body escapes.
The verifier and trusted vstd remain assumptions. Allocator implementation,
arbitrary user Drop/callback behavior, and every Rust unsafe detail are not
proved by these models.

Six changes to the actual source conditions caused semantic verification failures. This checks the
connection between source and proof; it does not make the abstraction complete.
Runtime memory and failure-injection checks complement those proof boundaries.

## Reproduction

From the repository root, with pinned tools installed:

```sh
python3 scripts/reproduce_kani_limits.py
python3 scripts/verify_memory.py
python3 scripts/verify_direct_verus.py --target init-prefix --target buffer --target local-budget
python3 scripts/verify_source.py --group budget --group init-prefix --mutations
```

`verify_memory.py` intentionally returns nonzero while enhanced checks are
unsupported or fail. The extended verification driver records those outcomes and
continues independent checks. `JZAP_VERUS` can select the Verus executable; inspect
script prerequisites before reproducing historical environment-specific probes.
Generated inputs and raw logs go to `target/verification/memory-proof/`.
Keep reviewed summaries in Git and retrieve earlier sources from commit history.
