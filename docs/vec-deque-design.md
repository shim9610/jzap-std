# Fallible VecDeque

Status: stable API implementation, executed verification, allocation replay and
std timing comparison complete. Fresh whole-library source/link composition passed.
Target: Rust 1.95 stable, `no_std + alloc`.

## Scope and references

`jzap::collections::VecDeque` provides
explicit counterparts for the 54 distinct stable inherent method names in the
pinned standard implementation, plus fallible ownership/iterator/clone adapters.
Byte-budget and optional element-limit policies use the existing foundations.
The target includes `insert_mut`, `push_front_mut`, `push_back_mut` and
`pop_front_if`/`pop_back_if`. The pinned unstable `extract_if`, `splice`,
`truncate_front`, front-extension and allocator APIs are outside this inventory.

Reference implementations were inspected locally; none is a new dependency:

| Source | Version / identity | Use |
| --- | --- | --- |
| Rust alloc VecDeque | 1.95.0, rustc 59807616e; mod.rs SHA-256 b166418e66bb4069f2e9284dc8c02ae788517888c4980554396bcc53a13ae0f6 | Stable API inventory, ring movement, capacity growth and contiguous conversion |
| Rust alloc VecDeque | nightly-2026-10-30; mod.rs SHA-256 c3f80e5c0c8c55fd58a82e4fc6ffefd9c935dae3aaf3c89b0542b0bd01092c60 | API stability changes and implementation comparison |
| heapless Deque | 0.9.3, commit ca95c4beaf7340d678acefe44817e87547a9b8b4 | Fixed-capacity rejection and ring-state comparison |

Adapted standard-library movement algorithms retain the Rust project's
MIT OR Apache-2.0 licensing. Archive identities for the reference crates are in
[research sources](research/sources.json).

## Representation and failure contracts

The owner stores a data pointer, validated capacity, checked head and length,
allocation policy and length policy. The unmetered/unlimited layout targets four
machine words, matching std. `len <= capacity`; nonempty storage has
`head < capacity`; the initialized elements occupy at most two disjoint slices.
Zero-sized values have no allocated payload and retain logical element limits.

Ring address calculations use checked or bounded wrapping arithmetic with
direct product-source contracts. No power-of-two capacity restriction or unused
sentinel slot is introduced.

| Operation | Failure/state contract |
| --- | --- |
| Constructors/reserve | Layout, limit, budget and allocation errors return explicitly. Failed growth preserves pointer, head, capacity, length and values. |
| Push/insert and mutable-return variants | Validate bounds and obtain capacity before transferring the input; refusal returns `Rejected<T>`. |
| Grow wrapped storage | Reallocate first; after success move the shorter admissible segment into the new region or move the high segment to the new end. |
| Shrink | Validate the request before mutation. If relocation is needed, linearize before the fallible resize and restore the original ring placement on refusal. |
| Pop/remove | Empty/out-of-range queries return `None`; removal retires ownership once before returning the value. |
| Range/swap/rotate | Invalid bounds return `Error::Bounds`; valid movement allocates nothing. |
| make_contiguous | In-place moves and rotation, no allocation or user callbacks. |
| Retain and conditional pop | Predicate failures return explicitly; predicate mutations remain. Retain preserves the successful prefix decisions and current/unvisited elements in order. |
| Drain | Checked range; forgetting the iterator retains the prefix and may leak unvisited removed/tail values, matching the established drain ownership contract. Drop retires the range and restores the remaining sequence. |
| Append/split | Reserve destination storage and clone the required policy handle before moving elements. Refusal preserves both owners. |
| Resize/extend/clone | Growth admission precedes writes. Callback/source/clone errors retain a documented initialized prefix; temporary owners clean up on refusal. |
| clone_from | Empty sources clear the destination. Opt-in bitwise clones without drop glue reuse admitted destination capacity. Other fallible clones construct a replacement before swapping, preserving the old destination on refusal. |
| Vec/std-owner conversions | Explicit ownership transfer; no delegation of collection operations to a standard owning container. |

