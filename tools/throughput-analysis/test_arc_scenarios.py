#!/usr/bin/env python3
"""Check all application's seeded results against std before long timing."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.binary.resolve())
    tasks = subprocess.check_output([binary, "tasks"], text=True).strip().split(",")
    assert len(tasks) == 19 and len(set(tasks)) == 19
    checks = 0
    for seed in [617, 6203]:
        for task in tasks:
            results = []
            for label in ["std", "unlimited", "borrowed", "owned"]:
                result = json.loads(subprocess.check_output([binary, label, task, str(seed), "3", "mixed"], text=True))
                assert result["epochs"] == 3 and result["cases"] == 64
                assert result["operations"] == 3 * 64 * result["workers"]
                assert result["peak_rss_kib"] > 0
                results.append((result["checksum"], result["corpus_digest"], result["operations"]))
                checks += 1
            assert len(set(results)) == 1, (seed, task, results)
    print(f"{checks} workflow processes agree with std, including concurrent locked totals and forced COW.")


if __name__ == "__main__":
    main()
