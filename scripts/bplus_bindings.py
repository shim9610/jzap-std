"""Bind the B+ proof domain to the reviewed lookup and snapshot implementations."""

from hashlib import sha256
from pathlib import Path

LOOKUP = "src/collections/btree_map/view/bplus.rs"
SOURCES = {
    LOOKUP: "1310346bf04380428f59b9673240041bb31c28c5f2c7adc3d6c5a6cba381c279",
    "src/collections/btree_map/view/bplus/fixed.rs":
        "60d0dc345a3b83eda212d7310d6415db914c7707271f4c496ce0b77071222526",
    "src/collections/btree_map/view/bplus/fixed/range.rs":
        "0d32761a073acc10f3033facddd9410a3934e95b64315e5b5e102f4f9018d296",
    "src/collections/btree_map/view/bplus/strided.rs":
        "c19e288d1e34b373ce6f0b243c809b9718ab894cd4bd3275de5c94f4ef214b1d",
}


def is_bplus(file):
    return file in SOURCES


def check(root):
    root = Path(root)
    for file, expected in SOURCES.items():
        assert sha256((root / file).read_bytes()).hexdigest() == expected, (
            "unbound B+ source", file)
