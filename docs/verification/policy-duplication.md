# Allocation-policy ownership verification

The policy migration follows BTreeMap checkpoint `546a19b`. It removes the
`AllocationPolicy: Copy` requirement, uses `TryClone` for new policy owners,
borrows for queries and cleanup, and moves policy ownership through consuming
conversions. The contract and operation boundaries are in
[policy duplication](../policy-duplication.md).

The initial migration is recorded at `f72dcfc`; the ledger-snapshot optimization
is recorded at `c4c16e2`. The final combined certificate includes that optimization.

## Executed checks

| Check | Result |
| --- | --- |
| Normal and overflow-checked release | 242 cases each: 216 runtime tests and 26 doctests |
| Non-Copy policy oracle | 14 scenarios each under native tests, x86_64 Miri, i686 Miri, and strict Tree Borrows |
| Collection memory paths | 148 scenarios under strict Tree Borrows across Box, Vec, String, shared owners and unsafe-path fixtures |
| AddressSanitizer | 217 runtime cases, including the separate moving-reallocation fixture |
| Thread bounds | 76 Send/Sync cases; 12 weakened-bound mutations rejected |
| Unsafe census | 415 blocks, 85 ordinary and 2 macro unsafe signatures, 28 unsafe implementations |
| Combined source certificate | 846 obligations; zero open obligations and zero unproved final-link edges |
| Final links | Release, LTO release, overflow-checked release and debug; 62 originating guards and 5 forwarding calls closed; all 8 exit controls detected |
| Code generation | 35 Box wrappers and 100 specialized Vec loops checked; Box drop/iteration match std; no outlined allocation-guard destructor in the checked Vec loops |
| Build boundaries | Core-only Clippy, all-features Clippy/rustdoc, wasm32 and thumbv7em builds pass |

The certificate input SHA-256 is
`1d34c148cef993cf77395ff010f8ce77f65f464723993a535795838cd423bc04`.
Rust 1.95.0, nightly 2026-10-30, Kani 0.68.0, and the repository's pinned Verus
installation were used. The local certificate is
`target/verification/policy-duplication/certificate-ledger/summary.json`.

The non-Copy oracle tracks owners independently from charges and refuses a
selected duplication attempt. Its last owner asserts that occupied and reserved
bytes have already been returned. Append checks every acquisition ordinal in
both shared and distinct accounting domains. Other paths cover refused clones
and splits, shared COW and final Weak release, rejected conversions, initializer
errors and missing writes, budget refusal, streaming splice, owning-iterator
cleanup, deliberate leaks with unsafe recovery, and one destructor unwind.

Actual Rust budget and node harnesses, source-connected numeric/guard models,
and compiled/semantic mutation controls are included in the combined check.
Kani's atomic checks are sequential; nine actual-source Loom scenarios supply
the concurrent interleaving checks. Typed ownership, initialization, aliasing
and destructor paths use the compiler, Miri and sanitizer checks. This is the
agreed division of verification work, rather than a separation-logic proof of
all Rust pointer provenance. Public unsafe and returning-callback contracts
remain explicit.

## Memory and timing

The renewed BTreeMap allocator run contains 12,288 observations. Maximum ratios
to std are 1.0000 for retained requested bytes, 1.1404 for peak requested bytes,
1.0214 for retained usable bytes, and 1.1381 for peak usable bytes. These are
allocator observations, not RSS limits. Handle sizes remain 24 bytes unmetered
and 32 bytes with a borrowed budget; the extra handle bytes are reported
separately from allocation storage.

The four campaigns contain **531 valid intervals**, every interval at least **one second**,
and [142 aggregate rows](policy-benchmarks.csv). Three seeds use equal inputs,
epochs and operation counts across the source baseline, initial migration and
ledger-snapshot variant. Each executable also runs direct std and repeated-std
controls. Build and solver work completes before timing begins.

