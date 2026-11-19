# Vec LeakSanitizer follow-up

The current runtime at `656c330` passes all 41 Vec scenarios with both
AddressSanitizer and LeakSanitizer enabled. This is a fresh run on the unchanged
product source, following the [inlining optimization](vec-extract-inline.md).
The process-exit leak scan is confirmed by verbose runtime output, not inferred
only from the test exit code.

| Check | Result |
| --- | --- |
| ASan + LeakSanitizer | 41 pass, zero failed/ignored/filtered, exit 0; no leak reported |
| Explicit scan evidence | `LeakSanitizer: checking for leaks` followed by processing the test process's thread |
| Normal jzap Vec cleanup control | Exit 0 with leak detection enabled |
| Intentionally forgotten jzap Vec control | Exit 23, exactly 257 bytes leaked in one allocation |
| Standard Vec controls | Normal cleanup exits 0; intentionally forgotten allocation reports 257 bytes and exits 23 |
| Product source | Unchanged throughout the checks; no runtime fix was needed |

The existing Vec scenarios include extraction error/mutation/order, long
retained prefixes, early iterator drop, partial initialization and allocation
refusal, unwinding cleanup, splice/drain cleanup, nested ownership and explicit
reclamation of the public leak API. These are executed scenarios, not a proof
that every possible program using Vec is leak-free. LeakSanitizer reports
unreachable allocations; reachable process-lifetime storage has a different
scope. No custom suppressions or stack, register or thread-local-storage scan
exclusions were used.

## Environment diagnosis

Inside the restricted execution-tool sandbox, even the normally freed control
fails with `LeakSanitizer has encountered a fatal error` and its ptrace hint.
The deliberately leaking control fails the same way. Those are unsupported
checks, not successful leak detection. The earlier reports' statement that
LeakSanitizer was unavailable in the cloud was too broad: the restriction was
the execution-tool sandbox.

Running the controls and the common verification entry point outside that
sandbox succeeds. `ASAN_OPTIONS=detect_leaks=1` and
`LSAN_OPTIONS=exitcode=23` remain enabled. No kernel ptrace/Yama setting,
container security setting or system configuration was changed. The local
execution permission applied to the diagnostic/test processes themselves.

The first Vec suite run passes quietly. A second run of the same 41 scenarios
with `verbosity=1:log_threads=1` confirms the actual exit-time leak scan. These
are repeated runs of 41 scenarios, not 82 distinct tests. The original ASan
receipt with `detect_leaks=0` retains its original scope.

## Reproduction

Run these commands in a context that permits LeakSanitizer to inspect its own
process; through the managed execution tool this required leaving its restricted
sandbox. Rust is `nightly-2026-10-30` and the executed target is Linux x86-64.

```sh
. /workspace/.devtools/activate.sh
XDG_CACHE_HOME=/workspace/.cache ASAN_OPTIONS=detect_leaks=1 \
  LSAN_OPTIONS=exitcode=23:verbosity=1:log_threads=1 \
  bash scripts/verify.sh --area vec --test-target vec --check asan \
  --output target/verification/vec-leak-replay
```

The maintained [detector control](../../tools/leak-check/control.rs) uses a real
jzap Vec with one 257-byte allocation. Normal invocation drops it; argument
`leak` intentionally forgets it. The control makes the allocation observable
with `black_box`, returns from the allocation helper and overwrites a stack
buffer before exit. Root scanning stays enabled.

```sh
mkdir -p target/verification/leak-controls-replay
rustc +nightly-2026-10-30 --edition=2024 -Zsanitizer=address -g -Copt-level=1 \
  --crate-type=rlib --crate-name=jzap --cfg 'feature="alloc"' src/lib.rs \
  -o target/verification/leak-controls-replay/libjzap.rlib
rustc +nightly-2026-10-30 --edition=2024 -Zsanitizer=address -g -Copt-level=1 \
  --extern jzap=target/verification/leak-controls-replay/libjzap.rlib \
  tools/leak-check/control.rs -o target/verification/leak-controls-replay/control
ASAN_OPTIONS=detect_leaks=1 LSAN_OPTIONS=exitcode=23 \
  target/verification/leak-controls-replay/control
# Expected failure: LeakSanitizer must report 257 bytes / one allocation, exit 23.
ASAN_OPTIONS=detect_leaks=1 LSAN_OPTIONS=exitcode=23 \
  target/verification/leak-controls-replay/control leak
```

Both exit status and the leak diagnostic are checked: the sandbox's fatal error
can also return 23 and must not be mistaken for detection. Raw logs, commands,
source hashes and common-runner receipts are ignored under
`target/verification/vec-leak-followup/`. A preliminary no-run build without
sanitizer flags is not counted as a sanitizer check; the passing suite uses the
runner's `-Zsanitizer=address` build.
