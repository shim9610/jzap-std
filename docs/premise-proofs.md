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

Two further bases were put to the owner on 2026-11-26 and approved. They are
reported under their own names and are never merged into the four above:

| Basis | Meaning |
| --- | --- |
| `instantiated-kani` | The premise is an invariant shown by actual-Rust Kani harnesses over every machine word for named instantiations (a zero-sized element for whole methods; sized elements for the pointer-free capacity paths), each assuming only that invariant on entry, together with an inductive closure: every constructor establishes it, every writer preserves it, and the set of writers is pinned by the scanner. It is not a solver proof for every `T`; the instantiations are listed with the row |
| `trust-boundary` | The premise is exactly one item of the fixed list under "Trust boundary" below and nothing inside the library can establish it. A reason outside that list is not accepted |

Not implemented yet: `scripts/premise_status.py` knows only the first four
bases, and no row uses the two new ones.

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
- Loop invariants on product loops that exist only under `cfg(kani)`
  (`cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(..))`) were
  put to the owner and approved on 2026-11-26, under the acceptance check
  below.
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
| 1 | Vec and Buffer owner | 59 | `len <= cap <= limit`, `byte_valid(cap)`; reservation and range results; Drain/hole and guard states tied to the owner | First attempt reverted (it changed the product). Under the constraint: step 1 done (contracts restated, no row closed); step 2 checked (no row closed: both candidates rest on a `requires` of the enclosing function); step 3 done (scanner pins borrows and pattern bindings, 10 rows bound, none closed); steps 4 to 10 below |
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
as separate fields. The scanner lists 34 assignments to a `len` place
(`src/vec.rs` 13, `src/vec/edit.rs` 11, `src/vec/convenience.rs` 9,
`src/vec/ownership.rs` 1), two assignments to `cap` and one through the local
reference in `grow_for_push`, the three swaps of `ptr`, `len` and `cap` in
`try_reuse_larger_buffer`, and two lends of `&mut len` to
`init::collection::fill`. There is no single setter.

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

Design from reading the source at `27a705a`. A bullet has been run only where
it says so; the step records further down give the runs.
The full report is kept locally under `target/verification/i16/designs/`.

- **Verus cannot carry a specification on a `Vec` or `Buffer` method.** Both
  structs have a `NonNull<T>` field, which this Verus does not accept
  (`docs/direct-source-migration.md`). The word-level kernels it can see
  already have specifications; what is unproved is the glue between a kernel
  result and the field store.
- **Means: actual-Rust Kani, per instantiation, plus an inductive closure**
  (basis `instantiated-kani`). Constructors establish
  `len <= cap <= limit` and the byte bound; each of the 34 assignments of
  `len`, the swap in `try_reuse_larger_buffer`, the two lends of `&mut len` to
  `init::collection::fill` and the three writes of `cap` is run by a harness
  that assumes only the invariant and asserts it on exit. Guards (`Drain`,
  `Splice`, `ExtractIf`, `Compact`, `IntoIter`) get a pair invariant with the
  owner, established at their constructor.
- **Scanner.** `state_sites` recorded assignments and struct literals only.
  Since step 3 it also records mutable borrows of field places and struct
  pattern bindings, and a new borrow or binding of an owner word fails.
- **A premise that was false as written.** `hole_valid` required
  `len <= tail`. After `Splice::try_finish` takes the `tail_len == 0` branch
  and pushes, `len > tail_start` is reachable and `restore_tail` still runs
  (with `tail_len == 0`). Confirmed by a run and restated in step 1 (below).
- **Known not to close this way:** `try_write_vectored` exists only for `u8`,
  so it has no zero-sized instantiation; the `Vec` that `VecDeque::into_vec`
  builds from the deque's words depends on unit 4; external callers of
  `Vec::from_raw_parts` and values adopted from `std` are trust boundary.
- **Not determined without running Kani:** loop invariants on the `for`,
  `while let` and `loop` forms at `src/vec/convenience.rs`, `src/vec/edit.rs`
  and `src/boxed/buffer.rs`; stubbing of the storage functions for the sized
  capacity harnesses; solver time and memory.

