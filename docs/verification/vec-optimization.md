# Vec implementation checkpoint and benchmarks

This checkpoint optimizes primitive cloning, deduplication and preparation for
small extraction/pop workloads. The reference is the immutable executable from
the [long-interval campaign](throughput-long.md). The library remains compatible
with Rust 1.95 stable; no runtime dependency or global alignment flag is added.

Small extraction/pop and dedup improve in the recorded default comparison.
Some mixed-size lifecycles regress, and the default build retains slow conditions.
The tables distinguish those results from the optional alignment settings.
Implementation work is provisionally closed; further changes are reserved for
defects or explicit requirements. The [API mapping](../vec-api.md) and
[Vec guide](../vec.md) describe the implemented functional scope.

## Implementation

| Area | Change | Preserved contract |
| --- | --- | --- |
| Primitive and shared-reference cloning | A statically dispatched, optional `TryClone::try_extend_vec` hook delegates to the existing checked `Vec::try_extend_copy` path | Length limits, fallible allocation, exact clone capacity and the destination allocation policy |
| Other element types | The hook defaults to `None`; existing per-element `InitSlot` initialization and suffix rollback remain | User cloning errors, missing writes, partial cleanup and nested ownership |
| Clone preparation | Inline `Vec::try_clone` and exact reservation together | Allocation failure still returns an error; initialization is published through checked Vec methods |
| Deduplication | Admit a two-element batch once, then use source-verified cursor transitions; handle the final odd element separately | Comparison argument/order, callback errors, retirement before Drop and restoration of the unvisited suffix |
| Extraction | Preserve the two-phase scan and optimize its shared clone preparation | The no-selection prefix, early Drop, predicate errors and retained-tail cleanup |

The bulk hook receives an initialized, safe Vec. Its successful return is never
used as evidence that raw slots were written. Library overrides use the existing
Copy append operation, which performs reservation, copying and length publication.
Custom implementations have the documented functional callback contract; safe
Vec operations enforce memory validity independently of a callback's `Ok(())`.

Dedup's `previous_kept`, `keep_live` and `remove_live` have explicit numeric
preconditions. The source-derived loop model must establish them before each
call. The parser inventories all such calls and rejects new callers or function
references outside the proved dedup body. These operations do not authorize
unchecked indices at a public API boundary.

Both batch steps are written out explicitly. A `0..2` iterator introduced a
debug-only `core::hint::unreachable_unchecked` guard; the final implementation
removes that dependency instead of accepting the new exit. The repeated memory
operations are verified in the actual loop body, including callback failure at
either step and the odd-element tail.

## Generated code and rejected variant

The default primitive clone now reaches the allocator and `memcpy` directly,
without the outlined reservation/clone setup and its second capacity-growth
branch. The duplicate-pair path uses nine executed instructions for two inputs,
versus the previous eight per input. The unchanged pop body remains a load,
output consumption, decrement and backedge. These are concrete measured-image
observations, not instruction-count guarantees for all consumers.

A single-loop extraction variant was rejected. With the same corpus and three
seeds, its no-selection workload took 6.646459 s versus 4.244528 s before the
change, a 56.6% regression. Its retained-element path tested for a hole on every
input; the two-phase version scans the untouched prefix without that test.
Faster full-selection results did not justify keeping this regression. The final
implementation retains the two-phase scan.

## Measurement protocol

Use the maintained [throughput runner](../../tools/throughput-analysis/README.md).
Generate inputs before timing, replay identical inputs/order/work across std and
each product binary, record at least one second per accepted interval, and repeat
with seeds 617, 6203 and 96011. Compute each reported mean as total accepted time
divided by total executed calls. Calibration and short attempts are excluded.

An edit workload includes source cloning, editing and destruction. A pop call
consumes the whole vector; it is not one element removal. Mixed inputs cover
0–131,072 u64 values, and the small profile covers 0–64. Per-seed ranges describe
variation across the recorded runs; they are not confidence intervals. Default
and optional alignment settings are evaluated separately.

Raw timings, diagnostic binaries, generated proofs and assembly remain under
ignored `target/verification/vec-optimization/`. The final verification and
measurement receipts identify the actual source and binaries.

## Safety and allocation verification

| Check | Final result |
| --- | --- |
| Normal / overflow-checked release tests | 156 / 156 pass |
| Miri / Tree Borrows with strict provenance / AddressSanitizer | 121 tests pass in each mode |
| Source connections | 477 checked; zero open |
| Native / specialized 32-bit numeric Verus composition | 410 / 192 verified; zero errors |
| Source controls | Nine semantic failures and nine compiled-source binding rejections detected |
| Fresh focused Kani | 18 memory/state harnesses, three native bridge laws and one UTF-8 adapter harness pass |
| Final links | Release, fat link-time optimization and overflow-checked release have no retained panic calls; debug has the original 19 originating guards and three forwarders, all connected |
| Allocation comparison | 540 combinations pass requested/live/peak, allocator-usable/live/peak, allocation-count, handle-layout and cleanup checks |
| Fuzzing | 10,000 Vec runs complete |
| Builds and policy | Clippy with/without alloc, wasm32 and thumb builds, arithmetic policy, consumer link controls and benchmark/report tests pass |

