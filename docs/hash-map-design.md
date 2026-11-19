# Fallible HashMap

`jzap::collections::HashMap<K, V, S = RandomState, P = Unmetered>` provides
the Rust 1.95 stable map/Entry counterparts through the explicit failure
contracts below. Product source: `bec6748`. The baseline campaign uses
`e3588b6`; the Copy-clone follow-up below records the updated source separately.

## Pinned references

| Reference | Identity | Use |
| --- | --- | --- |
| Rust stable | 1.95.0 `59807616e`; map SHA-256 `809f1f8ee73d80b575c76865be9d2e5dc1f1f188d4db2d1180ab7769568a06fd` | Stable map/Entry inventory and default SipHash-1-3 behavior |
| Rust nightly | nightly-2026-10-30 `92b5b8c87`; map SHA-256 `e4685a821cf7f655cceafd274bff40f1c66e0d4cd7a90281e5a1351ffd1a8add` | Compare unstable APIs without making the product nightly-only |
| hashbrown | 0.16.1, `1876e4f02708b93903d55ef598f68e82a826518f` | Group probing, mirrored control bytes, erase and in-place rehash algorithms |
| fallible_collections | 0.5.2, cached published source | Compare explicit allocation errors; std-backed allocation and seeding are not adopted |

References are not runtime dependencies or maintenance endorsements. Product
storage, policies and fallibility are implemented locally. Rust/hashbrown use
MIT OR Apache-2.0 licensing. No reference source dump is added to Git.

## Storage and failure contracts

Use power-of-two bucket counts, 16-byte groups, seven-bit fingerprints and a
maximum 7/8 load (small tables retain an empty bucket). A single checked layout
holds reverse-addressed key/value tuples and mirrored control bytes, matching
std allocation geometry. Masked triangular probing has a finite group bound.
Duplicate mutable lookup checks physical slot identity before creating references.

Allocate and admit byte budgets before growth mutation. Allocation refusal
preserves the map, capacity, accounting and rejected key/value. Existing-key
updates retain the stored key and do not allocate. Insertion retains a vacancy
while checking later matches, avoiding a second search for new keys. Tombstone
cleanup is in-place. Shrinking and growth copy, then publish, as std's
hashbrown 0.16.1 `resize_inner` does: the source keeps sole ownership and is not
written until one swap publishes the new table, whose length and vacancy credit
are set once. A `Hash` unwind or a structural error frees only the new storage
and leaves the map, its capacity and its budget charge unchanged. In-place
tombstone rehash may still lose entries on a `Hash` unwind, as in std. User
BuildHasher/Hasher, Hash/Eq and Drop code follows the existing callback
contract.

Construction/extension uses open fallible iterators and retains the documented
successful prefix on source error. Clone cleans its private prefix. Entry
initializers return errors explicitly; a successful reserve may retain enlarged
capacity after initializer failure. Retain/extraction errors preserve the current
entry while retaining prior edits. Draining clears unvisited entries on drop.

## Hashing and seeds

The default builder implements keyed SipHash-1-3, compared against std rather
than substituting a faster weak default. Since 2026-11-09 the keys follow std's
per-thread scheme: each thread obtains one pair on its first construction and
advances `k0` by one per `RandomState`, without atomic operations. Default
entropy follows std's platform table (Linux/Android `getrandom` then
`/dev/urandom`, Windows `ProcessPrng`, Apple `CCRandomGenerateBytes`, BSD family
`arc4random_buf`); every outcome std would panic on is returned as `Entropy`, and a
refusal leaves the thread uninitialized for a later retry. There is no
fixed-seed fallback. Other/no_std configurations accept explicit keys or an
explicit BuildHasher and report unavailable default entropy. The contract,
platform premises and evidence are in [RandomState](random-state.md); the
earlier process-wide atomic cache is retired.

## API and use

Enable the `std` feature for the Linux default entropy provider. Constructors
that can obtain entropy or allocate return `Result`; `with_hasher` is allocation-free.

