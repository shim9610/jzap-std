"""Bind the B+ proof domain to the reviewed lookup and snapshot implementations."""

from hashlib import sha256
from pathlib import Path

LOOKUP = "src/collections/btree_map/view/bplus.rs"
SOURCES = {
    LOOKUP: "aab3bd4cd8e77e57df7113a659e7a8d63268eda323ad38d5d0a23d5a90b74c72",
    "src/collections/btree_map/view/bplus/fixed.rs":
        "66bb46b7391d09b17a6bea6c1c5dc97f4400ed2e302c6474087179dd2ca3b732",
    "src/collections/btree_map/view/bplus/fixed/range.rs":
        "864e821668c07dedbaf50300f465f43ce8295280bc4f8ac95dd34e8429afd095",
}


def is_bplus(file):
    return file in SOURCES


def check(root):
    root = Path(root)
    for file, expected in SOURCES.items():
        assert sha256((root / file).read_bytes()).hexdigest() == expected, (
            "unbound B+ source", file)
