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
| 1 | Vec and Buffer owner | 58 | `len <= cap <= limit`, `byte_valid(cap)`; reservation and range results; Drain/hole and guard states tied to the owner | Steps 1-3 done for `Vec`; step 4 measured except the paired timing (below); steps 5-6 and `Buffer` not started |
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

### Status of steps 1-4 (2026-11-26)

`Vec` holds `ptr`, `extent`, `policy` and the marker. `src/extent.rs` is the
closed state; `src/capacity.rs` carries the byte bound. `premise_status.py` is
unchanged: no contract is rebound yet (step 5).

#### The state and its transitions

Type invariants (Verus, `--no-cheating`, generic over `T` and the sealed `L`):
`Extent`: `len <= cap <= limit.bound()`; `Capacity`: a sized element's count
times its size is at most `isize::MAX`. A zero-sized element is not required
to have `cap == limit`: the actual-Rust harnesses use unit owners with a
capacity below the limit to cover every numeric state.

| Function | Result | Refuses or clamps |
| --- | --- | --- |
| `Extent::unlimited()` (const), `Extent::empty(limit)` | length 0; capacity 0, or the limit for a zero-sized element | nothing |
| `Extent::try_new(len, capacity, limit)` | the three words | `Capacity` when `len > capacity` or `capacity > limit` |
| `Extent::unlimited_clamped(len, capacity)` | no limit; length `min(len, capacity)` | clamps the length |
| `Extent::from_raw_parts(len, capacity, limit)` (`unsafe`) | the three words | trusted entry: requires `len <= capacity <= limit` |
| `len`, `length`, `cap`, `capacity`, `limit`, `max_len` | the words | read only |
| `set_len_within(needed)` | length `needed` | `Capacity` when `needed > cap`; nothing changes |
| `set_len_clamped(next)` | length `min(next, cap)` | clamps (drop guards) |
| `shorten_to(next)` | length `min(len, next)` | never lengthens |
| `holding(needed, exact)` | a new state: same length and limit, capacity from the growth rule (`needed` when exact) | `Capacity` when `needed < len` or `needed > limit`; `Layout` beyond the address space |
| `resized(capacity)` | a new state over `capacity` | `Capacity` when `capacity < len` or `capacity > limit` |
| `Capacity::empty`, `Capacity::admitted` (checked), `Capacity::get` | the count | `Layout` beyond the address space |
| `Capacity::from_validated_raw(count)` (`unsafe`) | the count | trusted entry: requires the byte bound |

`LengthPolicy::max_len` returns `Sealed::bound()` for both policies.
`growth_for_layout` no longer requires `current < needed`.

`holding` and `resized` return a new state. The owner obtains it before it
replaces the storage and assigns it afterwards, so nothing can refuse after an
allocation. `Capacity::try_new` (checked, with the layout) and
`Capacity::for_slice` (slice validity) have no contract and construct only
through `admitted` and the trusted entry; their bodies are pinned.

#### Rules at the call sites

- The length is admitted (`set_len_within`) before a single move or a bitwise
  copy, which cannot unwind. A refusal changes nothing and returns or drops the
  input like the other refusals of the function.
- `push_within_capacity` writes the value before it stores the length, as
  before, and takes the value back out of the slot on a refusal.
- Element initialization through a callback publishes after each element;
  `try_resize_with_init` compares the reservation with the capacity once
  before its loop.
- Shortening uses `shorten_to`. Drop guards that restore a length use
  `set_len_clamped` (`ExtractIf`, `Drain`) or `shorten_to` (`Compact`).
- The length lent to `init::collection::fill` is a copy (`LentLength`,
  `src/vec/edit.rs`) published through `set_len_clamped` when the borrower
  returns or unwinds.

Public unsafe entries: `Vec::from_raw_parts` passes its arguments to the two
trusted entries, unchecked as before. `Vec::try_set_len` is `set_len_within`
alone; it refuses a length above the capacity with `Error::Capacity` as
before.

#### Checks

| Check | Result |
| --- | --- |
| Direct Verus target `vec-extent` | 310 obligations verified; 19 source mutation controls detected |
| Declaration check `scripts/verify_owner_extent.py` | 22 functions with a contract, 6 pinned without one; 12 compiled controls rejected |
| Actual-Rust harnesses `extent::proofs::*` (Kani 0.68, every word state and argument at unit storage) | 2 verified |
| Direct-source group `owner-extent` (`scripts/verify_source.py`; a `direct-foundation` group of the certificate) | passes with its controls |
| 13 other direct Verus targets on the changed crate (`vec-layout-growth`, `vec-geometry`, `owner-metadata`, `vec-queries`, `vec-cursors`, `layout`, `vec-flatten`, `vec-staging`, `vec-compaction`, `vec-extraction`, `init-prefix`, `buffer`, `string-cursors`) | pass, 310 to 317 obligations each; their 54 controls detected |
| 18 direct-source groups, Verus and Kani (`owner-extent`, `vec-queries`, `owner-metadata`, `vec-cursors`, `vec-layout-growth`, `vec-geometry`, `vec-conversion`, `layout`, `vec-staging`, `vec-splice-fill`, `vec-extraction`, `vec-extraction-values`, `vec-compaction`, `vec-compaction-values`, `string-cursors`, `buffer`, `init-prefix`, `state`) | pass; run again with their Kani mutation controls: all but `vec-extraction`, `vec-compaction`, `string-cursors`, `buffer`, `init-prefix` and `state` |
| `cargo test --all-features`; Miri, strict provenance, `--test vec` | 460 passed; 41 passed |
| `scripts/verify_owner_protocol.py --mutations` (fresh prerequisites: wrapping memory with 19 and 4 Kani harnesses, pointer checks, staging group) | pass: 585 obligations of the composed model, 53 controls detected, 469 source bindings, source closure complete |
| `scripts/verify_string.py --mutations --kani`; `scripts/verify_panic_state.py`; `scripts/check_unsafe_inventory.py`; `scripts/check_memory_controls.py` | pass |
| The five `Vec` entries of `scripts/mutation_check.py`, run alone | detected; the script itself stops at its first entry (`budget_release`), whose anchor is already absent at `5dadd51` |

