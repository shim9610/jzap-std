#!/usr/bin/env python3
"""Check the reviewed constructor-to-Vec contracts and run the prefix lemmas."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
POOL = ROOT / "tools/throughput-analysis/src/bplus_pool.rs"
# Path and content digest of the 29 reviewed files. Renewed after the src/vec/edit.rs
# retain closure and dedup routing change, which does not touch constructors.
VEC_SOURCE_SHA256 = "99ac117b2001e2cf99867745aa17c271a106624d6c52beb2ed49b302df14eb92"
VEC_SOURCES = (
    "src/vec.rs", "src/vec", "src/buffer.rs", "src/buffer",
    "src/arithmetic", "src/layout", "src/error.rs",
    "src/storage.rs", "src/storage", "src/capacity.rs", "src/capacity", "src/budget",
)

# These exact bodies instantiate the sequence transition proved in bplus_prefix.rs.
# The checked-input step only reads prefix.get. The sole publication is try_push
# after the step succeeds. A failed append exits before any root can be returned.
BOUND = {
    "pub(super) struct Page<'a>": "08f82286724224a8f1cba339b307beacfbeca61b4ceffae185c83985cbb45c7a",
    "impl Page<'_>": "292ecabc40028db9971b002cdb0572eb9df7b0b4c6ec09ea9ce9ac0b48955324",
    "fn leaf_index<K:": "d9a1344bec59bb130a1edbe2ea46dc9d3ca757bfc41a8c9e657f67921ba06795",
    "fn array<T,": "7c07b959a788be5067894d917085ef5511253458298049e2b9571b62f04c5401",
    "pub(super) struct Resident<'a>": "58d61d1426ddf4ed01b2ec73e575977401a201ae67e01870e0ba72b11682cbd9",
    "struct ResidentNode<'a>": "596dde9c3cdff731fb13316b7228adf31284670be239c26255f1da9cbe35aaf1",
    "enum ResidentContents<'a>": "9a89ced08bfcbe0cb843f142b4db890c578bace0ab5ca8ce2e05da8663e47efb",
    "fn summarize_branch(": "dc35297a4d38b96ef44a85cd08c7e1cc10dc6f849939785364be7b6336b5f99c",
    "fn validate_next<'a>(": "f8d13d4f5ba16a84abc474a91a883f14ac8b8a09e29ce43a61b4ffc0a39dac2b",
    "pub(super) fn check(pages:": "ff1bba918295a1077f58b901bbe96632f02724c5d81b5416465209641b9594fe",
    "pub(super) fn split<const": "f8985728a22dffccdb135d0e188d947f1be4c5dc5c5723e06b27279766732b07",
    "pub(super) fn pack_pools<const": "2aa587ae5c296e28c97ec07e5398e8b1ad12ed44f2b90e613a61b287f043b073",
}


def item(text, marker):
    if text.count(marker) != 1:
        raise ValueError(f"ambiguous Rust item: {marker}")
    start = text.index(marker)
    body = text.index("{", start)
    depth = 0
    for end in range(body, len(text)):
        if text[end] == "{":
            depth += 1
        elif text[end] == "}":
            depth -= 1
            if depth == 0:
                return text[start:end + 1]
    raise ValueError(f"unclosed Rust item: {marker}")


def vec_source_digest(root):
    files = set()
    for name in VEC_SOURCES:
        path = root / name
        if path.is_file():
            files.add(path)
        elif path.is_dir():
            files.update(p for p in path.rglob("*") if p.is_file())
    digest = hashlib.sha256()
    for name in sorted(path.relative_to(root).as_posix() for path in files):
        digest.update(name.encode() + b"\0")
        digest.update(hashlib.sha256((root / name).read_bytes()).digest())
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=POOL)
    args = parser.parse_args()
    text = args.source.read_text()
    for name, expected in BOUND.items():
        if hashlib.sha256(item(text, name).encode()).hexdigest() != expected:
            raise SystemExit(f"unbound B+ constructor item: {name}")
    if vec_source_digest(ROOT) != VEC_SOURCE_SHA256:
        raise SystemExit("Vec constructor dependencies differ from the reviewed prefix proof baseline")
    subprocess.run(
        [os.environ.get("JZAP_VERUS", "verus"), str(HERE / "bplus_prefix.rs"), "--no-cheating"],
        check=True,
    )


if __name__ == "__main__":
    main()
