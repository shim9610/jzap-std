"""Bind the B+ proof domain to the reviewed lookup and snapshot implementations."""

from hashlib import sha256
from pathlib import Path

LOOKUP = "src/collections/btree_map/view/bplus.rs"
SOURCES = {
    LOOKUP: "1cd5b9801d17c69cd0c19e98ea8eaa32cbfe267b8aecd3c7932e633a51b32ae9",
    "src/collections/btree_map/view/bplus/partial.rs":
        "3843f479cd79223d450dcaabde3f76dbb7eb0704c43939e3eaa51f415d617d0e",
    "src/collections/btree_map/view/bplus/fixed.rs":
        "813c11a4f561ceddfaa839cadcdb55bf90b756e69fe9913d2b65b6ce6ad29250",
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
