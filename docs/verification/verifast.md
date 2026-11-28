# VeriFast: auxiliary proof of `Capacity<T>` and `layout::array`

Status: auxiliary tool, adopted by owner decision. The primary method for the
premises of the source certificate stays the Kani-based closure described in
[`docs/premise-proofs.md`](../premise-proofs.md). The VeriFast result below is
supplementary evidence. It changes no disposition in
`scripts/premise_status.py` or in the certificate.

VeriFast is a verification tool dependency only. It is downloaded to a cache
outside the repository; the product crate, its manifest, lock file and build
are unchanged, so the product machine code is unaffected.

## What is proved

The proof runs on a subset crate assembled from the current `src/` (see
[Workflow](#workflow)). The subset contains `src/arithmetic.rs`,
`src/arithmetic/checked.rs` (without its two trailing `TryClone` impls),
`src/arithmetic/bounded.rs`, `src/capacity.rs`, `src/error.rs`,
`src/layout.rs` and `src/layout/numeric.rs`, under a new root that declares
only those modules.

VeriFast proves each specification below for every element type `T` (the
proof is generic in `T`; no instantiation is chosen), with `usize` and
`isize` of the LP64 target `x86_64-unknown-linux-gnu`. Every specified
function also has `on_unwind_ens false`: it does not unwind.

The invariant of `Capacity<T>` is its ownership predicate:

```text
<Capacity<T>>.own(t, c) = is_valid_layout(c.count * size_of::<T>(), align_of::<T>())
is_valid_layout(size, align) = align is a power of two && size <= isize::MAX - isize::MAX % align
```

For a safe function VeriFast also checks a `safety_proof`: the specification
implies the generated semantic type of the function, so every `Capacity<T>`
returned by a safe constructor satisfies the invariant, and every safe
caller may rely on it.

| Function | Class | Proved specification |
| --- | --- | --- |
| `layout::numeric::array_ceiling` | verified | `result == if size == 0 { usize::MAX } else { isize::MAX / size }` |
| `layout::numeric::array_bytes` | verified | `Ok(bytes)` implies `bytes == count * size` and `bytes <= isize::MAX`; `Err(e)` implies `e == Error::Layout` |
| `layout::array<T>` | verified on a rewrite, refined | `Ok(l)` implies `is_valid_layout(count * size_of::<T>(), align_of::<T>())`, `l.size() == count * size_of::<T>()`, `l.align() == align_of::<T>()`; `Err(e)` is an owned `Error` |
| `capacity::logical_capacity` | verified | `result == if size == 0 { limit } else { stored }` |
| `Capacity::<T>::empty` | verified | `result.count == if size_of::<T>() == 0 { limit } else { 0 }`; the result satisfies the invariant |
| `Capacity::<T>::try_new` | verified | `Ok((c, l))` implies `c.count == count`, the byte bound, and the two fields of `l`; the result satisfies the invariant |
| `Capacity::<T>::from_validated_raw` (unsafe) | verified | Under `req is_valid_layout(count * size_of::<T>(), align_of::<T>())`: `result.count == count` and the result satisfies the invariant |
| `Capacity::<T>::layout` | verified | Under the invariant: `count * size_of::<T>() <= isize::MAX` (the `wrapping_mul` does not wrap), `result.size() == count * size_of::<T>()`, `result.align() == align_of::<T>()`, so the `from_size_align_unchecked` precondition holds |
| `Checked::<T>::new`, `Checked::<T>::get` | verified | `result == Checked(value)`, `result == self.0` |
| `arithmetic::bounded::wrapping_mul` | verified | `left * right <= usize::MAX` implies `result == left * right` |
| `Capacity::<T>::for_slice` | skipped (specless) | Not checked. A constructor of `Capacity<T>`: VeriFast does not show that it establishes the invariant. Only the Kani harness `closure_slice_capacity_matches_the_slice_layout` covers it, for named instantiations |
| `Capacity::<T>::get`, `clone`, `sub`, `adopted_capacity` | skipped (specless) | Not checked |
| `wrapping_add`, `wrapping_sub`, the `Checked` operators, the `TryArithmetic`/`WrappingArithmetic` impls, `Display`/`From` impls | skipped (specless) | Not checked |
| Items under `cfg(verus_keep_ghost)` or `cfg(kani)` | not compiled | Not part of the VeriFast run |
| Derived impls (`Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`) | not specified | Not checked |

VeriFast reports `0 errors found (170 statements verified)`.

The full per-function table, with every `fn` of the subset and its class, is
the `functions` list of
[`tools/verifast/proofs/capacity/manifest.json`](../../tools/verifast/proofs/capacity/manifest.json);
`check.py` fails if the subset gains or loses a `fn` that the list does not
classify, if a `verified` function is missing from VeriFast's log, or if
VeriFast verifies a function the list does not name.

### Kani half for `layout::array`

The rewrite of `layout::array` exists only because VeriFast 26.10 cannot
verify the closure passed to `Result::map_err`. The refinement checker
connects the original to the rewrite. In addition, Kani checks the VeriFast
postcondition on the original product function (with `map_err`), for every
`count`, at `u8`, `u64`, `[u8; 1600]` and a 64-byte-aligned type
([`tools/verifast/kani/array_spec.rs`](../../tools/verifast/kani/array_spec.rs)).
The harnesses run on an assembled copy of the product crate under
`target/verification/verifast/kani-crate/`, where the file is appended to the
`cfg(kani)` module `src/layout/proofs.rs`; `src/` itself is not changed.
The unsupported-construct checks Kani reports (`caller_location`, the
`rust_begin_unwind` foreign call) have status SUCCESS: they are not reached.

## Workflow

```text
src/ (current) ─┐
                ├─ assemble ─> original crate ─┐
manifest.json ──┤                               ├─ refinement checker: original refines verified
stored files ───┴─ assemble ─> verified crate ─┴─ VeriFast: verified satisfies its specs
```

1. `tools/verifast/setup.py` downloads the pinned release
   ([pin](#version-pin)) to `~/.cache/jzap/verifast/26.10` (or
   `$XDG_CACHE_HOME`), checks the recorded sha256, unpacks it into a fresh
   `install/` directory, applies
   [`std-spec-overlay.patch`](../../tools/verifast/std-spec-overlay.patch)
   with exact context against pinned base files, and checks that
   `rustup` has `nightly-2026-09-25` with `rustc-dev` and `llvm-tools`
   (`--install-toolchain` installs it; nothing is installed without it).
2. `tools/verifast/check.py` assembles two subset crates under
   `target/verification/verifast/` from the manifest:
   - original side: each module is the product file (`product`) or a pinned
     prefix of it (`product-cut`, `src/arithmetic/checked.rs` lines 1-137;
     the dropped part has its own sha256);
   - verified side: the product file (`product`), a stored annotated copy
     (`annotations-only`), or a stored rewrite (`rewrite`, only
     `src/layout.rs`).
   No copy of a product file whose content must equal `src/` is stored in
   the repository.
3. Identity: each product file must equal the sha256 pinned in the
   manifest. A changed `src/` file fails the check; that is the signal to
   re-review the proof and update the pin. Original-side files are compared
   byte for byte with the product bytes (or the cut).
4. Comment strip: each `annotations-only` file must equal its original side
   after removing VeriFast comments (`/*@ ... @*/`, `//@ ...`). The
   comparison lexes both files with rustc's comment and literal rules
   (nested block comments, string, raw string, byte and char literals), so a
   `/*@` inside a string literal is not removed, and compares the remaining
   tokens ignoring whitespace. Ordinary comments must match too.
5. Inventory and trusted lemmas: every `fn` of the original side is
   classified in the manifest, and every `assume(..)` in the verified side
   is inside a lemma on the trusted list.
6. VeriFast on the verified crate:
   `verifast -skip_specless_fns -allow_assume -allow_dead_code -verbose 1 -rustc_args '--edition=2024 --cfg feature="alloc"' src/lib.rs`.
   The first three flags are those of the evaluation spikes; `-verbose 1`
   only adds the per-function log. What the flags hide:
   - `-skip_specless_fns`: functions without `req`/`ens` are not verified
     at all, not even against their generated safety specification. They
     are the `skipped-specless` rows above.
   - `-allow_assume`: `assume(..)` is accepted in ghost code. There are
     exactly two, both `assume(false)` in variance lemmas (below).
   - `-allow_dead_code`: code that symbolic execution finds unreachable is
     not reported as an error.
7. Refinement checker: `refinement-checker --rustc-args '--edition=2024 --cfg feature="alloc"' original/src/lib.rs verified/src/lib.rs`
   must report `No refinement errors found`. It also compares the text of
   each file pair outside VeriFast comments: in the spikes it rejected an
   attribute change (`#[inline(never)]`) and an added `#[repr(C)]` that way.
8. Kani half (above), then the negative controls.

### Negative controls

Each control mutates a fresh copy and must be rejected by the named check.

| Control | Mutation | Must be rejected by |
| --- | --- | --- |
| `refinement-array-returns-capacity` | the `Err` arm of the `array` rewrite returns `Error::Capacity` | refinement checker (VeriFast accepts it: its spec allows any owned `Error`) |
| `verifast-array-spec-count-below-5` | `array` postcondition strengthened with `count < 5` | VeriFast |
| `verifast-capacity-invariant-dropped` | `<Capacity<T>>.own` defined as `true` | VeriFast (in `Capacity::layout`: `Cannot prove 0 < alignof(T)`, a fact only the invariant supplied) |
| `strip-non-comment-edit` | `#[inline]` to `#[inline(never)]` in `capacity.rs` | comment-strip check and refinement checker |
| `identity-product-changed` | one character added to a temporary copy of `src/capacity.rs` | identity check |
| `kani-wrong-size-postcondition` | Kani harness asserting `l.size() == count` for `u64` | Kani |

The controls are stored as data in
[`tools/verifast/proofs/capacity/controls.json`](../../tools/verifast/proofs/capacity/controls.json).
Unit tests of the manifest, identity, cut and comment-strip logic are in
`scripts/test_verify.py` (`VeriFastTests`); they do not run VeriFast.

## Trusted base

Everything below is trusted, not checked by this workflow.

- The nightly compiler `nightly-2026-09-25` (`rustc 1.100.0-nightly
  (f7575a9da 2026-09-24)`) and VeriFast's MIR exporter
  (`vf-rust-mir-exporter`, linked against that compiler's `rustc_driver`).
  The product is built with stable 1.95; VeriFast sees the MIR of the
  nightly compiler.
- VeriFast 26.10 itself (symbolic execution, its SMT back end) and the std
  specifications it ships (`bin/rust/**/*.rsspec`), including
  `Layout::from_size_align_unchecked`, `ok_or`, `Option`/`Result` and the
  `is_valid_layout` lemmas.
- Our overlay additions in `tools/verifast/std-spec-overlay.patch`, each
  marked `JZAP-OVERLAY (trusted)` with the reason it is needed:
  - spec of `usize::checked_mul`, `usize::checked_div`,
    `usize::wrapping_mul` (called by `array_bytes`, `array_ceiling`,
    `bounded::wrapping_mul`);
  - spec of `Layout::from_size_align` (called by `array`; `Ok` implies
    `is_valid_layout` and the fields, `Err` unconstrained);
  - `from_residual`: `on_unwind_ens true` replaced by
    `typeid(E) != typeid(F)`, i.e. the identity `From` used by `?` on
    `Result<_, Error>` does not unwind;
  - axiom `close_tuple_2_own`, mirroring the shipped `close_tuple_4_own`
    (the safety proof of `try_new` returns a pair).
- The refinement checker's OCaml models of std functions and of MIR
  semantics.
- Two `assume(false)` lemmas: `Capacity_own_mono` and `Checked_own_mono`,
  the variance obligations of the two ownership predicates. VeriFast's own
  `RawVec` proof discharges this obligation the same way (VeriFast issue
  #610). `check.py` fails if any other `assume` appears.
- The byte-identity glue: `check.py`'s assembly, sha256 pins and comment
  strip connect the subset crate to `src/`. The subset root
  (`tools/verifast/proofs/capacity/lib.rs`) is not a product file; the
  product's `lib.rs` is not checked.

## What is not checked

- Unwind paths in the refinement checker: it does not compare unwinding
  behaviour of original and verified. (VeriFast's `on_unwind_ens false`
  applies to the verified side.)
- Storage liveness (`StorageLive`/`StorageDead`) in the refinement checker.
- `&mut` and `Box` aliasing: VeriFast does not check the Tree Borrows (or
  Stacked Borrows) model; Miri covers executed paths elsewhere.
- Specless and derived functions (table above), in particular
  `Capacity::for_slice`.
- The callers of `Capacity::from_validated_raw` (`src/deque.rs:512`,
  `src/vec/ownership.rs:32`) are outside the subset; VeriFast proves the
  function under its `req` and does not show that those callers establish
  it.
- Other targets: only `x86_64-unknown-linux-gnu` (LP64) was run. 32-bit and
  other platforms are not checked by this workflow.
- The product crate as a whole: the result is on the subset crate.

## Version pin

All in [`tools/verifast/pin.json`](../../tools/verifast/pin.json):

| Item | Value |
| --- | --- |
| Tag | `26.10` (released 2026-10-05) |
| Source commit of the tag | `f76012fb518703b1d98e243ac4f487f16877cfcb` |
| Archive | `verifast-26.10-linux.tar.gz` from `github.com/verifast/verifast/releases/download/26.10/` |
| sha256 | `5ad2a65997457fd5996a27f93107ef7e52c90c0d12b63b6e67f7acbc73e90efd` (recorded from the downloaded archive; the release publishes no checksum) |
| Toolchain | `nightly-2026-09-25` with `rustc-dev`, `llvm-tools` |
| Overlay base files | sha256 of `bin/rust/prelude.rsspec` and `bin/rust/std/lib.rsspec` |

## Reproduce

```sh
python3 tools/verifast/setup.py              # download (cached), verify, unpack, overlay
python3 tools/verifast/check.py              # setup + all checks + controls
bash scripts/verify.sh --check verifast      # the same through the runner
python3 scripts/test_verify.py               # includes VeriFastTests
```

`check.py` writes `target/verification/verifast/summary.json` (tool
versions, flags and what they hide, statement count, per-function coverage,
trusted list, control results) and the logs under
`target/verification/verifast/logs/`. Exit status 1 on any failure.

`--check verifast` is local only. It is in neither the quick nor the full
profile of `scripts/verify.py` (the full profile is not run by any
workflow either; the check is kept out because it downloads a tool and
needs a nightly toolchain) and in no CI workflow.

Last run (2026-11-28, product tree `09e9247` plus this tooling, fresh
download and unpack): all checks passed; VeriFast 170 statements, 0.6 s;
refinement checker 0.8 s; Kani 4 of 4 harnesses, 7.4 s (the negative harness
failed as required); 6 of 6 controls rejected.

## Scaling limits found in the evaluation spikes

- Closures need a rewrite each. VeriFast 26.10 cannot verify a closure
  passed to `map_err` (and similar combinators) without a spec for the
  combinator and the closure; the pilot rewrote the one closure in
  `layout::array` and connected it with the refinement checker. A lexical
  count of closure expressions in product files (excluding `*proofs.rs`;
  `|` after `(`, `,`, `=` or `move`; approximate, not a verified number)
  gives 141 in `collections/btree_map`, 53 in `vec`, 30 in `boxed`, 27 in
  `shared`, 24 in `collections/hash_map`, 18 in `deque`, 17 in
  `collections/hash_set`, 1 in `layout`, 0 in `capacity` and `arithmetic`.
- Subset crates only. On the whole product crate (annotated copy against
  the unchanged crate) the refinement checker stopped with the internal
  error `Failure("TODO: Dynamic")`, an unsupported construct, so each area
  needs its own subset root and cut, and each cut is a reviewed boundary.
- Specless functions cannot simply be included. Without
  `-skip_specless_fns`, VeriFast rejects the Capacity subset at
  `src/arithmetic.rs` ("Expressing shared ownership of an enum value is not
  yet supported"), so the skipped functions stay unchecked rather than
  checked against their generated safety specification.
- Structural rewrites. Each construct VeriFast does not support needs a
  rewrite of the verified side, and each rewrite is another
  refinement-checker obligation whose unwind paths are not compared.
- B-tree edges. In the B-tree spike, `node::edge`/`child` (pointer to the
  `edges` array of a `Branch` reached through a `Leaf` pointer) failed with
  `Pointer may be out of bounds (Cannot prove pointer_within_limits(ptr_add_(last_field_ptr(..)..)))`:
  VeriFast could not relate the address of the trailing array field reached
  through the cast to the bounds of the allocation. The spike did not
  resolve it; the B-tree edge reads are not covered.