The selected current-source comparisons contain thirteen default-policy means,
all within the observed +15% range (**−49.3% to +6.8%**). These cover Vec clone,
split and splice staging; Box slice/text cloning; BTreeMap construction, clone,
append and owning iteration; and Rc/Arc construction and COW. The three-operation
BTreeMap ledger recheck covers nine product policy means, all within +15%.
These are three-seed workload observations; the CSV retains seed ranges and
repeated-std controls, whose differences span −9.8% to +11.5%. This renewal does
not infer confidence-interval acceptance or a bound for every caller context.

### Iterator state traffic

The initial full campaign measured local-budget owning iteration at +18.7%
against std. A matched recheck reproduced the effect. Each complete call includes
cloning a prepared source map, consuming its entries and destroying the output:

| Variant | std ns/call | Local-budget ns/call | Relative to its std |
| --- | ---: | ---: | ---: |
| Source baseline `546a19b` | 4,952.3 | 4,539.9 | −8.3% |
| Initial policy migration | 5,005.4 | 5,987.2 | +19.6% |
| Ledger snapshot `c4c16e2` | 4,598.1 | 4,898.8 | +6.5% |

Capturing `&self.policy` in the retirement closure made LLVM materialize the
iterator state in memory. Normal leaf iterations stored and reloaded both the
packed cursor and the remaining count. The measured build included these
additional accesses:

```asm
mov    %rax, 0x40(%rsp)       # remaining-count store
mov    %bp,  0x28(%rsp)       # packed-cursor store
mov    0x40(%rsp), %rax       # remaining-count reload
movzwl 0x28(%rsp), %ebp       # packed-cursor reload
```

Snapshotting `self.policy.budget()` captures a non-owning accounting value.
The policy remains in the iterator, and the borrow keeps the ledger alive.
LLVM then keeps those cursor/count values in registers on normal leaf
iterations. The extra traffic scales with consumed entries. Changing the
policy reference passed by clone construction did not remove these spills;
the retained fix changes the retirement capture. It uses the standard build
settings and adds an exact layout/accounting binding plus a compiled wrong-kind
deallocation control. The recheck reduces local-budget elapsed time per call
by 18.2% against the initial migration.

### Concurrent-budget lifecycle costs

Three concurrent-budget means remain above the observed +15% range:

| Operation | std ns/call | Concurrent-budget ns/call | Extra ns | Change |
| --- | ---: | ---: | ---: | ---: |
| Rc construction | 11.288 | 13.037 | 1.749 | +15.5% |
| Arc construction | 11.178 | 13.884 | 2.706 | +24.2% |
| Arc shared COW | 12.788 | 16.903 | 4.115 | +32.2% |

The source baseline already measured these at +12.4%, +19.9% and +39.4%.
Concurrent admission uses a successful compare-exchange before allocation and
an atomic subtraction after deallocation. Commit adds no second atomic update.
The policy adds a header word, and final release retains retirement/refund work
after free. These costs occur per allocation lifecycle; shared handle-only
operations reuse the header policy. The earlier
[shared report](shared.md) records the same cost family. These budget-policy
results remain separate from the default-policy comparison.

Raw recordings, executable/manifest digests and assembly stay under
`target/verification/policy-duplication/`.

## Reproduction

```sh
cargo test --locked --all-features
RUSTFLAGS='-C overflow-checks=yes' cargo test --locked --release --all-features
cargo +nightly-2026-10-30 miri test --locked --all-features --lib policy_ownership_tests
cargo +nightly-2026-10-30 miri test --locked --all-features --target i686-unknown-linux-gnu --lib policy_ownership_tests
MIRIFLAGS='-Zmiri-tree-borrows -Zmiri-strict-provenance' cargo +nightly-2026-10-30 miri test --locked --all-features --lib policy_ownership_tests
python3 scripts/source_certificate.py --output target/verification/policy-duplication/certificate-ledger
```

The maintained main verification script also includes the policy unit tests in
its Miri and sanitizer suites. No automatic remote CI is enabled.