The 477 connections comprise 88 state publications, 131 constructor fields,
30 accounting calls, 17 storage calls, four private capacity calls, seven
allocation guards, 189 memory boundaries, ten pre-admitted cursor calls and one
safe bulk-clone dispatch. Compared with the earlier census, eight additional
unsafe blocks repeat dedup's existing element operations across the two batch
steps and odd tail. They introduce no new unsafe primitive or public unsafe API.

Controls include an empty retained prefix, a cursor advance after batch
exhaustion, a new unproved live-cursor caller and bulk success without appending.
The wrapping-memory composition reads the freshly verified numeric prerequisite
and checks its compaction-source hash; standalone development outputs are not
used as fresh state models.

These results use [Plan A's verification boundaries](unsafe-proof-plan.md):
numeric/source-guard proofs cover all admitted word-sized states, while compiler,
Miri, sanitizer and failure-injection checks cover their respective ownership,
initialization and aliasing scenarios. User callbacks and public unsafe imports
retain their stated contracts. The allocation matrix measures requested and
allocator-usable bytes, separately from process-resident memory.

Product SHA-256 (excluding proof-only files):
`d086b5afe2967ce22a1347af54458af00d85dae6eb68f41ee22a8d36ae2cd3e1`.
Complete certificate input SHA-256:
`2c3925f2b88cdda677aafe3d86fc271c4671642de98148a0b243c4f5ec3df7f2`.
The fresh receipt is
`target/verification/vec-optimization/panic-certificate/summary.json`.

## Measured outcome

The compact [benchmark summary CSV](vec-benchmarks.csv) retains all 185
policy/control means, totals, call counts and seed-ratio ranges. It contains
aggregates only; individual timing records remain outside Git. Seed-ratio
ranges are not confidence intervals. `baseline` identifies the preceding
executable; `default`, `align32` and `align64` identify the current source.

The main and guard campaigns contain 612 accepted recordings; the shortest is
1.015275 s. Each condition has three seed runs. The table uses the unmetered
policy and the default compiler setting. Std is measured in the new executable;
the baseline executable has its own std controls in the complete local output.
All binaries use equal-length executable paths to avoid changing argument-allocation
sizes. Source generation, counts and call order match in every comparison group.

| Workload | Unit per complete call | std | Before | After | Change from before | After vs std |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| dedup, mixed | µs | 28.615 | 34.930 | 32.529 | -6.9% | +13.7% |
| resize, mixed | µs | 7.848 | 7.366 | 8.004 | +8.7% | +2.0% |
| extract_all, mixed | µs | 13.304 | 13.362 | 14.921 | +11.7% | +12.2% |
| pop, mixed | µs | 10.449 | 10.080 | 10.847 | +7.6% | +3.8% |
| clone, small | ns | 15.566 | 15.121 | 14.703 | -2.8% | -5.5% |
| extract_all, small | ns | 24.652 | 32.240 | 25.651 | -20.4% | +4.1% |
| pop, small | ns | 30.362 | 34.407 | 28.912 | -16.0% | -4.8% |

Small extraction and pop improve by 20.4% and 16.0%, respectively. Mixed-size
resize, full extraction and pop regress relative to the before executable. Their
new unmetered std differences are +2.0%, +12.2% and +3.8%. Bulk copying and
inlining therefore do not establish an improvement for every size distribution.
The resize fill and pop removal bodies retain the same operations as std; the
shared preparation changes from a 32-byte vector copy loop to libc memcpy. The independent
cost of copying versus surrounding placement is not established for every mixed row.

| Other mixed-size checks | Unit | std | Before | After | Change from before | After vs std |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| clone | µs | 5.823 | 5.776 | 6.521 | +12.9% | +12.0% |
| extract_none | µs | 5.288 | 4.640 | 6.474 | +39.5% | +22.4% |
| retain | µs | 58.303 | 62.923 | 60.883 | -3.2% | +4.4% |

### Policy and alignment comparisons

The following values are pooled percentage differences from std compiled with
the same setting. Negative is faster. The 32-byte option puts all 28 targeted
conditions inside the observed +15% range; default has two above it and 64-byte
alignment has one. These three-seed observations are not a confidence-interval
acceptance decision. Alignment remains a consumer-selected build setting.

`unlimited` has no element limit or byte budget; `limited` adds the element limit,
`metered` adds the byte budget, and `limited_metered` combines them. All configured
limits permit the same successful work.

| Workload | Policy | Default | Align 32 | Align 64 |
| --- | --- | ---: | ---: | ---: |
| dedup, mixed | unlimited | +13.7% | +0.8% | +6.6% |
| dedup, mixed | limited | +14.2% | -4.2% | +6.9% |
| dedup, mixed | metered | +21.2% | +2.8% | +3.1% |
| dedup, mixed | limited_metered | +13.5% | +6.8% | -0.8% |
| resize, mixed | unlimited | +2.0% | +2.8% | +5.6% |
| resize, mixed | limited | -6.5% | +7.1% | -0.9% |
| resize, mixed | metered | -3.7% | +6.0% | +2.6% |
| resize, mixed | limited_metered | +1.3% | +1.6% | +10.3% |
| extract_all, mixed | unlimited | +12.2% | -26.0% | +3.2% |
| extract_all, mixed | limited | +5.8% | -23.1% | +7.0% |
| extract_all, mixed | metered | +16.7% | -13.8% | +8.6% |
| extract_all, mixed | limited_metered | -4.6% | -19.4% | +2.0% |
| pop, mixed | unlimited | +3.8% | -2.4% | +8.4% |
| pop, mixed | limited | +12.9% | -2.1% | -4.9% |
| pop, mixed | metered | -0.7% | +7.8% | -6.3% |
| pop, mixed | limited_metered | -3.0% | +2.2% | -5.2% |
| clone, small | unlimited | -5.5% | -3.8% | +0.5% |
| clone, small | limited | -2.8% | +8.7% | +9.5% |
| clone, small | metered | +7.1% | +6.6% | +12.9% |
| clone, small | limited_metered | +7.7% | +3.8% | +10.8% |
| extract_all, small | unlimited | +4.1% | +0.5% | -1.2% |
| extract_all, small | limited | +12.2% | +1.9% | +4.8% |
| extract_all, small | metered | +8.4% | +0.4% | +16.0% |
| extract_all, small | limited_metered | +9.7% | -0.3% | +0.1% |
| pop, small | unlimited | -4.8% | -4.7% | +7.0% |
| pop, small | limited | -4.9% | -9.1% | -5.7% |
| pop, small | metered | -6.6% | -6.2% | -5.2% |
| pop, small | limited_metered | -3.8% | -6.9% | -1.7% |

### No-selection regression and placement control

The separate no-selection replay adds 54 valid long recordings. Unmetered
extraction is 24.0% slower than std with the default setting, 8.3% faster with
32-byte alignment and 3.5% faster with 64-byte alignment. Other policy results
are included below. No per-element budget check appears in the scan.

| Policy | Default vs std | Align 32 vs std | Align 64 vs std |
| --- | ---: | ---: | ---: |
| unlimited | +24.0% | -8.3% | -3.5% |
| limited | +3.6% | -9.4% | -1.0% |
| metered | -3.1% | -11.2% | -7.3% |
| limited_metered | -6.2% | -10.2% | -13.0% |

In the default image the unchanged six-operation scan occupies 18 bytes at
`0x269b0`, offset 48 within a 64-byte line. A diagnostic image moves only those
operations two bytes earlier to `0x269ae`, repairing relative jumps and preserving
registers and every other function address. For the same 817,600 complete calls,
the original takes 5.306222 s and the shifted copy 4.089871 s: a 22.9% reduction.
All three seeds improve and every interval exceeds one second. This establishes
a placement penalty in this executable; it does not identify a particular CPU
front-end mechanism. The patched image is diagnostic only.

This is a known default-build regression. The compiler alignment configurations
provide the measured mitigation; the library does not silently force a global
flag or claim that every calling context will place its loops favorably.

## Reproduction

Implementation source commit:
`c885d3fc63bb443b8e8ec43ed24a87a5f63887da`.
Its product and verification-input hashes match the recorded certificate above.

Build the current throughput tool with the default configuration and with each
existing `.cargo/loop-align32.toml` / `.cargo/loop-align64.toml` configuration.
Run `compare.py` with `--tasks dedup,resize,extract_all,pop` and
`--small-tasks clone,extract_all,pop`, then run `report.py` on its output. Use
the default setting for the additional `clone,extract_none,retain` mixed checks,
and all three settings for the additional no-selection comparison. Full build,
runner and averaging instructions are in the tool README.

The immutable baseline executable SHA-256 is
`451eb586e7ce920d0c9c1bc2aab9f29f72e4090e41187ff37497c40cf84844e1`;
the current default executable SHA-256 is
`c9644aad05ac123864c345747d12952bb50cf426dcee60e3d1b3997c8595ea35`.
The baseline is an archived local executable from the preceding working-tree
checkpoint, not a claim that HEAD alone reproduces that earlier source.

Generated output directories are `performance/`, `guards/` and
`alignment-guards/` under `target/verification/vec-optimization/`. They contain
666 valid campaign recordings and 185 policy/control means. The additional
12-recording placement experiment remains separate. Identical-std total ratios
span 0.917–1.103 in the campaigns; timing variation remains material. No new
product-source change, compiler build or formal-verification job runs during timing.
