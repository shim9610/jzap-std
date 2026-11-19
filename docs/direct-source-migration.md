# Direct product-source proof migration

Remaining Vec caller/owner work: [current projection-removal checkpoint](verification/vec-projection-removal.md).

On `main`, the current four-profile source/link certificate covers the
[verified Splice batch implementation](verification/vec-splice-batch.md#verification).
The [ExtractIf report](verification/vec-extract-inline.md) records the measured
default-build optimization. The [handoff](next-session.md) retains the earlier checkpoints.

The migration replaces Python-extracted executable bodies with verification of
the Rust code used by the library. Python may select commands, enumerate source
sites, record results and prepare negative controls. It does not translate a
positive proof input. A Rust proof-only copy of a product function is not a
replacement for direct verification.

## Contract groups

| Group | Existing projection | Replacement and input domain | Status |
| --- | --- | --- | --- |
| Primitive arithmetic | Removed extraction/generation in `verify_arithmetic.py` | Original `integers!` methods for all 12 types and generic `Checked` add/sub/mul with mathematical contracts; actual-Rust checked/wrapping/division/remainder/operator harnesses | Direct contracts, API/control, codegen and link checks pass |
| Initialization slot | Former slot section of `verify_verus.py`, removed `slot.verus.rs` | Actual `InitSlot` methods through Kani; arbitrary payload bits, write/reject/read/finish/drop paths; typed memory scenarios remain separate | Direct checks pass; projection removed |
| Nested array reuse | Former array-reuse section of `verify_verus.py`, removed `array-reuse.verus.rs` | Product length/count function through Verus for every machine-word source length, destination length and array width; actual trait adapter through Kani | Direct checks pass; projection removed |
| Local budget | Removed budget-expression extraction and `credit_projection.py` | Four runtime numeric functions with direct dual specifications; actual Rust Cell/RAII transitions and another-owner frame; logical ledger calls the original numeric functions | Direct contracts, source controls and native accounting pass |
| Array/slice initialization prefixes | Removed three extractions and `array.verus.rs`; removed Box-prefix loop projection | Shared executable Rust cursor with all-word Verus contracts and call history; actual slot leaf and typed cleanup through Kani | Direct contracts, callers, memory and repeated timing checks pass |
| Collection buffer admission and reserved spans | Removed `verify_verus.py`, `buffer.verus.rs`, reserve-body and span-loop generation | Original reservation arithmetic and stop classifier through Verus; actual cursor and typed/word-state Rust callers | Direct checks, owner replay and repeated timing pass; outer growth model remains |
| Closed compaction/extraction/iterator states | Former body projection in `verify_state_types.py` | Contracts on the original `Checked`, `Compaction`, `Extraction` and `Span` methods; maintained adapters call those methods for remaining caller models | Direct contracts, nine solver controls, five compiled scope controls and owner composition pass |
| Vec required length and growth | Removed length/growth extraction in `verify_vec_proofs.py` | Original `required_len` and `growth_capacity`, including the actual checked operator and exact Capacity error contract | Direct contracts and source controls pass |
| Vec layout-aware growth | Removed ceiling/minimum extraction and the copied growth branch in `verify_wrapping_memory.py` | Actual shared array ceiling and size-aware growth proposal through Verus; actual storage publication and existing layout/reservation checks through Kani | Direct contracts, seven source controls and fresh caller replay pass |
| Vec range normalization | Removed range guard extraction and model body in the Vec/panic-state pipelines | Original endpoint functions and shared range validator through Verus; actual generic RangeBounds adapter through all-word Kani, including callback order | Direct contracts and source controls pass |
| Vec array flattening | Removed `FLATTEN_BODY` lowering and the copied shape algorithm/lemma | Original runtime shape function for every valid word-sized length/capacity/limit/width/element-size combination; actual ZST owner adapters | Direct contracts and four source controls pass |
| Vec scalar queries and length policies | Removed remaining-limit and spare-capacity substitutions and their two copied numeric bodies | Original Rust queries on arbitrary valid ZST metadata, both sealed policies; original policy methods and used subtraction through direct Verus | Direct checks, five controls and fresh caller replay pass |
| Vec owner metadata | Removed empty-length/capacity, set-length and std-adoption substitutions | Original capacity interpretation/admission through Verus; original constructors, length publication and ZST transfers through all-word Rust Kani | Direct contracts and seven source controls pass; fresh caller replay pass |
| Vec append, rollback and extraction cleanup | Removed `verify_vec_proofs.py` and `vec.verus.rs` | Actual append/suffix arithmetic through Verus; actual owner transitions and typed extraction moves through Kani | Direct contracts, seven source controls and fresh caller replay pass |
| Vec retain/dedup control | Removed loop lowering, old effect functions and pointer-expression projections | Actual generic runtime control in both word widths, arbitrary native-word owners and all byte-valued prefixes through length four | Direct proofs, caller controls, Miri/ASan, link inventory and repeated std comparison pass |
| Vec ExtractIf scan and movement control | Removed extraction loop lowering and pointer-expression projections | Original executable two-phase scan in both Verus word domains, actual all-word iterator/error transition and all byte-valued prefixes/ranges through length three | Direct proofs, content comparison, caller/source defects and executed memory checks pass; repeated std comparison recorded |
| Vec splice fill | Removed slice-loop lowering and fill-count/increment projections | Original shared initialization cursor in both Verus word domains; actual all-word Splice owner/error/tail checks and existing typed initialization checks | Direct proofs, caller controls and executed memory checks pass; repeated std comparison recorded |
| Vec splice excess collection | Removed `STAGED_LOOP_BODY` lowering and its pointer-expression projection | Executable batch controller and shared initialization cursor in both word domains; actual Rust Kani staging and typed caller checks | Implemented at `42afdab` and merged in PR #5. Direct contracts, executed memory checks, caller composition and the four-profile certificate pass. [Evidence and comparison](verification/vec-splice-batch.md) |
| Buffer outer growth and remaining Vec edit/caller composition | Owner/callsite and outer collection projections | Actual remaining loops and callers; preserve arbitrary valid cursors and both word widths | Pending |
| Array layout and reconstructed capacity bytes | Removed layout ceiling/rejection/product and capacity-product substitutions in `verify_panic_state.py` | Runtime array-byte admission through direct Verus; actual generic Layout adapter and Capacity constructors/reconstruction through all-word typed Kani | Direct contracts, six source controls and fresh caller replay pass |
| Pointer guards and pinned core guards | `verify_pointer_checks.py`, `btree_core.py`, caller projections | Product callers proved against the documented core conditions; the pinned core predicate text stays a projection, see [the interface](#documented-core-conditions-2026-11-10) | Vec owner, scratch, binary heap, VecDeque, B-tree, String edits and hash-table arrays end in the documented conditions, see [the unit](#area-callers-end-in-documented-core-conditions-2026-11-12); shared ownership payload pointers follow in [the next unit](#shared-ownership-callers-2026-11-12); the hash-table byte allocation and nine projected Vec callers pending |
| String cursor arithmetic | Removed seven product-expression substitutions and native/u32 body rewriting | Runtime cursor advance, retain restoration and drain geometry through direct Verus; actual Rust owner/error/consumption checks | Direct contracts, six source controls and String caller/core replay pass |
| Shared count admission and retirement words | Removed scalar body bindings and native/u32 model-body rewriting | Original MAX_COUNT/retirement constants and three executable functions, both default/reduced configurations and word domains; actual local/atomic callers | Direct contracts, six source controls and shared/Loom replay pass |
| BTree search and packed owned cursors | Removed binary-step and cursor-expression extraction and native/u32 model-body rewriting | Actual search midpoint/advance and packed coordinates in both Verus word domains; actual interval, Cursor and NodeSlice Rust callers | Direct contracts, six solver controls and BTree caller replay pass; focused memory and repeated std comparison pass |
| BTree allocation and retirement counts | Removed leaf/branch deficit and retirement-successor expression extraction | Actual all-word deficit/successor and two-field NodeCounts observation/retirement through Verus in both word domains; actual Rust field/frame checks | Direct contracts, six solver controls and fresh caller/memory replay pass |
| BTree iterator cleanup and borrowed leaf edges | Removed two iterator arithmetic body extractions | Actual skip/leaf kernels and shared bounded arithmetic in both word domains; actual all-word composition and byte-leaf iterator/cleanup callers | Direct contracts, six solver controls and fresh caller/Miri/ASan replay pass |
| BTree deletion decisions | Removed both deletion-condition body extractions | Actual capacity constant and three executable guard predicates, exact prior-condition equivalence in both word domains; native all-word arithmetic, retained node kernels and Miri | Direct contracts, six solver controls and fresh caller/Miri/ASan replay pass |
| BTree packed output shape | Removed copied `usize`/`u32` Kani bodies | Actual Rust loop through Verus in both word domains, including checked sums, termination and height; actual native Kani | Direct contracts, six source controls and fresh caller/Miri/ASan replay pass |
| BTree node spans | Removed all seven node-expression extraction groups and their Python lowering | Actual insert/split/suffix/remove/merge/left-borrow/right-borrow span producers; both word domains and unchanged all-occupancy typed Rust harnesses | Direct contracts, actual Rust/caller controls, native/Miri/ASan and repeated std comparison pass |
| String callers/core, shared ownership and trees | Domain projection generators | Original scalar/state kernels, actual Rust callers and atomic schedule checks | Pending |
| Hash-table phase contracts and global composition | Separate phase model and `source_certificate.py` | Actual control publication/probe/clone paths and direct-proof receipts attached to retained link guards | Table-kernel callers of `raw.rs` and `group.rs` bound, see [the unit](#hash-table-kernel-callers-through-product-contracts-2026-11-11); position callers in `hash_map.rs`, `iter.rs`, `entry.rs` bound, see [the follow-up](#hash-table-position-callers-2026-11-11); numeric part of the ownership/credit callers bound with stated premises, see [the follow-up](#hash-table-ownership-and-credit-callers-2026-11-11); credit and tag/ownership projections remain. The [four-profile certificate](verification/vec-splice-batch.md#verification) passes with the retained projections |

Existing direct targets for bounded arithmetic, shared-byte admission, hash-table
geometry, heap geometry, deque geometry and scratch/slice geometry are retained.
They include original product modules rather than generated bodies.

## Primitive arithmetic migration

The 36 mathematical add/sub/mul contracts now annotate the original `integers!`
methods in `src/arithmetic.rs`. The executable bodies are unchanged. Verus reads
`src/lib.rs` directly with `--no-cheating`; Python no longer extracts the methods
or emits `arithmetic.generated.rs` and a substituted wrong-add program.

Eleven actual-Rust harnesses cover all primitive input bit patterns for checked,
wrapping, division/remainder and `Checked` forwarding. The four Verus source
controls alter add/sub/mul or insert a panic. Six Kani controls alter wrapping,
zero-divisor handling or forwarding; these negative controls use concrete `u8`
counterexamples. The positive domains are unchanged. Scalar Kani uses the bundled
Z3 on `PATH`; the state/memory groups retain CaDiCaL. The obsolete fixed-size
full-suite receipt option has been removed from the arithmetic driver.

The arithmetic driver also passes release, overflow-checked release and debug
code-generation/link checks: optimized public wrappers match native checked
controls, 1,088 boundary operations run per profile, and intentional panic link
controls are detected. This campaign does not introduce new timing measurements.

## Checked operators and Vec length/growth follow-up

The original generic `Checked` add/sub/mul bodies now verify against mathematical
contracts. A verification-only extension of the empty sealed marker supplies
integer values, limits and an inverse conversion. Each of the 12 primitive
implementations proves its value range and conversion round trip. Native Cargo
builds retain the original empty marker and executable trait signatures.

The three standard-operator specifications accept every input (`*_req` is true)
and require the result to equal their specification (`obeys_*_spec` is true).
The original implementation bodies discharge both that obligation and the
explicit mathematical value/overflow conditions. The private Checked field
retains its representation and visibility. Python audits the enabled declaration
configuration without translating a function body.

`Vec::required_len` and `growth_capacity` now carry direct contracts. The former
proves the checked sum, element limit and exact `Error::Capacity` refusal; the latter
admits every current capacity when the requested length fits the ceiling. The
error-mapping closure has its own return contract. Its expression-attribute
feature is enabled only for Verus; stable Cargo builds erase that annotation.
Three Python substitutions and two model bodies have been removed. Maintained
caller adapters invoke these actual functions.

The positive domains remain all input bit patterns. Actual-Rust checks include
all 12 primitive types and the original Vec helpers. Product-source controls
cover dispatch, the standard-operator result, the numeric model, length limits,
growth clamping, error kind and a hidden panic. A changed numeric model is
rejected while proving the executable implementations, rather than accepted as
an independent model result.

Sixty public Checked wrappers and two Vec helper controls have identical release
and overflow-checked release assembly, instructions and relocations to `69b73c8`.
The arithmetic driver passes its release, overflow-checked release and debug ABI
and final-link checks. Native/doctests (369), core/all-feature library Clippy,
formatting, Rustdoc and runner checks (26) pass. This follow-up changes proof
metadata and names the otherwise ignored error argument; owning layouts are
unchanged. A fresh owner-protocol campaign passes without prerequisite reuse,
including its 18 actual-Rust memory harnesses and defect controls.

## Vec range normalization follow-up

The lower and upper endpoint functions now carry direct contracts on the code
called by `checked_range`. Included, excluded and unbounded endpoints retain
their mathematical meaning; overflow returns exactly `Error::Bounds`. The shared
slice range validator proves the accepted pair and exact refusal condition.
Three extracted guard substitutions and the copied range model are removed;
remaining caller models invoke the original validator through a maintained adapter.

The actual generic RangeBounds adapter is checked with arbitrary machine-word
lengths and endpoints. Observable getters check one lower call, then one upper
call only if lower normalization succeeds. An eager upper-getter mutation reaches
a solver counterexample. Six direct source controls cover both endpoint offsets,
range admission, length admission, growth clamping and error mapping; the actual
Rust hidden-panic control remains enabled. No callback implementation is assumed
to have a Verus contract.

Native tests/doctests (369), library Clippy in core/all-feature configurations,
formatting, Rustdoc and runner checks pass. The 16 memory scenarios pass release,
overflow-checked release, ASan and Miri on x86-64, i686 and Tree Borrows. The
whole-library source/link replay remains pending.

Partial reproduction:

```sh
bash scripts/verify.sh --area vec --check direct-source
# Only the length/growth/range group:
python3 scripts/verify_source.py --group vec-geometry --mutations
```

The range change preserves six standard range-form code-generation controls
exactly in release and overflow-checked release. The dynamic `(Bound, Bound)`
control changes from 105 to 110 bytes: its upper-bound dispatch tests Unbounded
and Excluded before Included, instead of testing Included first. Both source
versions still execute checked endpoint additions and reject invalid ranges.

The dynamic-pair diagnostic compared `9e3301d`, the migrated implementation and
Rust 1.95 std's range normalizer, on AMD EPYC 9V74. Three seeds prepare 4,096
valid descriptors per condition; each implementation executes the same
640,000,000 calls per interval, in rotated order. All 180 accepted intervals are
at least 1.413 seconds. No compiler or solver runs during timing. Results include
the common opaque call and descriptor loads, with no allocation in the timed loop.

| Lower / upper | std ns/call | Previous ns/call | Current ns/call | Current/std | Current/previous |
| --- | ---: | ---: | ---: | ---: | ---: |
| Included / Included | 2.640 | 2.688 | 2.742 | 1.039 | 1.020 |
| Included / Excluded | 2.629 | 2.368 | 2.397 | 0.912 | 1.012 |
| Included / Unbounded | 2.621 | 2.346 | 2.254 | 0.860 | 0.961 |
| Excluded / Included | 2.371 | 2.811 | 2.925 | 1.234 | 1.040 |
| Excluded / Excluded | 2.429 | 2.648 | 2.570 | 1.058 | 0.971 |
| Excluded / Unbounded | 2.407 | 2.526 | 2.440 | 1.014 | 0.966 |
| Unbounded / Included | 2.392 | 2.575 | 2.648 | 1.107 | 1.028 |
| Unbounded / Excluded | 2.496 | 2.514 | 2.537 | 1.017 | 1.009 |
| Unbounded / Unbounded | 2.381 | 2.329 | 2.278 | 0.957 | 0.978 |
| Mixed (nine combinations) | 3.629 | 3.633 | 3.703 | 1.020 | 1.019 |

The matched 64-byte function-alignment diagnostic gives current/previous means
of 0.982–1.058 across the nine fixed combinations and 1.029 for mixed input.
The Excluded/Included condition remains 1.232 times std there (default: 1.234),
so function alignment does not remove its branch-path difference. In the default
build the current implementation adds 0.114 ns over the preceding implementation
for that condition, and 0.554 ns over std; no element traversal is involved.
Product alignment flags, allocation behavior and owning layouts are unchanged.
[Compact totals and seed deviations](verification/range-source-timing.csv) retain
all conditions; means are total seconds divided by equal call counts.

The maintained analysis command builds every binary before timing. Only the
benchmark consumer enables the unstable diagnostic `std::slice::range` API; the
product libraries compile on stable Rust. Use a fresh output directory:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_range.py --baseline 9e3301d \
  --output target/verification/range-reproduction
# --profile default or --profile aligned selects one configuration.
# Raise JZAP_RANGE_BATCHES if any interval is shorter than one second.
```

## Vec array-flattening follow-up

The public `try_into_flattened` method now calls `vec/geometry.rs::flatten` for
its numeric admission. The direct contract retains arbitrary length, capacity,
element limit, array width and element size, under the existing valid-owner and
physical-layout preconditions. Success preserves allocation bytes and establishes
ordered output length/capacity within the original element limit. Zero-sized
outputs retain the limit as capacity; zero-width arrays with nonzero-sized output
elements produce zero length/capacity. Refusal retains the original owner.

Python no longer extracts, rewrites or substitutes the conversion body. Its
remaining read-only binding checks the original call arguments and subsequent
raw ownership transfer. The maintained proof adapter calls the actual numeric
function. The duplicated flattening implementation and shape lemma are removed.
Three direct source controls break multiplication, capacity admission and the
zero-sized capacity rule. An actual-Rust control passes the wrong array width;
all four reach solver failures. The actual adapter harness checks arbitrary
zero-sized lengths and limits at widths 0, 1, 3 and `usize::MAX`. That typed
selection supplements the all-width numeric contract.

```sh
python3 scripts/verify_source.py --group vec-conversion --mutations
# The common Vec direct-source selection includes this group.
bash scripts/verify.sh --area vec --check direct-source --plan
```

The checkpoint passes 369 native/doctests in debug, release and overflow-checked
release, library Clippy, formatting, Rustdoc and runner checks. The existing
conversion scenario passes x86-64/i686 strict-provenance Miri, Tree Borrows and
ASan. A fresh owner-protocol campaign passes without prerequisite reuse,
including its 18 original-Rust memory harnesses and defect controls. This also
rechecks the preceding range migration's caller composition. The shared range
kernel's review fingerprint was updated after its eight actual-Rust scratch/slice
harnesses and six compiled defect scenarios passed.

At Rust 1.95 release and overflow-checked release settings, 12 of 16 public
flattening wrapper text sections have identical machine bytes to `a05e61e`.
Two u64-array wrappers shorten by three bytes with different register allocation.
Two limited ZST wrappers change overflow/limit predicate combination. No new
allocation, element loop or data movement is introduced.

The public ZST diagnostic constructs an owner, admits its prepared logical
length and flattens it. Three seeds use equal 1,024,000,000 calls per
implementation/condition, rotated order and 4,096 randomized descriptors. All
36 intervals exceed 1.730 seconds. std has no element-limit policy; limited
conditions include the product's additional admission. Default compiler flags
are retained; no solver/compiler runs during timing.

| ZST array width / policy | std ns/call | Previous ns/call | Current ns/call | Current/std | Current/previous |
| --- | ---: | ---: | ---: | ---: | ---: |
| 3 / unlimited | 1.753 | 1.736 | 1.755 | 1.001 | 1.011 |
| 3 / limited | 1.739 | 2.153 | 2.077 | 1.195 | 0.965 |
| usize::MAX / unlimited | 1.729 | 2.104 | 2.054 | 1.188 | 0.976 |
| usize::MAX / limited | 1.738 | 1.746 | 2.005 | 1.153 | 1.148 |

In the last condition, the compiled consumer's successful path changes from an
11-instruction branch form to a 12-instruction conditional-move form. After
forming the same two predicates, the previous code branches to return its
already-computed length. The current code zeros the return length and uses
`cmovne` after the combined predicate. Its return value depends on that move;
the observed change is +0.258 ns/call. This caller has no element traversal.
The [compact totals and seed deviations](verification/flatten-source-timing.csv)
retain all four conditions.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
JZAP_RANGE_BATCHES=250000 python3 tools/throughput-analysis/prepare_range.py \
  --benchmark flatten --baseline a05e61e --profile default \
  --output target/verification/flatten-reproduction
```

## Array layout and capacity-byte follow-up

`layout::array<T>` now calls the actual `layout/numeric.rs::array_bytes` function.
Its direct contract covers every machine-word count and element size: accepted
bytes equal the mathematical product within `isize::MAX`; every refusal is
exactly `Error::Layout` and the product exceeds that limit. The original ceiling,
checked division and checked multiplication remain executable product code.
Proof-only division/remainder lemmas establish the ceiling implications.

The typed Rust harness compares the complete public Layout result against
`core::alloc::Layout::array`, for every count at seven types: u8, u64, odd-sized
byte/word arrays, unit, aligned payload and aligned unit. A second original-Rust
harness verifies Capacity construction and reconstructed layouts. These checks
include acceptance, rather than checking only successful returned values.
The full count/size numeric domain is supplied by direct Verus, not replaced by
that typed selection.

Four Python substitutions and two copied numeric function bodies are removed.
Maintained adapters call the runtime byte admission and the same verified bounded
multiplication used by Capacity. Read-only bindings check the actual caller
arguments. Three Verus source defects alter rejection, product and error kind;
three actual-Rust defects add a hidden panic, introduce a spurious refusal or
change Capacity's real arithmetic callee. All six reach solver failures. The
superseded layout projection controls are replaced by these direct controls.

Partial reproduction:

```sh
python3 scripts/verify_source.py --group layout --mutations
bash scripts/verify.sh --area foundation --check direct-source --plan
```

The layout checkpoint passes native/doctests (369) in debug, release and
overflow-checked release, library Clippy, formatting, Rustdoc and runner checks.
All 37 foundation/movement/wrapping scenarios pass x86-64/i686 strict-provenance
Miri, Tree Borrows and ASan. A fresh owner-protocol run passes without
prerequisite reuse, including its 18 actual-Rust memory harnesses and source
defects. The adapter is registered in the combined numeric/ownership namespace
audits. The final whole-library source/link replay remains pending.

Nine of eleven public layout/allocation caller controls have identical release
and overflow-checked release assembly to `7a57f33`, including byte Vec, limited
byte Vec and Box-copy allocation callers. The standalone byte Layout result
materializes alignment through a register and writes an unused error-payload
word; its successful path gains one move. The scalar byte-size wrapper removes
one move only on refusal; its successful path is unchanged. The six other typed
Layout wrappers are identical. These are code-generation comparisons; no new
throughput measurements are attached to this layout checkpoint.

## Vec scalar-query follow-up

The remaining-limit and spare-capacity models now call the same bounded
subtraction used by the product. Python no longer emits their arithmetic;
read-only bindings check the three original query bodies. The original
`LengthLimit::new` and both sealed `max_len` implementations also have direct
contracts. A closed proof-only view describes the private configured limit;
its original getter and constructor prove consistency with that view.

Actual-Rust Kani executes `remaining_limit`, `spare_capacity_mut` and
`split_at_spare_mut` with every valid word-sized length/capacity/limit triple on
unit-element owners. Both Unlimited and LengthLimit execute, including exact
limit lookup, spare partition lengths and unchanged metadata. The zero-sized
storage admits these large numeric states without allocating bytes. The fixture
constructs internal numeric states satisfying length ≤ capacity ≤ limit; it does
not use the stronger public raw-parts constructor contract. See the owner-metadata
follow-up for the fixture correction. Existing
nonzero-element memory scenarios cover their separate pointer/aliasing domain.
The native Kani word width is 64; direct subtraction and policy contracts retain
both 32/64-bit numeric domains.

Three direct source controls change the unlimited value, configured getter and
configured view. Two actual-Rust controls change remaining-limit arguments and
spare length. All five reach solver failures. Product executable bodies, pointer
fields, representation and allocation behavior are unchanged. Nineteen public
query/policy wrapper assemblies match `1776b7b` in release and overflow-checked
release. Native/doctests (369), library Clippy, formatting, Rustdoc, runner tests
and the arithmetic-policy checks pass.

```sh
python3 scripts/verify_source.py --group vec-queries --mutations
bash scripts/verify.sh --area vec --check direct-source --plan
```

A fresh owner-protocol campaign also passes without prerequisite reuse, including
its 18 actual-Rust memory harnesses and defect controls. The sealed-policy audit
retains the exact original implementations and now recognizes their proof-only
annotations. The whole-library source/link replay remains pending.

## Local budget migration

Local `Cell`/RAII transitions stay in `MemoryBudget`, `Reservation` and `Charge`.
Their all-word actual-Rust checks are retained and extended with another-owner
frame. Four small executable numeric functions supply pending-byte queries,
admission, credit and retirement. The runtime calls those same functions.
Their Verus-generated dual specifications let the logical credit ledger invoke
the verified arithmetic without Python substitution or a handwritten execution
copy. Budget fields, release order and handles remain unchanged. Validation
includes source defects, original-Rust accounting, current memory scenarios and
an assembly comparison with the preceding product revision.

The budget partial runner passes four actual-Rust harnesses: construction and
nested reservation, arbitrary-state commit/cancel, owned release, and another-owner
frame. Four direct Verus source controls and the compiled admission-panic control
are rejected. The replaced budget and credit-expression generators/templates
are removed. Python retains only command selection, receipts and read-only
identity/order audits.

Native tests and doctests (369), library Clippy in core/all-feature modes,
formatting, Rustdoc and runner tests (26) pass. The budget memory selection passes
11 scenarios in each of release, overflow-checked release, ASan and the three
Miri settings (x86-64, i686 and Tree Borrows). A fresh owner-protocol campaign
passes without prerequisite reuse, including the 18 actual-Rust memory harnesses
and source defect controls.

Six public budget wrappers have identical assembly to `e379c67` in release and
overflow-checked release. Debug output differs after factoring the numeric functions; this campaign
does not include debug timing measurements. Budget field and handle layouts are unchanged. The final whole-library
source/link replay remains part of the pending migration.

## Initialization and nested-array checkpoint

`scripts/verify_source.py` selects groups without generating Rust bodies. The
slot group runs three original-Rust harnesses. They check arbitrary `u64` payloads,
rejected-value preservation, reads, initialized/uninitialized completion, cleanup
and an exact destructor handoff. They are concrete typed-memory checks, not a
new generic Rust provenance theorem.

The clone group runs two original-Rust harnesses and Verus on the executable
`clone/geometry.rs` function called by the array trait implementation. Verus
checks every source length, destination length and width, including multiplication
overflow. Kani checks the actual adapter with arbitrary word-sized logical
lengths at widths 0, 1, 3 and `usize::MAX`, plus nonzero `u8` data and mismatch
preservation. Those adapter instantiations supplement the all-width Verus
contract; they do not replace its universal width domain.

Five product-source defects were detected by solver failures: lost slot write
state, incorrect finish disarm, wrong width passed by the trait adapter, inverted
length admission and unchecked overflowing multiplication. Compiler errors are
rejected as negative-control results. The selection/failure runner tests pass
(25 tests).

The checkpoint also passes native, release, overflow-checked release, Clippy,
arithmetic-policy, remaining foundation projections and their mutation controls;
foundation memory scenarios pass strict-provenance Miri on 64/32-bit targets,
Tree Borrows and AddressSanitizer.

The product change moves the existing length/count admission into one inline
function. At Rust 1.95 release settings, five of six array-adapter assembly
controls match the preceding product revision: `[u8; 0]`, `[u8; 3]`, `[u64; 2]`,
`[(); 0]` and `[(); 1]`. The `[(); usize::MAX]` control changes branch/result
materialization: a conditional move becomes an early mismatch branch, with
11 versus 12 static instructions. Equal lengths 0/1 still execute nine
instructions, and the overflow path still executes ten. This is a code-generation
comparison, not a new throughput measurement. The bodies of `InitSlot` are
unchanged; its new code is compiled only under `cfg(kani)`.

## Closed numeric state migration

`Compaction` and `Extraction` retain their private `Checked<usize>` fields and
the same executable method bodies. Contracts are attached directly to their
24 product methods. The original `Checked::new` and `Checked::get` bodies also
have direct contracts; Verus does not erase the wrapper through a Python
translation. Proof views describe the private values without changing runtime
layout or granting runtime field access.

The former extraction/lowering generator in `verify_state_types.py` has been
removed. Its replacement compiles `src/lib.rs` through Verus with `--no-cheating`.
Temporary maintained call adapters let the remaining caller models use the
original state methods. Each adapter body is a call, not a copied transition.
The caller models themselves still have their listed projection boundary.

Verus's pinned frontend uses the `Either32Or64` word domain when no global width
is selected ([frontend initialization](https://github.com/verus-lang/verus/blob/8bf68ee741df833ea5d0fe6a2255126fdea87e47/source/rust_verify/src/rust_to_vir.rs#L581)).
Two controls insert assertions fixing `usize::MAX` exclusively to the 32-bit or
64-bit maximum; both are rejected. The actual state/caller composition therefore
uses this word domain rather than replacing `usize` with `u32` in generated
text. This is a numeric proof domain, not a 32-bit final-link test.

Four other product-source controls break restored length, extraction stop,
one-past progress or insert an unmodeled panic. All reach solver failures.
The standalone receipt records the exact source hashes, commands and controls.

The three actual-Rust state harnesses use the established memory campaign's
CaDiCaL backend and ten object-identity bits. Machine-word inputs retain their
full width and all safety checks remain enabled.

A fresh `verify_owner_protocol.py --mutations` run passes without prerequisite
reuse, composing the actual state methods with the remaining caller models. Its
prerequisite memory campaign passes 18 actual-Rust harnesses and its negative
controls. Native tests (369), Clippy, formatting and runner tests (26) pass.
Six public nested-array code-generation controls are identical to `196fe03`;
the state migration adds proof annotations and ghost code only. The global
source/link certificate has not yet been rerun for this checkpoint.

The Vec restoration queries now call the same product methods instead of
extracting their bodies again. Proof-only constructors establish arbitrary valid
numeric states of the real private types; they do not replace the restoration
algorithms. The mixed Vec check passes with nine source defects rejected,
including the wrong extraction destination. Other Vec operations in that check
still use their listed projections.

The independent `syn` declaration census checks all 24 method signatures and
their attached contracts, private field types, macros and module boundaries.
It does not translate executable bodies. Five source copies compile and are
then rejected for an unregistered constructor, hidden constructor macro, exposed
field, new module or invalid state constant. These are scope checks, separate
from the six solver counterexamples. Default state-site inventory output is
unchanged; the declaration-only mode can be inspected independently:

```sh
cargo build --locked --offline --manifest-path tools/panic-audit/Cargo.toml \
  --bin state_sites --target-dir target/panic-inventory
target/panic-inventory/debug/state_sites --declarations src/compaction.rs
```

The earlier `37f4f27` checkpoint passes all eight migrated actual-Rust harnesses, both direct clone
controls, all state controls, native tests (369), Clippy, formatting and runner
tests (26). A fresh owner campaign passes without prerequisite reuse. The six
code-generation controls match `7213939`. Logs, generated caller inputs and
compiler outputs remain under the ignored verification directory.

## Owning-iterator span follow-up

Seven original `Span` methods now have direct contracts for ordered endpoints,
construction/refusal, clearing, front/back retirement and exact remaining length.
The private `Checked<usize>` fields retain their representation. The two endpoint
comparisons read the wrapped integers explicitly. Normal builds exclude the
proof-only view and arbitrary-state constructor.

Four cursor-expression substitutions and the two handwritten cursor model
functions have been removed from the panic-state pipeline. Remaining memory
callers invoke the product methods through maintained call adapters. The
independent declaration gate now covers 31 methods across all three state types.
Its nine solver controls include wrong clearing, one-past progress and reversed
retirement in `Span`; the five compiled scope controls remain enabled. Five
actual-Rust state harnesses cover the constructors and inductive transitions.

The 12 public Vec/Box owning-iterator controls cover front/back reads and length
for `u64` and unit payloads. Release and overflow-checked release assembly,
instructions and relocation targets match `c194f37`. Native/doctests (369),
core/all-feature library Clippy, formatting and runner tests (26) pass. The
16 unsafe/movement scenarios pass each of x86-64/i686 strict-provenance and
Tree Borrows Miri. Python emits only the maintained state-call include path;
it does not generate these cursor bodies. A fresh owner-protocol campaign passes
without prerequisite reuse, including its 18 actual-Rust memory harnesses and
source defect controls. The final global source/link replay remains pending.

## Acceptance and reproduction

```sh
# The maintained runner; each group can also be run independently.
bash scripts/verify.sh --area foundation --check direct-source
python3 scripts/verify_source.py --group slot --mutations
python3 scripts/verify_source.py --group clone --mutations
python3 scripts/verify_source.py --group state --mutations
python3 scripts/verify_source.py --group budget --mutations
python3 scripts/verify_source.py --group vec-geometry --mutations
python3 scripts/verify_direct_verus.py --target local-budget --mutations
python3 scripts/verify_direct_verus.py --target clone --mutations
```

Receipts and solver logs are written under `target/verification/direct-source/`
by the standalone runner, or under the selected common runner output directory.
They contain source fingerprints, exact commands, harness names and outcomes.
Raw outputs and temporary mutant sources are not committed.

Each migrated group must retain its old success/error conditions and state
invariants. An all-word arithmetic contract requires an all-word replacement;
a finite typed-memory scenario cannot replace it. Generic payload ownership,
allocator behavior and arbitrary callbacks are separate from numeric contracts.

Positive checks must pass and a corresponding product-source defect must compile
and fail in the solver. Compilation failure is not a successful negative control.
The runner must reject missing results and source changes during a run. Only
then may the obsolete generator and template be removed from that group's path.

The global certificate still consumes the pending projections in the table.
Its prior pass is not a certificate for a later source revision. Migration
checkpoints report their own checked scope until the global composition has
been converted and rerun.

## Shared array/slice initialization migration

`InitSlot::try_init_array`, `Box::try_init_slice_in` and array `TryClone` now use
one executable Rust initialization driver and one physical prefix guard. The
runtime cursor accepts an admitted start/end pair and advances only after its
step returns success. Its original loop carries a decreasing bound and a ghost
history: every completed index is contiguous and records the successful
postcondition of the callback state used for that invocation. Ghost history and
proof annotations are erased by normal Cargo builds.

Verus verifies that original loop in both word domains. It proves exact
completion, stopping below the bound on failure, and the arithmetic precondition
of its wrapping increment. No array-prefix body is extracted or emitted. The
three uses of `array.verus.rs`, the template itself, and the separate Box-prefix
loop in the panic-state model are removed. Remaining caller compositions invoke
the actual cursor rather than repeat its loop.

The private physical leaf calls the initializer with the admitted index and
requires `InitSlot::finish` before ownership joins the prefix. An actual-Rust
Kani harness covers every machine-word index with zero-sized storage and four
callback outcomes: failure before write, successful write, unwritten success,
and failure after write. Three existing caller harnesses check array values, heap array admission and
Box-slice budget restoration. A second private harness checks initialized values and exact
cleanup at lengths zero through three using nonzero elements with observable
destructors. These finite physical states complement the all-word loop proof;
they do not replace its arithmetic domain. Allocation layout, generic pointer
provenance and callback/destructor contracts retain the existing typed execution
and compiler checks.

Five direct Verus controls change progress, termination, callback index or error
propagation. Seven actual-Rust Kani controls change a caller extent or leaf index,
omit finish, retain a completed prefix after success, or omit prefix destruction. Each must
compile and reach a solver counterexample. The small read-only
`init_bindings.py` checks the three callers and private ownership glue; it emits
no positive proof input. The obsolete `mutate_verus.py` driver is removed: its
prefix controls moved to actual Rust and its remaining budget mutation duplicates
the maintained direct budget control.

Run the migrated group independently:

```sh
python3 scripts/verify_source.py --group init-prefix --mutations
bash scripts/verify.sh --area box --check direct-source
```

The collection buffer, shared-owner prefixes and Vec edit loops remain separate
migration groups. Their existing projection dependencies are unchanged by this
checkpoint.

The three native/doctest profiles pass 369 tests each. Strict-provenance Miri
on x86-64 and i686, Tree Borrows Miri and ASan each pass the 52 foundation/Box
scenarios. Both library Clippy configurations, Rustdoc, formatting, the arithmetic
policy gate and the 26 runner checks pass. The source census routes 585 unsafe blocks; the shared helper replaces three separate guards and
adds two private unsafe signatures.

Numeric, byte-span, actual-memory and pointer prerequisites were rebuilt with
their source controls. The final caller-composition run reused those outputs.
A renewed four-profile global source/link certificate remains pending.

### Initialization code generation and repeated timing

The runtime source is `2a63fcb`; the preceding source is `68417ff`. On Rust 1.95
with LLVM 22.1.2, default and overflow-checked release comparisons show the same
16-element clone and array-slot instruction sequences after excluding relocation
addresses and padding. Dynamic slice loops and the 1,024-element array-slot loop
retain the same instructions with different registers. The large array-clone consumer retains its seven-instruction element loop and
two array copies. Stack displacements shorten that loop from 37 to 34 bytes;
one address calculation outside the loop is removed. Its stack reservation
remains `0x8010` bytes. The change adds no element-loop instruction or heap allocation.

The primary timing campaign runs three corpus seeds in three separate processes,
pinned to logical CPU 0 on the EPYC 9V74. Each implementation receives the same
4,096 randomized input values and call count within each condition. Invocation
order rotates between std, previous and current. All 54 measured intervals exceed
3.271 seconds. Small conditions execute 204,800,000 calls per interval; large
conditions execute 6,397,952. The table divides total elapsed time by total calls
across seeds; it does not time individual operations. Both libraries and the
consumer use release optimization, one codegen unit and panic=abort, with default
loop/function alignment. Product Cargo builds retain stable Rust.

| Operation | Elements | std ns/call | Previous ns/call | Current ns/call | Current/std | Current/previous |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Array clone | 16 | 16.230 | 16.453 | 16.711 | 1.030 | 1.016 |
| Array slot | 16 | 16.940 | 17.239 | 16.829 | 0.993 | 0.976 |
| Slice initialization | 16 | 18.976 | 18.650 | 18.685 | 0.985 | 1.002 |
| Array clone | 1,024 | 926.066 | 962.563 | 927.242 | 1.001 | 0.963 |
| Array slot | 1,024 | 616.660 | 611.110 | 629.869 | 1.021 | 1.031 |
| Slice initialization | 1,024 | 608.316 | 593.737 | 602.905 | 0.991 | 1.015 |

[Compact totals and seed deviations](verification/initialization-source-timing.csv)
include the primary process-separated campaign and the initial campaign that ran
all three seeds in one process. The latter has 54 intervals of at least 3.311
seconds; its current/std means range from 0.979 to 1.086. The two campaigns remain
separate in the CSV. The small-slice current/previous mean changes from 1.073 in
the initial campaign to 1.002 in the primary campaign; the corresponding large
means are 0.992 and 1.015. The slice element loops contain the same seven operations and 26-byte length
with register substitutions; the array-slot loop is 30 bytes. These observations do not identify a
specific hardware counter or cache event.

The maintained comparison prepares both source revisions before executing any
timer and validates equal calls, checksums and the one-second floor. To reproduce:

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
JZAP_RANGE_BATCHES=50000 taskset -c 0 python3 tools/throughput-analysis/prepare_range.py \
  --benchmark initialize --baseline 68417ff --profile default \
  --output target/verification/initialization-comparison
```

Choose an available CPU on a different machine and a fresh output directory.
`--build-only --overflow-checks` prepares the symmetric overflow-checked assembly
comparison without timing it. All raw totals, metadata, binaries and disassembly
remain in ignored output storage; Git retains the compact aggregate CSV.

## Collection buffer reservation and span migration

The original Buffer reserve arithmetic now resides in `boxed/buffer_numeric.rs`.
Its contract covers every valid word-sized length/capacity pair, additional count
and allocation hint. It proves the exact required length, capacity reuse, first
allocation hint, doubling, overflow fallback and exact Capacity refusal. The
runtime still validates the resulting typed layout before reallocating and
publishes pointer, capacity and layout only after successful reallocation.

The two bounded collection loops now call `init/collection.rs`, which uses the
same executable cursor as array/slice initialization. Each private leaf asks the
source for one value and writes it before cursor advancement. A normalizer maps
bound completion, source exhaustion and source failure to the original status
and error. That normalizer is itself directly verified for arbitrary error
payloads. Sources remain borrowed through the helper, and private admitted bounds
retain control of the writes.

Four actual-Rust harnesses cover every word-sized unit-owner reserve state and
leaf offset, nonzero typed prefix/consumption/cleanup through capacity three, and
the real allocating public collection path over its established finite corpus.
Five Verus controls alter growth, initial hints, overflow error kind or source
status. Seven Kani controls omit a write, change its offset, change a source
status, pass the wrong reservation argument, or extend either private bound.
All twelve compile and reach solver counterexamples.

The reserve-body and bounded-span projections are removed from the owner and
operation generators. Maintained compositions call the actual numeric function,
cursor and normalizer. `verify_panic_state.py` selects original foundation checks
directly; it no longer calls a generator to obtain budget/prefix/buffer bodies.
The now-unused `verify_verus.py` and `buffer.verus.rs` are removed, including their
runner and prerequisite references. Historical checkpoint descriptions retain
their stated scope. `--area budget --check models` now selects the direct budget
group and identifies the result as direct evidence.

The outer collection/growth orchestration and its allocator/pointer composition
remain caller models. Their migration is separate from these original numeric
functions and bounded spans.

```sh
python3 scripts/verify_source.py --group buffer --mutations
python3 scripts/verify_direct_verus.py --target buffer --mutations
bash scripts/verify.sh --area box --check direct-source
```

Native/doctests pass in debug, release and overflow-checked release (369 each).
The 44 Box/iteration scenarios pass strict-provenance Miri on x86-64 and i686,
Tree Borrows and ASan. Core-only, alloc-only and all-feature Clippy, Rustdoc,
formatting, the 26 runner checks and the updated unsafe census pass. Fresh numeric,
byte-span, 18 actual-memory/state and pointer prerequisites pass their controls;
the final owner-composition/control run reuses those rebuilt outputs. It checks
82 state publications and 193 actual unsafe callers. A renewed four-profile
global source/link certificate remains pending.

The public collection comparison uses valid exact, zero-lower and half-lower
hints from a custom iterator. Its scalar values and lengths are randomized;
small inputs span 8–16 u64 values and large inputs span 512–1,024. A separate
Linux System allocator observer reuses the maintained allocation tracker without
adding instrumentation to the timing binary. Across 216 observations, previous
and current allocation-call counts, retained requests and peak requests match.
All three implementations retain exactly the final payload request and release
it on drop.

[Compact allocation observations](verification/buffer-source-memory.csv) include
all eight boundary lengths and three seeds. With no lower hint, previous/current
start at one element and std starts at four: collection of 16 values takes five
allocation/reallocation calls versus std's three; collection of 1,024 takes eleven
versus nine. Under the half-lower hint at 513 elements, previous/current start at
256, grow to 512 and 1,024, then shrink to 513. Std starts at 257, grows to 514,
then shrinks to 513. The allocator-observed request peaks are 8,192 and 4,112
bytes respectively, with the same final 4,104-byte request. These allocation
sequences are unchanged by the proof migration. The tracker records requested
and usable bytes at GlobalAlloc operation boundaries; it does not trace libc's
internal realloc temporaries.

### Collection code generation and repeated timing

The runtime source is `4c238ba`, compared with `aa2a238` and std on Rust 1.95 /
LLVM 22.1.2. Exact-hint previous/current consumers retain the same seven element
operations and 26-byte loop with register substitutions. In the outlined growing
collector, the previous common loop has 19 instructions over 69 bytes; the current
loop has 13 over 51. Allocation pointer, capacity and source fields are hoisted
or retained in registers rather than repeatedly loaded from the two objects.
The source position is no longer stored back on every element. Growth and
terminal paths remain outside the admitted span. The observation concerns the
compiled concrete custom iterator used by this comparison.

Three independent corpus-seed processes ran on logical CPU 0 of the EPYC 9V74.
All 54 intervals exceed 1.475 seconds. Each implementation executes 102,400,000
calls in each small interval and 3,198,976 in each large interval, with equal
randomized data, checksums and rotated execution order. The table uses total time
divided by total calls across seeds. Both source revisions and the consumer use
release optimization, one codegen unit, panic=abort and default alignment.

| Lower hint | Elements | std ns/call | Previous ns/call | Current ns/call | Current/std | Current/previous |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Exact | 8–16 | 22.161 | 18.705 | 18.586 | 0.839 | 0.994 |
| Zero | 8–16 | 93.389 | 117.983 | 114.726 | 1.228 | 0.972 |
| Half remaining | 8–16 | 50.171 | 73.780 | 74.489 | 1.485 | 1.010 |
| Exact | 512–1,024 | 689.046 | 473.458 | 466.185 | 0.677 | 0.985 |
| Zero | 512–1,024 | 986.324 | 1,237.370 | 967.919 | 0.981 | 0.782 |
| Half remaining | 512–1,024 | 792.625 | 876.307 | 755.742 | 0.953 | 0.862 |

[Totals and seed deviations](verification/buffer-source-timing.csv) retain the
calls and elapsed totals needed to recompute every mean. Small zero/half-lower
conditions retain std differences of +21.337/+24.318 ns per call; the preceding
implementation measured +24.594/+23.609 ns. The allocation observations above
identify the differing initial capacities, growth sequences and extra calls.
Large current conditions measure below std and below the preceding implementation
in this campaign; the growing-loop instruction/field-load reduction accompanies
that result. No allocator policy or default alignment setting changes here.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
JZAP_RANGE_BATCHES=25000 taskset -c 0 python3 tools/throughput-analysis/prepare_range.py \
  --benchmark collect --memory --baseline aa2a238 --profile default \
  --output target/verification/buffer-comparison
```

The timing executable contains no allocation instrumentation. `--memory` builds
and runs a separate System allocator observer; raw intervals, per-seed files and
binaries remain under ignored output storage.

## Vec owner-metadata follow-up

`logical_capacity` and `adopted_capacity` are now executable functions called by
Capacity and the original std-Vec adoption method. Their Verus contracts prove
zero-sized capacity interpretation and exact length/capacity refusal for every
word-sized input, with length ≤ source capacity for adoption. Initialized-length
admission reuses the directly proved `required_len`; the original method still
publishes the length only after admission. Python no longer emits the six
empty-owner, length-update and adoption expressions. Remaining caller models call
these actual functions. The unrelated reserved commits, split and vectored-loop
projections remain pending.

Four actual-Rust harnesses cover arbitrary valid internal numeric owner states,
empty constructors, std unit-Vec adoption/refusal/round-trip, and boxed unit-slice
transfers. Length-update refusal preserves length, capacity, limit and pointer.
Both sealed policies execute. Three Verus and four Kani source controls reach
solver failures. Existing nonzero-element memory checks retain their separate
allocation, initialization and aliasing scope.

The scalar-query fixture previously called the public unsafe raw-parts constructor
with arbitrary zero-sized capacities below the limit. That constructor requires
zero-sized capacity to equal the configured limit. The corrected test fixture
constructs the crate's internal numeric state directly, preserving every tuple
satisfying length ≤ capacity ≤ limit. Its capacity constructor requires a valid
array layout; every unit-element count describes zero bytes. Public constructors
are checked separately and retain canonical zero-sized capacity. This changes
verification setup, not the public runtime contract.

Native/doctests (369) pass in debug, release and overflow-checked release. The 40
Vec scenarios pass strict-provenance Miri on x86-64 and i686, Tree Borrows and
ASan. Core-only, alloc-only and all-feature Clippy, Rustdoc, formatting, the 26
runner checks and the updated unsafe census pass. A fresh caller replay without
prerequisite reuse passes, including its 18
actual-Rust memory/state harnesses and source-defect controls. It checks 82
state publications and 193 actual unsafe callers. The whole-library four-profile
source/link replay remains pending.

```sh
python3 scripts/verify_source.py --group owner-metadata --mutations
python3 scripts/verify_owner_protocol.py --mutations
bash scripts/verify.sh --area vec --check direct-source --plan
```

### Metadata code generation

Twelve public wrappers compare the preceding `9788862` runtime with this change
under release and overflow-checked release. Eight have identical assembly and
relocations. The three full-Result length-update wrappers change from eight
instructions to nine: a conditional branch and success-only owner store become
`setbe`, `cmova` and conditional selection of the store destination. On refusal
the selected destination is an unused Result payload word, while owner metadata
remains unchanged. Byte-element std adoption retains 24 instructions; two `seta`
and an OR become two `setbe` and a TEST, with inverted branch polarity. Unit
adoption, empty owners and boxed transfers are unchanged.

The maintained setter consumer observes success and final length. Its std
reference applies the same explicit logical element bound before std's unsafe
`set_len`; raw std unit storage alone has no such bound. Std, previous and
current consumers merge to a single 13-byte function: XOR, CMP, CMOVBE, SETBE and
RET. This consumer therefore has no additional current-runtime instruction.
The full-Result ABI comparison above remains a distinct observation.

Three independent seed processes compare success-only, refusal-only and random
50/50 requests, with randomized logical limits and lengths. Rust 1.95 / LLVM
22.1.2 uses release optimization, one codegen unit, panic=abort and default
alignment on logical CPU 0 of the EPYC 9V74. No compiler, solver or allocation
observer runs during timing. All 27 intervals last at least 1.723 seconds. Every
interval executes the same 1,024,000,000 calls for each implementation, with
matching results/checksums and rotated order. Means are total time / total calls.

| Requests | std with bound ns/call | Previous ns/call | Current ns/call | Current/std | Current/previous |
| --- | ---: | ---: | ---: | ---: | ---: |
| Success | 1.726 | 1.699 | 1.747 | 1.012 | 1.028 |
| Refusal | 1.729 | 1.745 | 1.724 | 0.998 | 0.988 |
| Mixed | 1.703 | 1.741 | 1.741 | 1.022 | 1.000 |

[Totals and seed deviations](verification/owner-metadata-timing.csv) retain the
calls and elapsed totals for recalculation. All three consumers execute the same
compiled function. Their timing deviations therefore do not correspond to extra
current-runtime instructions in this comparison.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
JZAP_RANGE_BATCHES=250000 taskset -c 0 python3 tools/throughput-analysis/prepare_range.py \
  --benchmark setter --baseline 9788862 --profile default \
  --output target/verification/owner-setter-comparison
```

## Vec nonallocating cursor transitions

`push_within_capacity` calls the executable `append_cursor`; `truncate` calls
`removed_suffix`. These functions retain the original guard and checked
arithmetic. Direct Verus contracts prove exact admission/refusal, one-element
advance and suffix subtraction for every input in both word domains. The rollback
guard still calls the original `truncate`. Compaction/extraction numeric queries
continue to use their directly proved product methods.

Four actual-Rust harnesses check within-capacity publication on arbitrary valid
internal metadata under both sealed policies, rollback to every admitted original
length, exact extraction-drop restoration at every valid native-word cursor, and
nonzero retained-prefix/unvisited-suffix contents for all cursor tuples through
four bytes. Three Verus source controls change admission, advance and subtraction;
four Kani controls change the capacity argument, rollback target, copy destination
and restored length. All seven reach solver failures.

The former Vec generator and template are removed. Publication routing now cites
the actual owner harnesses; the wrapping-memory prerequisites carry their direct
receipts. Legacy Vec model selectors, the extended Vec runner and the final
source/link runner select `vec-cursors`. Remaining edit-loop and owner projections
retain their separate scope. Read-only bindings check the actual helper arguments
and publication order; they emit no proof body.

Eight public push/truncate wrappers compare `284878a` with this change under Rust
1.95 release and overflow-checked release. Each pair has identical instruction
bytes and relocations, including merged aliases. Push takes 17 instructions for
byte/word/limited owners and 12 for unit owners; truncate takes four. These are
code-generation checks; this cursor checkpoint adds no timing campaign.

```sh
python3 scripts/verify_source.py --group vec-cursors --mutations
bash scripts/verify.sh --area vec --check models
python3 scripts/verify_owner_protocol.py --mutations
```

Native/doctests (369 each) pass in debug, release and overflow-checked release.
The 40 Vec scenarios pass strict-provenance Miri on x86-64 and i686, Tree Borrows
and ASan. Core-only, alloc-only and all-feature Clippy, Rustdoc, formatting, the
26 runner checks, arithmetic policy and the unsafe census pass. A fresh caller
replay without prerequisite reuse passes, including the four cursor harnesses,
18 memory/state harnesses and their controls; it checks 82 publications and 193
unsafe callers. The final four-profile global source/link replay remains pending.

## String cursor arithmetic

The original `advance`, `restore` and `drain` computations now live in
`src/string/numeric.rs`, called by retain and its guard, and by Drain::drop.
Their direct Verus contracts cover every admitted word input in both word
domains. Advance preserves the exact nonoverflowing sum; restore returns the
unvisited suffix length and retained final length; drain returns the exact
remaining tail and final length for every valid range. The saturating runtime
operations and order of pointer effects are unchanged.

Seven Python product-expression substitutions are removed. The retained caller
functions invoke these executable kernels. `verify_string.py` composes the
original crate with those calls and the remaining pinned-core models; it no
longer rewrites usize bodies to u32. Pop/character-boundary/core-cursor models
retain their separate core-model scope. Read-only bindings check the actual
kernel arguments and memory-effect order.

Two new actual-Rust memory harnesses use the fixed scalar pairs `(a, 😀)`,
`(é, 한)`, `(한, é)` and `(😀, a)`, covering UTF-8 widths 1–4. They check every
retain/remove/error reply, callback order, exact retained bytes, every selected
scalar-boundary drain range, and optional front/back consumption before drop.
The two existing all-word Edit admission/deletion harnesses are included in the
standalone group. Existing unrestricted Unicode encoder/decoder checks and the
nine String harnesses remain enabled. Three direct Verus defects change advance,
retain restoration and drain length. Three actual-Rust defects change the
written cursor, restore cursor and drain start passed by the product.

Eight public retain/drain consumers compare `45e5b19` with this change using Rust
1.95 release and overflow-checked release. Every pair has identical instruction
bytes and relocations, including aliases: retain-all 106 instructions, retain-none
74, parity retain 117, fallible retain 146 and drain 76. Limited and unlimited
variants match. This checkpoint adds no timing campaign.

```sh
python3 scripts/verify_source.py --group string-cursors --mutations
bash scripts/verify.sh --area string --check direct-source
python3 scripts/verify_string.py --mutations --kani
```

The full String campaign passes with its direct receipts, six solver controls,
retained core models, compiled pointer-routing control and existing nine
actual-Rust harnesses. Native/doctests (369 each) pass in debug, release and
overflow-checked release. Core-only, alloc-only and all-feature Clippy, Rustdoc,
formatting, the 26 runner checks, arithmetic policy and the unsafe census pass.

The 32-bit run found a fixed eight-byte expectation in the existing length-policy
layout test. That field is one usize; the assertion now uses size_of::<usize>().
The product layout is unchanged. All 18 String scenarios pass after this test
correction in the three native profiles and ASan. The 15 Miri-eligible scenarios
pass strict-provenance Miri on x86-64 and i686, and Tree Borrows. The final four-profile global source/link replay
remains pending.

## Vec layout-aware growth

`array_ceiling` is now the common executable calculation used by array byte
admission and Vec growth. `growth_for_layout` contains the original layout
ceiling check, minimum selection, exact-growth branch and clamped growth call.
The original `grow_to` calls this kernel before resizing storage. Verus checks
every word-sized element size and current < needed ≤ limit in both word domains.
Its accepted capacity is at least needed, within the element/layout limits and
equal to needed for exact growth; Layout refusal has the exact ceiling condition.

The two Python ceiling/minimum substitutions and copied growing branch are
removed from the wrapping-memory model. That model now invokes the product
kernel. Read-only bindings check its actual arguments and publication order.
Three actual-Rust harnesses cover arbitrary valid growing unit-owner metadata
under both sealed policies, existing all-word Capacity/layout reconstruction,
and the existing nonzero two-byte reservation/refusal witness. Four Verus
controls change the zero-size ceiling, boundary, clamp and error kind. Three
Kani controls change the size, needed length and exact flag passed by grow_to.
All seven reach solver failures.

Twelve public reserve, reserve-exact and array-layout consumers compare
`8b38dcd` with this change on Rust 1.95 release and overflow-checked release.
Every pair has identical instruction bytes and relocations, including aliases.
The consumers cover byte, word, unit, three-byte and limited-owner cases. This
checkpoint adds no timing campaign.

```sh
python3 scripts/verify_source.py --group vec-layout-growth --mutations
bash scripts/verify.sh --area vec --check direct-source --plan
python3 scripts/verify_owner_protocol.py --mutations
```

Native/doctests (369 each) pass in debug, release and overflow-checked release.
The 40 Vec scenarios pass strict-provenance Miri on x86-64 and i686, Tree Borrows
and ASan. Core-only, alloc-only and all-feature Clippy, Rustdoc, formatting, the
26 runner checks, arithmetic policy and the unsafe census pass. A fresh caller
replay without prerequisite reuse passes, including its actual memory/state
harnesses and controls, 82 publications and 193 unsafe callers. The final
four-profile global source/link replay remains pending.

## Shared admission and retirement words

`admitted_count`, `retired_state` and `retired_bytes` now carry contracts on
the original Rust. Admission returns the exact successor below MAX_COUNT or
Error::RefCount at/above it. Retirement sets the high bit for every valid Layout
size and decoding returns the low-word payload, expressed as modulo the native
high-bit value. One source proof lemma connects bit operations with integer
values using the pinned vstd shift/division lemma and bit-vector assertions.
The runtime operations are unchanged.

Both the default counter constants and the actual jzap_small_counts configuration
are verified without changing source word types. Verus retains Either32Or64.
The five actual-Rust checks retain unrestricted admission, local Cell transitions,
retired bytes and sequential atomic/weak-credit domains. Four Verus controls
change the admission boundary, successor, retired tag and mask. Two Kani controls
change arguments supplied by local admission and atomic retirement. All six reach
solver failures.

The shared runner composes the original crate with actual numeric calls and the
retained credit/phase/core models. A caller connects the executable retirement
encoder/decoder with the existing integer credit theorem. The native/u32 model
body rewrite is removed. Read-only declaration checks route the three directly
verified functions; the remaining source phase/argument checks continue. The
former structural admission control is replaced by its direct solver control.
Atomic schedules, platform headroom and other shared/budget phase models retain
their separate scopes.

Nine consumers compare `e0c91bc` with this change on Rust 1.95 release and
overflow-checked release: the three numeric functions and Rc/Arc clone, Weak
upgrade and drop. Every pair has identical instruction bytes and relocations.
Private numeric consumers use identical call-only analysis shims in the two
builds. This checkpoint adds no timing campaign.

```sh
python3 scripts/verify_source.py --group shared-numeric --mutations
python3 scripts/verify_direct_verus.py --target shared-numeric-reduced
bash scripts/verify.sh --area shared --check direct-source --plan
python3 scripts/verify_shared.py --mutations --kani --loom
```

The full shared campaign passes: 20 ownership/count harnesses, two concurrent
budget harnesses, six reduced-count harnesses, 12 ownership/budget Loom cases
and six headroom Loom cases. Nine compiled Loom defects are detected, alongside
the retained numeric/phase and structural controls. Native/doctests (369 each)
pass in debug, release and overflow-checked release. Shared scenarios pass 32
native/ASan and 31 Miri-eligible cases in x86-64/i686 strict provenance and Tree
Borrows. Core-only, alloc-only and all-feature Clippy, Rustdoc, formatting, the
26 runner checks, arithmetic policy and the unsafe census pass. The final
four-profile global source/link replay remains pending.

## BTree direct-source checkpoint

Runtime source `869562c` migrates these six groups. Every numeric contract retains
its admitted machine-word domain in both Verus word widths. Caller adapters call
the original executable functions; Python does not extract or lower their bodies.

| Group | Original executable code and contract | Actual Rust checks |
| --- | --- | --- |
| Search and packed cursors | Search midpoint/advance, decreasing interval, cursor coordinates and encoding | Three interval/cursor/validated NodeSlice harnesses |
| Allocation and retirement | Deficit maxima, checked successor and two-field NodeCounts frame | Two arbitrary-word field/update harnesses |
| Iterator frontiers | Owning drop skip and borrowed leaf edge; original bounded add/sub composition | Four arithmetic, borrowed-leaf and owning-cleanup harnesses |
| Deletion decisions | Actual capacity constant, sibling eligibility and exact merge predicates | One arbitrary-word decision harness; typed node checks below |
| Packed output shape | Actual loop: successful arithmetic, termination, exact leaf count and height below 32 | One arbitrary-word native harness |
| Node spans | Seven insert/split/suffix/remove/merge/left-borrow/right-borrow producers; initialized pair/edge bounds | One scalar harness and the eight existing typed node/view harnesses, with unchanged domains |

The independent native-word/packed-word harness remains in the BTree driver.
Together these are 21 actual Rust harnesses. Source mutations exercise numeric
results, field routing and actual caller arguments. All seven node-producer Verus
controls and four compiled/native node controls are detected. The fresh BTree
caller/model replay passes with all six direct groups and unchanged source.

Typed allocation, initialization, aliasing and graph ownership retain the agreed
actual-node Kani, compiler, Miri/ASan and differential-test roles. A supplemental
whole-rebalancer heap probe exceeded the local memory limit and is not a proof.
The full-word numeric contracts are unchanged by that probe.

### Validation and reproduction

On the recovered environment, native/doctests pass 369 cases in each of debug,
release and overflow-checked release. ASan passes all 20 Map and 18 Set scenarios.
Insertion/removal differential, append recycling/budget transfer, mixed structural
operations, mutable iterator disjoint slots and packed Set allocation-refusal
scenarios each pass x86-64/i686 strict-provenance Miri and Tree Borrows. Core and
alloc-only builds, API/boundary checks, Clippy, Rustdoc, formatting, arithmetic
policy, unsafe census and all 26 runner checks pass.

Partial checks now build all required source scanners on demand. A clean-cache
rebuild reproduces the previous scanner executables byte for byte and the same
993 unsafe rows and 3,347 state rows. This changes scanner bootstrapping, not the
product contracts. Generated receipts and diagnostics remain under `target/`.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
python3 scripts/setup_verus.py
rustup component add rust-src --toolchain 1.95.0
python3 scripts/verify_source.py --group btree-node-spans --mutations
python3 scripts/verify_btree.py --mutations --kani
bash scripts/verify.sh --area btree --check direct-source --plan
python3 tools/throughput-analysis/compare_btree_codegen.py --baseline bcf8c7a --output target/btree-codegen
python3 tools/throughput-analysis/prepare_btree.py --baseline bcf8c7a --output target/btree-timing --build-only
# After compilation, proof and memory processes finish:
python3 tools/throughput-analysis/prepare_btree.py --baseline bcf8c7a --output target/btree-timing --run-only
python3 tools/throughput-analysis/diagnose_btree_timing.py --binaries target/btree-timing --output target/btree-entry-repeat
python3 tools/throughput-analysis/diagnose_btree_timing.py --binaries target/btree-timing --output target/btree-linked-code --code-only
```

### Code generation and timing

Retaining all tuple fields across node copies initially introduced spills and a
16-byte larger insertion frame. Reading each verified field at its use restores
the frame from 0x98 to 0x88 bytes and the stack-reference count from 73 to 69.
Against `bcf8c7a`, nine actual consumers and 37 paired local sections preserve
instruction/operand multisets and corresponding relocation targets in release
and overflow-checked release: 32 sections retain instruction order and five
reorder instructions. The comparison script checks the callee mapping and
external targets as well as instruction rows.

The following table uses Rust 1.95.0, generic x86-64, optimization level 3,
`panic=abort`, one codegen unit and default alignment on an Intel Xeon Platinum
8573C. Previous means `bcf8c7a`; current means runtime `869562c`. Input seeds are
617, 6203 and 96011, with 64 random cases per corpus. Small inputs contain 0–32
keys and mixed inputs 0–4,096. All implementations execute equal work per seed.
Compilers/provers and memory tools are stopped during timing. Each interval is
at least 1.228 seconds; values are summed seconds divided by summed calls.

Entry reports ns per existing-key operation and remove_cycle ns per remove/insert
pair. The other rows report ns per whole map case, including fixture clone/drop
where the workload performs them. The initial 144 intervals and all 81 default
supplementary intervals are retained. Build/small and both entry rows use 12
processes per implementation; other rows use three. Supplementary processes use
twice the original epochs. No observation is discarded.

| Operation | Input | std ns/unit | Previous ns/unit | Current ns/unit | Current/std | Current/previous | Processes each |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| append | mixed | 17,125.128 | 17,810.679 | 17,739.091 | +3.59% | -0.40% | 3 |
| append | small | 254.175 | 328.857 | 319.181 | +25.58% | -2.94% | 3 |
| append_disjoint | mixed | 21,786.699 | 20,902.255 | 20,680.769 | -5.08% | -1.06% | 3 |
| append_disjoint | small | 284.847 | 357.588 | 348.683 | +22.41% | -2.49% | 3 |
| build | mixed | 56,437.907 | 58,467.259 | 60,504.850 | +7.21% | +3.49% | 3 |
| build | small | 122.786 | 144.298 | 141.145 | +14.95% | -2.18% | 12 |
| entry | mixed | 93.109 | 83.406 | 89.761 | -3.60% | +7.62% | 12 |
| entry | small | 7.369 | 8.060 | 7.981 | +8.29% | -0.98% | 12 |
| pop_all | mixed | 19,240.072 | 18,446.626 | 16,253.492 | -15.52% | -11.89% | 3 |
| pop_all | small | 163.284 | 137.546 | 138.889 | -14.94% | +0.98% | 3 |
| remove | mixed | 72,411.741 | 69,556.858 | 62,464.227 | -13.74% | -10.20% | 3 |
| remove | small | 188.302 | 154.560 | 169.036 | -10.23% | +9.37% | 3 |
| remove_cycle | mixed | 209.992 | 202.677 | 208.389 | -0.76% | +2.82% | 3 |
| remove_cycle | small | 43.797 | 37.198 | 35.946 | -17.93% | -3.37% | 3 |
| split | mixed | 6,841.090 | 5,216.350 | 5,074.778 | -25.82% | -2.71% | 3 |
| split | small | 87.066 | 100.172 | 94.689 | +8.76% | -5.47% | 3 |

Small append and disjoint append remain above std, by 65.006 and 63.836 ns per
case respectively; both are faster than the previous implementation in this
campaign. These measurements do not establish universal std performance parity.

The initial mixed-entry seed 96011 was 1.911 times the previous runtime. The 27
additional default intervals average +4.08% against previous and −7.30% against
std; the table includes the initial sample. An independent 27-interval CPU-0
campaign averages +10.25% against previous and +12.75% against std and is kept
separate from the default-affinity table. Exact linked disassembly preserves all
64 workload run functions and all 60 epoch functions, including addresses and
instruction bytes. Eight insertion/removal/repair helpers change; the timed
entry path only accesses existing keys. Process-CPU observations in the repeats
do not identify the cause of the original isolated sample, which had no such
observations. The compact [interval CSV](verification/btree-node-source-timing.csv)
contains all 252 intervals, corpus identifiers, work counts and supplementary
process observations. Whole-process CPU includes preparation and teardown; its
subtraction from the timed wall interval is only a lower bound on off-CPU time.

The earlier borrowed-node search campaign is retained in
[its interval CSV](verification/btree-source-timing.csv) and Git `b809adf`.
It measures slice lookup rather than the whole-map operations above.

### Remaining migration

The pinned std-core guard projection in `btree_core.py` and tree ownership/credit
models remain. Other Vec, pointer/core, String, shared-ownership and hash phase
projections also remain, as listed above. The final four-profile global
source/link replay follows their migration; the BTree checkpoint is not that
whole-library replay.

## Vec retain/dedup direct-source checkpoint

Runtime source `558dbfc` uses the executable `vec::compaction_driver` for the
prefix scan, single compaction step, paired/unpaired sweep and error propagation.
Verus checks those original generic functions for every admitted machine-word
state and arbitrary returning callback replies in both word widths. The contracts
establish cursor bounds, exact step effects, completion/error states and
termination. The physical pointer leaves remain in the public Vec methods.

The retain/dedup body lowerer, copied memory-effect functions and their per-site
pointer-expression projections are removed. Maintained adapters call the actual
driver. Read-only Rust AST checks bind the two entries, complete thin adapter
statements, storage identity, callback arguments, moves, retirement and final
length publication. They reject additional calls, function-pointer escapes,
alias imports and shadowed indices. No positive executable body is translated
into a proof input.

Two actual Rust harnesses retain arbitrary native-word owner lengths. Two
separate unabstracted harnesses compare allocated Vec/std fixtures for every
prefix length through four, all four-byte payloads, Boolean reply vectors and
failure ordinals through four. They compare callback count, mutated contents,
error payload and retained order. Four actual-source Verus controls and three
actual Rust solver controls detect broken progress, retirement, paired bounds,
input routing and dedup decisions. Compiled caller controls additionally detect
wrong destinations, invalid initial states and entry/adapter changes.

### Validation and partial reproduction

The selected runtime source matches the frozen actual-Rust proof inputs byte for
byte. Native/doctests pass 369 cases in each of debug, release and overflow-checked
release. All 40 Vec scenarios pass x86-64/i686 strict-provenance Miri, Tree Borrows
and ASan. Feature lint, formatting, Rustdoc, arithmetic/unsafe routing and all 26
runner checks pass. The unsafe census removes fifteen duplicated edit blocks.

Fresh numeric/pointer prerequisites and owner composition pass before the
inlining-only refinement. Final caller replays use those unchanged numeric
prerequisites and compile the final product methods; their receipts identify
prerequisite reuse. They close 82 state publications and 178 unsafe caller
bindings, plus the two directly verified driver entries. The final whole-library
source/link certificate remains a separate pending step.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
python3 scripts/verify_source.py --group vec-compaction --mutations
python3 scripts/verify_source.py --group vec-compaction-values --mutations
python3 scripts/verify_owner_protocol.py --mutations
bash scripts/verify.sh --area vec --check verus --plan
python3 tools/throughput-analysis/prepare_range.py --benchmark compaction --baseline bc4271d --profile default --memory --output target/compaction --build-only
# After compiler, prover and memory-check processes finish:
python3 tools/throughput-analysis/prepare_range.py --benchmark compaction --baseline bc4271d --profile default --memory --output target/compaction --run-only
```

The four-profile link inventory completes and all eight intentional exit controls
are detected. Retained edges have existing contract classifications:

| Profile | Retained direct edges | Unclassified edges |
| --- | ---: | ---: |
| Release | 6 | 0 |
| Release LTO | 9 | 0 |
| Overflow-checked release | 6 | 0 |
| Debug | 80 | 0 |

Optimized retained edges are the existing standard deque rotation and DebugMap
protocol, plus forwarding. This inventory does not renew the global proof
attachment. Extraction/splice, outer growth, pointer/core and other domain
projections listed above remain in the migration.

### Code generation and measured time

The initial generic prefix retained a conditional state advance: its loop had
nine instructions and 35 bytes instead of the previous five instructions and
19 bytes. The update included an extra comparison, two conditional moves and
an add-with-carry. Forcing only the prefix inline retained that code; forcing
run and prefix restored the scan. Forcing all four control helpers inline also
restored the paired path. The executable algorithms and public signatures are
unchanged. No padding or global alignment option is added.

In the final consumer, previous/current retain scans each use five instructions
and 19 bytes; the distinct dedup scan uses six instructions and 23 bytes. Their
loop starts have identical offsets within 64-byte lines. Disassembly is
reproducible with `objdump -d --insn-width=16 --demangle target/compaction/default`.

The comparison uses Rust 1.95.0 on an Intel Xeon Platinum 8573C, optimization level
3, one codegen unit, panic-abort and default alignment. Each call creates an
exact-capacity u64 clone, edits it and drops it. Small corpora contain 0–32 values;
mixed corpora 256–4,096. Each case has 64 random inputs per independent process
seed (1, 2, 3). Excluded calibration targets two seconds for the fastest method;
each recorded interval is at least 1.900 seconds. All 72 intervals have equal
work/checksums per comparison, with no concurrent compiler/prover/memory runs.
Values below are total recorded seconds divided by total calls, in ns/call.
Previous is `bc4271d`; current is `558dbfc`.

| Operation | Input | std ns/call | Previous ns/call | Current ns/call | Current/std | Current/previous |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| retain_all | small | 25.584 | 26.835 | 26.392 | +3.16% | -1.65% |
| retain_all | mixed | 1,504.801 | 1,498.941 | 1,537.025 | +2.14% | +2.54% |
| retain_half | small | 33.518 | 33.005 | 31.959 | -4.65% | -3.17% |
| retain_half | mixed | 12,761.936 | 12,026.601 | 12,164.323 | -4.68% | +1.15% |
| dedup_distinct | small | 26.525 | 25.962 | 25.531 | -3.75% | -1.66% |
| dedup_distinct | mixed | 1,532.850 | 1,579.502 | 1,428.187 | -6.83% | -9.58% |
| dedup_runs | small | 31.256 | 41.020 | 34.544 | +10.52% | -15.79% |
| dedup_runs | mixed | 8,270.082 | 9,673.878 | 9,710.450 | +17.42% | +0.38% |

The mixed dedup-runs difference is already present in the previous source:
+16.97% against std, compared with +17.42% now (+0.38% against previous).
For a fully duplicate pair, std falls through its first comparison and takes
the backedge; previous/current take a skip-copy branch and then the backedge.
The paired region occupies 36 bytes at offset 16 in std and 37 bytes at offset
48 in previous/current; the latter crosses a 64-byte boundary. These are the
observed branch/layout differences in this consumer. The compact
[time CSV](verification/vec-compaction-source-timing.csv) preserves each seed,
case, duration, call count and checksum.

### Allocation observations

All 72 isolated observations match std/previous/current in allocation calls,
held/peak requested bytes, allocator-usable bytes and complete release. The
following values apply to all three implementations and all four edit patterns.
Held storage is observed after the edit; no edit allocates or shrinks its clone.

| Input length | Allocation calls | Requested held/peak bytes | Usable held/peak bytes | Bytes after drop | Handle bytes |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 0 | 0/0 | 0/0 | 0 | 24 |
| 1 | 1 | 8/8 | 24/24 | 0 | 24 |
| 8 | 1 | 64/64 | 72/72 | 0 | 24 |
| 32 | 1 | 256/256 | 264/264 | 0 | 24 |
| 512 | 1 | 4,096/4,096 | 4,104/4,104 | 0 | 24 |
| 4,096 | 1 | 32,768/32,768 | 32,776/32,776 | 0 | 24 |

The [allocation CSV](verification/vec-compaction-source-memory.csv) contains the
individual observations. Timing uses the regular System allocator; allocation
tracking runs separately.


## Vec ExtractIf direct-source checkpoint

`ExtractIf::next` calls the executable `extract_next` control helper. Its prefix
and gapped loops carry direct Verus contracts for every admitted read, write,
end and original-length coordinate in either word width. Selection retires one
index before returning it; retained elements are moved before advancing the
initialized prefix. Callback errors return to the original cold `fail` method,
which stops the scan. The original Drop protocol remains in place.

Actual Rust checks preserve the full-word cursor domain and compare std with
all three-byte payload and Boolean reply arrays, every prefix length and valid
range through three, and every error ordinal through three. The oracle checks
mutation, emitted values, error payload, callback count and retained contents.
Fixed arrays record observations; unwinding assertions remain enabled. Three
Verus progress/retirement defects and three compiled native source, retirement
and error-stop defects are detected. Read-only AST checks bind the three
physical leaves, driver arguments and cold error route; the caller census is
`1509feb`: 82 state writes, 176 unsafe callers and three direct control entries.
After removing the separate splice initialization guard, the current census is
80 state writes, 176 unsafe callers and the same three control entries.

Run only these groups:

```sh
python3 scripts/verify_source.py --group vec-extraction \
  --group vec-extraction-values --mutations
# Select only the named Vec integration target for executed memory checks:
bash scripts/verify.sh --area vec --test-target vec --check miri \
  --check miri-32 --check miri-tree --check asan
```

`--test-target` can be repeated for tests in the selected area. It changes
runtime-test selection only and cannot be combined with the global certificate.
Miri keeps isolation and provenance checks enabled; property-test failure-file
persistence is disabled and its random seed is fixed to 1. These settings, and
the proptest generator choice, reach the interpreted program through
`-Zmiri-env-forward` (isolation otherwise hides host variables; before
`119c907` the settings had no effect and `vec_properties` stopped at an
unsupported `getcwd`). Under Miri proptest uses XorShift (`PROPTEST_RNG_ALGORITHM=xs`):
interpreting its default ChaCha SSE2 generator exhausted Miri's i686 address
space after the two 2048-case properties in `tests/properties.rs` ran in one
process (reproduced locally with one and several test threads and with
`-Zmiri-address-reuse-rate=1`; Miri memory stayed flat at about 235 MB, and the
stop was inside `chacha20`, not product code). Case counts are unchanged; the
generated inputs differ from native runs, which keep ChaCha.

The ExtractIf checkpoint passed a fresh numeric/pointer/owner replay without
prerequisite reuse. After the splice-fill change, numeric and byte-span inputs
were regenerated and the owner replay used those inputs and the unchanged pinned
pointer inputs (`reused_prerequisites=true`). The current replay closes all 80
publications and 176 unsafe callers and detects 22 compiled source/model defects.
Repeated time and allocation comparisons are in the
[edit performance report](verification/vec-edit-source-performance.md). Excess
staging, Buffer outer growth and the other groups marked pending above retain
their separate projection boundaries.


## Splice fill through the original initialization cursor

`Splice::fill` checks the hole endpoint and calls the same `init::collection::fill`
used by Box collection. The separate `Initialized` guard, slot loop and copied
count/increment models are removed. The shared cursor commits each successful
write and returns the source's end or error without advancing that failed step.
The drain continues to own the retained tail.

The original shared loop has direct Verus contracts over both word widths and
successful callback history. A new actual-Rust caller harness admits every valid
native-word start/end/original-length tuple and arbitrary source replies. It
checks complete versus partial fill, exact error propagation, unchanged tail
metadata and restoration after Drop. The all-word caller alone enables the
initialization-loop abstraction; existing nonzero typed-prefix harnesses execute
the original loop without abstraction. They are reused without adding a second
physical initialization model.

Five actual-Rust checks (one new caller, four existing initialization/buffer
checks), ten direct Verus defects across the reused kernel groups and nine native
source defects pass. The native defects include an end beyond the hole and a
changed source-error classification. The read-only adapter guard checks the
complete fill method and its four arguments. An altered fill endpoint is also
rejected by the compiled caller-binding controls.

The final source passes 369 native/doctests in each of debug, release and checked
release; core/all-feature Clippy, formatting and Rustdoc pass. The measurement
profile selection follow-up passes 28 runner checks.
All 40 Vec scenarios pass x86-64/i686 strict-provenance Miri, Tree Borrows and
ASan on unchanged inputs. The four current product link profiles retain
6/9/6/80 edges, all classified. Optimized edges remain the existing debug-map,
std-ring rotation and forwarding paths. Eight exit controls passed at the
preceding ExtractIf checkpoint. This product-only link replay is separate from
the pending final global source/link attachment.

Partial reproduction:

```sh
python3 scripts/verify_source.py --group vec-splice-fill --group buffer --mutations
bash scripts/verify.sh --area vec --test-target vec --check miri \
  --check miri-32 --check miri-tree --check asan
```

The [edit performance report](verification/vec-edit-source-performance.md) records
the default std/previous/current comparisons, matched loop-alignment control,
instruction differences, rejected candidates and compact per-seed CSVs.

## Splice excess-collection checkpoint

The executable staging controller admits every previously allowed word-sized
limit, initial capacity and element size in either Verus word domain. It bounds
the count before advancing and validates returned allocation metadata before
publishing that logical count. Erased successful-call history checks contiguous
indices and one push per committed step. The next source call follows that
commit; a source error preserves the committed prefix.

The owning Vec leaf independently checks its own length increment and reserves
before writing at its own initialized-length endpoint. Controller indices are
not unchecked pointer offsets. Actual-Rust checks cover every admitted native-word
unit-owner length/capacity/limit tuple, bounded-source executions with arbitrary
word extents and exact end/error/callback order, and all byte values in every
three-byte prefix. The word domains are separate from the bounded physical
witness. Three direct Verus and three compiled native defects are detected.

`verify_edit_loops.py` now composes calls to the executable controllers and
maintained span/reservation callbacks. It no longer translates a product loop
body. The staging leaf is also removed from pointer-expression projection.
Outer growth, tail commitment and the other pending caller/core models remain.
The final caller replay closes 82 state publications, 125 constructor fields,
176 unsafe callers and four direct control entries, with 23 compiled defect
controls. Numeric/pointer prerequisite reuse is recorded; the staging source
checks are refreshed on each replay. All 369 native/doctests pass in each of
three profiles; 28 runner checks, lint, formatting, Rustdoc and unsafe routing
pass. All 40 Vec scenarios pass x86-64/i686 strict-provenance Miri, Tree Borrows
and ASan on unchanged inputs. Path-covered timing and the unverified batch candidate are recorded in the
[edit comparison report](verification/vec-edit-source-performance.md#path-covered-staging-checkpoint).
The [next-session handoff](next-session.md) identifies the restart point and remaining work.

```sh
python3 scripts/verify_source.py --group vec-staging --mutations
```

### Staging benchmark path coverage

An excess replacement with an exact lower hint completes through the earlier
fill/growth path. An ordinary excess with a zero lower hint fits the first
grown spare span. Neither measures the private staging loop. The expanded
fixture provides the same honest hint wrapper to std and both product versions;
its zero-lower-hint large condition supplies `4 * input_length + 1` values.
Positive small lengths 1–32 and mixed lengths 256–4096 reach staging.

A separate entry-only diagnostic instruments disposable source copies and checks
full final contents against std. All 72 old/current observations pass: every
positive-length large zero-hint condition enters staging once; the other tested
conditions enter zero times. [Entry observations](verification/vec-staging-entry-coverage.csv)
are coverage data, separate from proofs and uninstrumented timing. The earlier
edit comparison tables identify their frozen source/fixture revision; the
current comparison adds these path-covered conditions.

This historical comparison requires the recorded source revisions. For a snapshot checkout, see the
[current verification workflow](verification/README.md#snapshot-regression-checks).

```sh
python3 tools/throughput-analysis/prepare_range.py --benchmark splice-fill --baseline 687db7d --entry-only --output target/staging-entries
```

## Global certificate renewal (2026-11-10)

The four-profile source/link certificate is renewed at `aa7182c`: complete,
no reused models, 0 open source obligations of 1,137 and 0 unproved retained
link edges of 101. Details and the two renewed pinned inputs are in
[random-state.md](random-state.md#global-certificate). The four groups marked
Pending above are unchanged; the certificate still consumes their projections.

## Staging checkpoint merge (2026-11-10)

The splice excess-collection checkpoint above (`0dd5d84`, `736d479`,
`3fdd55f`) is merged into `work/random-state-tls` at `c888c29`. That checkpoint
recorded reused numeric/pointer prerequisites; a fresh replay needed three
additions:

- `verify_panic_state.py` binds `EXCESS_ADVANCE` to `stage_push`, where the
  staged length increment now lives.
- `verify_wrapping_memory.py` routes the controller's `wrapping_add(len, 1)`.
- `tools/direct-verus/vec-adapters.rs` adds `staged_collection_extent`, which
  calls the actual `staging_driver::run` and proves the published count stays
  within the limit, so the increment does not wrap.

The fresh owner replay closes 83 state publications, 128 constructor fields
and 178 unsafe callers with 23 detected compiled controls. Local timing on the
staging path (`4 * input_length + 1` replacement values, zero lower hint;
`prepare_range.py --benchmark splice-fill --baseline 736d479`, three processes,
intervals of at least two seconds): the controller is 3.4% (lengths 1–32) and
2.4% (256–4096) slower than the previous in-place loop, at +13.2% and +11.0%
against std where the previous loop was +9.5% and +8.5%; the five conditions
that do not enter staging differ by at most 1.7% between the two versions.
The global certificate was rerun on the merge commit `c888c29` (local, 68 min
52 s, no reused models): `complete=True`, 18 steps, 1,140 source obligations
with 0 open, 101 retained link edges (89 originating guards, 12 forwarding)
with 0 unproved, and 6 / 9 / 6 / 80 retained sites in the four link profiles.
Area-wide Miri for the Vec area on the merged staging leaf has not been run.

## Documented core conditions (2026-11-10)

The pinned core guards are private code of the Rust 1.95 standard library.
Verus cannot include that code and its predicates take no annotations. Kani
0.68.0 compiles its own core (nightly-2026-08-21), replaces `ptr::add` with its
own offset model, and cannot give a non-ZST pointer an arbitrary word-sized
address or capacity. The projection of the core predicate text therefore has
no direct replacement in the pinned tool set and is retained.

The guard argument is split at the documented safety condition of each call:

| Half | Statement | Evidence |
| --- | --- | --- |
| Caller to documented condition | Every product call satisfies the documented condition | Storage model in `raw-owner.verus.rs` today; the caller projections are the part to migrate |
| Documented condition to guard | Under the documented condition the pinned predicate returns true | `verify_pointer_checks.py`: predicate text extracted from the pinned core source |

`tools/memory-proof/core-contracts.verus.rs` holds the conditions as maintained
Verus specifications. Each quotes its sentence in `library/core`. They state
address and count facts only; provenance, liveness, initialization and
aliasing stay with the Kani and Miri checks.

| Condition | Core call | Consumed by |
| --- | --- | --- |
| `allocation_wf`, `in_bounds`, `valid_access`, `aligned` | `ptr` module allocation and validity rules | All pointer conditions |
| `doc_ptr_add` | `ptr::add` | `pointer_add_precondition`, `offset_storage` |
| `doc_slice_from_raw_parts` | `slice::from_raw_parts(_mut)` | `slice_size_precondition`, `slice_caller` |
| `doc_copy`, `doc_copy_nonoverlapping` | `ptr::copy`, `ptr::copy_nonoverlapping` | `copy_nonoverlap_precondition`, the three copy callers |
| `doc_replace` | `ptr::replace` | `replacement_address_precondition` |
| `doc_write_bytes` | `ptr::write_bytes` | `slice_caller` |
| `doc_get_unchecked`, `doc_get_unchecked_range` | `slice::get_unchecked(_mut)` | Both index predicates and their callers |
| `doc_non_null_new_unchecked` | `NonNull::new_unchecked` | `offset_storage`, `slice_caller` |
| `doc_alignment_new_unchecked` | `Alignment::new_unchecked` | `alignment_argument_caller` |
| `doc_layout_from_size_align` | `Layout::from_size_align(_unchecked)` | `aligned_array_layout`, `layout_caller` |
| `doc_unchecked_add_usize` | `usize::unchecked_add` | `cursor_successor` |
| `doc_unchecked_shl` | `unchecked_shl` | Not consumed yet; the shift guards belong to `btree_core.py` and the hash model |

Three conditions are stricter than the documented text, by decision of the
user after review:

- `valid_access` does not use the documented exemption for accesses of size
  zero. A zero-byte access is also non-null and in bounds of its allocation.
- `doc_ptr_add` requires the start address in bounds for a zero offset too.
- `doc_get_unchecked_range` adds `start <= end`; the guard checks `end <= len`.

A stricter condition is a stronger obligation on the callers. The product
uses aligned non-null addresses for empty and zero-sized storage
(`NonNull::dangling`, the allocation path in `src/storage.rs`), and no unsafe
block of `src/` forms a null pointer; this is a text observation, the proof is
the storage model.

Old and new premises of the pinned predicates:

| Predicate | Previous premise | Premise now |
| --- | --- | --- |
| `pointer_add_precondition` | `count <= capacity`, capacity bytes within `isize::MAX`, no address wrap at capacity | `allocation_wf`, `doc_ptr_add` |
| `slice_size_precondition` | `len * size <= isize::MAX` | `doc_slice_from_raw_parts` |
| `copy_nonoverlap_precondition` | byte count within `isize::MAX`, ordered disjoint ranges | `allocation_wf` of both allocations, `doc_copy_nonoverlapping` |
| `alignment_mask_precondition` | power-of-two alignment, address a multiple of it | The same, written with `aligned` |
| `unchecked_index_precondition` | `this < len` | `doc_get_unchecked` |
| `unchecked_index_range_precondition` | `end <= len` | `doc_get_unchecked_range` |
| `aligned_array_layout` | unchanged | Unchanged; additionally ensures `doc_layout_from_size_align` |

The input domain is unchanged: every word-sized address, count, element size
and alignment admitted by the premise. `replacement_precondition` keeps its
`vstd` pointer-permission premise; `replacement_address_precondition` is the
same pinned text under `doc_replace`. The storage model maps a `RawStorage`
to one allocation (`allocation_of`) and proves `raw_valid` implies
`allocation_wf`.

What each kind of error does:

| Error | Result |
| --- | --- |
| A product caller violates a documented condition | The caller half fails |
| The pinned guard is stricter than the documented condition | The guard half fails |
| The projection is stricter than the pinned guard | The guard half fails; inspection finds the projection error |
| The projection is weaker than the pinned guard | Not detected by Verus. The caller half still holds; debug-profile tests and Miri execute the real guard on finite inputs |
| A documented condition is transcribed wrongly | Detected when one half no longer fits; not when both accept the same error |

One premise was missing from the storage model and is now stated.
`distinct_nonoverlap_caller` copies between two storages and required equal
element sizes only. `doc_copy` requires both addresses aligned for the copied
type, so the caller now also requires `source.align == destination.align`,
and `callsite_projection.py` states `b.align==a.align` next to the existing
`b.size==a.size`: both storages of a caller hold the same element type.

Controls in `verify_pointer_checks.py --mutations`:

| Control | Count | Detection |
| --- | ---: | --- |
| Pinned predicate text changed (as before) | 6 | One unproved predicate each |
| Documented condition weakened in one clause | 6 | One unproved predicate each: add bounds, slice size, overlap, index, range end, replacement alignment |

Product source under `src/` is unchanged by this step.

## Representation-only callers (2026-11-10)

Of the 178 unsafe callers of the owner composition, 168 had a generated
`actual_caller_N` function. For 81 of them the generated body was the constant
`offset_storage(a, 0)`: the block converts or passes a typed pointer and has no
element offset or count. They were classified by the absence of any pattern
the generator searched for.

These 81 sites are now routed to the maintained `representation_caller` in
`raw-owner.verus.rs` and no proof text is generated for them. The
classification is positive and checked twice:

- the `syn`-parsed operations of the block (`unsafe_operations`) are a subset
  of the `REPRESENTATION` list in `callsite_projection.py`: pointer casts and
  dereferences, owner constructors from raw parts, initialization slots,
  pinning and the allocator entry points;
- the generator finds no offset or count query in the same block.

A block with representation-only operations and a query, or with another
operation and no query, is refused. The former silent fallback is removed.

| Caller binding | Before | Now |
| --- | ---: | ---: |
| Generated `actual_caller_N` | 168 | 87 |
| Maintained `representation_caller` | 0 | 81 |
| Direct driver, initialization and staging adapters | 10 | 10 |

Control `representation_gains_offset`: `self.ptr.as_ref()` in `Box::deref`
replaced by `&*self.ptr.as_ptr().add(1)` leaves the representation contract
and is unproved as an offset (one solver error). Product source is unchanged.

## Vec single-element moves through product kernels (2026-11-10)

`Vec::try_insert`, `Vec::remove` and `Vec::swap_remove` computed their
positions inline. `verify_wrapping_memory.py` extracted the rejection
condition, the shortened length, the shifted count and the three pointer
offsets from each body and substituted them into `INSERT_*`, `REMOVE_*` and
`SWAP_*` placeholders; `callsite_projection.py` stated the resulting bounds as
assumptions of a generated caller.

The positions are now computed by three product kernels in
`src/vec/geometry.rs`, verified on the source in both Verus word domains:

| Kernel | Result | Used by |
| --- | --- | --- |
| `insertion(len, index)` | `shifted == len - index` when `index <= len`, otherwise `Error::Bounds` | `try_insert` |
| `shortened(len, index)` | `next == len - 1` when `index < len`, otherwise `Error::Bounds` | `swap_remove`, `removal` |
| `removal(len, index)` | `next == len - 1`, `shifted == next - index` when `index < len`, otherwise `Error::Bounds` | `remove` |

| Previous element | Replacement |
| --- | --- |
| Extracted `*_REJECT`, `*_NEXT`, `*_SHIFTED` bodies | Kernel contracts; all word-sized `len` and `index` |
| Extracted `*_SOURCE`, `*_DESTINATION`, `*_READ`, `INSERT_WRITE` offsets | Maintained span statements in `insertion_before_moves`, `removal_before_moves`, `swap_remove_before_moves`, which call the kernels |
| Assumptions `index<=len`, `count==len-index`, `next==len-1`, `count==next-index` of three generated callers | `vec_insert_caller`, `vec_remove_caller`, `vec_swap_remove_caller` in `raw-owner.verus.rs` derive them from the kernel result and establish the documented core conditions |
| Regex routing of the pointer expressions | Whole-body comparison of each caller (`SINGLE_ELEMENT_MOVES`) and exact comparison of its unsafe block (`KERNEL_CALLERS`) |

The premise `len < cap` of the insertion caller is the capacity after
`reserve_length(1)`, as before; `insertion_before_moves` composes the same
reservation model. The input domain is unchanged.

Controls:

| Control | Mechanism | Detection |
| --- | --- | --- |
| `insertion-count`, `shortened-guard`, `removal-count` | Kernel source changed | Verus counterexample in the kernel contract |
| `destination_routing` | `place.add(1)` changed to `place.add(2)` in `try_insert` | Caller body differs from the reviewed routing |
| `destination_span` | The same offset changed in the maintained span statement | Solver error |
| `one_past_allocation_read` (unchanged) | `remove` reads at the capacity | Kani object-bounds failure in the actual function |

The bound check `index > len` in `insertion` is implied by the checked
subtraction that follows it. Replacing the check with a condition that never
holds leaves the contract provable, so it has no control. The same redundancy
existed in the previous `try_insert` body and is kept.

Product source changed in this step:

| Function | Change | Executable control flow |
| --- | --- | --- |
| `vec::geometry::insertion`, `shortened`, `removal` | New `#[inline]` kernels | New |
| `Vec::try_insert` | Bound check and checked subtraction moved into `insertion` | Two rejection sites that built `Rejected { value, Error::Bounds }` are one; the checks are the same and in the same order |
| `Vec::remove` | Bound check and two checked subtractions moved into `removal` | Same checks in the same order |
| `Vec::swap_remove` | Bound check and checked subtraction moved into `shortened` | Same checks in the same order |

The unsafe blocks of the three callers are unchanged. No timing was taken in
the cloud; the comparison of compiled code before and after is for the local
measurement.

### Global certificate status for `fec074d` (2026-11-10)

No global certificate exists for `fec074d`. One run of
`bash scripts/verify.sh --area all --check certificate` was started alone on the
clean commit and was stopped by the cloud session's two-hour background limit
during its last step. An earlier run for `548a6cf` was also stopped unfinished.

| Part | State |
| --- | --- |
| Steps 1–17 (fixture/routing builds, budget-native, direct-foundation, direct-verus, shared, string, btree, btree-set, hash-map, hash-set, binary-heap, deque, scratch-slice, fresh owner-protocol, utf8-native, arithmetic-policy) | Each step log ends in its pass line; 1 h 57 min |
| Step 18, four-profile link inventory inside the certificate | Stopped after the `release` profile |
| Attachment of retained link edges to verified source contracts | Not run |
| Certificate summary and `complete` flag | Not produced |

A separate `python3 scripts/check_link_exits.py --profiles
release,release-lto,release-checked,debug` run on the same commit finished. It
is a partial check, not a certificate. All eight link controls of the two
controlled profiles were detected. The product fixtures retain 6 / 9 / 6 / 80
direct exit edges (release / release-lto / release-checked / debug); these are
the edges the certificate attaches to source contracts, and that attachment has
not been evaluated for this commit.

The certificate for this unit was produced afterwards on `04dceb1`; see below.

### Resumable certificate and result for `04dceb1` (2026-11-10)

`scripts/source_certificate.py --resume` continues the campaign recorded in
`--output`. `campaign.json` stores the input fingerprint and, per finished step,
its command, environment, exit code and the SHA-256 of its log and of the
summary files the composition reads. A later invocation skips a recorded step
only while those files are byte-identical; different inputs, a changed command
or changed evidence is refused. A step ended by a signal is not recorded and
runs again. The two incremental fixture builds run in every invocation. Without
`--resume` the script starts a new campaign. `--reuse-models` and the
`complete` conditions are unchanged. `scripts/verify.sh --check certificate`
does not pass `--resume`; the script is called directly for a continued
campaign. `scripts/test_verify.py` (30 tests pass) covers single execution and
refusal of a changed fingerprint, command, log and receipt.

`04dceb1` has the product source of `fec074d`; it changes only
`scripts/source_certificate.py` and `scripts/test_verify.py`.

```sh
python3 scripts/source_certificate.py --output target/verification/replay/unit3-campaign/certificate
python3 scripts/source_certificate.py --resume --output target/verification/replay/unit3-campaign/certificate
```

| Item | Result |
| --- | --- |
| Invocations | 2; the first was stopped by the two-hour limit during owner-protocol |
| Steps executed in invocation 1 | budget-native, direct-foundation, direct-verus, shared, string, btree, btree-set, hash-map, hash-set, binary-heap, deque, scratch-slice |
| Steps executed in invocation 2 | fixture/routing builds, owner-protocol (fresh, no reused prerequisites), utf8-native, arithmetic-policy, four-profile link |
| `complete` / `reused_models` / inputs unchanged | true / false / true |
| Source obligations | 1,140 connected, 0 open |
| Unsafe callers bound | 178 |
| Retained link edges | 101 (release 6, release-lto 9, release-checked 6, debug 80): 89 originating guards attached to verified contracts, 12 forwarding; 0 unproved |
| Link controls | 8 of 8 detected |

The Kani build for `utf8-native` was already cached; the harness itself was
verified in invocation 2. Miri and ASan were not run for this unit.

### Callers bound without expression lowering (2026-11-10, intermediate state)

Intermediate state at `79c60d0`; the certified state is in the next section. No product source changed after `fec074d`. The global
certificate has not been run for these commits; the results below are from
`python3 scripts/verify_owner_protocol.py --reuse-generated --mutations`, which
reuses the numeric and pointer prerequisites and is a partial replay.

`callsite_projection.py` no longer extracts offsets or counts for 65 further
callers. Each is bound to a maintained function in
`tools/memory-proof/raw-owner.verus.rs` by exact text: the whole unsafe block,
or the whole function body when the position is computed outside the block.
A changed block or body is refused; an unreviewed scalar access or a new
offset-bearing block in a bound function is refused instead of receiving a
generated assumption.

| Binding | Callers | Where the offset and count come from | Premise still stated |
| --- | ---: | --- | --- |
| Generated caller function (projection) | 27 | Regex-lowered expressions | Per-site assumptions |
| Representation only (`875ee3d`) | 81 | No offset or count | Typed-storage ABI |
| Owner invariant: prefix, spare, empty, layout | 17 | `len`, product `wrapping_sub(cap, len)` | `len <= cap` |
| One complete typed value | 10 | None | One element in its typed place |
| `Span` iterators (`IntoIter`, `Drain`) | 12 | Product `Span::front/len/next/next_back/clear` | `span.back <= cap` |
| Indices from `init::prefix::initialize` | 7 | Product `init::driver::run` from 0 to `n` | `n <= cap` |
| Index from `init::collection::fill` | 1 | Product `init::driver::run` from `initial` to `end` | `initial <= end <= cap` |
| `pop`, `truncate`, `push_within_capacity` (2), `try_insert_mut` | 5 | Checked subtraction, `removed_suffix`, `append_cursor`, `insertion` | `len <= cap`; `len < cap` for `try_insert_mut` |
| One slot after a reservation | 5 | `len` | `len < cap` (reservation result, not proved here) |
| Vec single-element moves (`fec074d`) | 3 | `insertion`, `removal`, `shortened` | `len < cap` for `try_insert` |
| Earlier driver, initialization and staging adapters | 10 | Product drivers | As recorded above |

The five reserved-slot callers (`Buffer::push`, `try_push_mut`,
`try_resize_with_init`, two in `try_resize_owned`) keep the assumption they had
under the projection; only the expression lowering is removed. That premise and
the 27 remaining projected callers (reserved-suffix copies, `try_split_off`,
`try_write_vectored`, the `try_resize_with_init` clone callbacks, compaction
drops, and the Drain tail moves) depend on the reservation result or on the
edit state composition of the next group.

Bodies of `init::prefix::initialize`, `init::collection::fill`,
`clone::array_from_fn`, `InitSlot::try_init_array` and
`Box::try_init_slice_in` are compared whole, so the element count and the index
reach the callbacks unchanged.

Result: 507 verified, 0 errors. Controls, all detected: 11 changed sources
refused before proof text is composed (`expose_spare_as_initialized`,
`scalar_gains_offset`, `into_iter_stale_index`, `drain_clear_after_count`,
`array_clone_shifted_index`, `prefix_driver_longer_count`, `pop_one_past`,
`truncate_from_capacity`, `collection_write_shifted`, `empty_slice_length`,
`drop_whole_capacity`; three of them by an earlier binding audit of the same
source) and 10 weakened maintained functions rejected by Verus
(`owner_prefix_without_invariant`, `owner_scalar_without_element`,
`owner_spare_count`, `span_without_storage_bound`, `span_remaining_count`,
`prefix_driver_one_past`, `pop_without_subtraction`, `truncate_whole_length`,
`collection_driver_past_end`, `reserved_slot_without_capacity`), plus the
existing `representation_gains_offset`.

`dangling_storage` failed once while functions were being added (505 verified,
1 error): its proof left the zero product of an empty extent to nonlinear
search. The product is now asserted with `by(nonlinear_arith)`; no assumption
was added.

### Reservation composition and certificate for `8535521` (2026-11-10)

No product source changed after `fec074d`; `8535521` changes
`scripts/callsite_projection.py`, `scripts/verify_owner_protocol.py` and
`tools/memory-proof/raw-owner.verus.rs`.

Seventeen further callers left the projection. Their maintained functions call
the product kernels in the product order, and the function body is compared
whole.

| Caller | Maintained function | Product kernels called |
| --- | --- | --- |
| `try_extend_from_slice_copy`, `try_append` (2) | `vec_extend_copy_caller` | `required_len`, `growth_for_layout`, `append_cursor` |
| `try_extend_from_within` (copy path) | `vec_extend_within_caller` | same, with the validated range |
| `try_split_off` | `vec_split_off_caller` | checked subtraction, `removed_suffix` |
| `try_from_array_in`, `try_into_array` | `vec_from_array_caller`, `vec_into_array_caller` | none (whole typed extent) |
| `Buffer::append`, `Buffer::push` | `buffer_append_caller`, `buffer_push_caller` | `buffer_numeric::reservation` |
| Whole-storage copies (3) | `whole_copy_caller` | none (layout facts) |
| Compaction and extraction drops, `Drain` tail restore, `Splice::move_tail` | `compact_drop_caller`, `extract_drop_caller`, `extract_peek_caller`, `drain_restore_tail_caller`, `splice_move_tail_caller` | `Compaction` and `Extraction` state methods, checked sums |
| `try_push_mut`, `try_resize_with_init`, `try_resize_owned` (2), `Splice::try_finish` | `reserved_slot_caller` | none |

`vec_reserve_length` and `vec_try_reserve` reproduce the reservation decision
by calling `required_len` and `growth_for_layout`. The capacity they return is
connected to the stored capacity by an executable guard in the maintained
function and by whole-body comparison of `Vec::reserve_length`, `grow_to`,
`grow`, `resize_storage`, `try_reserve`, `try_reserve_exact` and
`Buffer::reserve`. That the allocation call stores exactly that capacity is
fixed by the pinned text, not proved by Verus.

| Binding | Callers |
| --- | ---: |
| Generated caller function (projection) | 10 |
| Representation only | 81 |
| Exact unsafe block and owner invariant | 27 |
| Whole function body and product kernels | 47 |
| Vec single-element kernel adapters | 3 |
| Earlier driver, initialization and staging adapters | 10 |

Projected callers that remain: `try_write_vectored` (3),
`try_extend_from_within` clone loop (2), the clone callbacks passed to
`try_resize_with_init` (`try_extend_cloned`, `Vec::try_clone_in`,
`IntoIter::try_clone_in`), `Splice::stage_excess` (1) and
`<[T; N]>::try_copy_slice_from` (1). The first nine need the loop expressed
through a product driver; the last needs `clone::geometry::flatten_count` to be
reachable from the proof crate. Both are product source changes and are not
made here.

Premises stated in maintained functions and not proved: `len <= cap` and
`span.back <= cap` as owner invariants, `len < cap` for the five
`reserved_slot_caller` sites and for `try_insert`/`try_insert_mut`, the `Drain`
invariants, the range validator result `start <= end <= len`, the layout facts
of whole-storage copies and the disjointness of the two storages.

```sh
python3 scripts/source_certificate.py --output target/verification/replay/unit4-campaign-b/certificate
python3 scripts/source_certificate.py --resume --output target/verification/replay/unit4-campaign-b/certificate
```

| Item | Result |
| --- | --- |
| Invocations | 2; the first was stopped by the two-hour limit after arithmetic-policy |
| Steps executed in invocation 1 | budget-native, direct-foundation, direct-verus, shared, string, btree, btree-set, hash-map, hash-set, binary-heap, deque, scratch-slice, owner-protocol (fresh, no reused prerequisites, with controls), utf8-native, arithmetic-policy |
| Steps executed in invocation 2 | fixture/routing builds, four-profile link |
| `complete` / `reused_models` / inputs unchanged | true / false / true |
| Owner protocol | 505 verified, 0 errors |
| Owner-protocol controls | 51 of 51 detected: 21 compiled source mutants refused by binding checks, 14 changed sources refused by exact-text routing or an earlier binding audit, 16 weakened maintained functions rejected by Verus |
| Source obligations | 1,140 connected, 0 open |
| Unsafe callers bound | 178 |
| Retained link edges | 101 (release 6, release-lto 9, release-checked 6, debug 80): 89 originating guards attached, 12 forwarding; 0 unproved |
| Link controls | 8 of 8 detected |

Routing controls added in this unit: `append_from_capacity`,
`reserve_without_growth`, `compact_drop_whole`. Solver controls added:
`reserve_length_keeps_capacity`, `extend_copy_without_published_capacity`,
`buffer_append_old_capacity`, `extract_drop_to_end_only`,
`move_tail_without_gap_fill`.

Miri and ASan were not run for this unit. The B-tree guards
(`btree_core.py`) and the tree part of group 3 are scheduled last.

## String edit geometry and shared admission through product kernels (2026-11-11)

Group 3 without the B-tree part. The global certificate has not been run for
these commits; the results are from the area campaigns below.

Product source before `8535521` (same as `fec074d`), after `ab3407e`.

| File | Change | Executable control flow |
| --- | --- | --- |
| `src/string/edit.rs`, `Edit::new` | `verus_spec` contract; the two `map_err` closures carry a result contract and name their argument `_error` instead of `_` | Unchanged |
| `src/string/edit.rs`, `Edit` and its impl | `verus_verify` attribute | Unchanged |
| `src/string.rs` | `mod edit` is `pub(crate)` | Unchanged |
| `src/budget.rs` | `mod admission` is `pub(crate)` | Unchanged |

Release assembly of four consumers (`String::try_push`, `try_replace_range`,
`try_insert`, `remove`, Rust 1.95, one codegen unit) is identical before and
after once the symbol hash of `try_replace_range` is normalized (610 lines).
No timing was measured.

`Edit::new` is verified on its own body in the `string-cursors` direct target
for every word input in both word domains: exact `tail`, `destination`, `len`
and `additional`, `len <= limit`, and the exact `Bounds`/`Capacity` refusals.
The hand-written `string_append_admission` model is removed.
`tools/memory-proof/string.verus.rs` gains maintained callers that call the
product kernels:

| Caller | Maintained function | Product kernels called | Proved |
| --- | --- | --- | --- |
| `String::try_replace_range` | `string_replace_caller` | `Edit::new`, `required_len`, `growth_for_layout`, checked subtraction | Tail source, tail destination and inserted text lie inside the capacity after the reservation; the published length does too |
| `String::try_push` | `string_push_caller` | same | The `width` bytes at `plan.start` and the published length lie inside the capacity, with or without growth |

`verify_string.py` compares the whole bodies of `try_push` and
`try_replace_range` (as before) and of `Vec::try_reserve`, `grow`, `grow_to`
and `resize_storage`, and requires each maintained function to contain its
product calls. Premises stated and not proved: `len <= cap` of the byte owner,
`1 <= width <= 4` for a scalar, and that the reservation stores the capacity
its pinned body computes. Extents are stated as byte inequalities, not through
the documented core conditions used by the Vec owner protocol. `pop_length`
and the six `string_core_*` functions remain models of pinned core source.

In `tools/memory-proof/shared.verus.rs`, `concurrent_admit` no longer repeats
the admission arithmetic; it calls `crate::budget::admission::admit`, whose
contract is proved on source in the `budget-admit` target. The ghost credit
lemmas, `shared_prefix_next` and `shared_core_atomic_order` are unchanged.
`shared_prefix_next` models the two hand-written slice initialization loops in
`src/shared/value.rs`; binding them directly needs those loops to run through
`init::prefix::initialize`, an executable source change that is not made here.

```sh
python3 scripts/verify_direct_verus.py --target string-cursors --mutations
python3 scripts/verify_string.py --mutations --kani
python3 scripts/verify_shared.py --mutations --kani --loom
```

| Check | Result |
| --- | --- |
| `string-cursors` direct target | Pass, 251 obligations; 6 of 6 source defects rejected (3 new in `Edit::new`: destination, additional, tail) |
| String composition | 264 verified, 0 errors; 9 Kani harnesses pass |
| String controls | 4 weakened maintained functions rejected by Verus (`replace_without_reservation`, `replace_reserves_nothing`, `push_skips_growth`, `reserve_keeps_capacity`); 1 compiled source mutant refused |
| Shared composition | 287 verified, 0 errors; 20 + 2 + 6 Kani harnesses pass; Loom 12 + 6 tests pass |
| Shared controls | 3 model defects rejected (new: `admission-arguments`), 11 compiled source mutants refused, 9 Loom defects detected |
| Arithmetic policy, unsafe census, format, Clippy (alloc), 30 runner tests | Pass |

Miri and ASan were not run for this unit.

## Shared slice initialization through the product prefix driver (2026-11-11)

Product source before `68a4c9c`, after `c5ed368`; file `src/shared/value.rs`.
The global certificate has not been run for these commits.

| Function | Before | After | Executable control flow |
| --- | --- | --- | --- |
| `<[T] as CloneValue>::clone_into` | Hand-written `for` loop with a local `Prefix` guard and `checked_add` | `init::prefix::initialize(ptr, self.len(), ..)`; the callback reads `get_unchecked(index)` | Changed |
| `Strong<[T], C, P>::try_init_slice_in` | Hand-written `while` loop with the same guard | `init::prefix::initialize(ptr, len, &mut initialize)` | Changed |
| `Prefix<T>` and its `Drop` | Present | Removed; the driver's guard releases the completed prefix | Removed |

`clone_into` has one new unsafe block (`get_unchecked`), the same form as
`Box::try_clone_from_slice_in`. The unsafe census is now 581 blocks (shared
group 98) and 172 shared bindings.

Release assembly (Rust 1.95, one codegen unit, x86-64, symbol hashes
normalized) of five consumers, before and after:

| Consumer | Instructions | Difference |
| --- | --- | --- |
| `Rc<[u64]>::try_from_slice` | same | None |
| `Arc<[u64]>::try_from_slice` | 45 / 45 | None |
| `Rc<[u64]>::try_init_slice_in` | 54 / 54 | None |
| `Arc<[Box<u64>]>::try_init_slice_in` | 64 / 64 | None |
| `Rc<[Box<u64>]>::try_from_slice` | 75 / 75 | Loop exit compares the index with the length instead of decrementing a byte count; the allocator entry is loaded into a register once before the loop instead of being called through memory in each iteration |

No timing was measured.

`shared_prefix_next`, a model of the removed loops, is replaced by
`shared_prefix_driver` in `tools/memory-proof/shared.verus.rs`. It runs
`crate::init::driver::run` from 0 to `length`; `index < length` in the callback
is the driver's verified call condition, and the element extent
`offset + index*size + size <= total` is proved from it. `verify_shared.py`
compares the whole bodies of both callers, of `init::prefix::initialize` and of
`initialize_one`. Stated and not proved: the payload extent
`offset + length*size <= total` of the allocation and that the source slice has
`length` elements (the callers pass `self.len()` and the pinned text fixes it).

```sh
python3 scripts/verify_shared.py --mutations --kani --loom
```

| Check | Result |
| --- | --- |
| Shared composition | 287 verified, 0 errors |
| Kani | 20 + 2 + 6 harnesses pass |
| Loom | 12 + 6 tests pass; 9 of 9 defects detected |
| Model controls | 3 of 3 rejected (`prefix-overrun` now weakens the driver callback premise) |
| Compiled source mutants | 13 of 13 refused (new: `slice-clone-count`, `slice-init-pointer`) |
| Native tests, format, Clippy (alloc), arithmetic policy, unsafe census, 30 runner tests | Pass (369 native tests) |

Miri and ASan were not run in this session. They were run in a separate
session; see [the record](#executed-memory-checks-for-the-shared-slice-change-2026-11-11).


### Certificate for `fb3da98` (2026-11-11)

`fb3da98` contains the String edit and shared admission unit and the shared
slice initialization unit above. The campaign started from a clean tree at that
commit and finished in one invocation (106 minutes), so no `--resume` was
needed.

```sh
python3 scripts/source_certificate.py --output target/verification/replay/unit6-campaign/certificate
```

| Item | Result |
| --- | --- |
| Invocations | 1 |
| Steps executed | fixture/routing builds, budget-native, direct-foundation, direct-verus, shared, string, btree, btree-set, hash-map, hash-set, binary-heap, deque, scratch-slice, owner-protocol (fresh, no reused prerequisites, with controls), utf8-native, arithmetic-policy, four-profile link |
| `complete` / `reused_models` / inputs unchanged | true / false / true |
| Direct foundation / direct Verus controls | 54 of 54 / 120 of 120 detected |
| Shared composition | 287 verified, 0 errors; 3 of 3 model controls rejected (286 verified, 1 error each) |
| String composition | 264 verified, 0 errors; 4 of 4 solver controls rejected (263 verified, 1 error each) |
| Owner protocol | 507 verified, 0 errors; 51 of 51 controls detected |
| Source obligations | 1,141 connected, 0 open |
| Shared source bindings | 172 |
| Retained link edges | 101 (release 6, release-lto 9, release-checked 6, debug 80): 89 originating guards attached, 12 forwarding; 0 unproved |
| Link controls | 8 of 8 detected |

The link step exits with status 1 because the product fixtures retain the 101
edges listed above; the certificate attaches each of them to a source contract.

Not run in this campaign: Miri, ASan and timing benchmarks. The Miri and ASan
runs for the shared slice initialization change in `c5ed368` are recorded
[separately](#executed-memory-checks-for-the-shared-slice-change-2026-11-11).

## Hash-table kernel callers through product contracts (2026-11-11)

Product source: `7beafdb` to `bfb1fe7`. No executable statement changed.

| File | Change | Executable control flow |
| --- | --- | --- |
| `src/collections/hash_map.rs` | `geometry`, `group`, `raw` modules `pub(crate)` | Unchanged |
| `src/collections/hash_map/geometry.rs` | `pub(super)` to `pub(crate)`; `checked_sizes` contract also states the exact offset | Unchanged |
| `src/collections/hash_map/group.rs` | `pub(super)` to `pub(crate)`; contracts on `first`, `clear_first`; `WIDTH` visible to Verus; ghost proof block in `clear_first` | Unchanged |
| `src/collections/hash_map/raw.rs` | `pub(super)` to `pub(crate)`; contracts on `capacity_for`, `tag`; ghost proof block in `tag` | Unchanged |

The ghost blocks are compiled only under `verus_keep_ghost`. A static-library
probe with 21 `#[inline(never)]` entry points (`HashMap<u64, u64>`,
`HashMap<u64, Box<u64>>`, `HashSet<u64>` with a fixed-key SipHash-1-3 builder:
insert, lookup, remove, reserve, shrink, clear, clone, iteration, retain, drop,
replace) was built in release mode with `codegen-units=1` before and after.
Both listings have 5,305 lines; eight lines differ, all in the hash suffix of
symbol names, and the listings are identical after normalizing symbol hashes
and local labels. Other element types, LTO and other architectures were not
compared.

`tools/memory-proof/hash-map.verus.rs` is now verified together with the
product crate through `verus_composition.py` instead of as a separate crate.
Its maintained functions call the product kernels:

| Product function | Maintained function | Product kernels called |
| --- | --- | --- |
| `RawTable::allocate` | `hash_allocate_caller` | `checked_sizes`, `capacity_for`, bounded add/sub, `WIDTH` |
| `RawTable::allocation`, `try_clone_copy`, `Owner`/`Storage` drop | `hash_allocation_caller` | `checked_sizes`, bounded add/mul/sub |
| `RawTable::slot` | `hash_slot_caller` | `checked_sizes` |
| `RawTable::control`, `set_control` | `hash_set_control_caller` | `mirror` |
| `RawTable::find`, `vacant`, `group::matching`, `group::vacant` | `hash_probe_caller` | `probe_next`, `first`, `clear_first` |
| `RawTable::find_insert` | `hash_find_insert_caller` | `probe_next`, `first`, `clear_first` |
| `RawTable::insert_at` | `hash_insert_at_caller` | `capacity_for`, `tag` |
| `RawTable::erase` | `hash_erase_caller` | `mirror`, `first` |
| `RawTable::clear`, end of `Clear::run` | `hash_reset_caller` | `checked_sizes`, `capacity_for` |
| `Clear::run` scans | `hash_clear_run_caller` | `first`, `clear_first` |
| `RawTable::rehash` distance classes | `hash_rehash_distance_caller` | bounded add/sub |

What the maintained functions prove for every word-sized input: the control
span `buckets + 16` equals `bytes - offset` of the admitted sizes; the
reconstructed layout in `allocation` equals the admitted one; reverse slot
`index + 1` lies inside `offset`; both `set_control` writes lie inside the
control span; every probe position stays at most `mask`, every `probe_next`
call meets its verified precondition and both probe loops terminate; the
erase group loads lie inside the control span; a FULL tag has no vacant bit.

Stated and not proved:

- `RawTable` contains `NonNull<u8>`, which this Verus version does not
  support, so `RawTable::groups` and `RawTable::buckets` are restated in
  `hash_groups` and `hash_buckets`. `hash_map_bindings.py` compares both the
  product body and the maintained body whole.
- `buckets_for` uses `checked_next_power_of_two`, which Verus does not support.
  That a nonempty bucket count is a power of two of at least four is a premise
  of `table_shape`.
- That a live table was admitted with its current shape (the `Err` branch of
  the repeated `checked_sizes` call returns).
- The credit invariant `len + growth + deleted == capacity` and
  `empty ==> growth > 0` in `hash_insert_at_caller`; `len >= 1` and
  `len + growth <= mask` in `hash_erase_caller`.
- `hash_clear_run_caller` requires at least one whole group (`mask >= 15`).
  For tables of four or eight buckets the scanned bits name real slots because
  the padding controls are EMPTY; that is a data invariant, not proved here.
- The group masks are arbitrary `u16` parameters; the same value is used for
  every group of one call.
- The file hashes in `hash_map_bindings.py` still seal all eight files. The
  retained lemmas (`hash_insert_credit`, `hash_retire_progress`,
  `hash_rehash_pending`, `hash_physical_duplicates`, `hash_clone_publication`,
  `hash_layout_ranges`, `hash_probe_progress` and the core shift/chunk lemmas)
  are unchanged and still bind the callers in `hash_map.rs`, `iter.rs` and
  `entry.rs` by function name.

Unsafe-site bindings, 102 in total: 37 now name a maintained caller
(`hash_allocation_caller` 9, `hash_probe_caller` 8, `hash_set_control_caller` 4,
`hash_insert_at_caller` 3, `hash_erase_caller` 3, `hash_clear_run_caller` 3,
`hash_allocate_caller` 2, `hash_slot_caller` 2, `hash_reset_caller` 2,
`hash_find_insert_caller` 1); 62 keep a retained lemma; 3 keep the Kani
replacement harness.

```sh
python3 scripts/verify_hash_map.py --kani --mutations
python3 scripts/verify_hash_set.py --kani --mutations
python3 scripts/verify_direct_verus.py --target hash-map --mutations
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 312 verified, 0 errors |
| Weakened maintained callers | 9 of 9 rejected (311 verified, 1 error each) |
| Changed product kernels, compiled | 4 of 4 rejected by Verus (`tag-shift`, `capacity-factor`, `clear-first-or`, `sizes-offset-rounding`) |
| Changed product callers, compiled | 5 of 5 refused by text comparison (`groups-body`, `find-probe-arguments`, `set-control-mirror`, `slot-offset`, `allocation-rounding`) |
| Earlier controls | `copy-bound-removed`, `pending-retirement`, `actual-wrong-slot`, `actual-copy-publication` detected |
| Kani | 7 harnesses pass |
| HashSet area | Pass, complete |
| Direct `hash-map` target | 21 obligations pass; 3 of 3 controls detected |
| Format, Clippy (alloc), arithmetic policy, unsafe census (581 blocks), 30 runner tests | Pass |
| Native tests (alloc) | 369 pass, 0 fail |

Not run: the global certificate for `bfb1fe7`, Miri, ASan and timing
benchmarks.

## Hash-table position callers (2026-11-11)

No product source changed after `bfb1fe7`. This unit changes
`tools/memory-proof/hash-map.verus.rs`, `scripts/hash_map_bindings.py` and
`scripts/verify_hash_map.py`.

Eighteen unsafe blocks outside the table kernel use an index only as a
position: the slot or control at an index returned by `find`, `find_insert`,
`vacant` or the scan. They now name a maintained caller, and the product
function that produces and uses the index is compared whole.

| Product site | Maintained function | Source of the index (text-bound) |
| --- | --- | --- |
| `HashMap::get`, `get_key_value`, `get_mut` | `hash_slot_caller` | `self.find(key)?`, whose body is `self.table.find(..)` |
| `HashMap::try_insert` (occupied replacement) | `hash_slot_caller` | `InsertSlot::Occupied(index)` from `find_insert` |
| `OccupiedEntry::key`, `get`, `get_mut`, `into_mut` | `hash_slot_caller` | `self.index`, constructed in `entry` from `table.find` and in `try_insert_entry` from `insert_vacant` |
| `HashMap::try_retain`, `ExtractIf::try_next` | `hash_slot_caller` | `scan.next()` |
| `Iter`/`IterMut` `next` and `fold` | `hash_slot_caller` | `scan.next()` / `fold_indices`; the inline `sub(add(index, 1))` is the slot expression |
| `HashMap::remove_entry`, `OccupiedEntry::remove_entry` | `hash_erase_caller` | as above |
| `HashMap::prepare_insert_at` | `hash_set_control_caller` | `candidate` or `table.vacant(hash)` |
| `Scan::fill_group` | `hash_scan_caller` (new) | group cursor |

`hash_scan_caller` calls `group::first`, `group::clear_first` and the bounded
add/sub in the order of `fill_group` and `take_cached`. For every word-sized
mask it proves that each group load lies inside the control span and that the
scan terminates; for tables of at least one group it proves that each yielded
index is a bucket. `hash_probe_caller` now also follows the first-group
fallback of `RawTable::vacant` and its `index < buckets` check.

Stated and not proved, in addition to the previous unit:

- That the slot at a yielded index holds an initialized pair (FULL tag). The
  position proofs cover the address only.
- For tables of four or eight buckets, that the scanned bits name real slots
  (EMPTY padding).
- `Scan::take_cached` decrements `remaining`; that it is positive is the
  frontier invariant checked by the existing Kani scan harness.
- `Scan` copies `ctrl` and `mask` from the table in `Scan::new`; that the table
  shape is unchanged while a scan is live follows from the borrow, not from
  Verus.

Unsafe-site bindings, 102 in total:

| Binding | Sites |
| --- | ---: |
| Maintained caller that calls product kernels | 55 |
| Retained ownership/credit lemma (`hash_retire_progress` 11, `hash_clone_publication` 5, `hash_physical_duplicates` 3, `hash_layout_ranges` 3, `hash_rehash_pending` 2, `hash_insert_credit` 1) | 25 |
| Kani replacement harness | 3 |
| `Send`/`Sync` marker impls (no numeric content; compile-time capability cases) | 12 |
| `entropy.rs` system calls | 7 |

```sh
python3 scripts/verify_hash_map.py --kani --mutations
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 317 verified, 0 errors |
| Weakened maintained callers | 13 of 13 rejected (316 verified, 1 error each); new: `scan_half_group`, `scan_past_last_group`, `scan_wrong_base`, `vacant_fallback_unchecked` |
| Changed product kernels, compiled | 4 of 4 rejected by Verus |
| Changed product callers, compiled | 10 of 10 refused by text comparison; new: `scan-group-bound`, `scan-base`, `get-mut-slot`, `extract-slot`, `entry-index-source` |
| Earlier controls | 4 of 4 detected |
| Kani | 7 harnesses pass |
| Unsafe census (581 blocks), 30 runner tests | Pass |

Not run: the HashSet area and native tests (no product source changed), the
global certificate, Miri, ASan and timing benchmarks.

## Executed memory checks for the shared slice change (2026-11-11)

These runs were made in a separate session and reported by the user. They were
not repeated in the formal-verification session that maintains this document,
and their raw logs are not in Git.

Run 1, commit `fb3da98`, clean tree, shared and string areas with
`--check miri --check miri-32 --check miri-tree --check asan --keep-going`:

| Area | miri (x86-64 strict) | miri-32 (i686 strict) | miri-tree | asan |
| --- | --- | --- | --- | --- |
| shared | 46 pass | 46 pass | 46 pass | 47 pass |
| string | 15 pass | 15 pass | 15 pass | 18 pass |

No failed, ignored or filtered test. The count difference is four tests gated
by `#[cfg(not(miri))]`. This run did not reach four paths of the changed code:
`try_from_slice[_in]` called directly, the `Rc` copy-on-write clone refusal,
`try_init_slice_in` with zero-sized elements or length zero, and an
initializer that returns `Ok(())` without writing.

Run 2, commit `79339f1` with the then-uncommitted `tests/shared.rs` that became
`0acf911`, shared area, same checks. `0acf911` adds 13 tests for those four
paths (`Rc` and `Arc` variants of direct `try_from_slice` success and refusal
at each of five positions, the `Rc` copy-on-write refusal, zero length,
zero-sized elements with and without `Drop`, and the unwritten slot at the
first, a middle and the last position, which returns
`InitError::Storage(Error::Uninitialized)`).

| Configuration | Tests run | Failed / ignored / filtered |
| --- | ---: | --- |
| Native `cargo test --offline --features alloc --test shared` | 45 | 0 / 0 / 0 |
| miri | 59 | 0 / 0 / 0 |
| miri-32 | 59 | 0 / 0 / 0 |
| miri-tree | 59 | 0 / 0 / 0 |
| asan | 60 | 0 / 0 / 0 |

All 13 new tests were found by name as `ok` in the four memory configurations.
In a scratch copy, removing the prefix cleanup of `init::prefix` and shortening
it by one each made 10 tests fail, 8 of them new. The success, zero-length and
empty-source tests do not fail under those two changes; their defect detection
was not checked.

No product source changed between `79339f1` and the commits after it up to this
record. The string area was not rerun after `fb3da98`.

## Hash-table ownership and credit callers (2026-11-11)

No product source changed after `bfb1fe7`. This unit changes
`tools/memory-proof/hash-map.verus.rs`, `scripts/hash_map_bindings.py` and
`scripts/verify_hash_map.py`.

The arithmetic and positions of the remaining callers moved into maintained
functions that call the product kernels. The two invariants those callers
depend on are not proved; they are the stated premises of the functions.

| Product site | Maintained function | Product kernels called | Stated premise |
| --- | --- | --- | --- |
| `IntoIter`/`Drain` `next` and `fold` (blocks and length writes), `Clear::run` length writes | `hash_retire_caller` | `mirror`, bounded sub | `len >= 1`: each yielded slot is counted once in `len` |
| `HashMap::resize` (block and length write) | `hash_resize_caller` | `mirror`, `tag`, `capacity_for`, `checked_sub` | Source slot initialized; the two allocations are disjoint. The credit subtraction is checked at run time |
| `HashMap::try_clone` (two blocks and length write) | `hash_clone_caller` | `checked_sizes`, `mirror`, `capacity_for`, bounded add/sub | The cloned FULL slots number at most the capacity (the source's `len`); both tables have the same shape |
| `HashMap::insert_vacant_at`, `VacantEntry::initialize` | `hash_insert_at_caller` | `capacity_for`, `tag`, bounded add/sub | `len + growth + deleted == capacity`, `EMPTY ==> growth > 0` |
| `Free::drop` | `hash_allocation_caller` | `checked_sizes` | The guard holds the admitted layout |
| End of `RawTable::rehash` and of a pending `Clear::run` | `hash_growth_caller` | `capacity_for`, bounded sub | `len <= capacity` |

Sites that keep a lemma or harness for a fact that is not arithmetic, with the
maintained callers that cover their positions recorded beside it (`also` in the
binding):

| Site | Contract | Positions covered by |
| --- | --- | --- |
| `try_get_disjoint_mut`, `get_disjoint_unchecked_mut` (3) | `hash_physical_duplicates` | `hash_slot_caller` |
| `RawTable::rehash` (2 blocks) | `hash_rehash_pending` | `hash_set_control_caller`, `hash_slot_caller`, `hash_rehash_distance_caller`, `hash_growth_caller` |
| `RawTable::empty`, `IntoIterator::into_iter` (2) | `hash_layout_ranges` | none; constant metadata and an ownership move |
| `replace_key` (3) | Kani replacement harness | unchanged |

The two invariants that remain premises for the whole table:

- Credit: `len + growth + deleted == capacity` after every operation.
- Tag and ownership: the FULL tags are exactly the initialized slots and their
  number is `len`.

Both are statements about the contents of the control bytes. `RawTable` is not
visible to this Verus version (`NonNull` field), so they are not attached to the
product type. The existing Kani harnesses check them on concrete small tables
only. `EMPTY ==> growth > 0` is established on the first path of
`prepare_insert_at` by its run-time test; after `try_reserve` it is a premise.

Unsafe-site bindings, 102 in total:

| Binding | Sites |
| --- | ---: |
| Maintained caller that calls product kernels | 73 |
| Retained lemma (`hash_physical_duplicates` 3, `hash_rehash_pending` 2, `hash_layout_ranges` 2) | 7 |
| Kani replacement harness | 3 |
| `Send`/`Sync` marker impls | 12 |
| `entropy.rs` system calls | 7 |

`hash_insert_credit`, `hash_retire_progress`, `hash_clone_publication`,
`hash_probe_progress` and `hash_mirror_padding` no longer bind any site; they
remain in the file as verified lemmas.

```sh
python3 scripts/verify_hash_map.py --kani --mutations
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 323 verified, 0 errors |
| Weakened maintained callers | 19 of 19 rejected (322 verified, 1 error each); new: `retire_two`, `resize_source_mask`, `resize_unchecked_credit`, `clone_double_count`, `clone_without_source_count`, `growth_reversed` |
| Changed product kernels, compiled | 4 of 4 rejected by Verus |
| Changed product callers, compiled | 15 of 15 refused by text comparison; new: `resize-control-index`, `clone-published-tag`, `disjoint-check`, `rehash-final-credit`, `vacant-entry-index` |
| Earlier controls | 4 of 4 detected |
| Kani | 7 harnesses pass |
| Unsafe census (581 blocks), 30 runner tests | Pass |

Not run: the HashSet area and native tests (no product source changed since
`bfb1fe7`), the global certificate, Miri, ASan and timing benchmarks.

### Certificate for `7d7ed21` (2026-11-11)

`7d7ed21` contains the three hash-table units above (kernel callers, position
callers, ownership and credit callers). The campaign started from a clean tree
at that commit. The first invocation was stopped by the two-hour limit after 14
steps (about 114 minutes of step time); the second invocation resumed the same
output directory and ran the remaining steps (about 21 minutes). No step failed
or was repeated.

```sh
python3 scripts/source_certificate.py --output target/verification/replay/unit9-campaign/certificate
python3 scripts/source_certificate.py --resume --output target/verification/replay/unit9-campaign/certificate
```

| Item | Result |
| --- | --- |
| Invocations | 2 (first stopped by the time limit, second resumed) |
| Steps recorded in invocation 1 | budget-native, direct-foundation, direct-verus, shared, string, btree, btree-set, hash-map, hash-set, binary-heap, deque, scratch-slice |
| Steps recorded in invocation 2 | fixture/routing builds, owner-protocol (fresh, no reused prerequisites, with controls), utf8-native, arithmetic-policy, four-profile link |
| `complete` / `reused_models` / inputs unchanged | true / false / true |
| Direct foundation / direct Verus controls | 54 of 54 / 120 of 120 detected |
| Hash-map composition | 323 verified, 0 errors; 24 of 24 negative Verus runs rejected (322 verified, 1 error each) |
| Hash-map controls | 42 of 42 detected: 19 weakened maintained callers, 4 changed product kernels, 15 changed product callers by text comparison, 2 Kani, 1 compiler, 1 model |
| Hash-map source bindings / Kani harnesses | 102 / 7 |
| Shared composition | 295 verified, 0 errors |
| String composition | 272 verified, 0 errors; 4 of 4 solver controls rejected (271 verified, 1 error each) |
| Owner protocol | 515 verified, 0 errors; 51 of 51 controls detected |
| Source obligations | 1,141 connected, 0 open |
| Retained link edges | 101 (release 6, release-lto 9, release-checked 6, debug 80): 89 originating guards attached, 12 forwarding; 0 unproved |
| Link controls | 8 of 8 detected |

The shared, String and owner-protocol compositions each verify 8 more items
than in the `fb3da98` certificate. They verify the whole product crate, which
now includes the hash-table kernel contracts added in `bfb1fe7`.

The link step exits with status 1 because the product fixtures retain the 101
edges listed above; the certificate attaches each of them to a source contract.

Not run in this campaign: Miri, ASan and timing benchmarks.

## B-tree callers through product kernels (2026-11-11)

No product source changed. This unit changes
`tools/direct-verus/btree-adapters.rs`, `scripts/btree_bindings.py`,
`scripts/verify_btree.py` and adds `scripts/btree_callers.py`.

Before this unit the seven node span producers and the search, cursor,
planning, frontier, deletion and packed-shape kernels were already proved on
product source, but most unsafe sites outside `geometry.rs` were bound by
function name to integer lemmas with assumed inputs. They now name a maintained
function that is verified together with the product crate and calls the
product geometry and bounded add/sub with the arguments of the product body.
The product body is compared whole (comments and whitespace removed).

| Product area | Maintained functions |
| --- | --- |
| `node.rs` loops and cleanup | `btree_fix_caller`, `btree_drain_caller`, `btree_destroy_caller`; the seven existing span adapters now also call `btree_fix_caller` with the product's `fix` arguments |
| `btree_map.rs` search and slots | `btree_search_node_caller` (arbitrary comparison replies), `btree_found_slot_caller` |
| `btree_map.rs` insertion | `btree_commit_fit_caller`, `btree_plan_caller`, `btree_commit_split_caller`, `btree_commit_final_caller`, restated `btree_splitpoint` |
| `btree_map.rs` removal | `btree_remove_start_caller`, `btree_rebalance_caller` (one iteration; calls the steal and merge adapters), `btree_shrink_caller` |
| `iter.rs` | `btree_descend_caller`, `btree_cursor_step_caller`, `btree_state_take_caller`, `btree_retire_chain_caller`, `btree_borrow_remaining_caller`, `btree_borrow_interval_caller`, `btree_borrow_take_caller` |
| `operations.rs` | `btree_ordered_caller`, `btree_fix_border_caller`, `btree_count_caller`, `btree_split_path_caller`, `btree_split_off_caller`, `btree_bound_caller`, `btree_clone_caller` |
| `bulk.rs`, `view.rs` | `btree_pillar_caller`, `btree_bytes_caller`, `btree_bulk_push_caller`, `btree_packed_border_caller`, `btree_view_child_caller` |

Loops over a single node (`fix`, `drain`, the key scans, the clone prefix, the
border steals) are restated with the product's bounded arithmetic and proved
to terminate inside the slot or edge range. Loops over tree levels are proved
one iteration at a time for arbitrary header values inside the premises below.

Stated and not proved by Verus (tree premises of the callers):

- Every node count is at most 11 and a stored parent index is at most the
  parent's count.
- A pair cursor has `index < count`; a vacant leaf edge has `index <= count`.
- The level of a node that has a parent is below the tree height, and the
  height is below `usize::BITS`.
- An internal node on a removal or border path has at least one pair.
- The map length is positive when a pair is removed, and a recounted subtree
  is not larger than the map length.
- `commit_insert` consumes one spare per full node on the path, and the node
  reached after the last spare is the one `plan_insert` found not full.
- In packed construction the left sibling of the rightmost child is full.
- The first `bulk_push` runs on an empty map.
- Tracked cursors of `remove_at` are pair cursors of the node they are
  compared with.

Unsafe-site bindings, 225 in total:

| Binding | Sites |
| --- | ---: |
| Maintained caller or adapter that calls product kernels | 147 |
| Retained lemma: node address primitives `key`/`val`/`edge`/`child`/`set_child`/`pairs`/`edges`/`free`, view slices (`btree_layout_slot` 10, `btree_layout_edge` 8) | 18 |
| Retained lemma: count narrowing, spare list, cleanup guards, set replacement (`btree_closed_node_count` 6, `btree_cleanup_prefix` 6, `btree_initial_state` 4, `btree_set_replace_slot` 3) | 19 |
| Retained lemma: state field initialization and owner publication (`btree_cursor_state` 18, `btree_cursor` 1) | 19 |
| Retained credit lemma for append (`btree_planned_allocation`) | 6 |
| `Send`/`Sync` marker impls | 16 |

The pinned core guard projection in `btree_core.py` (debug-map flags,
`is_multiple_of`, masked shift) is unchanged and remains a projection.

```sh
python3 scripts/verify_btree.py --mutations --kani
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 411 verified, 0 errors |
| Weakened maintained callers | 23 of 23 rejected (410 verified, 1 error each) |
| Core formatter control | Rejected (410 verified, 1 error) |
| Changed product callers, compiled | 18 of 18 refused by whole-body comparison |
| Earlier phase controls, compiled | 9 of 9 detected |
| Six direct-source groups with controls | Pass |
| Kani | 21 harnesses pass; 1 of 1 native control detected |
| Whole-body comparisons / maintained callers checked for product calls | 67 / 38 |
| Runner tests | 30 pass |

The area check was run three times. The first run failed because `node::fix`
was missing from the body table and the `fix-loop-bound` control was accepted;
the second failed because one control variant did not compile. Both were
defects of the controls. The figures above are from the third run.

Not run: the B-tree set area, native tests, the global certificate, Miri, ASan
and timing benchmarks.

## Scratch and binary heap callers through product kernels (2026-11-12)

The sealed areas were surveyed first.

| Area | Unsafe sites | Binding before this unit | Geometry visibility |
| --- | ---: | --- | --- |
| HashSet | 0 | Safe delegation, three Kani harnesses, file seals | Uses the hash map kernels |
| Scratch and checked slices | 25 (all in `scratch.rs`, `scratch/owned.rs`) | Kani harness by function name, file seals | `pub(crate)` |
| BinaryHeap | 50 | Kani harness by function name, file seals | `pub(super)` |
| VecDeque | 100 | Kani harness by function name, file seals | `pub(super)` |

HashSet has no unsafe site, so there is nothing to bind to a maintained caller.
VecDeque is not migrated in this unit.

### Product source change

Product source `09b4cf1` to `fc08446`. Visibility only; no executable
statement, contract or attribute other than the visibility changed.

| File | Change | Executable control flow |
| --- | --- | --- |
| `src/collections/binary_heap.rs`, `src/deque.rs` | `mod geometry` to `pub(crate) mod geometry` | Unchanged |
| `src/collections/binary_heap/geometry.rs` | `parent`, `children`, `selected`, `child_offset`: `pub(super)` to `pub(crate)` | Unchanged |
| `src/deque/geometry.rs` | nine functions: `pub(super)` to `pub(crate)` | Unchanged |

Assembly was compared on a scratch copy before the change was applied: release,
`codegen-units=1`, x86-64, 41 `#[inline(never)]` heap and deque entry points
(`u64` and `Box<u64>` elements). After removing symbol hashes and local labels,
all 49 emitted function bodies are identical. The only difference is the order
in which the two instantiations of `VecDeque::grow` are emitted. Timing was not
measured.

The review seals of the four changed files were refreshed. The VecDeque area
and the direct Verus targets that include the two geometry files were not rerun
after the change.

### Scratch

`tools/memory-proof/scratch.verus.rs` is verified with the product crate and
calls `slice::geometry::{admitted_end, retired_last, initialized_next}` and the
bounded subtraction. `scripts/scratch_callers.py` holds the 14 compared product
bodies.

| Product site | Maintained function |
| --- | --- |
| `Scratch::try_push` | `scratch_push_caller` |
| `Scratch::pop` | `scratch_pop_caller` |
| `Scratch::truncate` | `scratch_truncate_caller` |
| `Scratch::copy_admitted`, `try_extend_copy`, `ScratchBuf::copy_admitted` | `scratch_copy_caller` |
| `try_extend_cloned` (both owners) | `scratch_cloned_caller` |
| `Scratch::try_resize_with_init` | `scratch_resize_init_caller` (loop restated, terminates at the admitted length) |
| `try_resize` zero-sized path (both owners), `ScratchBuf::try_clone_from` | `scratch_admitted_length_caller` |

Stated and not proved: the owner length is at most the fixed capacity. Bindings:
19 of 25 sites name a maintained caller and keep their Kani harness as a second
contract; 6 keep only the harness (slice views of the initialized prefix, the
byte-storage cast, state-field initialization).

```sh
python3 scripts/verify_scratch_slice.py --kani --mutations
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 267 verified, 0 errors |
| Weakened maintained callers | 7 of 7 rejected (266 verified, 1 error each) |
| Changed product callers, compiled | 7 of 7 refused by whole-body comparison |
| Kani | 8 harnesses pass |
| Earlier compiled regression controls | 6 of 6 detected |

This run was made at `09b4cf1`, before the visibility change. It was not
repeated at `fc08446`.

### Binary heap

`tools/memory-proof/binary-heap.verus.rs` calls
`binary_heap::geometry::{parent, children, selected, child_offset}` and the
bounded add/sub. Comparison results are arbitrary values. `scripts/heap_callers.py`
holds the 12 compared product bodies; `core::first_child` is restated with a
Boolean for `size_of::<T>() == 0` and both texts are fixed.

| Product site | Maintained function |
| --- | --- |
| `core::sift_up` | `heap_sift_up_caller` |
| `core::sift_down` | `heap_descend_caller` (early return) |
| `core::sift_bottom` | `heap_sift_bottom_caller` (full descent, then `heap_sift_up_caller`) |
| `core::rebuild`, `core::rebuild_tail` | `heap_rebuild_caller`, `heap_rebuild_tail_caller` |
| `core::pop_nonempty`, `BinaryHeap::pop` | `heap_pop_caller` |
| `BinaryHeap::try_push` | `heap_push_caller` |
| `BinaryHeap::into_sorted_vec` | `heap_sorted_caller` |

Every index read or written by these loops is proved below the slice length,
and each loop is proved to terminate, for every word-sized length. Stated and
not proved: a slice of sized elements has at most `isize::MAX` elements, and
the slice length is the heap length. Bindings: 31 of 50 sites name a maintained
caller and keep their Kani harness; 19 keep only the harness (`Hole`
construction, slot moves and restoration, `set_len`, `PeekMut`, `Drain`).

```sh
python3 scripts/verify_binary_heap.py --kani --mutations
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 273 verified, 0 errors |
| Weakened maintained callers | 10 of 10 rejected (272 verified, 1 error each) |
| Changed product callers, compiled | 8 of 8 refused by whole-body comparison |
| Kani | 6 harnesses pass |
| Earlier compiled regression controls | 9 of 9 detected |
| Runner tests | 30 pass (the heap receipt test now lists the three new inputs) |

The heap area was run twice. The first run failed on a control of mine,
`rebuild_from_end`: starting `rebuild` at `end` instead of `end / 2` is still
inside the slice, so Verus was right to accept it. It was replaced by
`rebuild_without_progress`. The figures above are from the second run.

`scripts/maintained_callers.py` now holds the shared correspondence and control
code used by the heap runner. `scripts/source_certificate.py` accepts the new
caller names as verified contracts.

Not run: the global certificate, the VecDeque and HashSet areas, native tests,
Miri, ASan and timing benchmarks.

## VecDeque callers through product kernels (2026-11-12)

### Product source change

Product source `fade107` to `bba2b87`. Only the `verus_spec` text of one
function changed.

| File | Function | Change | Executable control flow |
| --- | --- | --- | --- |
| `src/deque/geometry.rs` | `grown_head` | Two postconditions added: when the ring is wrapped and the head is kept, the low segment fits in the added capacity; when the head moves, it is `new - (old - head)` | Unchanged |

The earlier contract only bounded the result by the new capacity, which does
not bound either copy of `VecDeque::grow`. The direct Verus target that
includes this file was rerun with its controls after the change (below).

### Maintained callers

`tools/memory-proof/deque.verus.rs` is verified with the product crate and
calls all nine functions of `deque::geometry`, the bounded add/sub and, for
retention, the product `compaction::Compaction` state. `scripts/deque_callers.py`
holds the 41 compared product bodies. `VecDeque::physical` is restated with a
Boolean for `size_of::<T>() == 0`; both texts are fixed.

| Product site | Maintained function |
| --- | --- |
| `slot`, `get`, `get_mut`, `swap`, `Drain::next`/`next_back`/`fmt` | `deque_get_caller` |
| `as_slices`, `as_mut_slices`, `Drop` | `deque_slices_caller` |
| `front`, `front_mut`, `try_push_front_mut` | `deque_front_caller` |
| `try_push_back`, `try_push_back_mut` | `deque_push_back_caller` |
| `try_push_front` | `deque_push_front_caller` |
| `pop_front` | `deque_pop_front_caller` (proves the `spare` fact passed to `assert_unchecked`) |
| `pop_back`, `truncate` | `deque_pop_back_caller` (same `spare` fact) |
| `make_contiguous`, `normalize` | `deque_contiguous_caller`, `deque_normalize_caller` (every copy and slice of the four rotation kinds, including the non-overlap of the two `copy_nonoverlapping` calls) |
| `try_shrink_to` | `deque_shrink_caller` (move decision and restoration after refusal) |
| `grow` | `deque_grow_caller` |
| `copy_ring` | `deque_copy_ring_caller` (both chunk loops restated; each chunk inside the allocation; terminates) |
| `try_insert_mut`, `remove`, `rotate_left`, `rotate_right` | `deque_insert_caller`, `deque_remove_caller`, `deque_rotate_caller` |
| `write_slice`, `try_append`, `try_split_off` | `deque_write_slice_caller`, `deque_append_caller`, `deque_split_off_caller` |
| `try_retain_mut`, `Repair::drop` | `deque_retain_caller` |
| `binary_search_by`, `partition_point` | `deque_search_caller` (arbitrary callback replies) |
| `clone_to`, `try_clone_from`, `write_clone_slice`, `try_extend_copy`, `try_resize` | `deque_clone_slices_caller`, `deque_extend_copy_caller`, `deque_resize_copy_caller` |

Stated and not proved:

- The owner state satisfies `ring`: `len <= cap`; `head < cap` when `cap > 0`
  and `head == 0` otherwise; `cap <= isize::MAX` for sized elements; `head == 0`
  for zero-sized elements.
- After `grow`, `try_reserve` or `try_reserve_exact` returns `Ok`, the capacity
  holds the requested length (`len < cap` for one push, `len + n <= cap` for
  `n` elements), and `grow` is entered with a new capacity above the old one.
- The target of `try_split_off` and of the clone paths holds at least the moved
  length.
- A `Drain` index is below the length the ring had when the drain started.
- That ranges written by `write_slice` are free and ranges read are
  initialized. The proofs cover addresses only.

Bindings, 100 sites: 79 name a maintained caller and keep their Kani harness
as a second contract; 19 keep only the harness (state-field initialization 10,
owner publication writes 3, and six blocks: `Allocation::drop`, `from_vec`,
`into_vec`, `try_from_std_in`, `IntoIter::fold`, the tail move of
`Drain::drop`); 2 are `Send`/`Sync` marker impls.

```sh
python3 scripts/verify_deque.py --kani --mutations
python3 scripts/verify_direct_verus.py --target deque --target binary-heap --mutations --output target/verification/direct-verus-geometry
cargo test --locked --offline --all-features --test deque --test binary_heap --test scratch_slice
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 289 verified, 0 errors |
| Weakened maintained callers | 21 of 21 rejected (20 with 288 verified and 1 error; the weakened `ring` predicate with 271 verified and 18 errors) |
| Changed product callers, compiled | 17 of 17 refused by whole-body comparison |
| Kani | 9 harnesses pass |
| Earlier compiled regression controls | 10 of 10 detected |
| Direct Verus, `deque` and `binary-heap` targets after the source changes | 13 and 8 obligations verified, 0 errors; 10 of 10 source controls detected |
| Native tests, three area targets | 18, 19 and 12 pass |
| Runner tests | 30 pass |

The area check passed on its first run. Before it, one planned control
(`get_at_length`) was replaced because a slot address at `index == len` is
still inside the allocation; it would not have been a defect for Verus.

Not run: the global certificate, the scratch area at this commit, the HashSet
area, Miri, ASan and timing benchmarks.

### Certificate for `a758d85` (2026-11-12)

`a758d85` contains the B-tree, scratch, binary-heap and VecDeque caller units
above (`9ddb2e3`, `fade107`, `bba2b87`, `a758d85`). The campaign started from a
clean tree at that commit. The first invocation was stopped by the two-hour
limit after 14 steps (about 119 minutes of step time); the second invocation
resumed the same output directory and ran the remaining steps (about 22
minutes). No step failed or was repeated.

```sh
python3 scripts/source_certificate.py --output target/verification/replay/unit13-campaign/certificate
python3 scripts/source_certificate.py --resume --output target/verification/replay/unit13-campaign/certificate
```

| Item | Result |
| --- | --- |
| Invocations | 2 (first stopped by the time limit, second resumed) |
| Steps recorded in invocation 1 | budget-native, direct-foundation, direct-verus, shared, string, btree, btree-set, hash-map, hash-set, binary-heap, deque, scratch-slice |
| Steps recorded in invocation 2 | fixture/routing builds, owner-protocol (fresh, no reused prerequisites, with controls), utf8-native, arithmetic-policy, four-profile link |
| `complete` / `reused_models` / inputs unchanged | true / false / true |
| Direct foundation / direct Verus controls | 54 of 54 / 120 of 120 detected |
| B-tree composition | 411 verified, 0 errors; 24 of 24 negative Verus runs rejected (410 verified, 1 error each) |
| B-tree caller controls | 41 of 41 detected: 23 weakened maintained callers, 18 changed product bodies by text comparison |
| B-tree bindings / Kani harnesses / phase controls | 225 / 21 / 9 of 9 |
| Binary-heap composition | 273 verified, 0 errors; 10 of 10 weakened callers rejected (272 verified, 1 error each) |
| Binary-heap controls | 18 of 18 caller controls (10 solver, 8 text); 9 of 9 existing controls; 6 Kani harnesses |
| VecDeque composition | 289 verified, 0 errors; 21 of 21 weakened callers rejected (20 runs with 288 verified, 1 error; `ring_head_at_capacity` with 271 verified, 18 errors) |
| VecDeque controls | 38 of 38 caller controls (21 solver, 17 text); 10 of 10 existing controls; 9 Kani harnesses |
| Scratch composition | 267 verified, 0 errors; 7 of 7 weakened callers rejected (266 verified, 1 error each) |
| Scratch controls | 14 of 14 caller controls (7 solver, 7 text); 6 of 6 existing controls; 8 Kani harnesses |
| Hash-map composition | 323 verified, 0 errors; 24 of 24 negative Verus runs rejected; 42 of 42 controls |
| Shared composition | 295 verified, 0 errors |
| String composition | 272 verified, 0 errors; 4 of 4 solver controls rejected |
| Owner protocol | 515 verified, 0 errors; 51 of 51 controls detected |
| Source obligations | 1,141 connected, 0 open |
| Retained link edges | 101 (release 6, release-lto 9, release-checked 6, debug 80); 0 unproved |
| Link controls | 8 of 8 detected |

The scratch area step in this campaign ran at `a758d85`; the earlier area run
for that unit was at `09b4cf1`.

The link step exits with status 1 because the product fixtures retain the 101
edges listed above; the certificate attaches each of them to a source contract.

Not run in this campaign: Miri, ASan and timing benchmarks.

## Area callers end in documented core conditions (2026-11-12)

No product source changed. Until this unit only the Vec owner composition
proved the documented conditions of `core` pointer calls
([interface](#documented-core-conditions-2026-11-10)); the area models proved
index and count facts. The storage model is now a maintained file of its own
and each area composition includes it.

### Shared files

| File | Content |
| --- | --- |
| `tools/memory-proof/extent.verus.rs` | `byte_valid`, `memory_span`, `ordered_elements_do_not_overlap`, moved unchanged out of `wrapping-memory.verus.rs` |
| `tools/memory-proof/storage.verus.rs` | `RawStorage`, `raw_valid`, `offset_storage`, `slice_caller`, the copy callers, the index callers and the four `Span` callers, moved unchanged out of `raw-owner.verus.rs`; new: `unordered_nonoverlap_caller`, `replace_caller`, `same_element`, `apart`, and the proof-mode statements `storage_range_condition`, `storage_copy_condition`, `storage_replace_condition` |
| `scripts/pointer_predicates.py` | Extraction of the pinned predicate text, moved unchanged out of `verify_pointer_checks.py` |
| `scripts/storage_kernel.py` | Returns pinned predicates, documented conditions, extent lemmas and storage model as one text placed in front of an area model |

`raw-owner.verus.rs` keeps the Vec callers. The owner composition reads
`storage.verus.rs` followed by `raw-owner.verus.rs`; the memory template reads
`extent.verus.rs` followed by `wrapping-memory.verus.rs`.

A maintained caller now passes each pointer operation of its product body to a
storage function. The precondition of that function is the index fact the
caller proves from the product kernels; its postcondition is the documented
condition, and its body runs the pinned predicate under that condition.

| Storage function | Core operation |
| --- | --- |
| `offset_storage(s, index)` | `ptr.add(index)` |
| `slice_caller(s, start, count)` | `slice::from_raw_parts(_mut)`, `write_bytes`, and the read or write of `count` elements (one element is stated as a range of one) |
| `initialized_caller`, `span_caller` | `get_unchecked(index)`, `get_unchecked(range)` |
| `overlapping_copy_caller(s, source, destination, count)` | `ptr::copy` inside one storage |
| `unordered_nonoverlap_caller(s, source, destination, count)` | `copy_nonoverlapping` inside one storage, either order |
| `distinct_nonoverlap_caller(source, destination, start, write, count)` | `copy_nonoverlapping` between two storages that share no byte |
| `replace_caller(s, index)` | `ptr::replace` |
| `layout_caller(s)`, `representation_caller(s)` | `Layout::from_size_align`, `NonNull::new_unchecked` of the storage start |

Stated and not proved, in every area: a product allocation is described by a
`raw_valid` storage of its capacity (non-null, aligned, element size a multiple
of the alignment, byte size within `isize::MAX`, no address wrap); a borrowed
source slice is a `raw_valid` storage of its own length; two owners, or an
owner and a borrowed slice, are `apart`. These come from the allocator result
and from Rust borrows. Initialization, provenance and aliasing stay with the
Kani and Miri checks.

### `ptr::sub`

`RawTable::slot` forms `ctrl.cast::<(K, V)>().sub(index + 1)`. The interface
had no condition for `sub`; it was added on the user's instruction.

| Item | Content |
| --- | --- |
| `doc_ptr_sub(a, addr, count, size)` in `core-contracts.verus.rs` | The byte offset fits `isize`, is not above the address, and the range from the result to `addr` is in bounds of the allocation. Stricter than the text in the same way as `doc_ptr_add`: the range is in bounds for a zero offset too |
| `pointer_sub_precondition` in `pointer-checks.verus.rs` | The pinned `runtime_sub_nowrap` text of Rust 1.95, read from `ptr/mut_ptr.rs` and `ptr/const_ptr.rs` and required to be the same in both; proved true under `allocation_wf` and `doc_ptr_sub` |
| `storage_sub_condition`, `sub_storage` in `storage.verus.rs` | `ptr.sub(count)` from the address of element `from`, for `count <= from <= cap` |
| Controls | `sub_address` (pinned comparison reversed) and `contract_sub_address` (the address clause removed from the condition) |

```sh
python3 scripts/verify_pointer_checks.py --mutations
```

15 predicates verified, 0 errors; 14 of 14 controls detected (7 changed pinned
texts, 7 weakened conditions).

### Statements for every storage of a shape

B-tree nodes and hash tables reach their arrays through a few primitive
functions (`node::key`, `val`, `edge`, `pairs`, `edges`; `RawTable::slot`,
`control`, the group loads). Their maintained callers do not take a storage
argument. Each index function states the documented condition for every valid
node or table:

```text
fn btree_pair_index(i) requires i < 11
  ensures forall n. btree_node(n) ==> btree_pair_slot(n, i)
fn hash_slot_index(mask, index) requires table_shape(mask), index <= mask
  ensures forall t. hash_table(t, mask) ==> hash_slot(t, mask, index)
```

The proofs use `storage_range_condition`, `storage_copy_condition`,
`storage_replace_condition` and `storage_sub_condition`, which state the
documented conditions for any `raw_valid` storage. These are specifications:
the pinned predicates are proved true under the same conditions by
`verify_pointer_checks.py`, and are not run again inside these functions.

Stated and not proved: a live B-tree node is a `btree_node` (key and value
arrays of 11 elements, edge array of 12, each a `raw_valid` storage) and two
nodes of one map hold the same element types; a live table of mask `m` is a
`hash_table` (`m + 1` slots that end at the control pointer, `m + 17` control
bytes of size one).

### B-tree

`tools/direct-verus/btree-adapters.rs` gains the node primitives. The index
assertions of the seven span adapters and of the owner, traversal, operation,
bulk and view callers are replaced by calls to them; an assertion that named no
pointer operation (cursor indices, narrowing to `u16`, loop bounds) is kept.
The six direct B-tree groups of `verify_direct_verus.py` compose the adapters
behind the storage kernel.

| Primitive | Product operation | Index condition |
| --- | --- | --- |
| `btree_pair_address_index`, `btree_edge_address_index` | `key`, `val`, `edge`: the address | `i <= 11`, `i <= 12` |
| `btree_pair_index` | read, write or drop through `key`/`val` | `i < 11` |
| `btree_edge_index` | `child`, `set_child` | `i < 12` |
| `btree_pairs_index`, `btree_edges_index` | `pairs`, `edges` (`ptr::copy`, also between two nodes) | both spans inside 11, inside 12 |
| `btree_replace_index` | `replace_key` | `i < 11` |
| `btree_view_index` | `NodeView::keys`, `values` | `count <= 11` |
| `btree_count_index` | `set_len`: `n as u16` reads back as `n` | `n <= 11` |

Unsafe-site bindings, 225 in total:

| Binding | Sites before | Sites now |
| --- | ---: | ---: |
| Maintained caller, adapter or primitive | 147 | 171 |
| Retained lemma: state field initialization, owner publication, `parent`, `Send`/`Sync` impls (`btree_cursor_state`) | 32 | 32 |
| Retained lemma: `len`, spare-list `take` (`btree_closed_node_count`) | 6 | 3 |
| Retained lemma: node address primitives, set replacement, `free`, deallocation and cleanup guards (before: `btree_layout_slot` 12, `btree_layout_edge` 8, `btree_cleanup_prefix` 6, `btree_set_replace_slot` 3, `btree_cursor` 1; now: `btree_layout_slot` 4, `btree_cleanup_prefix` 4, `btree_cursor` 1) | 30 | 9 |
| Retained lemma: `allocate`, `reserve`, `recycle`, `new` (`btree_initial_state`) | 4 | 4 |
| Retained credit lemma for append (`btree_planned_allocation`) | 6 | 6 |

The 32 `btree_cursor_state` sites include the 16 `Send`/`Sync` marker impls;
two more marker impls of the view are among the `btree_layout_slot` sites.

```sh
python3 scripts/verify_btree.py --mutations --kani
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 487 verified, 0 errors |
| Weakened maintained callers and primitives | 35 of 35 rejected (34 with 486 verified and 1 error; one with 484 verified and 3 errors) |
| Core formatter control | Rejected |
| Changed product callers, compiled | 18 of 18 refused by whole-body comparison |
| Six direct-source groups with controls, phase controls, Kani | Pass |

The first run of this area check failed: the direct groups composed the
adapters without the storage kernel and did not compile. The figures are from
the run after the runner was changed.

### Hash map

`tools/memory-proof/hash-map.verus.rs` gains `hash_slot_index`,
`hash_control_index`, `hash_group_index` and `hash_controls_index`. Thirteen
kernel callers call them next to their existing assertions: slot access through
`sub`, the control byte and its mirror, each sixteen-byte group load, and the
fill or copy of the whole control span.

Not connected: the byte allocation that holds both arrays.
`RawTable::allocation` (`ctrl.sub(offset)` and the rebuilt layout) and the
`add(offset)` of `allocate` keep the byte-count facts of
`hash_allocation_caller`. Site bindings are unchanged (102; 21 on
`hash_layout_ranges`, 3 on `hash_physical_duplicates`, 2 on
`hash_rehash_pending`).

```sh
python3 scripts/verify_hash_map.py --kani --mutations
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 393 verified, 0 errors |
| Negative Verus runs | 31 of 31 rejected (26 weakened callers and primitives, 4 changed product kernels, 1 model) |
| Controls in total | 49 of 49 detected (the above, 15 text comparisons, 2 Kani, 1 compiler) |
| Kani | 7 harnesses pass |

The first run failed on a control that was not a defect: a group load at
`mask + 1` still lies inside the `mask + 17` control bytes and Verus accepted
it. The control now uses `mask + 2`.

### Scratch, binary heap, VecDeque and String

| Area | Change | Composition | Caller controls | Sites on a maintained caller |
| --- | --- | --- | --- | --- |
| Scratch | Seven callers take the storage; `scratch_prefix_caller` for `as_slice`/`as_mut_slice`; the copy takes the source slice as a separate storage | 334 verified, 0 errors | 20 of 20 (13 solver, 7 text) | 21 of 25 (19 before) |
| Binary heap | `heap_element_caller`, `heap_move_caller`, `heap_restore_caller`, `heap_peek_caller`, `heap_peek_drop_caller`; Drain bound to the four `Span` callers of the storage model | 344 verified, 0 errors | 24 of 24 (16 solver, 8 text) | 47 of 50 (31 before) |
| VecDeque | All callers take the storage; `deque_slot_caller`, `deque_clone_slice_caller`, `deque_drain_drop_caller`, `deque_deallocate_caller`, `deque_raw_parts_caller` | 360 verified, 0 errors | 46 of 46 (29 solver, 17 text) | 85 of 100 (79 before) |
| String | `string_replace_caller` and `string_push_caller` take the byte storage and the inserted text | 338 verified, 0 errors | 7 of 7 solver controls | 14 bindings, unchanged |

Sites that keep only a Kani harness: scratch `from_bytes` and three
state-field initializations; heap `set_len` (2) and `Drain::new`; VecDeque
`from_vec`, ten state-field initializations, three owner publication writes
and two marker impls.

`deque_drain_drop_caller` proves the tail move of `Drain::drop` and the ring
state it publishes from `start <= end <= len` and `tail == len - end`, the
values `drain` stores.

```sh
python3 scripts/verify_scratch_slice.py --kani --mutations
python3 scripts/verify_binary_heap.py --kani --mutations
python3 scripts/verify_deque.py --kani --mutations
python3 scripts/verify_string.py --mutations --kani
python3 scripts/verify_owner_protocol.py --mutations
```

All five pass. Kani: 8, 6, 9 and 9 harnesses. Existing compiled regression
controls: 6 of 6, 9 of 9, 10 of 10. The owner composition after the file split
verifies 531 items with 0 errors. These five ran at `08e5f64`; the later
commits changed `verify_direct_verus.py`, `verify_source.py` and one control of
`verify_hash_map.py`, which are inputs of the String and owner fingerprints,
so those two receipts are not for the final tree.

### Not done in this unit

- Shared ownership (`Rc`/`Arc`): done in
  [the next unit](#shared-ownership-callers-2026-11-12).
- The hash-table byte allocation, as stated above.
- B-tree state initialization, publication, allocation and credit lemmas
  (54 sites), hash ownership and credit lemmas (26 sites).
- HashSet and BTreeSet area checks, the shared area check and the global
  certificate were not run after these changes.

Not run: Miri, ASan and timing benchmarks.

## Shared ownership callers (2026-11-12)

No product source changed. `tools/memory-proof/shared.verus.rs` gains
maintained callers for the counter words and the payload pointers;
`scripts/shared_bindings.py` binds each unsafe block and counter body by the
operation it contains. The whole-body texts of the shared functions were
already fixed by `PHASES`.

### Counter callers

Each runs the product kernels of `src/shared/count.rs` on an arbitrary loaded
word.

| Caller | Product body | Statement |
| --- | --- | --- |
| `shared_increment_caller` | `LocalCount::increment` (and `clone_strong`, `clone_weak`, `upgrade` through it); one iteration of `AtomicCount::increment`; `AtomicCount::upgrade` after `observed_live_word` | Calls `admitted_count`; the stored word is the old word plus one and at most `MAX_COUNT` |
| `shared_release_caller` | `LocalCount::release`, `release_weak` | With one owned credit the bounded subtraction does not wrap; the result reports the last credit |
| `shared_buffered_clone_caller` | `AtomicCount::buffered_clone_strong`, `clone_strong` | The guard test `(old + 1) & (MAX_COUNT + 1) != 0` equals `old + 1 > MAX_COUNT` |
| `shared_clone_weak_caller` | `AtomicCount::clone_weak`, one iteration after the LOCK test | The addition does not overflow, the retirement flag is unchanged and the credit part grows by one |
| `shared_source_retirement` (existing) | `retire` and its callers | `retired_state` and `retired_bytes` round-trip the byte count |
| `concurrent_admit` (existing) | `ConcurrentMemoryBudget::reserve` | Calls `budget::admission::admit` |

Which words a live owner can load is not part of these callers. It stays with
the retained protocol lemmas (`shared_valid`, `buffered_valid`) and the Kani and
Loom schedules. `shared_buffered_clone_caller` requires
`old + 1 < 2 * (MAX_COUNT + 1)`, which `buffered_reservation_extent` supplies.
`WEAK_MAX_COUNT` and the guard are restated as `MAX_COUNT - 1` and
`MAX_COUNT + 1`; the product definitions are fixed as text. The composition is
verified in the default configuration; the branch for `jzap_small_counts` is in
the caller and is not verified by this composition.

### Payload callers

| Caller | Product operation |
| --- | --- |
| `shared_data_caller` | `Allocation::data`: `ptr.add(offset)` in the byte allocation |
| `shared_value_caller` | One object of a storage: the header written by `Allocation::new` and dropped by its guard; a sized payload written, read, dropped or borrowed through `data` |
| `shared_header_caller` | The non-null, aligned start of a live storage: `NonNull::new_unchecked` in `control`, `uninit_slice`, the slice and str casts and the unsizing macros; the header fields reached through a control pointer |
| `shared_bytes_copy_caller` | `copy_nonoverlapping` of bytes into the payload (5 sites) |
| `shared_slice_copy_caller` | `copy_nonoverlapping` of typed elements into the payload (3 sites) |
| `shared_prefix_driver` | Slice clone and initialization: runs the product driver; each index is passed to `offset_storage`, `slice_caller` and `initialized_caller` |

Stated and not proved: the allocation made by `Allocation::new` is a
`raw_valid` byte storage of `layout.size()` bytes; `Layout::extend` returns an
offset with `offset + payload <= size`; the payload is a `raw_valid` storage of
its element count that starts at that offset; a source value or slice is
`apart` from the allocation. `shared_header_caller` states only that the start
of a valid storage is non-null and aligned.

### Bindings

172 sites.

| Binding | Before | Now |
| --- | ---: | ---: |
| Maintained caller | 3 | 95 |
| Direct contract on product source (`direct:shared-numeric` 3, `direct:budget-admit` 1) | 1 | 4 |
| Kani harness only | 2 | 2 |
| Retained protocol lemma | 166 | 71 |

Maintained callers by site: `shared_header_caller` 44, `shared_value_caller`
14, `shared_increment_caller` 9, `shared_source_retirement` 7,
`shared_release_caller` 5, `shared_bytes_copy_caller` 5,
`shared_slice_copy_caller` 3, `shared_prefix_driver` 3,
`shared_buffered_clone_caller` 2, `shared_data_caller` 1,
`shared_clone_weak_caller` 1, `concurrent_admit` 1. A site bound to a caller
keeps its earlier lemma as a second contract.

The 71 retained sites have no numeric or address statement: ownership
transfers (`from_raw`, `assume_init`, pinning, the weak unsizing macros,
`control_from_value`), counter bodies that load or store constants (`new`,
`load`, `publish`, `take_last`, `unique`, `initial_weak`, `live_count`,
`external_weaks`, the atomic `release` and `release_weak`,
`observed_live_word`), and the budget forwarding and query bodies in
`budget.rs`, `budget/concurrent.rs` and `budget/owned.rs`.

```sh
python3 scripts/verify_shared.py --mutations --kani --loom
```

| Check | Result |
| --- | --- |
| Composition with the product crate | 380 verified, 0 errors |
| Negative Verus runs | 12 of 12 rejected (379 verified, 1 error each): 9 weakened callers, prefix overrun, swapped admission arguments, unbounded debt |
| Controls in total | 34 of 34 detected (the above, compiled source changes refused by the phase binding, Loom and direct-numeric controls) |
| Kani | 23 harnesses pass, and the six reduced-count harnesses |
| Loom | 12 and 6 tests pass |

The first run of this area check stopped before the composition: the runner
refused any `checked_add` in the model, and `shared_clone_weak_caller` uses it
as its product body does. The text check now covers `concurrent_admit` only.

Not run: the global certificate, Miri, ASan and timing benchmarks.

### Certificate for `df13cf5` (2026-11-12)

`df13cf5` contains the units above that end area callers in documented core
conditions, the `ptr::sub` condition and pinned guard, and the shared ownership
callers. No file under `src/` changed between `a758d85` and `df13cf5`. The
campaign started from a clean tree at that commit. The first invocation was
stopped by the two-hour limit after 9 steps (about 106 minutes of step time);
the second invocation resumed the same output directory and ran the remaining
steps (about 48 minutes). No step failed or was repeated.

```sh
python3 scripts/source_certificate.py --output target/verification/replay/unit14-campaign/certificate
python3 scripts/source_certificate.py --resume --output target/verification/replay/unit14-campaign/certificate
```

| Item | Result |
| --- | --- |
| Invocations | 2 (first stopped by the time limit, second resumed) |
| Steps recorded in invocation 1 | budget-native, direct-foundation, direct-verus, shared, string, btree, btree-set, hash-map, hash-set |
| Steps recorded in invocation 2 | fixture/routing builds, binary-heap, deque, scratch-slice, owner-protocol (fresh, no reused prerequisites, with controls), utf8-native, arithmetic-policy, four-profile link |
| `complete` / `reused_models` / inputs unchanged | true / false / true |
| Direct foundation / direct Verus controls | 54 of 54 / 120 of 120 detected |
| B-tree composition | 487 verified, 0 errors; 36 of 36 negative Verus runs rejected (35 with 486 verified, 1 error; one with 484 verified, 3 errors) |
| B-tree caller controls | 53 of 53 detected: 35 weakened maintained callers, 18 changed product bodies by text comparison |
| B-tree bindings / Kani harnesses / phase controls | 225 / 21 / 9 of 9 |
| Binary-heap composition | 344 verified, 0 errors; 16 of 16 weakened callers rejected (343 verified, 1 error each) |
| Binary-heap controls | 24 of 24 caller controls; 9 of 9 existing controls; 6 Kani harnesses |
| VecDeque composition | 360 verified, 0 errors; 29 of 29 weakened callers rejected (28 runs with 359 verified, 1 error; one with 341 verified, 19 errors) |
| VecDeque controls | 46 of 46 caller controls; 10 of 10 existing controls; 9 Kani harnesses |
| Scratch composition | 334 verified, 0 errors; 13 of 13 weakened callers rejected (333 verified, 1 error each) |
| Scratch controls | 20 of 20 caller controls; 6 of 6 existing controls; 8 Kani harnesses |
| Hash-map composition | 393 verified, 0 errors; 31 of 31 negative Verus runs rejected (30 with 392 verified, 1 error; one with 390 verified, 3 errors); 49 of 49 controls; 7 Kani harnesses |
| Shared composition | 380 verified, 0 errors; 12 of 12 negative Verus runs rejected (379 verified, 1 error each); 34 of 34 controls; 23 Kani harnesses and the reduced model; Loom run |
| Shared bindings | 172 |
| String composition | 338 verified, 0 errors; 7 of 7 solver controls rejected (337 verified, 1 error each) |
| Owner protocol | 531 verified, 0 errors; 51 of 51 controls detected |
| Source obligations | 1,141 connected, 0 open |
| Retained link edges | 101 (release 6, release-lto 9, release-checked 6, debug 80); 0 unproved |
| Link controls | 8 of 8 detected |

Not run in this session: Miri, ASan and timing benchmarks.

### Certificate for `5257f2f` (2026-11-13)

`5257f2f` adds the soundness tests in
[`verification/soundness-tests.md`](verification/soundness-tests.md) and changes
six product bodies (`BTreeMap::remove`, `HashMap::remove`, both
`OccupiedEntry::remove`, and the `IntoKeys`/`IntoValues` projections of both
maps) so that the discarded half of a removed pair is dropped before the other
half is returned. The change contains no unsafe block, index or arithmetic; the
whole-file review hashes of the six files in `scripts/btree_bindings.py` and
`scripts/hash_map_bindings.py` were updated.

The campaign started from a clean tree at that commit. The first invocation
was stopped by a container restart after 3 recorded steps (about 47 minutes of
step time) while the shared step was running; the second invocation resumed
the same output directory and ran the remaining steps (about 89 minutes). No
step failed or was repeated.

```sh
python3 scripts/source_certificate.py --output target/verification/replay/unit15-campaign/certificate
python3 scripts/source_certificate.py --resume --output target/verification/replay/unit15-campaign/certificate
```

| Item | Result |
| --- | --- |
| Invocations | 2 (first stopped by a container restart, second resumed) |
| Steps recorded in invocation 1 | budget-native, direct-foundation, direct-verus |
| Steps recorded in invocation 2 | fixture/routing builds, shared, string, btree, btree-set, hash-map, hash-set, binary-heap, deque, scratch-slice, owner-protocol (fresh, no reused prerequisites, with controls), utf8-native, arithmetic-policy, four-profile link |
| `complete` / `reused_models` / inputs unchanged | true / false / true |
| Direct foundation / direct Verus controls | 54 of 54 / 120 of 120 detected |
| B-tree composition | 487 verified, 0 errors; 36 of 36 negative Verus runs rejected; 53 of 53 caller controls; 225 bindings; 21 Kani harnesses; 9 of 9 phase controls |
| Hash-map composition | 393 verified, 0 errors; 31 of 31 negative Verus runs rejected; 49 of 49 controls; 102 bindings; 7 Kani harnesses |
| Binary-heap composition | 344 verified, 0 errors; 16 of 16 weakened callers rejected; 24 of 24 caller controls; 9 of 9 existing controls |
| VecDeque composition | 360 verified, 0 errors; 29 of 29 weakened callers rejected; 46 of 46 caller controls; 10 of 10 existing controls |
| Scratch composition | 334 verified, 0 errors; 13 of 13 weakened callers rejected; 20 of 20 caller controls; 6 of 6 existing controls |
| Shared composition | 380 verified, 0 errors; 12 of 12 negative Verus runs rejected; 34 of 34 controls; 172 bindings; 23 Kani harnesses |
| String composition | 338 verified, 0 errors; 7 of 7 solver controls rejected |
| Owner protocol | 531 verified, 0 errors; 51 of 51 controls detected |
| Source obligations | 1,141 connected, 0 open |
| Retained link edges | 101 (release 6, release-lto 9, release-checked 6, debug 80); 0 unproved |
| Link controls | 8 of 8 detected |

Not run in this session: ASan and timing benchmarks. The Miri runs of the new
tests are recorded in the linked document.
