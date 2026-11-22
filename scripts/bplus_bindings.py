"""Bind the B+ proof domain to the reviewed lookup and snapshot implementations."""

from hashlib import sha256
from pathlib import Path

LOOKUP = "src/collections/btree_map/view/bplus.rs"
SOURCES = {
    LOOKUP: "b4f8ae4a2ad7544b030722a9d08fceea1ddc85f0e02de05eae0861c81519d43f",
    "src/collections/btree_map/view/bplus/fixed.rs":
        "66bb46b7391d09b17a6bea6c1c5dc97f4400ed2e302c6474087179dd2ca3b732",
    "src/collections/btree_map/view/bplus/fixed/range.rs":
        "864e821668c07dedbaf50300f465f43ce8295280bc4f8ac95dd34e8429afd095",
    "src/collections/btree_map/view/bplus/strided.rs":
        "fc3e32d3769059b2630df7774a7432aacc026383f71b33e9996f69c4c13920c2",
    "src/collections/btree_map/view/bplus/strided/tree.rs":
        "1cd8cb37eeffa7d0d36c66f79a476f3a37e65501f622f877dd2f354e06a3779e",
}


def is_bplus(file):
    return file in SOURCES


def check(root):
    root = Path(root)
    for file, expected in SOURCES.items():
        assert sha256((root / file).read_bytes()).hexdigest() == expected, (
            "unbound B+ source", file)