Borrowed iterators use the two initialized slices; mutable iteration follows their
disjointness. Owning/draining iterators retire values before moving them out.
User callbacks and destructors follow the existing execution contract.

Consuming `fold` scans the two contiguous segments. Its guard records each moved
value before calling user code and retires that prefix on completion or unwind.
Clone uses the existing `CopyWitness` bulk path when the element opts in;
other element implementations retain their fallible cloning contract.

## Implementation checkpoint

The pinned 54 stable inherent names have declared counterparts. Default/TryDefault,
fallible cloning, comparisons, hashing, array/iterator construction, explicit
Vec/std owner conversion and borrowed/owning/draining bidirectional iteration are
implemented. Mutable indexing is checked through `get_mut`; growth, reserve and
callback operations expose their failure channels.

Native scenarios include all valid heads and lengths for capacities 1 through 14,
middle edits, rotations, contiguous conversion, exact growth and shrink. Smaller
exhaustive suites exercise ranges and drains. Refused shrink checks both slice
contents, addresses and lengths. The same suite checks allocation ordinals,
owned local/concurrent policies, limits, callback failures and exact destruction.
Coverage-guided scenarios also check wrapped comparison/hash/ranges, non-Copy
clone refusal at every allocation ordinal, nested target-budget propagation,
std-owner admission and drain restoration after one user destructor unwinds.

The nine arithmetic kernels in `src/deque/geometry.rs` are included directly by
`tools/direct-verus/deque.rs`. Forward/backward chunk contracts prove each copied
range fits capacity and each nonempty step decreases the remaining count.
Contiguous-branch contracts prove the rotation bounds. Kani separately executes
the original word geometry and bounded ownership/movement kernels. The latter
uses concrete short layouts; it does not quantify arbitrary heap size or pointer
provenance. Reviewed caller seals invalidate edits, and Miri/sanitizers check
the separate aliasing, initialization and drop scenarios. No Python function-body
projection is used for the new kernels.

The standard Deque-to-Vec adapter also retains rotation guards in generated code.
The final link check includes these predicates, with the pinned standard source
identity and the ring-bound contracts above.

The randomized property scenario runs natively. Miri executes the other 18
scenarios with reduced exhaustive bounds. The ZST word-length proof covers
arbitrary logical lengths.

## Verification and performance protocol

- Stable-name inventory and executable API scenarios; differential operation
  sequences against std, including arbitrary non-power-of-two capacities.
- Every allocation refusal, wrapped growth/shrink, callback failure, drop counts,
  zero-sized values, large alignment, owned/borrowed budgets and element limits.
- Actual Rust Kani kernels for ring ownership/movement and direct-source Verus
  for machine-word ring geometry. Compiled defect controls accompany the checks.
- Native, checked release, 32-bit/Tree Borrows Miri, sanitizers, fuzzing, coverage,
  arithmetic policy and whole-library source/link integration.
- Timing binaries exclude allocator observation. Random inputs are prepared
  before timing; matched std/product/policy groups use identical counts and at
  least one second per accepted interval, three input seeds, two controlled stack layouts and std repeats.
- Compare unmetered default-build means with std using the existing +15%
  observational criterion. Compare allocation calls, held/peak requested bytes
  and allocator-usable bytes separately. Optional budget-handle words are listed.
- FIFO/LIFO cycles, growth, wrapping, iteration, range editing, rotation,
  contiguous conversion, append/split, clone, retain and large values are separate
  workloads. Regressions require generated-code and workload-size analysis.

## Optimization checkpoint

Array `TryClone` now composes the element's `CopyWitness`. This removes the
per-array temporary and selects the existing bulk clone path. Non-Copy clone
uses Vec's tracked in-place initializer. Copy resize fills admitted spare ranges
before publishing the new length.

Rust 1.95 `VecDeque::pop_front` passes `len < capacity` to LLVM with
`assert_unchecked` after retiring one element. The same fact is now established
by the directly verified `after_pop` product function and passed to LLVM after
front/back removal. Kani asserts the exact Boolean before the optimizer hint;
its proof cannot pass by merely assuming the Boolean. A compiled inverted-fact
control is rejected. The borrowed-final-scan control did not remove the earlier
FIFO gap. The pop fact removed its repeated capacity branch. Equal-call-boundary
assembly inspection confirms this change; its earlier timing run failed the
std-repeat gate and is excluded from performance conclusions.

