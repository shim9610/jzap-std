# String implementation and verification

The implementation targets Rust 1.95 stable String with explicit failure
channels. The [API mapping](../string-api.md) covers 38 inherent methods;
[contracts](../string-contracts.md) describe editing and ownership semantics.

| Area | Implemented behavior |
| --- | --- |
| Storage | Existing Vec allocation/initialization/policy machinery, default 24-byte handle |
| Bounds | Independent byte ceiling and budget; scalar-boundary rejection before edits |
| Encoding | UTF-8 adoption/lossy repair, UTF-16 strict/lossy conversion |
| Editing | Append, insert, replace, truncate, pop/remove, retain, split, drain, internal extension |
| Adapters | Clone/target-budget clone, explicit formatting and standard/Cow/Box/raw transfers |

## Source and verification

Baseline: `fea2479ed7b6ee8b464c8a4a7c4a94bf96ee1ae8` plus the current String
implementation. Exact verification inputs are bound by the generated local
certificate; raw records remain outside Git. The completed String checkpoint certificate binds
its original inputs:

```text
920beb0aadf11774d755c31890f009c94fdbb9dd6a40795341b7142eef948742
```

String adds 14 unsafe blocks, three public unsafe interfaces, one private unsafe
encoder, and two policy-bounded Send/Sync implementations for Drain. Existing
Box/Vec operations retain their separate source contracts. String pointer argument routing is checked
against the actual source; Edit geometry is proved on actual Rust, and retain
cursor/cleanup expressions are extracted into native-word/32-bit Verus checks.
Six additional core guard contracts cover boundary/range decisions, byte cursors,
standard byte-owner capacity and negated byte steps. The Unicode decoder
preconditions additionally reference the actual-source scalar and UTF-16 Kani
results. Core predicates and source hashes are pinned to the linking toolchain.
The combined driver requires both certificates; new or changed source inputs
invalidate completion.

Kani checks unrestricted machine-word edit inputs, deletion without growth,
every Unicode scalar's four-byte encoding, and arbitrary UTF-16 unit pairs.
A separate native proof compares the direct UTF-8 writer with core for every
Unicode scalar and arbitrary original destination bytes. Four constant-width
heap witnesses cover actual push/pop and insert/remove paths.

The typed allocation, UTF-8/core contracts and public unsafe/callback obligations
are explicit proof boundaries. Miri, its Tree Borrows/strict-provenance mode,
AddressSanitizer and differential/failure-injection tests cover the actual
pointer/alias/initialization scenarios. These executed scenarios and bounded heap
witnesses are distinguished from the all-input numeric results.

## Executed checks

| Check | Result |
| --- | --- |
| Stable API inventory | 38 inherent methods mapped; nightly-only declarations remain separate |
| Normal and overflow-checked release suites | 174 tests each, including 18 String tests |
| Stable compiler Unicode comparison | All 1,112,064 Unicode scalars; default and limit+budget owners |
| Miri | 15 String scenarios under each of the standard and Tree Borrows/strict-provenance models |
| AddressSanitizer | 18 String tests |
| Fuzzing | 10,000 String state-machine runs |
| Native Kani | Nine String harnesses, including the all-scalar direct writer and all-word edit geometry |
| String Verus kernels | 11 native-word and 11 32-bit contracts; retained-cursor, cleanup and pointer-destination mutations rejected |
| LLVM coverage, String and formatting files | 589/623 lines and 37/42 branch outcomes executed; proof-only code excluded |
| Thread bounds | 28 compiler cases; eight weakened Send/Sync implementations detected |
| Cross-target builds | wasm32 and thumbv7em; core-only/default/std configurations; no-global-OOM build |

The final source-to-link certificate reruns the shared Box/Vec ownership, state,
arithmetic and guard checks: **491 source obligations, zero open obligations or
unproved linked edges**. Release, fat link-time optimization (LTO), and
overflow-checked release fixtures contain no retained panic/abort/termination
edges. Debug retains 39 originating guard calls and four forwarding calls; each
is attached to a source-connected unreachability contract. The rejection controls
include explicit panic, bounds failure, allocation termination and abort.

These are Linux x86_64 fixtures over the instantiated library roots, with 9,642
runtime cases and 9,447 explicit refusals per profile. They complement the
source-level obligations; they do not inspect arbitrary consumer executables.

