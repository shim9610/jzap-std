#!/usr/bin/env python3
"""Summarize a paired A/B of two builds from long-interval benchmark CSVs.

Each CSV row holds a `seed`, identifying columns, `seconds`, `operations` and
`checksum` (the format of the `bplus*` benchmark executables). Pass every
file of each side:

    python3 tools/perf-audit/paired_ab.py --base base-seed*.csv --new new-seed*.csv \\
        [--controls complete,std] [--min-interval 1.0]

For each row key it prints the call-weighted ns per operation of both sides,
their ratio (new / base), the range of the per-seed ratios and the shortest
interval. It fails if a checksum or operation count differs between the
sides for the same seed and key, or if an interval is shorter than the
minimum. Methods named in `--controls` should run unchanged code; their
ratios show how far the two executables differ without a code change, so a
smaller difference elsewhere is not evidence of a change.
"""
import argparse
import collections
import csv
import sys

VALUES = ("seconds", "operations", "checksum")


def load(paths):
    rows = {}
    for path in paths:
        with open(path) as source:
            for row in csv.DictReader(source):
                key = tuple((name, value) for name, value in row.items() if name not in VALUES + ("seed",))
                seed = row["seed"]
                if (key, seed) in rows:
                    sys.exit(f"duplicate row for seed {seed} and {dict(key)} in {path}")
                rows[(key, seed)] = (float(row["seconds"]), int(row["operations"]), row["checksum"])
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", nargs="+", required=True)
    parser.add_argument("--new", nargs="+", required=True)
    parser.add_argument("--controls", default="", help="comma-separated implementation names running unchanged code")
    parser.add_argument("--min-interval", type=float, default=1.0)
    args = parser.parse_args()
    base, new = load(args.base), load(args.new)
    if set(base) != set(new):
        sys.exit("the two sides do not cover the same keys and seeds")
    controls = set(filter(None, args.controls.split(",")))
    totals = collections.defaultdict(lambda: [0.0, 0, 0.0, 0])
    per_seed = collections.defaultdict(list)
    shortest = collections.defaultdict(lambda: float("inf"))
    for (key, seed), (seconds, operations, checksum) in base.items():
        other = new[(key, seed)]
        if (operations, checksum) != other[1:]:
            sys.exit(f"operations or checksum differ for seed {seed} and {dict(key)}")
        if min(seconds, other[0]) < args.min_interval:
            sys.exit(f"interval shorter than {args.min_interval} s for seed {seed} and {dict(key)}")
        total = totals[key]
        total[0] += seconds
        total[1] += operations
        total[2] += other[0]
        total[3] += operations
        per_seed[key].append((other[0] / operations) / (seconds / operations))
        shortest[key] = min(shortest[key], seconds, other[0])
    print("| Case | base ns | new ns | new/base | per-seed range | shortest s |")
    print("| --- | ---: | ---: | ---: | --- | ---: |")
    for key in sorted(totals):
        b_seconds, b_ops, n_seconds, n_ops = totals[key]
        label = " ".join(value for _, value in key)
        if dict(key).get("implementation") in controls:
            label += " (control)"
        ratios = per_seed[key]
        print(f"| {label} | {b_seconds / b_ops * 1e9:.1f} | {n_seconds / n_ops * 1e9:.1f} | "
              f"{(n_seconds / n_ops) / (b_seconds / b_ops):.3f} | {min(ratios):.3f}–{max(ratios):.3f} | "
              f"{shortest[key]:.2f} |")


if __name__ == "__main__":
    main()