Front insertion now publishes the admitted length before the infallible slot
write, following the standard implementation's update order. In the paired
17-workload diagnostic build, the old loop reconstructed its prior length with
an extra `lea` on every iteration. The reordered loop removes that instruction,
reducing the observed loop from 70 to 66 bytes. Head remains in a register in
both default-build versions. A two-stage retain experiment showed no timing
improvement and was reverted.

Empty Copy extension admits capacity before resetting the empty ring's head and
copying one contiguous source range. Nonempty extension retains the circular
split path. Failed admission leaves the original head and storage unchanged.
For types without drop glue, `truncate` retires a suffix by updating length;
no per-element walk is required. No-drop drain cleanup similarly retires its
remaining span at once before restoring the retained prefix and tail. An actual
Rust Kani harness covers every
machine-word logical length for zero-sized values, including `usize::MAX`.

## Large-value copy layout diagnosis

The default-build assembly for 256-byte push has two 256-byte `memcpy` calls
per element in both std and jzap: source to a stack temporary, then temporary
to ring storage. The natural-layout run included repeated std intervals that
changed substantially between child processes. Instruction addresses within
the executable were unchanged; the initial stack's low bits varied.

A separate [controlled copy kernel](../tools/throughput-analysis/copy_layout.c)
holds its code and source/destination addresses fixed and shifts only its
scratch buffer by 0, 16, 32 or 48 bytes from a 64-byte aligned base. Every
interval is at least one second and uses the same iteration count within a
mode. [The observations](verification/vec-deque-copy-layout.csv) are:

| Scratch offset, bytes | Source → temporary → destination, ns/block | Source → temporary only | Temporary → destination only |
| --- | ---: | ---: | ---: |
| 0 | 8.563 | 3.938 | 4.569 |
| 16 | 14.864 | 4.212 | 4.780 |
| 32 | 8.497 | 3.904 | 4.614 |
| 48 | 14.983 | 4.240 | 4.554 |

The two-copy sequence changes by about 6 ns per 256-byte block; either copy
alone changes much less. Hardware performance counters are unavailable in this
guest, so no hardware stall-counter attribution is reported. This diagnostic
changes neither the product's type alignment nor its allocation layout.

The controlled collection comparison disables ASLR only in benchmark children
and varies an unused environment string by 16 bytes. Both variants execute the
same sealed binary. Each layout has matched std/product call counts and an
independent std repeat. The pooled table divides summed measured seconds by
summed calls over both controlled layouts.

## Executed verification

| Check | Result / covered scope |
| --- | --- |
| Stable API inventory | 54 pinned inherent names mapped; declaration inventory is separate from semantic tests |
| Native library and documentation tests | 357 passed |
| VecDeque native/release/checked release | 19 scenarios passed in each configuration, including differential sequences, storage reuse and allocation refusal |
| Strict-provenance Miri | 18 scenarios passed on x86-64 and i686 |
| Tree Borrows Miri | 18 scenarios passed |
| AddressSanitizer | All 19 VecDeque scenarios passed |
| Fuzzing | 10,000 executions passed |
| Coverage | Deque product lines 1,041/1,108 (94.0%); branches 210/256 (82.0%); geometry branches 34/34 |
| Static/target checks | Strict Clippy/rustdoc, core-only, wasm32, thumb, no-global-OOM and compiler ownership/thread boundaries passed |
| Direct Verus | Original geometry functions passed; 13 verification conditions, with incorrect-source controls |
| Actual Rust Kani | Nine geometry/ownership/movement/refusal groups and ten compiled runtime defect controls passed |

Fresh whole-library source/link composition passed with no model reuse, zero open
source conditions and zero unproved linked paths. The final timing matrix
contains all 39 predeclared conditions.

| Consumer build | Retained guard/forwarding calls | Unproved paths |
| --- | ---: | ---: |
| Release | 6 | 0 |
| Release LTO | 9 | 0 |
| Overflow-checked release | 6 | 0 |
| Debug | 76 | 0 |