The scripts that pin `Vec` source text name the new text. Every transition
call is bound, in source order, to the contract its assignment was bound to
(`caller_bindings.py`, `TRANSITIONS`); the scanner emits one
`extent_transition` row per call. Source bindings of the owner protocol
(without solvers): 469 (97 publications, 122 constructor fields, 184 unsafe
callers); before: 470 (91, 132, 181). The differences are the two fewer fields
of each `Vec` constructor, the removed `Capacity::for_slice` literal, the
lent length, the growth and resize transitions, and three unsafe blocks
(`for_slice`, `holding`, the push read-back). The unsafe census is 611 blocks
and 119 unsafe functions (before: 608 and 118).

Bindings whose former site no longer exists: the `required_len(0, new_len,
cap)` admission of `try_set_len` (the transition is the check), and the
`boxed_to_owner` field rows of `Capacity::for_slice`. The writes and
constructor fields inside `src/extent.rs` are closed by the `owner-extent`
group and are not rows of the owner-protocol bindings.

Three Kani controls changed their defect because the transition now refuses
the former one: `push-capacity-argument` became `push-length-publication`,
`owner-length-capacity-argument` became `owner-length-argument`, and
`owner-adoption-length-argument` adopts a wrong length instead of omitting
the length check.

#### Machine code and executed instructions

Base `5dadd51`, toolchain 1.95.0, `-C symbol-mangling-version=v0`,
`tools/throughput-analysis` release profile. Executed instructions are counted
with Callgrind over four timed epochs (five-epoch run minus one-epoch run,
seed 617, `tools/throughput-analysis/instruction_counts.py` method) for every
`Vec`, `String`, `BinaryHeap` and scratch task and label: 266 conditions.
No timing was run.

| Change of executed instructions per call | Conditions |
| --- | ---: |
| within 0.5% | 235 |
| more than +0.5% | 13 |
| less than -0.5% | 18 |

Largest increases: scratch push and pop of 256-byte values with a local or
concurrent budget, +4.33% (+836 of 19,294); `BinaryHeap` reserved push of
256-byte values with a local budget, +1.44%; the small `Vec` profile with a
metered policy (`clone`, `pop`, `extract_all`), +0.8% to +1.4% (+4 to +7
instructions per call). Largest decreases: reserved push with a length limit,
-8.33% (one instruction per push); `String` UTF-16 conversion with a budget,
-4.74%; `extract_all` with a length limit, -3.42%. Every `Vec` task of the
mixed profile without a limit is within 0.03%.

Function-level comparison (`tools/perf-audit/codegen_diff.py`): the `btree`,
`btree_set`, `shared`, `arc_scenarios`, `bplus_partial` and `bplus_strided`
executables are identical; `deque` differs in two functions by a register
exchange with equal instruction counts (`VecDeque::grow`, from the repeated
byte admission of `Capacity::try_new`, which folds); the `Vec`, `String`,
`BinaryHeap`, scratch and scenario executables differ in the functions that
inline `Vec` operations.

Causes found in the machine code and removed:

| Form tried | Observation | Change |
| --- | --- | --- |
| `try_push` assigned the whole state returned by the cold growth | the push loop counted the length separately: +2 instructions per push (`reserved`, 202,124 to 235,809 per call) | the grown capacity is published over the owner's own length word (`resized`) |
| `push_within_capacity` stored the length before the value | `lea` and `mov` instead of `inc`: +1 instruction per push | the value is written first, as before |
| `set_len_within` after each initialized element | compare, branch and length store in the loop: +4 instructions per element (`scratch_resize_256` +20.8%) | one comparison of the reservation before the loop |
| `resized(capacity)` after `Capacity::try_new` in `grow_to` | growth no longer inlined into `String::try_from_utf16_in`; the owner stayed in memory: 48.6 instead of 42.1 instructions per character, 9.3 stack-operand instructions | `holding`: the capacity comes from the verified growth rule, without a separate comparison |
| `set_len_within` after a bitwise copy | the refusal branch kept two values live across `memcpy`; in `String` pop (limited, metered) the storage pointer was reloaded from the stack: +15.3% | the length is admitted before the copy |

A growth path with a length limit gains the comparisons the optimizer does
not fold: `grow` +75 instructions per call of 209,166 (limited), -13
(unlimited).

Differences not removed:

- Scratch push and pop of 256-byte values (local, concurrent): the loop
  saves the owner pointer around `memmove` in a callee-saved register, two
  register moves per element; no load, store, comparison or branch is added,
  and stack-operand instructions fall from 5.15 to 1.79 per element. The
  result equals the unchanged count of the unlimited label (20,130).
- `BinaryHeap` reserved push of 256-byte values (local): +2.7 instructions
  and +4.3 stack-operand instructions per element of 459.6 and 166.5; the
  opcode difference is `mov` and alignment padding only. The instruction that
  causes it was not isolated.
- The small-profile and splice differences of at most 1.4% were not analyzed
  per instruction.

The paired timing protocol of step 4 has not been run.

Not run for this change: the combined certificate
(`scripts/source_certificate.py`), the link audit, the 32-bit and Tree Borrows
Miri modes, Miri on tests other than `vec`, and the `BinaryHeap`, `VecDeque`
and scratch area scripts (one `BinaryHeap` control anchor in `src/vec.rs` was
renamed to the new text and not exercised).
