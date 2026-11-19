# jzap-std

**jzap-std** stands for **J**ustified **Z**ero-**A**bort **P**anic-free **Std**.
It provides fallible ownership types and checked operations for Rust. The package
is `jzap-std`, and the library is imported as `jzap`.

**Version `0.1.0-beta.1`** is a prerelease for evaluating the current APIs and
failure contracts. APIs may change before `0.1.0`. The support guide lists
implemented features, remaining work, and verification evidence. [Beta support status](docs/support.md)

## Getting started

Before publication, point your application's `Cargo.toml` at a local checkout.
Adjust the path to match your checkout location.

```toml
[dependencies]
jzap-std = { path = "../jzap-std", version = "=0.1.0-beta.1" }
```

```rust
use jzap::{boxed::Box, budget::{Budget, MemoryBudget}, TryClone, TryCloneIn};

fn example() -> Result<(), jzap::Error> {
    let original = Box::try_new(42_u64).map_err(|rejected| rejected.error)?;
    let copy = original.try_clone()?;
    let budget = MemoryBudget::new(1024);
    let metered = copy.try_clone_in(Budget::Limited(&budget))?;
    assert_eq!(*metered, 42);
    Ok(())
}
```

Once the beta appears on crates.io, use the exact prerelease version in your
application:

```toml
[dependencies]
jzap-std = "=0.1.0-beta.1"
```

The library requires Rust 1.95 or later and has no external runtime dependencies.

| Cargo configuration | Available APIs |
| --- | --- |
| Default (`alloc`) | `no_std` with owning containers. Hash tables accept explicit seeds or hash builders. Atomic shared owners require pointer-width atomics. |
| `default-features = false` | Core-only APIs including checked arithmetic, borrowed scratch storage, and checked slices. |
| `features = ["std"]` | Adds OS string and path interoperability and lets `HashMap` and `HashSet` obtain seeds on [supported platforms](docs/random-state.md#platform-boundaries-and-premises). |

Import `BTreeMap`, `HashMap`, and `VecDeque` from `jzap::collections`. Named deque
iterator types such as `Iter` and `Drain` live under `jzap::collections::vec_deque`.

## Documentation

| Documentation | Purpose |
| --- | --- |
| [Selectable verification](docs/verification/README.md) | Per-area tests, memory checks, direct Verus, projected models and final links; exact commands and local logs |
| [Optional build tuning](docs/build-tuning.md) | Opt-in 32/64-byte loop alignment; compiler defaults remain the default |
| [Memory budget guide](docs/budget.md) | Borrowed and owning policies, thread-safe application state, bootstrap costs and reallocation admission |
| [Owning budget verification](docs/verification/owned-budgets.md) | Lifetime, failure, concurrency, source-to-link and paired performance checks |
| [Arc application workflows](docs/verification/arc-scenarios.md) | Shared state, request fanout, page COW, contention, memory costs and practical suitability |
| [Arc headroom experiment](docs/verification/arc-headroom.md) | Historical locked-increment clone experiment and its saturation findings |
| [BTreeMap guide](docs/btree.md) | Fallible ordered maps, Entry operations and caller-owned node slices |
| [BTreeMap verification](docs/verification/btree.md) | Acceptance checks, allocation contracts and std comparison |
| [BTreeSet guide and results](docs/btree-set-design.md) | Stable API counterparts, failure contracts, memory checks, per-operation std comparisons and selective reproduction |
| [HashMap guide and results](docs/hash-map-design.md) | Fallible map/Entry APIs, keyed hashing, ownership and budget contracts, verification and std comparisons |
| [HashSet guide and results](docs/hash-set-design.md) | Fallible set algebra, replacement, iteration, source checks and std timing/allocation comparisons |
| [BinaryHeap guide and results](docs/binary-heap-design.md) | Fallible priority queues, mutable peek, ownership recovery, selective checks and direct std comparisons |
| [Scratch and checked slices](docs/scratch-and-slice.md) | Fixed Vec-backed/borrowed storage, tracked initialization and checked slice operations |
| [VecDeque guide and results](docs/vec-deque-design.md) | Fallible ring storage, wrapping and contiguous conversion, ownership/iteration, verification and std comparisons |
| [Shared ownership guide](docs/shared.md) | Fallible Rc/Arc/Weak, pinning and concurrent memory budgets |
| [Shared API mapping](docs/shared-api.md) | Stable std counterparts, explicit conversions and coercions |
| [String guide](docs/string.md) | UTF-8 ownership, byte limits, checked editing and fallible formatting |
| [String API mapping](docs/string-api.md) | Explicit counterparts for all 38 stable inherent methods |
| [Vec guide](docs/vec.md) | Fallible growth, construction-time element limits, and implemented editing APIs |
| [Vec checkpoint and benchmarks](docs/verification/vec-optimization.md) | Verified scope, repeated operation means, optional alignment and remaining regressions |
| [ExtractIf deletion-count experiment](docs/verification/vec-extract-deleted-count.md) | Retained-prefix improvement, all-selected regression, repeated timing and focused proof scope |
| [ExtractIf normal-inlining follow-up](docs/verification/vec-extract-inline.md) | Five-instruction selected loop; repeated timing passes the adjusted std +15% criterion in all 12 measured u64 conditions |
| [Splice batch verification and timing](docs/verification/vec-splice-batch.md) | Verified batch staging, default-build comparisons, remaining small-input gaps and reproduction |
| [Box guide](docs/box.md) | Usage, ownership, errors, initialization, and adapters |
| [Support status and remaining work](docs/support.md) | Implemented scope, remaining specification items, and measurement/verification follow-up |
| [Box contracts](docs/box-completion-contracts.md) | Implementation and benchmark contracts |
| [Box API and verification](docs/verification/box-completion/README.md) | Feature mapping, executed checks, and evidence |
| [Beta release procedure](docs/releasing.md) | Package checks and publication from an approved candidate |
| [Performance methodology](docs/benchmarks.md) | Conditions, uncertainty, reproduction, and remaining work |
| [1MiB string clone analysis](docs/verification/box-str-clone-cause/README.md) | Actual binary paths and the reported 6.4% estimate |
| [Foundation contracts](docs/foundation-contracts.md) | Clone, initialization, and budget semantics |
| [Reference provenance](https://github.com/shim9610/jzap-std/blob/main/reference/README.md) | Fixed source snapshots and their licenses |

## Build and verification

From the repository checkout, run the basic checks with Rust 1.95 and the locked
development dependencies.

```sh
cargo test --locked --all-features
cargo clippy --locked --lib --all-features -- -D warnings
cargo check --locked --no-default-features
cargo doc --locked --all-features --no-deps
```

GitHub Actions runs Rust tests, selected Miri, Verus, and Kani checks, and
CodeQL scanning for pull requests targeting `main` and pushes to `main`.
[CI coverage](docs/verification/README.md#continuous-integration)

For a single-commit export, see [snapshot preparation and regression checks](docs/verification/README.md#snapshot-regression-checks).
The [verification index](docs/verification/README.md) describes Miri, sanitizers,
Kani, Verus, fuzzing, mutation checks, and local scripts. Benchmark tooling has
additional prerequisites, including a pinned reference checkout. It is separate
from the runtime dependency graph.

Choose either the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE). Reference snapshots
retain their upstream notices. The original [project specification](docs/jzap_std_specification_final.md)
and repository working instructions are retained in Korean; the Box, Vec, String, shared-ownership and collection publication
narrative is in English.
