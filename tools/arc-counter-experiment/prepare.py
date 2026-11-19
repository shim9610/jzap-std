#!/usr/bin/env python3
"""Generate disposable CAS/headroom Arc variants; never patch product sources.

The headroom variant is a performance experiment for bounded low-count work.
Its transient overflow values still conflict with the product's retired tag.
It is deliberately not a generally valid replacement for AllocationPolicy/Arc.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "target/verification/arc-counter-experiment"

# The only atomic hot-path change. The small-count build sets the marker to 8.
FAST = '''    #[inline]
    fn clone_owned_strong(&self) -> Result<(), Error> {
        let old = self.0.fetch_add(1, Ordering::Relaxed);
        let guard = MAX_COUNT.wrapping_add(1);
        if old.wrapping_add(1) & guard != 0 {
            self.0.fetch_sub(1, Ordering::Release);
            Err(Error::RefCount)
        } else {
            Ok(())
        }
    }
'''
DEFAULT_METHOD = '''
    #[inline]
    fn clone_owned_strong(&self) -> Result<(), Error> {
        self.increment(false).map(|_| ())
    }
'''

def once(source, old, new):
    assert source.count(old) == 1, ("source shape changed", old)
    return source.replace(old, new)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--output", type=Path, default=DEFAULT)
    p.add_argument("--source-ref", default="8332575ef5bd7aa2a28daa1fa46d22491771598a",
                   help="historical CAS source; the integrated product uses a different protocol")
    args = p.parse_args()
    output = args.output.resolve()
    assert output.is_relative_to(ROOT / "target"), "generated source belongs under ignored target/"
    output.mkdir(parents=True, exist_ok=True)
    source_commit = subprocess.check_output(["git", "rev-parse", args.source_ref + "^{commit}"], cwd=ROOT, text=True).strip()
    snapshot = tempfile.TemporaryDirectory(prefix="historical-input-", dir=output)
    source_root = Path(snapshot.name)
    tracked = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", source_commit], cwd=ROOT, text=True).splitlines()
    for name in tracked:
        if name in ["Cargo.toml", "Cargo.lock", "clippy.toml"] or name.startswith(("src/", "tests/", "examples/", "tools/throughput-analysis/", "tools/shared-loom/")):
            path = source_root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(subprocess.check_output(["git", "show", source_commit + ":" + name], cwd=ROOT))
    checks = (Path(__file__).parent / "tests.rs").read_text()
    source_hash = hashlib.sha256(b"".join(p.read_bytes() for p in sorted((source_root / "src").rglob("*.rs")))).hexdigest()
    for name in ["cas", "headroom"]:
        variant = output / name
        variant.mkdir()  # Fail closed rather than overwrite an earlier experiment.
        for d in ["src", "tests", "examples"]:
            shutil.copytree(source_root / d, variant / d)
        for f in ["Cargo.toml", "Cargo.lock", "clippy.toml"]:
            shutil.copyfile(source_root / f, variant / f)
        for workspace in ["throughput-analysis", "shared-loom"]:
            shutil.copytree(source_root / "tools" / workspace, variant / "tools" / workspace,
                            ignore=shutil.ignore_patterns("target", "__pycache__"))
        (variant / "tools/shared-loom/tests/headroom_negative.rs").write_text((Path(__file__).parent / "counterexample.rs").read_text())
        if name == "headroom":
            path = variant / "src/shared/count.rs"
            code = once(path.read_text(),
                        "    fn increment(&self, live: bool) -> Result<bool, Error>;",
                        "    fn increment(&self, live: bool) -> Result<bool, Error>;" + DEFAULT_METHOD)
            start = code.index("impl Counter for AtomicCount {")
            pos = code.index("    #[inline]\n    fn release(&self)", start)
            path.write_text(code[:pos] + FAST + code[pos:])
            path = variant / "src/shared.rs"
            path.write_text(once(path.read_text(), "counts(this.ptr).0.increment(false)?;",
                                 "counts(this.ptr).0.clone_owned_strong()?;"))
            # The comparison root is otherwise byte-for-byte identical.
            assert [p.relative_to(variant).as_posix() for p in sorted((variant / "src").rglob("*.rs"))
                    if p.read_bytes() != (source_root / p.relative_to(variant)).read_bytes()] == ["src/shared/count.rs", "src/shared.rs"]
            (variant / "tests/headroom_experiment.rs").write_text(checks)
            loom = variant / "tools/shared-loom/tests/concurrent.rs"
            loom.write_text(loom.read_text() + (Path(__file__).parent / "loom.rs").read_text())
    manifest = dict(source_commit=source_commit,
                    product_source_sha256=source_hash, hot_path=FAST,
                    scope="Strong::try_clone only, including owning-ledger Arc clones; upgrades/downgrades/weak clones retain existing CAS",
                    restriction="Known low-count benchmark domain only; retired-tag ambiguity and unbounded pending overflow are not resolved",
                    roots={n:str(output/n) for n in ["cas","headroom"]},
                    variant_source_sha256={n:hashlib.sha256(b"".join(
                        str(p.relative_to(output/n)).encode()+b"\0"+p.read_bytes()
                        for p in sorted((output/n/"src").rglob("*.rs")))).hexdigest()
                        for n in ["cas","headroom"]})
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")
    snapshot.cleanup()
    print(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