### Steps

1. Restate the Drain/hole contracts to the invariant the product keeps; drop
   requirements the product checks itself.
2. Rebind the contracts whose premise is discharged inside a directly
   verified function (`admitted_cursor`, the staging and dedup ledger rows).
3. Extend the scanner to every mutation channel; bind the swap and the lends;
   add defect controls.
4. Buffer closure harnesses.
5. Vec closure harnesses without new loop invariants.
6. Second instantiations: the capacity paths at sized elements with the
   storage functions stubbed; a zero-sized array and a failing-clone type.
7. Guard pair invariants and the whole `try_finish`.
8. Loop invariants under `cfg(kani)` on four product loops, with the machine
   code comparison.
9. The closure check that maps every pinned site to a harness; only then do
   contracts move to `instantiated-kani`.
10. Split the contracts shared with VecDeque and BinaryHeap per owner.

Each step is committed and pushed. Row counts are reported from a certificate
run, not from this plan.

### Step 1: the Drain/hole contracts state the words the product keeps

Done on 2026-11-26. No product file changed and no contract changed status.

**The claim, run.** The harness
`vec::edit::panic_proofs::empty_tail_splice_publishes_past_the_tail_start`
(`src/vec/edit_proofs.rs`) runs the real `try_drain(len..len)` and
`Splice::try_finish` on a zero-sized owner for every word `len` and zero, one
or two replacement values, with no assumption on `len`. Kani 0.68 verifies
that the length ends at `tail_start + count` (or the push is refused at
`usize::MAX`) and satisfies the cover `published > tail_start`. In a
disposable copy of the source, a `kani::cover!` and an `assert!` placed in
`Drain::restore_tail` showed, on the same harness: `len > tail_start` is
reached at `restore_tail` (cover satisfied, `assert!(len <= tail_start)`
fails), and `len > tail_start` with `tail_len != 0` is not reached. The
maintained model `hole_push` had assigned `tail = next`, a write the product
does not make.

**Restated.** The words a Drain keeps are

```
tail + tail_len <= cap  &&  (len <= tail || (tail_len == 0 && len <= cap))
```

| Contract | File | Change |
| --- | --- | --- |
| `hole_valid` (spec) | `tools/memory-proof/operation-closure.verus.rs` | The invariant above, with `cap <= limit` and the byte bound as before |
| `hole_push` | same | The model-only write of `tail` is removed; `ensures` now also states `tail` unchanged |
| `hole_fill` | same | Takes the product's branch: `Splice::fill` returns `Error::Bounds` when `tail_start - len` underflows (generated from the pinned text by `scripts/closure_projection.py`). `ensures` weakened to what holds: `len` does not decrease, and stays at most `tail` when it started so |
| `drain_before_tail_restore` | `tools/memory-proof/wrapping-memory.verus.rs` | `requires` is the invariant above |
| `drain_restore_tail_caller` | `tools/memory-proof/raw-owner.verus.rs` | `requires` is the invariant above with `raw_valid` |
| `adopted_owner` | `tools/memory-proof/owner-state.verus.rs` | `byte_valid(source_cap, size)` dropped from `requires`: the function checks it as the product does (`Capacity::try_new`) |

`hole_available`, `hole_reserve`, `hole_move`, `hole_restore`, `stage_commit`,
`stage_operation`, `splice_operation`, `drain_constructor` and
`complete_splice` are unchanged in text and verify under the restated
`hole_valid`. `hole_move`, `stage_commit` and `stage_operation` still require
the phase fact `len == tail`.

The restated contracts are still `premise-stated-not-proved`: the invariant is
a `requires`, and no proof yet shows that the product establishes it at
`restore_tail`, `fill` and `move_tail` (steps 5 and 7).

Not exercised by any run: the fallback branch of
`scripts/callsite_projection.py` for other `Drain` methods carries the same
restated assumption, but every `Drain` block is routed to a maintained
function today, so that branch generates no text.

### Step 2: the two candidates stay stated

Checked on 2026-11-27. No contract changed status and no binding moved.

