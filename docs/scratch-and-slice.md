# Fixed scratch storage and checked slices

Status: implementation, domain checks and the final timing/allocation matrix are
complete. Fresh whole-library source/link composition passes all four profiles.
Target:
Rust 1.95 stable, `no_std`, owning scratch with `alloc`.

## Scope

`ScratchBuf<T, P, L>` privately owns an existing `Vec<T, P, L>`. Construction
admits storage once. Subsequent writes use its actual capacity and never grow,
shrink or replace that allocation. Explicit conversion into Vec ends the fixed
storage contract. Default Vec-sized handles reuse the existing capacity word;
optional element-limit and budget policies retain their existing handle costs.
Zero-sized types (ZSTs) follow Vec capacity rules: unlimited owners report
`usize::MAX`, while
an explicitly selected element limit bounds their logical capacity.

`Scratch<'a, T>` borrows caller-owned `[MaybeUninit<T>]` slots, tracks one
initialized prefix and drops that prefix when cleared or released. Its capacity
is the supplied slice length, including for zero-sized elements. The byte
constructor accepts an initialized `[u8]` without clearing it: safe operations
write valid bytes and expose only the used prefix. The backing uninitialized
slice is private and has no safe mutable escape.

`slice::Slice` and `SliceMut` borrow initialized storage. Checked access, range,
splitting, copy and swap return bounds failures. Neither wrapper implements
implicit dereference or indexing. Explicit raw-slice interoperability and
fallible iteration are named operations. Views allocate nothing.

## Use and interoperability

```rust
use jzap::{Error, error::InitError, scratch::{Scratch, ScratchBuf}};

fn reuse() -> Result<(), InitError<Error>> {
    let mut owned = ScratchBuf::<u8>::try_with_capacity(4096)?;
    owned.try_extend_copy(b"header")?;
    owned.clear(); // keeps exactly the same backing allocation

    let mut bytes = [0u8; 256];
    let mut scratch = Scratch::from_bytes(&mut bytes); // no allocation or clearing
    scratch.try_resize_with_init(4, |index, slot| {
        slot.write(index as u8).map_err(|rejected| rejected.error)
    })?;
    let mut view = scratch.as_view_mut();
    let (mut left, mut right) = view.split_at_mut(2)?;
    left.copy_from_slice(&[7, 8])?;
    right.swap(0, 1)?;
    Ok(())
}
```

| Type | Construction and access | Mutation and ownership |
| --- | --- | --- |
| `ScratchBuf<T, P, L>` | `new`, fallible capacity/limit constructors, `from_vec`, initialized slices, checked views/access, fallible iteration, policy/capacity queries | push/pop, copy/clone/iterator extension, resize/slot initialization, truncate/clear, fallible clone/target-budget clone/clone-from, `into_vec` |
| `Scratch<'a, T>` | typed MaybeUninit slice; `from_bytes` for initialized u8; initialized prefix, checked views/access and fallible iteration | push/pop, copy/clone/iterator extension, resize/slot initialization, truncate/clear; never frees the borrowed backing |
| `Slice<'a, T>` | initialized shared borrow, checked get/range/split, first/last, fallible iterator, shallow Copy/Clone/TryClone | explicit raw-slice interoperability; fallible owning `try_to_vec` with `alloc` |
| `SliceMut<'a, T>` | initialized exclusive borrow, shared reborrow, checked access/ranges/disjoint split and fallible mutable iteration | checked swap, matching-length copy/swap, overlap-aware `copy_within`; explicit raw-slice/borrow conversion and fallible `try_to_vec` |

`ScratchBuf::new()` has no capacity for a non-zero-sized element. Use a capacity
constructor or adopt a prepared Vec before writing. Capacity is fixed until
`into_vec` explicitly transfers ownership. A length limit is an additional,
optional policy; it does not authorize growth after construction.

`Scratch::new` takes unowned slots. It owns only values written into its recorded
prefix, not other preexisting values a caller may have placed in MaybeUninit.
Typed backing establishes alignment and lifetime through the Rust borrow.
`capacity()` and `remaining_capacity()` expose slot requirements; element
alignment is `align_of::<T>()`.

