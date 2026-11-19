#!/usr/bin/env python3
"""Reproduce the Crux enum boundary experiment and optional Vec diagnostics.

Positive harnesses include/import original product code. Only harness paths,
Cargo fixture metadata and explicitly negative source copies are generated.
No result from this probe retires a Vec caller projection.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "tools/crux-probe"


def product_hashes():
    names = subprocess.check_output(
        ["git", "ls-files", "src", "Cargo.toml", "Cargo.lock"], cwd=ROOT, text=True
    ).splitlines()
    return {n: hashlib.sha256((ROOT / n).read_bytes()).hexdigest() for n in names}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", required=True, type=Path,
                        help="Extracted saw-suite directory containing bin/ and rlibs/")
    parser.add_argument("--output", type=Path,
                        default=ROOT / "target/verification/vec-crux-probe/replay")
    parser.add_argument("--include-callers", action="store_true",
                        help="Also reproduce the incomplete original Vec and tool diagnostics")
    args = parser.parse_args()
    suite = args.suite.resolve()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target/verification"):
        parser.error("Output must be under this repository's target/verification/")
    out.mkdir(parents=True, exist_ok=True)
    before = product_hashes()
    env = dict(os.environ, PATH=str(suite / "bin") + os.pathsep + os.environ["PATH"],
               CRUX_RUST_LIBRARY_PATH=str(suite / "rlibs"), CARGO_NET_OFFLINE="true",
               CARGO_TARGET_DIR=str(out / "cargo-target"))
    results = []

    def run(name, command, cwd=out, classification="positive"):
        try:
            p = subprocess.run(command, cwd=cwd, env=env, text=True,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=180)
            code, log = p.returncode, p.stdout
        except subprocess.TimeoutExpired as exc:
            code, log = None, exc.stdout or ""
            if isinstance(log, bytes):
                log = log.decode(errors="replace")
        (out / (name + ".log")).write_text(log)
        clean = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", log)
        counts = {key.lower(): int(n) for key, n in re.findall(
            r"\[Crux\]\s+(Total|Proved|Disproved|Incomplete|Unknown):\s+(\d+)", clean)}
        tool_error = "Translation error" in clean or "Unsupported feature:" in clean
        valid = (code == 0 and counts.get("total", 0) > 0
                 and counts.get("proved") == counts.get("total") and not tool_error
                 and "Overall status: Valid." in clean)
        rejected = (code not in (None, 0) and counts.get("disproved", 0) > 0
                    and "MIR assertion at" in clean and not tool_error
                    and counts.get("incomplete") == 0 and counts.get("unknown") == 0)
        status = ("pass" if valid else "rejected" if rejected else
                  "blocked" if tool_error else "timeout" if code is None else "failed")
        row = dict(name=name, command=list(map(str, command)), cwd=str(cwd),
                   classification=classification, exit=code, status=status, goals=counts,
                   log=name + ".log")
        results.append(row)
        print(f"{name}: {status} {counts}", flush=True)
        return row

    tool = str(suite / "bin/crux-mir-comp")
    flags = ["--solver=z3", "--path-sat", "--timeout=60", "--goal-timeout=15"]
    versions = {}
    for binary in ("crux-mir-comp", "mir-json", "z3"):
        p = subprocess.run([str(suite / "bin" / binary), "--version"], env=env,
                           capture_output=True, text=True, timeout=30)
        versions[binary] = dict(exit=p.returncode, output=p.stdout + p.stderr)
        if p.returncode:
            raise RuntimeError(f"Tool prerequisite failed: {binary}: {p.stderr}")
    source = HARNESS.joinpath("error-leaf.rs").read_text().replace(
        "../../src/error.rs", str(ROOT / "src/error.rs"))
    (out / "error-leaf.rs").write_text(source)
    positive = run("leaf", [tool, "error-leaf.rs", *flags, "--output-directory=leaf"])
    (out / "wrong-variant.rs").write_text(source.replace(
        "input.map_err(InitError::<Error>::Storage)",
        "input.map_err(InitError::<Error>::Initialize)"))
    variant = run("wrong-variant", [tool, "wrong-variant.rs", *flags,
                  "--test-filter=constructor_as_function", "--output-directory=wrong-variant"],
                  classification="negative")
    original = (ROOT / "src/error.rs").read_text()
    assert original.count("Self::Storage(error)") == 1
    (out / "wrong-payload-error.rs").write_text(original.replace(
        "Self::Storage(error)", "Self::Storage(Error::Bounds)"))
    (out / "wrong-payload.rs").write_text(source.replace(
        str(ROOT / "src/error.rs"), str(out / "wrong-payload-error.rs")))
    payload = run("wrong-payload", [tool, "wrong-payload.rs", *flags,
                  "--test-filter=original_from_implementation", "--output-directory=wrong-payload"],
                  classification="negative")
    if args.include_callers:
        for name in ("zst-offset", "allocation-shim"):
            shutil.copyfile(HARNESS / (name + ".rs"), out / (name + ".rs"))
            run(name, [tool, name + ".rs", *flags, "--output-directory=" + name],
                classification="tool-diagnostic")
        caller = out / "caller"
        (caller / "src").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(HARNESS / "caller.rs", caller / "src/lib.rs")
        (caller / "Cargo.toml").write_text(
            '[package]\nname = "jzap-crux-original-caller-probe"\nversion = "0.0.0"\n'
            'edition = "2024"\n[dependencies]\njzap-std = { path = '
            + json.dumps(str(ROOT)) + ' }\n')
        for kind in ("zst", "byte"):
            run("caller-" + kind, ["cargo", "+nightly-2026-03-21", "crux-test", "--lib",
                "original_initializer_limited_" + kind, "--", *flags,
                "--output-directory=" + str(out / ("caller-" + kind))], cwd=caller,
                classification="original-caller-probe")
    after = product_hashes()
    good_leaf = positive["status"] == "pass" and all(
        r["status"] == "rejected" for r in (variant, payload))
    summary = dict(source_commit=subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        status="partial" if good_leaf and args.include_callers else "pass" if good_leaf else "failed",
        scope="x86_64 enum boundary only; no Vec caller projection retired",
        source_unchanged=before == after, product_files_checked=len(before),
        product_hashes=after, versions=versions, results=results,
        vec_caller_projections_retired=0)
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    assert before == after, "Product files changed during probe"
    return 0 if good_leaf and not args.include_callers else 2 if good_leaf else 1


if __name__ == "__main__":
    raise SystemExit(main())
