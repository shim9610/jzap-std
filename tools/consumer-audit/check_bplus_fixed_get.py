#!/usr/bin/env python3
"""Prove the unchecked FixedTree lookup with actual Rust Kani and its defect controls.

The positive run verifies every harness in the fixed proof module on the product
source. Each control copies the crate, applies one source defect and requires
the named harness to report a counterexample, so a harness that stops reaching
the unchecked indexing cannot pass silently.
"""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from bplus_bindings import check as check_bplus

PROOFS = "collections::btree_map::view::bplus::fixed::proofs::"
HARNESSES = 11
COVERS = 10
FIXED = "src/collections/btree_map/view/bplus/fixed.rs"
WIDTH_TWO = PROOFS + "fixed_get_reads_only_pool_values_for_any_order_width_two"
FLOOR = PROOFS + "floor_slot_stays_below_every_width_for_any_order"
LEAF = PROOFS + "fixed_leaf_search_reads_only_pool_values_for_any_order"
STABLE = PROOFS + "fixed_get_matches_checked_and_linear_lookup_at_height_one"
ROUTE = "base = select_unpredictable(admits(key), middle, base);"
CONTROLS = (
    # Validation accepts children of different heights, so a descent can
    # treat a leaf ID as a branch.
    ("mixed-child-heights", "Some(expected) => expected != child.height,",
     "Some(expected) => false,", WIDTH_TWO),
    # The descent runs one level past the validated height.
    ("height-past-root", "let mut remaining = self.height;",
     "let mut remaining = self.height.wrapping_add(1);", WIDTH_TWO),
    # The routed slot can reach the width.
    ("slot-past-floor", ROUTE,
     "base = select_unpredictable(admits(key), middle.wrapping_add(1), base);", FLOOR),
    # The value index can pass the last key of a leaf.
    ("value-past-match", "                Some(base)\n",
     "                Some(base.wrapping_add(1))\n", LEAF),
    # A strict separator comparison routes equal queries to the wrong child.
    ("strict-routing", "&|key: &K| key.borrow() <= query,",
     "&|key: &K| key.borrow() < query,", STABLE),
)


def kani(manifest, harness, *extra, env=None, **kwargs):
    command = ["cargo", "kani", "--manifest-path", str(manifest), "--lib", "--harness", harness,
               *extra]
    return subprocess.run(command, cwd=ROOT, env=env, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/verification/fixed-bplus-get")
    args = parser.parse_args()
    check_bplus(ROOT)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)

    # The positive log goes to the caller, which counts the verified harnesses.
    positive = kani(ROOT / "Cargo.toml", PROOFS, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                    text=True)
    sys.stdout.write(positive.stdout)
    sys.stdout.flush()
    complete = f"Complete - {HARNESSES} successfully verified harnesses, 0 failures, {HARNESSES} total."
    if positive.returncode != 0 or complete not in positive.stdout:
        raise SystemExit("fixed B+ Kani harnesses did not all verify")
    # Every shape instance must reach its expected height.
    covers = re.findall(r"Status: (\w+)\n\s*- Description: \"cover condition", positive.stdout)
    if covers != ["SATISFIED"] * COVERS:
        raise SystemExit(f"fixed B+ cover properties not all satisfied: {covers}")

    for name, old, new, harness in CONTROLS:
        with tempfile.TemporaryDirectory(prefix="jzap-fixed-get-") as directory:
            work = Path(directory)
            shutil.copytree(ROOT / "src", work / "src")
            shutil.copytree(ROOT / "examples", work / "examples")
            for file in ("Cargo.toml", "Cargo.lock"):
                shutil.copyfile(ROOT / file, work / file)
            path = work / FIXED
            text = path.read_text()
            if text.count(old) != 1:
                raise SystemExit(f"control {name} no longer matches the source")
            path.write_text(text.replace(old, new))
            env = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
            result = kani(work / "Cargo.toml", harness, "--exact", env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            (out / f"{name}.log").write_text(result.stdout)
            if result.returncode == 0 or "VERIFICATION:- FAILED" not in result.stdout or (
                "error[E" in result.stdout
            ):
                raise SystemExit(f"control {name} was not rejected by {harness}")
            print(f"Control {name} rejected by {harness.removeprefix(PROOFS)}", flush=True)
    print(f"{len(CONTROLS)} fixed B+ lookup defect controls rejected", flush=True)


if __name__ == "__main__":
    main()
