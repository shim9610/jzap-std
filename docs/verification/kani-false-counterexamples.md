# Kani false counterexamples: reproductions and diagnosis

This report prepares the debugging of Kani failures that were judged false
counterexamples in the partial-key B+ lookup work. It adds standalone
reproductions under `tools/kani-repro/`, runs them in Kani and in CBMC
directly, and records which CBMC component produces the wrong verification
condition for the reproductions that fail. No product source under `src/`
and no existing harness was changed.

It does not diagnose every earlier failure. Which of the current product
harness failures come from the fault found here is not yet checked; see
[What remains unknown](#what-remains-unknown).

## Conditions

- Rust 1.95.0 (`RUSTUP_TOOLCHAIN=1.95.0`) for native tests; Kani 0.68.0
  (`cargo install --locked kani-verifier --version 0.68.0`, then `cargo kani
  setup`), with its bundled CBMC 6.11.0 (`cbmc-6.11.0`) and Kissat.
- Kani runs: `cargo kani --solver kissat -Z unstable-options --cbmc-args
  --object-bits 12`, one job at a time, Linux x86-64, about 15 GB of memory.
- Direct CBMC runs use the command line Kani itself runs for the harness
  (from `cargo kani --verbose`): `--no-malloc-may-fail
  --no-undefined-shift-check --no-signed-overflow-check --nan-check
  --no-self-loops-to-assumptions --no-pointer-primitive-check --slice-formula`
  plus the harness's `--unwind`, on the goto binary kept with `--keep-temps`.
  Kani's reachability checks are `assert(false)` and always report FAILURE;
  the tables count only the other properties.
- Upstream sources, read only and not built: CBMC tag `cbmc-6.11.0`
  (`820ff0f555b43fb78e0cd9332e498461bd14244b`), CBMC `develop` at
  `6c5f75f0128d24c8e09b030b6dde3785bbf54863` (2026-11-24), Kani tag
  `kani-0.68.0` (`0d2328a93f0e0ff66132d6bfa1a7d884877cf862`). They were
  fetched under `target/verification/upstream/`.

## Summary

| Reproduction | Kani 0.68.0 | Concrete playback, native | Native exhaustive test |
| --- | --- | --- | --- |
| (a) symbolic index into an array of key structs (`symbolic-index`) | A1, A2, A4 fail; controls A3, A5 verify | A1, A2, A4: the generated test passes (does not reproduce) | passes |
| (b) consuming `(0..len).filter(..)` over such keys (`filter-range`) | B1 fails; B2 and controls B3, B4 verify | B1: the generated test passes (does not reproduce) | passes |
| (c) `keys.get(start..).windows(2)` with a symbolic `start` (`windows-from`) | C1, C2 and control C3 verify | not run (no failure) | passes |
| CBMC-only C program (`cbmc-c`) | not applicable; CBMC 6.11.0 fails V1, V2 | not applicable; `native.c` holds for all 131,072 inputs | — |

The failing reproductions share one trigger: a pointer to an array-typed
struct member at a nonzero offset, where the struct is an element of an array
reached at a symbolic position (an index, or an iterator's position). The
same harnesses verify when the array member is at offset 0. The 12-line C
program in `tools/kani-repro/cbmc-c/` fails in CBMC 6.11.0 without Kani, so
the fault is in CBMC, not in Kani's code generation.

The diagnosis in [CBMC diagnosis](#cbmc-diagnosis) locates the wrong
expression in CBMC's dereference of such pointers and reads the CBMC branch
that produces it. The branch was identified from the source and the
verification conditions; no instrumented or patched CBMC was built.

## Reproductions

Each crate is standalone (`[workspace]`, no dependencies, no jzap). Its key
struct mirrors `PrefixKey<'a, 1>` of
`src/collections/btree_map/view/bplus/partial.rs`: `prefix: [u8; 1]`,
`len: usize`, `full: Option<&[u8]>`. In Rust layout a mirror of that struct
has size 32 with `prefix` at offset 24 (`full` at 0, `len` at 16; Rust 1.95.0,
x86-64, checked with `offset_of!` on a scratch copy). Each crate has a native
exhaustive test of the stated property over keys of up to two bytes built
from {0, 1, 2}.

### (a) `symbolic-index`

- A1: every key admits its own completion (checked first with a concrete
  loop); then, for a symbolic `requested < 2`,
  `keys[requested].admits(completed[requested])`. Kani fails it.
- A2: the removed harness shape of `docs/bplus-partial-keys.md`: keys
  attached at random, then `keys[requested].attach(completed[requested])`
  for an unresolved key. Kani fails it.
- A3 (control): the same as A2, selecting the key with a concrete loop, as
  the product harnesses now do. Verifies.
- A4: the reduced form. `#[repr(C)] struct { a: u8, prefix: [u8; 1] }`,
  `let p = &keys[r].prefix; assert!(p[0] == v[r])`. Kani fails it.
- A5 (control): A4 with the array member first, at offset 0. Verifies.

Reduction steps in a scratch crate under `target/` (not kept), each a Kani
result: removing the `Option<&[u8]>` field still fails; indexing only the
key array symbolically fails, indexing only the completion array passes; a
plain read `keys[r].prefix[0]` passes; a reference to a `u8` field passes; a
reference to a `usize` field passes; an array member at offset 0 passes; an
array member at offset 1 or 8, of `u8` or `u64` elements, fails; a raw
pointer `keys[r].prefix.as_ptr()` read fails.

Concrete playback of A1 (raw keys `[65, 127]` and `[1, 127]`, both of length
2, `requested = 1`), A2 and A4 was generated with `--concrete-playback` and
each generated test passes natively.

### (b) `filter-range`

`listed(keys)` is `(0..keys.len()).filter(|&i| needs(keys, i))`, the shape of
`PartialLookup::unresolved_keys`; `needs` reads `keys.get(index)` and
compares the inline bytes of neighbouring keys. Three keys of up to two
bytes, attached at random.

- B1: the filter consumed by a `for` loop gives the same index mask as a
  concrete loop. Kani fails it (92 s).
- B2: `listed(keys).count()` equals the loop's count. Verifies.
- B3, B4 (controls): B1 and B2 with `#[repr(C)]` and the array member at
  offset 0. Both verify.

The B1 playback (three keys `[80, 255]` of length 2, none attached: every
key is listed) passes natively.

### (c) `windows-from`

`pairs_by_windows` is the pair loop of `PartialLookup::pass`:
`keys.get(left..).unwrap_or(&[]).windows(2)` from a symbolic `start <= 3`,
stopping at the first pair the stored parts cannot order. The reference
iterates every concrete pair index and skips those below `start`.

- C1: symbolic start, Rust layout. Verifies (52 s).
- C2: start 0. Verifies.
- C3: symbolic start, array member at offset 0. Verifies.

This reproduction does not confirm the hypothesis that resuming
`windows(2)` from a symbolic start triggers the product failures. It
exercises only the pair loop, not the binary search or the attachment steps
of `pass`.

## CBMC option variants

Same goto binary per row; "fails" means the reproduction's assertion
reports FAILURE.

| CBMC options added | A1 | A4 | B1 | C program V1 | C program V2 |
| --- | --- | --- | --- | --- | --- |
| `--external-sat-solver kissat` (Kani's run) | fails | fails | fails (88 s) | — | — |
| `--sat-solver minisat2` (C program: CBMC's default solver) | fails | fails | fails (25 s) | fails | fails |
| `--sat-solver cadical` | fails | fails | fails (38 s) | not run | not run |
| `--object-bits 8` / `16` | fails / fails | fails / fails | not run / fails | not run | not run |
| `--no-simplify` | verifies (116 s) | verifies | out of memory (killed after 49 s) | verifies | fails |
| `--arrays-uf-always` / `--arrays-uf-never` | not run | fails / fails | not run | not run | not run |
| `--no-array-field-sensitivity` | not run | fails | not run | not run | not run |
| without `--slice-formula` | not run | fails | not run | not run | not run |

The solver and object bits do not change the outcome. `--no-simplify`
removes the A1, A4 and V1 failures but not V2, so it is not a workaround.
CBMC 6.11.0 has no `--no-slice-formula` option (usage error); the row above
removes `--slice-formula` from the command line instead.

## CBMC diagnosis

The goto program of A4 is a direct translation of the source (`goto-instrument
--show-goto-functions`):

```text
var_18::p := address_of(var_4::keys[var_15::r].prefix)
var_24 := *var_18::p[var_25]          // var_25 = 0
```

`cbmc --show-vcc` gives the value of the read. With default simplification:

```text
var_24 = { keys[[0]]..prefix[[0]] }[r * 2]
```

that is, the one-element array `keys[0].prefix` indexed by `2r`. For `r = 0`
this is the right byte; for `r = 1` the index is outside the one-element
array, the read is unconstrained, and the `--trace` of a scratch copy of A4
shows the read `var_24 = 16` with `v = [0, 0]`. The expected value is `keys[r].prefix[0]`. With
`--no-simplify` the read is a `byte_extract_little_endian` of the whole
`keys` array at `pointer_offset(p)`, which is correct, and A4 verifies.

The C program shows the same expression (`{ keys[[0]]..prefix[[0]] }[r * 2]`)
for V1 by default and, for V2 (`unsigned char *q = keys[r].prefix; q[0]`),
also with `--no-simplify` (`keys[0].prefix[r * 2 + 0]`). In B1 the same
shape appears inside CBMC's `memcmp` model for the slice comparison:
`{ keys[[0]]..prefix[[0]] }[index * 32]`, the struct size being the stride.

Reading of `src/util/pointer_offset_size.cpp` at `cbmc-6.11.0`: the overload
of `get_subexpression_at_offset` for a non-constant offset, called from
`value_set_dereferencet::build_reference_to` in
`src/pointer-analysis/value_set_dereference.cpp` ("try to build a
member/index expression - do not use byte_extract"), has an array branch:

> If we have an offset C + x (where C is a constant) we can try to recurse
> by first looking at the member at offset C.

For the root object `keys` (two 2-byte structs), target `u8` and offset
`1 + 2r`, the subexpression at constant offset 1 is `keys[0].prefix[0]`, an
index expression with index 0, so the function recurses into its array
`keys[0].prefix` with the remaining offset `2r`. That recursion assumes that
adding `x` keeps the access inside the innermost array containing offset C.
Here `x` steps over whole outer elements, so the result indexes
`keys[0].prefix` instead of `keys[r].prefix`. The same pattern is in the
struct branch of that function. By the same reading, when the member array is
at offset 0 the offset has no constant term and the multiplication branch
indexes the outer array; the controls verify, which is consistent with this.

The same branch is unchanged on CBMC `develop` at `6c5f75f` (2026-11-24, by
source comparison only; that revision was not built or run).

Not confirmed: that this branch, rather than another path with the same
result, produced each expression. That needs a debugger or an instrumented
CBMC build, which was outside this preparation.

## Earlier cases

| Case | Status after this work |
| --- | --- |
| 1. width-one `get_unchecked` chains in `FixedTree` (playback diverges, "Not enough det vals found") | not reproduced here; no reproduction targets it. The failure mode differs (the trace is not an execution) |
| 2. `keys[requested]` over `PrefixKey` | reproduced by A1, A2 and reduced to A4; the C program fails in CBMC alone |
| 3. consuming `unresolved_keys` | reproduced by B1 (`for` consumption); `count()` (B2) verifies |
| current `resumed_*_pass_equals_fresh_pass`, removed `visit_of_*` harnesses | not run; see below |

## Upstream candidates

None found matches this trigger exactly. Searched with the web search tool
on 2026-11-25; issue pages read for the four closest.

| Issue | State | Relation |
| --- | --- | --- |
| [model-checking/kani#5104](https://github.com/model-checking/kani/issues/5104) "False counterexample for a conditional pointer read of `[u8; 1]`" | open, Kani 0.68.0 / CBMC 6.11.0 | Same versions, `[u8; 1]` elements and a false counterexample for a pointer read. Its programs are arrays of arrays and a scalar struct, with the pointer chosen by a condition; no array member at a nonzero offset. The reporter did not identify a component. Already cited for case 1 in `linux-bottlenecks.md`. |
| [diffblue/cbmc#8128](https://github.com/diffblue/cbmc/issues/8128) "Unsound optimization of pointer dereferencing" | open since 2023-12 | An array member at a nonzero offset (`middle` at offset 4), read through `*(p + i)`; CBMC returns other fields' values. Attributed to `try_add_offset_to_indices` in `value_set_dereferencet::dereference` (PR #5418). Its struct is a single local, not an array element, and its index may leave the member (a comment argues undefined behaviour). Related code area, different branch. |
| [diffblue/cbmc#9520](https://github.com/diffblue/cbmc/issues/9520) spurious counterexample for `{ {1, 1}, {1, 1} }[i][0] == 1` | open | Symbolic index into nested arrays; needs `--arrays-uf-always`, passes with `--no-simplify`. Our failures occur without that option. |
| [diffblue/cbmc#9521](https://github.com/diffblue/cbmc/issues/9521) invariant failure for `a[i].data[k]` | open | Array member of a struct in an array at a symbolic index, but at offset 0, an invariant failure in `add_array_constraints` with `--arrays-uf-always`, no pointer. |

Other results of the same searches (CBMC byte-update issues #9170–#9173 and
#9237, #188, #9404, #9405) concern byte updates, memcpy or writes and were
not compared further.

## Reproduce

```sh
# Native exhaustive tests (each crate)
RUSTUP_TOOLCHAIN=1.95.0 CARGO_TARGET_DIR=target/verification/kani-repro/symbolic-index \
  cargo test --manifest-path tools/kani-repro/symbolic-index/Cargo.toml
# Kani, all harnesses of one crate (run from the crate directory)
cd tools/kani-repro/symbolic-index
RUSTUP_TOOLCHAIN=1.95.0 CARGO_TARGET_DIR=../../../target/verification/kani-repro/symbolic-index \
  cargo kani --solver kissat -Z unstable-options --cbmc-args --object-bits 12
cd ../../..
# Concrete playback of one harness, natively, in a copy under target/
bash tools/kani-repro/playback.sh tools/kani-repro/symbolic-index \
  proofs::a4_member_array_at_nonzero_offset
# Direct CBMC option variants on the kept goto binary
bash tools/kani-repro/cbmc_variants.sh tools/kani-repro/symbolic-index \
  proofs::a4_member_array_at_nonzero_offset
# CBMC without Kani, and the native C check
bash tools/kani-repro/cbmc-c/run.sh
```

Replace the crate and harness names for the other reproductions
(`filter-range`: `proofs::b1_filter_matches_loop` …; `windows-from`:
`proofs::c1_symbolic_start_windows_match_indexes` …). Logs, goto binaries,
goto listings and playback copies go to `target/verification/kani-repro/`.
`cbmc --show-vcc` on B1 writes about 17 GB; filter it as it is written.

## What remains unknown

- Whether the current product failures (`resumed_leaf_pass_equals_fresh_pass`
  reporting `assertion failed: same`, the removed `visit_of_a_two_key_leaf_*`
  and `visit_of_a_branch_*` harnesses) come from this fault. They were not
  run in this work. The pass reads `PrefixKey::inline` (the `prefix` member
  at offset 24) of keys at positions that are symbolic in those harnesses,
  including `keys.get(middle)` in the binary search, so they can meet the
  same dereference; this is not checked. One-key harnesses, where every
  position is 0, verify, which is consistent with the trigger but does not
  confirm it.
- The `windows(2)` hypothesis: (c) verifies, so the symbolic resumption of
  the pair loop alone does not reproduce the fault.
- `resumed_branch_pass_equals_fresh_pass` ("too many addressed objects" at
  object bits 12) and the out-of-memory playbacks are not addressed.
- Case 1 (`FixedTree` width-one chains) has no reproduction here.
- The exact CBMC code path is inferred from the expressions and the source,
  not traced in a CBMC build. A fix was not written or tested.

## Suggested next steps

1. In a throwaway copy of the crate under `target/`, rerun
   `resumed_leaf_pass_equals_fresh_pass` with `PrefixKey` changed only to
   `#[repr(C)]` with `prefix` first (offset 0). If the harness then verifies,
   the product failure is this CBMC fault; if not, another cause remains.
   This copy must not become a product change.
2. Search the product harness's `--show-vcc` output (streamed through a
   filter; it is large) for `..prefix[[0]] }[` indexed by a non-constant
   expression, which marks the wrong dereference.
3. Build CBMC 6.11.0 from the fetched source in a scratch directory, confirm
   the branch with a debugger on `tools/kani-repro/cbmc-c/member_array.c`,
   and test a fix that recurses into the inner array only when the remaining
   offset is known to stay inside it (otherwise return no subexpression, so
   the byte-extract path is used).
4. Report the C reproduction to diffblue/cbmc, linking #8128 and Kani #5104,
   once the user approves posting.
