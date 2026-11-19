# RandomState: per-thread keys with returned entropy failures

Design contract for the 2026-11-09 rework merged in
PR #3. Requirement basis: specification 1.1, "hash seed
generation" (entropy failures are returned; no silent fixed-seed fallback) and
the HashMap default-constructor redesign row. Reference: Rust 1.95.0
`library/std/src/hash/random.rs` and the platform table in
`library/std/src/sys/random/` (`mod.rs`, `linux.rs`, `windows.rs`, `apple.rs`,
`arc4random.rs`). Code: `src/collections/hash_map/hash.rs` (per-thread keys)
and `src/collections/hash_map/entropy.rs` (platform sources).

## Why the previous design is replaced

The previous `RandomState::try_new` published one process-wide key pair in a
static `SeedCache` and advanced `k0` with `AtomicU64::fetch_add`. On x86-64 that
is one `lock xadd` per constructed map or set. Local measurement on an
i7-13700KF P-core at a fixed 3.4 GHz (2026-11-09) recorded 25.1 cycles per
`HashMap` `default_new` against std's 5.2, with exactly one locked instruction
per call; a control kernel attributed about 18 cycles to the locked increment.
Std keeps the keys in a thread-local cell and advances them with a plain store.

The atomic cache was chosen to avoid std's panicking lazy TLS initialization.
That reason does not require a process-wide atomic: a `const`-initialized,
drop-free `thread_local!` compiles (Rust 1.95.0 native TLS) to a plain
`#[thread_local] static`, has no lazy initializer, registers no destructor and
cannot fail access. Initialization is then performed explicitly and fallibly.

## Behaviour matched with std

| Aspect | std 1.95.0 | jzap after this change |
| --- | --- | --- |
| Key scope | One `(k0, k1)` pair per thread, created on the thread's first `RandomState::new` | Same |
| Per-construction variation | Returns the current pair, stores `k0.wrapping_add(1)` | Same |
| Initial Linux entropy | `getrandom(GRND_INSECURE)`; on `EINVAL` retry with `GRND_NONBLOCK`; `EINTR` retries; `EAGAIN` under `GRND_NONBLOCK`, `ENOSYS` or `EPERM` fall back to reading `/dev/urandom` | Same order and same process-wide availability flags |
| Other OS errors, short device read | `panic!` | `Err(Error::Entropy { code })` |
| `getrandom` returning 0 or more than requested | Loops forever (0) or slice panic (more) | `Err(Error::Entropy { code: 0 })` |
| Hasher | SipHash-1-3 with the pair | Unchanged (`SipHasher13`) |
| Other platforms | Platform source (table below) | Same source for the platforms in the table below; others, and builds without `std`, return `Err(Error::Entropy { code: 0 })` and accept explicit keys via `from_seed` |

