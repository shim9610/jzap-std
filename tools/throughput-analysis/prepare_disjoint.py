#!/usr/bin/env python3
"""Prepare reproducible read-only HashSet performance experiments from Git.

Only ignored output storage is edited. No positive verification body is
projected: these are compiled timing prototypes, not product/proof changes.
The caller variants share one archive/build path to control path effects.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


VARIANTS = ["base", "noinline", "split", "split_noinline", "min_all", "for", "try_each", "inline", "find"]


def prepare(source_ref, output, variants=None):
    output = output.resolve()
    assert output.is_relative_to(ROOT / "target"), "Use ignored target/ storage"
    output.mkdir(parents=True, exist_ok=True)
    source = subprocess.check_output(
        ["git", "rev-parse", source_ref + "^{commit}"], cwd=ROOT, text=True
    ).strip()
    work = output / "work"
    work.mkdir(exist_ok=True)
    archive = output / "source.tar"
    with archive.open("wb") as stream:
        subprocess.run(["git", "archive", source], cwd=ROOT, stdout=stream, check=True)
    subprocess.run(["tar", "-xf", str(archive), "-C", str(work)], check=True)
    archive.unlink()
    file = work / "src/collections/hash_set/algebra.rs"
    original = file.read_text()
    old = """    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.intersection(other).next().is_none()
    }"""
    direct = """    pub fn is_disjoint(&self, other: &Self) -> bool {
        let (small, large) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        small.iter().all(|key| !large.contains(key))
    }"""
    if original.count(old) != 1:
        old = direct
    if original.count(old) != 1:
        old = """    pub fn is_disjoint(&self, other: &Self) -> bool {
        // Keep the size relation visible in each scan. The optimizer can then
        // infer that a nonempty smaller input implies a nonempty lookup table.
        if self.len() <= other.len() {
            self.iter().all(|key| !other.contains(key))
        } else {
            other.iter().all(|key| !self.contains(key))
        }
    }"""
    assert original.count(old) == 1, "Review the pinned baseline method"
    variants = list(variants or VARIANTS[:5])
    assert len(variants) == len(set(variants)) and set(variants) <= set(VARIANTS)
    binaries = {}
    for name in variants:
        body = "    " + ("#[inline(never)]\n    " if "noinline" in name else "")
        body += "pub fn is_disjoint(&self, other: &Self) -> bool {\n"
        if name in ("min_all", "for", "try_each", "find"):
            body += """        let (small, large) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
"""
            if name == "for":
                body += """        for key in small {
            if large.contains(key) { return false; }
        }
        true
"""
            elif name == "try_each":
                body += """        core::iter::Iterator::try_for_each(&mut small.iter(), |key| {
            if large.contains(key) { Err(()) } else { Ok(()) }
        }).is_ok()
"""
            elif name == "find":
                body += "        small.iter().find(|key| large.contains(*key)).is_none()\n"
            else:
                body += "        small.iter().all(|key| !large.contains(key))\n"
        elif name.startswith("split"):
            body += """        if self.len() <= other.len() {
            self.iter().all(|key| !other.contains(key))
        } else {
            other.iter().all(|key| !self.contains(key))
        }
"""
        else:
            body += "        self.intersection(other).next().is_none()\n"
        body += "    }"
        if name == "base":
            body = old
        elif name == "noinline":
            body = "    #[inline(never)]\n" + old
        elif name == "inline":
            body = "    #[inline(always)]\n" + old
        file.write_text(original.replace(old, body))
        target = output / "call-build"
        command = [
            "cargo", "build", "--locked", "--offline", "--release",
            "--manifest-path", str(work / "tools/throughput-analysis/Cargo.toml"),
            "--features", "hash-set", "--bin", "hash_set", "--target-dir", str(target),
        ]
        with (output / (name + "-build.log")).open("w") as stream:
            subprocess.run(command, cwd=ROOT, stdout=stream, stderr=stream, check=True)
        binary = output / (name + "-calls")
        shutil.copyfile(target / "release/hash_set", binary)
        binary.chmod(0o755)
        binaries[name] = hashlib.sha256(binary.read_bytes()).hexdigest()
        if name == "base":
            libraries = list((target / "release/deps").glob("libjzap-*.rlib"))
            assert len(libraries) == 1, "Use a fresh output directory"
            # Compile now, before later variants replace this cached rlib.
            probe = output / "main.rs"
            shutil.copyfile(Path(__file__).parent / "experiments/disjoint.rs", probe)
            subprocess.run([
                "rustc", "--edition=2024", "--crate-name=disjoint_analysis",
                "-Copt-level=3", "-Ccodegen-units=1", "-Cpanic=abort",
                "--extern", "jzap=" + str(libraries[0]),
                "-L", "dependency=" + str(target / "release/deps"),
                str(probe), "-o", str(output / "probe"),
            ], check=True)
        print("Built", name, flush=True)
    (output / "preparation.json").write_text(json.dumps(dict(
        source_commit=source, binaries=binaries,
        rustc=subprocess.check_output(["rustc", "-Vv"], text=True),
        probe="No-inline API wrappers; shared prepared sets; matching-hasher std control; >=1-second intervals",
        scope="Analysis prototypes only; production source and its certificate are unchanged",
    ), indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-ref", default="f460c807ef83d8eb366623f40ae25672ae4b682a")
    parser.add_argument("--output", type=Path, default=ROOT / "target/verification/hash-set/disjoint-reproduction")
    parser.add_argument("--variants", help="comma-separated forms: " + ",".join(VARIANTS) + "; default: original five")
    args = parser.parse_args()
    prepare(args.source_ref, args.output, args.variants.split(",") if args.variants else None)
