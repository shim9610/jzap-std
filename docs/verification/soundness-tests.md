# Soundness tests outside the formal proofs (2026-11-13)

The Verus and Kani proofs cover the path from lengths, indices and offsets to
memory accesses. They do not cover auto-trait bounds, variance, lifetimes of
views, callbacks that unwind, or pointer accesses on paths no test runs. This
document records the tests added for those properties, one source defect they
found, and the results at `fe79766`.

## What was added

| File | Property | Cases |
| --- | --- | --- |
| `tests/auto_traits.rs` | `Send`/`Sync` of each public type equals its std counterpart for an element that is both, only `Send`, only `Sync`, or neither; expected values for budget and policy types; covariance of owners and shared iterators | 85 type rows × 4 elements, 19 policy rows, 21 covariance functions |
| `tools/compile-fail/` | Mutable views and entries are invariant in the element; views do not outlive or alias their owner; thread-bound values do not cross threads | 43 pairs |
| `tests/panic_callbacks.rs` | A panic in `Ord`, `Hash`, `Eq`, `TryClone`, `Drop` or a closure, at every invocation of an operation, drops no element twice, leaks none, and leaves the container usable | 20 tests |
| `tests/fuzz_replay.rs` | The 11 libFuzzer target bodies, compiled without copying, run on generated inputs so that Miri can execute them | 11 tests |
| `tests/unsafe_reach.rs` | Unsafe blocks that no other test executed | 5 tests |
| `scripts/unsafe_coverage.py` | Lists unsafe blocks in `src/` with no executed line under LLVM source coverage | 581 blocks |

Each compile-fail case is a pair with the same prelude: one doctest must
compile and one must be rejected. A nightly toolchain also checks the error
code. `tests/panic_callbacks.rs` contains a control: a deliberately defective
owner is reported as a double drop.

`tests/fuzz_replay.rs` uses deterministic pseudo-random bytes. A corpus found
by libFuzzer is not stored in the repository and was not replayed.

## Findings

**Auto traits.** All rows equal std. `InitSlot` is neither `Send` nor
`Sync`, narrower than the `&mut T` it stands for.

**Unexecuted unsafe blocks.** Before these tests, 18 of 581 unsafe blocks had
no executed line in any integration test, among them both blocks of the
in-place rehash `RawTable::rehash`. `tests/unsafe_reach.rs` and the rehash
sweep in `tests/panic_callbacks.rs` now execute all 18. The remaining 17 have
no coverage record on x86-64 Linux:

| Blocks | Location | Reason |
| --- | --- | --- |
| 12 | `boxed/call.rs`, `boxed/owned.rs`, `boxed/pinned.rs`, `shared/unsize.rs` | bodies of exported macros; coverage is attributed to the expansion site |
| 3 | `hash_map/entropy.rs` `os_seed` | branches for other operating systems |
| 2 | `hash_map/group.rs` | portable group scan, compiled when the target is not x86-64; executed by the i686 Miri run |

A block counts as executed when one of its lines ran. This does not show that
every branch inside it ran.

**Returned half leaked when the discarded half's destructor unwinds.** Six
bodies returned one half of a removed pair and dropped the other half after
the returned half had been moved to the return place. When that destructor
panicked, the returned half was never dropped. std's `BTreeMap::remove` has
the same body and leaked the same single value in the same sweep. This is a
leak, not a double drop.

| Function | Before (`efa06b2`) | After (`eab055f`) |
| --- | --- | --- |
| `BTreeMap::remove`, `HashMap::remove` | `self.remove_entry(key).map(\|(_, v)\| v)` | bind both halves, `drop` the key, return the value |
| `btree_map::OccupiedEntry::remove`, `hash_map::OccupiedEntry::remove` | `self.remove_entry().1` | same |
| `btree_map::IntoKeys`/`IntoValues` (`next`, `next_back`) | `\|(k, _)\| k`, `\|(_, v)\| v` | `\|(k, v)\| { drop(v); k }`, `\|(k, v)\| { drop(k); v }` |
| `hash_map::IntoKeys`/`IntoValues` (`next`, `fold`) | same | same |

On the returning path the order is unchanged: the discarded half is dropped,
then the other half is returned. Only the unwind path changed. The sweep runs
29 operations of this shape; 12 leaked before the change and none after.

Release assembly of eight monomorphic probes (`remove`, entry `remove`,
`IntoKeys::next`, `IntoValues` `next_back`/`fold`, for both maps):

| Key and value | Result |
| --- | --- |
| `u64` | 8 of 8 functions identical before and after |
| `String` | 7 of 8 differ |

In the two `String` functions read line by line (`btree_map::OccupiedEntry::remove`,
`hash_map::IntoKeys::next`) the calls and branches are the same; the returned
value is written to the return place after the deallocation call instead of
before it and is kept in callee-saved registers meanwhile, which adds two to
three push/pop pairs. The other five were compared by line count only. Timing
was not measured.

The six files are covered by whole-file review hashes in
`scripts/btree_bindings.py` and `scripts/hash_map_bindings.py`. The diff
contains no unsafe block, index or arithmetic; the hashes of the six files were
updated.

## Results at `fe79766`

| Check | Result |
| --- | --- |
| `cargo test --locked --offline --all-features` | 433 passed, 0 failed |
| `cargo fmt --all --check`; clippy on the library with all features and with none, `-D warnings` | pass |
| `tools/compile-fail` doctests, 1.95.0 and nightly-2026-10-30 | 43 accepted and 43 rejected on both |
| Miri, strict provenance, four new test files | 39 passed (3, 11, 20, 5) |
| Miri, strict provenance and Tree Borrows | 39 passed |
| Miri, strict provenance, `i686-unknown-linux-gnu` | 39 passed |
| `scripts/unsafe_coverage.py`, all integration tests | 581 blocks: 564 executed, 0 unexecuted, 17 without a record |
| B-tree, HashMap, BTreeSet, HashSet area checks with Kani and controls | pass, after the review-hash update |

The first run of the B-tree and HashMap area checks after the source change
was refused by the review hashes. An earlier Miri run was stopped by the time
limit: the setup of the rehash sweep did not terminate at the reduced Miri
size; the sweep now uses a fixed table size.

Under Miri the sweeps use 24 elements and sample about 12 panic positions per
scenario; natively they use 96 elements and every position.

The global source certificate was then run at `5257f2f`, which contains the
changed source: complete, 1,141 obligations connected and 0 open, 101 retained
link edges and 0 unproved, no reused models. See
[`../direct-source-migration.md`](../direct-source-migration.md).

Not run: ASan, timing benchmarks, and any platform other than x86-64 Linux and
the i686 Miri interpretation.

## Reproduction

```sh
cargo test --locked --offline --all-features --test auto_traits --test panic_callbacks --test fuzz_replay --test unsafe_reach
cargo test --locked --offline --manifest-path tools/compile-fail/Cargo.toml --doc
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly-2026-10-30 miri test --locked --offline --all-features \
  --test auto_traits --test panic_callbacks --test fuzz_replay --test unsafe_reach
python3 scripts/unsafe_coverage.py
```

Add `-Zmiri-tree-borrows` to `MIRIFLAGS` or `--target i686-unknown-linux-gnu`
for the other two Miri configurations.
