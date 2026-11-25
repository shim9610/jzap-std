#!/usr/bin/env bash
# Generate Kani's concrete playback test for one harness of a reproduction
# crate, in a copy of the crate, and run it natively.
#
#   bash tools/kani-repro/playback.sh <crate-dir> <harness>
#
# A passing playback test means the counterexample does not reproduce
# natively. The copy and logs go to
# target/verification/kani-repro/<crate>/playback-<harness>/; the crate in
# the repository is not modified.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
crate=$(cd "$1" && pwd)
harness=$2
name=$(basename "$crate")
work=$root/target/verification/kani-repro/$name/playback-${harness##*::}
rm -rf "$work"
mkdir -p "$work"
cp -r "$crate/Cargo.toml" "$crate/src" "$work/"
cd "$work"
export RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-1.95.0} CARGO_TARGET_DIR=$work/target
cargo kani --harness "$harness" --exact --solver kissat -Z concrete-playback \
  --concrete-playback=inplace -Z unstable-options --cbmc-args --object-bits 12 > generate.log 2>&1 || true
grep -E 'VERIFICATION:' generate.log
generated=$(grep -c 'fn kani_concrete_playback' src/lib.rs || true)
echo "generated playback tests: $generated"
[ "$generated" -gt 0 ] || exit 1
cargo kani playback -Z concrete-playback -- kani_concrete_playback > playback.log 2>&1 || true
grep -E '^test |test result|panicked' playback.log
