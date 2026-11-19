#!/usr/bin/env python3
"""Compose calls to the original executable edit controllers.

The maintained callbacks retain numeric span/reservation obligations. Product
loop bodies are neither extracted nor translated. Remaining outer owner models
and actual-pointer/provenance checks retain their separate scopes.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target/verification/edit-loops"


def block(text, marker):
    opening = text.index("{", text.index(marker)); end = opening + 1; depth = 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}"); end += 1
    return text[opening + 1:end - 1]


def composition():
    return (ROOT / "tools/memory-proof/edit-loops.verus.rs").read_text()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mutations", action="store_true")
    parser.add_argument("--reuse-generated", action="store_true", help="Development only: reuse the already generated prerequisite model")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True); (OUT / "summary.json").unlink(missing_ok=True)
    env = dict(os.environ, PATH="/workspace/.devtools/verus/verus-x86-linux:" + os.environ["PATH"])
    if not args.reuse_generated:
        with (OUT / "prerequisites.log").open("w") as log:
            subprocess.run(["python3", "scripts/verify_wrapping_memory.py", "--mutations"], cwd=ROOT, env=env, stdout=log, stderr=log, check=True)
    base = (ROOT / "target/verification/wrapping-memory/memory.generated.rs").read_text()
    base = re.sub(r"fn main\(\)\s*\{\}", "", base)
    source = (ROOT / "src/vec/edit.rs").read_text()
    verus = env.get("JZAP_VERUS", "/workspace/.devtools/verus/verus-x86-linux/verus")
    from verus_composition import run as compose
    results = []

    def verify(name, text):
        generated = base + composition() + "\nfn main(){}\n"
        path = OUT / (name + ".generated.rs"); path.write_text(generated)
        command, r = compose(verus, ROOT, path, "jzap_edit_loops", env)
        log = r.stdout + r.stderr; (OUT / (name + ".log")).write_text(log)
        return dict(name=name, command=command, composition_input=str(path), exit_code=r.returncode, verification=re.findall(r"verification results:: (\d+) verified, (\d+) errors", log)), log

    row, log = verify("edit-loops", source); results.append(row)
    assert row["exit_code"] == 0, log[-12000:]
    assert (ROOT / "src/vec/edit.rs").read_text() == source
    (OUT / "summary.json").write_text(json.dumps(dict(status="pass", source_sha256=hashlib.sha256(source.encode()).hexdigest(), results=results, reused_prerequisites=args.reuse_generated,
        scope="Original executable retain/dedup/extraction/staging control calls and maintained span/reservation callbacks for arbitrary replies; no product loop-body translation. Outer owner models and actual reference/heap semantics remain separate."), indent=2) + "\n")
    print("Original edit controllers and maintained span/reservation callbacks verified.")


if __name__ == "__main__": main()
