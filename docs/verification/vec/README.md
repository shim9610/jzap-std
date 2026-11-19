# Vec implementation and verification

Vec implementation work is **provisionally closed**. All 46 Rust 1.95 stable
inherent methods have explicit counterparts, with fallible adapters for related
trait families. The current source-connected safety checks pass within the
documented verification boundaries. Performance remains conditional.

| Area | Current result |
| --- | --- |
| Functional scope | [Stable API mapping](../../vec-api.md), [guide](../../vec.md), and [failure contracts](../../vec-contracts.md) |
| Normal / overflow-checked release tests | 156 pass in each configuration |
| Miri / Tree Borrows with strict provenance / AddressSanitizer | 121 tests pass in each mode |
| Source certificate | 477 connections; zero open obligations |
| Native / 32-bit numeric Verus composition | 410 / 192 verified; zero errors |
| Fresh focused Kani | 18 memory/state harnesses, three bridge laws and one UTF-8 adapter harness pass |
| Final-link fixtures | Release, fat LTO and overflow-checked release have no retained panic calls; debug's 19 originating guards and three forwarders have source-connected unreachability proofs |
| Long-interval benchmarks | 666 valid recordings, each at least one second; three seeds, identical corpus/order/work, default/32/64-byte settings |
| Allocation comparisons | 540 combinations pass requested/usable live and peak bytes, allocation counts, handle layout and final cleanup checks |
| Remaining performance conditions | Default-build regressions remain. All 28 targeted conditions are within the observed +15% std range with optional 32-byte alignment; this is not confidence-interval acceptance |

The [current checkpoint](../vec-optimization.md) contains operation means,
before/after and policy tables, code-generation findings, source hashes and
reproduction instructions. No external runtime dependency was introduced.
The default allocator and optional element/byte limits retain their explicit
failure and ownership contracts.

The certificate follows [Plan A](../unsafe-proof-plan.md): numeric state and
guard conditions are proved from the source; actual initialization, aliasing,
cleanup and ownership scenarios also use compiler, Miri, sanitizer and
failure-injection checks. Public unsafe imports, allocators and user callbacks
retain their documented contracts. A complete Rust provenance theorem is not
claimed. Consumer `unwrap` and explicit panic calls remain outside the fixture
link policy; see [consumer checks](../../iteration.md).

## Reproduction

Use the commands in the [checkpoint](../vec-optimization.md#reproduction) and
[verification index](../README.md). The maintained full gate is
`python3 scripts/verify_panic_paths.py`; its passing receipt requires fresh
prerequisites, source bindings, negative controls and all four link profiles.
The throughput tool's [README](../../../tools/throughput-analysis/README.md)
defines the repeated one-second minimum and operation-weighted mean calculation.
Generated logs, binaries, assembly, raw timings and proofs stay under ignored
`target/verification/`.

## Earlier checkpoints

| Report | Recorded stage |
| --- | --- |
| [Fixed-input recheck](../performance-recheck.md) | Source-form changes, collection fixes and earlier allocation measurements |
| [Long randomized campaign](../throughput-long.md) | Baseline total-time results across Vec and Box |
| [Baseline operation means](../throughput-averages.md) | Weighted averages derived from that campaign |
| [Expanded fixed-input benchmarks](expanded-benchmarks.md) | Earlier 20-operation confidence-interval comparisons |
| [Retain source and alignment controls](retain-source-controls.md) | Source-form and linked-layout diagnoses |
| [Extraction analysis](extraction-analysis.md) | Earlier prefix-loop code generation |

These reports retain their original source identities and decisions. Their
historical non-passes are not rewritten as successful measurements of the
current source.
