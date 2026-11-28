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

Both are implemented in `scripts/premise_status.py` (step 9 below):
`instantiated-kani` is accepted for a contract only while
`scripts/owner_closure.py` reports its owner closed, its maintained `requires`
is exactly the owner invariant (plus `call_requires`) and every bound row is in
a function of the owner generic in its element type; the certificate fails
otherwise. The certificate reports the two bases under their own names and
counts; their rows do not block completion.

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

A contract whose every row rests only on one of these items has the basis
`trust-boundary` (approved on 2026-11-26), recorded in `premise_status.py`
(`TRUST_BOUNDARY_ITEMS` is this list; `TRUST_BOUNDARY` maps a contract to one
item). No contract has it yet (step 9).

## Supplementary evidence

VeriFast was adopted as an auxiliary tool (owner decision); the Kani-based
closure stays the primary method. [`docs/verification/verifast.md`](verification/verifast.md)
records a VeriFast proof, for every element type `T`, on a subset crate
assembled from the current `src/` (`tools/verifast/check.py`): every
`Capacity<T>` returned by `empty`, `try_new` or `from_validated_raw` (under
its `req`) satisfies `is_valid_layout(count * size_of::<T>(), align_of::<T>())`,
and `Capacity::layout` under that invariant has `count * size_of::<T>() <=
isize::MAX`, so its `wrapping_mul` does not wrap and the
`from_size_align_unchecked` precondition holds. This speaks to the premise of
`capacity_bytes` (the byte bound of `Capacity` for every `T`) and to the byte
bound of `layout::array`. The other contracts bound at `src/capacity.rs`
(`empty_owner`, `external_count_before_allocation`) are already `no-premise`.

It does not change any disposition in `scripts/premise_status.py` or the
certificate, and is not a basis:

- the `capacity_bytes` rows rest on the callers of
  `Capacity::from_validated_raw` (`src/deque.rs`, `src/vec/ownership.rs`),
  which are outside the subset; VeriFast proves the function under its `req`
  and does not show that those callers establish it;
- `Capacity::for_slice` is specless and not checked by VeriFast;
- the refinement checker that connects the rewritten `layout::array` to the
  product ignores unwind paths and storage liveness;
- the result is on a subset crate, connected to `src/` by sha256 pins and a
  comment-strip comparison, with a trusted overlay of std specifications and
  two `assume(false)` variance lemmas.

## Units

Each unit proves the invariants a group of contracts requires, so that its rows
close together. Counts are contracts in `premise_status.py` `STATED`.

| # | Unit | Contracts | Invariant to prove | Status |
| --- | --- | ---: | --- | --- |
| 1 | Vec and Buffer owner | 59 | `len <= cap <= limit`, `byte_valid(cap)`; reservation and range results; Drain/hole and guard states tied to the owner | First attempt reverted (it changed the product). Under the constraint: step 1 done (contracts restated, no row closed); step 2 checked (no row closed: both candidates rest on a `requires` of the enclosing function); step 3 done (scanner pins borrows and pattern bindings, 10 rows bound, none closed); step 4 done (Buffer closure harnesses verified and registered; no contract changed status); step 5 done except L11 (Vec closure harnesses for the writers without a new loop invariant; the existing loop contract of `try_resize_with_init` does not verify here); step 6 done (sized capacity paths with stubbed storage, `[(); 1]`, a failing clone with bounded counts, `Capacity::for_slice`); step 7 done (task A: the loop contract of `try_resize_with_init` restated, L11 has its closure harness; task B: the splice leaves, the whole `try_finish` with a retained tail, IntoIter and the two whole-owner replacements); step 8 done for three loops (`try_resize_owned`, `try_extend_from_within`, `try_extend_fallible`: L16, L18, L19 and the iterator-driven extenders have closure harnesses) and not for two (the empty-tail loop of `try_finish`, the loop of `Buffer::collect_rest`); step 9 done (the Buffer lends have closure harnesses for a source without a bound; `scripts/owner_closure.py` reports Buffer closed and Vec not closed, blocked by the two `Vec::from_raw_parts` calls in `src/deque.rs`; four Buffer contracts are `instantiated-kani`); step 10 below |
| 2 | Budget ledger | 15 | `occupied + available <= limit`, charge ownership | Not started |
| 3 | B-tree | 53 | Node counts, cursor and parent indices, heights, node shape | Not started |
| 4 | VecDeque | 27 | `ring(head, len, cap)`, reservation results | Steps B to E done (below): every VecDeque constructor and writer has a closure harness at `T = ()` (the retain loop under a loop contract that uses the owner-approved `cfg(kani)` getter `probe_ptr`); `owner_closure.py` closes Vec and VecDeque jointly; 8 Vec contracts moved to `instantiated-kani` (21 rows); no deque contract moves (none requires exactly the ring invariant) |
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

### Step 4: Buffer closure harnesses

Done on 2026-11-27. No product file changed and no contract changed status:
`scripts/premise_status.py` is untouched and the basis `instantiated-kani` is
still not implemented. This step adds verified harnesses and registers them.

Before the step, `python3 scripts/verify_owner_protocol.py --mutations` was run
on `bc757eb` (the run that step 2 left open): pass, 1,246 s, 65 results,
`source_closure_complete`, no reused prerequisite.

**Fixture** (`src/boxed/buffer_proofs.rs`). `closure_invariant` is the Buffer
invariant in the owner's own words:
`len <= cap && layout::array::<T>(cap) == Ok(layout)`. The second part is the
byte bound `cap * size <= isize::MAX` and the stored layout; for a zero-sized
element every capacity has the same zero-byte layout, so it constrains nothing
but the layout word. `closure_owner::<T>()` builds an owner from arbitrary
length and capacity words, assumes `closure_invariant` and nothing else, and is
the only place where a closure harness assumes anything about the owner. Each
harness asserts `closure_invariant` on every return.

| Harness (`boxed::buffer::proofs::`) | Method | Element | Inputs besides the owner |
| --- | --- | --- | --- |
| `closure_new_establishes_the_invariant` | `new` | `()`, `u8`, `u64`, `[u64; 200]`, a 64-aligned 64-byte type | none |
| `closure_reserve_preserves_the_invariant_for_every_unit_owner` | `reserve` | `()` | every `additional` and `hint` |
| `closure_push_preserves_the_invariant_for_every_unit_owner` | `push` (and `reserve`) | `()` | every `hint` |
| `closure_append_preserves_the_invariant_for_every_unit_owner` | `append` (and `reserve`) | `()` | a slice of every length |
| `closure_collect_preserves_the_invariant_for_every_unit_owner_and_two_value_source` | `collect`, `collect_rest` | `()` | a source that yields at most two values; every call may fail, every size hint is an arbitrary word or an error |
| `closure_finish_transfers_exactly_the_length_of_every_unit_owner` | `finish` | `()` | none |
| `sized::closure_reserve_preserves_the_invariant_for_every_sized_owner` | `reserve` | `u8`, `u64`, `[u64; 200]`, the aligned type | every `additional` and `hint`; `storage::reallocate` stubbed |
| `sized::closure_finish_transfers_exactly_the_length_of_every_sized_owner` | `finish` | the same four | `storage::reallocate` and `storage::deallocate` stubbed |