## Code generation

String retain initially issued an out-of-line memmove for every retained scalar
following a deletion. Fixed 1/2/3/4-byte copies now compile to byte/word loads and
stores inside the loop. A single tail-restoration memmove can remain at exit.
Drain stores Chars directly and ends its borrow before direct tail compaction; it
avoids an extra Option discriminant and repeated range validation during Drop.
Lossy UTF-8 conversion continues the first chunk iterator instead of scanning the
valid prefix twice.

UTF-16 initially used an out-of-line memcpy per character. Direct emission removes
that call and the temporary byte buffer. The larger fallible push body then
remained out of line: each character paid a function call, a Result-tag store,
and a tag reload/comparison. Explicit inlining removes that per-character result
application binary interface (ABI) boundary. Inlining the edit-plan constructor also eliminates its per-scalar
56-byte plan/result transfer. The all-scalar native proof and edit admission
checks cover the direct writer.

Append validates arithmetic before writing. Existing capacity already obeys the
logical ceiling; storage growth rechecks the ceiling and budget. A source-bound
admission lemma connects these two paths. Pop uses core Chars' remaining byte
length instead of recomputing scalar width.

The numeric cursor operations retain checked-state wrappers; transparent wrapper
representation adds no metadata words. Saturating cursor instructions can remain
where LLVM does not infer a proven bound, so timings are measured separately.

Maintained inspection entry points are in `tools/throughput-analysis/src/lib.rs`:

```sh
cargo rustc --release --locked --manifest-path tools/throughput-analysis/Cargo.toml --lib -- --emit=asm,llvm-ir
```

## Runtime and memory protocol

The String executable uses 64 pre-generated shuffled cases per seed, including
empty/one/maximum sizes, ASCII and multibyte text. Mixed cases span up to 16,384
scalars; the short profile uses the same logarithmic generator with a 32-scalar
maximum. Inputs and operation counts match across std, a second std control,
unlimited/limited and budgeted variants. Each retained interval must be at least
one second; short groups are repeated together. Three seeds use 617, 6203 and
96011. Calibration intervals are excluded. Operation means divide summed elapsed
time by summed calls.

Construct/clone/edit workloads include creation and destruction; reserved append
reuses preallocated owners. Edit lifecycles begin by cloning the prepared source.
Formatting begins with an empty owner. No random generation or clock read occurs
inside the timed loop. Compiler/proof jobs finish before timing starts.

The separate Linux System-allocator build records allocation calls, requested
live/peak bytes and malloc_usable_size live/peak bytes for 3,520 cases. Handle
words are reported separately: 24 bytes default, 32 with either policy, 40 with
both. Allocator-observed peaks do not reveal temporary storage inside libc
realloc; they are distinct from whole-process RSS.

## Measured checkpoint

The retained campaign contains **270 intervals**, each at least 1.073 seconds:
11 mixed-size workloads and four short-input workloads, with three seeds and six
implementations per group. CPU affinity was 0, on Linux x86_64 with Rust 1.95.0
(`59807616e`, LLVM 22.1.2), release defaults and no forced loop-alignment flag.
The [75 aggregate rows](string-benchmarks.csv) preserve total seconds, call counts,
seed ranges and standard deviation, including the second std control.

| Policy | Observed time change against std, across 15 conditions |
| --- | ---: |
| Default | −32.1% to +14.2% |
| Byte limit | −30.7% to +10.5% |
| Budget | −23.8% to +6.5% |
| Byte limit + budget | −19.1% to +11.1% |
| Identical std control | −5.7% to +4.3% |

All 60 product comparisons are inside the observed +15% range. These are
three-seed, operation-weighted means for the specified workloads, rather than
confidence-interval acceptance or a universal performance bound. Timings below
refer to complete workload calls; ns/op means nanoseconds per complete call. Pop
consumes the entire text.

### Mixed-size text