Both view wrappers omit Deref and Index. Raw slices are obtained explicitly,
and retain std's indexing/iterator contracts. The named `iter`/`iter_mut` return
`TryIterator` adapters; they do not enable implicit std iteration or collection.
A shared checked slice copies its reference without copying any elements.
A mutable view cannot be cloned. Views and borrowed scratch support core-only
builds; owning scratch and `try_to_vec` require `alloc`.

## Failure and ownership contracts

| Operation | Contract |
| --- | --- |
| Owning construction | Capacity/layout/budget/allocation refusal returns explicitly; no partially initialized owner escapes |
| Push | Full storage returns the original value in `Rejected<T>` before writing |
| Copy extension | Capacity is admitted for the whole source before a bulk write; refusal preserves the original prefix |
| Cloned/initialized extension | Admission precedes callbacks; callback failure or success without a write cleans the new suffix and current slot, preserving the old prefix |
| Fallible iterator extension | Source errors and capacity errors roll back the new suffix; yielded inputs and callback effects follow the consumption contract |
| Truncate/clear/drop | Retire length before destruction; drop only initialized values, retaining the backing allocation |
| Clone | Creates a new owner with its own initial storage; source storage is unchanged |
| Clone-from | Retains destination storage. Bitwise-capable values copy directly after admission; other values stage in spare capacity before retiring the old prefix, refusing insufficient staging space |
| Borrowed views | References remain tied to the original borrow; mutable partitions are disjoint |
| Copy/swap | Check indices or matching lengths before mutation; no temporary allocation |

For non-bitwise clone-from, the destination needs room for its old prefix plus
the complete replacement. Refusal happens before cloning if this staging space
does not fit. Successful replacement removes the old prefix after staging;
bitwise-capable clone-from can overwrite directly after length admission.
`try_clone` and `try_clone_in` allocate a separate owner with the original
capacity and element-limit policy.

Initialized-byte backing remains valid after rollback; writes outside the
restored used prefix are not undone.

Iterator extension restores the scratch prefix on failure, while yielded items
and source side effects remain consumed. A yielded value rejected for capacity
is dropped. Pass a mutable iterator if the caller needs access to its remaining
source. No exact remaining input count is inferred from an iterator's hints.

Element callbacks/cloning can perform their own work and allocation. The
container's fixed storage contract applies to its backing storage. A generic
`TryClone::try_extend_vec` hook is never given the mutable backing Vec; cloning
uses the existing private Copy capability or an individual InitSlot.

## Reference implementations

References are implementation sources, not added runtime dependencies:

- Rust 1.95 Vec, slice and MaybeUninit: capacity admission, safe checked slicing,
  prefix initialization and destruction.
- Rust 1.95 `core::io::BorrowedBuf`: initialized-byte to MaybeUninit borrowing
  while retaining initialized backing bytes. BorrowedBuf itself is unstable in
  this pinned release; the product uses stable primitives.
- heapless 0.9.3 Vec: capacity refusal returning the owned input and initialized
  prefix cleanup. Its archive identity is in [research sources](research/sources.json).

## Executed domain verification

The initial runtime checkpoint is `3167ca6`; the bounds-normalization revision
is `32133b4`.
The common runner records commands, source identity, logs and outcomes under
ignored `target/verification/scratch-slice/`.

| Property | Executed check |
| --- | --- |
| Capacity, exact backing identity and constructor refusal | Native/release/overflow-checked tests, injected allocator refusal, ledger cleanup and zero allocation calls during repeated fixed-buffer writes |
| Prefix ownership and rollback | 12 scenarios in native/release/overflow-checked builds and AddressSanitizer (ASan); current-slot write/error/missing-write cases, suffix cleanup/retry, generic clone hooks, iterator source/capacity errors and destructor-unwind retirement |
| Typed memory | 11 scenarios per mode in x86-64/i686 strict-provenance Miri and Tree Borrows; destructor unwind is checked in native/ASan; zero-sized full-word length, aligned storage and initialized-byte backing cases |
| State transitions | 5,000-step native differential sequence plus 10,000 libFuzzer executions comparing owned scratch, borrowed scratch and std Vec/slices |
| Numeric endpoints and progress | Original `admitted_end`, `range`, `retired_last`, `initialized_next` bodies included directly in Verus; nine verified functions including included dependencies; four source mutants rejected |
| Actual Rust models | Eight Kani harnesses: four scratch and four slice; arbitrary-word endpoints/range normalization/ZST lengths and actual copy/swap/split core guards, plus bounded typed storage/rollback/replacement/partition cases |
| Defect detection | Six compiled product mutants: growing push, wrong prefix length, missing rollback, wrong copy offset, incomplete destruction and inverted copy-length admission |
| Compiler boundaries | Fixed backing privacy, borrowed lifetime, exclusive mutable partitions, no implicit indexing/dereference/std iteration/cloning and local/concurrent policy thread traits; 87 combined collection probes, four weakened-bound controls |
| Source census | 13 new unsafe blocks, two private unsafe helpers and ten metadata sites routed to reviewed callers and actual Rust harnesses; checked-view source itself has no unsafe blocks |
| Build and tooling | Strict all-feature/core Clippy; core-only, wasm, thumb and no-global-OOM builds; Rustdoc; existing runner/report regressions |

