# Proving the stated premises of the source certificate

Issue [#16](https://github.com/shim9610/jzap-std/issues/16). Status: in
progress; the certificate reports `complete=false` and its CI stage fails until
every row below is proved (user decision, 2026-11-26). The branch is merged
only when the certificate is complete.

## Rule

A certificate row binds one product site (an owner write, a constructor field,
an unsafe block or a modular call) to a verified contract. The contract proves
its conclusion under its `requires`. A row is `solver-and-source-connected`
only when `scripts/premise_status.py` records how that premise is established
at the site:

| Basis | Meaning |
| --- | --- |
| `no-premise` | The contract has no `requires` |
| `state-invariant` | The premise is the invariant of a closed state type: private fields, every constructor establishes it and every transition preserves it, verified on the product methods |
| `verified-caller` | Every product call is made by a function verified on the product source that establishes the premise |
| `direct-source` | The site is inside a product function with its own verified specification |

Every other row is `premise-stated-not-proved` and keeps the certificate
incomplete. Baseline at `c9c61b7`, from the binding lists without solvers:
93 connected, 1,078 stated and not proved (1,064 and the 14 String rows),
0 unproved.

## Trust boundary

These are not proved by this library and are stated wherever a premise rests
on them. They are the boundary the certificate already declares (compiler and
`core`/`alloc` semantics, a valid allocator, safe ownership, public unsafe and
returning callback contracts); a premise that is exactly one of them is not a
library invariant.

- The compiler, `core` and `alloc` behave as documented, including the
  validity of safe references and slices.
- A successful allocation or reallocation returns storage valid for the
  requested layout (`raw_valid` of fresh storage).
- The caller of a public `unsafe fn` meets its documented contract
  (`Vec::from_raw_parts`, `Vec::try_set_len`, `String::from_raw_parts`,
  `String::from_utf8_unchecked` and the like).
- Values adopted from `std` (`alloc::vec::Vec`, `Box<[T]>`, `String`) satisfy
  the `std` invariants.
- User callbacks return (they do not panic or abort) and satisfy the stated
  trait laws.

Whether a row whose premise is only such a boundary counts as connected is
recorded separately in `premise_status.py` (basis `trusted-boundary`) and is
listed in every report; it is pending the user's confirmation and no row uses
it yet.

## Units

Each unit proves the invariants a group of contracts requires, so that its rows
close together. Counts are contracts in `premise_status.py` `STATED`.

| # | Unit | Contracts | Invariant to prove | Status |
| --- | --- | ---: | --- | --- |
| 1 | Vec and Buffer owner | 58 | `len <= cap <= limit`, `byte_valid(cap)`; reservation and range results; Drain/hole and guard states tied to the owner | Design below; feasibility spike passes |
| 2 | Budget ledger | 15 | `occupied + available <= limit`, charge ownership | Not started |
| 3 | B-tree | 53 | Node counts, cursor and parent indices, heights, node shape | Not started |
| 4 | VecDeque | 27 | `ring(head, len, cap)`, reservation results | Not started |
| 5 | Hash table | 19 | Table shape, credit, tag and ownership | Not started |
| 6 | Shared ownership | 19 | Counter words of a live owner, allocation and payload extents | Not started |
| 7 | Binary heap | 13 | Storage and length (on unit 1) | Not started |
| 8 | Scratch | 7 | Length at most the fixed capacity (on unit 1) | Not started |
| 9 | String | 6 | Byte-extent premises (on unit 1); UTF-8 invariant over every mutator | Not started |
| 10 | Bounded Kani scenarios and the B+ ancestor read | 9 | Replace concrete-state scenarios by all-state proofs; verify `descend` against the initialized prefix | Not started |

Unit 1 comes first: it has the most rows (399) and String, BinaryHeap and
Scratch are built on `Vec`.

## Unit 1: the owner extent

### Where the premise enters today

`len <= cap <= limit` is never derived. It enters as the `requires` of the
maintained functions (`owner_valid`), as generated `requires` on projected
callers (`scripts/callsite_projection.py`), and as `kani::assume` in three
harnesses. `Vec` holds `len: Checked<usize>`, `cap: Capacity<T>` and `limit: L`
as separate fields, written directly at 36 sites for `len` and 4 for `cap`
(`src/vec.rs`, `src/vec/convenience.rs`, `src/vec/edit.rs`,
`src/vec/ownership.rs`), and lends `&mut len` to `init::collection::fill`.
There is no single setter.

### Design

A closed state type holds the three words:

```rust
pub(crate) struct Extent<T, L: LengthPolicy> {
    len: Checked<usize>,
    cap: Capacity<T>,
    limit: L,
}
```

- **Invariant** (a Verus type invariant):
  `len <= cap <= limit.bound()` and, for a sized element,
  `cap * size_of::<T>() <= isize::MAX`. A zero-sized element has `cap == limit`.
- **Closed.** The fields are private to the module. `Vec` keeps
  `ptr`, `extent`, `policy` and the marker; the size of `Vec` is unchanged.
- **Every transition is total.** A transition never has a numeric
  precondition for the caller to meet: it checks what it needs and reports a
  refusal (`Option`, `bool` or `Result`), so the invariant holds whatever the
  caller passes. Examples: `admit_one() -> Option<usize>` (the index to write,
  or `None` when full), `set_len_within(needed) -> Result<(), Error>`
  (requires nothing; refuses `needed > cap`), `shorten_to(next)` (never
  lengthens), `install(capacity) -> Result<(), Error>` (refuses a capacity
  below the length or above the limit). The checks duplicate what the
  surrounding code has just established (`reserve_length` grows until
  `needed <= cap`), so on the paths without growth they are expected to fold
  away; this is measured, not assumed (below).
- **Specifications.** Constructors ensure the invariant; transitions state the
  exact new words. `LengthPolicy` gets a trait-level specification
  (`bound()`), so the proof is generic over `L` and `T`.
  `Capacity<T>` carries `byte_valid` as its own invariant; its marker changes
  from `PhantomData<fn() -> T>` to a form Verus accepts.
- **Feasibility.** A standalone model in this style (generic `T`, sealed
  `L` with a trait-level specification, type invariant, total transitions)
  verifies with `--no-cheating` on Verus 0.2026.11.02.

### What the invariant closes and what needs more

| Premise kind | Contracts | After the extent type |
| --- | ---: | --- |
| `len <= cap <= limit`, `owner_valid`, the spelled-out form, `len <= cap` with `byte_valid` (Vec) | 27 | Follow from the invariant |
| The same for `Buffer` | 4 | The same treatment of `Buffer`'s words |
| Reservation and range results (`len < cap`, `len + n <= cap`, `start <= end <= len`), cursor admission | 4 and the per-site conditions | Postconditions of the verified transitions and of `checked_range`, carried to the site by the transition's result |
| Drain and hole state | 5 | A closed hole state holding the tail words, tied to the extent |
| Compaction, Extraction and Span tied to the owner (`original <= cap`, `back <= cap`) | 9 | Constructors of those states take the extent, so the bound is part of their invariant |
| Adoption from `std` and from raw parts | 2 | Trust boundary plus the verified `adopted_capacity` |
| Staging | 2 | Follows from the invariant and the staging driver's specification |

`raw_valid` of the storage remains the allocator boundary.

### Steps

1. `LengthPolicy` trait-level specification; `Capacity<T>` invariant.
2. `Extent<T, L>` with verified constructors and transitions; register the
   Verus target and the closure check (struct, fields and method set pinned,
   with mutation controls), as for `Compaction`.
3. Move `Vec` onto `Extent`: every read through a getter, every write through
   a transition. Update the binding tables that pin source text
   (`caller_bindings.py`, `owner_state_projection.py`,
   `callsite_projection.py`), the Kani owner builders and the unsafe
   inventory.
4. Measure: machine code of every Vec benchmark executable against the
   previous form (`tools/perf-audit/codegen_diff.py`), executed instructions,
   then the paired timing protocol. A regression is analyzed and fixed before
   the step is kept.
5. Rebind the contracts whose premise is now the invariant to
   `state-invariant`, with the evidence, and rerun the owner stages, Miri and
   the link audit.
6. The remaining premise kinds of the table above, then `Buffer`.

Each step is committed and pushed; the row counts are reported after step 5 and
after each later unit.
