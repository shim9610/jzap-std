# Vec checkpoint verification

The product checkpoint is `54fb5123c6763b506d9e940d30ea492104ad22ce` from
PR #5. Its runtime matches `63975d7` after normalizing the
verifier-only `clone::geometry` visibility declaration. The retained ExtractIf
optimization stays in place. Nine Vec caller projections and the separate
owner/reservation models remain; this campaign does not remove them.

## Completed full-run checks

The full local run selected 48 steps. It passed 47 and failed the final
source/link certificate at the HashMap review gate. Its product inputs remained
unchanged. These results are from that run, not reruns after the review update.

| Check | Observed result |
| --- | --- |
| Native, release, overflow-checked release | 433 tests each; zero failures |
| Strict-provenance Miri, native and i686 | 423 tests each; zero failures |
| Tree Borrows Miri | 423 tests; zero failures |
| AddressSanitizer with `detect_leaks=1` | 433 tests; zero failures |
| Separate leak controls | Normal control exits 0; deliberately forgotten 257-byte allocation detected, exit 23 |
| Fuzzing | Eleven targets, 10,000 runs each |
| Coverage | Lines 88.32%; branches 82.21%; these percentages are observations, not full coverage |
| Compiler and repository checks | API inventories, format, library Clippy, rustdoc, core/wasm32/thumbv7em/no-global-OOM builds, bounds checks, unsafe routing, dependency/license audits and SBOM pass |
| Original global source/link certificate | Incomplete: HashMap review gate rejected the changed `src/clone.rs` hash |

LeakSanitizer ran outside the restricted execution-tool sandbox. The scan was
observed in both separate leak controls; no system ptrace setting was changed.
The forgotten allocation is an injected test defect, not a product leak.

An execution-session interruption killed the original coordinator while its
i686 Miri child survived. Recovery captured the actual Cargo wait status 0,
preserved completed checks and ran the remaining selected checks. The root
receipt records this recovery; no success was inferred solely from log text.

## Copy-capability review update

`scripts/hash_map_bindings.py` hashes all of `src/clone.rs` to require review of
Copy capability constructors and privacy changes. The reviewed file was from
`2a63fcb`; comparison against the current file found exactly this difference:

```rust
#[cfg(not(verus_keep_ghost))]
mod geometry;
#[cfg(verus_keep_ghost)]
pub(crate) mod geometry;
```

`CopyWitness` fields, bounds and constructors are byte-identical. Normal builds
still use the private module declaration. The review update changes only the
expected whole-file hash and its review comment; it does not normalize or
remove the gate and does not change product source.

The updated gate and caller correspondence pass: nine reviewed files, 102
bindings and 20 maintained callers. Separate temporary mutations removing the
`Copy` bound or exposing the private marker field are both rejected by the
review gate. All 30 verification-runner tests pass after activating the Rust
tool environment.

## Fresh source/link certificate

Status: incomplete; stopped at the owner-protocol prerequisite scanner. Updating
the review script changes the certificate input
fingerprint, so this is a new campaign without historical model reuse. It does
not resume the old fingerprint or mark the failed run as passing.

```sh
. /workspace/.devtools/activate.sh
XDG_CACHE_HOME=/workspace/.cache \
  bash scripts/verify.sh --area all --profile full --check certificate \
  --output target/verification/vec-final-certificate-review
```

The previous full run is reproducible with `--area all --profile full
--keep-going`; its saved output is
`target/verification/vec-final-full-54fb512/`. The fresh certificate, runner
tests and product-file identity record are under
`target/verification/vec-final-certificate-review/`. Raw receipts, generated
proofs and logs remain ignored by Git.

The new campaign completed 14 steps: consumer fixture builds, source routing,
native budget, direct foundation, direct Verus, Shared, String, BTreeMap,
BTreeSet, HashMap, HashSet, BinaryHeap, VecDeque and Scratch/slice.

| Fresh result | Observed result |
| --- | --- |
| Direct foundation | 20 groups pass; all 55 injected controls detected |
| Direct Verus | 31 targets pass; 121 injected controls detected |
| HashMap | Complete; seven actual Rust harnesses and 49 controls; source unchanged |
| HashSet / BinaryHeap / VecDeque | Complete; actual Rust harnesses 3 / 6 / 9, with required controls |
| Product source and manifests | All 138 recorded files remain byte-identical |
| Global source/link certificate | Incomplete; owner prerequisites stop before final link |

The original coordinator and certificate process survived an environment
reconnection; their receipts, rather than the lost execution-tool session,
recorded the subsequent steps.

The next failure is in the maintained syntax scanner,
`tools/panic-audit/src/main.rs`: it rejects the Kani-only loop invariant
`state.valid_progress(end, total, minimum_read, minimum_write)` in
`src/vec/compaction_driver.rs`. The scanner recognizes selected scalar getters
and `valid_for`, but not this predicate. Scanning that file alone reproduces
`loop invariant contains an unapproved operation`. The predicate in
`src/compaction.rs` uses `checked_sub`, returns false on subtraction refusal and
otherwise reads state; it is compiled only under `cfg(kani)`. This is a scanner
rejection before numeric proof execution, not a solver counterexample.

The certificate wrapper then reports a secondary missing owner summary because
the failed prerequisite produced none. No global certificate was produced.
Source identity remained unchanged throughout the new run.

## Scanner fix and owner verification

The scanner now checks the full Kani-only `Extraction::valid_progress`
definition, then accepts its exact invocation in `extract_next` under
`all(kani, jzap_loop_contracts)`. The complete source scan and three scanner
unit tests pass. Four temporary mutations are rejected, covering a different
caller, a callback argument, a panic and a callback in the observer body.
The product source and Cargo files match checkpoint `51ed37a` byte for byte.

The affected owner-protocol check passes with freshly generated prerequisites.
Verus reports 533 verified obligations and zero errors. All 52 owner controls
are detected, and the source inventory accounts for 87 state writes, 132
constructor fields and 178 unsafe callers.

The standalone link run detects all eight injected exit controls. It reports
101 retained calls across release, release-LTO, overflow-checked release and
debug. The certificate classifier recognizes all 101, and the global campaign
still needs to match them to verified source contracts.

Reproduce the focused owner check with
`python3 scripts/verify_owner_protocol.py --mutations`. The new global
certificate run uses the command above with output
`target/verification/vec-final-scanner-review/global`. It is in progress at
this checkpoint.
