#!/usr/bin/env python3
"""Check all unsafe Send/Sync implementations and weakened-bound mutants."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target/verification/thread-bounds"
OWNERS = {
    "arc": "jzap::sync::Arc",
    "arc_weak": "jzap::sync::Weak",
    "rc": "jzap::rc::Rc",
    "rc_weak": "jzap::rc::Weak",
    "box": "jzap::boxed::Box",
    "vec": "jzap::vec::Vec",
    "deque": "jzap::deque::VecDeque",
    "iter": "jzap::boxed::IntoIter",
    "string_drain": "jzap::string::Drain",
}


def build(source, work):
    target = work / "libjzap.rlib"
    command = ["rustc", "--edition=2024", "--crate-type=rlib", "--crate-name=jzap",
               '--cfg=feature="alloc"', str(source / "src/lib.rs"), "-o", str(target)]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    (work / "build.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, result.stderr
    return target


def check(library, work, name, owner, trait, element, policy):
    type_name = (f"{OWNERS[owner]}<'static,{policy}>" if owner == "string_drain" else f"{OWNERS[owner]}<{element},{policy}>")
    text = f"fn require<T: {trait}>() {{}}\nfn test() {{ require::<{type_name}>(); }}\n"
    source = work / (name + ".rs"); source.write_text(text)
    command = ["rustc", "--edition=2024", "--crate-type=lib", "--emit=metadata",
               "--error-format=json", "--extern", "jzap=" + str(library),
               str(source), "-o", str(work / (name + ".rmeta"))]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    (work / (name + ".log")).write_text(result.stdout + result.stderr)
    errors = [json.loads(line) for line in result.stderr.splitlines() if line.startswith("{")]
    errors = [e for e in errors if e.get("level") == "error" and e.get("code")]
    return dict(exit_code=result.returncode,
                errors=[dict(code=e["code"]["code"], message=e["message"]) for e in errors],
                source=text, command=command)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mutations", action="store_true")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "summary.json").unlink(missing_ok=True)
    files = sorted((ROOT / "src").rglob("*.rs")) + [Path(__file__)]
    fingerprint = lambda: hashlib.sha256(b"".join(str(p.relative_to(ROOT)).encode() + b"\0" + p.read_bytes() for p in files)).hexdigest()
    before = fingerprint()
    library = build(ROOT, OUT)
    cases = []
    for owner in OWNERS:
        for trait in ["Send", "Sync"]:
            for label, element, policy, accepted in [
                ("allowed", "u64", "jzap::budget::Unmetered", owner not in ("rc", "rc_weak")),
                ("concurrent", "u64", "jzap::budget::ConcurrentBudget<'static>", owner not in ("rc", "rc_weak")),
                ("owned_local", "u64", "jzap::budget::OwnedBudget", False),
                ("owned_concurrent", "u64", "jzap::budget::OwnedConcurrentBudget", owner not in ("rc", "rc_weak")),
                ("rc", "std::rc::Rc<u64>", "jzap::budget::Unmetered", False),
                ("budget", "u64", "jzap::budget::Budget<'static>", False),
            ] + ([("cell", "std::cell::Cell<u64>", "jzap::budget::Unmetered", trait == "Send" and owner not in ("arc", "arc_weak", "rc", "rc_weak"))]):
                if owner == "string_drain" and label in ("rc", "cell"):
                    continue
                name = f"{owner}_{trait.lower()}_{label}"
                row = check(library, OUT, name, owner, trait, element, policy)
                if accepted:
                    assert row["exit_code"] == 0, row
                else:
                    assert row["exit_code"] and row["errors"] and all(e["code"] == "E0277" for e in row["errors"]), row
                    assert any("cannot be sent" in e["message"] or "cannot be shared" in e["message"] for e in row["errors"]), row
                cases.append(dict(name=name, accepted=accepted, passed=True, **row))
    mutations = []
    if args.mutations:
        from arithmetic_projection import replace_once
        for owner, file, generic in [
            ("box", "src/boxed.rs", "T: ?Sized + TRAIT, P: AllocationPolicy + TRAIT"),
            ("deque", "src/deque.rs", "T: TRAIT, P: AllocationPolicy + TRAIT, L: LengthPolicy + TRAIT"),
            ("vec", "src/vec.rs", "T: TRAIT, P: AllocationPolicy + TRAIT, L: LengthPolicy + TRAIT"),
            ("iter", "src/boxed/slice.rs", "T: TRAIT, P: AllocationPolicy + TRAIT"),
            ("string_drain", "src/string/edit.rs", "P: AllocationPolicy + TRAIT, L: LengthPolicy + TRAIT"),
        ]:
            for trait in ["Send", "Sync"]:
                name = owner + "_" + trait.lower()
                work = OUT / ("mutation-" + name); work.mkdir(exist_ok=True)
                with tempfile.TemporaryDirectory(prefix="jzap-thread-") as tmp:
                    source = Path(tmp); shutil.copytree(ROOT / "src", source / "src")
                    path = source / file
                    old = "unsafe impl<" + generic.replace("TRAIT", trait) + ">"
                    new = old.replace("T: ?Sized + " + trait, "T: ?Sized").replace("T: " + trait, "T")
                    if owner == "string_drain":
                        new = old.replace("P: AllocationPolicy + " + trait, "P: AllocationPolicy")
                    path.write_text(replace_once(path.read_text(), old, new))
                    mutant = build(source, work)
                    probe = check(mutant, work, "rc_should_be_rejected", owner, trait,
                                  "std::rc::Rc<u64>", "jzap::budget::Budget<'static>" if owner == "string_drain" else "jzap::budget::Unmetered")
                    assert probe["exit_code"] == 0, probe
                    mutations.append(dict(name=name, missing_rejection_detected=True))
    if args.mutations:
        from arithmetic_projection import replace_once
        for owner,kind in [("arc","Strong"),("arc_weak","Weak")]:
            for trait in ["Send","Sync"]:
                name=owner+"_"+trait.lower()
                work=OUT/("mutation-"+name);work.mkdir(exist_ok=True)
                with tempfile.TemporaryDirectory(prefix="jzap-atomic-thread-")as temporary:
                    source=Path(temporary);shutil.copytree(ROOT/"src",source/"src")
                    path=source/"src/shared.rs"
                    old="unsafe impl<T: ?Sized + Send + Sync, P: SharedPolicy + Send + Sync> "+trait+" for "+kind+"<T, count::AtomicCount, P>"
                    new=old.replace("T: ?Sized + Send + Sync","T: ?Sized")
                    path.write_text(replace_once(path.read_text(),old,new))
                    mutant=build(source,work)
                    probe=check(mutant,work,"rc_should_be_rejected",owner,trait,"std::rc::Rc<u64>","jzap::budget::Unmetered")
                    assert probe["exit_code"]==0,probe
                    mutations.append(dict(name=name,missing_rejection_detected=True))
    assert fingerprint() == before
    report = dict(status="pass", source_sha256=before,
                  rustc=subprocess.check_output(["rustc", "-Vv"], text=True),
                  scope="Default alloc feature; compiler trait bounds, not a concurrency-algorithm proof.",
                  cases=cases, mutations=mutations)
    (OUT / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"{len(cases)} Send/Sync cases pass ({sum(c['accepted'] for c in cases)} positive); {len(mutations)} weakened implementations detected.")


if __name__ == "__main__":
    main()
