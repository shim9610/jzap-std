#!/usr/bin/env python3
"""Derive operation-weighted means from validated long-interval recordings.

Sampling stays in compare.py. This report step does not change frozen protocols,
raw observations or which completed attempts are selected.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import statistics
import sys
import math

from compare import BOX_LABELS, VEC_LABELS, labels_for, merge_offsets, summarize, validate_equal_work


def validated_records(protocol, completed, records):
    assert protocol["minimum_seconds"] >= 1, "subsecond measurement protocol"
    expected = {(profile, task, seed) for profile, task, _ in protocol["plan"]
                for seed in protocol["seeds"]}
    keys = [(c["profile"], c["task"], c["seed"]) for c in completed]
    assert len(keys) == len(set(keys)) and set(keys) == expected, "incomplete/duplicate completed groups"
    selected = {(c["profile"], c["task"], c["seed"], c["attempt"]) for c in completed}
    accepted = merge_offsets([r for r in records if r["phase"] == "measurement" and
                              (r["profile"], r["task"], r["seed"], r["attempt"]) in selected])
    # Every stack-offset process must itself meet the interval minimum.
    assert all(r.get("minimum_offset_seconds", r["elapsed_seconds"]) >= protocol["minimum_seconds"]
               for r in accepted), "short stack-offset interval"
    for profile, task, variants in protocol["plan"]:
        labels = labels_for(task)
        for seed in protocol["seeds"]:
            group = [r for r in accepted if (r["profile"], r["task"], r["seed"]) == (profile, task, seed)]
            validate_equal_work(group, labels, variants, protocol["minimum_seconds"])
    return accepted


def operation_means(records):
    """All operations have equal weight; preserve seed-level variation separately."""
    summaries = summarize([{**r, "accepted_attempt": True} for r in records])
    groups = {}
    for r in records:
        groups.setdefault((r["profile"], r["task"], r["variant"], r["label"]), []).append(r)
    for row in summaries:
        key = row["profile"], row["task"], row["variant"]
        selected = sorted(groups[(*key, row["label"])], key=lambda r: r["seed"])
        base = {r["seed"]: r for r in groups[(*key, "std")]}
        assert len(selected) == len({r["seed"] for r in selected})
        assert all(r["operations"] == base[r["seed"]]["operations"] > 0 for r in selected), "unequal call count"
        ns = [r["elapsed_seconds"] * 1e9 / r["operations"] for r in selected]
        std_ns = [base[r["seed"]]["elapsed_seconds"] * 1e9 / r["operations"] for r in selected]
        row.update(
            mean_recording_seconds=row["total_seconds"] / row["seeds"],
            std_mean_recording_seconds=row["std_total_seconds"] / row["seeds"],
            mean_ns_per_operation=row["total_seconds"] * 1e9 / row["operations"],
            std_mean_ns_per_operation=row["std_total_seconds"] * 1e9 / row["operations"],
            relative_percent=(row["ratio"] - 1) * 100,
            seed_mean_ns_min=min(ns), seed_mean_ns_max=max(ns),
            seed_mean_ns_arithmetic=statistics.mean(ns),
            seed_mean_ns_sample_stdev=statistics.stdev(ns) if len(ns) > 1 else 0,
            std_seed_mean_ns_min=min(std_ns), std_seed_mean_ns_max=max(std_ns),
            per_seed=[dict(seed=r["seed"], operations=r["operations"],
                           seconds=r["elapsed_seconds"], std_seconds=base[r["seed"]]["elapsed_seconds"],
                           mean_ns_per_operation=a, std_mean_ns_per_operation=b)
                      for r, a, b in zip(selected, ns, std_ns)],
        )
    return summaries


def markdown_table(rows, tasks, profile="mixed", variant="default", family="Vec"):
    labels = ["unlimited", "borrowed", "owned"] if family == "ArcScenario" else ["unlimited", "local", "concurrent"] if family in ("Rc", "Arc", "BTreeMap", "BTreeSet", "HashMap", "HashSet", "BinaryHeap", "VecDeque", "ScratchBuf", "Scenario") else ["unlimited", "limited", "metered", "limited_metered"] if family in ("Vec", "String") else ["unlimited"] if family == "CheckedSlice" else ["unlimited", "metered"]
    names = {"unlimited": "Unmetered", "limited": "Length limit", "metered": "Budgeted", "limited_metered": "Limit + budget", "local":"Local budget", "concurrent":"Concurrent budget", "borrowed":"Borrowed concurrent", "owned":"Owning concurrent"}
    changes = ["unlimited", "owned"] if family == "ArcScenario" else ["unlimited", "concurrent"] if family in ("Rc", "Arc", "BTreeMap", "BTreeSet", "HashMap", "HashSet", "BinaryHeap", "VecDeque", "ScratchBuf", "Scenario") else ["unlimited"] if family == "CheckedSlice" else ["unlimited", "metered"]
    columns = ["Operation", "Calls", "std ns/op"] + [names[label] + " ns/op" for label in labels] + [names[label]+" change" for label in changes]
    lines = ["| " + " | ".join(columns) + " |", "| --- |" + " ---: |" * (len(columns) - 1)]
    for task in tasks:
        selected = {r["label"]: r for r in rows if (r["profile"], r["variant"], r["task"]) == (profile, variant, task)}
        if not selected:
            continue
        base = selected["unlimited"]
        cells = [task, f"{base['operations']:,}", f"{base['std_mean_ns_per_operation']:,.3f}"]
        cells += [f"{selected[label]['mean_ns_per_operation']:,.3f}" for label in labels]
        cells += [f"{selected[label]['relative_percent']:+.1f}%" for label in changes]
        lines.append("| " + " | ".join(cells) + " |")
    return "\n".join(lines) + "\n"


# One row per matched seed group keeps published means independently recomputable.
COMPACT_KEYS=['profile','task','variant','seed','epochs','operations','elements',
              'corpus_digest','checksum','cases','min_size','max_size']
def compact_records(records,path):
    groups={}
    for r in records:groups.setdefault((r['profile'],r['task'],r['variant'],r['seed']),[]).append(r)
    labels=sorted({r['label']for r in records})
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('w',newline='')as stream:
        writer=csv.DictWriter(stream,fieldnames=COMPACT_KEYS+[label+'_seconds'for label in labels],lineterminator="\n");writer.writeheader()
        for key,group in sorted(groups.items()):
            validate_equal_work(group,labels_for(key[1]),[key[2]],1.0)
            row={k:group[0].get(k,'')for k in COMPACT_KEYS}
            row.update({r['label']+'_seconds':r['elapsed_seconds']for r in group});writer.writerow(row)
def read_compact(path):
    records=[];seen=set()
    with path.open(newline='')as stream:
        for row in csv.DictReader(stream):
            common={k:(row[k]if k in ('profile','task','variant')else int(row[k]))for k in COMPACT_KEYS if row[k]!=''}
            key=tuple(common[k]for k in ('profile','task','variant','seed'));assert key not in seen,'duplicate compact seed group';seen.add(key)
            labels=labels_for(common['task']);group=[]
            for label in labels:
                seconds=float(row[label+'_seconds']);assert math.isfinite(seconds)and seconds>=1,'invalid/short compact interval'
                group.append(dict(common,label=label,elapsed_seconds=seconds,phase='measurement',accepted_attempt=True))
            validate_equal_work(group,labels,[common['variant']],1.0);records.extend(group)
    assert records,'empty compact dataset'
    return records

def export(input_dir, compact=None):
    protocol_path = input_dir / "protocol.json"
    protocol = json.loads(protocol_path.read_text())
    completed = json.loads((input_dir / "completed.json").read_text())
    records = [json.loads(line) for line in (input_dir / "records.jsonl").read_text().splitlines()]
    accepted = validated_records(protocol, completed, records)
    if compact is not None:compact_records(accepted,compact)
    rows = operation_means(accepted)
    metadata = dict(
        aggregation="sum(valid elapsed seconds) / sum(valid operation counts)",
        duration_rule="every selected interval >= 1 second; repeated seed groups",
        valid_records=len(accepted), minimum_seconds=min(r["elapsed_seconds"] for r in accepted),
        protocol_sha256=hashlib.sha256(protocol_path.read_bytes()).hexdigest(),
        journal_sha256=hashlib.sha256((input_dir / "records.jsonl").read_bytes()).hexdigest(),
        reporter_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        note="Means describe complete workload calls, not individual elements. Seed spread includes corpus variation; no confidence-interval acceptance is inferred.",
    )
    (input_dir / "per-operation.json").write_text(json.dumps(dict(metadata=metadata, rows=rows), indent=2) + "\n")
    with (input_dir / "per-operation.csv").open("w", newline="") as stream:
        keys = [key for key in rows[0] if key != "per_seed"]
        writer = csv.DictWriter(stream, fieldnames=keys, extrasaction="ignore", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    sections = ["# Per-operation means from long-interval measurements\n",
                "Mean time per operation = sum of valid measured seconds / sum of executed calls.\n",
                "Every selected interval lasts at least one second. Calibration and uncommitted attempts are excluded.\n"]
    for profile in dict.fromkeys(entry[0] for entry in protocol["plan"]):
        for variant in protocol["binaries"]:
            for family in ["Vec", "Box", "String", "Rc", "Arc", "ArcScenario", "BTreeMap", "BTreeSet", "HashMap", "HashSet", "BinaryHeap", "VecDeque", "ScratchBuf", "CheckedSlice", "Scenario"]:
                tasks = [task for prof, task, variants in protocol["plan"]
                         if prof == profile and variant in variants and ("Scenario" if task.startswith("scenario_") else "ArcScenario" if task.startswith("arc_app_") else "Box" if task.startswith("box_") else "String" if task.startswith("string_") else "Rc" if task.startswith("rc_") else "Arc" if task.startswith("arc_") else "HashMap" if task.startswith("hash_map_") else "HashSet" if task.startswith("hash_set_") else "CheckedSlice" if task.startswith(("scratch_borrowed_", "scratch_get_", "scratch_range_", "scratch_copy_within_")) else "ScratchBuf" if task.startswith("scratch_") else "VecDeque" if task.startswith("deque_") else "BinaryHeap" if task.startswith("binary_heap_") else "BTreeSet" if task.startswith("btree_set_") else "BTreeMap" if task.startswith("btree_") else "Vec") == family]
                if tasks:
                    sections += [f"\n## {profile} / {variant} / {family}\n", markdown_table(rows, tasks, profile, variant, family)]
    (input_dir / "per-operation.md").write_text("\n".join(sections))
    return metadata, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=Path(__file__).resolve().parents[2] / "target/verification/throughput-long")
    parser.add_argument('--compact',type=Path,help='Also export one independently replayable CSV row per matched seed group')
    parser.add_argument('--from-compact',type=Path,help='Recompute published means from the compact CSV without target logs')
    args = parser.parse_args()
    if args.from_compact:
        rows=operation_means(read_compact(args.from_compact))
        fields=['profile','task','variant','label','seeds','operations','std_total_seconds','total_seconds','ratio','std_mean_ns_per_operation','mean_ns_per_operation']
        writer=csv.DictWriter(sys.stdout,fieldnames=fields,extrasaction='ignore',lineterminator="\n");writer.writeheader();writer.writerows(rows)
        return
    metadata, rows = export(args.input.resolve(),args.compact)
    print(f"{metadata['valid_records']} valid >=1-second recordings; {len(rows)} operation means exported to {args.input}")


if __name__ == "__main__":
    main()
