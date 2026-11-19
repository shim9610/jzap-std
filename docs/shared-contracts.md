# Shared ownership contracts

Implementation scope: explicit fallible counterparts of Rust 1.95 stable Rc/Weak
and Arc/Weak, including dynamically sized payloads, cyclic construction,
uninitialized storage, raw ownership, pinning, uniqueness and copy-on-write.
Existing Box/Vec/String work remains intact. No runtime dependency is added.

## Ownership and accounting

A shared allocation has two counters: strong owners, and explicit weak owners
plus one implicit weak credit while the payload is alive or being destroyed.
Cyclic construction begins with zero strong owners and one construction weak;
upgrade returns absence until payload initialization is published. Last strong
release destroys the payload exactly once and then releases the implicit weak.
Last weak release frees storage and refunds its complete charge, including when
the payload destructor unwinds. Zero-sized values still require a control block.

Count admission rejects saturation before changing the counter. Rc uses a Cell
transition; Arc uses a compare-exchange loop, without transient overflow or a
standard clone call. Upgrade distinguishes dead payload (Ok(None)) from a live
saturated counter (Err). Acquiring a mutable borrow requires one strong owner and
no explicit weak owner. Arc temporarily locks the weak counter to establish this
condition; downgrade observes that lock. Counter decrements use release ordering
and final destruction acquires prior releases. Failed unwrapping preserves the
owner. Arc into_inner differs from try_unwrap: among concurrent consumed owners,
exactly one retrieves the value.

Copy-on-write reserves replacement storage before retiring the original owner.
Other strong owners require a fallible deep clone; only weak owners require a
move and invalidate their upgrade ability. Allocation or clone refusal preserves
the original owner. No fallible operation follows a successful retirement before
replacement publication. Callbacks retain the existing return/execution contract.

A handle clone leaves the allocation and its original budget unchanged, including
TryCloneIn into a different budget. New deep copies explicitly choose accounting.
A control block records the charge once; no per-handle accounting or hidden
allocation is used. The default Unmetered policy type matches std's two-counter
header. Budget and ConcurrentBudget policy types add one reference word; even
the optional Budget::Unmetered value retains that word. After the final strong release, the
strong-counter word is tagged as retired and its remaining bits store the
allocation byte charge. This preserves exact accounting after a dynamically
sized payload is destroyed without adding a separate byte-count field. Upgrade
rejects both zero and tagged retired states before count admission.

## Concurrent budget

The caller owns ConcurrentMemoryBudget without heap allocation. One atomic used
counter represents reserved plus committed bytes; reserve admits a request with
one successful compare-exchange, retrying observed conflicts. Commit transfers
token ownership without changing
used. Cancellation or final release subtracts exactly one token's bytes. Thus
reserved plus occupied never exceeds the configured limit. Concurrent used and
available queries are individual observations, not a multi-field snapshot. The
existing local budget and its handle representation remain unchanged.

ConcurrentBudget is a sealed allocation policy usable by Box, Vec, String, Rc and Arc.
The accounting sum is exact for requested layouts, not allocator rounding or
process resident memory.
Neither a reservation nor a budget operation invokes user code or holds a lock
across allocation/initialization/destruction. Budget admission uses a lock-free
compare-exchange loop. Arc admission retries observed counter changes; downgrade
also waits for a concurrent uniqueness probe to release its weak-counter lock.
That probe executes only atomic steps and calls no user code. Progress requires
the lock holder to resume; no per-thread completion bound is claimed.

## References and verification routine

Reviewed before implementation: Rust 1.95 (`59807616e`) alloc rc.rs/sync.rs;
nightly-2026-10-30 (`92b5b8c87`) alloc rcs/rc.rs and rcs/arc.rs;
triomphe 0.1.17; portable-atomic-util 0.2.8 (based on Rust 1.93). The latter shows
stable-Rust deallocation of a dead dynamically sized payload via a standard Box
of a ManuallyDrop tail, without constructing a reference to the dead payload.
This path performs deallocation only. Their infallible count/allocator behavior
is not adopted. Varlit's shared-storage/COW ordering is an additional reference.

Pinned third-party references are implementation studies, not product
requirements or dependencies:

| Reference | Reviewed role | Stable/toolchain evidence |
| --- | --- | --- |
| [triomphe 0.1.17](https://docs.rs/crate/triomphe/0.1.17/source/src/arc.rs) | Strong-only Arc layout and ownership transitions; no Weak support | Package declares Rust 1.85; optional unstable drop-check feature is separate |
| [portable-atomic-util 0.2.8](https://docs.rs/crate/portable-atomic-util/0.2.8/source/src/arc.rs) | Stable DST allocation, metadata transfer and ManuallyDrop-tail deallocation | Package declares Rust 1.36; atomic backend is its dependency, not adopted here |

The current product is compiled and tested on Rust 1.95; these manifest minimums
are reference metadata, not independently verified minimum-version claims.
Reference source SHA-256 values:

```text
triomphe-0.1.17/src/arc.rs
68a8048894d2c02d7a13b7392a0a958a4bedcc54916c0cae1227af1b5f28d593
portable-atomic-util-0.2.8/src/arc.rs
e23f18ca0592f694ac2da85e9b83ea3add4fad978cd37ac05c742af254fa82a9
portable-atomic-util-0.2.8/src/utils.rs
afe133c869f3a833a8a3d35afeb96e4caffc39da5c5bc063501b0293ae0344db
```

The verification routine combines source-connected all-word count, admission
and layout proofs, reduced count-limit
saturation tests, allocation and initialization failure injection, differential
state-machine/property/fuzz tests, Miri with both alias models, sanitizers,
compiler thread/lifetime rejection tests, and Loom over the actual atomic
implementation. Final-link inventories must reject new unproved exits. It runs
release, fat link-time optimization (LTO), overflow-checked release and debug
guard attachment.

Performance inputs are frozen before measurement: 64 shuffled cases, three seeds,
equal work across std/control/product/policy variants, every retained interval
at least one second. Compare construction/drop, strong/weak clone/drop,
downgrade/upgrade, unique access, unwrap, copy-on-write and cyclic/slice creation. Record
requested/actual final and peak allocations, fixed handle/header costs, and
assembly differences. Use the existing observed +15% comparison threshold;
separate uncertainty and explicitly identified policy costs. Raw artifacts stay
under ignored target/verification/shared; maintained summaries are in English.
