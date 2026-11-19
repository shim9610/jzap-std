#!/usr/bin/env python3
"""Prove the pinned Rust core debug predicates under valid-owner contracts.

The bridge rewrites for overflowing_add and abs_diff are separately verified on
the real Rust primitives by Kani, for u32, u64 and usize. No new trusted Verus
specification is introduced for those currently unsupported operations.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from pointer_predicates import pinned, program

ROOT = Path(__file__).resolve().parents[1]
LAWS = ["native_overflow_flag_matches_checked_add", "native_abs_diff_matches_ordered_subtraction",
        "native_power_of_two_matches_bit_mask"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mutations", action="store_true")
    args = parser.parse_args()
    out = ROOT / "target/verification/pointer-checks"
    out.mkdir(parents=True, exist_ok=True)
    rustc, inputs, bodies = pinned()
    env = os.environ.copy()
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    env["RUSTFLAGS"] = "--cfg jzap_loop_contracts"
    verus = Path(env.get("JZAP_VERUS", "/workspace/.devtools/verus/verus-x86-linux/verus"))
    env["PATH"] = str(verus.parent) + os.pathsep + env["PATH"]
    command = ["cargo", "kani", "--lib", "-Z", "loop-contracts"]
    for name in LAWS:
        command += ["--harness", name]
    with (out / "native-laws.log").open("w") as log:
        r = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=log)
    log = (out / "native-laws.log").read_text()
    assert r.returncode == 0 and "Complete - 3 successfully verified harnesses, 0 failures, 3 total." in log
    template, contracts = program()
    report = dict(rustc=rustc, kernel_sources={name: hashlib.sha256(text.encode()).hexdigest()
        for name, text in inputs.items()}, native_laws=dict(command=command, exit_code=r.returncode),
        bodies=bodies, results=[],
        contracts_sha256=hashlib.sha256(contracts.encode()).hexdigest(), boundary="Valid live allocation, layout, ownership and nonoverlap contracts; not a whole-crate pointer provenance proof.")

    def verify(name, pieces):
        text = template
        for key, value in pieces.items():
            text = text.replace(key, value)
        path = out / (name + ".generated.rs")
        path.write_text(text)
        command = [str(verus), str(path), "--crate-name", "jzap_pointer_checks", "--no-cheating"]
        r = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        log = r.stdout + r.stderr
        (out / (name + ".log")).write_text(log)
        row = dict(name=name, command=command, exit_code=r.returncode,
                   verification=re.findall(r"verification results:: (\d+) verified, (\d+) errors", log))
        report["results"].append(row)
        return row, log

    row, log = verify("pointer-checks", bodies)
    assert row["exit_code"] == 0, log
    if args.mutations:
        for name, key, old, new in [
            ("add_flag", "POINTER_ADD_BODY", "&& !overflow", "&& overflow"),
            ("sub_address", "POINTER_SUB_BODY", "base >= byte_offset", "base < byte_offset"),
            ("slice_length", "SLICE_SIZE_BODY", "len <= max_len", "len > max_len"),
            ("copy_distance", "COPY_NONOVERLAP_BODY", "diff >= size", "diff <= size"),
            ("index_bound", "INDEX_BODY", "this < len", "this > len"),
            ("index_range_bound", "INDEX_RANGE_BODY", "end <= len", "end >= len"),
            ("replace_nonnull", "REPLACE_BODY", "base!=0", "base==0"),
        ]:
            assert bodies[key].count(old) == 1
            row, log = verify("mutation-" + name, dict(bodies, **{key: bodies[key].replace(old, new)}))
            row["detected"] = (row["exit_code"] != 0 and
                any(int(n) > 0 for _, n in row["verification"]) and "error[E" not in log)
            assert row["detected"], log
        # A documented condition weakened in one clause must leave the pinned
        # predicate unproved: each premise is used, not merely stated.
        for name, old, new in [
            ("contract_sub_address", "    && offset <= addr as int\n", ""),
            ("contract_add_bounds", "    && in_bounds(a, addr, offset)\n}", "\n}"),
            ("contract_slice_size", "    && bytes <= isize::MAX as int\n    && addr as int + bytes <= usize::MAX as int\n    && valid_access(a, addr, bytes)", ""),
            ("contract_overlap", "(src as int + bytes <= dst as int || dst as int + bytes <= src as int)", "true"),
            ("contract_index", "    index < len\n", "    index <= len\n"),
            ("contract_range", "    start <= end && end <= len\n", "    start <= end\n"),
            ("contract_replace_alignment", "    valid_access(a, dst, size as int) && aligned(dst, align)\n}", "    valid_access(a, dst, size as int)\n}"),
        ]:
            assert template.count(old) == 1, name
            text = template.replace(old, new)
            for key, value in bodies.items():
                text = text.replace(key, value)
            path = out / ("mutation-" + name + ".generated.rs")
            path.write_text(text)
            command = [str(verus), str(path), "--crate-name", "jzap_pointer_checks", "--no-cheating"]
            r = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
            log = r.stdout + r.stderr
            (out / ("mutation-" + name + ".log")).write_text(log)
            row = dict(name="mutation-" + name, command=command, exit_code=r.returncode,
                       verification=re.findall(r"verification results:: (\d+) verified, (\d+) errors", log))
            row["detected"] = (row["exit_code"] != 0 and
                any(int(n) > 0 for _, n in row["verification"]) and "error[E" not in log)
            report["results"].append(row)
            assert row["detected"], log
    (out / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print([(r["name"], r["exit_code"]) for r in report["results"]])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
