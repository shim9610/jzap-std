#!/usr/bin/env bash
# Run member_array.c with CBMC (default and --no-simplify), and native.c with
# the C compiler. Uses the CBMC of the Kani 0.68.0 installation unless `cbmc`
# is already on PATH. Output goes to target/verification/kani-repro/cbmc-c/.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
out=$root/target/verification/kani-repro/cbmc-c
mkdir -p "$out"
command -v cbmc > /dev/null || export PATH=$HOME/.kani/kani-0.68.0/bin:$PATH
cbmc --version
for flags in "" "--no-simplify"; do
  echo "== cbmc $flags member_array.c"
  # shellcheck disable=SC2086
  cbmc $flags "$here/member_array.c" > "$out/cbmc${flags// /}.log" 2>&1 || true
  grep -E '^\[main\.assertion|^VERIFICATION' "$out/cbmc${flags// /}.log"
done
cc -O0 -o "$out/native" "$here/native.c"
"$out/native"
