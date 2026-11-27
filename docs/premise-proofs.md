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
| 1 | Vec and Buffer owner | 59 | `len <= cap <= limit`, `byte_valid(cap)`; reservation and range results; Drain/hole and guard states tied to the owner | First attempt reverted (it changed the product). Under the constraint: step 1 done (contracts restated, no row closed); step 2 checked (no row closed: both candidates rest on a `requires` of the enclosing function); step 3 done (scanner pins borrows and pattern bindings, 10 rows bound, none closed); step 4 done (Buffer closure harnesses verified and registered; no contract changed status); step 5 done except L11 (Vec closure harnesses for the writers without a new loop invariant; the existing loop contract of `try_resize_with_init` does not verify here); step 6 done (sized capacity paths with stubbed storage, `[(); 1]`, a failing clone with bounded counts, `Capacity::for_slice`); steps 7 to 10 below |
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