All-word numeric proofs and bounded typed-memory checks establish different
properties. Original scratch/slice bodies are checked without Python source-body
projection. Source hashes identify reviewed caller correspondences; they do not
prove pointer provenance. Rust borrows, actual Miri/ASan executions and the
existing allocator/element/unsafe contracts cover that separate boundary. The
remaining legacy projection models elsewhere in the library are unchanged.

## Fresh source/link composition

The fresh run at source checkpoint `04a247c` used no reused model prerequisites.
The complete pipeline returned success with zero open source conditions and zero
unproved linked edges. It reran every existing verification domain and the new
scratch/slice domain before composing the final call graph.

| Consumer profile | Retained guard/forwarding calls | Unproved linked paths |
| --- | ---: | ---: |
| Release | 6 | 0 |
| Release with link-time optimization (LTO) | 9 | 0 |
| Release with overflow checks | 6 | 0 |
| Debug | 80 | 0 |

Retained core guards are bound to original source contracts and reviewed callers;
the counts above are call-graph observations, not separate proof obligations.
Eight intentional panic/bounds/allocation-abort/explicit-abort link controls were
detected in release/LTO. The selected Linux x86-64 fixtures include owned and
borrowed scratch, typed destruction/error cleanup, views, and all five allocation
policies. The raw receipt is generated at
`target/verification/scratch-slice/certificate-final/summary.json` in this run;
`--check certificate` reproduces the global composition in a fresh output folder.
The tool/compiler/core, valid storage/allocator and returning element/callback
contracts are the existing verification boundary described above.

## Partial reproduction

Each invocation creates a local receipt. Select checks independently; the quick
profile does not launch global proof campaigns or timing.

```sh
bash scripts/verify.sh --area scratch-slice --plan
bash scripts/verify.sh --area scratch-slice
bash scripts/verify.sh --area scratch-slice --check release --check overflow
bash scripts/verify.sh --area scratch-slice --check verus
bash scripts/verify.sh --area scratch-slice --check kani
bash scripts/verify.sh --area scratch-slice --check models
bash scripts/verify.sh --area scratch-slice --profile memory
bash scripts/verify.sh --area scratch-slice --check fuzz --check bounds --check unsafe
# Fresh whole-library composition, including this domain and all four profiles:
bash scripts/verify.sh --check certificate
```

Here `verus` includes original numeric modules; `kani` runs original Rust
harnesses; `models` runs the scratch/slice source correspondence, Kani and six
compiled controls. No generated proof model or source snapshot is committed.
The global certificate also reruns the existing domains and their distinct
proof mechanisms. Tool installation and other selectable checks are documented
in the [verification guide](verification/README.md).

## Code generation controls

[Paired code-generation records](verification/scratch-slice-codegen.csv) contain
81 intervals of at least one second: three seeds, three operations, three build
variants and std/repeated-std/product labels. The baseline uses `3167ca6`; the
source variant separates bound normalization from range admission. Each cell
below is std ns/call / product ns/call (product change).

| Operation | Baseline default | Both with `-align-loops=32` | Revised source, default |
| --- | ---: | ---: | ---: |
| u64 checked get scan | 201.005 / 305.270 (+51.9%) | 200.792 / 209.755 (+4.5%) | 205.725 / 215.939 (+5.0%) |
| u64 range view | 1.995 / 2.539 (+27.3%) | 1.967 / 2.601 (+32.2%) | 1.963 / 2.095 (+6.8%) |
| 256-byte range view | 2.080 / 2.667 (+28.2%) | 2.054 / 2.636 (+28.3%) | 2.086 / 2.137 (+2.4%) |

