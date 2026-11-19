#!/usr/bin/env python3
"""Report paired Arc workflow totals, uncertainty, controls and allocation costs."""
import argparse
import csv
import json
from pathlib import Path
import random

from report import export, validated_records


def paired_interval(pairs):
    """Seed-pair bootstrap of the weighted elapsed-time ratio, not request tails."""
    rng = random.Random(17291)
    ratios = []
    for _ in range(10000):
        sample = [pairs[rng.randrange(len(pairs))] for _ in pairs]
        ratios.append(sum(a for a, _ in sample) / sum(b for _, b in sample))
    ratios.sort()
    return ratios[249], ratios[9749]


def apply_repeat_gate(rows):
    controls = {(r["task"], r["variant"]): r for r in rows if r["label"] == "std_control"}
    for r in rows:
        r["repeat_required"] = abs(controls[r["task"], r["variant"]]["relative_percent"]) > 10
        if r["repeat_required"]:
            r["time_classification"] = "repeat-required-std-drift"


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--input", type=Path, required=True)
    p.add_argument("--memory", type=Path)
    args = p.parse_args()
    directory = args.input
    metadata, rows = export(directory)
    protocol = json.loads((directory / "protocol.json").read_text())
    completed = json.loads((directory / "completed.json").read_text())
    records = [json.loads(s) for s in (directory / "records.jsonl").read_text().splitlines()]
    accepted = validated_records(protocol, completed, records)
    specs = {s["task"]: s for s in protocol["scenarios"]}
    for row in rows:
        selected = [r for r in accepted if (r["task"], r["variant"], r["label"]) ==
                    (row["task"], row["variant"], row["label"])]
        bases = {r["seed"]: r for r in accepted if r["task"] == row["task"] and
                 r["variant"] == row["variant"] and r["label"] == "std"}
        lo, hi = paired_interval([(r["elapsed_seconds"], bases[r["seed"]]["elapsed_seconds"]) for r in selected])
        row.update(
            workers=specs[row["task"]]["workers"],
            payload_bytes=specs[row["task"]]["payload_bytes"],
            configured_reads=specs[row["task"]]["reads"],
            bootstrap_95_low_percent=(lo - 1) * 100,
            bootstrap_95_high_percent=(hi - 1) * 100,
            added_ns_per_transaction=row["mean_ns_per_operation"] - row["std_mean_ns_per_operation"],
            process_peak_rss_kib_min=min(r["peak_rss_kib"] for r in selected),
            process_peak_rss_kib_max=max(r["peak_rss_kib"] for r in selected),
            process_cpu_seconds=sum(r["process_cpu_seconds"] for r in selected),
            process_cpu_to_timed_wall=sum(r["process_cpu_seconds"] for r in selected) / row["total_seconds"],
            time_classification="above-15-percent" if lo > 1.15 else "within-15-percent" if hi <= 1.15 else "uncertain-at-15-percent",
        )
    apply_repeat_gate(rows)
    fields = [k for k in rows[0] if k != "per_seed"]
    with (directory / "scenario-means.csv").open("w") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, extrasaction="ignore", lineterminator="\n")
        writer.writeheader(); writer.writerows(rows)
    lines = ["# Arc workflow results", "", "All means are summed wall seconds / summed transactions. Multi-worker values express aggregate throughput, not individual request latency.", "",
             "| Workflow | Variant | Workers | std ns/transaction | Unmetered | Borrowed concurrent | Owning concurrent | Owning change | Paired bootstrap 95% | std repeat change |",
             "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: |"]
    for task, variant in dict.fromkeys((r["task"], r["variant"]) for r in rows):
        rs = {r["label"]:r for r in rows if (r["task"], r["variant"])==(task, variant)}
        r = rs["owned"]
        cells = [task,variant,str(r["workers"]),f"{r['std_mean_ns_per_operation']:.3f}"]
        cells += [f"{rs[k]['mean_ns_per_operation']:.3f}" for k in ["unlimited","borrowed","owned"]]
        cells += [f"{r['relative_percent']:+.1f}%", f"{r['bootstrap_95_low_percent']:+.1f}% to {r['bootstrap_95_high_percent']:+.1f}%", f"{rs['std_control']['relative_percent']:+.1f}%"]
        lines.append("| " + " | ".join(cells) + " |")
    if args.memory:
        memory = [json.loads(s) for s in args.memory.read_text().splitlines()]
        assert all(r["final_bytes"]==0 for r in memory)
        with (directory / "scenario-memory.csv").open("w") as stream:
            writer=csv.DictWriter(stream,fieldnames=list(memory[0]),lineterminator="\n")
            writer.writeheader();writer.writerows(memory)
        lines += ["", "| Working set | std requested peak | Owned requested peak | std usable peak | Owned usable peak | Usable change |", "| --- | ---: | ---: | ---: | ---: | ---: |"]
        for task in dict.fromkeys(r["scenario"] for r in memory):
            selected={r["label"]:r for r in memory if r["scenario"]==task}
            a,b=selected["std"],selected["owned"]
            lines.append(f"| {task} | {a['requested_peak']} | {b['requested_peak']} | {a['usable_peak']} | {b['usable_peak']} | {(b['usable_peak']/a['usable_peak']-1)*100:+.2f}% |")
    (directory / "scenario-report.md").write_text("\n".join(lines)+"\n")
    print(json.dumps(metadata,indent=2))
    print("Repeat required:",[(r["task"],r["variant"]) for r in rows if r["label"]=="std_control" and r["repeat_required"]])


if __name__ == "__main__":
    main()
