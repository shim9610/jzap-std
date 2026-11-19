# Checked integers and internal collection state

This is the preceding metadata checkpoint. The [arithmetic-policy follow-up](arithmetic-policy.md) records the subsequent operator cleanup; the [Vec checkpoint](verification/vec-optimization.md) records the November 4 source/guard certificate and measurements.

Box/Vec metadata now uses transparent checked integers and private state types.
Ordinary `Vec<T>` remains generic over its existing element contracts; callers
can store their own types without converting payloads to library integer types.
This change does **not** establish whole-library panic freedom. The remaining
source/caller and final-link obligations are listed below.

Verification checkpoint: baseline HEAD `535b358` plus the working-tree changes.
The complete `src/**/*.rs` graph used by the successful Kani run has SHA-256
`d344c3e6c74a509da6ed037864e6e6f38241a6fe0d1fcb15c0f09eb26f6ac066`.
The local receipts bind the commands, outcomes and source hashes; no new product
commit is implied by this identifier.

## Representation and failure handling

| Representation | Use | Contract |
| --- | --- | --- |
| Public `Checked<T>` | All 12 primitive integer types | `+ - * / %` return Result; no assignment arithmetic or implicit dereference |
| `Capacity<T>` | Vec storage capacity | Safe constructors validate array layout or use an already valid slice; raw adoption retains an explicit unsafe validity contract |
| `Span` | Box/Vec owned iterators and Drain cursors | Private bounds maintain `front <= back`; forward/backward steps handle exhaustion |
| `Compaction` | Retain/dedup counters and cleanup | Private counters maintain `write <= read <= original` |
| `Extraction` | ExtractIf range and original tail | Maintains `write <= read <= range_end <= total`; early stop retains the original tail |
| Checked length/counter fields | Vec, Box collection buffer and dynamic slice initialization | Check the next state before taking new ownership or publishing initialized storage |

The wrappers add no metadata words. The default Vec remains three words;
an element limit adds one, a budget reference adds one, and combining them adds
two. No new runtime dependency or allocation is introduced by these types.
Returned Results can be larger than primitive integers; this is distinct from
the stored handle layout.

At fallible boundaries, the implementation calculates the next length, range,
or byte layout first, propagates failure, and only then moves inputs or publishes
newly initialized storage. Partial initialization retains an ownership guard.
Infallible queries, exhaustion and destruction cannot manufacture a Result that
their signatures cannot return. Their remaining primitive arithmetic therefore
requires a bounded-state proof. This includes the closed cursor/compaction
transitions and reconstruction of an already validated capacity's layout.
Wrapping arithmetic is not used to bypass those obligations.

The raw `usize` interfaces required by slices, pointers, allocators and std
adapters remain explicit boundaries. Some immutable bounds and tail descriptors
also remain primitive values. A `Checked` field alone does not prove the
relationships between an owner's allocation, length, capacity and policy.

## Verification

| Check | Current result and limit |
| --- | --- |
| Actual-source Kani suite | 39 default-feature harnesses pass: 22 foundations, 11 arithmetic/operator, one capacity, two span and three compaction/extraction |
| Closed numeric state closure | 22 source-generated Verus contracts cover every product constructor, transition and query; three semantic mutations are rejected |
| Source-linked numeric transitions | 43 Verus contracts pass; nine budget-ledger results pass; all five numeric mutations are detected |
| Vec arithmetic kernels | Seven Verus contracts pass; all eight mutations are detected |
| Core pointer predicates | Nine Verus results and three actual native-integer bridge Kani harnesses pass; all three predicate mutations are detected |
| Unit, property and compile-fail tests | Normal and overflow-checked release suites pass; 20 doctests include Checked assignment restrictions |
| Memory safety | Foundation, Box and Vec suites: 90 tests in ordinary Miri, Tree Borrows with strict provenance, and ASan |
| Portability | Wasm, thumb and nightly no-global-OOM compilation checks |
| Arithmetic code generation and linking | Same Result ABI as core controls for all five operators; 1,088 boundary operations in each of three link configurations |
| Collection code generation | Box 33 wrappers and Vec 100 specialized benchmark functions checked |

The numeric-state runner extracts the real method bodies, erases transparent
Checked construction/access, and verifies the full closed scalar state machine.
Its mutation controls change actual source expressions before regeneration.
The arithmetic operator projection is backed by separate all-input actual-Rust
Kani proofs. Verus's trusted integer specifications and this reviewed source
projection remain part of the verification boundary.

