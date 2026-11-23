"""Build the trusted core specifications that vstd lacks and supply their import.

`tools/direct-verus/std-specs.rs` holds `assume_specification` entries for
Rust 1.95 core functions used by directly verified product code. The product
crate is verified with `--no-cheating`, so these entries live in a separate
crate, like vstd's own `std_specs`. Before building, `check()` binds every entry
to the installed core source: the function signature and the documented
behaviour that the entry states must both be present, and the file may contain
no other trusted entry.
"""
import hashlib
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tools/direct-verus/std-specs.rs"
CRATE = "jzap_verus_std_specs"
OUT = ROOT / "target/verification/verus-std-specs"

# Each entry: the core file, the declaration and the documented behaviour that
# the specification restates, and the `ensures` text of the specification.
ENTRIES = {
    "core::hint::select_unpredictable::<T>": dict(
        core="hint.rs",
        declaration="pub const fn select_unpredictable<T>(condition: bool, true_val: T, false_val: T) -> T",
        documentation=[
            "/// This method is functionally equivalent to",
            "/// fn select_unpredictable<T>(b: bool, true_val: T, false_val: T) -> T {",
            "///     if b { true_val } else { false_val }",
        ],
        ensures="r == if condition { true_val } else { false_val },",
    ),
}


def check():
    rustc = subprocess.check_output(["rustc", "--version"], text=True).strip()
    assert rustc.startswith("rustc 1.95.0 "), rustc
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    core = sysroot / "lib/rustlib/src/rust/library/core/src"
    text = SOURCE.read_text()
    assert "external_body" not in text and "admit" not in text and "assume(" not in text, \
        "std-specs.rs may only contain assume_specification entries"
    names = re.findall(r"assume_specification\s*(?:<[^>]*>)?\s*\[\s*([^\]]+?)\s*\]", text)
    assert sorted(names) == sorted(ENTRIES), ("unreviewed std specification set", names)
    for name, entry in ENTRIES.items():
        source = (core / entry["core"]).read_text()
        assert entry["declaration"] in source, ("core declaration changed", name)
        for line in entry["documentation"]:
            assert line in source, ("core documentation changed", name, line)
        assert entry["ensures"] in text, ("specification text changed", name)
    return dict(rustc=rustc, entries=sorted(ENTRIES),
                source_sha256=hashlib.sha256(text.encode()).hexdigest())


def build(binary):
    """Check and build the spec crate; return the flags that import it."""
    check()
    OUT.mkdir(parents=True, exist_ok=True)
    vir = OUT / f"{CRATE}.vir"
    rlib = OUT / f"lib{CRATE}.rlib"
    result = subprocess.run(
        [str(binary), str(SOURCE), "--crate-type", "lib", "--crate-name", CRATE,
         "--edition=2024", "--export", str(vir),
         "--compile", "-o", str(rlib)],
        cwd=OUT, capture_output=True, text=True)
    if result.returncode != 0 or not vir.exists() or not rlib.exists():
        raise RuntimeError("std specification crate did not build:\n" + result.stdout + result.stderr)
    return ["--import", f"{CRATE}={vir}", "--extern", f"{CRATE}={rlib}", "-L", str(OUT)]


if __name__ == "__main__":
    import os
    print(check())
    print(build(os.environ.get("JZAP_VERUS", "verus")))
