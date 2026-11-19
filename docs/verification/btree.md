# BTreeMap implementation and verification

This report records the original map checkpoint. Later shared traversal and
packed-construction changes are covered by the [BTreeSet report](../btree-set-design.md),
including its renewed source/link checks and map timing regressions.

Status: the pinned stable API checkpoint is complete. Source/guard and memory
checks pass, and the reported timing and allocation criteria are met. The [guide and API mapping](../btree.md) defines behavior; the
[design review](../btree-design.md) pins comparison sources.

The combined run, including the mandatory direct Rust budget checks, completed
with **841 source obligations, zero open obligations and zero unproved linked
edges**. Its input fingerprint is
`5da42073cad48bad7e92790d8d8685cdfbc853d37c3e0eff95ec78ef8d212571`.
It attached 62 originating guards and five forwarders across four profiles,
including the formatting protocol guards described below. The local budget's
two direct Rust harnesses passed, and an added panic outside the Verus projection
was rejected at its actual source location.

## Executed verification

| Check | Result |
| --- | --- |
| Stable API inventory | 31 map and 14 Entry names mapped to explicit counterparts |
| Normal and overflow-checked release | 228 cases each: 202 runtime tests (20 BTreeMap scenarios) and 26 doctests |
| Miri | 20 scenarios each on x86_64, i686, and x86_64 Tree Borrows with strict provenance |
| AddressSanitizer | 20 BTreeMap scenarios |
| Seeded state-machine fuzzing | 10,000 runs, maximum input length 16,384 |
| Verus | 101 native-word and 99 32-bit contracts; arithmetic, cursor, append-credit and formatter defects rejected |
| Actual Rust Kani | Seven node/borrowed-view harnesses; output-shape function separately checked over unrestricted word and 32-bit lengths |
| Compiler boundaries | 22 accept/reject cases; four weakened thread bounds detected |
| Source/guard closure | 841 obligations; zero open obligations or unproved linked edges; eight explicit exit controls detected |
| Unsafe census | 411 product blocks; the tree's 217 sites have separate source/contract routing |
| LLVM coverage | 2,000/2,121 emitted tree lines and 423/512 branch outcomes; node kernels 261/261 lines and 28/28 outcomes |
| Build and dependency checks | Core-only/default/std, wasm32/thumbv7em, no-global-OOM handling, Clippy, rustdoc, audit, licenses and software bill of materials |

Coverage refers to emitted instantiations in this test executable, rather than
every possible generic instantiation. No product runtime dependency was added.

## Verification boundaries

The implementation checkpoint is `871dacd`, with the agreed implementation order
recorded in `689292e`. Source fingerprints identify exact verification inputs.
Generated solver inputs, binaries, assembly, coverage,
fuzzer corpus and raw timing records stay in ignored `target/verification/btree`.

| Property | Tool and maintained input |
| --- | --- |
| Stable functionality | `check_btree_api.py`: 31 map and 14 Entry names from Rust 1.95; explicit trait counterparts |
| Ownership on refusal | `tests/btree.rs`: allocation-ordinal refusal, retry, budget transfer, clone-prefix failures and initializer errors |
| Tree state | std differential operations, mixed structural properties, split borders and append overlap patterns |
| Arithmetic before memory | `verify_btree.py`: actual node offset/count expressions as machine arithmetic; native-word and 32-bit Verus checks |
| Node transfers | Seven Kani harnesses execute actual insertion, splitting, deletion, borrowing, merging, suffix moves and checked external slices |
| Packed output shape | Source-extracted scalar function checked by Kani over arbitrary word and 32-bit input lengths |
| Recycling admission | Verus prefix/retired-forest credits and source-derived allocation-deficit maxima; planning and commit share the actual retirement kernel |
| Typed pointers and destruction | Miri on x86_64/i686, Tree Borrows with strict provenance, AddressSanitizer and destructor/failure scenarios |
| Borrow and thread boundaries | Compiler accept/reject cases, with weakened Send/Sync controls |
| Broader state exploration | Seeded libFuzzer differential target and LLVM branch coverage |
| Library exit paths | Combined source certificate and concrete release/LTO/overflow-checked/debug consumer link roots |

The finite node Kani witnesses use byte payloads. Arbitrary slot extents and
ownership-token partitions use Verus; generic typed pointers and destructors use
the compiler and memory tools. An experimental internal-node heap Kani harness
exceeded the available memory and is not counted as a pass. Source-derived
child-slot bounds, universal index partitions, exact transfer/parent-update
bindings and actual internal-node memory tests cover that division of work.