| Operation | Calls | std ns/op | Unmetered ns/op | Length limit ns/op | Budgeted ns/op | Limit + budget ns/op | Unmetered change | Budgeted change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| string_construct | 37,673,920 | 104.274 | 97.574 | 97.176 | 94.202 | 93.409 | -6.4% | -9.7% |
| string_reserved | 37,705,920 | 102.762 | 99.323 | 102.582 | 103.771 | 100.660 | -3.3% | +1.0% |
| string_clone | 38,608,512 | 107.619 | 104.808 | 101.281 | 102.956 | 103.997 | -2.6% | -4.3% |
| string_replace | 29,126,528 | 132.159 | 128.476 | 134.455 | 133.478 | 125.299 | -2.8% | +1.0% |
| string_retain | 276,352 | 13,836.034 | 14,376.003 | 13,996.220 | 13,070.395 | 13,055.640 | +3.9% | -5.5% |
| string_pop | 359,552 | 10,180.371 | 10,315.690 | 10,462.342 | 10,220.909 | 10,543.560 | +1.3% | +0.4% |
| string_format | 22,107,328 | 171.257 | 180.029 | 176.407 | 182.366 | 181.221 | +5.1% | +6.5% |
| string_within | 25,134,080 | 164.558 | 152.207 | 150.101 | 153.367 | 157.639 | -7.5% | -6.8% |
| string_drain | 3,278,400 | 1,301.779 | 1,486.043 | 1,326.456 | 1,195.378 | 1,193.369 | +14.2% | -8.2% |
| string_utf16 | 297,600 | 12,406.848 | 12,314.288 | 12,709.170 | 12,876.525 | 13,781.061 | -0.7% | +3.8% |
| string_lossy | 420,672 | 9,090.985 | 9,074.774 | 9,203.709 | 9,272.180 | 9,046.656 | -0.2% | +2.0% |


### Short text (0–32 scalars)

| Operation | Calls | std ns/op | Unmetered ns/op | Length limit ns/op | Budgeted ns/op | Limit + budget ns/op | Unmetered change | Budgeted change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| string_construct | 261,006,080 | 19.828 | 13.465 | 13.737 | 15.107 | 16.037 | -32.1% | -23.8% |
| string_retain | 92,592,064 | 44.048 | 39.986 | 41.873 | 41.640 | 42.241 | -9.2% | -5.5% |
| string_pop | 123,863,424 | 29.711 | 31.088 | 31.275 | 31.081 | 31.386 | +4.6% | +4.6% |
| string_drain | 159,629,184 | 23.668 | 23.653 | 26.150 | 24.355 | 24.442 | -0.1% | +2.9% |

Across 3,520 allocation cases, allocation-call counts and requested held/peak
payload bytes matched std. Allocator-observed held/peak bytes never exceeded std;
one formatting input used 104 actual bytes against std's 120 for the same
98-byte request. The maximum per-case ratio for each metric was 1.0000. The
[44 allocation aggregates](string-memory.csv) record each policy and operation
separately, including the fixed handle costs.

Reproduction identifiers:

```text
benchmark source SHA-256: 1af036053d6378cb8b37c8609b02d9cb1a486a22ca09b4af397ac1e879431197
executable SHA-256:       6922dd33e89415236b5e401c176abbe36996118992dcda829dd3d4e591d79697
protocol SHA-256:         dc628512b84ad7cb5a7f1d6b5fe419d8bf34cfc9d69cb1332dfb29248857388f
```

## Reproduction

```sh
bash scripts/verify.sh --area string --check api --check tests --check models
bash scripts/verify.sh --area string --profile memory
cargo build --release --locked --manifest-path tools/throughput-analysis/Cargo.toml --bin string
python3 tools/throughput-analysis/compare.py --binary default="$PWD/tools/throughput-analysis/target/release/string" --small-tasks string_construct,string_retain,string_pop,string_drain --small-variants default --output target/verification/string/accepted-performance
cargo build --release --locked --manifest-path tools/throughput-analysis/Cargo.toml --bin string --features string-memory --target-dir tools/throughput-analysis/target/string-memory
tools/throughput-analysis/target/string-memory/release/string memory > target/verification/string/memory.jsonl
python3 tools/throughput-analysis/report.py --input target/verification/string/accepted-performance
python3 tools/throughput-analysis/memory_report.py --input target/verification/string/memory.jsonl --output target/verification/string/memory.csv
```

No automatic CI workflow, runtime dependency, source snapshot, raw timing dump or
assembly/LLVM artifact is added to Git.
