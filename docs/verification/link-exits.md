# Link-time exit-path checks

**Historical fixture results follow.** The expanded fixture and the next round of
removal and verification results are recorded in the
[panic-path follow-up](panic-paths.md): 5,612 cases per profile, now with zero overflow-checked-release calls and 22 debug core checks/forwarding
calls; see the [arithmetic-policy report](../arithmetic-policy.md). Exhaustive
unreachability remains open.

Tool checkpoint: `1537840` (full identifier in the Vec summary). Product source
remains `37beeb3598e8ac2d94756fcd77644b1adf29d3a9`. This adds a verification
fixture and does not remove checks, change arithmetic, or alter product behavior.

## Result

| Build | Strict link result | Execution result |
| --- | --- | --- |
| Release, opt-level=3, no LTO | Pass: no retained panic-sink/wrapped-exit reference; no UD2/INT3 instructions | 2,148 cases, including 2,018 refusals; zero final allocations and budget charges |
| Release, fat LTO | Same pass | Same 2,148 cases and 2,018 refusals |
| Release, overflow checks enabled | Fail: retained arithmetic-panic edges | Separate diagnostic executable passes the 2,148 cases; strict failure remains |
| Debug | Fail: retained panic edges, plus a prebuilt-core personality reference | Separate diagnostic executable passes the 2,148 cases; strict failure remains |

Eight negative controls are detected: explicit panic, dynamic bounds panic,
infallible alloc::Vec allocation and direct C abort, each in release and LTO
release. Each control fails the final link at the deliberately unresolved sink
or wrapped abort symbol. The ordinary product fixture links in those same builds,
so unrelated build failures are not accepted as successful controls.

These results concern the concrete linked fixture. They do not certify all API
instantiations, callbacks, targets or process-termination mechanisms. In particular,
**checked release and debug do not meet this absence-of-exit-edges criterion**.
No crashing supported input was demonstrated by the executed fault cases.

## Mechanism and scope

The no_std Rust static library uses the actual jzap implementation. Its panic
handler calls an intentionally undefined `jzap_link_audit_panic_reachable` symbol.
A final ELF link with section garbage collection succeeds only when the referenced
panic path is eliminated. Direct references to abort, exit, _Exit, _exit,
quick_exit, pthread_exit and __assert_fail are redirected with linker --wrap to
undefined symbols. Successful binaries are also scanned for UD2/INT3 trap
instructions. This catches retained edges, not just source spellings of panic!.

The C driver supplies a fallible global allocator backed by libc. It injects a
null return at every allocation ordinal reached by each successful run, preserves
the old allocation on realloc refusal, counts live allocations, and checks the
returned status. Each of 65 input seeds also runs with a zero-byte budget.
The two Rust roots verify that occupied and reserved budget counters return to
zero. Earlier benchmark/test unwraps and std startup code are excluded from the
Rust fixture; otherwise their exit paths would obscure the product result.

The fixture instantiates Vec<u64> with default, limit-only, budget-only and combined
policies. It exercises reserve, push, insertion/removal, resize, copying, internal
extension, split/append, retain, dedup, extraction, explicit splice completion,
drain, shrink, clone, boxed-slice conversion, checked rejection and owned iteration.
Box<usize> and Box<str> creation/cloning/drop run with default and budget policies.
Callbacks in this fixture are known implementations, including explicit refusal.

Successful linking excludes the tested direct panic/exit edges in that executable.
It does not prove termination, absence of undefined behavior, arbitrary indirect
calls, callback/Drop behavior, stack sufficiency, or allocator/libc internals.
Library types are generic: another type, callback, compiler or caller context is
a new instantiation to inspect. The C allocator and runtime remain explicit
external boundaries; their internals are not transformed by --wrap.

## Why checked/debug builds retain panic

Checked release retains 221 call sites to panic routines in the diagnostic
executable, including monomorphized duplicates and core forwarding routines:
102 addition checks, 81 subtraction checks, 35 multiplication checks and three
panic_fmt forwarding calls. These counts are static call sites, not executions
or distinct bugs. Debug has 318 recognized direct/GOT-resolved panic call sites,
including arithmetic, pointer/slice precondition and unwind-boundary checks.

Examples in product source include:

- Vec length advancement after successful reservation (`src/vec.rs:323`).
- Free-capacity subtraction and byte-layout multiplication (`src/vec.rs:237`, `:224`).
- Extraction's `end - read` and cursor advancement (`src/vec/edit.rs:367`, `:375`).
- Budget release subtraction/addition (`src/budget.rs:48`, `:49`).

These expressions rely on maintained ownership, capacity and budget invariants.
The compiler does not eliminate every corresponding check in these builds.
A retained branch is therefore not by itself a counterexample showing that a
valid API call can crash. Conversely, passing finite execution cases does not
prove those branches unreachable for all supported inputs. Existing source-linked
Kani/Verus and memory-safety checks remain complementary evidence, not a reason
to relabel this link failure as a pass. No checks are disabled to change the result.

After a failed strict link, a separately named diagnostic binary defines the panic
sink to print a fixed diagnostic and abort. The debug-only prebuilt-core unwind
personality reference is also connected to a diagnostic abort. Neither stub is
used to pass the strict gate. These binaries permit call-site inspection and fault
execution; the default script still returns nonzero for the retained edges.

## Reproduction

Requires Rust 1.95, Linux x86-64, a C compiler, GNU-compatible ELF linker, nm,
objdump and Python 3. The fixture has no external Rust package dependency.

```sh
python3 scripts/check_link_exits.py
# Select only the two configurations that passed the strict gate:
python3 scripts/check_link_exits.py --profiles release,release-lto
```

The default all-profile command is expected to return nonzero for checked/debug
retained edges. Do not count that nonzero outcome as a successful proof. The
runner records build/link commands, configuration, source/binary hashes, maps,
imports, diagnostics and runtime counts under ignored target/verification/link-audit.
The source hash and compact results are in [the Vec summary](vec/summary.json).