The plan named two rows whose premise looked discharged inside a directly
verified function.

| Row | Site | Enclosing function, direct target | Numeric `requires` of that function |
| --- | --- | --- | --- |
| `admitted_cursor` (state write) | `*state = Checked::new(wrapping_add(index, 1))`, after `while state.get() < end` | `init::driver::run`, `init-prefix` | `old(state)@ <= end` |
| `staged_collection_extent` (modular call) | `wrapping_add(len, 1)`, after `if len >= limit { return }` | `vec::staging_driver::run`, `vec-staging` | `initial_cap <= limit`, `byte_valid(initial_cap, size)` |

What was run:

- `python3 scripts/verify_direct_verus.py --target init-prefix --target
  vec-staging --mutations`: pass (290 and 287 verified, 0 errors; 8 controls
  detected, among them `init-advance`, `init-early-stop` and `staging-limit`).
  Each function has its own `verus_spec` and is verified on the product
  source.
- A one-off experiment on disposable copies (not a maintained check): with the
  numeric `requires` deleted, and again with the numeric loop invariants
  deleted too (`--multiple-errors 50`), Verus reports no error at either store.
  The failures are the loop invariant on entry, the final assertion or the
  postconditions, and the callback's precondition. The precondition of the
  `wrapping_add` at the store follows from the guard alone.

Why they stay stated. A verified function proves its body under its own
`requires`; it does not discharge that `requires`. `direct-source` is recorded
only where the callers' side is established too, and here it is not:
`init::driver::run` is called by `init::prefix::initialize` (a cursor started
at 0) and by `init::collection::fill` for `Buffer::collect`,
`Buffer::collect_rest`, `Splice::fill` and `stage_batch`, which pass an owner
length and an end; `vec::staging_driver::run` is called by
`Splice::stage_excess`. None of these callers is verified on the product
source. `scripts/premise_status.py` now records this reason for each of the two
contracts in place of the general owner reason, and
`scripts/test_verify.py` keeps them stated.

Follow-up, the two modular-call rows of those stores. The row in
`init::driver::run` had been bound to `initialized_box_prefix`, a maintained
function without a `requires` that calls `run` from 0, and so counted as
established although `run` has other callers; its twin in
`vec::staging_driver::run` counted as stated. Both are now bound to
`guarded_increment` (`tools/direct-verus/init-adapters.rs`): it has no
`requires`, takes the guard as its own branch and calls the product
`wrapping_add` below the bound, so the increment cannot wrap whatever the
caller passed. `verify_wrapping_memory.guarded_stores` pins each guard next to
its increment in the product text (`while state.get() < end`,
`if len >= limit { return }`). Basis `no-premise`; the ledger now has 26
established and 27 stated rows of 53 (25 and 28 before). The numeric
`requires` of the two functions is a different premise and stays stated under
`admitted_cursor` and `staged_collection_extent`.

Run: `scripts/verify_wrapping_memory.py --mutations` passes (388 verified,
0 errors; 9 controls detected, of which three are new: the increment without
its guard is refused by Verus with 1 error, and each moved guard fails the
source pin); `scripts/test_verify.py` 50 OK. The owner protocol was not rerun
after this change.

`c_previous_kept` was not touched: it rests on a zero-sized Kani instantiation
and belongs to `instantiated-kani`.

### Step 3: the scanner pins borrows and pattern bindings of the owner words

Done on 2026-11-27. No product file changed and no contract changed status.

**Scanner** (`tools/panic-audit/src/bin/state_sites.rs`). Two row kinds are
new, without a field-name list in the scanner:

| Kind | Recorded |
| --- | --- |
| `field_borrow` | Every `&mut` and `&raw mut` of a field place, with the chain of members (`self.drain.owner.len` gives `drain`, `owner`, `len`). The arguments of `mem::swap`, `mem::replace` and `mem::take` are such expressions |
| `field_pattern` | Every field of a struct pattern that is not a wildcard, with a flag for a `ref mut` binding. A plain binding is recorded too: against a `&mut` value, default binding modes make it a mutable reference |

