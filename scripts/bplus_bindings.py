"""Bind the B+ proof domain to the reviewed lookup and snapshot implementations."""

from hashlib import sha256
from pathlib import Path

LOOKUP = "src/collections/btree_map/view/bplus.rs"
SOURCES = {
    LOOKUP: "eae8c883f59fbec168f92ef6a6d52da308d4b7bdc3c8d02a8ecad5324d014f6f",
    "src/collections/btree_map/view/bplus/fixed.rs":
        "60d0dc345a3b83eda212d7310d6415db914c7707271f4c496ce0b77071222526",
    "src/collections/btree_map/view/bplus/fixed/range.rs":
        "f1c908b1e7b5e5806d80db83e793d926f98c631e6e4db420a572fb177444f26a",
}


def is_bplus(file):
    return file in SOURCES


def check(root):
    root = Path(root)
    for file, expected in SOURCES.items():
        assert sha256((root / file).read_bytes()).hexdigest() == expected, (
            "unbound B+ source", file)