```rust
use jzap::{Error, collections::HashMap};

fn count() -> Result<u64, Error> {
    let mut counts = HashMap::<u64, u64>::try_new()?;
    counts.try_insert(10, 1).map_err(|rejected| rejected.error)?;
    let value = counts.entry(10).try_or_insert(0).map_err(|rejected| rejected.error)?;
    *value = value.checked_add(1).ok_or(Error::Capacity)?;
    Ok(counts.get(&10).copied().unwrap_or(0))
}
```

For `no_std`, pass a caller-selected `BuildHasher`, or supply caller-owned keys
with `RandomState::from_seed(k0, k1)`. Explicit fixed keys are useful for replay;
they do not acquire entropy for the caller.

| std behavior | Explicit interface |
| --- | --- |
| Allocating constructors and default | `try_new`, `try_with_capacity`, `try_default`, including policy/hasher variants |
| Insert, capacity growth and shrinking | `try_insert`, `try_reserve`, `try_shrink_to`, `try_shrink_to_fit` |
| Entry insertion/default/initializer/modification | `try_or_insert*`, `try_or_default`, `try_insert_entry`, `try_and_modify`; occupied replacement/removal remains allocation-free |
| Lookup, heterogeneous borrowing and removal | `get*`, `contains_key`, `remove*`; stored keys remain immutable |
| Multiple mutable lookups | `try_get_disjoint_mut` returns `Bounds` for duplicate physical slots; unchecked variant retains its unsafe contract |
| Retain and extraction callbacks | `try_retain` and `try_extract_if` use `Result` callbacks and fallible iteration |
| Clone and policy-changing clone | `TryClone`, transactional `try_clone_from`, `TryCloneIn` |
| Array/iterator construction and extension | `try_from_array`, `try_from_iter`, `try_from_iter_in`, `try_extend` |
| Borrowing, owning and draining iteration | Borrowing, owning and draining iterators, empty defaults, grouped folds and fallible mirrors; exact length is returned directly |
| Formatting and equality | `Debug`, `PartialEq` across builders/policies, `Eq` |

No implicit allocating `Clone`, `Default`, `FromIterator`, `Extend` or indexing
panic is introduced. The pinned declaration inventory maps 42 distinct stable
map/Entry names; repeated names across owners count once.

The stable reference-pair `Extend` behavior is available through the explicit
standard-iterator bridge:
`destination.try_extend(jzap::iter::from_std(source.iter().map(|(&k, &v)| (k, v))))`
for Copy keys and values. Storage failures still return `ExtendError`; the bridge
retains the source iterator and callback contracts.

## Copy and destruction paths

Scalar and shared-reference `TryClone` implementations supply a typed
`CopyWitness`. Its safe constructors require `Copy`, and its private field stops
safe code from supplying the capability for owning non-Copy values. Opting in
also promises that bitwise duplication matches the implementation's clone
semantics. Without that opt-in, even a Copy type retains its fallible callback.

When both fields provide witnesses, clone allocates uninitialized raw storage
under a deallocation-only guard, copies the whole admitted allocation and
publishes the new table owner last. It does not pre-fill the control array.
Other clones publish each completed pair
separately and clean the private prefix on failure. Neither route silently
converts a failing user clone into success. Types with no destructor skip the
per-slot destruction walk; clearing a retained table resets its control bytes.

## Acceptance and reproduction

The selective runner prints each command and keeps a fresh receipt under ignored
`target/`. Partial commands report their selected scope.

```sh
bash scripts/verify.sh --area hash-map --check api --check tests --check clippy
bash scripts/verify.sh --area hash-map --check verus
bash scripts/verify.sh --area hash-map --check kani
bash scripts/verify.sh --area hash-map --profile memory
bash scripts/verify.sh --area hash-map --check fuzz --check coverage --check bounds
python3 scripts/verify_hash_map.py --kani --mutations
bash scripts/verify.sh --area all --check certificate
```

The direct Verus entry includes the original product modules unchanged: checked
allocation sizes, mirrored control offsets and masked probe movement. Kani runs
actual Rust group masks, word-sized geometry, representative layout sizes across
all capacities, raw slot/Copy-clone publication and retirement, later-group
matches beyond tombstones, and a bounded probe with no EMPTY control byte. Phase models
cover layout extents, insertion credits, retirement, pending rehash entries and
physical slot distinctness. Core shift/chunk guards are checked against the
pinned core source protocol. Deliberate defects must produce solver failures.