On the product at this commit: 59 `field_borrow` rows and no `field_pattern`
row.

**Binding** (`scripts/caller_bindings.py`). The owner words are the storage
words of `Vec` and `Buffer`: `ptr`, `len`, `cap`, `limit`, `layout`. A row whose
place names one of them, at any position, must match the `LENDS` table by
file, function, expression and order. The check runs on the whole inventory
before any area is filtered out, so such a row in a file of another area is
refused instead of being dropped. A pattern binding of an owner word is always
refused: the product has none.

| Site | Rows | Contract (stated) |
| --- | ---: | --- |
| `Buffer::collect`, `Buffer::collect_rest`: `&mut self.len` lent to `init::collection::fill` | 2 | `buffer_span` |
| `Splice::fill`: `&mut self.drain.owner.len`; `stage_batch`: `&mut extra.len` | 2 | `splice_fill_loop` |
| `Vec::try_reuse_larger_buffer`: the operands of the three swaps | 6 | `exchanged_owners` (new) |

`exchanged_owners` (`tools/memory-proof/owner-state.verus.rs`) is a new
maintained function, verified: two valid owners exchange length and capacity,
each keeps its limit, and the product's refusals and reservation are its
branches. The body of `try_reuse_larger_buffer` is compared whole. It requires
both owners valid, so it is `premise-stated-not-proved`, and unit 1 now has 59
stated contracts (58 before). No existing contract stated the premise of the
exchange; `replaced_owner` is a whole-value replacement and does not apply,
because each owner keeps its own limit and policy.

**Pinned counts.** State writes 91, constructor fields 132 and unsafe callers
181 are unchanged. Bindings rise from 470 to 480: the ten rows above have the
new role `state-lend`, and `scripts/source_certificate.py` pins
`state_lends == 10`. The owner composition reports 565 verified and 0 errors
(564 before): `exchanged_owners` is the one added.

**Controls**, all detected. Source mutants that compile and must be refused by
the binding: an assignment to the owner length in a function without a binding
(`unbound_owner_write`), `mem::swap` and `mem::replace` on an owner word
(`unbound_owner_lend`, `unbound_owner_take`), a `ref mut` binding and a plain
binding of the length in a pattern (`owner_ref_mut_pattern`,
`owner_default_binding_pattern`), and the exchange without its limit check
(`exchange_without_limit_check`). A weakened maintained function that Verus
must reject: `exchange_without_equal_limits`. On rows
(`scripts/test_verify.py`): a borrow in a file of another area, a borrow of a
part of an owner word, a missing or reordered lend, and the scanner itself on
a sample file.

**Not finished.**

- 49 of the 59 `field_borrow` rows borrow a field that is not an owner word
  and are bound to no contract. The binding check counts them per area and
  the owner-protocol summary lists the counts (`unbound_field_borrows`):
  hash-map 11, binary-heap 9, foundation 8, btree 7, hash-set 6, deque 3,
  shared 2, string 2, bplus 1. The eight in the foundation area are the
  cursor states lent to the verified drivers (`&mut g.state` twice,
  `&mut self.state`, `&mut prefix.state`), the user's iterator and filter
  (`&mut self.replacement` twice, `&mut self.filter`) and the byte buffer of
  `CharBytes`. A new borrow of such a field does not fail any check.
- The other areas (String, B-tree, hash, BinaryHeap, VecDeque, Scratch) do not
  read `state_sites` rows at all: their scripts bind unsafe blocks from the
  `unsafe_sites` scanner, and `caller_bindings.py` has always filtered their
  assignments and constructor fields out. Only the owner-word guard above
  covers their files. Their own writes are not bound by this step.
- Channels the scanner does not record: a method call that borrows its field
  receiver by auto-reference (the methods called on `len`, `cap` and `limit`
  places today are `get`, `layout`, `max_len`, `saturating_sub` and `min`,
  found by a text search and not pinned), the operand of `addr_of_mut!`
  (an unparsed macro body; every use today is inside an unsafe block), and a
  borrow of an element or of a dereference of a field (`&mut self.x[i]`,
  `&mut *self.x`).
