#!/usr/bin/env python3
"""Link concrete Box/Vec/String roots with an undefined panic sink and wrapped exits.

A successful link proves absence of these retained direct edges for this fixture,
not absence of UB, nontermination, indirect/foreign exits, or arbitrary callbacks.
Raw builds, maps and diagnostics remain under target/verification/.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PROFILES = {
    "release": (True, {}),
    "release-lto": (True, {"CARGO_PROFILE_RELEASE_LTO": "fat"}),
    "release-checked": (True, {"CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS": "true"}),
    "debug": (False, {"CARGO_PROFILE_DEV_DEBUG": "1"}),
}
EXITS = ["abort", "exit", "_Exit", "_exit", "quick_exit", "pthread_exit", "__assert_fail"]
CONTROLS = ["panic-control", "bounds-control", "oom-control", "abort-control"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profiles", default=",".join(PROFILES))
    parser.add_argument("--output", type=Path, default=ROOT / "target/verification/link-audit")
    args = parser.parse_args()
    names = args.profiles.split(",")
    if not set(names) <= PROFILES.keys():
        parser.error("Unknown profile")
    if os.uname().machine != "x86_64" or os.uname().sysname != "Linux":
        parser.error("This ELF/disassembly fixture currently targets Linux x86_64")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    source_files = sorted((ROOT / "src").rglob("*.rs")) + sorted(
        p for p in (ROOT / "tools/link-audit").rglob("*")
        if p.is_file() and p.suffix in (".rs", ".c", ".toml", ".lock") and "target" not in p.parts) + [Path(__file__)]
    source_sha256 = hashlib.sha256(b"".join(
        str(p.relative_to(ROOT)).encode() + b"\0" + p.read_bytes() for p in source_files
    )).hexdigest()
    results = []
    for name in names:
        release, extra = PROFILES[name]
        # Negative controls are run in both optimized link configurations.
        controls = [None] + (CONTROLS if name in ("release", "release-lto") else [])
        for control in controls:
            work = args.output / name / (control or "product")
            work.mkdir(parents=True, exist_ok=True)
            env = os.environ.copy()
            env.pop("RUSTFLAGS", None)
            env.pop("CARGO_ENCODED_RUSTFLAGS", None)
            for key in ("CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS"):
                env.pop(key, None)
            env.update(extra)
            build = ["cargo", "build", "--locked", "--offline", "--manifest-path",
                     "tools/link-audit/Cargo.toml", "--target-dir", str(work / "cargo")]
            if release:
                build.append("--release")
            if control:
                build += ["--features", control]
            archive = work / "cargo" / ("release" if release else "debug") / "libjzap_link_audit.a"
            binary = work / "audit"
            link = ["cc", "-O2", "-Wall", "-Wextra", "-Werror", "-ffunction-sections", "-fdata-sections",
                    "tools/link-audit/driver.c", str(archive), "-Wl,--gc-sections",
                    "-Wl,-Map=" + str(work / "link.map")]
            link += ["-Wl,--wrap=" + symbol for symbol in EXITS]
            link += ["-ldl", "-lpthread", "-lm", "-o", str(binary)]
            row = dict(profile=name, control=control, environment=extra, build=build, link=link)
            with (work / "build.log").open("w") as log:
                row["build_exit"] = subprocess.run(build, cwd=ROOT, env=env, stdout=log, stderr=log).returncode
            if row["build_exit"]:
                row["status"] = "build-error"
            else:
                result = subprocess.run(link, cwd=ROOT, text=True, capture_output=True)
                diagnostic = result.stdout + result.stderr
                (work / "link.log").write_text(diagnostic)
                row["link_exit"] = result.returncode
                row["forbidden_edge_reported"] = bool(re.search(
                    r"undefined reference.*(?:jzap_link_audit_panic_reachable|__wrap_)", diagnostic))
                if control:
                    row["status"] = ("control-detected" if result.returncode and row["forbidden_edge_reported"]
                                     else "control-invalid")
                elif result.returncode:
                    row["status"] = "retained-exit-edge" if row["forbidden_edge_reported"] else "link-error"
                    diagnostic_binary = work / "diagnostic"
                    diagnostic_link = ["cc", "-O2", "-Wall", "-Wextra", "-Werror", "-ffunction-sections", "-fdata-sections",
                        "tools/link-audit/driver.c", "tools/link-audit/diagnostic.c", str(archive),
                        "-Wl,--gc-sections", "-ldl", "-lpthread", "-lm", "-o", str(diagnostic_binary)]
                    with (work / "diagnostic-link.log").open("w") as log:
                        code = subprocess.run(diagnostic_link, cwd=ROOT, stdout=log, stderr=log).returncode
                    row["diagnostic_link_exit"] = code
                    if code == 0:
                        asm = subprocess.check_output(["objdump", "-dCl", str(diagnostic_binary)], text=True)
                        (work / "diagnostic-disassembly.txt").write_text(asm)
                        symbols = {int(address, 16): symbol for address, symbol in re.findall(
                            r"^([0-9a-f]+) <(.+)>:$", asm, re.M)}
                        relocations = subprocess.check_output(["objdump", "-R", str(diagnostic_binary)], text=True)
                        got = {int(slot, 16): int(target, 16) for slot, target in re.findall(
                            r"^([0-9a-f]+)\s+R_X86_64_RELATIVE\s+\*ABS\*\+0x([0-9a-f]+)", relocations, re.M)}
                        calls = []
                        function, location = "", ""
                        source_context = []
                        for line in asm.splitlines():
                            if re.match(r"^[0-9a-f]+ <", line):
                                function = line
                                location = ""
                                source_context = []
                            if line.startswith("/"):
                                location = line
                                if line.startswith(str(ROOT / "src") + "/"):
                                    if line not in source_context:
                                        source_context.append(line)
                            if not re.search(r"\b(?:call|jmp)\s", line): continue
                            indirect = re.search(r"# ([0-9a-f]+) <", line)
                            direct = re.search(r"\b(?:call|jmp)\s+([0-9a-f]+) <", line)
                            target = ""
                            if indirect:
                                target = symbols.get(got.get(int(indirect[1], 16)), "")
                            elif direct:
                                target = symbols.get(int(direct[1], 16), "")
                            if target.startswith(("core::panicking", "alloc::alloc::handle_alloc_error", "core::slice::index::slice_index_fail")):
                                # Debug locations can point at a core macro, and
                                # optimized cold blocks may merge several source
                                # expressions. Preserve all observed context;
                                # the proof gate must not guess a unique site.
                                calls.append(dict(function=function, location=location,
                                    source_context=list(source_context), target=target,
                                    instruction=line.strip()))
                        (work / "retained-panic-calls.json").write_text(json.dumps(calls, indent=2))
                        row["retained_panic_call_sites"] = len(calls)
                        diagnostic_runs = []
                        for seed in range(65):
                            run = subprocess.run([str(diagnostic_binary), str(seed)], capture_output=True, text=True, timeout=10)
                            if run.returncode:
                                row["diagnostic_runtime_error"] = dict(seed=seed, exit_code=run.returncode, stderr=run.stderr)
                                break
                            diagnostic_runs.append(json.loads(run.stdout))
                        row["diagnostic_runtime_cases"] = sum(r["runs"] for r in diagnostic_runs)
                        row["diagnostic_runtime_refusals"] = sum(r["refusals"] for r in diagnostic_runs)
                        row["diagnostic_seeds"] = len(diagnostic_runs)
                        (work / "diagnostic-runtime.json").write_text(json.dumps(diagnostic_runs, indent=2))
                else:
                    symbols = subprocess.check_output(["nm", "-u", str(binary)], text=True)
                    (work / "imports.txt").write_text(symbols)
                    assembly = subprocess.check_output(["objdump", "-d", str(binary)], text=True)
                    (work / "disassembly.txt").write_text(assembly)
                    row["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
                    row["trap_instructions"] = len(re.findall(r"\t(?:ud2|int3)\b", assembly))
                    runs = []
                    for seed in range(65):
                        run = subprocess.run([str(binary), str(seed)], capture_output=True, text=True, timeout=10)
                        if run.returncode:
                            row["runtime_error"] = dict(seed=seed, exit_code=run.returncode, stderr=run.stderr)
                            break
                        runs.append(json.loads(run.stdout))
                    (work / "runtime.json").write_text(json.dumps(runs, indent=2))
                    row["runtime_cases"] = sum(r["runs"] for r in runs)
                    row["runtime_refusals"] = sum(r["refusals"] for r in runs)
                    row["seeds"] = len(runs)
                    row["status"] = "pass" if len(runs) == 65 and not row["trap_instructions"] else "runtime-or-trap-failure"
            results.append(row)
            print(name, control or "product", row["status"], flush=True)
            (args.output / "summary.json").write_text(json.dumps(dict(
                source_commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                source_sha256=source_sha256,
                rustc=subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
                scope=__doc__, wrapped_exits=EXITS, results=results), indent=2) + "\n")
    return int(any(r["status"] not in ("pass", "control-detected") for r in results))


if __name__ == "__main__":
    raise SystemExit(main())