Capacity Kani instantiates six representative layouts; it is not a proof
quantified over every Rust type. Span and compaction/extraction harnesses cover
arbitrary machine-word states satisfying the stated invariants. Other ownership
Kani harnesses use bounded payloads. Neither numeric-state closure nor a zero-size
payload loop establishes arbitrary pointer provenance and callback ownership.

The unbounded dedup Kani attempt remains unsuccessful: the loop-contract
instrumentation reports pointer/write-frame failures. Exposing scalar fields to
the verifier did not fix it; that experiment is not part of the product. Separate
numeric proofs and memory-safety checks supplement it without changing the
unsuccessful whole-loop result into a pass.

## Compiler regressions caught during migration

The first full comparison of the typed implementation exposed two accumulating
costs. Both were corrected before the final comparison; the intermediate result
was not accepted as completion.

| Trigger | Observed machine code | Correction |
| --- | --- | --- |
| Fresh-owner cloning after an outlined reservation | LLVM lost knowledge that the initialized length stayed zero. The u64 copy retained a 24-byte, eight-instruction scalar loop with a carry-tested backedge. | Restate the empty initialized prefix after successful reservation. The copy vectorizes to four u64 values per seven-instruction SIMD iteration; source-offset overflow checks disappear from that loop. |
| Four-element prefix batching in extraction | Each retained item kept a conditional cursor update: `cmp`, `setb`, then two dependent additions. | Use one bounded scan. Its bound lets LLVM fold the state's exhaustion check and merge the equal read/write cursors. The pure prefix scan becomes six instructions, 17 bytes. |

The scalar clone cost was paid per element and affected every benchmark that
cloned its source before editing. It was not a fixed allocation-time cost.
Likewise, the conditional prefix updates accumulated with the number of scanned
elements. These findings come from disassembly of the actual timing executable;
they do not assign a measured latency to each individual CPU instruction.

## Final-link obligations

| Collection fixture profile | Strict link | Retained static panic call sites |
| --- | --- | ---: |
| Release | Pass | 0 |
| Fat LTO release | Pass | 0 |
| Overflow-checked release | Pass | 0 |
| Debug | Retained; source certificate supplements the raw check | 22 |

Each profile's product/diagnostic execution passes 5,612 cases, including 5,482
allocation or budget refusals. All eight deliberate exit controls are detected.
The previous working-tree checkpoint retained 386 checked-release and 575 debug
sites with the same fixture. Counts include monomorphized duplicates and core
forwarding functions; they are not observed runtime panics or distinct defects.

The approved numeric state, operation-composition and caller/guard connections
are implemented by the [Plan A certificate](verification/unsafe-proof-plan.md).
Its combined gate supplements the retained debug calls with source-connected
conditions. Rust ownership/aliasing retain their separate compiler/Miri evidence;
no full Rust provenance theorem is inferred from numeric models.
These validation fixtures install no application build hook and do not prohibit
user code from calling `unwrap`.

## Direct std comparison

The [2026-11-04 performance recheck](verification/performance-recheck.md)
supersedes the measurements below and includes the 64-byte opt-in setting.
The following tables describe this earlier metadata checkpoint.

Measured on Linux x86-64, AMD EPYC 9V74, CPU 0, Rust 1.95 / LLVM 22, opt-level 3,
one codegen unit, panic=abort and no LTO. The matrix covers 20 ownership-lifecycle
workloads, three lengths (32, 4,096 and 131,072 u64 values), and four product
policies. Each condition uses 30 independent processes, two warmups and 30
retained batches. Compilation and formal verification finish before timing.
The acceptance bound is an upper bootstrap 95% time ratio of 1.15; an identical
std control must also pass. Intervals apply per comparison.

| Compiler option, applied to both implementations | Product comparisons: pass / fail / inconclusive | Identical std controls |
| --- | --- | --- |
| Default alignment | 216 / 19 / 5 | 60/60 pass |
| `-C llvm-args=-align-loops=32` | 216 / 15 / 9 | 60/60 pass |

Neither build passes the complete performance gate. Alignment changes which
policy has an unfavorable extraction loop; it is not a universal solution.
The application may opt into the existing [32-byte setting](../.cargo/loop-align32.toml).
The library does not enable it by default or infer global workload frequencies.

Default timing image SHA-256:
`cbf59533399ee056d4b585b71791f476de3b7a7a409cdf81d570d5ef4d871d0a`.
The benchmark source graph (product files plus benchmark sources) has SHA-256
`7d8254c1c8e265c00c33c466aa8aca4f7a7823231786a0ec0dc54dc835bc29fa`.
This graph differs from the complete Kani graph because it excludes proof modules
and includes the timing driver.

