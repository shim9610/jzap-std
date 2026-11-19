# Direct-source migration handoff

> Merged in PR #5 (18 November 2026): `42afdab` ports the staged
> replacement batch path and passes the fresh four-profile global certificate.
> Mixed large replacement with a zero lower hint takes 11.17 µs versus 15.09 µs
> for the scalar baseline and 11.83 µs for std. Three existing small-input
> gaps remain above the std margin, and the small longer replacement with a
> known hint has an unresolved std margin. For timing and verification on the
> merged implementation, use the [Splice batch reproduction guide](verification/vec-splice-batch.md#reproduction).

> Earlier verification checkpoint (18 November 2026): the full run passed 47 of 48 steps.
> The stale clone capability hash was reviewed and HashMap verification passed.
> The scanner now recognizes the exact Kani-only `Extraction::valid_progress`
> invariant. The owner check passes with fresh prerequisites, 533 Verus
> obligations, 52 detected controls and 178 linked unsafe callers. The
> standalone link run detects all eight injected exits. The renewed global
> certificate passed. See the [checkpoint results](verification/vec-final-verification.md)
> and the subsequent [batch certificate](verification/vec-splice-batch.md#verification).

> Current decision (18 November 2026): pause further tool experiments and retain
> the existing Vec projections. No Crux fork/tool patch or additional tool trial
> is scheduled. The Crux enum boundary passes two checks (27/27 conditions) and
> rejects two deliberately injected defects; both original Vec caller probes
> remain blocked by simulator support gaps. These partial results do not retire
> any Vec projection. The recorded work at `4c9b93f` is in the history of
> merged PR #5.
>
> Migration constraint: remove projections only without altering execution. The user
> rejected the runtime loop refactors in `82e7293`, `c8a0275`, `4c4cdd6` and the
> uncommitted vectored controller. Product source/Cargo now matches `63975d7`
> after normalizing a verifier-only geometry visibility declaration; normal
> build bodies, conditions, error paths and inline attributes are restored.
> Keep the earlier authorized ExtractIf optimization. Only the shared-array
> caller migration remains accepted (10 → 9 generated callers). All nine Vec
> callers and outer models remain pending. Previous refactored-controller
> receipts/performance measurements are historical, not current certificates.
> Do not repeat execution refactoring as a verifier workaround. See the
> [corrected migration report](verification/vec-projection-removal.md).

> Historical follow-up (17 November 2026): the
> [assembly optimization report](verification/vec-edit-optimization.md)
> and [selected-first ExtractIf verification](verification/vec-extract-selected-first.md)
> record the initial experiments. The candidate at `f92e94f` passes fresh global source/link
> verification without model reuse. [Repeated cloud timing](verification/vec-extract-repeated-timing.md)
> confirms all-selected improvements and retained-prefix regressions; that
> candidate is not accepted as a general default optimization. The requested
> [deletion-count experiment](verification/vec-extract-deleted-count.md) is at
> `047bcda`: focused proofs and memory/compiler checks pass. Its completed
> twelve-process comparison reduces retained-prefix time 29.9–34.7% against
> the read/write candidate, but increases mixed all-selected time 48.5%
> (std +47.4%). It remains an experiment with no fresh global certificate;
> do not apply the historical `f92e94f` certificate to this representation.
> Compiler controls in that report now isolate a `SimpleLoopUnswitchPass` cost
> decision: changing only its threshold or code-growth multiplier yields a
> five-instruction all-selected path. Product source/defaults stay unchanged;
> timing for those option controls remains unmeasured. The normal-inlining
> follow-up below now recovers five selected instructions under default options.
> The [typed scan follow-up](verification/vec-extract-phase-types.md) now tests
> const generics and NonZero without changing product/default compiler settings.
> Its selected-first candidate passes native and memory checks; the selected
> path is 10 instructions and retained prefix is 7. Twelve-process timing
> observes mixed all-selected -34.7% versus deletion-count, with the verdict
> unresolved by A/A control drift. Mixed retained prefixes regress 6.2–9.2%
> with stable controls and adjusted bounds excluding no change; small
> all-selected remains +24.2% versus std. The typed driver remains experimental,
> with no replacement-driver formal certificate or product adoption.
> The [normal-inlining follow-up](verification/vec-extract-inline.md) changes
> only the private driver's `inline(always)` to `inline`. It reaches five
> selected instructions without changing copies or contracts; prefix work is
> six instructions. Fresh native, memory and focused source checks pass.
> Twelve-process timing confirms all-selected improvements of 23.0% for small
> sizes and 41.5% for mixed sizes versus deletion-count. All A/A controls and
> all twelve adjusted std +15% gates pass. Mixed retained-prefix changes
> (-0.2%, +0.6%) are unresolved; mixed parity regresses 5.0% versus baseline
> but remains within the std bound. The attribute change is retained for this
> measured checkpoint; no new whole-library source/link certificate is claimed.
> The [leak follow-up](verification/vec-extract-leaks.md) now passes all 41 Vec
> scenarios with `detect_leaks=1`, confirming the exit-time scan and detection
> of a deliberately forgotten 257-byte jzap allocation. LeakSanitizer works
> outside the restricted execution-tool sandbox; no system ptrace setting was
> changed. Earlier `detect_leaks=0` runs remain historical ASan-only results.

> Status on 10 November 2026: the staging checkpoint reached `main`
> through merge `c888c29` in PR #3. PR #2 closed. The fresh
> prerequisite replay that this checkpoint had reused is completed there, see
> [direct-source-migration.md](direct-source-migration.md#staging-checkpoint-merge-2026-11-10).
> The saved-state table below describes the 8 November state.

The following sections record the scalar checkpoint and the batch experiment
as of 8 November 2026.

## Saved state

| Item | Location / state |
| --- | --- |
| Main working checkout | `/workspace/jzap-std`, local branch `recovered-btree-map-budget-review` |
| Remote working branch | `work/btree-map-budget-review` |
| Existing Draft PR | #2; no merge, release or automatic CI |
| Validated implementation checkpoint | `736d479`; scalar staging controller and owning Vec leaf |
| Batch experiment | Prototype `468477a` on scalar checkpoint `736d479`. PR #5 later merged the verified implementation at `42afdab`. |
| Candidate adoption | Not adopted. The PR branch retains the validated scalar implementation. No second PR was created. |

Compact timing, allocation and entry-count CSVs record the historical
measurements. Generated builds, proof inputs, disassembly and raw logs remain
ignored.

## Completed in this checkpoint

- Retain/dedup, ExtractIf, Splice fill and excess-collection control now execute
  the same source kernels checked by Verus. Their loop-body lowering is removed
  from `verify_edit_loops.py`; the runner composes original controllers and
  maintained span/reservation callbacks.
- Scalar staging has both-word-domain numeric/control proofs, erased successful
  call history, actual-Rust all-word owning-Vec checks, bounded-source word-state
  checks and all three-byte-prefix content checks. Three Verus and three native
  source defects are detected. The owning leaf checks/reserves its own length;
  controller indices are not unchecked pointer offsets.
- The final caller replay closes 82 state publications, 125 constructor fields,
  176 unsafe callers and four direct controller entries; 23 compiled defect
  controls are detected. It refreshes the staging source checks. Numeric/pointer
  prerequisites were reused, explicitly recorded in the receipt.
- Native/doctests pass 369 cases in each of debug, release and overflow-checked
  release. Runner checks (28), formatting, core/all-feature Clippy, Rustdoc and
  unsafe routing pass. All 40 Vec scenarios pass x86-64/i686 strict-provenance
  Miri, Tree Borrows and ASan. `tests/vec.rs` includes `vec_cases/complete.rs`,
  including unhinted splice, allocation-failure and extraction cases.
- Partial measurement now has `--entry-only`: disposable source instrumentation
  verifies that the selected benchmark actually enters the collector and checks
  complete final contents against std. All 72 old/current entry observations
  pass. Instrumentation is excluded from timing and formal proof inputs.

These are component checkpoints. Remaining owner/core/domain projections and
the final fresh whole-library source/link certificate are not completed.

## Performance scope and open issue

Earlier exact-hint excess inputs finish through fill/growth and never enter
`stage_excess`. Ordinary zero-lower-hint excess also fits the first grown spare
span. A zero-lower-hint replacement of `4 * input_length + 1` values reaches the
collector for positive input lengths. Small lengths are 1–32; mixed are 256–4096.
Both implementations and std receive identical source types, inputs and work.

The 21–23% report refers to this new staging scenario, not to a new slowdown of
all Vec operations or to the earlier nearly identical fill/compaction loops.

| Run / staging size | std | Previous | Current / candidate | vs std | vs previous |
| --- | ---: | ---: | ---: | ---: | ---: |
| Scalar checkpoint, small | 427.346 ns | 456.929 ns | 516.558 ns | +20.88% | +13.05% |
| Scalar checkpoint, mixed | 33.001 µs | 49.742 µs | 40.671 µs | +23.24% | −18.24% |
| Batch experiment, small | 456.181 ns | 689.776 ns | 592.888 ns | +29.97% | −14.05% |
| Batch experiment, mixed | 35.753 µs | 44.498 µs | 36.521 µs | +2.15% | −17.93% |

Each run has 108 intervals across three process seeds; minimum intervals are
1.295 and 1.141 seconds. Means use total seconds divided by total calls. Runs
remain separate: scalar placement/context differs between the two comparisons.
The batch candidate improves the mixed staging condition, but its small-case
gap and verification are unresolved. Do not describe it as performance parity.

Scalar staging validates metadata per element; the batch candidate reuses the
existing initialization cursor to fill reserved spare capacity and validates per
batch. It passes 40 optimized native Vec scenarios. It has no completed Verus,
Kani, caller, Miri/ASan or link validation. Its existing proof interfaces still
need updating. Full tables, allocation differences from std, an allocator
control and CSVs are in the [edit report](verification/vec-edit-source-performance.md#path-covered-staging-checkpoint).

## Reproduction and remaining verification work

- To check current batch staging, use the
  [Splice regression and timing instructions](verification/vec-splice-batch.md#reproduction).
  The historical prototype and scalar comparisons above describe earlier runs.
- For the remaining Buffer, owner/callsite/core, String, shared, tree and hash
  verification work, see the [migration guide](direct-source-migration.md#contract-groups).

Do not replace full-word proofs with finite witnesses, insert assumptions or
external bodies, or replace Python lowering with proof-only Rust algorithm
copies. Native finite contents/provenance checks retain their stated role.
Keep runtime dependencies/nightly requirements out of the product. Do not
expand library scope or start unrelated collection/features at this handoff.

## Historical scalar checkpoint commands and receipts

These commands apply to scalar checkpoint `736d479`. For a snapshot checkout,
use the [current checks](verification/README.md#snapshot-regression-checks).

```sh
python3 scripts/verify_source.py --group vec-staging --mutations
python3 scripts/verify_edit_loops.py --reuse-generated
python3 scripts/verify_owner_protocol.py --reuse-generated --mutations
bash scripts/verify.sh --area vec --test-target vec --check miri --check miri-32 --check miri-tree --check asan
```

Reused prerequisites are a partial replay; omit reuse flags for the eventual
fresh campaign. Inspect commands without running via `scripts/verify.sh --plan`.
Benchmark commands and phases are in the [measurement guide](../tools/throughput-analysis/README.md#extractif-and-splice-fill-source-comparison).
Use fresh output directories and the implementation appropriate to the checkout.

Local receipts, if this workspace survives, are under `target/verification/`:
`staging-quality-delivery`, `staging-memory-delivery`, `owner-protocol`,
`staging-entry-delivery` and `staging-covered-final`.

Pinned tools: stable Rust 1.95.0 / LLVM 22.1.2; Verus
0.2026.11.02.8bf68ee; Kani 0.68.0; Miri/ASan nightly-2026-10-30.
Existing optional 32-byte loop settings are comparison options, not default flags.