Eight deliberate panic/bounds/OOM/abort controls were detected in release/LTO.
These retained calls are classified as unreachable under their verified source
contracts. The combined receipt uses the direct geometry kernels, actual Rust
ownership/movement harnesses, reviewed caller correspondence and existing
projected core/owner models. Miri/ASan and compiler checks cover the separately
stated memory boundary. The [verification methodology](verification/README.md)
describes each kind of evidence and its reproduction.

## Time comparison

Measurements use Rust 1.95.0 / LLVM 22.1.2, Linux x86-64, an AMD EPYC 9V74 guest,
glibc 2.41 and CPU 0 affinity. Product source: `8556b4d`. The default executable
is unchanged by the no-drop Drain fix: the preceding release optimizer already
eliminated that trivial retirement loop. The explicit fix also applies to debug
and interpreted execution.

The [compact timing data](verification/vec-deque-timing.csv) contains 234 matched
layout/seed rows, representing 1,170 intervals; the shortest is 1.047 seconds.
Each condition uses three input seeds and both controlled stack layouts. Every
std-repeat layout/condition mean is between 0.925 and 1.095 of its std reference.
Values below use `sum(seconds) * 1e9 / sum(calls)` over both layouts and all seeds.
One call processes one prepared case. Elements are `u64` or `[u64; 32]`; both
support bitwise cloning. Local/concurrent columns include the respective borrowed
ledger work; std has no byte ledger.

| Workload | Element bytes | std ns/call | Unmetered ns/call | Change | Local budget ns/call | Concurrent budget ns/call |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| push_back | 8 | 566.458 | 589.583 | +4.1% | 590.233 | 629.360 |
| push_front | 8 | 622.186 | 608.181 | -2.3% | 653.583 | 693.132 |
| fifo | 8 | 712.167 | 625.376 | -12.2% | 657.085 | 667.064 |
| pop_all | 8 | 522.643 | 506.032 | -3.2% | 500.438 | 497.032 |
| iter | 8 | 53.751 | 66.257 | +23.3% | 52.543 | 52.265 |
| index | 8 | 288.093 | 285.039 | -1.1% | 283.835 | 290.314 |
| insert_remove | 8 | 295.210 | 257.245 | -12.9% | 268.169 | 278.198 |
| rotate | 8 | 181.766 | 151.061 | -16.9% | 148.990 | 152.527 |
| contiguous | 8 | 407.133 | 387.506 | -4.8% | 386.062 | 429.068 |
| shrink | 8 | 598.832 | 604.826 | +1.0% | 621.026 | 612.757 |
| append | 8 | 267.078 | 240.378 | -10.0% | 272.944 | 255.846 |
| split | 8 | 206.867 | 220.921 | +6.8% | 229.437 | 236.958 |
| clone | 8 | 152.526 | 130.108 | -14.7% | 129.685 | 162.468 |
| retain | 8 | 703.390 | 629.749 | -10.5% | 665.277 | 690.017 |
| drain | 8 | 320.085 | 294.644 | -7.9% | 315.274 | 300.928 |
| resize | 8 | 354.864 | 337.765 | -4.8% | 344.960 | 389.823 |
| extend | 8 | 142.932 | 133.939 | -6.3% | 167.029 | 139.169 |
| clone_from | 8 | 131.477 | 122.517 | -6.8% | 158.943 | 158.614 |
| push_back | 256 | 6,340.485 | 6,476.503 | +2.1% | 6,700.791 | 6,633.246 |
| push_front | 256 | 7,452.867 | 7,243.933 | -2.8% | 7,468.537 | 7,611.170 |
| fifo | 256 | 14,272.039 | 13,960.091 | -2.2% | 14,557.646 | 14,549.109 |
| pop_all | 256 | 5,193.290 | 5,410.988 | +4.2% | 5,425.684 | 5,407.620 |
| iter | 256 | 456.613 | 457.350 | +0.2% | 439.092 | 435.226 |
| index | 256 | 509.046 | 529.654 | +4.0% | 518.856 | 517.006 |
| insert_remove | 256 | 6,215.372 | 6,058.475 | -2.5% | 6,026.484 | 6,045.564 |
| rotate | 256 | 3,848.097 | 3,705.997 | -3.7% | 3,767.082 | 3,782.205 |
| contiguous | 256 | 11,764.638 | 11,598.296 | -1.4% | 10,502.839 | 10,482.518 |
| shrink | 256 | 28,158.293 | 26,844.929 | -4.7% | 25,878.251 | 26,634.042 |
| append | 256 | 8,193.993 | 8,477.861 | +3.5% | 8,446.116 | 8,171.028 |
| split | 256 | 5,075.603 | 5,116.251 | +0.8% | 4,947.663 | 5,055.585 |
| clone | 256 | 3,741.901 | 3,577.577 | -4.4% | 3,581.858 | 3,567.335 |
| retain | 256 | 6,815.475 | 4,985.080 | -26.9% | 5,316.023 | 5,400.508 |
| drain | 256 | 4,396.223 | 4,604.828 | +4.7% | 4,496.721 | 4,472.520 |
| resize | 256 | 8,635.452 | 8,255.683 | -4.4% | 8,182.953 | 8,303.125 |
| extend | 256 | 3,681.267 | 3,660.993 | -0.6% | 3,677.616 | 3,606.008 |
| clone_from | 256 | 4,581.364 | 4,682.882 | +2.2% | 4,696.787 | 4,864.706 |
| clone (small) | 8 | 26.221 | 17.969 | -31.5% | 18.619 | 18.761 |
| fifo (small) | 8 | 38.003 | 27.869 | -26.7% | 24.459 | 25.440 |
| push_back (small) | 8 | 40.999 | 46.099 | +12.4% | 49.851 | 54.940 |