`finish` consumes the owner, so its harnesses assert the words of the returned
Box (length equal to the owner's length, same address) instead of the invariant.

**Stubbing works here.** `-Z stubbing` replaces `storage::reallocate` (an
`#[inline(always)] unsafe fn`) and `storage::deallocate` in this crate: Kani
0.68 lists both stubs for the harness and verifies it. The stub of `reallocate`
returns the same pointer or `Error::Allocation`, chosen freshly, and touches no
byte, so the capacity word of a sized owner ranges over every count with a
valid array layout. The cover statements of the sized reserve harness are
satisfied for each of the four types: a grown capacity, an allocator refusal
and a layout refusal. A harness with a stub attribute does not compile in a
Kani run without `-Z stubbing` (`error: Using the stub attribute requires
activating the unstable stubbing feature`), and `scripts/verify.py` selects
`boxed::buffer::proofs::` by prefix without that option, so the two sized
harnesses are in a module under `cfg(jzap_closure_stubs)`. That name is not in
the manifest's `check-cfg` list (the manifest was not edited): every Kani build
now prints one `unexpected cfg` warning for it.

**`collect_rest` without a loop invariant.** The source of the collect harness
yields at most two values, so the cursor loop of `init::collection::fill` and
the `loop` of `collect_rest` are unwound (`kani::unwind(5)`) and no product loop
needs an invariant. The length and capacity words are arbitrary; the bound is
on the number of values only. The covers show a run that takes two values and
returns `Ok`, a storage error and a source error. An unbounded source was not
run: with `--cfg jzap_init_loop_contracts` the fill loop would be abstracted,
but the `loop` of `collect_rest` has no invariant and a source that never ends
does not unwind.

**Registration** (`scripts/verify_source.py`). `CLOSURE` appends the six
unit-element harnesses to the `buffer` group (10 harnesses in its Kani
invocation, 4 before). `EXTRA` is new: a second Kani invocation of the same
group for harnesses that need options the group does not use, here
`-Z stubbing` and `--cfg jzap_closure_stubs` for the two sized harnesses. A
control takes the options of the invocation that owns its harness. Nine
controls are new; each mutates `src/boxed/buffer.rs` in a disposable copy so
that one writer breaks the invariant, and must end in a solver counterexample:

| Control | Mutation | Harness that must fail |
| --- | --- | --- |
| `closure-buffer-new-length` | `new` starts with length 1 | `closure_new_...` |
| `closure-buffer-reserve-capacity` | `reserve` publishes `additional` as the capacity | `closure_reserve_..._unit_owner` |
| `closure-buffer-push-length` | `push` publishes `needed + 1` | `closure_push_...` |
| `closure-buffer-append-length` | `append` publishes `needed + 1` | `closure_append_...` |
| `closure-buffer-collect-initial-bound` | the first span ends one past the reserved end | `closure_collect_...` |
| `closure-buffer-collect-growth-bound` | the later spans end one past the capacity | `closure_collect_...` |
| `closure-buffer-finish-length` | `finish` hands over `cap` elements | `closure_finish_..._unit_owner` |
| `closure-buffer-sized-reserve-layout` | `reserve` stores the layout of `needed`, not of the new capacity | `sized::closure_reserve_...` |
| `closure-buffer-sized-finish-length` | as `closure-buffer-finish-length` | `sized::closure_finish_...` |

**Run.** `python3 scripts/verify_source.py --group buffer --mutations`: pass in
397 s. Direct Verus target `buffer` with its mutations; Kani 0.68.0 (CBMC
6.11.0, CaDiCaL, `--object-bits 10`): 10 of 10 and 2 of 2 harnesses verified;
16 of 16 controls detected (7 existing, 9 new). Solver times: the collect
harness 42 s, the sized reserve harness 17 s, the sized finish harness 5 s,
every other new harness under 2 s. Peak memory was not measured.
`python3 scripts/test_verify.py`: 50 OK. No pinned count changed.

**Not covered.**

- `Buffer::drop` writes no word and was not given a harness; its
  `needs_drop` branch cannot run for an arbitrary length.
- `push`, `append` and `collect` at a sized element: they write through the
  pointer, so a harness would need storage of arbitrary size.
- A source that yields more than two values (above).
- The harnesses use `Unmetered`. A metered policy changes only what
  `storage::reallocate` does with the budget, which the unit-element paths
  (equal sizes return early) and the stub do not execute.
- The closure check that maps every pinned Buffer site to these harnesses and
  the status change of `buffer_reserve`, `buffer_push`, `buffer_finish`,
  `buffer_collect`, `buffer_span` and `reservation_before_bulk_write` are
  step 9.

### Step 5: Vec closure harnesses without new loop invariants

Done on 2026-11-27. No product file changed and no contract changed status.

**Site list, checked against the source at `bc757eb`** (a text search for
assignments, swaps and `&mut` of the owner words in `src/vec.rs` and
`src/vec/*.rs`): 34 assignments to a `len` place (`vec.rs` 13,
`convenience.rs` 9, `edit.rs` 11, `ownership.rs` 1), the swap at `vec.rs:490`,
the two lends at `edit.rs:450` and `edit.rs:678`, and three capacity writes
(`vec.rs:327`; `vec.rs:811` through the reference, stored at `vec.rs:359`; the
swap at `vec.rs:491`). The design report's list is complete for these
channels; no further writer was found. `limit` is never assigned. Two
whole-owner replacements exist and have no harness of their own
(`mem::swap(self, &mut replacement)` at `vec.rs:700` in `try_clone_from` and at
`ownership.rs:252` in `IntoIter::try_clone_from`): each side is a complete
owner built by other methods.

**Fixture** (`src/vec/query_proofs.rs`). `closure_invariant` is
`len <= cap <= limit` read from the owner's words. `closure_owner::<T, P, L>`
builds an owner of zero-sized elements from arbitrary length and capacity
words under a given policy and limit, assumes `closure_invariant` and nothing
else, and is the only place where a closure harness assumes anything about an
owner. `closure_exit` asserts the invariant and the unchanged limit. Every
harness runs under `LengthLimit` with an arbitrary limit and under
`Unlimited`, unless the method exists for one policy only.

A guard built directly, not by its constructor, assumes only its pair
invariant with a `closure_owner` (`src/vec/edit_proofs.rs`):
`closure_drain` assumes `closure_drain_invariant`,
`tail_start + tail_len <= cap && (len <= tail_start || tail_len == 0)`, over
any ordered `Span`; the `ExtractIf` drop harness assumes
`closure_extract_invariant`, a valid `Extraction` whose total is at most the
capacity. The constructor harnesses assert these pair invariants instead of
assuming them.

| Site | Function | Harness | Result |
| --- | --- | --- | --- |
| `vec.rs:98` | `new` | `metadata_proofs::closure_empty_constructors_establish_the_invariant` (`()`, `[(); 3]`, `u8`, `u64`, `[u64; 200]`, a 64-aligned type) | verified |
| `vec.rs:189` | `empty`, `new_in`, `with_limit`, `with_limit_in` | the same harness | verified |
| | `try_with_capacity(_in)`, `try_with_capacity_and_limit(_in)` | `metadata_proofs::closure_capacity_constructors_establish_the_invariant` (`()`) | verified |
| | `try_from_slice_in` | `metadata_proofs::closure_slice_constructor_establishes_the_invariant` (`()`, every slice length) | verified |
| `ownership.rs:87`, `:122` | `try_from_std_in`, `from_boxed_slice` | `metadata_proofs::closure_adopting_constructors_establish_the_invariant_for_unit_storage` (`()` only; the `std` facts of a sized value are trust boundary) | verified |
| L1 `vec.rs:381` | `try_insert` | `metadata_proofs::closure_insert_preserves_the_invariant` | verified |
| L2 `vec.rs:386` | `pop` | `metadata_proofs::closure_pop_preserves_the_invariant` | verified |
| L3 `vec.rs:397` | `remove` | `metadata_proofs::closure_remove_preserves_the_invariant` | verified |
| L4 `vec.rs:403` | `swap_remove` | `metadata_proofs::closure_swap_remove_preserves_the_invariant` | verified |
| L5 `vec.rs:417` | `truncate` | `metadata_proofs::closure_truncate_preserves_the_invariant` | verified |
| L6 `vec.rs:446` | `try_extend_copy` | `metadata_proofs::closure_extend_copy_preserves_the_invariant` | verified |
| L7, L8 `vec.rs:462-463` | `try_append` | `metadata_proofs::closure_append_preserves_the_invariant_of_both_owners` (all four pairs of length policies) | verified |
| S1, C3 `vec.rs:489-491` | `try_reuse_larger_buffer` | `metadata_proofs::closure_buffer_exchange_preserves_the_invariant_of_both_owners` (two arbitrary limits) and `..._of_both_unlimited_owners`; both owners charge one `Budget::Limited` | verified |
| L9, L10 `vec.rs:507-508` | `try_split_off` | `metadata_proofs::closure_split_off_preserves_the_invariant_of_both_owners` | verified |
| L11 `vec.rs:541` | `try_resize_with_init` | none | **not verified**, see below |
| L13 `vec.rs:686` | `try_clone` | `metadata_proofs::closure_clone_establishes_the_invariant_of_the_copy` (`()`: the bulk hook, so the copy is published by L6) | verified |
| L14 `convenience.rs:32` | `push_within_capacity` | `metadata_proofs::closure_push_within_capacity_preserves_the_invariant` | verified |
| L15 `convenience.rs:89` | `try_set_len` | `metadata_proofs::closure_set_len_preserves_the_invariant` | verified |
| L17 `convenience.rs:153` | `try_extend_copy_from_within` | `metadata_proofs::closure_extend_copy_from_within_preserves_the_invariant` (both endpoints of any `Bound` form) | verified |
| L20 `convenience.rs:246` | `try_into_array` | `metadata_proofs::closure_into_array_preserves_the_invariant_of_a_returned_owner` (`N` = 0, 1, 3, `usize::MAX`) | verified; the store of 0 is followed only by the owner's drop, so the harness cannot observe it |
| L21 `convenience.rs:267` | `try_from_array_in` | `metadata_proofs::closure_array_constructor_establishes_the_invariant` (`N` = 0, 1, 3, `usize::MAX`) | verified |
| `convenience.rs:371` | `try_into_flattened` (through `from_raw_parts`) | `metadata_proofs::closure_flatten_establishes_the_invariant_of_the_element_owner` (`[(); N]`, `N` = 0, 1, 3, `usize::MAX`) | verified |
| L23 `edit.rs:53` | `Compact::drop` | the callback-error return of the two harnesses below | verified through them; no stand-alone harness, see below |
| L24 `edit.rs:93` | `try_retain_mut` | `edit::panic_proofs::closure_retain_preserves_the_invariant` | verified |
| L25 `edit.rs:149` | `try_dedup_by` | `edit::panic_proofs::closure_dedup_preserves_the_invariant` | verified |
| L26 `edit.rs:184` | `try_drain` | `edit::panic_proofs::closure_drain_constructor_establishes_the_pair_invariant` | verified |
| L27 `edit.rs:203` | `try_extract_if` | `edit::panic_proofs::closure_extract_constructor_establishes_the_pair_invariant` | verified |
| L28 `edit.rs:280` | `Drain::restore_tail` | `edit::panic_proofs::closure_drain_drop_restores_the_invariant_from_every_pair_state` (after one `next` and one `next_back`) | verified |
| L29 `edit.rs:406` | `ExtractIf::drop` | `edit::panic_proofs::closure_extract_drop_restores_the_invariant_from_every_pair_state` | verified |
| F1 `edit.rs:450` | `Splice::fill` | `edit::panic_proofs::closure_splice_fill_preserves_the_owner_and_pair_invariants` (a source without a bound; the cursor loop under its existing contract) | verified |
| L33 `edit.rs:695` | `stage_push` | `edit::panic_proofs::closure_stage_push_preserves_the_invariant` | verified |
| F2 `edit.rs:678` | `stage_batch` | `edit::panic_proofs::closure_stage_batch_preserves_the_invariant` (a source without a bound) | verified |
| L34 `ownership.rs:155` | `into_iter` | `ownership::panic_proofs::closure_into_iter_preserves_the_invariant` | verified |
| C1 `vec.rs:327` | `resize_storage` | `growth_proofs::closure_reserve_...`, `closure_reserve_exact_...`, `closure_grow_...`, `closure_reserve_length_preserves_the_invariant` | verified |
| C2 `vec.rs:811`, `:359` | `grow_for_push`, `try_push` | `growth_proofs::closure_push_preserves_the_invariant` | verified |
| C1 | `try_shrink_to`, `try_shrink_to_fit` | `growth_proofs::closure_shrink_preserves_the_invariant_of_a_unit_owner` | verified, but a zero-sized element returns before the write; the sized run is step 6 |

All of these are at zero-sized elements with `Unmetered` (the exchange with
one `Budget::Limited`). The harnesses assume nothing but the fixture; the two
drop harnesses of a directly built guard assume its pair invariant.

**L11 is not verified: the loop contract of `try_resize_with_init` does not
verify in this environment.** A closure harness for it (the fixture, a callback
that fails, skips the write or writes, both limit policies) fails under
`-Z loop-contracts`, and so does the existing harness
`panic_proofs::vec_resize_covers_every_length_and_callback_result`, run as
`scripts/verify_panic_paths.py` runs it (`RUSTFLAGS="--cfg
jzap_loop_contracts" cargo kani --lib -Z loop-contracts --exact --harness ...`,
Z3 on `PATH`): `VERIFICATION:- FAILED` with the failed checks
`__CPROVER_contracts_car_set_insert` "ptr NULL or writable up to size" and two
"dereference failure: pointer outside object bounds". The result is the same on
unmodified `bc757eb`, on the initial snapshot `59f729b`, with CaDiCaL, and with
`--object-bits 10`; Kani 0.68.0, CBMC 6.11.0. The failing checks belong to the
write-set instrumentation of the loop contract, not to an assertion of the
harness, and the cause was not isolated. The closure harness additionally fails
its exit assertions; with the instrumentation failing, that is not evidence
about the product. The harness was not committed. `verify_panic_paths.py` was
not run in this step; its entry for this harness is expected to report
"unproved" here. No record of this failure was found in `docs/`.

**L23 has no stand-alone harness.** `Compaction` has no constructor for an
arbitrary valid state outside its module (`Extraction` has
`from_cursor_state` under `cfg(kani)`; `Compaction` does not, and adding one
edits a product file). `Compact::drop` runs on the callback-error return of the
retain and dedup harnesses, from the states the driver's loop contracts admit;
the control `closure-vec-compact-drop-length` shows that this return reaches
the store.

**Registration and controls** (`scripts/verify_source.py`). 37 harnesses in
five groups; 41 controls, at least one per harness. Each control changes one
product place in a disposable copy and must end in a solver counterexample of
the named harness. Most make the writer publish a word that breaks the
invariant; the others drop a call or change a guard so that another asserted
result fails (`closure-vec-slice-extension`,
`closure-vec-into-array-length-check`, `closure-vec-reserve-length-growth`,
`closure-vec-shrink-unit-return`).

| Group | Harnesses before, after | Controls before, after | Run time | Result |
| --- | --- | --- | ---: | --- |
| `owner-metadata` | 4, 26 | 4, 29 | 764 s | pass |
| `vec-layout-growth` | 4, 10 | 5, 11 | 224 s | pass |
| `vec-compaction` (Z3) | 2, 4 | 0, 3 | 274 s | pass |
| `vec-splice-fill` (`jzap_init_loop_contracts`) | 4, 9 | 4, 9 | 368 s | pass |
| `vec-extraction` | 1, 3 | 0, 2 | 82 s | pass |

Each line is `python3 scripts/verify_source.py --group <name> --mutations`
(the group's direct Verus targets with their mutations, then Kani 0.68.0 with
CBMC 6.11.0, `--object-bits 10`). Every harness verified and every control was
detected. Solver times of the new harnesses: retain and dedup 17 s and 18 s,
flatten 10 s, splice fill 8 s, every other one at most 5.3 s. Peak memory was
not measured. `python3 scripts/test_verify.py`: 50 OK.

The drain, staging and extraction harnesses are in
`src/vec/edit_proofs.rs`, which compiles only with `--cfg
jzap_loop_contracts`. `stage_push` and `stage_batch` are registered in
`vec-splice-fill`, not in `vec-staging`: that group runs without loop
contracts, and `stage_batch` runs the shared cursor loop under its contract.
The groups `vec-staging` (3 harnesses, pinned by
`verify_owner_protocol.py`) and `vec-cursors` (4, pinned by
`verify_wrapping_memory.py`) are unchanged, and no pinned count changed.

**Not covered by this step.**

| Site | Reason |
| --- | --- |
| L11 `vec.rs:541` `try_resize_with_init`, and with it `try_resize`, `try_resize_with`, the per-element branch of `try_extend_cloned` and `try_clone_in` | the existing loop contract does not verify here (above) |
| L12 `vec.rs:589` `try_extend_cloned`, witness branch | `()` takes the bulk hook; step 6 |
| L16 `convenience.rs:129` `try_extend_from_within` | needs a new invariant on a `for` loop; step 8 |
| L18, L19 `convenience.rs:213`, `:233` `try_resize_owned` | needs a new invariant on a `while` loop; step 8 |
| L22 `convenience.rs:424` `try_write_vectored` | exists for `u8` only; no zero-sized instantiation |
| L30 `edit.rs:518`, L31, L32 `edit.rs:592-593` `Splice::try_finish`, `stage_excess` | step 7; the `tail_len == 0` branch needs a loop invariant (step 8) |
| `try_extend_fallible`, `try_extend`, `try_from_iter_in`, `try_from_fallible_iter_in` | no direct write; their `while let` loop over `try_push` has no invariant |
| `try_clone_from` (`vec.rs:700`), `IntoIter::try_clone_from` (`ownership.rs:252`) | whole-owner replacement, no harness |
| `try_write_fmt` and the `SuffixGuard` sites other than through `truncate` | `u8` only; not run |
| `deque.rs:522`, `deque.rs:579`, `string/convert.rs:218` (`Vec::from_raw_parts`) | other units and the public unsafe boundary |
| every sized element, every metered policy other than the one exchange fixture, every `needs_drop` path | step 6 for the capacity paths; the rest is not runnable for all words |

The closure check that maps each pinned site to its harness, and any status
change, are step 9.

### Step 6: second instantiations

Done on 2026-11-27. No product file changed and no contract changed status.

**(a) The capacity paths at sized elements, with the storage functions
stubbed** (`src/vec/growth_proofs.rs`, module `sized`). `-Z stubbing` replaces
`storage::allocate`, `storage::reallocate` and `storage::deallocate` in this
crate; Kani lists the three stubs for each harness. `allocate` returns an
aligned address or `Error::Allocation`, `reallocate` returns the same pointer
or `Error::Allocation`, each chosen freshly on every call, and `deallocate`
does nothing, so no storage exists and the words range over every admitted
value. The fixture `closure_sized_owner::<T, L>` takes any length and any
capacity that `Capacity::try_new` admits and assumes `closure_invariant` and
nothing else; the admission is the byte bound of the closed `Capacity` type.
Every harness asserts `closure_invariant`, the unchanged limit and the byte
bound (`layout::array::<T>(cap)` is `Ok`) on every return, for `u8`, `u64`,
`[u64; 200]` (1,600 bytes) and a 64-aligned 64-byte type, under `LengthLimit`
with an arbitrary limit and under `Unlimited`.

| Harness (`vec::growth_proofs::sized::`) | Entry point | Solver time |
| --- | --- | ---: |
| `closure_reserve_preserves_the_invariant_of_every_sized_owner` | `try_reserve` | 66 s |
| `closure_reserve_exact_preserves_the_invariant_of_every_sized_owner` | `try_reserve_exact` | 55 s |
| `closure_grow_preserves_the_invariant_of_every_sized_owner` | `grow`, both `exact` values | 24 s |
| `closure_reserve_length_preserves_the_invariant_of_every_sized_owner` | `reserve_length` | 23 s |
| `closure_shrink_preserves_the_invariant_of_every_sized_owner` | `try_shrink_to`, `try_shrink_to_fit` | 44 s |
| `closure_push_growth_preserves_the_invariant_of_every_sized_owner` | `grow_for_push`, called as `try_push` calls it when `len >= cap` | 19 s |
| `closure_capacity_constructors_establish_the_invariant_of_every_sized_owner` | `try_with_capacity_in`, `try_with_capacity_and_limit_in` | 15 s |

The covers are satisfied for each type and policy: a grown capacity, an
allocator refusal and a layout refusal of the two reserve harnesses; a shrunk
capacity and a refusal of the shrink harness. Limits of this instantiation:
the stubs do not run the budget steps of the storage functions; the stub of
`reallocate` refuses with `Error::Allocation` only (the alignment refusal of
the real function is not modelled; `Vec` never changes the alignment);
`try_push` itself is not run, because its element write needs storage: the
harness calls `grow_for_push` on the owner's words and makes the two stores
of `vec.rs:358-359` itself.

**(b) `T = [(); 1]`, the bitwise branch of `try_extend_cloned` (L12,
`vec.rs:589`).** `[(); 1]` has a copy witness and no bulk hook.
`metadata_proofs::closure_extend_cloned_witness_preserves_the_invariant`
asserts the witness, runs the method from a `closure_owner` on a slice of every
length and asserts the invariant. The control `closure-vec-witness-length`
mutates the store of that branch and is detected, so the branch is the one
that ran.

**(c) A zero-sized type whose clone fails or succeeds on every call**
(`ClosureFlaky`: no witness, no hook). These harnesses carry an assumption
beyond the shared invariant and are **not closure evidence**: every call asks
for at most two new elements (`CLOSURE_CLONES`), so that the element loops are
unwound. They run without `--cfg jzap_loop_contracts`, so the loop of
`try_resize_with_init`, whose contract does not verify here (step 5), is
unwound as well. The owner is a `closure_owner`: its length, capacity and
limit are arbitrary.

| Harness (`vec::metadata_proofs::`) | Method | Stores reached | Extra assumption |
| --- | --- | --- | --- |
| `closure_bounded_extend_cloned_returns_preserve_the_invariant` | `try_extend_cloned` (per-element branch) | L11 | the source has at most 2 elements |
| `closure_bounded_resize_returns_preserve_the_invariant` | `try_resize` | L5, L11 | `new_len <= len + 2` |
| `closure_bounded_resize_owned_returns_preserve_the_invariant` | `try_resize_owned` | L5, L18, L19 | `new_len <= len + 2` |
| `closure_bounded_extend_from_within_returns_preserve_the_invariant` | `try_extend_from_within` | L16 | the range has at most 2 elements |

Each asserts the invariant and the exact length on the success return and on
the error return (the `SuffixGuard` restores the length); the covers show a
success with two new elements and a clone failure. L11, L16, L18 and L19 stay
without closure evidence: the bounded runs do not replace the all-count
harnesses of step 8.

**(d) `Capacity::for_slice`** (`src/layout/proofs.rs`; the proof module of
`capacity.rs` is inside the product file).
`layout::proofs::closure_slice_capacity_matches_the_slice_layout`: for `()`
and `[(); 3]`, a slice of every length and every limit gives the limit as the
count and a zero-byte layout; for `u8`, `u64` and `[u64; 200]`, each prefix of
a real three-element array gives its length and the layout of that slice. A
longer sized slice needs storage: that a valid slice satisfies the byte bound
is the trust boundary on `core`, not shown here.

**Registration** (`scripts/verify_source.py`). The seven stubbed harnesses are
an `EXTRA` invocation of `vec-layout-growth` (`-Z stubbing`,
`--cfg jzap_closure_stubs`). The witness harness is appended to `clone` and
the slice-capacity harness to `layout`. The four bounded harnesses are an
`EXTRA` invocation of `clone` without loop contracts, marked
`extra_assumption`. Fourteen controls are new, one per harness and two for
`try_resize_owned`.

**Run**, all on the tree of this step, each
`python3 scripts/verify_source.py --group <name> --mutations`:

| Group | Harnesses (main, extra) | Controls | Run time | Result |
| --- | --- | ---: | ---: | --- |
| `vec-layout-growth` | 10, 7 stubbed | 18 (11 before) | 756 s | pass |
| `clone` | 3 (2 before), 4 bounded | 7 (1 before) | 480 s | pass |
| `layout` | 3 (2 before) | 4 (3 before) | 75 s | pass |
| `owner-metadata` (rerun, its proof file changed) | 26 | 29 | 763 s | pass |
| `vec-compaction` (rerun) | 4 | 3 | 250 s | pass |
| `vec-splice-fill` (rerun) | 9 | 9 | 376 s | pass |
| `vec-extraction` (rerun) | 3 | 2 | 80 s | pass |
| `buffer` (rerun) | 10, 2 stubbed | 16 | 388 s | pass |

Every harness verified and every control was detected. Solver times of the
new harnesses besides the table above: the four bounded harnesses 11 s to
23 s, the witness harness 3 s, the slice-capacity harness 1 s. Peak memory
was not measured. `python3 scripts/test_verify.py`: 50 OK.
`python3 scripts/verify_owner_protocol.py --mutations` on the same tree: pass,
1,288 s, 65 results, 26 actual-Rust harnesses and 3 staging harnesses as in
the run before step 4. No pinned count changed in any script: the groups
`vec-staging` and `vec-cursors` were not extended.
`cargo kani --lib --harness boxed::buffer::proofs::`, the prefix selection of
`scripts/verify.py` for the Box area, verifies 7 of 7 (1 before step 4).
Not run: `scripts/verify_panic_paths.py`, `scripts/source_certificate.py`,
`scripts/verify.py`.

**Sites after steps 4 to 6.**

| Evidence | Sites |
| --- | --- |
| Closure harness at a zero-sized element (only the shared invariant, or the guard's pair invariant, assumed) | the constructors `new`, `empty` and their `*_in` forms, `try_from_array_in` (L21), `try_into_flattened`; L1-L10, L12 (at `[(); 1]`), L13-L15, L17, L20, L23 (through retain and dedup), L24-L29, L33, L34; S1/C3; F1, F2; C1, C2; Buffer `new`, `reserve`, `push`, `append`, `finish` |
| The same, and at four sized elements with the storage functions stubbed | C1 and C2 through `try_reserve`, `try_reserve_exact`, `grow`, `reserve_length`, `try_shrink_to`, `grow_for_push`; the capacity constructors; Buffer `reserve` and `finish` |
| Harness with an extra assumption (a bound on the request), not closure evidence | L11, L16, L18, L19 (at most two new elements); Buffer `collect` and `collect_rest` (a source of at most two values) |
| No harness | L22, L30, L31, L32; the iterator-driven extenders; the two whole-owner replacements; `try_write_fmt`; the `Vec::from_raw_parts` calls outside `src/vec` |

### Step 7, task A: the loop contract of `try_resize_with_init`

Done on 2026-11-27. One product line changed, inside a `cfg(kani)` attribute;
no contract changed status.

**The failure, isolated.** Kani gives CBMC no write set for a loop, so CBMC
infers one. For every place the loop writes through a pointer (`(*p).len`,
and in the callees it inlines for this analysis `(*q).initialized` of the
`InitSlot`), it takes every local pointer its alias analysis puts in the same
class, casts it to the pointer type of that write and adds the field at that
offset. The analysis does not look at types. The closure that Kani builds from
the loop invariant borrows each place the invariant names, and those borrows
are local pointers of that class. The write set is taken once, at loop entry.

With the invariant as it was (`guard.owner.len.get()`,
`guard.original == original`, `guard.owner.ptr == on_entry(..)`,
`guard.owner.cap.get() == on_entry(..)`), the instrumented program of
`panic_proofs::vec_resize_covers_every_length_and_callback_result` lists 26
targets (`goto-instrument --show-goto-functions` on the kept program). A trace
of the failing check (`cbmc --trace --property
__CPROVER_contracts_car_set_insert.assertion.1`) gives:

| Target (index) | From the borrow | Address at loop entry | Effect |
| --- | --- | --- | --- |
| `(*guard.owner).len` (0 to 4) | the owner pointer | `&v.len`, 8 bytes | the write the loop makes |
| `((InitSlot*)p).initialized`, `((Vec*)p).len` (5, 6) | `p = &v.len` | `&v.cap`, 1 and 8 bytes | the loop may change `cap`; the invariant restated it with `on_entry` |
| the same two (11, 12) | `p = &v.cap` | `&v.limit`, 1 and 8 bytes | the loop may change `limit`; nothing restated it |
| `((InitSlot*)p).initialized` (18) | `p = &guard.original` | `&guard + 16`, one byte past the 16-byte guard | `car_set_insert` asserts that a target is writable: **"ptr NULL or writable up to size" fails**; the two "pointer outside object bounds" failures are reads of this target and of index 19 |

So the instrumentation failure came from `guard.original == original` in the
invariant, and the unframed `limit` came from `guard.owner.cap.get()`. Neither
is a defect of the product loop.

**Minimal reproduction** (a disposable crate outside the repository; kept
locally under `target/verification/i16/step7/repro/`): an owner of four words,
a guard holding `&mut Owner` and a word, a slot with a flag, the same loop.
Kani 0.68.0, CBMC 6.11.0, Z3 4.16.0, one run per variant of the invariant:

| Variant | Result |
| --- | --- |
| the product's form | the same three failed checks, and the harness's frame assertion on `cap` and `limit` |
| without `guard.original` | `car_set_insert` still fails, at the target built from the `on_entry` copy of the pointer word: that copy is a pointer of the class, and offset 8 from the one-byte object it points to is outside it (trace) |
| the length only, by field | no instrumentation failure; the frame assertion fails (the word after `len` is in the write set) |
| the product's form with `kani::loop_modifies(&guard.owner.len)` | "Check that var_N is assignable" fails for the compiler temporaries of the loop body and for `slot`: an explicit write set has to name every local the body assigns, and temporaries have no name |
| the length only, read through a `&self` getter of the owner | no instrumentation failure. The targets built from the invariant's borrow are `(*owner).len` and the byte at offset 8 of the owner. The model's body also gives whole-object targets (the owner and its storage, through pointers the body loads inside the loop), so the model's frame assertion still fails; the product's write set has no such target (below) |

**The fix** (`src/vec.rs:531`, the whole edit):

```
#[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(original <= guard.owner.len() && guard.owner.len() <= new_len))]
```

`guard.owner.len()` borrows the owner whole, so the only pointer of the class
is the owner's address. The reproduction shows the mechanism, not the fix: the
fix is checked on the product. The write set of the same harness is then 10
entries, all of them listed by `goto-instrument`:
`(*guard.owner).len` (twice), the byte at offset 8 of the owner (twice; it is
the low byte of `len`, the trace above has `&v.len` at offset 8) and six
locals of the body. `ptr`, `cap`, `limit` and the guard are outside it and
need no clause. This depends on the layout of `Vec<(), Unmetered, L>` and of
`InitSlot<()>` under this toolchain; if byte 8 were another word, the closure
harness below would fail, not pass.

**Pins changed.**

| Pin | Change |
| --- | --- |
| `scripts/callsite_projection.py`, `RESIZE_WITH_INIT` | the attribute text inside the pinned function body |
| `tools/panic-audit/src/main.rs`, the loop-predicate grammar | `.get()` on `guard.owner.len` and `guard.owner.cap` is no longer accepted, and `src/vec.rs::try_resize_with_init` left the list of loops with metadata getters. The new predicate is accepted as this exact text at this loop only (`OWNER_LENGTH_INVARIANTS`), and only when `Vec::len` is the reviewed one-expression getter (`reviewed_owner_length`). Two unit tests are new (the predicate at another file, function or with a changed text is refused; a changed getter is refused), one is extended |

**Harnesses.**

| Harness | Assumes | Result | Solver time |
| --- | --- | --- | ---: |
| `panic_proofs::vec_resize_covers_every_length_and_callback_result` (existing; `RUSTFLAGS="--cfg jzap_loop_contracts" cargo kani --lib -Z loop-contracts --exact --harness ..`, Z3 on `PATH`) | `len <= limit` on a `cap == limit` owner | verified | 11 s |
| `vec::metadata_proofs::closure_resize_with_init_preserves_the_invariant` (new; L11) | `closure_owner` only | verified, in the `owner-metadata` group (CaDiCaL) | 25 s |

The closure harness runs the method on every length and capacity word under
`LengthLimit` with any limit and under `Unlimited`, for every requested
length; the callback fails, returns without a write or writes, chosen on every
call. It asserts the invariant, the unchanged limit and pointer, a capacity
that did not shrink and the exact length on every return (the error returns
go through the `SuffixGuard`). Covers satisfied: growth, a callback error, a
missing write, and a limit refusal (limited case). Two controls are new and
detected: `closure-vec-resize-init-length` (the store publishes
`usize::MAX`) and `closure-vec-resize-init-reservation` (the reservation asks
for nothing).

**Run.**

- `python3 scripts/verify_source.py --group owner-metadata --mutations`: pass
  in 1,050 s; 27 of 27 harnesses verified, 31 of 31 controls detected.
- Machine code. The four benchmark executables built from an export of
  `4e0158a` and from an export of this tree, in two directories of equal path
  length: 851, 728, 668 and 594 function bodies, all identical (only first 0,
  only second 0, each). Built one after the other in the same directory, the
  four files are byte-identical.
- `cargo fmt --check`: clean. `cargo test --locked --offline --features std`:
  460 passed, 0 failed. `cargo clippy --locked --offline --lib` with
  `--all-features` and with `--no-default-features`, `-D warnings`: clean.
- `cargo test` of `tools/panic-audit`: 5 passed. `scripts/test_verify.py`:
  50 OK. `scripts/verify_owner_protocol.py` (without mutations): pass, 385 s.

Not run in this task: `scripts/verify_panic_paths.py` (its entry for the
existing harness is expected to report "pass" now), the other loop-contract
harnesses of `src/panic_proofs.rs`, `scripts/source_certificate.py`.

`try_resize`, `try_resize_with`, the per-element branch of
`try_extend_cloned` and the two `try_clone_in` functions call this loop with
other callbacks. They write no owner word themselves; they were not run under
the loop contract in this task.

### Step 7, task B: guards, the whole splice and the whole-owner replacements

Done on 2026-11-27. No product file changed and no contract changed status.

**Splice** (`src/vec/edit_proofs.rs`). The owner comes from `closure_owner`
and the guard from `closure_drain` (any ordered unread range and any tail words
that satisfy `closure_drain_invariant`). The source of every harness ends,
fails or yields on each call, without a bound, and its size hint is an
arbitrary word or an error.

| Site | Function | Harness (`vec::edit::panic_proofs::`) | Assumes | Result |
| --- | --- | --- | --- | --- |
| F1 `edit.rs:450` | `Splice::fill` | `closure_splice_fill_preserves_the_owner_and_pair_invariants` (step 5), now also asserting that `Ok(true)` leaves `len == tail_start` | owner and pair invariant | verified |
| `edit.rs:439` (`tail_start`) | `Splice::move_tail` | `closure_move_tail_preserves_the_owner_invariant_and_the_filled_pair` | owner and pair invariant | verified |
| L31, L32 `edit.rs:592-593`, `edit.rs:591` (`tail_start`) | `Splice::stage_excess` | `closure_stage_excess_preserves_the_owner_invariant_and_the_filled_pair` (`LengthLimit`) and `.._unlimited` | owner and pair invariant | verified |
| L30 `edit.rs:518` | `Splice::try_finish`, whole | `closure_splice_finish_with_a_tail_preserves_the_invariant` (`LengthLimit`) and `.._unlimited` | owner and pair invariant, **and `tail_len != 0`** | verified; not closure evidence while the path is selected by an assumption |

`move_tail` and `stage_excess` keep the pair invariant only from the phase a
completed `fill` leaves, `len == tail_start`. Their harnesses assume nothing
about the phase: they run from every pair state, assert the owner invariant
and the unchanged limit on every return, and assert the pair invariant (and,
for `stage_excess`, `len == tail_start` again) for the states that started in
the phase. On refusal every word is unchanged. `stage_excess` asserts that the
length and `tail_start` move by the same count.

The phase fact at the two call sites inside `try_finish`:

- `fill` returns `Ok(true)` only with `len == tail_start`, from every pair
  state (the harness above).
- No Kani run asserts the fact at the call sites of the actual `try_finish`.
  The method consumes the guard, so a harness reads the state only after the
  guard's drop, and a proof cannot place an assertion inside product code.
  Between the return of `fill` and each call the function writes neither word
  (`try_next`, `try_size_hint`, arithmetic and `try_reserve`): this is a
  reading, not a run.
- The maintained function `splice_operation` is generated from the pinned text
  of `try_finish`; its callees `hole_move` and `stage_operation` require
  `len == tail` and Verus derives it from the `ensures` of `hole_fill`. That
  covers the projection, as before this step.

The whole-`try_finish` harnesses assert the owner invariant after the guard's
drop on every return (success, source error, refusal) and that the length is
at least the old length plus the tail. The addition in `Drain::restore_tail`
is checked in place by the `cfg(kani)` assertion of `wrapping_add`.

The staging loop of `staging_driver::run` has no contract. At a zero-sized
element the staging owner starts with its whole limit as capacity, so the loop
takes at most two turns; the unwinding assertion (`kani::unwind(4)`) checks
that bound, and the cursor loop runs under its existing contract. L32
(`extra.len = 0`) is followed only by the drop of `extra`; no harness can
observe it.

**IntoIter** (`src/vec/ownership_proofs.rs`). The pair it keeps with its
owner is `closure_iter_invariant`: `span.front <= span.back`,
`span.back <= cap` and `len == 0`. `closure_iter` builds an owner from
`closure_owner` and any range, and assumes that pair invariant and nothing
else.

| Method | Harness (`vec::ownership::panic_proofs::`) | Result |
| --- | --- | --- |
| `into_iter` (L34) | `closure_into_iter_preserves_the_invariant` (step 5), now asserting `closure_iter_invariant` | verified |
| `as_slice`, `as_mut_slice`, `as_ref`, `as_mut`, `size_hint`, `len`, `next`, `next_back`, `drop` | `closure_iter_steps_preserve_the_pair_invariant` | verified |
| `try_clone` | `closure_iter_clone_establishes_the_pair_invariant_of_the_copy` | verified |
| `try_clone_from` (the swap at `ownership.rs:252`) | `closure_iter_clone_from_replaces_the_whole_value_under_the_pair_invariant` | verified |
| `default` | `closure_iter_default_establishes_the_pair_invariant` | verified |

**The two whole-owner replacements.**
`metadata_proofs::closure_clone_from_replaces_the_whole_owner_under_the_invariant`
runs `Vec::try_clone_from` (the swap at `vec.rs:700`) on two owners from
`closure_owner` with their own limits: the receiver satisfies the invariant
under its own limit on every return, takes the source's length on success and
keeps every word on refusal; the source is unchanged. The covers show the
replacement path, the in-place path of equal lengths and a limit refusal. The
IntoIter harness above does the same for the pair invariant. The value that
the swap moves out is dropped inside the method, where no harness can read it:
it is the receiver's previous value, word for word.
`closure_whole_value_exchange_keeps_both_invariants` states that exchange on
the two types (`mem::swap` of two values that each satisfy their invariant).
It runs no product statement, so it has no control.

**The callers of the L11 loop.**
`metadata_proofs::closure_resize_callers_preserve_the_invariant` runs
`try_resize`, `try_resize_with` and the per-element branch of
`try_extend_cloned` under the loop contract of task A, with a zero-sized value
whose clone fails or succeeds on every call and with no bound on the request.

**Registration and controls** (`scripts/verify_source.py`). The move-tail and
stage-excess harnesses join `vec-splice-fill`; the two whole-`try_finish`
harnesses are an `EXTRA` invocation of that group marked
`extra_assumption='a retained tail (tail_len != 0)'`. The IntoIter harnesses
and the `Vec::try_clone_from` harness join `owner-metadata`; the L11 callers
join `clone`. Thirteen controls are new, each a product mutation that must end
in a solver counterexample of the named harness:

| Control | Mutation | Harness that must fail |
| --- | --- | --- |
| `closure-vec-splice-fill-phase` | `init::collection::finished` reports a full hole when the source ended | the fill harness (the new phase assertion) |
| `closure-vec-move-tail-reservation` | `move_tail` reserves the hole without the tail | the move-tail harness |
| `closure-vec-stage-excess-length` | L31 publishes one more | the stage-excess harness (`LengthLimit`) |
| `closure-vec-stage-excess-growth` | `stage_excess` never grows | the stage-excess harness (`Unlimited`) |
| `closure-vec-splice-finish-length` | L30 publishes one more | the whole-`try_finish` harness (`LengthLimit`) |
| `closure-vec-splice-finish-move` | `try_finish` skips `move_tail` | the whole-`try_finish` harness (`Unlimited`) |
| `closure-vec-clone-from-swap` | `Vec::try_clone_from` returns without the swap | the `try_clone_from` harness |
| `closure-vec-iter-step` | `Span::next` jumps to the end | the IntoIter steps harness |
| `closure-vec-iter-clone-length`, `closure-vec-iter-default-length` | `into_iter` publishes `usize::MAX` | the IntoIter clone and default harnesses |
| `closure-vec-iter-clone-from-swap` | `IntoIter::try_clone_from` returns without the swap | the IntoIter `try_clone_from` harness |
| `closure-vec-resize-callers-commit` | `try_resize_with_init` drops its guard on success | the L11 callers harness |

**Run**, each `python3 scripts/verify_source.py --group <name> --mutations`
on the tree of this task (Kani 0.68.0, CBMC 6.11.0, CaDiCaL,
`--object-bits 10`):

| Group | Harnesses (main, extra) | Controls | Run time | Result |
| --- | --- | ---: | ---: | --- |
| `vec-splice-fill` | 12 (9 before), 2 with a retained tail | 15 (9 before) | 1,216 s | pass |
| `owner-metadata` | 33 (27 before) | 36 (31 before) | 1,778 s | pass |
| `clone` | 4 (3 before), 4 bounded | 8 (7 before) | 1,392 s | pass |

Every harness verified and every control was detected. Solver times of the
new harnesses: the whole `try_finish` 192 s (`LengthLimit`) and 135 s
(`Unlimited`), stage-excess 50 s and 48 s, move-tail 20 s, the L11 callers
157 s, every IntoIter and replacement harness at most 10 s. Peak
memory was not measured. `python3 scripts/test_verify.py`: 50 OK.
Not run in this task: `scripts/verify_owner_protocol.py`,
`scripts/verify_wrapping_memory.py` (both at the end of step 8),
`scripts/verify_panic_paths.py`, `scripts/source_certificate.py`.

**Not covered by this task.**

| Site | Reason |
| --- | --- |
| the empty-tail branch of `Splice::try_finish` (`while let` over `try_push`) | needs a loop contract; step 8. Until then the whole-`try_finish` harnesses select the other path by an assumption |
| the Drain words at the return of `try_finish`, and the phase fact at its two call sites, on the actual function | not observable by a harness (above) |
| L32 `edit.rs:593` | not observable (above) |
| `try_extend_fallible`, `try_extend`, `try_from_iter_in`, `try_from_fallible_iter_in` | a `while let` over the source without a contract; step 8 |
| `IntoIter::try_clone_in`, `Vec::try_clone_in`, `IntoIter`'s `Debug` | not run |
| `Splice`'s and `Drain`'s iterator methods on a Splice, `Drain::as_slice`, the `Debug` impls | read-only or delegated to the Drain steps of step 5; not run separately |

### Step 8 (task C): loop contracts on three more product loops

Done on 2026-11-27 for three loops; two loops are not covered (below). Three
product lines were added, each a `cfg(kani)` attribute; no contract changed
status.

**What Kani 0.68 accepts.** A `kani::loop_invariant` compiles on `while`,
`while let`, `loop` and `for` over a `usize` range (the unknown U1 and U2 of
the design). The macro rewrites `while let` to `loop { match .. }`, and `for`
to an index loop over Kani's own model of the iterator
(`kani::KaniIntoIter`): the `core` range iterator is not executed under the
contract. A separate check on a disposable crate (kept locally under
`target/verification/i16/step8/range/`) verifies, for every pair of words
`a <= b`, that the model of `a..b` runs `b - a` turns with the index
`a + turn`; for `a > b` the model's length subtraction fails verification
(`library/kani_core/src/iter.rs:142`), where `core` runs no turn. The product
computes `end - start` with a checked subtraction before its loop.

**The attributes** (the whole product change):

| File, loop | Attribute |
| --- | --- |
| `src/vec/convenience.rs`, the `for i in start..end` of `try_extend_from_within` | `#[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(original <= guard.owner.len() && guard.owner.len() - original == kani::index))]` |
| `src/vec/convenience.rs`, the `while` of `try_resize_owned` | `#[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(original <= guard.owner.len() && guard.owner.len() <= last))]` |
| `src/vec.rs`, the `while let` of `try_extend_fallible` | `#[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(original <= guard.owner.len() && guard.owner.len() <= guard.owner.capacity() && guard.owner.capacity() <= guard.owner.max_len() && guard.owner.max_len() == on_entry(guard.owner.max_len()) && !guard.owner.as_ptr().is_null()))]` |

Each reads the owner through `&self` getters only, for the reason found in
task A. The loop of `try_extend_fallible` calls `try_push`, which writes the
pointer and the capacity through pointers of its own; the inferred write set
is then the whole owner (`goto-instrument` lists a whole-object target), so
the invariant restates what the loop keeps of every word: the order of the
three counts, the unchanged limit and a non-null pointer. `kani::index` is
the count of completed turns of the `for` loop.

By a text search of the proof files and of the product callers of the three
functions, no existing harness reaches one of these loops in a Kani run with
loop contracts: the bounded harnesses of step 6 run without the cfg, and
`scripts/verify_wrapping_memory.py` and the `scripts/verify.py` Kani
selections do not pass it. The other groups of `scripts/verify_source.py`
were not rerun on this tree.

**Harnesses** (`src/vec/metadata_proofs.rs`, all from `closure_owner` only,
every word, `LengthLimit` with any limit and `Unlimited`):

| Store | Function | Harness (`vec::metadata_proofs::`) | Inputs | Solver time |
| --- | --- | --- | --- | ---: |
| L18, L19 `convenience.rs:215`, `:235` | `try_resize_owned` | `closure_resize_owned_preserves_the_invariant` | any requested length; a zero-sized value whose clone fails or succeeds on every call | 21 s |
| L16 `convenience.rs:130` | `try_extend_from_within` | `closure_extend_from_within_preserves_the_invariant` | both endpoints of any `Bound` form; the same value type | 44 s |
| (through C2 and L14) | `try_extend_fallible` | `closure_extend_fallible_preserves_the_invariant` | a source that ends, fails or yields on every call | 26 s |
| (the same loop) | `try_extend`, `try_from_iter_in`, `try_from_fallible_iter_in` | `closure_iterator_entry_points_preserve_the_invariant` | a `core` iterator and a fallible source without a bound | 94 s |

Each asserts the invariant and the unchanged limit on every return; the
error returns go through the `SuffixGuard`. `try_resize_owned` asserts the
exact length, `try_extend_from_within` the count of the normalized range.
The covers show growth, a clone or source failure and a limit refusal.

**Pins changed.**

| Pin | Change |
| --- | --- |
| `scripts/callsite_projection.py`, `RESIZE_OWNED` | the attribute text inside the pinned body of `try_resize_owned` |
| `tools/panic-audit/src/main.rs` | `OWNER_INVARIANTS` lists the four predicates (the one of task A and the three above), each accepted as exact text at its loop only; the reviewed getters are `len`, `capacity`, `max_len` and `as_ptr` of `Vec<T, P, L>`, each compared with its whole definition and required exactly once. Unit tests: each predicate is refused at another file or function and with one more conjunct; six changed getter bodies are refused |

**Registration.** A new group `vec-element-loops` in
`scripts/verify_source.py` holds the four harnesses and six controls
(`closure-vec-resize-owned-loop-length`, `-last-length`,
`closure-vec-extend-within-loop-length`, `-reservation`,
`closure-vec-extend-fallible-length`,
`closure-vec-iterator-constructor-length`): each makes one store publish a
wrong length or drops the reservation, and must end in a solver
counterexample. The group is not in `FOUNDATION_GROUPS` of
`scripts/source_certificate.py`; the certificate does not run it.

**Run.**

- `python3 scripts/verify_source.py --group vec-element-loops --mutations`:
  pass in 1,688 s; 4 of 4 harnesses verified, 6 of 6 controls detected.
- Machine code. The four benchmark executables built from an export of the
  commit before this step (`71ca6e5`) and from an export of this tree, in two
  directories of equal path length: 851, 728, 668 and 594 function bodies,
  all identical (only first 0, only second 0, each). Built one after the
  other in the same directory, the four files are byte-identical.
- `cargo fmt --check`: clean. `cargo test --locked --offline --features std`:
  460 passed, 0 failed. `cargo clippy --locked --offline --lib` with
  `--all-features` and with `--no-default-features`, `-D warnings`: clean.
- `cargo test` of `tools/panic-audit`: 5 passed. `scripts/test_verify.py`:
  50 OK.
- `scripts/verify_owner_protocol.py --mutations` on this tree: pass,
  1,299 s, 65 results, `source_closure_complete`, no reused prerequisite, 26
  actual-Rust harnesses and 3 staging harnesses, as before step 7.
- `scripts/verify_wrapping_memory.py --mutations` on this tree: pass, 626 s,
  9 controls detected; 53 modular calls, 26 established and 27 stated, as
  before.

Not run: `scripts/verify_panic_paths.py`, `scripts/source_certificate.py`,
`scripts/verify.py`, and the other groups of `scripts/verify_source.py` on
this tree.

**Not covered: two loops.**

| Loop | What was run | Result |
| --- | --- | --- |
| the empty-tail `while let` of `Splice::try_finish` (`edit.rs:467`) | An invariant of the form used for `try_extend_fallible`, in a disposable copy, with a whole-`try_finish` harness. `goto-instrument --dfcc` was killed by the kernel for memory once (another verifier was running), and under a 9.5 GB address-space limit it ended with "Out of memory" after 172 s with the cursor-loop contract compiled in and after 168 s without it | no CBMC run took place; the attribute was not added. The branch keeps the bounded harness of step 1 (at most two values) and the whole-`try_finish` harnesses keep their path assumption |
| the `loop` of `Buffer::collect_rest` (`buffer.rs:140`) | An invariant that calls a helper of the proof module on the whole owner, in a disposable copy, with an unbounded source and both loop contracts (this loop and the cursor loop of `init::collection::fill` that it calls) | no result: CBMC was stopped after about 40 minutes of symbolic execution. The instrumented program lists, for the outer loop, targets built by casting the owner and source pointers to the write-set types of the contract library (the inner loop's instrumentation is part of the outer loop's body); the attribute was not added. `collect` and `collect_rest` keep the two-value harness of step 4 |

A loop contract on a product loop changes what every Kani run with
`--cfg jzap_loop_contracts` and `-Z loop-contracts` does at that loop
(abstraction in place of unwinding). For the two loops above this would also
have reached existing harnesses that assert exact contents or counts
(`proofs::collected_slice_reallocation_preserves_values` and the two-value
collect harness in `buffer`; `empty_tail_splice_publishes_past_the_tail_start`
in `vec-splice-fill`).

`try_write_vectored` (L22, `u8` only) stays out of scope and is not covered.

**Sites after steps 4 to 8** (line numbers of this tree).

| Evidence | Sites |
| --- | --- |
| Closure harness at a zero-sized element (only the shared invariant, or the guard's pair invariant, assumed) | the constructors `new`, `empty` and their `*_in` forms, the capacity constructors, `try_from_slice_in`, the two adopting constructors (unit storage), `try_from_array_in` (L21), `try_into_flattened`; L1-L10, L11 (direct and through its callers), L12 (at `[(); 1]`), L13-L15, L16, L17, L18, L19, L20, L23 (through retain and dedup), L24-L29, L31, L33, L34; S1/C3; F1, F2; C1, C2; the two writes of `tail_start`; `try_extend_fallible` and its three entry points; the `IntoIter` methods; the two whole-owner replacements; Buffer `new`, `reserve`, `push`, `append`, `finish` |
| The same, and at four sized elements with the storage functions stubbed | C1 and C2 through `try_reserve`, `try_reserve_exact`, `grow`, `reserve_length`, `try_shrink_to`, `grow_for_push`; the capacity constructors; Buffer `reserve` and `finish` |
| Harness with an extra assumption, not closure evidence | L30 `edit.rs:518` (the whole `try_finish` assumes `tail_len != 0`; the store is reached only on that path); the empty-tail branch of `try_finish` (at most two values); Buffer `collect` and `collect_rest` (a source of at most two values); the bounded harnesses of step 6 for L11, L16, L18 and L19, now beside their closure harnesses |
| Run by a harness that cannot observe it | L32 `edit.rs:593` |
| No harness | L22 `convenience.rs:426` (`u8` only); `try_write_fmt` and its `Writer`; `Vec::try_clone_in` and `IntoIter::try_clone_in`; the `Debug` impls; `Buffer::drop`; the `Vec::from_raw_parts` calls outside `src/vec` |

As before: every instantiation is a zero-sized element with `Unmetered`
(one exchange fixture with `Budget::Limited`), the capacity paths also at four
sized elements with stubbed storage; no pointer-touching writer runs at a
sized element for every word, and no `needs_drop` path runs.

### Step 9: the closure check and the two approved bases

Done on 2026-11-28. No product file changed: `git diff cc45c14 -- src
':!src/**/*proofs.rs'` is empty. Four contracts changed status, all to
`instantiated-kani`; none to `trust-boundary`.

**Task 1: the Buffer lends for a source without a bound.** The WIP commit
`cc45c14` had two uncompiled harnesses. Kept: the open source and both
harnesses. Changed: the `collect` harness binds the reservation as
`let Ok(reserved_end) = owner.reserve(lower, lower) else`, so that the producer
of the end can be pinned, and asserts the invariant after the reservation.

| Harness (`boxed::buffer::proofs::`) | Lend | End | Solver time |
| --- | --- | --- | ---: |
| `closure_collect_lend_preserves_the_invariant_for_every_unit_owner_and_open_source` | `collect` | `reserve(lower, lower)` for any hint | 4.1 s |
| `closure_collect_rest_lend_preserves_the_invariant_for_every_unit_owner_and_open_source` | `collect_rest` | the capacity word | 2.7 s |

Each makes the lend itself on a `closure_owner` (`T = ()`), with a source
that ends, fails or yields on every call and no bound on the count; the
cursor loop of `init::driver::run` runs under its existing contract
(`--cfg jzap_init_loop_contracts`; the log lists its `loop_invariant_base`
and `loop_invariant_step` checks). Each asserts the Buffer invariant on every
exit. A harness cannot enter `collect` at the lend, so the correspondence is
pinned: `caller_bindings.LEND_HARNESSES` lists, per lend, the product
statements and the harness statements (compacted, without comments); each
occurs exactly once and in order, and the harness lend is the product lend
with `owner.` for `self.` (checked in `caller_bindings.check` and by a unit
test with six mutated texts). The state at the lend is any valid owner; in the
product it is reached only through writers that keep the invariant (the
closure below). Registration: a second `EXTRA` invocation of the `buffer`
group with that cfg; two controls, each detected:
`closure-buffer-collect-lend-end` (`reserve` admits one more than it
reserved) and `closure-buffer-collect-rest-lend-publication` (the fill
publishes `usize::MAX`).

**Task 2.** `vec-element-loops` is in `FOUNDATION_GROUPS` and in the
direct-source plans of `scripts/verify.py` (areas `vec`, `foundation`,
`all`). The direct-foundation stage pinned 57 controls; that pin had not been
updated since before step 1 (58 controls after step 1, 136 after step 8), so
the stage would have failed on it. It is now `FOUNDATION_CONTROLS = 144`
(136, the two lend controls, the six of `vec-element-loops`), and a unit test
keeps it equal to the count in `verify_source.CONTROLS` and requires every
group with a closure harness to be one the certificate runs.

**Task 3: `scripts/owner_closure.py`.** Sites, from the scanner inventory of
`caller_bindings.py`: every struct literal of the owner, every assignment to
an owner word (`Vec`: `len`, `cap`, `limit`; `Buffer`: `len`, `cap`,
`layout`), as a member place or through a local reference (`*cap` in
`grow_for_push`), every `state-lend` row, and, from the unsafe-operation
scanner, every call of `Vec::from_raw_parts` (another spelling of a
`from_raw_parts` path that is not a known non-owner one fails). The owner
fields are private and the owner modules include no non-proof file by
`#[path]` (both checked), so only these files can write the words. The
table `SITES` maps each site (owner, kind, file, impl, function, expression,
occurrence) to harnesses or to `trust-boundary` (an item of the fixed list,
only for the literal of a `pub unsafe fn`), `other-unit:<n>`,
`not-instantiable` (checked: the impl is `Vec<u8, ..>` or `String`),
`unobservable` (with the argument) or `uncovered`.

The script fails on a site without an entry, an entry without a site, a
harness that is not in a `verify_source.py` group of `FOUNDATION_GROUPS`, a
changed fixture helper (the exact `kani::assume` list of `closure_owner`,
`closure_sized_owner`, `closure_iter`, `closure_drain`, `closure_extract` and
Buffer's `closure_owner`, and the owner and pair invariants they assume, are
pinned) and an extra-assumption harness of `verify_source.EXTRA` that it does
not list. Admissibility is read from the proof source: the harness and every
proof function or macro it names (module-aware, transitively, the pinned
helpers excluded) contains no `kani::assume` or `any_where`, builds no owner
by a literal or `from_raw_parts`, stores to and lends no owner word (except the
pinned lend statements above), and a writer harness takes its owner from a
helper unless the function writes an owner it creates itself (pinned:
`let mut result = Self::empty(policy, limit);` in `try_from_array_in`).
Extra-assumption harnesses (the four `closure_bounded_*`, the two-value
Buffer collect, the two whole-`try_finish`) count for no site, with one
exception: the whole-`try_finish` harnesses cover L30
(`self.drain.owner.len = next`), because the store is only on the
`tail_len != 0` path (checked: the `if self.drain.tail_len == 0` branch ends
in `return Ok(())` and writes no owner word, and `tail_len` is not written
before the store) and their only extra assumption is exactly
`kani::assume(tail_len != 0)`. They cover nothing on the empty-tail path,
which has no site of its own (it calls `try_push`).

Proof-only change for this check: the ExtractIf drop harness assumed its pair
state inline; the two assumptions are now the helper `closure_extract`, as
for `closure_drain`. `vec-extraction` was rerun.

Output of `python3 scripts/owner_closure.py --controls`:

| Owner | Closed at `T = ()` | Sites | Dispositions |
| --- | --- | ---: | --- |
| `Buffer` | **yes** | 8 | 8 harness: the constructor `new`; `reserve` (`cap`, `layout`), `push`, `append` and `finish` (`layout`); the lends of `collect` and `collect_rest` |
| `Vec` | **no** | 54 | 47 harness, 2 unobservable, 2 not-instantiable, 1 trust-boundary, 2 other-unit:4 |

| Vec site | Disposition | Reason |
| --- | --- | --- |
| `src/vec/ownership.rs:30`, the literal of `Vec::from_raw_parts` | trust-boundary (public unsafe caller) | the words are the arguments of the `pub unsafe fn`; each library call is a site of its own |
| `src/vec/convenience.rs:373`, `try_into_flattened` calls `Vec::from_raw_parts` | harness | `closure_flatten_establishes_the_invariant_of_the_element_owner` at `[(); N]` |
| `src/deque.rs:521`, `VecDeque::into_vec` calls `Vec::from_raw_parts` | **other-unit:4, blocks** | the deque's ring words |
| `src/deque.rs:578`, `VecDeque::try_from_std_in` calls `Vec::from_raw_parts` | **other-unit:4, blocks** | words from the deque's adoption of a std value |
| `src/string/convert.rs:218`, `String::from_raw_parts` calls `Vec::from_raw_parts` | not-instantiable | a `Vec<u8>` only; it forwards the words of the public unsafe `String::from_raw_parts` |
| `src/vec/convenience.rs:426`, L22 `try_write_vectored` | not-instantiable | `Vec<u8>` only |
| `src/vec/convenience.rs:248`, L20 `try_into_array` | unobservable | followed only by the read of the array and the drop of the consumed owner |
| `src/vec/edit.rs:593`, L32 `stage_excess` (`extra.len = 0`) | unobservable | followed only by `Ok(())` and the drop of the local staging owner |
| every other constructor, write and lend (47) | harness | the harnesses of steps 4 to 8, each admissible; L30 under the path assumption above |

Sized capacity paths (`u8`, `u64`, `[u64; 200]`, a 64-aligned 64-byte type,
storage stubbed): for Buffer `reserve` and `finish`; for Vec the six stubbed
harnesses of `try_reserve`, `try_reserve_exact`, `grow`, `reserve_length`,
`try_shrink_to` and the capacity constructors. The stubbed push-growth
harness is not admissible: it makes the two stores of `try_push`
(`owner.ptr = ptr`, `owner.cap = words`) itself and that text is not pinned.

Controls, each detected: a second, unmapped `self.len` store in
`Buffer::push` (the script fails), a `kani::assume` added to the Buffer push
harness (Buffer not closed), the Buffer push harness removed from its group
(the script fails), the `collect_rest` lend mapped to `other-unit:9` (Buffer
not closed). Unit tests also refuse a stale entry, a missing entry, a
`trust-boundary` on a write, an unknown harness, an extra-assumption harness
used for another site and the path assumption used for another site.

Not decided by the script: an early `return` on a nondeterministic value
restricts inputs like an assumption and is not detected by the text check;
the scanner channels that step 3 lists as not recorded remain so; the guard
words (`tail_start`, `tail_len`, the `Extraction` and `Compaction` cursors)
are not input sites (their constructors and writers have harnesses or are
closed state types, but this script does not map them).

**Task 4: the bases.** `premise_status.py` has the dispositions
`instantiated-kani` and `trust-boundary` beside the three before;
`source_certificate.py` counts all five, prints them and the instantiations,
and is complete only with no `premise-stated-not-proved` and no `unproved`
row. The certificate runs `owner_closure.report` and `check_instantiated`,
which fails when a contract has `instantiated-kani` while its owner is not
closed, when its maintained `requires` is not exactly the owner invariant
(compared as text with `{len}<={cap},byte_valid({cap},size)` plus one
`call_requires` per named callback), or when a bound row is outside a
function of the owner generic in its element type.

| Contract | Rows | Maintained `requires` | Old | New |
| --- | ---: | --- | --- | --- |
| `buffer_reserve` | 5 | `len<=cap, byte_valid(cap, size)` | stated | instantiated-kani |
| `buffer_push` | 3 | the same | stated | instantiated-kani |
| `buffer_finish` | 5 | the same | stated | instantiated-kani |
| `buffer_collect` | 4 | `initial<=initial_cap, byte_valid(initial_cap, size)` and two `call_requires` | stated | instantiated-kani |

Instantiations: `T = ()` for every method and lend (`Unmetered`); `T = u8`,
`u64`, `[u64; 200]` and a 64-aligned type for `reserve` and `finish` with the
storage functions stubbed. The harness invariant
(`layout::array::<T>(cap) == Ok(layout)`) implies the byte bound of the
maintained `requires`.

Considered and left stated: `reservation_before_bulk_write` (Buffer::append;
its `requires` is `len <= cap <= limit`, Buffer has no limit), `buffer_span`
(its rows include the unsafe blocks of `init::collection`, shared with the
Vec callers `Splice::fill` and `stage_batch`, and its `requires` adds
`count <= cap - initial`), `empty_buffer` (already `no-premise`), every Vec
contract (Vec is not closed). For `trust-boundary` no contract qualifies:
`raw_parts_owner` binds the literal of `Vec::from_raw_parts`, which library
callers in `src/deque.rs` also reach; `adopted_owner` also requires the
budget invariant; `capacity_bytes` rests on `Capacity::from_validated_raw`,
a `pub(crate)` unsafe fn.

**Row counts.** Computed without the link stage: the certificate's own
`obligations` and `obligation_counts` on the binding lists, with the
verified set composed as in `source_certificate.main`; before with
`premise_status.py` of `cb8565b`, after with this tree. Owner rows: the
fresh `verify_owner_protocol.py --mutations` run below. Area rows: the
local summaries of bindings-only runs of 2026-11-27 (String, shared, B-tree,
hash, heap, deque, scratch); no area source file or binding script changed in
git since `63dc3b7`, but those summaries were not regenerated for this step.
The script is kept locally as `target/verification/i16/step9/counts.py`.

| Disposition | Before | After |
| --- | ---: | ---: |
| solver-and-source-connected | 105 | 105 |
| instantiated-kani | — | 17 |
| trust-boundary | — | 0 |
| premise-stated-not-proved | 1,040 | 1,023 |
| unproved | 36 | 36 |
| total | 1,181 | 1,181 |

The 17 moved rows are exactly those of the four contracts above (accounting
calls, state publications and unsafe blocks in `src/boxed/buffer.rs`); no
other row changed. The 36 unproved rows are an artifact of this computation:
their contracts are `kani:` harnesses and `direct:` targets
(`deque::proofs::*`, `string::proofs::scalar_encoding_*`,
`direct:shared-numeric`, `direct:budget-admit`, the heap, hash-set and
scratch scenarios) whose names the bindings-only summaries do not list as
verified; a certificate run fills them from its Kani stages. The certificate
itself was not run.

**Run.**

- `python3 scripts/verify_source.py --group buffer --mutations`: pass, 390 s;
  10 + 2 (stubbed) + 2 (lend) harnesses, 18 of 18 controls detected.
- `python3 scripts/verify_source.py --group vec-extraction --mutations`:
  pass, 61 s; 3 harnesses, 2 controls detected.
- `python3 scripts/owner_closure.py --controls`: the output above, 4 of 4
  controls detected.
- `python3 scripts/verify_owner_protocol.py --mutations`: pass, 1,056 s,
  65 results, owner composition 566 verified and 0 errors,
  `source_closure_complete`, no reused prerequisite; bindings 480 (91 state
  writes, 10 lends, 132 constructor fields, 181 unsafe callers), unchanged.
- `python3 scripts/test_verify.py`: 56 OK.

Not run: `scripts/source_certificate.py` (any stage), the other groups of
`verify_source.py` (their harness texts did not change; `vec-element-loops`
was last run in step 8), `scripts/verify_panic_paths.py`,
`scripts/verify_wrapping_memory.py`. The certificate stays incomplete: 1,023
rows are `premise-stated-not-proved`.

## Unit 4: VecDeque

The design report (`target/verification/i16/designs/unit-4-no-product-change.md`,
local) was written from reading only. Its finding 1 (`try_shrink_to` left
`head == cap` for an empty ring) was fixed with an owner-approved product
change in `09e9247`; since then every head store keeps `cap > 0 => head < cap`
by the geometry kernels (`retreat`, `allocated_physical`, `physical`,
`grown_head` all ensure `out < cap`), and the shrink success path stores 0
when the ring moved or is empty. The steps below follow the design's
section 6 (B to E).

**The invariant** (`closure_invariant` in `src/deque/closure_proofs.rs`, read
from the owner's words):
`len <= cap && cap <= limit && layout::array::<T>(cap).is_ok() &&
(if size_of::<T>() == 0 || cap == 0 { head == 0 } else { head < cap })`.
At `T = ()` it is `len <= cap <= limit && head == 0`. The VecDeque struct
literals are `new`, `empty`, `from_vec` and `clone_to`; its words are
`head`, `len`, `cap` and `limit` (`ptr` is not an invariant word; `limit` is
never assigned).

### Step B: closure harnesses without a new loop invariant

Done on 2026-11-28. No product file changed: only `src/deque/proofs.rs` (one
`#[path = "closure_proofs.rs"] mod closure;`) and the new proof file
`src/deque/closure_proofs.rs`; no contract changed status.

**Fixtures.** `closure_deque::<T, P, L>` builds an owner of zero-sized elements
from arbitrary `cap`, `head` and `len` words under the given policy and limit
and assumes `closure_invariant` and nothing else. `closure_vec_owner` builds a
`Vec` of zero-sized elements with `Vec::from_raw_parts` from arbitrary length
and capacity words and assumes only the Vec invariant
`len() <= capacity() <= max_len()` (the reviewed one-expression getters of
`src/vec.rs`); its owner is Vec. `closure_exit` asserts the invariant and the
unchanged limit.

| Harness (`deque::proofs::closure::`) | Sites run | Instantiation | Solver time |
| --- | --- | --- | ---: |
| `closure_empty_constructors_establish_the_invariant` | literals of `new`, `empty` (`new_in`, `with_limit`, `with_limit_in`, `Default`, `TryDefault`) | `()`, `u8`, `u64`, `[u64; 200]`, a 64-aligned type | 3.8 s |
| `closure_capacity_constructors_establish_the_invariant` | `empty` through the four capacity constructors | `()` | 2.2 s |
| `closure_reserve_preserves_the_invariant` | `grow`: `head`, `cap` (`try_reserve`, `try_reserve_exact`, every `additional`) | `()` | 11.1 s |
| `closure_push_preserves_the_invariant` | `try_push_back`, `try_push_front` and the `_mut` forms | `()` | 31.2 s |
| `closure_pop_preserves_the_invariant` | `pop_front`, `pop_back`, `try_pop_*_if`, `swap_remove_*` | `()` | 7.6 s |
| `closure_truncate_preserves_the_invariant` | `truncate`, `clear` | `()` | 0.9 s |
| `closure_make_contiguous_preserves_the_invariant` | `make_contiguous` | `()` | 1.5 s |
| `closure_shrink_preserves_the_invariant_of_a_unit_owner` | `try_shrink_to`, `try_shrink_to_fit` (return before any write) | `()` | 0.9 s |
| `closure_insert_preserves_the_invariant` | `try_insert`, `try_insert_mut` (and `grow`) | `()` | 13.1 s |
| `closure_remove_preserves_the_invariant` | `remove` | `()` | 1.1 s |
| `closure_rotate_preserves_the_invariant` | `rotate_left`, `rotate_right` | `()` | 1.0 s |
| `closure_append_preserves_the_invariant_of_both_owners` | `try_append` (two owners, own limits) | `()` | 5.4 s |
| `closure_split_off_preserves_the_invariant_of_both_owners` | `try_split_off` (the fresh `result` from `empty`) | `()` | 2.8 s |
| `closure_drain_preserves_the_invariant` | `drain`, `Drain::drop` (`Finish`); any bounds; forget, `next`, `next_back` or drop | `()`, `kani::unwind(2)` | 8.6 s |
| `closure_clone_establishes_the_invariant_of_the_copy` | literal and store of `clone_to` (bitwise path) | `()` | 2.4 s |
| `closure_clone_from_preserves_the_invariant_of_both_owners` | bitwise path of `try_clone_from` | `()` | 8.1 s |
| `closure_extend_copy_preserves_the_invariant` | `try_extend_copy`, a slice of every length | `()` | 5.8 s |
| `closure_resize_preserves_the_invariant` | bitwise branch of `try_resize` | `()` | 5.7 s |
| `closure_from_vec_establishes_the_invariant` | literal of `from_vec`, from `closure_vec_owner` | `()` | 0.6 s |
| `closure_into_vec_establishes_the_vec_invariant` | `into_vec` calls `Vec::from_raw_parts` (V1); asserts the Vec invariant of the result | `()` | 2.9 s |
| `closure_adopting_constructor_establishes_the_invariant_for_unit_storage` | `try_from_std_in` calls `Vec::from_raw_parts` (V2), then `from_vec` | `()` | 3.0 s |
| `sized::closure_capacity_constructors_establish_the_invariant_of_every_sized_owner` | `grow` from an empty owner (`old == 0`, no copy) through `try_with_capacity_in`, `try_with_capacity_and_limit_in`; `storage::reallocate` and `storage::deallocate` stubbed | `u8`, `u64`, `[u64; 200]`, the aligned type | 20.8 s |

`()` has a copy witness (`scalar!` in `src/clone.rs`), so `clone_to`,
`try_clone_from`, `try_resize` and `try_extend_copy` take their bitwise paths
at this instantiation. std's `VecDeque::from(Vec<()>)`, std's
`make_contiguous` at a zero-sized element (it sets `head = 0`) and
`Vec::from(VecDeque<()>)` ran without unwinding bounds. Covers satisfied:
growth and refusal of the reserve and push harnesses, a nonempty pop and drain,
a nonempty into_vec and from_vec, acceptance and refusal of the adopting
constructor; the refusal cover of the adopting constructor under `Unlimited`
is unreachable (`length > usize::MAX`).

**Not run by a harness of this step.** The retain loop store (`Repair::drop`,
W30, step C); `IntoIter::fold` (`Retire::drop`, step D: unobservable); the
non-bitwise paths of `clone_to` and `try_clone_from`, which go through
`into_vec`, Vec's own writers and `from_vec` (each a site of its own); the
loops of `try_resize_with` and `try_extend` over `try_push_back` (no direct
store); every sized ring path that touches storage; `needs_drop` paths and
metered policies.

**Registration.** `scripts/verify_source.py`: a new group `deque-closure`
(loop contracts on, as every group), 21 harnesses in its main invocation and
the sized harness in an `EXTRA` invocation (`-Z stubbing`,
`--cfg jzap_closure_stubs`), and 22 controls, one per harness. Each control
mutates one product statement in a disposable copy and must end in a solver
counterexample of the named harness: the literal of `empty` or of `from_vec`
stores head 1; the capacity constructor stores head 1; `grow` takes
`needed - 1` (or, for the sized harness, `needed + 1`) as the exact capacity;
the stores of push_back, pop_back, truncate, insert, remove, append,
split_off, `Finish::drop` (`end + tail`), clone_to, try_clone_from,
extend_copy and try_resize publish a wrong length; make_contiguous returns a
slice of `cap` elements; try_shrink_to drops its zero-size return;
rotate_left refuses `n == len`; into_vec passes `len + 1`; try_from_std_in
drops its `source.len() > max` refusal. `scripts/source_certificate.py`:
`deque-closure` is in `FOUNDATION_GROUPS`, `FOUNDATION_CONTROLS = 166`.
`scripts/verify.py`: the deque area's Kani job selects the nine model
harnesses by exact name, so `deque::proofs::closure::` does not change its
selection or count (as `scripts/verify_deque.py`, which now passes `--exact`
and the nine names); `deque-closure` is in the direct-source plans of the
areas `deque`, `foundation` and `all`. Unit tests in `scripts/test_verify.py`
keep the two harness lists equal and the group in those plans.

**Run** (Kani 0.68.0, CBMC 6.11.0, CaDiCaL, `--object-bits 10`; each on a
copy of this tree):

- `python3 scripts/verify_source.py --group deque-closure --mutations`: pass,
  864 s; 21 of 21 and 1 of 1 harnesses verified, 22 of 22 controls detected.
- `python3 scripts/verify_deque.py --kani --mutations`: pass, 682 s;
  9 of 9 Kani harnesses (`--exact`), 11 compiled mutations, 48 caller
  controls, maintained callers 388 verified and 0 errors.
- `python3 scripts/test_verify.py`: 64 OK.
- `python3 scripts/owner_closure.py --controls`: unchanged; Buffer closed, Vec
  not closed (the two `other-unit:4` calls), 4 of 4 controls detected. The
  script does not know VecDeque yet (step D).
- `cargo fmt --all --check`: clean.

Peak memory was not measured. Not run in this step:
`scripts/verify_owner_protocol.py`, `scripts/verify_wrapping_memory.py`, the
Miri deque checks, `scripts/source_certificate.py`.


### Step C: the retain loop invariant does not verify without a product addition

Run on 2026-11-28; not committed to the product. No product file changed in
this step, so no machine-code comparison was made; `src/deque/edit.rs`, the
`deque_callers.py` body pin, the `verify_deque.py` review hash and
`tools/panic-audit` are as at step B. The store of `Repair::drop` (W30,
`src/deque/edit.rs:249`) stays without a harness.

**What was run.** In a copy of the tree, the design's attribute on the
`while let` of `try_retain_mut`:

```
#[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(guard.state.valid_for(guard.owner.len()) && guard.owner.len() <= guard.owner.capacity() && guard.owner.capacity() <= guard.owner.max_len() && guard.owner.max_len() == on_entry(guard.owner.max_len())))]
```

(accepted by the extended panic-audit grammar of that copy, which reviewed
`VecDeque::len`, `capacity` and the non-inline `max_len` as exact texts), and a
harness `closure_retain_preserves_the_invariant`: a `closure_deque` owner of
every length under both limit policies, a callback that keeps, removes or
fails on every call, `try_retain_mut` and `try_retain`, the invariant asserted
on every return. Kani 0.68.0, CBMC 6.11.0, CaDiCaL, `--object-bits 10`,
`--cfg jzap_loop_contracts -Z loop-contracts`: **verification failed** after
32 s; the four failed checks are "null reference produced" at
`src/deque/edit.rs:258` (`&mut *p`, the callback's argument, with `p` from
`guard.owner.slot(read)`), one per instantiation.

**Cause, isolated.** `goto-instrument --show-goto-functions` on the kept
program lists the loop's inferred write set: `guard.state.read` and
`guard.state.write`, and, for each of five pointers to the owner (`&*self` and
four pointers loaded from `guard.owner`), the
owner pointer cast to the `Compaction` type with the fields `read` (offset 0)
and `write` (offset 8), eight bytes each. Under this toolchain
`VecDeque<(), Unmetered, L>` has `ptr` at offset 0, `cap` at 8, `head` at 16,
`len` at 24 and `limit` at 32 (`mem::offset_of!`), and `Compaction` has `read`
at 0 and `write` at 8. So the contract havocs `ptr` and `cap` of the owner at
loop entry. The invariant restates `cap` (`len <= cap <= max_len`); nothing
restates `ptr`, and the havocked pointer may be null.

| Variant (each in a disposable copy) | Result |
| --- | --- |
| the design's invariant (above) | failed: null reference at `edit.rs:258`, 32 s |
| `guard.state.valid_for(on_entry(guard.state.remaining()))`, which reads no owner word | failed: the same write-set targets (from `&*self`), the null reference and the exit assertion `closure_invariant` |
| the design's invariant `&& !guard.owner.probe_ptr().is_null()`, with a temporary `#[cfg(kani)] pub(crate) fn probe_ptr(&self) -> *const T { self.ptr.as_ptr() }` added to `src/deque.rs` | verified, 31 s |

The targets come from the loop body, not from the invariant's form, and the
one fact missing is that `ptr` is not null. VecDeque has no one-expression
getter of `ptr`: its `&self` methods that read it (`as_slices`, `iter`, `get`,
`front`, ...) compute through the ring geometry and slice constructors. The
only fix found is a new `cfg(kani)` getter in a product file, which this step
does not make (owner instruction for unit 4); it is put to the owner. With a
getter, the predicate would be the design's with `!guard.owner.<getter>().is_null()`
added, as for `try_extend_fallible` in unit 1 step 8.

The harness and the attribute are kept locally
(`target/verification/i16/unit4/c/kept/`); the logs and the write-set listing
are in `target/verification/i16/unit4/c/`.

### Step D: the closure check for VecDeque and the joint closure

Done on 2026-11-28. No product file changed.

**`scripts/owner_closure.py`.**

- `OWNERS['VecDeque']`: struct `src/deque.rs`, words `head`, `len`, `cap`,
  `limit`, the invariant text of `closure_invariant` pinned. `owner_file` and
  `check_privacy` cover `src/deque.rs` and `src/deque/`. The struct-literal
  and store checks of `admissible` include `VecDeque` and `head`.
- A borrow or pattern binding of a VecDeque word in the deque files fails
  (`caller_bindings` refuses `len`, `cap` and `limit` outside the foundation
  area; `head` is checked here).
- `HELPERS` adds `closure_deque` (assumes `closure_invariant`) and
  `closure_vec_owner` (assumes the Vec invariant through the getters); the
  three Vec getters it reads are pinned as whole definitions of `src/vec.rs`.
  `HELPER_OWNERS` names the owner each helper assumes.
- **Joint closure.** A harness counts for a site of owner O only if every other
  owner whose helper it uses is closed (a harness may assume O's own
  invariant: the inductive step). The closed set is the greatest fixpoint,
  starting from all owners.
- **Dead stores.** 15 stores cannot execute at `T = ()`. Each is mapped to the
  harness of its function with a `dead` guard that the script checks in the
  comment-stripped body: `enclosed` (every occurrence inside
  `if mem::size_of::<T>() != 0 {`, or inside `if kind != 0 {` /
  `if self.head.get() != 0 {` for `make_contiguous` and `normalize`, where
  `head == 0` at `T = ()` makes the ring one segment) or `return` (an
  `if .. mem::size_of::<T>() == 0 .. { return ..; }` at the top level of the
  body precedes the store: `try_shrink_to`, `rotate_left`, `rotate_right`).
  The output marks them `[not executed at T = ()]`: no mutation of such a
  store is detectable at this instantiation.
- **`Retire::drop`** (both stores): `unobservable`, pinned by
  `check_retire_fold`: `IntoIter::fold` takes `mut self`, uses no `self` after
  `let mut guard = Retire { owner: &mut self.owner, .. }`, and ends with
  `b.iter().fold(value, consume)`.
- **V1** (`into_vec`) is mapped to `closure_into_vec_establishes_the_vec_invariant`
  (helper of VecDeque); **V2** (`try_from_std_in`) to the adopting harness
  with `observed='from_vec'`: the script checks that the Vec built at the call
  is handed only to `Ok(Self::from_vec(owner))` and pins the bodies of
  `VecDeque::from_vec` and `Vec::into_raw_parts`, which copy `len`, `cap` and
  `limit` verbatim.
- **The `from_vec` literal** is mapped to `closure_from_vec_establishes_the_invariant`
  only (helper of Vec): `from_vec` is reached with every Vec state
  (`try_from_array_in`, `clone_to`, `try_clone_in`, `try_from_std_in`).
- **W30** (`Repair::drop`) is `uncovered` (step C).

**Output** of `python3 scripts/owner_closure.py --controls`:

| Owner | Closed at `T = ()` | Sites | Dispositions | Blocking |
| --- | --- | ---: | --- | --- |
| `Buffer` | yes | 8 | 8 harness | none |
| `Vec` | **no** | 54 | 48 harness, 2 unobservable, 2 not-instantiable, 1 trust-boundary, 1 uncovered | `src/deque.rs:522` `into_vec` calls `Vec::from_raw_parts`: its harness assumes the VecDeque invariant, and VecDeque is not closed |
| `VecDeque` | **no** | 44 | 40 harness (15 of them not executed at `T = ()`), 2 unobservable, 2 uncovered | `src/deque/edit.rs:249` `Repair::drop` (no harness, step C); `src/deque.rs:509` the `from_vec` literal: its harness assumes the Vec invariant, and Vec is not closed |

The VecDeque sites with a harness: the literals of `new`, `empty` and
`clone_to`; the stores of `grow` (`head`, `cap`), `try_push_back`,
`try_push_front` (`len`; `head` dead), `pop_front` (`len`; `head` dead),
`pop_back`, `truncate`, `make_contiguous` (4, dead), `normalize` (dead),
`try_shrink_to` (`cap` and two `head`, dead), `try_insert_mut` (`len`;
`head` dead), `remove` (`len`; `head` dead), `rotate_left`, `rotate_right`
(dead), `try_append` (2), `try_split_off` (2, the fresh `result` pinned),
`drain`, `Finish::drop` (`len`; `head` dead), `clone_to`, `try_clone_from`
(3), `try_extend_copy` (2), `try_resize`. Sized capacity path:
`sized::closure_capacity_constructors_establish_the_invariant_of_every_sized_owner`
for the `empty` literal and the `cap` store of `grow`.

**Controls**, each detected (8 of 8): the four of step 9; an unmapped second
`self.len = Checked::new(at)` in `try_split_off` (the script fails); the
product table (W30 uncovered: both not closed). Two controls use a stand-in
table in which W30 alone is mapped to an unrelated VecDeque harness, where
both owners close (asserted before the controls run): a `kani::assume` added
to the into_vec harness, and one added to the from_vec harness; each leaves
both owners not closed. Unit tests (`scripts/test_verify.py`): the product
result above; the joint fixpoint on the stand-in table, and both opened again
by an `other-unit` entry for the into_vec call; the checks of a dead guard
(dropped `if`, dropped early return), of `IntoIter::fold` (a use of `self`
after the guard), of `from_vec` (a changed word) and of a Vec getter (changed
body) each fail.

**Run.** `python3 scripts/owner_closure.py --controls`: the output above,
8 of 8 controls detected. `python3 scripts/test_verify.py`: 66 OK.
`cargo fmt --all --check`: clean (no Rust file changed in this step).

**Step E is not applied.** Vec is not closed, so no Vec contract moves to
`instantiated-kani` and `scripts/premise_status.py` is unchanged (below).

### Unit 4 state after steps B to D

Checks on the tree of step D (`58fab39`), each run alone:

- `python3 scripts/verify_owner_protocol.py --mutations`: pass, 1,346 s,
  65 results, owner composition 566 verified and 0 errors,
  `source_closure_complete`, no reused prerequisite, 26 actual-Rust and 3
  staging harnesses; bindings 480 (91 state writes, 10 lends, 132 constructor
  fields, 181 unsafe callers), unchanged: deque rows are filtered there.
- `python3 scripts/verify_wrapping_memory.py --mutations`: pass, 692 s,
  9 controls detected, 53 modular calls (26 established, 27 stated), as before.
- `python3 scripts/verify.py --area deque --check miri --check miri-tree --check miri-32`:
  pass, 20 of 20 tests in each configuration.

An earlier attempt ran the two scripts above at the same time; the owner
protocol starts `verify_wrapping_memory.py` itself, the two runs collided on
the lock of `verify_source.py`, both were stopped and rerun one after the
other. Not run: `scripts/source_certificate.py`, `scripts/verify_panic_paths.py`,
`cargo test` and clippy (no product file and no non-proof Rust file changed in
unit 4).

**Step E (contract statuses) is not applied.** Vec and VecDeque are not
closed (step D), so no Vec contract becomes `instantiated-kani` and
`scripts/premise_status.py` is unchanged. No deque contract qualifies in any
case: every maintained deque `requires` adds facts of its operation
(`index < len`, `len < cap`, `raw_valid`, ...) to the ring, so none is exactly
an owner invariant. The certificate's row counts were not recomputed: the
premise table, the binding lists and Buffer's closure are the same as at
step 9 (17 `instantiated-kani`, 1,023 `premise-stated-not-proved` in that
computation). `c_keep_live` and `c_remove_live` stay stated: the design's
section 5 needs an owner decision between a stub of the two compaction
transitions and a requires-free restatement.

**What closes the unit.** One owner decision: a `cfg(kani)` getter of the
deque pointer in `src/deque.rs` (as `Compaction::valid_for` is a
`cfg(kani)` method in a product file), so that the retain loop invariant can
restate `!ptr.is_null()` (step C, verified in a disposable copy). With it, W30
has its harness and the joint closure closes VecDeque and Vec together;
step E then applies.

### Step C, completed: the retain loop contract with the approved pointer getter

Owner decision, 2026-11-28: a `cfg(kani)`-only getter of the deque pointer is
approved as a product addition (the pattern of `Compaction::valid_for`).
Done on 2026-11-28. The whole non-proof product change:

```
// src/deque.rs, after VecDeque::max_len
    #[cfg(kani)]
    fn probe_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }
// src/deque/edit.rs, on the while-let of try_retain_mut
        #[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(guard.state.valid_for(guard.owner.len()) && guard.owner.len() <= guard.owner.capacity() && guard.owner.capacity() <= guard.owner.max_len() && guard.owner.max_len() == on_entry(guard.owner.max_len()) && !guard.owner.probe_ptr().is_null()))]
```

`probe_ptr` is private (the call site `deque::edit` is a child module).

**Machine code.** The five benchmark executables (`deque`,
`jzap-throughput-analysis`, `string`, `binary_heap`, `scratch_slice` of
`tools/throughput-analysis`) built with `RUSTFLAGS="-C symbol-mangling-version=v0"`
from `git archive 660a180` and from an export of this tree, in two
directories of equal path length: 691, 834, 706, 645 and 573 function bodies,
all identical by `tools/perf-audit/codegen_diff.py` (0 differing, 0 present in
one build only) and as multisets of normalized bodies. Rebuilt in the same
directory, the five files are byte-identical.

**Harness** `deque::proofs::closure::loops::closure_retain_preserves_the_invariant`
(module under `cfg(jzap_loop_contracts)`): a `closure_deque` owner of every
length under both limit policies, a callback that keeps, removes or fails on
every call and no bound on the turns; `try_retain_mut` and `try_retain`; the
invariant and the unchanged limit asserted on every return. Verified in
29.8 s (alone) and 32.2 s (in the group); the base and step checks of the
loop contract are in the log; six covers satisfied (a strict shrink to a
nonempty length, a callback error with length above 1, `try_retain` success,
for each policy). Control `closure-deque-retain-length`
(`restored_len() + 1`) detected.

**Pins.** `scripts/deque_callers.py` (body of `try_retain_mut`),
`scripts/verify_deque.py` (`REVIEWED` hashes of `src/deque.rs` and
`src/deque/edit.rs`), `tools/panic-audit/src/main.rs`: `OWNER_INVARIANTS`
lists the predicate for `src/deque/edit.rs::try_retain_mut` as exact text;
`OWNER_GETTERS` now carries the owner type and the full attribute and
visibility text, and reviews `VecDeque::len`, `capacity`, `max_len` (without
`#[inline]`) and `probe_ptr` (`#[cfg(kani)]`, private) as whole definitions;
a unit test refuses changed bodies, a `pub` or missing `cfg(kani)` on
`probe_ptr` and an `#[inline]` on `max_len`. `scripts/source_certificate.py`:
`FOUNDATION_CONTROLS = 167`.

**`scripts/owner_closure.py`.** W30 maps to the retain harness. Controls (9
of 9 detected): the four of step 9; an unmapped deque write; a
`kani::assume` added to the retain, the into_vec or the from_vec harness
(each: neither owner closed); W30 without a harness (neither closed). The
stand-in table of step D is removed.

| Owner | Closed at `T = ()` | Dispositions | Uses the invariant of |
| --- | --- | --- | --- |
| `Buffer` | yes | 8 harness | — |
| `Vec` | **yes** | 49 harness, 2 unobservable, 2 not-instantiable, 1 trust-boundary | VecDeque (the into_vec harness) |
| `VecDeque` | **yes** | 42 harness (15 not executed at `T = ()`), 2 unobservable | Vec (the from_vec harness) |

**Run**, one at a time on this tree:

- `python3 scripts/verify_source.py --group deque-closure --mutations`: pass,
  1,142 s; 22 of 22 and 1 of 1 harnesses verified, 23 of 23 controls detected.
- `python3 scripts/verify_deque.py --kani --mutations`: pass, 663 s; 9 Kani
  harnesses, 11 compiled mutations, 48 caller controls, maintained callers
  388 verified and 0 errors.
- `cargo test --locked --offline --features std`: 462 passed, 0 failed.
  `python3 scripts/verify.py --area deque --check clippy` (both feature sets,
  `-D warnings`): pass. `cargo fmt --all --check`: clean. `cargo test` of
  `tools/panic-audit`: 6 passed.
- `python3 scripts/owner_closure.py --controls`: the table above, 9 of 9.
  `python3 scripts/test_verify.py`: 66 OK.

### Step E: Vec contracts on the jointly closed owner

Done on 2026-11-28. With Vec and VecDeque closed (step C, completed), eight
Vec contracts move to `instantiated-kani` in `scripts/premise_status.py`.
`OWNER_INVARIANT['Vec']` accepts the Vec invariant in two exact texts, the
form the maintained function uses: `owner_valid({s},size)` and its expansion
`{len}<={cap}<={limit},byte_valid({cap},size)` (`form='expanded'`).
`maintained_requires` now also reads a `requires` clause that is followed by
the body instead of `ensures` (`internal_copy_before_append` has none).
`check_instantiated` on every binding row of the owner protocol and of the
area summaries accepts all twelve contracts (each row in `src/vec*.rs` inside
`Vec<T, P, L>`).

| Contract | Maintained `requires` | Rows | Functions |
| --- | --- | ---: | --- |
| `checked_owner_length` | `owner_valid(s,size)` | 1 | `try_set_len` |
| `split_owner` | `owner_valid(original,size)` | 3 | `try_split_off` |
| `shortened_owner` | `owner_valid(s,size)` | 4 | `pop`, `truncate` |
| `owner_to_array` | `owner_valid(s,size)` | 2 | `try_into_array` |
| `memory_reserve` | `len<=cap<=limit,byte_valid(cap,size)` | 2 | `grow`, `reserve_length` |
| `insertion_before_moves` | the same | 2 | `try_insert` |
| `internal_copy_before_append` | the same | 2 | `try_extend_copy_from_within` |
| `drain_constructor` | the same | 5 | `try_drain` |

Instantiations recorded with the rows: `T = ()` for every Vec method and lend
(`Unmetered`, one exchange fixture with `Budget::Limited`), jointly with
VecDeque; `[(); 1]` and `[(); N]` for the bitwise extension and
`try_into_flattened`; `u8`, `u64`, `[u64; 200]` and a 64-aligned type for
the capacity paths with the storage functions stubbed. The byte bound of the
maintained `requires` is vacuous at `T = ()` and asserted by the sized
capacity harnesses only.

Considered and left stated: `commit_reserved_owner` (a row in
`src/clone.rs`, outside `Vec<T, P, L>`); `append_owner`, `exchanged_owners`
(two owner invariants; `check_instantiated` compares one);
`raw_parts_owner` (its rows are the literal of the public unsafe
`Vec::from_raw_parts`, whose premise for external callers is the
public-unsafe contract, not the closure); `clone_within_operation`,
`owned_resize_operation` (`len<=cap<=limit` without the byte bound: not the
exact text); `initialized_operation`, `complete_splice` (the invariant plus
`call_requires`; not moved without an owner decision);
`reservation_before_bulk_write` (Buffer rows), `extend_operation` and
`vectored_owner` (`Vec<u8>` rows), `resized_owner` and `adopted_owner`
(further conjuncts); every deque contract (each adds operation facts to
`ring`). `c_keep_live` and `c_remove_live` stay stated (design section 5,
owner decision).

**Control.** A unit test (`scripts/test_verify.py`) makes the expanded
`requires` of `memory_reserve` weaker (`len <= cap, byte_valid(cap, size)`)
and stronger (an added `len > 0`): `check_instantiated` refuses both and
accepts the original; a `drain_constructor` row in `src/clone.rs` or in
`Vec<u8, ..>` is refused; each Vec contract fails when Vec is not closed.

**Row counts**, computed as in step 9 (`counts.py`, local copy
`target/verification/i16/unit4/e/counts.py`; before with `premise_status.py`
of `660a180` and Buffer's closure, after with this tree and the joint
closure; owner rows from the fresh owner-protocol run below; area rows from
the area summaries on disk: the deque summary from the step C run, the others
from bindings-only runs of 2026-11-27):

| Disposition | Before | After |
| --- | ---: | ---: |
| solver-and-source-connected | 105 | 105 |
| instantiated-kani | 17 | 38 |
| trust-boundary | 0 | 0 |
| premise-stated-not-proved | 1,038 | 1,017 |
| unproved | 21 | 21 |
| total | 1,181 | 1,181 |

The 21 moved rows are exactly the rows of the eight contracts above. Against
step 9 (1,023 stated, 36 unproved) the before column differs by 15 rows that
moved from `unproved` to stated: the deque summary on disk now lists its nine
Kani harnesses as verified (the bindings-only artifact noted in step 9). The
certificate itself was not run.

**Run.**

- `python3 scripts/verify_owner_protocol.py --mutations` on the tree of
  step C plus this change: pass, 1,395 s, 65 results, owner composition 566
  verified and 0 errors, `source_closure_complete`, no reused prerequisite;
  bindings 480 (91, 10, 132, 181), unchanged.
- `python3 scripts/test_verify.py`: 66 OK. `cargo fmt --all --check`: clean.

Not run: `scripts/source_certificate.py`, `scripts/verify_panic_paths.py`,
`scripts/verify_wrapping_memory.py` and Miri after steps C and E (run on
`58fab39`; step C changed only `cfg(kani)` code, and the machine code of the
five executables is identical).

