#!/usr/bin/env python3
"""Require strict Miri to reject actual-source alias and uninitialized-read bugs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import shutil
from arithmetic_projection import replace_once

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target/verification/memory-controls"


def method(text, marker):
    start = text.index("{", text.index(marker)); end = start + 1; depth = 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}"); end += 1
    return start, end


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "summary.json").unlink(missing_ok=True)
    files = sorted((ROOT / "src").rglob("*.rs")) + [ROOT / "tests/unsafe_paths.rs", Path(__file__)]
    fingerprint = lambda: hashlib.sha256(b"".join(str(p.relative_to(ROOT)).encode() + b"\0" + p.read_bytes() for p in files)).hexdigest()
    before = fingerprint(); results = []
    for name, file, marker, old, new, test, expected in [
        ("overlapping_split", "src/vec/convenience.rs", "pub fn split_at_spare_mut(",
         "self.ptr.as_ptr().add(self.len.get()).cast()", "self.ptr.as_ptr().add(0).cast()",
         "split_borrows_reallocation_and_noncopy_values_keep_distinct_ownership", ["borrow", "tag", "permission"]),
        ("uninitialized_slot", "src/init.rs", "pub fn get(",
         "if self.initialized {", "if !self.initialized {",
         "empty_slot_observation_never_reads_uninitialized_storage", ["uninitialized"]),
    ]:
        with tempfile.TemporaryDirectory(prefix="jzap-miri-control-") as tmp:
            source = Path(tmp)
            for folder in ["src", "tests", "examples"]: shutil.copytree(ROOT / folder, source / folder)
            for filename in ["Cargo.toml", "Cargo.lock"]: shutil.copy2(ROOT / filename, source / filename)
            path = source / file; text = path.read_text(); start, end = method(text, marker)
            path.write_text(text[:start] + replace_once(text[start:end], old, new) + text[end:])
            env = dict(os.environ, MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-strict-provenance",
                       CARGO_TARGET_DIR=str(OUT / "cargo"))
            command = ["cargo", "+nightly-2026-10-30", "miri", "test", "--locked", "--offline",
                       "--all-features", "--test", "unsafe_paths", test, "--", "--exact"]
            result = subprocess.run(command, cwd=source, env=env, capture_output=True, text=True)
            log = result.stdout + result.stderr; (OUT / (name + ".log")).write_text(log)
            assert result.returncode and "Undefined Behavior:" in log and "error[E" not in log, log[-4000:]
            diagnostic = log[log.index("Undefined Behavior:"):]
            assert any(word in diagnostic.lower() for word in expected), diagnostic[:2000]
            results.append(dict(name=name, command=command, detected=True,
                                diagnostic=diagnostic.split("\n\n")[0], exit_code=result.returncode))
    assert fingerprint() == before
    (OUT / "summary.json").write_text(json.dumps(dict(status="pass", source_sha256=before, results=results), indent=2) + "\n")
    print("Two actual-source memory bugs rejected by strict Miri; compilation/assertion failures are not counted.")


if __name__ == "__main__":
    main()