The unmetered mean is at most 15% above std in 38 of 39 conditions. The remaining
condition is `iter` with 8-byte elements: +12.506 ns/call in this mixed corpus.
The 32-byte-alignment comparison below isolates its generated-code placement.

`shrink` with 256-byte elements moved from -4.7% (table above) to +11.6% in the
2026-11-09 local campaign. Counters on 2026-11-10 (CPU 6, seed 617, mixed): per
epoch jzap executes 7.90M instructions against std's 8.71M but 5.67M cycles
against 5.14M. Both runs make the same 993 `mmap`/`munmap`/`mremap`/`brk`
calls and 76,750 page faults, and spend the same kernel cycles (208M vs 212M).
The user-mode difference sits at the page-fault entry address (3,266 vs 1,846
samples of 20,000 cycles) while `memmove` samples are lower for jzap (2,228 vs
2,890). Why equal fault counts cost more here is not identified.

## Iteration alignment comparison

The [matched alignment data](verification/vec-deque-iteration-alignment.csv)
contains another 120 intervals, each at least 1.186 seconds. All four variants
(default/32-byte flag, two stack layouts each) use identical counts and inputs
within each seed group. The per-layout std-repeat means range from 0.979 to 1.014.

| Input profile | Mean elements/call | Build option, both implementations | std ns/call | Unmetered ns/call | Difference ns/call | Change |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| mixed | 504.043 | Default | 53.658 | 66.176 | +12.518 | +23.3% |
| mixed | 504.043 | `-align-loops=32` | 52.975 | 52.830 | -0.146 | -0.3% |
| small | 6.099 | Default | 3.251 | 2.608 | -0.643 | -19.8% |
| small | 6.099 | `-align-loops=32` | 3.252 | 2.437 | -0.815 | -25.1% |

The main first-segment loops have the same seven instructions and 30-byte length.
Each implementation's loop bytes are identical between its default and aligned
build. The addresses in these executables are:

| Build | std loop start | Start modulo 64 | jzap loop start | Start modulo 64 |
| --- | --- | ---: | --- | ---: |
| Default | `0x4bd80` | 0 | `0x4b330` | 48 |
| 32-byte request | `0x4d7a0` | 32 | `0x4cce0` | 32 |