Review seals invalidate results when layouts, trait bounds, methods, macros or
source phases change. A seal is a correspondence check, not a proof. Numeric
contracts, actual-source Kani witnesses and deliberate semantic defects are
checked independently. Rust/compiler behavior, valid allocator contracts,
private balanced ownership graphs and returning user callbacks remain explicit
boundaries. This is not a general separation-logic proof of arbitrary Rust code.

Formatting reaches core's `DebugMap` protocol assertions. The maintained
`btree_core.py` projection reads the pinned builder's actual guards, flag writes
and error-publication order. Verus proves that the map's paired `entries` calls
preserve the protocol for an arbitrary finite sequence of successful or failed
formatting writes. Changing the value flag or calling `finish` after an unpaired
key is rejected. Retained formatting guards therefore require this source proof;
their presence alone does not establish a reachable library panic. The same
projection checks core's guarded remainder and masked sixteen-bit shift.

Range construction captures each user-provided bound once. A regression test
combines changing `RangeBounds` replies with inconsistent ordering and verifies
that mutable iteration cannot bypass physical cursor ordering by reading the
bounds again. The corrected path and formatting-write refusals are exercised
under both Miri alias models and on the 32-bit target.

## Performance method and identified costs

The maintained executable is `tools/throughput-analysis/src/bin/btree.rs`.
It compares std with unmetered, local-budget and concurrent-budget maps using
the same 64 pregenerated shuffled `u64/u64` cases per seed. Mixed sizes reach
4,096 pairs; small sizes reach 32. Seeds are 617, 6203 and 96011. Timings include
one continuous interval per accepted recording, each at least one second.
Calibration and short/incomplete attempts are excluded. Mean ns/call is summed
valid elapsed time divided by summed executed calls.

Queries use prepared maps. Destructive workloads include source cloning and
output destruction; append includes both source clones. `get`, replacement and
Entry count individual key calls; remove-cycle counts one removal/reinsertion
pair. Other rows count complete map workloads. The executable's `describe`
command records these units. The corpus distribution is synthetic.

The first insertion control found that a nonfull leaf still called a planner
returning a 64-byte record, saved six registers, used 56 stack bytes, and then
called a separate commit routine. The direct-fit path checks length and inserts
in place. In the three-seed matched control, unmetered build overhead fell from
14.5% to 2.4%; small removal/reinsertion fell from 28.7% to 1.7%.

Borrowed traversal originally used key/value cursors, checking cursor height and
both endpoints on every item. Leaf-edge traversal keeps stored leaf height at
zero, ranges terminate when physical edges meet, and exact whole-tree
iterators use their remaining count. Direct cursor fields remove an additional
per-item optional-pointer test and branch observed in the measured assembly.
Partly consumed owning iterators retain internal pending pairs without following
children already retired at the opposite frontier.

The exact iterator can still test the internal-pair flag needed by that owning
iterator view. This is a per-pair instruction cost, rather than a fixed cost per
map call. Code layout and register allocation also differ between monomorphized
policy instances. Alignment controls distinguish these observed layout effects;
instruction addresses alone do not establish a hardware front-end stall.

Local-budget owning traversal exposed a separate partial-store problem. Its
compiled loop wrote the 16-bit index and height separately, then loaded a wider
word from the same stack slot. A single encoded scalar preserves both fields and
removes that partial-store/wide-load sequence. In the matched three-seed control,
local-budget owning traversal changed from +87.1% to +0.4% versus std. Verus checks
the actual encoding, reads and index updates; changing the index mask is a
compiled semantic negative control.

Loop-alignment controls apply `-C llvm-args=-align-loops=32` or
`-C llvm-args=-align-loops=64` to both product and std in the same executable.
The library does not impose a consumer compiler option. Requested bytes,
allocator-actual held/peak bytes and handle sizes are measured separately from
uninstrumented timing.

## Final default-build timing results

Measured on 2026-11-05 with Rust 1.95.0 / LLVM 22.1.2, Linux x86_64, CPU affinity
0, optimization level 3, one codegen unit and panic=abort. Default loop alignment
is unchanged. The campaign contains **300 valid intervals**, each at least
**1.1368 seconds**: sixteen mixed-size operations and four small-input
conditions, three seeds, and std/control/three product policies.
The [80 aggregate rows](btree-benchmarks.csv) retain total seconds, calls,
seed ranges and standard deviation. All **60 product means** meet the observed
+15% criterion. The identical-std control ranges from -9.7% to +6.2%.

