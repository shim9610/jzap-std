#!/usr/bin/env python3
"""Verify the source-to-link guard certificate; retain legacy Kani diagnostics.

The default entry uses source_certificate.py and the source-derived Verus
compositions. --legacy-kani reproduces the earlier unsupported whole-loop
instrumentation without treating those failures as successful Kani proofs.
Raw syntax, link diagnostics and solver logs remain under target/verification.
"""
import argparse
import collections
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
HARNESSES = {
    "budget::panic_proofs::reservation_and_commit_preserve_every_valid_state":
        "All valid usize accounting states; reservation/commit/cancel and charge Drop.",
    "budget::panic_proofs::release_preserves_every_valid_owned_charge_partition":
        "All valid usize accounting states, assuming an owned charge <= occupied.",
    "panic_proofs::box_iterator_counters_cover_every_machine_word_length":
        "Box<[()]>; every usize initial length; front/back steps and cleanup.",
    "panic_proofs::vec_single_edits_cover_every_machine_word_length":
        "Vec<()>; every valid usize length/limit/index; single edits and truncation.",
    "panic_proofs::vec_bulk_copy_covers_every_machine_word_length":
        "Vec<()>; every valid usize length/limit and every input slice length.",
    "panic_proofs::vec_retain_all_lengths_with_loop_contracts":
        "Vec<()>; every usize length; nondeterministic predicate result/error; loop induction.",
    "vec::edit::panic_proofs::dedup_all_lengths_with_loop_contracts":
        "Vec<()>; every usize length; nondeterministic comparison result/error; loop induction.",
    "vec::edit::panic_proofs::extraction_preserves_every_valid_cursor_state":
        "ExtractIf<()>; every valid usize cursor state; nondeterministic predicate; loop induction.",
    "vec::edit::panic_proofs::drain_preserves_every_valid_cursor_state":
        "Drain<()>; every valid usize cursor state; front/back steps and tail restoration.",
    "vec::ownership::panic_proofs::vec_iterator_preserves_every_valid_cursor_state":
        "Vec IntoIter<()>; every valid usize cursor state; borrows/front/back/cleanup.",
    "boxed::slice::panic_proofs::box_iterator_preserves_every_valid_cursor_state":
        "Box IntoIter<()>; every valid usize cursor state; front/back/cleanup.",
    "boxed::text::panic_proofs::character_byte_cursor_preserves_every_valid_state":
        "Actual UTF-8 iterator adapter; every valid byte cursor and every Unicode scalar value.",
    "panic_proofs::layout_array_matches_std_for_every_count":
        "Every usize count for u8/u64/three-byte/64-aligned/ZST/aligned-ZST layouts, compared to std.",
    "panic_proofs::box_slice_initialization_covers_every_length_and_callback_result":
        "Box<[()]>; every usize length; callback error/missing write/success; loop induction.",
    "panic_proofs::vec_resize_covers_every_length_and_callback_result":
        "Vec<()>; every valid usize old/new length and limit; callback results; loop induction.",
}


def digest(paths):
    return hashlib.sha256(b"".join(
        str(p.relative_to(ROOT)).encode() + b"\0" + p.read_bytes()
        for p in sorted(paths))).hexdigest()


def run(command, log, env, timeout=240):
    with log.open("w") as output:
        p = subprocess.Popen(command, cwd=ROOT, env=env, stdout=output,
                             stderr=subprocess.STDOUT, start_new_session=True)
        try:
            return p.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGTERM)
            p.wait()
            return 124


def properties(text):
    rows = []
    for match in re.finditer(
            r"Check \d+: ([^\n]+)\n\s*- Status: (\w+)\n"
            r'\s*- Description: "([^\n]*)"\n'
            r"\s*- Location: ([^\n]+)", text):
        ident, status, description, location = match.groups()
        rows.append(dict(id=ident, status=status, description=description, location=location))
    return rows


