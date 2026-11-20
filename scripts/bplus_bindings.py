"""Bind the B+ proof domain to the reviewed lookup and snapshot implementations."""

from hashlib import sha256
from pathlib import Path

LOOKUP = "src/collections/btree_map/view/bplus.rs"
SOURCES = {
    LOOKUP: "b4f8ae4a2ad7544b030722a9d08fceea1ddc85f0e02de05eae0861c81519d43f",
    "src/collections/btree_map/view/bplus/fixed.rs":
        "60d0dc345a3b83eda212d7310d6415db914c7707271f4c496ce0b77071222526",
    "src/collections/btree_map/view/bplus/fixed/range.rs":
        "0d32761a073acc10f3033facddd9410a3934e95b64315e5b5e102f4f9018d296",
    "src/collections/btree_map/view/bplus/strided.rs":
        "26e62ab3a0ec03daad6bc475c5d749fa9952451dcbbc615c19c10525246a48e5",
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
