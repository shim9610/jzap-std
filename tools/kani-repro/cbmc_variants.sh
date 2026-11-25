#!/usr/bin/env bash
# Build one Kani harness of a reproduction crate to a goto binary, then run
# CBMC on it directly with option variants, one at a time.
#
#   bash tools/kani-repro/cbmc_variants.sh <crate-dir> <harness> [variant ...]
#
# A variant is a quoted string of extra CBMC options; the default set is
#   "--external-sat-solver kissat" "--sat-solver minisat2"
#   "--sat-solver cadical" "--no-simplify --external-sat-solver kissat"
#   "--object-bits 8" "--object-bits 16"
# Every run uses the CBMC command line Kani 0.68.0 itself runs for the
# harness (taken from `cargo kani --verbose`, which includes the harness's
# `--unwind`), without its solver, object bits and JSON output options; each
# variant then adds its own, with `--object-bits 12` unless it sets them.
# Each CBMC run is limited to CBMC_TIMEOUT seconds (default 1800); a run cut
# off by the limit, or one that ran out of memory, prints "unfinished".
# Raw CBMC VERIFICATION lines are not used: Kani's reachability checks are
# `assert(false)` and always fail; the summary counts the other properties.
# Output goes to target/verification/kani-repro/<crate>/cbmc-<harness>/.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
crate=$(cd "$1" && pwd)
harness=$2
shift 2
name=$(basename "$crate")
short=${harness##*::}
out=$root/target/verification/kani-repro/$name/cbmc-$short
mkdir -p "$out"
export RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-1.95.0}
export CARGO_TARGET_DIR=$root/target/verification/kani-repro/$name
kani_bin=$(dirname "$(command -v kani-driver 2>/dev/null || ls -d "$HOME"/.kani/kani-0.68.0/bin/kani-driver)")
export PATH=$kani_bin:$PATH

# Run Kani's pipeline once with --keep-temps: the kept binary is the goto
# program after Kani's goto-cc and goto-instrument steps, as CBMC receives it.
(cd "$crate" && cargo kani --harness "$harness" --exact --keep-temps --verbose \
  --solver kissat -Z unstable-options --cbmc-args --object-bits 12) > "$out/kani.log" 2>&1 || true
command=$(grep -o '\[Kani\] Running: `cbmc [^`]*`' "$out/kani.log" | tail -1 | sed 's/^\[Kani\] Running: `//; s/`$//')
if [ -z "$command" ]; then
  echo "no CBMC command found in $out/kani.log" >&2
  exit 1
fi
read -r -a words <<< "$command"
base=()
goto=
skip=0
for word in "${words[@]:1}"; do
  if [ "$skip" -gt 0 ]; then skip=$((skip - 1)); continue; fi
  case "$word" in
    --external-sat-solver|--object-bits|--verbosity) skip=1 ;;
    --json-ui) ;;
    *.out) goto=$word ;;
    *) base+=("$word") ;;
  esac
done
cp "$goto" "$out/harness.goto"
printf '%s\n' "${base[@]}" > "$out/base-options.txt"
goto-instrument --show-goto-functions "$out/harness.goto" > "$out/goto-functions.txt" 2>/dev/null || true

if [ $# -eq 0 ]; then
  set -- "--external-sat-solver kissat" "--sat-solver minisat2" "--sat-solver cadical" \
    "--no-simplify --external-sat-solver kissat" "--object-bits 8" "--object-bits 16"
fi
for variant in "$@"; do
  # shellcheck disable=SC2206
  extra=($variant)
  bits=(--object-bits 12)
  case " $variant " in *" --object-bits "*) bits=() ;; esac
  tag=$(echo "$variant" | tr -c 'a-z0-9' '_')
  start=$(date +%s)
  status=0
  timeout "${CBMC_TIMEOUT:-1800}" cbmc "${base[@]}" "${bits[@]}" "${extra[@]}" "$out/harness.goto" > "$out/cbmc-$tag.log" 2>&1 || status=$?
  end=$(date +%s)
  # Kani encodes its reachability checks as `assert(false)`: their FAILURE
  # means "reachable", so they are not counted as failed properties.
  failures=$(grep -E ': FAILURE$' "$out/cbmc-$tag.log" | grep -vc 'reachability_check' || true)
  failed=$(grep -E ': FAILURE$' "$out/cbmc-$tag.log" | grep -v 'reachability_check' \
    | sed 's/^\[[^]]*\] line [0-9]* \(\[[^]]*\] \)\{0,1\}//; s/: FAILURE$//' | tr '\n' ';' || true)
  if grep -qE '^VERIFICATION (SUCCESSFUL|FAILED)' "$out/cbmc-$tag.log"; then done_=finished; else done_=unfinished; fi
  printf '%-44s exit=%-3s %-10s %5ss  failed properties: %s %s\n' "$variant" "$status" "$done_" \
    "$((end - start))" "$failures" "${failed:+($failed)}"
done | tee "$out/summary.txt"
