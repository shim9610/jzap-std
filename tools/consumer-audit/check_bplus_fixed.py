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
BOUND = {
    "pub struct FixedTree<": "5fd18d87f241a7b8dc83c0c3156fcddd14edd7c294276d48d56fc8b66d37b8a7",
    "struct Summary<": "0e770ad9e97dd75f4f5bc7096603c6218760988620abc5a9fa05c92e275f1153",
    "pub fn try_new(": "74ca71d91e5c1c3d366b81b7fcbea9ee2f3ef98451f6ed8be4555ebfebd472f2",
    "pub fn get<": "12ddf8eb35319e2ab59ebeea40b8d8ef229e5e2942d84a2f204d6394690f15c1",
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
    subprocess.run(
        [os.environ.get("JZAP_VERUS", "verus"), str(HERE / "bplus_fixed.rs"), "--no-cheating"],
        check=True,
    )


if __name__ == "__main__":
    main()