| Instruction | Encoded bytes |
| --- | ---: |
| First `movdqu` | 7 |
| First `paddq` | 4 |
| Second `movdqu` | 6 |
| Second `paddq` | 4 |
| Counter `add` | 4 |
| `cmp` | 3 |
| Backward `jne` | 2 |
| Total | 30 |

The default jzap loop spans aligned 32-byte and 64-byte blocks. The aligned
loop fits in one 32-byte block and the observed mixed-input difference closes. The small-input
result and repeated loop body identify a size-dependent placement effect;
12.5 ns is the measured difference for that corpus, rather than a fixed API cost.
The loop contains no extra allocation, failure check or element operation.
Hardware stall events were not measured. The `.text` size changes from
655,123 to 666,819 bytes (+1.8%) with the option. Product source and default
compiler settings are unchanged; the existing [optional tuning setting](build-tuning.md)
selects the flag at the consuming workspace.

## Allocation observations

The final allocator run contains 27,648 isolated observations: 18 workload types,
two widths, three input seeds, 64 cases and four implementations. The compact
[allocation table](verification/vec-deque-memory.csv) has 108 aggregate rows.

| Metric | Unmetered | Local budget | Concurrent budget |
| --- | ---: | ---: | ---: |
| Owner handle, bytes (std: 32) | 32 | 40 | 40 |
| Maximum allocation-call ratio to std | 1.0000 | 1.0000 | 1.0000 |
| Maximum held requested-byte ratio | 1.0000 | 1.0000 | 1.0000 |
| Maximum peak requested-byte ratio | 1.0000 | 1.0000 | 1.0000 |
| Maximum held/peak allocator-usable-byte ratio | 1.0000 | 1.0000 | 1.0000 |

These are observations at the `GlobalAlloc` boundary. Handles are reported
separately from payload allocation. Timing builds exclude this instrumentation.

## Usage and selective reproduction

Import `VecDeque` from `jzap::collections`. Import named iterator types such as
`Iter`, `IterMut`, and `Drain` from `jzap::collections::vec_deque`. Existing code
can also import these types from `jzap::deque`.

```rust
use jzap::collections::VecDeque;

fn example() -> Result<(), jzap::Error> {
    let mut queue = VecDeque::try_with_capacity_and_limit(8, 64)?;
    queue.try_push_back(7u64).map_err(|rejected| rejected.error)?;
    queue.try_push_front(3).map_err(|rejected| rejected.error)?;
    assert_eq!(queue.pop_front(), Some(3));
    Ok(())
}
```

The policy constructors also accept borrowed and owned local/concurrent budgets.
`as_slices` borrows the ring's two initialized segments; `make_contiguous` returns
one initialized mutable slice without allocating.

Run from the repository root with the pinned toolchains on `PATH`:

```sh
python3 scripts/verify.py --area deque --profile quick
python3 scripts/verify.py --area deque --check verus --check kani
python3 scripts/verify.py --area deque --profile memory
python3 scripts/verify.py --area deque --check asan --check fuzz --check coverage
python3 scripts/verify.py --area all --check certificate
```

Each invocation writes its commands, logs and status under ignored `target/`.
The certificate selection runs the whole-library source/link composition,
including release, release LTO, overflow-checked release and debug consumers.
Direct Verus can be run separately with
`python3 scripts/verify_direct_verus.py --target deque --mutations`.

The timing matrix has 18 workload types, each for 8-byte and 256-byte elements,
plus small-input clone, FIFO and push-back conditions. The 64-case corpora include
empty and single-element inputs, 4,096 elements and logarithmically distributed
intermediate lengths. Three seeds select independently shuffled corpora. The controlled replay uses two
stack layouts of the same executable for each seed. Iteration
and indexing use prepared owners; clone_from copies between prepared owners with
enough capacity and then checksums the destination. Destructive tasks include cloning, destruction
and checksumming. FIFO performs one front-pop/back-push cycle per input element.
Reported nanoseconds refer to a complete workload call, rather than one element.