`/dev/urandom` is opened (`O_RDONLY | O_CLOEXEC`), read and closed with direct
`open`/`read`/`close` calls on each fallback; `EINTR` retries, end of file or an
oversize count is refused. Std caches the open file in a process-wide
`OnceLock`; reproducing that cache would need a fallible once-initialization
primitive and is not required for the same keys. `std::fs::File` is not used:
its descriptor and `read_exact` paths retain std-internal panic branches in the
final link (the seed link check's `std-file-control` demonstrates them). The
fallback runs only during a thread's first construction after `getrandom` was
found unusable.

## Platform sources

| Platforms (`cfg`) | Source, as in std 1.95.0 | std on failure | jzap on failure |
| --- | --- | --- | --- |
| Linux, Android | `getrandom` then `/dev/urandom` (above) | `panic!` | `Entropy { code: errno or 0 }` |
| Windows 10+ (`target_os = "windows"`, not `win7` vendor) | `ProcessPrng` from `bcryptprimitives.dll` (`raw-dylib`; x86 uses undecorated import names) | `debug_assert` only (documented never to fail) | `Entropy { code: 0 }` if it returns `FALSE` |
| Apple (`target_vendor = "apple"`: macOS, iOS, tvOS, watchOS, visionOS) | `CCRandomGenerateBytes` | `assert_eq!(status, kCCSuccess)` | `Entropy { code: status }` |
| FreeBSD, NetBSD, OpenBSD, DragonFly, Haiku (`libbsd`), illumos, Solaris | `arc4random_buf` | none (no failure result) | none |
| Everything else (wasm, WASI, Fuchsia, UEFI, Hermit, Redox, SGX, `win7`, ...) | Platform-specific | mostly `panic!` | Not implemented: `Entropy { code: 0 }` |

The platforms outside the table are not claimed as supported. Per the
2026-11-09 cross-platform principle in `AGENTS.md`, they remain explicit
follow-up work rather than silent failures.

## Contract of `RandomState::try_new`

- Returns `Ok` with the calling thread's current pair and advances that
  thread's `k0` by one with explicit wrapping. Other threads are unaffected.
- On the thread's first call, or after a previous refusal on that thread, it
  obtains 16 bytes as above. On refusal it returns `Error::Entropy` and leaves
  the thread uninitialized, so a later call retries. No partial key is
  published. No fixed seed is substituted.
- A thread-local access failure maps to `Error::Entropy { code: 0 }`. For the
  `const`, drop-free cell this branch is unreachable on native-TLS targets; it
  is retained instead of `LocalKey::with`, which would panic.
- Never panics, never allocates, never blocks on entropy (`GRND_INSECURE`,
  `GRND_NONBLOCK` and `/dev/urandom` do not wait for pool initialization).
- Complexity: O(1) after initialization; initialization performs at most a
  bounded number of system calls per retry class plus one device read loop.
  `EINTR` retries are unbounded, as in std.

## Removed API

`collections::hash_map::SeedCache` is removed. It existed only to implement the
process-wide atomic cache and has no std counterpart. `RandomState`,
`from_seed`, `try_new`, `TryDefault`, `TryClone` and `BuildHasher` are unchanged.

## Platform boundaries and premises

- Default entropy requires the `std` feature and a platform in the table above.
- `thread_local!` must lower to native TLS for the access to be infallible and
  allocation-free. Linux, Windows, Apple and the BSD targets use native TLS.
  Android may use emulated TLS (`__emutls_get_address`), whose runtime
  allocates a thread's block on first access; that allocation and its failure
  behaviour are outside jzap and recorded as an unverified platform boundary.
  A target using std's OS-key fallback would also allocate and is not claimed.
- When jzap is linked into a shared object loaded with `dlopen`, glibc may
  allocate the module's TLS block lazily on a thread's first access. That
  allocation is outside jzap and is recorded as an unverified platform
  boundary, not as a jzap guarantee. Executables and statically linked
  libraries use the static TLS block.
- Error classification uses `std::io::Error::kind()` (`Interrupted`,
  `InvalidInput`, `WouldBlock`, `Unsupported`) and the universal Linux value
  `EPERM = 1`, so per-architecture errno numbering is taken from std.

## Verification status by platform

| Platform | Checked here | Not checked here (separate validation) |
| --- | --- | --- |
| Linux x86-64 | Everything in the plan below | none |
| Android aarch64 | Miri cross-interpretation: all 8 seed tests (getrandom outcome classes, per-thread keys) | Native execution, emulated-TLS runtime, link check |
| Windows x86-64 and x86 (MSVC) | Miri cross-interpretation: per-thread keys, refusal/unwind, `ProcessPrng` source test (3 tests each) | Native execution, `raw-dylib` linking, link check |
| macOS aarch64 | Miri cross-interpretation: same 3 tests | Native execution, link check |
| iOS aarch64 | Compiles under Miri; test execution blocked because Miri cannot emulate `pthread_setname_np` on iOS (used by the test harness) | Execution of any test, link check |
| FreeBSD x86-64 | Miri cross-interpretation: same 3 tests | Native execution, other BSDs, Haiku, illumos, Solaris |

Miri replaces the OS calls with its own shims, so these runs check the Rust
logic, memory safety and call signatures, not the operating systems.

## Verification plan

| Property | Check |
| --- | --- |
| Per-thread independence, `k0` advance, refusal leaves thread uninitialized, retry, provider unwind leaves state unchanged | Unit tests on the actual acquisition function with injected providers |
| Every `getrandom` outcome class (`EINTR`, `EINVAL` under insecure, `EAGAIN` under nonblock, `ENOSYS`, `EPERM`, other errors, zero and oversize returns, partial reads) and the device fallback | Unit tests on the actual fill function with injected system-call results |
| Availability flags under concurrent first use | Each flag only moves from available to unavailable with relaxed stores, exactly as std; no ordering protocol exists. The retired `SeedCache` Loom models are removed with it, and the runner now rejects `--check loom` for the hash areas instead of reporting a pass. |
| Progress through the 16 key bytes | No arithmetic: each successful call narrows the remaining slice with `get_mut(written..)`, and a zero or oversize count is refused. The previous `bounded::wrapping_add`/`wrapping_sub` call sites are removed, so no arithmetic obligation remains for Kani or Verus. |
| No panic, allocation or exit in the linked seed path | The no_std link fixture cannot link std entropy, so `scripts/check_seed_link.py` builds `tools/seed-link-audit` (std, `panic=abort`) and walks the disassembled call graph from `RandomState::try_new`, `HashMap::try_new` and `HashSet::try_new`, following direct calls and GOT/`lea` function-pointer loads; libc entries are leaves. Controls: `unwrap`, `Box` allocation and the std `File` path must be reported. Runner: `--check link` for `all`/`hash-map`/`hash-set` |
| Memory safety of the OS calls | Each pointer and length comes from one live `&mut [u8]` (or a static C string for `open`); native, Miri (strict provenance, i686, Tree Borrows) and ASan tests on Linux, Miri on the other platforms above. The unsafe census counts 7 OS-call blocks in `entropy.rs` (`getrandom`, `open`, `read`, `close`, `ProcessPrng`, `CCRandomGenerateBytes`, `arc4random_buf`) under the HashMap group (G17: 55 → 61, total 570 → 576; the previous `os_seed` had one). The certificate binding routes them by the existing default HashMap rule, as for the previous `os_seed`. |
| Existing HashMap/HashSet behaviour | Area tests, API pins, source review hashes renewed after review |
| Tests detect representative defects | Seven injected defects (no `k0` advance, readiness set before a successful provider, no `EAGAIN` fallback, `ENOSYS` not disabling `getrandom`, `EINVAL` not disabling the insecure flag, oversize count accepted, fixed-seed substitution) each fail at least one test |

## Performance and memory criteria (fixed before measurement)

- `RandomState::try_new` followed by `build_hasher`, and `HashMap`/`HashSet`
  `default_new`, against std on the same CPU under the repository's
  one-second-interval protocol: unbudgeted mean within +15% of std.
- No locked instruction in the steady-state construction path (checked with
  `mem_inst_retired.lock_loads` and the disassembly).
- Static memory: the 24-byte `SeedCache` is removed; two one-byte availability
  flags are added on Linux/Android. Thread-local memory: one `ThreadKeys`
  (`Cell<(u64, u64)>` plus a `Cell<bool>` readiness flag, 24 bytes on 64-bit)
  per thread, the same size as std's lazily initialized cell.

## Results (2026-11-09, local)

Environment: i7-13700KF, CPU 6 (P-core, sibling CPU 7 offline), turbo off,
`performance` governor with minimum = maximum = 3.4 GHz; Rust 1.95.0;
`tools/throughput-analysis` release profile. Standard protocol: seeds 617,
6203 and 96011, every interval at least one second, means are
`sum(seconds) / sum(calls)`. Raw records stay under
`target/verification/local-2026-11-09/random-state/`.

| Operation | std ns/op | jzap unbudgeted ns/op | Change | Seed ratio range | std control |
| --- | ---: | ---: | ---: | --- | ---: |
| `random_state_new` (builder only) | 1.333 | 1.157 | −13.2% | 0.80–0.91 | −3.5% |
| `random_state_hash` (builder + one `u64` `hash_one`) | 6.505 | 7.770 | +19.4% | 1.194–1.195 | 0.0% |
| HashMap `default_new`, before (atomic cache) | 1.609 | 7.371 | +358.2% | — | −5.0% |
| HashMap `default_new`, after | 1.611 | 1.905 | +18.2% | 1.07–1.25 | −0.1% |
| HashSet `default_new`, after | 1.893 | 2.481 | +31.0% | — | −0.7% |

Counters on the same CPU (`perf stat`, per call): HashMap `default_new` went
from 25.1 cycles with 1.00 locked instruction to 6.5 cycles with none (std 5.1).
The thread-local part of the construction loop is instruction-for-instruction
the same as std's (`cmpb $0,%fs:..`, two loads, `lea`, one store).

Criteria outcome:

- Builder alone: within +15% (faster than std). Locked instructions: none.
- `default_new` (+18.2% / +31.0%): not met. The remaining ~3 instructions per
  call come from HashMap's empty-table construction, not from `RandomState`:
  std stores two 16-byte constants held in registers, jzap issues three stores
  plus `lea` and `xorps`. This code is outside this change.
- Builder + `hash_one` (+19.4%): not met. jzap executes 120.6 instead of 99.6
  instructions per call: `<u64 as Hash>::hash` is not inlined and runs the
  general byte-oriented `SipHasher13::write` with the hasher state in memory,
  while std's SipHash-1-3 is fully inlined. The cost is in the existing
  `SipHasher13`, not in key acquisition, and is left for a separate decision.

### Follow-up: SipHash integer writes and copy-then-publish resize

Commits `aec07df` (integer `short_write`, std 1.95 inline hints on lookups) and
`ca54f50` (resize copies, then publishes). Same environment and protocol; raw
records under `target/verification/local-2026-11-09/{current,final}/`. "Before"
is the same campaign on `e0487d4`; "After" is `ca54f50`. Unbudgeted means; the
budget columns are local / concurrent policies.

| Operation | std ns/op | jzap ns/op | Before | After | Seed ratio range | Budgeted (local / concurrent) | std control |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: |
| HashMap `random_state_new` | 1.216 | 1.184 | +32.1% | -2.7% | 0.971–0.977 | -8.2% / -4.4% | -0.6% |
| HashMap `random_state_hash` | 6.615 | 6.452 | -0.1% | -2.5% | 0.974–0.977 | -0.8% / -0.8% | -0.0% |
| HashMap `default_new` | 1.509 | 1.921 | +25.4% | +27.3% | 1.265–1.279 | +49.1% / +26.1% | +6.8% |
| HashMap `get` | 9.676 | 10.165 | +1.3% | +5.1% | 1.047–1.058 | +4.9% / +4.7% | -0.5% |
| HashMap `update` | 9.765 | 9.843 | +14.9% | +0.8% | 0.999–1.014 | +0.9% / +1.6% | -0.3% |
| HashMap `entry` | 9.307 | 9.521 | +2.1% | +2.3% | 1.018–1.032 | +2.8% / +2.3% | +0.3% |
| HashMap `build` | 18,074.716 | 16,895.407 | +8.7% | -6.5% | 0.933–0.936 | -6.4% / -5.8% | +0.0% |
| HashMap `reserve` | 6,387.125 | 5,584.788 | +4.2% | -12.6% | 0.871–0.879 | -13.6% / -13.6% | -0.2% |
| HashMap `shrink` | 11,825.339 | 11,311.871 | +14.3% | -4.3% | 0.938–0.969 | -4.2% / -5.0% | -0.6% |
| HashMap `remove_cycle` | 26.570 | 27.971 | +7.3% | +5.3% | 1.043–1.067 | +5.5% / +5.1% | -0.5% |
| HashMap `iter` | 658.669 | 712.644 | +7.3% | +8.2% | 1.076–1.087 | +6.6% / +7.2% | -0.3% |
| HashMap `drain` | 1,087.038 | 1,224.105 | +12.3% | +12.6% | 1.117–1.143 | +11.8% / -4.2% | -0.0% |
| HashSet `default_new` | 1.898 | 2.415 | +24.9% | +27.2% | 1.183–1.350 | +29.5% / +34.6% | -4.4% |
| HashSet `get` | 9.162 | 9.954 | +5.4% | +8.6% | 1.066–1.101 | +8.4% / +7.2% | +0.7% |
| HashSet `build` | 14,139.943 | 14,333.324 | +3.1% | +1.4% | 1.006–1.025 | +1.1% / +1.2% | +0.7% |
| HashSet `reserve` | 6,002.111 | 6,111.783 | +7.7% | +1.8% | 1.007–1.024 | +1.3% / +0.9% | +0.2% |
| HashSet `shrink` | 11,296.589 | 11,611.997 | +9.4% | +2.8% | 1.022–1.032 | +2.7% / +1.3% | +0.7% |
| HashSet `iter` | 603.465 | 608.983 | +16.0% | +0.9% | 0.997–1.022 | +2.0% / +25.6% | -0.0% |
| HashSet `extract` | 2,715.624 | 3,154.218 | -7.3% | +16.2% | 1.096–1.229 | +5.6% / +12.6% | -1.2% |
| HashSet `owned_union` | 19,917.926 | 21,483.386 | +7.5% | +7.9% | 1.062–1.087 | +8.2% / +7.8% | +0.0% |
| HashSet `owned_difference` | 15,689.724 | 16,642.187 | +5.5% | +6.1% | 1.057–1.066 | +5.7% / +4.1% | +0.1% |
| HashSet `owned_symmetric_difference` | 31,763.875 | 34,007.935 | +9.3% | +7.1% | 1.069–1.074 | +7.9% / +6.8% | -0.1% |

Cause analysis of the remaining gaps (assembly and `perf` counters on CPU 6;
not fixed):

- `get` (+5–9%): the product lookup is 141 instructions against 139 for std's
  same-algorithm instance (fixed hasher). The benchmark wrapper is not inlined
  into its loop (LLVM inline cost 565, threshold 525). std's seeded instance is
  inlined because its `hash_one` is still a call when the decision is made (cost
  195). `inline(always)` on HashSet `get` changed nothing and was reverted.
- HashMap `iter` (+4–10% between runs, about 0.11 ns or 0.37 cycles per element
  at 502 elements per map): jzap retires 1.3% fewer instructions, the same
  branch mispredictions and L1 replacements. The difference is mostly
  bad-speculation slots (586M for std, 581–716M across the three jzap
  instances). The index-based scanner recomputes the data pointer and checks
  the termination bound per group; hashbrown advances a data pointer. Draining
  the mask before refilling (hashbrown's order) removed the recomputation but
  added one address computation per element (+4% instructions, unchanged
  cycles) and was reverted.
  Scanner review (2026-11-10, CPU 6, seed 617, `perf stat` over 6,000 epochs of
  64 maps): the compiled fold loop already carries a data pointer (LLVM
  strength-reduces the index: `add $-0x100` per group). Per element jzap runs
  11 instructions and std 12; per group jzap runs about 15 (bound check and a
  four-instruction pointer rebase after each drained group) and std 8; per
  epoch 376.6K against 380.6K. Cycles follow branch mispredictions of the
  data-dependent group-exit branch, which both implementations have, and those
  follow code placement, not the scanner:

  | Build | Instance | Mispredictions | Cycles |
  | --- | --- | ---: | ---: |
  | default | std | 6.90M | 700.5M |
  | default | jzap unmetered / local / concurrent | 6.73M / 5.92M / 6.10M | 737.8M / 718.8M / 725.0M |
  | `-align-loops=32` | std | 7.23M | 718.6M |
  | `-align-loops=32` | jzap unmetered / local / concurrent | 5.11M / 10.67M / 10.20M | 687.4M / 862.3M / 850.4M |
  | `-align-loops=64` | std | 5.50M | 674.7M |
  | `-align-loops=64` | jzap unmetered / local / concurrent | 5.69M / 7.42M / 7.74M | 693.8M / 781.2M / 790.3M |

  The three jzap instances are the same loop instantiated at different
  addresses and differ by up to 25% in cycles; jzap unmetered is 4.3% faster
  than std in one build and 5.3% slower in another. The default-build unmetered
  row has fewer mispredictions than std but more cycles; that part is not
  explained. No scanner change is made: an explicit pointer-carrying scanner
  would remove at most the per-group rebase (about 1% of instructions) and
  would replace the verified index frontier.
- HashSet `iter`, HashMap `drain`, `random_state_new`: identical instruction
  streams at different addresses give different results (HashSet `iter`
  mispredictions 3.1M–6.1M between three byte-identical loops; budget columns
  disagree in sign). Recorded as placement effects; the exact
  microarchitectural mechanism is not identified, and `drain` was not compared
  at assembly level.
- HashSet `extract` (+16.2%, previously −7.3%): jzap instructions are unchanged
  across the three binaries (about 237M against std's 219M); std's mispredictions
  fell from 3.3M to 2.3M in this build. Not a regression of these commits.
- HashSet `owned_*` (+6–8%): std builds results with `collect` (internal
  `fold`), jzap's `try_own` uses an external `next()` loop because stable Rust
  cannot implement `try_fold`. `inline(always)` on the algebra iterators moved
  results by ±2–6% in both directions and was reverted.
- `default_new` (+27%): unchanged. The steady-state loop is 24 instructions
  against std's 21; the thread-local key part is identical. In LLVM IR the
  empty table `RawTable::empty()` (`ctrl = 1`, three zero words) becomes
  `store ptr 1` plus a 24-byte `llvm.memset`, lowered to
  `movq $1; lea; xorps; movups; movq $0` on every iteration. std's empty table
  holds the address of a static control group, so its 32-byte constant needs a
  relocation and stays a memory constant: two `movups` loads hoisted out of the
  loop and two `movaps` stores per iteration. Source experiments, each checked
  in the disassembly (2026-11-10) and reverted: hashbrown field order (Rust
  re-laid the fields, no change); `#[repr(C)]` with the zero words first, and
  with `ctrl` between them (still store + 16/24-byte memset, five instructions);
  `ctrl` pointing at a static byte (adds a GOT load, six instructions). No
  source form tried removes the extra stores.

## Global certificate

Renewed on 2026-11-10 at `aa7182c` (local, 65 min 51 s, no reused models):
`bash scripts/verify.sh --area all --check certificate` reports
`complete=True`, 18 steps, 1,137 source obligations with 0 open, and 101
retained link edges (89 originating guards, 12 forwarding) with 0 unproved,
over the four link profiles (6, 9, 6 and 80 retained sites, identical by
function, location and target to `95b289a`). Two inputs had to be renewed
first: the reviewed `src/clone.rs` capability hash (`f117932`; unchanged
CopyWitness constructors since `3f8f2fc`) and the pinned owner-protocol counts
(`aa7182c`; 176 → 178 unsafe callers and 80 → 81 state writes from the Box
direct clone and the Vec witnessed bulk clone). The certificate keeps its
documented boundary: reviewed source projections, direct kernels and executed
memory checks remain distinct, and user callbacks are contracts. Area-wide
Miri for the two newest unsafe paths is listed in the PR as not yet run.
The certificate was rerun and passed again on `c888c29` after the staging
checkpoint merge (1,140 obligations, 0 open; 101 edges, 0 unproved); see
[direct-source-migration.md](direct-source-migration.md#staging-checkpoint-merge-2026-11-10).