Separate allocator instrumentation passes **21,060 memory assertions across
1,620 cases**, including std controls: 18 operations, nine sizes, five owner
representations and both normal/moving reallocation. Each case also clones and
drops the result. Requested totals/live/peak, usable live/peak, final release,
allocation calls and handle sizes meet the configured bounds. Final requested
and usable live allocations are zero. Handles are 24 bytes for std/default,
32 for limit-only/budget-only, and 40 for their combination.

### Remaining accumulating extraction cost

| N, full extraction | Default/default-policy ratio | Default/combined-policy ratio | Align32/default-policy ratio | Align32/limit-only ratio |
| ---: | ---: | ---: | ---: | ---: |
| 4,096 | 1.2653 | 1.2655 | 1.0262 | 1.2682 |
| 131,072 | 1.2032 | 1.2173 | 0.9962 | 1.2418 |

A controlled copied executable isolates placement without changing the selected
path's instructions or its branch destinations under relocation. Only one basic
block moves from `0x4a830` to `0x4a82b` inside existing padding; branch
displacements are updated. The executed selected-item sequence moves from
`0x4a833` to `0x4a82e`. Its 18 bytes previously occupied cache-line offsets 51–68;
after relocation they occupy 46–63. The compare/backedge pair fits the same
64-byte line as the preceding instructions.

In a separate 30-process, alternating-order control:

| N | std, ns | Original jzap, ns | Shifted diagnostic, ns | Shift/original ratio, paired 95% interval |
| ---: | ---: | ---: | ---: | --- |
| 32 | 27.049 | 33.860 | 28.385 | 0.8383 [0.8086, 0.8614] |
| 4,096 | 2,285.879 | 2,895.777 | 1,770.078 | 0.6113 [0.6095, 0.6126] |
| 131,072 | 86,902.563 | 104,916.625 | 64,822.813 | 0.6179 [0.6062, 0.6410] |

All identical-image control intervals include equality. This establishes a
placement-dependent accumulating penalty in this image, rather than an
unavoidable cost of checked integer operators. It does not identify a particular
CPU stall mechanism without hardware counters. The shifted image is a diagnostic
control, is not shipped, and does not change the official failing verdict.
Align32 moves the large-input regression to the limit-only instantiation, so its
better default/combined results are not promoted to a global performance win.

### Small-input lifecycle differences

Examples from the official default matrix:

| Workload/policy | N=32: std / jzap, ns | N=4,096 ratio | N=131,072 ratio |
| --- | --- | ---: | ---: |
| Clone, default | 11.643 / 12.724 | 0.9643 | 1.0685 |
| Clone, limit + budget | 11.643 / 17.349 | 0.9889 | 0.9751 |
| Clone + pop-all + drop, default | 27.130 / 36.197 | 0.9923 | 1.0455 |
| Clone + pop-all + drop, limit + budget | 27.130 / 37.295 | 0.9954 | 1.0559 |

The actual pop loops in std, default jzap and combined-policy jzap all contain
four instructions and 15 bytes: load, consume, decrement and backedge. None of
these loops crosses its 64-byte line. The surrounding lifecycle differs: std
calls libc memcpy for cloning, while the product initializes through SIMD copy
loops with scalar tails and fallible reservation checks. The combined policy
also performs limit checks, budget reservation/commit/release and additional
state loads/stores outside the pop loop. Those policy operations have a fixed
count per allocation lifecycle; copying itself scales with byte length.

Consequently these results do not establish a universal fixed-nanosecond cost,
a per-element pop penalty, or performance equality. Individual contributions
from copy strategy, checks, register spills, accounting and placement are not
separately timed here. The 19 failures and five inconclusive comparisons remain
open; full performance acceptance is not claimed.

## Reproduction

Use Rust 1.95, nightly-2026-10-30, Kani 0.68.0, Verus
0.2026.11.02.8bf68ee and its Z3 binary. Generated models, disassembly, raw samples
and logs stay under ignored `target/verification/`; Git retains reusable checks
and compact reports.

```sh
cargo test --locked --all-features
PATH=/path/to/verus-directory:$PATH cargo kani --lib -j 2 --output-format=terse
python3 scripts/verify_state_types.py
python3 scripts/verify_source.py --group vec-cursors --mutations
python3 scripts/verify_panic_state.py --mutations
python3 scripts/verify_pointer_checks.py --mutations
python3 scripts/verify_arithmetic.py
python3 scripts/check_link_exits.py
```

The last command returns nonzero for retained raw debug edges.
`python3 scripts/verify_panic_paths.py` runs the combined source-to-guard gate,
including actual-argument queries, constructor/publication checks and controls.