| Policy | Time change versus std across twenty conditions |
| --- | ---: |
| Unmetered | -29.0% to +12.4% |
| Local budget | -26.2% to +1.3% |
| Concurrent budget | -20.6% to +4.5% |

Values below are operation-weighted means of the long intervals. Lookup,
replacement and Entry rows count individual key calls; remove-cycle counts a
removal/reinsertion pair. Other rows count the complete workloads described
above. Seed variation is recorded separately; these means are not a bootstrap
confidence-interval gate.

### Mixed sizes

| Operation | Calls | std ns/op | Unmetered ns/op | Local budget ns/op | Concurrent budget ns/op | Unmetered change | Concurrent budget change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| btree_build | 133,824 | 34,017.042 | 34,046.416 | 33,546.455 | 33,017.131 | +0.1% | -2.9% |
| btree_get | 101,990,560 | 47.955 | 46.897 | 47.355 | 46.125 | -2.2% | -3.8% |
| btree_replace | 89,281,713 | 51.932 | 52.544 | 51.210 | 49.952 | +1.2% | -3.8% |
| btree_iter | 4,422,336 | 1,091.559 | 953.013 | 977.882 | 1,025.517 | -12.7% | -6.1% |
| btree_range | 11,009,664 | 458.900 | 515.695 | 382.854 | 382.225 | +12.4% | -16.7% |
| btree_entry | 94,027,576 | 51.916 | 50.605 | 52.137 | 51.710 | -2.5% | -0.4% |
| btree_clone | 1,622,016 | 3,958.359 | 2,811.445 | 2,921.133 | 3,468.690 | -29.0% | -12.4% |
| btree_remove | 122,880 | 45,551.683 | 39,465.569 | 40,374.847 | 38,165.613 | -13.4% | -16.2% |
| btree_retain | 497,984 | 9,309.863 | 8,836.591 | 8,981.460 | 8,940.826 | -5.1% | -4.0% |
| btree_extract | 535,488 | 8,743.694 | 9,468.123 | 8,823.999 | 8,846.379 | +8.3% | +1.2% |
| btree_split | 1,379,584 | 3,913.206 | 3,131.793 | 3,101.152 | 3,405.498 | -20.0% | -13.0% |
| btree_append | 370,112 | 13,281.036 | 12,785.071 | 13,192.634 | 12,726.422 | -3.7% | -4.2% |
| btree_append_disjoint | 335,680 | 16,664.590 | 14,365.678 | 15,149.708 | 14,251.508 | -13.8% | -14.5% |
| btree_pop_all | 369,408 | 15,048.548 | 11,802.634 | 12,508.201 | 11,950.185 | -21.6% | -20.6% |
| btree_into_iter | 1,105,856 | 4,183.131 | 4,052.710 | 3,942.926 | 4,103.602 | -3.1% | -1.9% |
| btree_remove_cycle | 36,038,734 | 136.611 | 129.698 | 129.204 | 133.670 | -5.1% | -2.2% |


### Small sizes (0–32 pairs)

| Operation | Calls | std ns/op | Unmetered ns/op | Local budget ns/op | Concurrent budget ns/op | Unmetered change | Concurrent budget change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| btree_get | 1,074,421,952 | 4.243 | 4.251 | 4.248 | 4.395 | +0.2% | +3.6% |
| btree_iter | 655,519,296 | 9.007 | 7.092 | 8.745 | 7.842 | -21.3% | -12.9% |
| btree_clone | 173,776,896 | 30.799 | 27.374 | 30.998 | 29.429 | -11.1% | -4.4% |
| btree_remove_cycle | 158,928,748 | 30.470 | 28.726 | 30.869 | 31.847 | -5.7% | +4.5% |

Reproduction identifiers:

```text
benchmark inputs SHA-256: 43e52e6de904b21549af93e252adc902b55529c178598fdb37cdddbbbbc31ea8
default executable SHA-256: 7dd68ca22caac67074e008166602882bb14c73531f82ad25264be774db895e9b
```

## Alignment controls and independent repeats

The alignment campaign contains 135 valid intervals (minimum 1.3284 seconds),
followed by 45 independent mixed-iteration repeat intervals (minimum 1.3956
seconds). All 36 product means meet the observed +15% criterion. The
[48 policy/control aggregates](btree-alignment.csv) preserve both campaigns.
Both std and product receive the chosen global compiler option. The compiler's
default remains the library's default.

