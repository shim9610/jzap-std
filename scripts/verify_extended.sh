#!/usr/bin/env bash
set -u
cd "$(dirname "$0")/.."
logs=target/verification
mkdir -p "$logs"
status=0
run_check() {
    name="$1"
    shift
    "$@" > "$logs/$name.log" 2>&1
    code=$?
    printf '%s %s\n' "$name" "$code" >> "$logs/extended-status.txt"
    if [ "$code" -ne 0 ]; then status=1; fi
}
: > "$logs/extended-status.txt"
run_check mutation-run python3 scripts/mutation_check.py
run_check kani-validity cargo kani --no-default-features -Z valid-value-checks
run_check kani-memory-extra python3 scripts/verify_memory.py
run_check direct-foundation python3 scripts/verify_source.py --mutations
run_check verus python3 scripts/verify_direct_verus.py --mutations
run_check kani-reproductions python3 scripts/reproduce_kani_limits.py
run_check asan env RUSTFLAGS=-Zsanitizer=address cargo +nightly-2026-10-30 test --locked --all-features --target x86_64-unknown-linux-gnu --test foundation --test box_complete --test vec
run_check no-oom-build env RUSTFLAGS=--cfg\ no_global_oom_handling cargo +nightly-2026-10-30 -Z build-std=core,alloc check --locked --lib --target x86_64-unknown-linux-gnu
exit "$status"
