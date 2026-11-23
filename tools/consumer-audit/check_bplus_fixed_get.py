#!/usr/bin/env python3
"""Prove the unchecked FixedTree lookup with actual Rust Kani and its defect controls.

The positive run checks every harness in the fixed proof module on the product
source. Each control copies the crate, applies one source defect and requires
the named harness to report a counterexample, so a harness that stops reaching
the unchecked indexing cannot pass silently.

The width-one harness fails under Kani 0.68.0 with CBMC 6.11.0 with a
counterexample that does not exist: the same class as the open
model-checking/kani#5104 (false counterexamples for pointer reads). It is not
counted as verified. The run must reproduce exactly the recorded failure, and
every concrete-playback replay of it must diverge from the native execution;
a different failure, a replay that reproduces natively, or a pass all stop
this check for review. The width-one shapes are covered by the direct Verus
proof (verify_direct_verus.py, target fixed-bplus) and by the exhaustive
native and Miri test fixed_bplus_width_one_chains_read_only_their_pools.
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
WIDTH_ONE = PROOFS + "fixed_get_reads_only_pool_values_for_any_order_width_one"
# The exact checks Kani reports as failed for the width-one harness.
KNOWN_FALSE = {
    "<usize as core::slice::SliceIndex<[([collections::btree_map::view::bplus::fixed::proofs::Chaos; 1],"
    " [u8; 1])]>>::get_unchecked.assume.1",
    "<usize as core::slice::SliceIndex<[([collections::btree_map::view::bplus::fixed::proofs::Chaos; 1],"
    " [usize; 1])]>>::get_unchecked.assume.1",
    "arithmetic::bounded::wrapping_sub.assertion.1",
}
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


def replay_width_one(out):
    """Replay every width-one counterexample natively; none may reproduce."""
    with tempfile.TemporaryDirectory(prefix="jzap-fixed-playback-") as directory:
        work = Path(directory)
        for name in ("src", "examples", "tests"):
            shutil.copytree(ROOT / name, work / name)
        for file in ("Cargo.toml", "Cargo.lock"):
            shutil.copyfile(ROOT / file, work / file)
        env = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
        generated = kani(work / "Cargo.toml", WIDTH_ONE, "--exact", "-Z", "concrete-playback",
                         "--concrete-playback=inplace", env=env, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True)
        (out / "width-one-playback-generate.log").write_text(generated.stdout)
        path = work / FIXED
        text = path.read_text()
        # Kani may emit the same test more than once; keep the first copy.
        seen = set()
        for match in list(re.finditer(r"\n#\[test\]\nfn (kani_concrete_playback_\w+)\(\) \{.*?\n\}\n",
                                      text, re.S)):
            if match.group(1) in seen:
                text = text.replace(match.group(0), "\n", 1)
            seen.add(match.group(1))
        if not seen:
            raise SystemExit("Kani produced no width-one concrete playback")
        text = text.replace("mod proofs {\n    use super::*;",
                            "mod proofs {\n    use super::*;\n    extern crate std;\n    use std::{vec, vec::Vec};", 1)
        path.write_text(text)
        replay = subprocess.run(["cargo", "kani", "playback", "-Z", "concrete-playback", "--lib",
                                 "--", "kani_concrete_playback"],
                                cwd=work, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True)
        (out / "width-one-playback.log").write_text(replay.stdout)
        outcomes = dict(re.findall(r"^test \S*::(kani_concrete_playback_\w+) \.\.\. (\w+)$",
                                   replay.stdout, re.M))
        if set(outcomes) != seen:
            raise SystemExit("width-one concrete playbacks did not all run")
        for name, outcome in outcomes.items():
            if outcome == "ok":
                continue
            message = re.search(r"thread '[^']*" + name + r"' \(\d+\) panicked at [^\n]*:\n([^\n]*)",
                                replay.stdout)
            if not message or message.group(1) != "Not enough det vals found":
                raise SystemExit(f"width-one counterexample {name} reproduces natively; review it")
        return len(outcomes)


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
    results = dict(re.findall(r"Checking harness (\S+)\.\.\.\n(?:.*\n)*?VERIFICATION:- (\w+)",
                              positive.stdout))
    if len(results) != HARNESSES:
        raise SystemExit(f"expected {HARNESSES} fixed B+ harnesses, found {len(results)}")
    if any(verdict != "SUCCESSFUL" for name, verdict in results.items() if name != WIDTH_ONE):
        raise SystemExit("fixed B+ Kani harnesses did not all verify")
    complete = (f"Complete - {HARNESSES - 1} successfully verified harnesses, 1 failures, "
                f"{HARNESSES} total.")
    section = positive.stdout.split(f"Checking harness {WIDTH_ONE}...", 1)[-1]
    section = section.split("Checking harness ", 1)[0]
    failed = set(re.findall(r"Check \d+: (.+)\n\s*- Status: FAILURE", section))
    if positive.returncode == 0 or complete not in positive.stdout or failed != KNOWN_FALSE:
        raise SystemExit(f"width-one harness changed from the recorded Kani false counterexample: {failed}")
    replayed = replay_width_one(out)
    print(f"Width-one harness: recorded Kani false counterexample (model-checking/kani#5104 class); "
          f"{replayed} concrete playbacks diverge natively; not counted as verified", flush=True)
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