All measured methods inline. Adding inline hints alone produced identical `.text`
bytes. In the u64 scan, std uses eight instructions/31 bytes; the product uses
nine instructions/30 bytes, including one register copy and a rotated boundary
branch. The original timed product loop starts at executable offset `0x1fa70`
and crosses a 64-byte boundary; std's `0x1a4e0` loop fits. The source revision
moves the same product loop to `0x1fa50`, which fits. Matched 32-byte alignment
also removes most of the scan gap while preserving its instructions.

Range views previously checked normalized endpoints in `geometry::range` and
again in safe slice `get`/`get_mut`. They now normalize bound forms once and
admit the range through those safe methods. `copy_within` still uses full checked
range admission before computing its count. The revision adds no unsafe code.
An extra explicit get precheck produced the same executable code and was removed.
The code-generation CSV calls this source variant `guard`.

These controls identify instruction placement and duplicate range admission as
separate effects. Loop alignment remains an optional consumer compiler setting;
the product does not force it. The final complete timing matrix follows below.

## Final timing and allocation matrix

Runtime source: `32133b4`. The final matrix uses the default compiler settings,
three seeds and two controlled initial-stack layouts of the same sealed binary.
Address space layout randomization (ASLR) is disabled only for benchmark children. Every one of the 480 accepted
intervals is at least one second. The largest repeated-std mean drift within a
layout is 7.1%. [The compact records](verification/scratch-slice-timings.csv)
retain epochs, calls, input identity, result checksum and each label's seconds.

The following means divide summed measured seconds by summed calls over both
layouts and all three seeds. A call is a complete prepared-buffer operation:
push-all/pop-all, refill, resize, initialization, clone-from, full get scan,
range lookup or overlapping half-copy. Constructor/source setup is outside timing.
Blank policy columns identify borrowed scratch and checked-view operations.

| Operation / element bytes | std ns/call | Unmetered | Change | Local budget | Concurrent budget |
| --- | ---: | ---: | ---: | ---: | ---: |
| push_pop / 8 | 383.586 | 394.248 | +2.8% | 397.798 | 392.858 |
| extend / 8 | 83.956 | 83.514 | -0.5% | 85.176 | 82.861 |
| resize / 8 | 77.813 | 77.535 | -0.4% | 78.194 | 76.518 |
| init / 8 | 75.913 | 75.013 | -1.2% | 74.705 | 74.696 |
| clone_from / 8 | 81.193 | 81.246 | +0.1% | 80.262 | 81.558 |
| borrowed_push_pop / 8 | 372.722 | 388.069 | +4.1% | — | — |
| borrowed_extend / 8 | 81.145 | 82.915 | +2.2% | — | — |
| get / 8 | 201.827 | 216.340 | +7.2% | — | — |
| range / 8 | 1.998 | 2.075 | +3.8% | — | — |
| copy_within / 8 | 42.134 | 42.042 | -0.2% | — | — |
| push_pop / 256 | 7,062.549 | 5,336.085 | -24.4% | 5,199.323 | 5,020.282 |
| extend / 256 | 4,259.230 | 4,369.511 | +2.6% | 4,356.768 | 4,251.025 |
| resize / 256 | 2,316.551 | 2,288.546 | -1.2% | 2,317.288 | 2,310.852 |
| init / 256 | 2,371.577 | 2,302.784 | -2.9% | 2,341.832 | 2,348.778 |
| clone_from / 256 | 4,501.123 | 4,480.014 | -0.5% | 4,555.165 | 4,628.819 |
| borrowed_push_pop / 256 | 6,948.038 | 7,181.732 | +3.4% | — | — |
| borrowed_extend / 256 | 4,560.130 | 4,551.295 | -0.2% | — | — |
| get / 256 | 485.202 | 509.598 | +5.0% | — | — |
| range / 256 | 2.097 | 2.187 | +4.3% | — | — |
| copy_within / 256 | 1,493.911 | 1,495.818 | +0.1% | — | — |

