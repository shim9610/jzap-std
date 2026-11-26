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

## Constraint: a proof does not change the product

Owner instruction, 2026-11-26: the premises are to be proved; the behavior of
the product is not to be changed for the sake of a proof.

- No runtime check, clamp, refusal branch, reordering or new field is added to
  product code to make a premise hold by construction.
- A proof may add what compiles to nothing in a product build: specifications
  under `cfg_attr(verus_keep_ghost, ...)`, ghost and proof blocks, harnesses
  under `cfg(kani)`, maintained proof functions, binding tables and scripts.
- Acceptance for a step that touches a product file: the machine code of every
  benchmark executable is identical to the commit before: every function
  body compares equal (`tools/perf-audit/codegen_diff.py` on builds with
  `-C symbol-mangling-version=v0`, so that each monomorphization has its own
  name).
- A premise that cannot be proved under this constraint stays
  `premise-stated-not-proved` and is reported; it is not closed by changing
  the product.

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
| 1 | Vec and Buffer owner | 58 | `len <= cap <= limit`, `byte_valid(cap)`; reservation and range results; Drain/hole and guard states tied to the owner | First attempt reverted (it changed the product); approach under the constraint not designed |
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

## Unit 1: Vec and Buffer owner

### Where the premise enters today

`len <= cap <= limit` is never derived. It enters as the `requires` of the
maintained functions (`owner_valid`), as generated `requires` on projected
callers (`scripts/callsite_projection.py`), and as `kani::assume` in three
harnesses. `Vec` holds `len: Checked<usize>`, `cap: Capacity<T>` and `limit: L`
as separate fields, written directly at 36 sites for `len` and 4 for `cap`
(`src/vec.rs`, `src/vec/convenience.rs`, `src/vec/edit.rs`,
`src/vec/ownership.rs`), and lends `&mut len` to `init::collection::fill`.
There is no single setter.

### Withdrawn: a closed state type with total transitions

Commits `36d250d` to `7f2e75c` moved the three words into a new type
(`Extent`) whose transitions checked their arguments and refused or clamped,
so that the invariant held for every caller. That changed the product: about
850 added and 280 removed source lines; 153, 96, 37 and 10 function bodies
that differ from `5dadd51` in the Vec, String, BinaryHeap and Scratch
benchmark executables; two more comparisons per `grow_to` call; up to 4.33%
more executed instructions (Scratch `push_pop_256`); and an unreachable error
branch in `try_push`. The change broke the constraint above and was reverted
in full on 2026-11-26 (`b7c4ab0`); the product files and the scripts that pin
their text are those of `5dadd51` again. After the revert every function body
of the four executables equals the `5dadd51` build (851, 728, 668 and 594
bodies, compared as normalized instruction lists; these builds use legacy
symbol names, which merge monomorphizations, so a comparison by name is not
usable on them). The 26 contracts it had moved to `state-invariant` are
`premise-stated-not-proved` again.

Corrections from that work that do not depend on the product change are
kept: `replaced_owner` without a `requires`, `staged_collection_loop` and the
new `safe_iterator_state` as `no-premise`, `raw_parts_owner` as the contract of
`Vec::from_raw_parts` (it had been bound to `resized_owner`), and
`c_keep_live`/`c_remove_live` returned to stated because
`VecDeque::try_retain_mut` is a second, unverified caller.

### Approach under the constraint

Not designed yet. The invariant has to be shown on the product as it is: every
constructor establishes it and every existing write of `len`, `cap` and
`limit` preserves it, with the set of writers pinned by the scanner so that a
new write fails the check. The means available are specifications on the
existing functions that compile to nothing, and actual-Rust Kani harnesses
over every machine word of the three fields that assume only the invariant
on entry. Which of the 40 write sites each means can cover has not been
determined.
