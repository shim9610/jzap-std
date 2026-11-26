# Choosing a memory budget policy

Use `OwnedBudget` for local ownership and `OwnedConcurrentBudget` for ownership
that crosses threads or a `Send + 'static` task boundary. Both keep their ledger
alive through the existing fallible Rc/Arc implementation. A container storing
either policy has no budget lifetime parameter.

| Policy | Ledger storage | Thread sharing | Creation and duplication |
| --- | --- | --- | --- |
| `Unmetered` | None | Supported | No allocation or duplication failure |
| `Budget<'a>` | Borrows `MemoryBudget` | Local | Borrow duplication does not allocate |
| `ConcurrentBudget<'a>` | Borrows `ConcurrentMemoryBudget` | Supported | Borrow duplication does not allocate |
| `OwnedBudget` | Fallible Rc owns `MemoryBudget` | Local | `try_new` allocates once; `try_clone` can return `RefCount` |
| `OwnedConcurrentBudget` | Fallible Arc owns `ConcurrentMemoryBudget` | Supported | `try_new` allocates once; `try_clone` can return `RefCount` |

Owning policies require `alloc`; concurrent policies also require pointer-width
atomics. They introduce no runtime dependency. Default containers continue to
use `Unmetered`.

## Application state without a borrowed budget lifetime

```rust
use jzap::{Error, TryClone, budget::OwnedConcurrentBudget, vec::Vec};

struct AppState {
    budget: OwnedConcurrentBudget,
    bytes: Vec<u8, OwnedConcurrentBudget>,
}

impl AppState {
    fn try_new(limit: usize) -> Result<Self, Error> {
        let budget = OwnedConcurrentBudget::try_new(limit)?;
        let bytes = Vec::try_with_capacity_in(64, budget.try_clone()?)?;
        Ok(Self { budget, bytes })
    }
}

fn require_state<T: Send + Sync + 'static>() {}
fn check() { require_state::<AppState>(); }
```

The same policy works with Box, String, Rc/Arc and BTreeMap. Containers duplicate
the policy when they need another owner and move it during consuming transfers.
Dropping the original budget handle leaves the containers' ledger owners alive.
Shared object handles use one policy in their control block; their final Weak
release frees storage and refunds its charge before dropping that policy.

`OwnedBudget` provides the corresponding local form for a UI callback or other
local state that needs `'static` ownership. Its Rc ledger keeps it local. Data
types' own lifetime and Send/Sync requirements still apply.

## Queries, tokens and scoped borrows

`limit` and `available` query either owning policy. Local accounting exposes
`occupied` and `reserved`; concurrent accounting exposes combined `used` bytes.
`reserve` returns the existing reservation token, whose cancellation, commitment
and charge destruction follow the original ledger protocol.

`ledger()` borrows the underlying ledger. This permits an explicit scoped policy
when the owning handle already stays alive for the complete operation:

```rust
use jzap::{Error, budget::{ConcurrentBudget, OwnedConcurrentBudget}, vec::Vec};

fn scoped_work(owner: &OwnedConcurrentBudget) -> Result<(), Error> {
    let mut scratch = Vec::<u8, _>::new_in(ConcurrentBudget(owner.ledger()));
    scratch.try_extend_copy(&[1, 2, 3])?;
    Ok(())
}
```

The scoped borrow avoids acquiring another ledger owner. APIs explicitly taking
a borrowed `Budget<'a>`, including the existing `TryCloneIn<'a>` target, retain
their declared borrow contracts. Owning construction and ordinary `TryClone`
keep ownership in the chosen policy type.

## Allocation and admission costs

On the checked 64-bit target, either owning policy handle is eight bytes. A
domain's bootstrap allocation requests 40 bytes for the local Rc ledger or
32 bytes for the concurrent Arc ledger. Allocator usable size can be larger.
This allocation occurs once per domain, survives through its last policy owner,
and is freed afterwards. Cloning a policy acquires a count credit without
allocating another ledger.

Bootstrap storage is allocated with `Unmetered`. The domain's requested-byte
limit governs storage charged to that ledger and starts with zero occupied/
reserved bytes. Thus creating an owning zero-byte budget still needs its ledger
allocation and can return `Error::Allocation`. The separate bootstrap cost is
observable in the allocator regression; it is included when measuring overall
heap usage. Existing borrowed ledgers use caller-provided storage.

Owning policies keep the established conservative reallocation contract:
reserve the new requested layout while the old charge remains, then refund the
old layout after successful reallocation. For example, 60 → 100 can be refused
under a 150-byte limit. The [reallocation review](budget-review.md) explains the
standard allocator's moving default, shrink behavior, planned capacity, and the
distinct proposed retained-size contract.