All 20 combined unmetered means are at most std +7.2%; 39 of the 40 individual
layout means are within std +15%. Borrowed 256-byte push/pop measures +54.8%
in one layout and −30.7% in the other, giving +3.4% combined.

The timed by-value push loops in both std and borrowed scratch contain two
256-byte copies through a stack temporary. The warmup can use a different
single-copy form; it is excluded from measured intervals. A fresh replay of the
existing [copy-layout diagnostic](../tools/throughput-analysis/copy_layout.c)
changes only the temporary's offset from a 64-byte boundary. Its
[12 recordings](verification/scratch-slice-copy-layout.csv) give:

| Temporary offset | Two copies, ns/block | Source → temporary only | Temporary → destination only |
| --- | ---: | ---: | ---: |
| 0 | 8.787 | 3.577 | 4.617 |
| 16 | 15.280 | 3.978 | 4.740 |
| 32 | 8.277 | 3.580 | 4.586 |
| 48 | 15.182 | 3.928 | 4.971 |

The two-copy sequence has a large phase effect; each copy alone changes much
less. Adding an inline hint to borrowed push also produced identical `.text`
bytes and was discarded. Type alignment, backing layout and default compiler
options are unchanged. No hardware performance-counter event was measured.

[Allocation records](verification/scratch-slice-memory.csv) cover element widths
8/256, lengths 0/1/4/32/4096, and std/unmetered/local/concurrent owners in 40 fresh
processes. Requested held/peak bytes, allocator usable held/peak bytes and
allocation-call counts match std in every case. Nonempty owners allocate once;
repeated refill/resize/initialization performs zero further allocation calls.
After destruction the measured live and usable bytes return to zero.

The separate initialized-byte scratch observation performs no allocation from
construction through clear/drop. Its 4,096-byte backing is caller-provided.
These observations record requested and allocator-usable bytes, not resident
memory. Container callback/element allocations follow their own contracts.

| x86-64 handle | Bytes |
| --- | ---: |
| std Vec / unmetered ScratchBuf | 24 / 24 |
| ScratchBuf with borrowed local/concurrent budget | 32 |
| Borrowed Scratch | 24 |
| Slice / SliceMut / raw slice reference | 16 / 16 / 16 |

### Measurement reproduction

Build and finish all checks before running timing; compilation/proof jobs must
not run concurrently with it. The layout control requires Linux `setarch -R`.

```sh
cargo build --locked --manifest-path tools/throughput-analysis/Cargo.toml --release --bin scratch_slice
python3 tools/throughput-analysis/prepare_stack_layout.py \
  --binary tools/throughput-analysis/target/release/scratch_slice \
  --output target/verification/scratch-slice/layout
python3 tools/throughput-analysis/compare.py \
  --binary stack_0=target/verification/scratch-slice/layout/stack_0 \
  --binary stack_16=target/verification/scratch-slice/layout/stack_16 \
  --small-tasks '' --seeds 617,6203,96011 \
  --output target/verification/scratch-slice/timing
python3 tools/throughput-analysis/report.py \
  --input target/verification/scratch-slice/timing \
  --compact target/verification/scratch-slice/timings.csv
python3 tools/throughput-analysis/report.py \
  --from-compact docs/verification/scratch-slice-timings.csv
```

The final command recomputes every per-layout mean from committed CSV without
raw logs. Combine layouts by summing their seconds and calls before division.
The same command accepts `scratch-slice-codegen.csv` for the three source/flag
controls.

Allocation instrumentation is compiled separately. Run each tuple in a new
process to match allocator history; `memory LABEL WIDTH LENGTH` selects one case:

```sh
cargo build --locked --manifest-path tools/throughput-analysis/Cargo.toml \
  --release --bin scratch_slice --features btree-memory \
  --target-dir target/verification/scratch-slice/memory-build
# Repeat for std/unlimited/local/concurrent, widths 8/256, lengths 0/1/4/32/4096.
target/verification/scratch-slice/memory-build/release/scratch_slice memory std 256 4096
target/verification/scratch-slice/memory-build/release/scratch_slice memory unlimited 256 4096
```

The instrumented executable is not used for timing. The shared copy diagnostic
can be compiled with `cc -O3 -fno-builtin-memcpy` and run independently; it
calibrates every interval to at least one second.
