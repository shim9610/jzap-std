#!/usr/bin/env python3
"""Bind the fixed B+ composition lemmas to the reviewed Rust source."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tempfile

from check_bplus_prefix import HERE, ROOT, item

FIXED = ROOT / "src/collections/btree_map/view/bplus/fixed.rs"
# The child lookup reads the completed prefix, the flag claims a single
# parent, and the checked height and separators describe the new record.
# get descends height(root) levels with the unchecked branch, child and leaf
# reads that every_descent_ends_at_a_leaf and floor_slot_step justify.
BOUND = {
    "pub struct FixedTree<": "5fd18d87f241a7b8dc83c0c3156fcddd14edd7c294276d48d56fc8b66d37b8a7",
    "struct Summary<": "0e770ad9e97dd75f4f5bc7096603c6218760988620abc5a9fa05c92e275f1153",
    "pub fn try_new(": "74ca71d91e5c1c3d366b81b7fcbea9ee2f3ef98451f6ed8be4555ebfebd472f2",
    "pub fn get<": "02cc6b9fc11a1e0ea961042f76457c9901a389c71cb7ec38c4d5149fbdbde57c",
    "unsafe fn branch_unchecked(": "a48c3a4d4338b097eb372a58159318e683d3023bc2a268020e7551b4ba9384f7",
    "fn floor_slot<": "b8e8c5ef7fd293984589634d82a97a690a9a9acdc93a4a706e0b26a9c3a16cf3",
    "fn summarize_branch<": "77c261f16f9e0620e8885d0914a3bc923a631952ba80e1f94bf1fd5b4cebf239",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=FIXED)
    args = parser.parse_args()
    text = args.source.read_text()
    for name, expected in BOUND.items():
        if hashlib.sha256(item(text, name).encode()).hexdigest() != expected:
            raise SystemExit(f"unbound fixed B+ item: {name}")
    verus = os.environ.get("JZAP_VERUS", "verus")
    proof = HERE / "bplus_fixed.rs"
    subprocess.run([verus, str(proof), "--no-cheating"], check=True)

    # Letting a child sit at any lower height, or a floor step past the
    # width, must leave a solver error in the unchecked-descent lemmas.
    text = proof.read_text()
    for name, old, new in (
        ("child-below-parent-height", "&&& heights[children[j][s]] == heights[leaves + j] - 1",
         "&&& heights[children[j][s]] <= heights[leaves + j] - 1"),
        ("floor-past-width", "requires 0 < width, 0 <= base, 1 < remaining, base + remaining <= width,",
         "requires 0 < width, 0 <= base, 1 < remaining, base + remaining <= width + 1,"),
    ):
        if text.count(old) != 1:
            raise SystemExit(f"fixed B+ Verus control {name} no longer matches the proof")
        with tempfile.TemporaryDirectory(prefix="jzap-bplus-fixed-") as directory:
            mutant = Path(directory) / f"{name}.rs"
            mutant.write_text(text.replace(old, new))
            result = subprocess.run([verus, str(mutant), "--no-cheating"], capture_output=True, text=True)
            if result.returncode == 0 or not re.search(
                r"verification results:: \d+ verified, [1-9]\d* errors", result.stdout + result.stderr
            ):
                raise SystemExit(f"fixed B+ Verus control {name} did not reach a solver error")
        print(f"Fixed B+ Verus control {name} rejected", flush=True)


if __name__ == "__main__":
    main()
