# Fallible String contracts

Implementation target: Rust 1.95 stable `alloc::string::String`, using the existing
owned `Vec<u8, P, L>` storage. The [verification report](verification/string.md) records the executed checks
and performance status.

The owner stores valid UTF-8 at every public boundary. Its allocation policy and
optional byte-length ceiling are identical to Vec's policies. Capacity, initialized
byte length, maximum byte length, and the allocation budget remain independent.
Default handles retain Vec/std's three-word size. No runtime dependency is added.

| Operation family | Failure and state contract |
| --- | --- |
| Construction/conversion | Validate encoding, layout, budget and ceiling. Rejected owned input is returned; partial new owners are destroyed. |
| Append, insert, replace, reserve, shrink, split | Validate byte ranges and UTF-8 boundaries before editing. Reserve before moving bytes. Returned errors preserve original text; successful earlier reservations may retain capacity. Split retains the source policy and byte ceiling. |
| Truncate, remove, drain | Invalid byte boundaries return an error. Truncating beyond the end is a no-op, matching std. Drain removal occurs on Drop; forgetting it retains the original text, matching std String. |
| Retain | Process complete Unicode scalar values. On predicate error, earlier removals remain removed and the current/unvisited suffix survives. Callback unwind preserves valid UTF-8. |
| Formatting and iterator extension | Roll back appended bytes on storage, formatting or traversal error. Formatting remembers failed sink writes even if Display ignores them. Capacity may have grown; callback side effects and consumed inputs are not restored. |
| Clone | Preserve source; clone replacement retains destination policy and ceiling. Target-budget clone charges newly owned storage to the requested budget. |
| Standard/raw/Box transfer | Explicit ownership and accounting boundary. Unsafe reconstruction requires the existing Vec allocation/initialization contract plus valid UTF-8. Safe APIs expose no mutable byte owner. |

Explicit `as_str`/`as_mut_str` borrows retain core str contracts. No Deref, Index,
ordinary allocating Clone/Add/Extend/FromIterator, or mutable byte access is
provided as a safe implicit operation. Fallible equivalents supply their function.
Standard char/string iterators are explicit bridges; native fallible inputs use
TryIntoIterator. User callbacks follow the existing trusted execution contract.

The implementation was reviewed against Rust stable 1.95 (`59807616e`) and
nightly-2026-10-30 (`92b5b8c87`), using `library/alloc/src/string.rs`.

Validation includes std differential/property tests, Unicode/invalid encoding
boundaries, every allocation refusal, limits/budgets, Miri with both alias models,
ASan, overflow-checked release, linked exit inspection and source-connected
arithmetic/memory-boundary proofs. Runtime comparison uses equal randomized work,
three seeds, and paired intervals of at least one second. Baseline acceptance
uses the existing +15% time/memory criteria; handle policy words are shown
separately. Raw results and generated artifacts belong under ignored target/.
