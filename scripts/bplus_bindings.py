"""Bind the B+ proof domain to the reviewed lookup and snapshot implementations."""

from hashlib import sha256
from pathlib import Path

LOOKUP = "src/collections/btree_map/view/bplus.rs"
SOURCES = {
    LOOKUP: "83915dd7b53ceb7d3590392e6b3138fbe1833155ec9f8c18374a6ed138d9eb59",
    "src/collections/btree_map/view/bplus/fixed.rs":
        "c24e576cf57ecf1bb8be8bc801d95235bc1aee037f2e9707d4c330c4866da3d2",
}


def is_bplus(file):
    return file in SOURCES


def check(root):
    root = Path(root)
    for file, expected in SOURCES.items():
        assert sha256((root / file).read_bytes()).hexdigest() == expected, (
            "unbound B+ source", file)