Phase models and reviewed correspondence checks are separate from direct Rust
proofs. Kani uses the product's scalar group fallback; the native SIMD path is
exercised by differential tests, Miri and AddressSanitizer. Pointer provenance,
aliasing, initialization and destructor paths use Miri, AddressSanitizer,
compiler boundaries and failure-injection scenarios.
Allocator, foreign entropy and user hasher/Hash/Eq/Drop/iterator contracts remain
the execution boundary. The fresh source/link composition includes this module
in all four registered Linux x86-64 profiles, with zero open source obligations
or unproved linked exit edges. Consumers can apply the same linker check to their concrete roots.


## Performance reproduction

Build both executables before measuring. Run these commands with no compiler,
solver, sanitizer or other benchmark active. The default matrix includes all 14
primary operations and four separately named equal-key hash controls. Small-input
conditions cover construction, clone, lookup, iteration and empty construction.

```sh
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml --features hash-map --bin hash_map
RUSTFLAGS='-C llvm-args=-align-loops=32' cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml --features hash-map --bin hash_map --target-dir target/verification/hash-map/align32-build
python3 tools/throughput-analysis/compare.py --binary default=tools/throughput-analysis/target/release/hash_map --small-tasks hash_map_build,hash_map_clone,hash_map_iter,hash_map_get,hash_map_default_new --small-variants default --output target/verification/hash-map/performance-default-final
python3 tools/throughput-analysis/compare.py --binary align32=target/verification/hash-map/align32-build/release/hash_map --tasks hash_map_update,hash_map_remove_cycle,hash_map_entry,hash_map_iter,hash_map_drain,hash_map_into_iter --small-tasks hash_map_iter,hash_map_drain --small-variants align32 --output target/verification/hash-map/performance-align32-final
python3 tools/throughput-analysis/report.py --input target/verification/hash-map/performance-default-final
python3 tools/throughput-analysis/report.py --input target/verification/hash-map/performance-align32-final
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml --features hash-map,btree-memory --bin hash_map --target-dir target/verification/hash-map/memory-build
target/verification/hash-map/memory-build/release/hash_map memory >target/verification/hash-map/memory-final.jsonl
python3 tools/throughput-analysis/memory_report.py --input target/verification/hash-map/memory-final.jsonl --output target/verification/hash-map/memory-final.csv --format hash-map --max-ratio 1.15
```

The primary std comparison uses its real default builder. The four `fixed_*`
controls use equal zero SipHash-1-3 keys to isolate table/code-generation effects;
the std control then has a zero-sized builder (32-byte handle), whereas the
product retains its 16-byte keys (48-byte handle). Those controls are reported
separately. Both benchmark paths use the same optional 32-byte loop-alignment flag;
it is never forced on consumers; see the existing Cargo alignment configurations.

For assembly inspection, build the same benchmark with
`cargo rustc --release --manifest-path tools/throughput-analysis/Cargo.toml --features hash-map --bin hash_map -- --emit=asm`.
Grouped folds load 16 control bytes, extract the full-slot mask, retire one bit
per initialized tuple, and check the remaining count between groups. Both std
and the product retain the count decrement. The product also bounds group loads.
Consuming no-drop paths retire ownership through the private scan frontier rather
than rewriting table metadata for every yielded tuple. Drop-bearing values retain
per-slot retirement before callbacks or destruction. Copy-witness clone admits
its new allocation before a bulk copy. These paths are covered by the actual
Rust frontier/slot kernels and unwind/aliasing memory scenarios.

## Memory results

The allocation campaign contains 6,144 observations: three seeds, 64 cases,
eight operations and four implementations/policies. Maximum product/std held
and peak ratios are 1.0000 for both requested bytes and allocator-usable bytes;
all cases return their tracked live allocation count to zero after destruction.
The [24 aggregate rows](verification/hash-map-memory.csv) distinguish the two
byte measures and operation endpoints. They do not measure process resident set
size (RSS).