```sh
cargo build --locked --manifest-path tools/throughput-analysis/Cargo.toml --release --bin deque
python3 tools/throughput-analysis/prepare_stack_layout.py \
  --binary tools/throughput-analysis/target/release/deque \
  --output target/verification/vec-deque/layout
python3 tools/throughput-analysis/compare.py \
  --binary stack_0=target/verification/vec-deque/layout/stack_0 \
  --binary stack_16=target/verification/vec-deque/layout/stack_16 \
  --small-tasks deque_clone_8,deque_fifo_8,deque_push_back_8 \
  --seeds 617,6203,96011 --small-variants stack_0,stack_16 \
  --output target/verification/vec-deque/performance
python3 tools/throughput-analysis/report.py \
  --input target/verification/vec-deque/performance \
  --compact target/verification/vec-deque/timings.csv
```

Recompute the individual layout means from the published compact data:

```sh
python3 tools/throughput-analysis/report.py \
  --from-compact docs/verification/vec-deque-timing.csv
```

Recompute the pooled table with Python's standard library:

```sh
python3 - <<'PYDATA'
import csv, collections
sums = collections.defaultdict(lambda: [0, 0.0, 0.0, 0.0, 0.0])
for row in csv.DictReader(open('docs/verification/vec-deque-timing.csv')):
    total = sums[row['profile'], row['task']]
    total[0] += int(row['operations'])
    for i, label in enumerate(['std', 'unlimited', 'local', 'concurrent'], 1):
        total[i] += float(row[label + '_seconds'])
for key, (calls, std, own, local, concurrent) in sorted(sums.items()):
    print(*key, *(round(t * 1e9 / calls, 3) for t in [std, own, local, concurrent]),
          round((own / std - 1) * 100, 2))
PYDATA
```

For the matched default/32-byte iteration comparison, first build the default
executable and layout launchers above, then:

```sh
RUSTFLAGS='-C llvm-args=-align-loops=32' cargo build --locked \
  --manifest-path tools/throughput-analysis/Cargo.toml --release --bin deque \
  --target-dir target/verification/vec-deque/aligned-build
python3 tools/throughput-analysis/prepare_stack_layout.py \
  --binary target/verification/vec-deque/aligned-build/release/deque \
  --output target/verification/vec-deque/aligned-layout
python3 tools/throughput-analysis/compare.py \
  --binary default_0=target/verification/vec-deque/layout/stack_0 \
  --binary default_16=target/verification/vec-deque/layout/stack_16 \
  --binary align32_0=target/verification/vec-deque/aligned-layout/stack_0 \
  --binary align32_16=target/verification/vec-deque/aligned-layout/stack_16 \
  --tasks deque_iter_8 --small-tasks deque_iter_8 \
  --small-variants default_0,default_16,align32_0,align32_16 \
  --seeds 617,6203,96011 --output target/verification/vec-deque/iteration
```

Timing and allocator instrumentation use separate binaries. The allocation
observer records requested and allocator-usable live/peak bytes at the
`GlobalAlloc` boundary, allocation calls and the owner handle size:

```sh
cargo build --locked --manifest-path tools/throughput-analysis/Cargo.toml \
  --release --bin deque --features btree-memory \
  --target-dir target/verification/vec-deque/memory-build
python3 tools/throughput-analysis/memory_report.py \
  --collection-binary target/verification/vec-deque/memory-build/release/deque \
  --format deque --records target/verification/vec-deque/memory.jsonl \
  --output target/verification/vec-deque/memory.csv --max-ratio 1.15
```

The layout control requires Linux `setarch -R` permission. For a normal-ASLR
comparison, pass the compiled binary directly with `--binary default=PATH`.
Restrict either run with `--tasks deque_extend_8,deque_push_back_256` and
`--small-tasks ''`. Each selected interval still lasts at least one second.

Reproduce the independent copy diagnostic:

```sh
cc -std=c11 -D_POSIX_C_SOURCE=200809L -O3 -Wall -Wextra -Werror \
  -fno-builtin-memcpy -fno-ipa-cp tools/throughput-analysis/copy_layout.c \
  -o target/verification/vec-deque/copy-layout
taskset -c 0 target/verification/vec-deque/copy-layout
```