Local owner duplication adds Rc count work. Concurrent duplication adds an Arc
count acquisition; destruction releases it after the ledger's storage charge is
handled. These owner changes occur when policy ownership changes. Buffer growth
keeps its current policy, and shared handle clones reuse the header policy.

The [application-workflow evaluation](verification/arc-scenarios.md) measures
these costs with 1/2/4 workers. Long-lived state with substantial reads remains
close to std. High-rate allocation through one shared concurrent domain creates
ledger contention, and high-frequency cloning of the same object also contends
without a budget. For large values built from source data, `try_init_in` can
avoid a temporary payload copy retained by by-value construction. Request-local
owned domains add a bootstrap allocation to each request and enforce separate
quotas. The report records the timing and memory tradeoffs for these choices.

### Measured cost breakdown (2026-11-09, local)

i7-13700KF, CPU 6 fixed at 3.4 GHz (turbo off), Rust 1.95.0; per-call counts are
`perf stat` differences between two run lengths, timing uses the one-second
protocol with seeds 617/6203/96011.

| Path | Cause (assembly / counters) | Measured |
| --- | --- | --- |
| Vec with a local `Budget<'a>` (runtime enum policy) | LLVM declined the `#[inline]` hint on `resize_storage`; `try_clone` called it and `Result` and the reservation passed through stack memory | Fixed in `69daa38` (`#[inline(always)]`): small clone 54.2 → 40.8 cycles (unbudgeted 38.9), +44.0% → −0.0% vs std; small pop +20.0% → +6.4% |
| Box (and `storage::allocate`) with a local budget | Ledger work per allocate/free pair: reserve (load, compare, subtract, store), commit (one add), release (one 16-byte vector add); the reservation enum is written to and reloaded from the stack across the allocator call | About +3.4 cycles and +20 instructions per pair; about 1 cycle of it is the stack round trip |
| Concurrent ledger, one thread | One `lock cmpxchg` per reservation, one `lock sub` per release, about 20 cycles each | `request64`: +23.7% vs std (borrowed), +35.2% (owned) |
| Concurrent ledger, four threads on one ledger | Every allocation moves the ledger's cache line between cores | `request64`: +182% (borrowed); `request4096`: +88% |
| Owned concurrent policy | Linux x86-64 already duplicates the policy with `lock inc` (headroom, no CAS) and releases it with `lock dec`; the policy count and the ledger share one allocation, so four threads also contend on it | Four threads: +212% to +412% vs std; about 17× the cross-core forwarded loads and 130× the memory-ordering clears of the borrowed policy |
| Request-local owned domain | `OwnedConcurrentBudget::try_new` allocates and frees its Arc ledger per request | `domain64`, one thread: +61% vs std (borrowed: +22%) |

Replacing the reservation CAS with `fetch_add` plus rollback (experiment only,
not committed) changed one-thread results by less than 2% and four-thread
results by 3–14%: the number of shared atomic operations, not their form, sets
the cost. A chunked-credit lease that avoids most shared operations is designed
in [budget-lease-design.md](budget-lease-design.md) and is on hold by user
decision. Unbudgeted paths are unaffected by these costs.

## Verification

`tests/owned_budget.rs` checks real heap-ledger lifetime across creator drop,
Box/Vec/String/BTreeMap use, raw transfers, last Weak release, both COW paths,
failed initialization with an escaped Weak, bootstrap refusal and zero-byte
budgets. Native tests and Miri on 64-bit, 32-bit and strict Tree Borrows pass.
Compiler cases check owned concurrent state/futures and reject local ownership
at thread boundaries. Loom adds actual owning-policy saturation/COW and creator-
drop interleavings; the two concurrent new scenarios use preemption bound two.
The combined source-to-link certificate closes 864 obligations with zero open
obligations and zero unproved linked edges, as counted before the premise rule:
a row was closed when its contract name was verified, or by a fixed
disposition, and rows whose premises are stated and not proved were counted as
connected; see
the [premise rule](direct-source-migration.md#certificate-dispositions-and-premise-status-2026-11-26). Normal and overflow-checked release
each pass 227 runtime tests and 26 doctests. The
[verification report](verification/owned-budgets.md) records the exact input hash,
executed checks and boundaries. Its paired timing renewal contains 240 intervals
of at least one second across borrowed, owning and std implementations. Owning
policies add count work to new policy-owner lifecycles; the report includes
budgeted construction/COW overheads and the unchanged hot-handle path.