On Linux x86-64, std and unmetered handles are both 48 bytes. Borrowed local and
concurrent policies make the product handle 56 bytes; ledger work is an optional
feature cost. Default keys occupy one 24-byte thread-local cell per thread and two
static flag bytes (formerly a 24-byte static cache), outside the allocator
measurements. Owning budget functionality and compiler lifetime/thread boundaries
are tested, but owning policies are not part of this timing/memory campaign.

## Timing interpretation

The payload is `u64` keys and values. Each seed prepares 64 shuffled cases before
measurement: sizes 0–4,096, or 0–32 for the small profile. `get` counts two lookups
per key; `update` and `entry` count one existing-key operation; `remove_cycle`
counts one remove/reinsert pair. Other rows count a whole map workload. Destructive
workloads include cloning the prepared source; `reserve` includes clone and
growth, and `shrink` includes clone, growth and shrinking. `default_new` measures
one cached empty constructor and drop. Initial OS entropy acquisition is outside
these warm recordings.

The timing CSV retains each accepted seed interval with call counts, elapsed
seconds and matched std/control seconds. A row of the table is recomputed as
`sum(elapsed_seconds) * 1e9 / sum(calls)`; its std ratio is
`sum(elapsed_seconds) / sum(std_elapsed_seconds)`. Every retained interval is at
least one second. Calibration and incomplete groups are excluded. Seed spread
includes corpus and OS-selected hash-placement variation. Means are observations
in this compilation context, rather than confidence bounds for arbitrary callers.

The budget columns include the policy handle and ledger work when an allocation
occurs. std has no corresponding byte-ledger feature. The four equal-key controls
also retain the builder/handle differences described above.

## Generated-code changes

The initial existing-key update path retained a call returning the full rejected-
input `Result`: it saved five registers, reserved a stack result and stored the
success discriminator and value. Inlining the public operation and successful
admission path lets the caller eliminate those result temporaries. One insertion
probe now supplies both the existing-key result and a vacancy; tombstones do not
terminate the search for later existing keys. Its stride limit is checked after
an unsuccessful group, preserving finite traversal without preparing a separate
group counter on the first-hit path.

The previous resize loop tested one control byte per source bucket. This was
particularly costly after growth, when shrinking scans a sparse source. Resize
now uses the verified group scanner: a 16-byte control load and mask select the
initialized entries. The old source-byte compare/backward conditional jump is
replaced by group-mask traversal. A separate three-seed comparison reduced the
observed shrink ratio from 1.2172 to 0.9401. No allocator layout or product
padding was added.

