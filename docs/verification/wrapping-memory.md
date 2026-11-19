# Wrapping arithmetic at memory boundaries

The [Vec checkpoint](vec-optimization.md) records the subsequent complete source
certificate and the current census of 28 internal modular calls. The 24-call
counts below describe the preceding checkpoint and its proof families.

Date: 2026-11-04. Baseline commit: `535b3583c1d84cb4d0814cbed7665dce107b0a86`,
plus the arithmetic, metadata, iteration, and verification working-tree changes.
The maintained runner records the verification-input hash, tool versions,
commands, and outcomes under ignored `target/verification/wrapping-memory/`.

Explicit wrapping arithmetic remains available without runtime overflow checks.
Its result grants no permission to access storage. A safe container validates the
result as an arbitrary integer before using it as a count, bound, or index.
The private wrapping helpers used by infallible metadata operations have the
stronger obligation that wrapping never occurs in a valid reachable state.

## Proven arithmetic properties

The source-linked model uses the owner invariant
`0 <= len <= cap <= limit` and the validated layout bound
`cap * size_of::<T>() <= isize::MAX`, with multiplication interpreted over
mathematical integers. For each covered memory span it proves:

* `0 <= start`, `0 <= count`, and `start + count <= cap`;
* the start offset, byte count, and end offset fit within the allocation and
  `isize::MAX`;
* a single-element access additionally has `index < len`;
* growth produces a capacity at least as large as the checked final length,
  below both the element limit and layout ceiling;
* internal cursor, compaction, extraction, tail-restoration, and capacity
  arithmetic cannot overflow or underflow under their established invariants.

Fallible count, range, and layout checks precede the corresponding element
read, write, or transfer. Reallocation validates the layout before calling
storage, and publishes the new pointer and capacity only after success.
The growth model derives its capacity from the actual `growth_capacity`
implementation, rather than assuming an allocator supplies sufficient space.
The initialized-footprint lemmas also show that compaction/extraction transitions
remove a consumed element from their live ranges, and restoration maps each
remaining live index to exactly one retained index.

Once an explicit wrapping operation has produced an ordinary integer, its
history is unavailable. For example, `usize::MAX.wrap_add(2)` is `1`; index `1`
is accepted when it is valid. The contract rejects invalid current values and
invalid derived sizes, rather than reconstructing a discarded overflow flag.

## Source connection and regression checks

An AST census finds all 24 current internal modular calls across budget,
capacity, cursor, compaction/extraction, Vec query, and Drain restoration code.
Each call family must have a named, freshly verified numeric contract; memory
consumers are connected to byte-span contracts. Unknown calls, direct primitive
methods, function references, aliased imports, and visible modular calls in
unreviewed macros cause the census gate to fail.

The runner validates the private helper bodies before projecting them to
mathematical arithmetic. Their actual Kani implementations assert the matching
`checked_add/sub/mul` preconditions; they do not assume them. Closed state
constructors, transitions, and queries are projected from their real method
bodies. Actual memory argument expressions and preceding guards are extracted
for insertion/removal, spare views, internal copy/clone, compaction/extraction,
Drain, and splice tail moves. Reserve/growth/layout/storage composition is
checked against the real source. The scalar checked getter delegates to the
pinned core slice implementation, whose source hash is recorded.

This is a reviewed source projection with explicit allocator and ownership
boundaries. It is not a compiler-verified translation of all Rust unsafe code.
The census rejects changes outside its known obligations; it does not certify
arbitrary future macro expansion or arbitrary new unsafe APIs.