def legacy_main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/verification/panic-proof")
    parser.add_argument("--profiles", default="release,release-lto,release-checked,debug")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    # Model runners have shared output directories even when --output differs.
    # Concurrent collectors must not overwrite each other's proof evidence.
    lock_path = ROOT / "target/verification/.panic-proof.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    lock = lock_path.open("a")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        parser.error("Another panic proof runner is using the shared solver outputs")
    env = os.environ.copy()
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    env["RUSTFLAGS"] = "--cfg jzap_loop_contracts"
    # The verifier's pinned Z3 is a verification dependency, never a product tool.
    verus = Path(env.get("JZAP_VERUS", "/workspace/.devtools/verus/verus-x86-linux/verus"))
    env["PATH"] = str(verus.parent) + os.pathsep + env["PATH"]
    inputs = sorted((ROOT / "src").rglob("*.rs"))
    proof_inputs = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", Path(__file__)]
    proof_inputs += sorted(p for p in (ROOT / "tools/memory-proof").glob("*.rs"))
    proof_inputs += sorted(p for p in (ROOT / "tools/panic-audit").rglob("*")
                           if p.is_file() and "target" not in p.parts)
    proof_inputs += [ROOT / "scripts" / name for name in (
        "verify_direct_verus.py", "verify_source.py", "verify_panic_state.py",
        "verify_state_types.py", "arithmetic_projection.py",
        "verify_pointer_checks.py", "check_link_exits.py")]
    proof_inputs += sorted(p for p in (ROOT / "tools/link-audit").rglob("*")
                           if p.is_file() and "target" not in p.parts)
    snapshot = digest(inputs)
    verification_snapshot = digest(proof_inputs)
    report = dict(source_sha256=snapshot, verification_sha256=verification_snapshot,
                  complete=False, proofs=[], branches=[], errors=[], models=[],
                  environment={"RUSTFLAGS": env["RUSTFLAGS"]}, negative_controls=[])

    def save():
        (out / "summary.json").write_text(json.dumps(report, indent=2) + "\n")

    command = ["cargo", "build", "--locked", "--offline", "--manifest-path",
               "tools/panic-audit/Cargo.toml", "--target-dir", str(out / "parser")]
    parser_env = env.copy()
    parser_env.pop("RUSTFLAGS", None)
    code = run(command, out / "parser-build.log", parser_env)
    if code:
        report["errors"].append(dict(step="parser-build", exit_code=code))
        save()
        return 1
    files = sorted(str(p.relative_to(ROOT)) for p in inputs if not p.name.endswith("proofs.rs"))
    inventory = json.loads(subprocess.check_output(
        [str(out / "parser/debug/jzap-panic-inventory"), *files], cwd=ROOT, text=True))
    (out / "syntax.json").write_text(json.dumps(inventory, indent=2) + "\n")
    report["syntax_counts"] = dict(collections.Counter(r["kind"] for r in inventory))
    for name, command, result_path in [
        ("numeric-transitions-and-ledger", ["python3", "scripts/verify_panic_state.py", "--mutations"],
         ROOT / "target/verification/panic-state/summary.json"),
        ("vec-kernels", ["python3", "scripts/verify_source.py", "--group", "vec-cursors", "--mutations", "--output", str(ROOT / "target/verification/direct-source/vec-cursors-panic")],
         ROOT / "target/verification/direct-source/vec-cursors-panic/summary.json"),
        ("core-pointer-predicates", ["python3", "scripts/verify_pointer_checks.py", "--mutations"],
         ROOT / "target/verification/pointer-checks/summary.json"),
    ]:
        code = run(command, out / (name + ".log"), parser_env, timeout=600)
        model = dict(name=name, command=command, exit_code=code, passed=code == 0)
        if result_path.exists():
            model["report"] = json.loads(result_path.read_text())
        report["models"].append(model)
        if code:
            report["errors"].append(dict(step="model-proof", name=name, exit_code=code))
        save()
        print(name, "pass" if code == 0 else "unproved", flush=True)
    for name in ["negative_panic_is_detected", "negative_overflow_is_detected", "negative_initializer_panic_is_detected"]:
        log = out / (name + ".log")
        command = ["cargo", "kani", "--lib", "-Z", "loop-contracts", "--exact", "--harness", "panic_proofs::" + name]
        code = run(command, log, env)
        text = log.read_text()
        checks = properties(text)
        expected_failure = [p for p in checks if p["status"] == "FAILURE"
                            and p["id"].startswith("panic_proofs::" + name)
                            and ".assertion." in p["id"]
                            and p["location"].startswith("src/panic_proofs.rs:")]
        detected = (code not in (0, 124) and bool(expected_failure)
                    and "VERIFICATION:- FAILED" in text and "error[E" not in text
                    and "Complete - 0 successfully verified harnesses, 1 failures, 1 total." in text
                    and not re.search(r"out of memory|SIGKILL|Timed out|solver.*error", text, re.I))
        report["negative_controls"].append(dict(harness=name, detected=detected,
            exit_code=code, expected_failure=expected_failure))
        if not detected:
            report["errors"].append(dict(step="proof-negative-control", harness=name))
        save()
        print(name, "detected" if detected else "invalid", flush=True)
    for name, scope in HARNESSES.items():
        log = out / (name.replace("::", "-") + ".log")
        command = ["cargo", "kani", "--lib", "-Z", "loop-contracts", "--exact", "--harness", name]
        code = run(command, log, env)
        text = log.read_text()
        checks = properties(text)
        passed = (code == 0 and "Complete - 1 successfully verified harnesses, 0 failures, 1 total." in text
                  and not any(r["status"] in ("FAILURE", "ERROR", "UNDETERMINED") for r in checks))
        row = dict(harness=name, scope=scope, command=command, exit_code=code,
                   passed=passed, properties=checks, source_sha256=snapshot)
        report["proofs"].append(row)
        save()
        print(name, "pass" if passed else "unproved", flush=True)
    source_proofs = collections.defaultdict(set)
    for proof in report["proofs"]:
        if proof["passed"]:
            for prop in proof["properties"]:
                m = re.search(r"(src/[^:]+):(\d+):\d+", prop["location"])
                if m and prop["status"] == "SUCCESS":
                    source_proofs[m.groups()].add(proof["harness"])
    command = ["python3", "scripts/check_link_exits.py", "--profiles", args.profiles,
               "--output", str(out / "link")]
    link_started = time.time_ns()
    code = run(command, out / "link.log", env, timeout=600)
    report["link_process_exit"] = code
    if code not in (0, 1):
        report["errors"].append(dict(step="link-process", exit_code=code))
    path = out / "link/summary.json"
    if path.exists():
        if path.stat().st_mtime_ns < link_started:
            report["errors"].append(dict(step="stale-link-summary"))
        report["link"] = json.loads(path.read_text())
        expected = {(profile, None) for profile in args.profiles.split(",")}
        expected |= {(profile, control) for profile in args.profiles.split(",")
                     if profile in ("release", "release-lto") for control in
                     ("panic-control", "bounds-control", "oom-control", "abort-control")}
        observed = [(r["profile"], r["control"]) for r in report["link"]["results"]]
        if set(observed) != expected or len(observed) != len(expected):
            report["errors"].append(dict(step="incomplete-link-profile-matrix"))
        for result in report["link"]["results"]:
            if result["control"]:
                if result["status"] != "control-detected":
                    report["errors"].append(dict(step="negative-control", result=result))
                continue
            if result["status"] not in ("pass", "retained-exit-edge"):
                report["errors"].append(dict(step="link", result=result))
            calls = out / "link" / result["profile"] / "product/retained-panic-calls.json"
            if result["status"] == "retained-exit-edge" and not calls.exists():
                report["errors"].append(dict(step="missing-edge-inventory", profile=result["profile"]))
            if result["status"] == "retained-exit-edge" and result.get("diagnostic_link_exit") == 0 and calls.exists():
                for branch in json.loads(calls.read_text()):
                    branch.update(profile=result["profile"], disposition="unproved")
                    # Evidence is attached for review. Reached scalar assertions
                    # under a ZST harness do not by themselves prove a non-ZST
                    # caller's storage/ownership preconditions.
                    locations = [branch["location"], *branch.get("source_context", [])]
                    candidates = set()
                    for location in locations:
                        m = re.search(r"/(src/[^:]+):(\d+)", location)
                        if m:
                            candidates.update(source_proofs[m.groups()])
                    branch["related_implementation_proofs"] = sorted(candidates)
                    report["branches"].append(branch)
    else:
        report["errors"].append(dict(step="missing-link-summary", exit_code=code))
    if digest(inputs) != snapshot:
        report["errors"].append(dict(step="source-changed-during-proof"))
    if digest(proof_inputs) != verification_snapshot:
        report["errors"].append(dict(step="verification-input-changed-during-proof"))
    numeric_bindings = (report["models"][0].get("report", {}).get("bindings", {})
                        if report["models"][0]["passed"] else {})

    def contains_site(parent, site):
        return (parent["file"] == site["file"] and parent["context"] == site["context"]
                and parent["function"] == site["function"]
                and (parent["line"], parent["column"]) <= (site["line"], site["column"])
                and (site["end_line"], site["end_column"]) <= (parent["end_line"], parent["end_column"]))

    for site in inventory:
        # A local arithmetic property is evidence for this numeric expression,
        # not a certificate for ownership or every path into its containing API.
        related = set()
        for line in range(site["line"], site["end_line"] + 1):
            related.update(source_proofs.get((site["file"], str(line)), set()))
        numeric_models = [name for name, binding in numeric_bindings.items()
            if any(contains_site(parent, site) for parent in binding.get("sites", []))]
        site["related_counter_proofs"] = sorted(related)
        site["related_numeric_models"] = numeric_models
        site["numeric_evidence"] = bool(related or numeric_models)
        site["disposition"] = "caller-closure-unproved"
    (out / "source-obligations.json").write_text(json.dumps(inventory, indent=2) + "\n")
    arithmetic = [s for s in inventory if s["kind"] in ("add", "subtract", "multiply", "divide", "remainder", "shift", "negate")]
    report["arithmetic_inventory"] = dict(total=len(arithmetic),
        with_related_numeric_evidence=sum(s["numeric_evidence"] for s in arithmetic),
        # Related evidence is deliberately not promoted to a caller certificate.
        caller_closure_unproved=len(arithmetic))
    report["unproved_link_edges"] = len(report["branches"])
    report["open_source_obligations"] = len(inventory)
    report["complete"] = (not report["errors"] and not report["branches"]
                          and not report["open_source_obligations"]
                          and all(p["passed"] for p in report["proofs"]))
    save()
    print(f"{len(report['branches'])} retained edges require complete source/caller proof binding; "
          f"completion={'pass' if report['complete'] else 'unproved'}", flush=True)
    return int(not report["complete"])


if __name__ == "__main__":
    import sys
    if '--legacy-kani' in sys.argv:
        sys.argv.remove('--legacy-kani')
        raise SystemExit(legacy_main())
    from source_certificate import main
    raise SystemExit(main())