That loop still moved each pair: it cleared the source control byte, decremented
the source length and charged the destination's vacancy credit per entry, and an
unwind emptied the map. The copy-then-publish resize removes those per-entry
stores and keeps the map on unwind. Counters on the local CPU (cycles relative
to std, before → after): HashSet `shrink` 1.126 → 1.027, `reserve` 1.086 →
1.013, `build` 1.042 → 1.006; HashMap `shrink` 1.023 → 0.984. One-second
timing is in [RandomState](random-state.md#results-2026-11-09-local).

Cached empty construction initially retained four register saves, a call and a
stack result from seed acquisition. Making that constructor available for
inlining reduced its separate three-seed mean from about 9.0 ns to 2.45 ns
(std about 2.29 ns). The atomic seed-ticket operation remained until the
2026-11-09 per-thread rework ([RandomState](random-state.md)). Indirect calls can
retain failure-result storage in the public application binary interface (ABI);
these observations concern the concrete benchmark callers. The final matrix
below uses the final source throughout.

## Recorded timing results

The following baseline tables use product source `e3588b6`.

Linux x86-64, Rust 1.95.0 / LLVM 22, release optimization, one pinned CPU and
seeds 617, 6203 and 96011. The campaign contains 465 accepted intervals across
31 conditions, including the four equal-key controls. Each interval lasts at
least 1.0504 seconds. Default-build conditions were collected in two selective
runs using the identical executable; alignment uses a separately built executable.
Repeated identical-std mean ratios stay within 0.9–1.1 for every condition.

Of the 19 primary default-build conditions, 18 unmetered means are within +15%
of std. Small clone adds 3.10 ns (19.0%); mixed-size clone adds 7.99 ns (2.1%).
The baseline product pre-fills `buckets + 16` control bytes before copying the
allocation; std/hashbrown clones into uninitialized storage. The follow-up
removes that extra fill while retaining checked admission. The Copy-witness
path performs no per-entry clone callbacks. The optional alignment
campaign has one short-condition exception: small drain adds 4.77 ns (16.8%).
Alignment remains a consumer option; these results do not establish a universal
best setting.

The [timing CSV](verification/hash-map-timing.csv) contains 279 compact seed rows
for independent recomputation, including matched std and control seconds. Raw
calibrations, logs, generated source and disassembly remain outside Git.

### Default build, mixed sizes

| Operation | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: |
| `build` | 21158.30 | 18819.79 | -11.1% | 19787.89 | 19576.40 |
| `get` | 12.10 | 13.45 | +11.1% | 12.95 | 12.92 |
| `update` | 12.19 | 12.62 | +3.5% | 12.82 | 12.77 |
| `entry` | 12.39 | 13.06 | +5.4% | 12.66 | 12.38 |
| `iter` | 517.53 | 494.21 | -4.5% | 473.69 | 499.23 |
| `clone` | 387.66 | 395.65 | +2.1% | 389.46 | 398.31 |
| `remove_cycle` | 31.40 | 31.55 | +0.5% | 31.35 | 30.58 |
| `retain` | 2029.45 | 2302.38 | +13.4% | 2295.22 | 2207.11 |
| `extract` | 1930.39 | 1858.16 | -3.7% | 1882.94 | 1917.08 |
| `drain` | 1263.95 | 911.61 | -27.9% | 882.10 | 879.72 |
| `into_iter` | 1039.49 | 868.48 | -16.5% | 865.66 | 864.37 |
| `reserve` | 8121.67 | 6308.96 | -22.3% | 6493.87 | 6501.79 |
| `shrink` | 13930.61 | 13095.88 | -6.0% | 13118.02 | 13078.27 |
| `default_new` | 2.50 | 2.59 | +3.6% | 2.87 | 3.03 |

### Default build, small sizes

| Operation | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: |
| `build` | 209.49 | 198.77 | -5.1% | 205.73 | 212.37 |
| `clone` | 16.31 | 19.41 | +19.0% | 20.35 | 23.47 |
| `iter` | 7.25 | 6.87 | -5.2% | 6.79 | 7.29 |
| `get` | 9.01 | 9.70 | +7.7% | 9.74 | 9.87 |
| `default_new` | 2.53 | 2.52 | -0.3% | 2.92 | 2.98 |

### Equal-key controls, mixed sizes

| Operation | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: |
| `fixed_build` | 23069.86 | 18709.76 | -18.9% | 19296.05 | 19292.32 |
| `fixed_get` | 15.68 | 11.89 | -24.2% | 12.01 | 12.63 |
| `fixed_clone` | 384.16 | 359.38 | -6.5% | 382.12 | 394.01 |
| `fixed_iter` | 515.82 | 466.12 | -9.6% | 472.90 | 461.68 |

### Optional 32-byte alignment, mixed sizes

| Operation | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: |
| `update` | 11.86 | 12.94 | +9.1% | 12.63 | 12.53 |
| `remove_cycle` | 29.61 | 30.05 | +1.5% | 31.37 | 31.42 |
| `entry` | 11.38 | 11.67 | +2.6% | 12.06 | 11.86 |
| `iter` | 510.07 | 468.25 | -8.2% | 478.89 | 496.40 |
| `drain` | 1122.35 | 857.17 | -23.6% | 871.37 | 860.33 |
| `into_iter` | 1035.62 | 899.93 | -13.1% | 873.92 | 905.57 |

### Optional 32-byte alignment, small sizes

| Operation | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: |
| `iter` | 7.45 | 7.32 | -1.8% | 7.01 | 7.16 |
| `drain` | 28.39 | 33.16 | +16.8% | 28.45 | 30.17 |


## Copy-clone follow-up

Product `bec6748` removes the control pre-fill from the Copy-witness route.
The new allocation has a deallocation-only guard until the complete byte copy
finishes; publishing the table transfers that guard's allocation ownership.
Ordinary fallible element cloning retains its initialized-prefix cleanup.
Allocation and budget refusal still preserve the source and its accounting.

The actual release executables retain 39 `memcpy` references across their
specialized benchmark `epoch` functions, while `memset` references fall from
28 to 7. These are static code locations, not per-call counts. The eliminated
work is `buckets + 16` bytes per Copy clone with an allocated table, proportional
to capacity. The change also moves instructions within compiled callers, so
complete workload differences below do not isolate the cost of `memset` alone.

The follow-up compares the recorded baseline executable (`e3588b6`) and the new
executable in one campaign: three seeds, 240 accepted intervals, each at least
1.077 seconds. Both executables use the same inputs, order and call counts.
Each also runs its own std baseline and repeated identical-std control; the
16 control mean ratios range from 0.9493 to 1.0444. All eight new unmetered
condition means are within +15% of their matched std baseline. Small clone's
observed gap is 0.16 ns (+0.9%). Input profiles and workload units are those
defined above; destructive operations include cloning.

| Profile / operation | std before ns/call | Before ns/call | std after ns/call | After ns/call | Change from before | After vs std |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| mixed `clone` | 359.80 | 378.44 | 363.12 | 368.45 | -2.6% | +1.5% |
| mixed `retain` | 1973.87 | 2207.60 | 1985.69 | 2033.56 | -7.9% | +2.4% |
| mixed `extract` | 1884.31 | 1989.68 | 1909.17 | 1986.70 | -0.1% | +4.1% |
| mixed `drain` | 1123.34 | 862.52 | 1091.56 | 811.58 | -5.9% | -25.6% |
| mixed `into_iter` | 1098.77 | 885.92 | 1080.41 | 830.24 | -6.3% | -23.2% |
| mixed `reserve` | 8329.04 | 6710.18 | 8060.39 | 6355.61 | -5.3% | -21.2% |
| mixed `shrink` | 14127.20 | 13126.21 | 13805.13 | 13073.74 | -0.4% | -5.3% |
| small `clone` | 17.02 | 18.69 | 17.33 | 17.49 | -6.4% | +0.9% |

The [144 compact seed rows](verification/hash-map-copy-clone-timing.csv) include
local and concurrent budget policies and matched std/control seconds. The
6,144 allocation observations were rerun: the aggregate CSV is byte-identical
to [the allocation results above](verification/hash-map-memory.csv), with
maximum held/peak requested/usable ratios of 1.0000 and zero live allocations
at teardown.

To reproduce the baseline source without storing an old source snapshot in Git,
extract the pinned commit into a temporary directory, then build both versions
with the same compiler and flags. Complete verification before timing:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
baseline_source=$(mktemp -d)
git archive e3588b6 | tar -x -C "$baseline_source"
cargo build --release --locked --manifest-path "$baseline_source/tools/throughput-analysis/Cargo.toml" --features hash-map --bin hash_map --target-dir target/verification/hash-map/copy-clone/before-build
cargo build --release --locked --manifest-path tools/throughput-analysis/Cargo.toml --features hash-map --bin hash_map --target-dir target/verification/hash-map/copy-clone/build
python3 tools/throughput-analysis/compare.py --binary before=target/verification/hash-map/copy-clone/before-build/release/hash_map --binary after=target/verification/hash-map/copy-clone/build/release/hash_map --tasks hash_map_clone,hash_map_retain,hash_map_extract,hash_map_drain,hash_map_into_iter,hash_map_reserve,hash_map_shrink --small-tasks hash_map_clone --small-variants before,after --output target/verification/hash-map/copy-clone/performance
python3 tools/throughput-analysis/report.py --input target/verification/hash-map/copy-clone/performance
```

## Verification checkpoint

The baseline checkpoint passed all 25 selected checks. For `bec6748`, all
22 selected quick/memory/build regression checks and whole-crate tests with
23 doctests pass without source changes. The HashMap integration suite now
contains 23 scenarios, including sparse/tombstone controls, aligned borrowed
payloads, allocated-empty/zero-size tables and allocation/budget refusals.
The actual Rust clone kernel also rejects a compiled defect that publishes
length zero after copying a live entry. Seed-cache Loom and dependency-audit
results below are from the unchanged baseline paths. The seed-cache row is
superseded by [RandomState](random-state.md).

The fresh global run rebuilds its proof prerequisites with no model reuse;
all six HashMap Kani kernels and direct Verus/phase checks pass. Source/link
composition again has zero open obligations or unproved edges in all four
profiles, and detects all eight deliberate exit controls.

| Evidence | Checked scope | Result |
| --- | --- | --- |
| API inventory and native scenarios | 42 distinct stable map/Entry names; iterator traits; allocator refusals, budget boundaries, collisions, cleanup, cloning and policy ownership | Pass |
| Actual Rust Kani | Six kernels covering masks, word/layout bounds, slot publication/copy, tombstones/later matches, bounded probing and scan retirement | Pass |
| Direct-source Verus and phase models | Original size/mirror/probe functions; explicit mathematical ownership/credit models; deliberate defects | Pass; model and pointer boundaries remain as described above |
| Runtime memory | Native and i686 strict-provenance Miri, native Tree Borrows, AddressSanitizer | Pass |
| State and compiler boundaries | 10,000 fuzz runs, differential sequences, LLVM coverage, lifetime/Send/Sync and Copy-capability probes with weakening controls | Pass |
| Seed publication (retired atomic cache) | Native races/refusals/unwind and actual-code Loom atomic substitution | Pass at that checkpoint; superseded by per-thread keys, see [RandomState](random-state.md) |
| Source and final linking | Release, LTO, overflow-checked release and debug; eight deliberate exit controls | Zero open obligations or unproved edges; all controls detected |
| Build and dependencies | Core/alloc/target builds, no-global-OOM configuration, strict lints/rustdoc, security/license audit and software bill of materials | Pass |

All raw receipts are regenerated under `target/verification/hash-map`. The
commands above reproduce selected components or the full source/link composition.

## Group-mask retirement on clear and drop (2026-11-10)

`Clear::run` retired a whole table (map drop and `clear` when the pair needs
drop) by testing every control byte with one branch per bucket. In the mixed
`scenario_wordcount` benchmark (owned string keys; small inputs) that branch
carried 54% of the scenario body's sampled branch mispredictions, and total
mispredictions were 13.7M against std's 6.3M over 60,000 epochs; code shared
with std (`str::Split`, `memchr`) also mispredicted 2.6–5× more in the jzap
run. hashbrown retires by group mask.

The full-retirement path now reads one FULL mask per 16 control bytes, as the
iterator scanner does. `index` stays on the current group and each slot is
marked EMPTY before its destructor runs, so a resumed run after an unwinding
destructor re-reads the group and visits only still-owned slots. The pending
(rehash recovery) path keeps the per-bucket walk. Integer-keyed maps do not
run this loop.

One-second protocol (CPU 6 at 3.4 GHz, seeds 617/6203/96011), unmetered vs std:

| `scenario_wordcount` | Before | After | Seed ratio range after |
| --- | ---: | ---: | --- |
| small (up to 32 tokens) | +18.6% (251.0 → 297.6 ns) | +7.6% (261.2 → 281.0 ns) | 1.063–1.086 |
| mixed (up to 4096 tokens) | +3.7% | +2.8% | 1.022–1.032 |

Mispredictions after the change: 8.4M (std 5.8M). The remaining small-input
difference is mostly instruction count (265.7K against 248.8K per epoch).
Ruled out while locating the cause: string hashing (isolated `hash_one` of
short strings is 23.7 cycles for jzap against 26.6 for std), the non-inlined
`Hasher::write_str` (outlining the byte loop to let it inline raised the
scenario from 68.5K to 83.5K cycles per epoch and was reverted), and allocator
call counts (callgrind: identical `malloc`/`free`/`realloc` instruction totals).

Checks on this change: all-features tests (381, including
`destructor_unwind_clear_drain_and_map_drop_retire_each_owner_once`), clippy,
unsafe inventory (580 blocks), hash-map and hash-set models, hash-map Kani
(2 groups), and Miri x86-64 / Tree Borrows / i686 for the destructor-unwind
test. Area-wide Miri for hash-map and hash-set was not run locally.
