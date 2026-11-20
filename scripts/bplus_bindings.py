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
        "e94dd8e0adb04fbc7ad7981b7b7fba1b94388838753487e529b1dd611a267e8e",
    "src/collections/btree_map/view/bplus/strided/tree.rs":
        "cd6bbbe8e380bbce9d5a759a14dda23fdbf3d45c37b82fe1e0bea64f09ba6467",
}


def is_bplus(file):
    return file in SOURCES


def check(root):
    root = Path(root)
    for file, expected in SOURCES.items():
        assert sha256((root / file).read_bytes()).hexdigest() == expected, (
            "unbound B+ source", file)
