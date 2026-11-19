# Internal panic-path certificate

This Box/Vec checkpoint uses `python3 scripts/verify_panic_paths.py`. It connects the
Box/Vec source projections to the concrete link roots; it no longer leaves every
source obligation unconditionally marked `unproved`.

This checkpoint's receipt is
`target/verification/vec-optimization/panic-certificate/summary.json`. Its 477
source obligations and final-link results are summarized in the
[Vec checkpoint](vec-optimization.md). [Plan A](unsafe-proof-plan.md) retains the
preceding receipt and the agreed verification boundaries. See the
[Splice batch report](vec-splice-batch.md#verification) for the later
whole-library source/link certificate and its reproduction command.

## Gate behavior

1. Regenerate numeric, state, prefix, growth and core-predicate prerequisites.
2. Verify the composed owner/credit/loop models and actual-argument guard queries.
3. Check all owner publications, constructors, accounting/storage calls, saved
   allocation guards and private helper callers against those models.
4. Reject unsupported unsafe operations and explicit product exit paths.
5. Run the actual all-input UTF-8 adapter proof and arithmetic-policy checks.
6. Build release, fat-LTO, overflow-checked release and debug link fixtures,
   including panic, bounds, OOM and abort controls in the optimized profiles.
7. Attach the verified guard conditions to every retained originating call and
   close forwarding calls only after those origins are covered.

The raw debug linker still retains 22 calls: 19 guard origins and three panic
forwarders. Their absence is not asserted. The combined gate requires their
source-connected unreachability conditions under the documented typed-storage,
allocator, public-unsafe and callback contracts. Unknown edges cannot pass by
matching a hash or appearing near a passing test.

The certificate is a numeric/source-projection result. Actual initialization,
borrowing, once-only cleanup and Send/Sync scenarios retain the separate
compiler/Miri/ASan/failure-injection evidence. It is not a full Rust provenance
proof, nor a certification of arbitrary user callbacks.

## Negative controls and freshness

Controls mutate real product source. Semantic controls must produce a solver
verification error, not a Rust compilation error. Structural controls first
compile as Rust, then must be rejected by a specific source-binding check.
The underlying memory controls require actual UB diagnostics.

Fresh input hashes, generated-input hashes, source stability checks and execution
locks protect result attribution. Reused model runs and incomplete profile sets
cannot produce a complete certificate. Unsupported earlier Kani loop-contract
instrumentation remains reproducible with `--legacy-kani`; the source-derived
Verus alternative supplies the current loop/composition proofs.

These linker checks apply to test fixtures. They do not install a linker policy
in the product or prohibit consumer `unwrap`/explicit panic calls. Consumers can
use the separate [consumer link checker](../iteration.md) for their own roots.
