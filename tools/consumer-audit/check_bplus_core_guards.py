#!/usr/bin/env python3
"""Verify the index lemmas for the pinned core slice implementation."""
import hashlib
import os
from pathlib import Path
import subprocess

PINNED_SLICE = "decea260b5d73957cd573e827257d8c00d6614fb1cb10f8001d54e3c264de08d"
PINNED_ITER = "c309e04ca3da2fc4220ee3f93b4466c578ea7f3f2ef52cca6a3fdf8ac28e53d3"
PROOF = Path(__file__).resolve().with_name("bplus_core_guards.rs")


def main():
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    source = sysroot / "lib/rustlib/src/rust/library/core/src/slice/mod.rs"
    data = source.read_bytes()
    if hashlib.sha256(data).hexdigest() != PINNED_SLICE:
        raise SystemExit("core slice source differs from the verified Rust 1.95.0 implementation")

    # Bind the proven transition and endpoint predicates to the reviewed source.
    text = data.decode()
    binary = text.split("pub fn binary_search_by", 1)[1].split("/// Binary searches", 1)[0]
    split = text.split("pub const fn split_at_checked", 1)[1].split("/// Divides", 1)[0]
    for fragment in (
        "let mut size = self.len();",
        "let mut base = 0usize;",
        "while size > 1",
        "let half = size / 2;",
        "let mid = base + half;",
        "self.get_unchecked(mid)",
        "base = hint::select_unpredictable(cmp == Greater, base, mid);",
        "size -= half;",
        "self.get_unchecked(base)",
        "hint::assert_unchecked(base < self.len())",
        "let result = base + (cmp == Less) as usize;",
        "hint::assert_unchecked(result <= self.len())",
    ):
        if fragment not in binary:
            raise SystemExit(f"unbound core binary-search operation: {fragment}")
    if "if mid <= self.len()" not in split or "self.split_at_unchecked(mid)" not in split:
        raise SystemExit("unbound core split operation")

    iterator = source.with_name("iter.rs").read_bytes()
    if hashlib.sha256(iterator).hexdigest() != PINNED_ITER:
        raise SystemExit("core slice iterator differs from verified Rust 1.95.0")
    for fragment in (
        "self.array_windows().all(|[a, b]| compare(a, b))",
        "pub const fn first_chunk<const N: usize>(&self) -> Option<&[T; N]> {\n        if self.len() < N",
    ):
        if fragment not in text:
            raise SystemExit(f"unbound pair window operation: {fragment}")
    if "let ret = self.v.first_chunk();\n        if ret.is_some() {\n            self.v = &self.v[1..];" not in iterator.decode():
        raise SystemExit("unbound pair window advance")

    subprocess.run(
        [os.environ.get("JZAP_VERUS", "verus"), str(PROOF), "--no-cheating"], check=True
    )


if __name__ == "__main__":
    main()