| Check | Result and domain |
| --- | --- |
| Verus, native 64-bit words | 100 verified results with `--no-cheating`, including the shared numeric, closed-state, Vec-kernel, and memory/composition models |
| Verus, 32-bit word instance | The same 100 results pass after specializing `usize/isize` to `u32/i32`; this is a scalar proof, not 32-bit heap execution |
| Actual Rust Kani | 18 focused harnesses pass: 12 boundary/memory probes and six existing state-closure harnesses. The complete default-feature suite also passes all 55 harnesses, including those 18 |
| All-word rejection | Actual range normalization is checked for every machine-word length/endpoint and every bound kind; scalar, slice, reservation, split, and Box probes accept arbitrary incoming words |
| Actual heap transitions | Finite non-ZST owners: byte lengths 0–2, capacities up to four, a two-element u64 owner, and fixed admitted movement/cleanup cases |
| Deliberate memory defects | A one-element destination overrun fails Verus; a one-past-allocation read fails Kani's actual object-bounds check |
| Census bypass controls | Direct modular method, aliased helper, and macro-body controls are rejected |
| Underlying source mutations | Five numeric, three closed-state, and eight Vec-kernel defects are rejected |
| Runtime memory checks | Five dedicated tests pass normal execution, Miri, Tree Borrows with strict provenance, and AddressSanitizer |

The physical Kani probes retain the normal memory, overflow, undefined-behavior,
and unwinding assertions. They use CaDiCaL, ten object bits, and explicit loop
bounds; object exhaustion and incomplete unwinding remain failures. The finite
heap checks supplement the all-size arithmetic proof and are not described as
unbounded heap verification. Under Kani's normal allocator model allocation
succeeds; the compositional model permits storage refusal, and runtime budget
tests exercise refusal before initialization or element transfer.

The new regression tests feed real public wrapping results into indexed access,
insertion, range editing, capacity construction, Box initialization, and iterator
hints. They compare accepted operations with std and check rejected-input,
content, initialization, and budget preservation.

## Boundaries and performance

The arithmetic-to-byte-extent theorem assumes an already valid owner/allocation,
Rust's element layout, and the allocator and explicit unsafe raw-import
contracts. It does not establish arbitrary user callback behavior or complete
Rust pointer provenance, aliasing, initialization, and allocator correctness.
Those obligations remain distinct from proving that wrapping cannot shrink a
checked allocation or advance the covered spans past its end. The existing
whole-source ownership-to-pointer refinement and residual debug link obligations
are not marked complete by this work.

The subsequent [unsafe verification assessment](unsafe-proof-plan.md) separates
the required numeric/caller proofs from properties covered by Miri, failure
tests and compiler checks. It does not require a new complete Rust aliasing
model for every unsafe block.

Only proof code and verification infrastructure were added for this task.
The rebuilt optimized Vec timing executable is byte-identical to the preceding
iteration checkpoint, SHA-256
`dfaefbf62f29d0899b8440a536dbd05a6f37487bcb750d51792d2f909fecb03a`.
No additional instructions or allocations were added to that executable.
Existing performance acceptance failures remain as recorded in the
[iteration report](iteration.md); no fresh universal speed claim is made.

## Reproduction

Use Rust 1.95, nightly-2026-10-30, Kani 0.68.0 / CBMC 6.11.0, and Verus
0.2026.11.02.8bf68ee. Put Verus's Z3 binary on `PATH` or use `JZAP_VERUS`
to select the verifier; the runner searches beside that executable.

```sh
python3 scripts/verify_wrapping_memory.py --mutations
cargo test --locked --all-features --test wrapping_memory
cargo +nightly-2026-10-30 miri test --locked --all-features --test wrapping_memory
MIRIFLAGS='-Zmiri-tree-borrows -Zmiri-strict-provenance' \
  cargo +nightly-2026-10-30 miri test --locked --all-features --test wrapping_memory
RUSTFLAGS='-Zsanitizer=address' cargo +nightly-2026-10-30 test --locked \
  --all-features --target x86_64-unknown-linux-gnu --test wrapping_memory
```

The runner regenerates dependencies and both word-width proofs, executes the
real Rust probes and requested mutation controls, and writes a passing receipt
only if every stage succeeds with unchanged verification inputs. The root and
Vec verification drivers include this gate and the new memory tests. No consumer
build hook or automatic CI trigger is installed.
