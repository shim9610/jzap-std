#!/usr/bin/env python3
"""Replay a Kani counterexample natively, in a temporary copy of the crate.

    python3 scripts/kani_playback.py --harness PATH::TO::HARNESS \\
        --file src/path/of/the/harness.rs [--kani-arg ARG ...] [--output DIR]

Kani writes the concrete-playback unit test into the source file that holds
the harness, resolved from the working directory, so generation runs inside a
copy of `src`, `tests`, `examples` and the manifests under `target/`, and the
checked-out file is compared before and after. Duplicate generated tests are
removed, `std` is made available to the harness module, and the tests run
natively with `cargo kani playback`.

Exit status 0 means every generated test ran and passed natively: the
counterexample does not reproduce, so the failure is a Kani false
counterexample for the reported inputs, not a demonstrated defect. Exit status
1 means a test failed natively (a real counterexample) or none was generated.
Logs go to `--output` (default `target/verification/kani-playback/<harness>`).
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--harness", required=True, help="full harness path, as passed to --harness --exact")
    parser.add_argument("--file", required=True, type=Path, help="source file holding the harness, relative to the crate")
    parser.add_argument("--kani-arg", action="append", default=[],
                        help="extra `cargo kani` argument, repeatable (e.g. --solver=kissat)")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    name = args.harness.rsplit("::", 1)[-1]
    out = (args.output or ROOT / "target/verification/kani-playback" / name).resolve()
    out.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / "target/verification"
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="kani-playback-", dir=scratch) as directory:
        work = Path(directory)
        for item in ("src", "tests", "examples"):
            if (ROOT / item).exists():
                shutil.copytree(ROOT / item, work / item)
        for item in ("Cargo.toml", "Cargo.lock"):
            shutil.copyfile(ROOT / item, work / item)
        env = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
        original = (ROOT / args.file).read_bytes()
        generate = subprocess.run(
            # The extra arguments go last: they may end with --cbmc-args.
            ["cargo", "kani", "--lib", "--harness", args.harness, "--exact",
             "-Z", "concrete-playback", "--concrete-playback=inplace", *args.kani_arg],
            cwd=work, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        (out / "generate.log").write_text(generate.stdout)
        if (ROOT / args.file).read_bytes() != original:
            sys.exit("concrete playback modified the checked-out source")
        path = work / args.file
        text = path.read_text()
        seen = []
        for match in list(re.finditer(
                r"(?:\n///[^\n]*)*\n#\[test\]\nfn (kani_concrete_playback_\w+)\(\) \{.*?\n\}\n", text, re.S)):
            if match.group(1) in seen:
                text = text.replace(match.group(0), "\n", 1)
            else:
                seen.append(match.group(1))
        if not seen:
            print(f"no concrete playback was generated; see {out / 'generate.log'}")
            return 1
        # Harness modules build without std; the generated tests use vec!.
        text = re.sub(r"(#\[cfg\(kani\)\]\nmod \w+ \{\n)", r"\1    extern crate std;\n    use std::{vec, vec::Vec};\n", text)
        path.write_text(text)
        replay = subprocess.run(["cargo", "kani", "playback", "-Z", "concrete-playback", "--lib", "--",
                                 "kani_concrete_playback"],
                                cwd=work, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        (out / "replay.log").write_text(replay.stdout)
    outcomes = dict(re.findall(r"^test \S*::(kani_concrete_playback_\w+) \.\.\. (\w+)$", replay.stdout, re.M))
    if set(outcomes) != set(seen):
        print(f"not every generated test ran; see {out / 'replay.log'}")
        return 1
    failed = [test for test, outcome in outcomes.items() if outcome != "ok"]
    for test, outcome in outcomes.items():
        print(f"{test}: {outcome}")
    if failed:
        print("the counterexample reproduces natively: treat it as a real defect")
        return 1
    print(f"{len(seen)} concrete playback(s) pass natively: Kani false counterexample for these inputs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