| Workload | Build | std ns/call | Unmetered ns/call | Unmetered change | Local-budget change | Concurrent-budget change |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| mixed btree_iter | default | 1,184.300 | 1,068.274 | -9.8% | -6.3% | -7.1% |
| mixed btree_iter | align32 | 1,230.306 | 1,025.401 | -16.7% | -9.4% | -14.7% |
| mixed btree_iter | align64 | 1,171.583 | 1,074.689 | -8.3% | -9.1% | -9.1% |
| mixed btree_range | default | 516.398 | 551.932 | +6.9% | -10.2% | -12.9% |
| mixed btree_range | align32 | 488.373 | 484.035 | -0.9% | +1.1% | -8.7% |
| mixed btree_range | align64 | 518.279 | 458.220 | -11.6% | -11.6% | -7.9% |
| small btree_iter | default | 9.078 | 7.296 | -19.6% | -11.9% | -15.4% |
| small btree_iter | align32 | 9.346 | 7.601 | -18.7% | -10.6% | -20.6% |
| small btree_iter | align64 | 9.475 | 7.886 | -16.8% | -19.1% | -16.6% |

The first align32 mixed-iteration std control was 16.3% faster than its identical
std command. Seed 6203 recorded 2.983 seconds versus 2.351 seconds; process CPU
time changed alongside elapsed time. The independent repeat's align32 control
was 2.2% faster. Default and align64 repeat controls were -0.8% and -12.0%.
Both campaigns use identical executable hashes, inputs and call counts within
each comparison; these control differences describe runtime variation. The
records do not isolate a hardware cause. Their spread limits comparisons between
alignment settings, so the configuration remains an explicit consumer choice.

## Allocation results

The current final-source allocator run covers **12,288 observations**: sixteen
operations, 64 cases, three seeds and std plus three allocation policies. The
48 policy aggregates retain requested and allocator-observed bytes separately.
No extra allocation occurs during lookup, borrowed traversal, replacement or
occupied Entry access.

| Metric | Worst individual product/std ratio |
| --- | ---: |
| Requested bytes held after the operation | 1.0000 |
| Allocator-actual bytes held | 1.0214 |
| Requested peak bytes | 1.1404 |
| Allocator-actual peak bytes | 1.1381 |

All held/peak cases meet the +15% allocation criterion. The unmetered map handle
is 24 bytes, matching std on this target. Both budgeted handles are 32 bytes:
their policy reference is a separate fixed eight-byte cost. The node layouts do
not contain a policy field. Handle size is not included in the allocated-byte
ratios above.

The initial append planner reserved for the sum of both input lengths even when
duplicate removal produced a much smaller output. Planning now simulates key
selection and the same retirement events as the owning streams, recording the
maximum difference between output-node demand and retired nodes available for
reuse. It holds shared source borrows and allocates nothing. The conservative
bounded-credit path remains when its requested spare bytes are at most 10% of a
lower bound on the already owned node bytes. All spare allocation still precedes
mutation. The largest observed peak ratio fell from 1.8333 to 1.1404.

Append timing/allocation workloads use a common budget object for both maps.
Different-budget transfer admission, refusal preservation and refunds have
separate functional tests.

## Reproduction

Activate the installed Rust/Kani/Verus tools, then run the local driver:

```sh
bash scripts/verify.sh --area btree --check api --check tests --check bounds --check models
bash scripts/verify.sh --area btree --profile memory
bash scripts/verify.sh --profile full --plan
```

Complete builds and verification before timing:

```sh
cargo build --release --locked --offline \
  --manifest-path tools/throughput-analysis/Cargo.toml --bin btree
python3 tools/throughput-analysis/compare.py \
  --binary default="$PWD/tools/throughput-analysis/target/release/btree" \
  --small-tasks btree_get,btree_iter,btree_clone,btree_remove_cycle \
  --small-variants default --target-seconds 1.5 --output target/verification/btree/performance
python3 tools/throughput-analysis/report.py \
  --input target/verification/btree/performance
```

Use separate target directories for alignment builds. Allocation measurements
enable the `btree-memory` feature and invoke `btree memory`; this instrumented
binary must not supply timing data. The current allocator instrumentation uses
Linux `System` and `malloc_usable_size`; requested and allocator-actual bytes
are separate metrics, and neither is process resident memory.
