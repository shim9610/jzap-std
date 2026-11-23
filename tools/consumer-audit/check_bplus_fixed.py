#!/usr/bin/env python3
"""Bind the fixed B+ composition lemmas to the reviewed Rust source."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess

from check_bplus_prefix import HERE, ROOT, item

FIXED = ROOT / "src/collections/btree_map/view/bplus/fixed.rs"
# The child lookup reads the completed prefix, the flag claims a single
# parent, and the checked height and separators describe the new record.
# summarize holds the unique-root check over the earlier records. The lookup's
# memory safety is verified directly on the source (verify_direct_verus.py,
# target fixed-bplus); these lemmas cover the remaining unique-parent and
# range reasoning.
BOUND = {
    "pub struct FixedTree<": "5fd18d87f241a7b8dc83c0c3156fcddd14edd7c294276d48d56fc8b66d37b8a7",
    "struct Summary<": "0e770ad9e97dd75f4f5bc7096603c6218760988620abc5a9fa05c92e275f1153",
    "pub fn try_new(": "3f10399b54818fd18de2f8d1c66855c159eac18641673f2744fa349bf8dd5c82",
    "pub fn get<": "673f603f210413767d72861e39d6d1f6fe764f15765e725eb4b19c1d31d67f9f",
    "fn summarize<": "bae443874a72ad986f9382728b274215e26cd94472fd13923ee8e08a210e3053",
    "fn summarize_branch<": "4f529519675b1e16e928b9617b89f09ecfa1cf9a9c5c775ad5a07971717a521a",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=FIXED)
    args = parser.parse_args()
    text = args.source.read_text()
    for name, expected in BOUND.items():
        if hashlib.sha256(item(text, name).encode()).hexdigest() != expected:
            raise SystemExit(f"unbound fixed B+ item: {name}")
    subprocess.run(
        [os.environ.get("JZAP_VERUS", "verus"), str(HERE / "bplus_fixed.rs"), "--no-cheating"],
        check=True,
    )


if __name__ == "__main__":
    main()
