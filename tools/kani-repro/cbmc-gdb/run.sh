#!/usr/bin/env bash
# Trace the wrong dereference in a debug build of CBMC 6.11.0, and run the
# causal check (skip.gdb) on V1 and V2, with and without --no-simplify.
#
#   bash tools/kani-repro/cbmc-gdb/run.sh
#
# The debug build is made once, outside the repository:
#   git clone --depth 1 --branch cbmc-6.11.0 https://github.com/diffblue/cbmc.git \
#     target/verification/upstream/cbmc-6.11.0
#   cmake -S target/verification/upstream/cbmc-6.11.0 \
#     -B target/verification/upstream/cbmc-6.11.0-debug -G Ninja \
#     -DCMAKE_BUILD_TYPE=Debug -DWITH_JBMC=OFF
#   ninja -C target/verification/upstream/cbmc-6.11.0-debug cbmc
# CBMC_DEBUG overrides the binary path. Needs gdb. Logs go to
# target/verification/kani-repro/cbmc-gdb/.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
cbmc=${CBMC_DEBUG:-$root/target/verification/upstream/cbmc-6.11.0-debug/bin/cbmc}
out=$root/target/verification/kani-repro/cbmc-gdb
mkdir -p "$out"
"$cbmc" --version
for program in v1 v2; do
  for flags in "" "--no-simplify"; do
    tag=$program${flags// /}
    # shellcheck disable=SC2086
    gdb -q -batch -x "$here/trace.gdb" --args "$cbmc" $flags "$here/$program.c" > "$out/$tag-trace.log" 2>&1 || true
    # shellcheck disable=SC2086
    gdb -q -batch -x "$here/skip.gdb" --args "$cbmc" $flags "$here/$program.c" > "$out/$tag-skip.log" 2>&1 || true
    echo "== $program $flags"
    echo "  C+x recursions: $(grep -c '^== C+x recursion,' "$out/$tag-trace.log" || true)"
    grep -E '^recurses into|^#[0-9]+ .* in [a-z_:]+(simplify_byte_extract|build_reference_to)' "$out/$tag-trace.log" \
      | sed 's/ (.*) at .*\/src\// at src\//; s/^/  /' | sort -u || true
    echo "  unchanged CBMC: $(grep -E '^\[main\.assertion\.1\]' "$out/$tag-trace.log" | sed 's/.*: //')"
    echo "  recursion skipped: $(grep -E '^\[main\.assertion\.1\]' "$out/$tag-skip.log" | sed 's/.*: //')"
  done
done
