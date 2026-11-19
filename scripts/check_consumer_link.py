#!/usr/bin/env python3
"""Check a consumer module in a no_std final ELF link, with negative controls.

The module exports `pub fn run(seed: usize) -> Result<(), jzap::Error>` and can
implement public jzap traits. This checks retained panic and direct exit edges
for these concrete roots, not all generic implementations or the user's app.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tools/consumer-audit"
EXITS = ("abort", "exit", "_Exit", "_exit", "quick_exit", "pthread_exit", "__assert_fail")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=FIXTURE / "cases/checked.rs")
    parser.add_argument("--source-dependency", type=Path, action="append", default=[])
    parser.add_argument("--require-success", action="store_true",
                        help="require success without injection and Err at each injected allocation")
    parser.add_argument("--output", type=Path, default=ROOT / "target/verification/consumer-iterator")
    args = parser.parse_args()
    if os.uname().machine != "x86_64" or os.uname().sysname != "Linux":
        parser.error("This fixture requires Linux x86_64, Cargo and a GNU-compatible C linker")
    if not args.source.is_file():
        parser.error("--source must name an existing Rust source file")
    source = args.source.resolve(strict=True)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    files = sorted((ROOT / "src").rglob("*.rs")) + [
        FIXTURE / "Cargo.toml", FIXTURE / "Cargo.lock", FIXTURE / "src/lib.rs",
        FIXTURE / "driver.c", Path(__file__), source,
    ] + sorted((FIXTURE / "cases").glob("*.rs")) + [
        path.resolve(strict=True) for path in args.source_dependency
    ]
    report = dict(
        source=str(source), source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
        inputs={str(p.relative_to(ROOT) if p.is_relative_to(ROOT) else p): hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
        rustc=subprocess.check_output(["rustc", "-Vv"], text=True).strip(), results=[],
        scope="Concrete no_std ELF roots only; direct wrapped exits and retained panic edges. Not UB, termination, indirect/foreign exits or all caller inputs.",
    )
    cases = [("positive", FIXTURE / "cases/checked.rs", False),
             ("consumer", source, False)] + [
                 (name + "-control", FIXTURE / "cases" / (name + ".rs"), True)
                 for name in ("overflow", "bounds", "abort")]
    for profile, lto in [("release-checked", "false"), ("release-lto-checked", "fat")]:
        for name, path, negative in cases:
            work = out / profile / name
            work.mkdir(parents=True, exist_ok=True)
            env = os.environ.copy()
            env.pop("RUSTFLAGS", None)
            env.pop("CARGO_ENCODED_RUSTFLAGS", None)
            settings = dict(CARGO_PROFILE_RELEASE_LTO=lto,
                            CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS="true",
                            CARGO_PROFILE_RELEASE_PANIC="abort",
                            JZAP_CONSUMER_SOURCE=str(path.resolve()))
            env.update(settings)
            build = ["cargo", "build", "--locked", "--offline", "--release", "--manifest-path",
                     str(FIXTURE / "Cargo.toml"), "--target-dir", str(out / "cargo" / profile)]
            row = dict(profile=profile, name=name, negative_control=negative,
                       environment=settings, build=build)
            report["results"].append(row)
            def run_step(command, step, **kwargs):
                try:
                    return subprocess.run(command, **kwargs)
                except subprocess.TimeoutExpired:
                    row["status"] = step + "-timeout"
                    row[step + "_exit"] = 124
                    return None
            with (work / "build.log").open("w") as log:
                build_result = run_step(build, "build", cwd=ROOT, env=env, stdout=log, stderr=log, timeout=180)
            if build_result is None:
                pass
            elif build_result.returncode:
                row["build_exit"] = build_result.returncode
                row["status"] = "build-error"
            else:
                row["build_exit"] = 0
                archive = out / "cargo" / profile / "release/libjzap_consumer_audit.a"
                binary = work / "audit"
                link = ["cc", "-O2", "-Wall", "-Wextra", "-Werror", "-ffunction-sections", "-fdata-sections",
                        str(FIXTURE / "driver.c"), str(archive), "-Wl,--gc-sections",
                        "-Wl,-Map=" + str(work / "link.map")]
                link += ["-Wl,--wrap=" + symbol for symbol in EXITS]
                if name == "consumer" and args.require_success:
                    link += ["-DAUDIT_REQUIRE_SUCCESS"]
                link += ["-ldl", "-lpthread", "-lm", "-o", str(binary)]
                row["link"] = link
                linked = run_step(link, "link", cwd=ROOT, capture_output=True, text=True, timeout=60)
                diagnostic = "link timeout" if linked is None else linked.stdout + linked.stderr
                (work / "link.log").write_text(diagnostic)
                row["link_exit"] = 124 if linked is None else linked.returncode
                forbidden = bool(re.search(r"undefined reference.*(?:jzap_link_audit_panic_reachable|__wrap_)", diagnostic))
                if linked is None:
                    pass
                elif negative:
                    row["status"] = "control-detected" if linked.returncode and forbidden else "control-invalid"
                elif linked.returncode:
                    row["status"] = "retained-exit-edge" if forbidden else "link-error"
                else:
                    result = run_step([str(binary)], "runtime", capture_output=True, text=True, timeout=30)
                    if result is not None:
                        row.update(runtime_exit=result.returncode, runtime_output=result.stdout + result.stderr)
                        row["status"] = "pass" if result.returncode == 0 else "runtime-error"
            (out / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
            print(profile, name, row["status"], flush=True)
    if any(row["status"] != ("control-detected" if row["negative_control"] else "pass") for row in report["results"]):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
