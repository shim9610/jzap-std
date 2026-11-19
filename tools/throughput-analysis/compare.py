#!/usr/bin/env python3
"""Compare >=1-second, equal-work, replayable collection workloads with std.

Build every executable and complete checks before invoking this driver. Raw
calibrations, short attempts and valid measurements remain under target/.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import resource
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
VEC_LABELS = ["std", "std_control", "unlimited", "limited", "metered", "limited_metered"]
SHARED_LABELS = ["std", "std_control", "unlimited", "local", "concurrent"]
ARC_APP_LABELS = ["std", "std_control", "unlimited", "borrowed", "owned"]

def labels_for(task):
    if task.startswith(("scratch_borrowed_", "scratch_get_", "scratch_range_", "scratch_copy_within_")):
        return ["std", "std_control", "unlimited"]
    if task.startswith("arc_app_"):
        return ARC_APP_LABELS
    return SHARED_LABELS if task.startswith(("rc_", "arc_", "btree_", "hash_map_", "hash_set_", "binary_heap_", "deque_", "scratch_", "scenario_")) else BOX_LABELS if task.startswith("box_") else VEC_LABELS

BOX_LABELS = ["std", "std_control", "unlimited", "metered"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_equal_work(rows, labels, variants, minimum):
    """Fail closed on missing cases, changed inputs/work, or a short interval."""
    expected = {(v, label) for v in variants for label in labels}
    actual = {(r["variant"], r["label"]) for r in rows}
    assert len(rows) == len(expected) and actual == expected, "missing/duplicate measurement"
    signatures = {(r["epochs"], r["operations"], r["elements"], r["corpus_digest"],
                   r["cases"], r["min_size"], r["max_size"]) for r in rows}
    assert len(signatures) == 1, "unequal input or work"
    if any("checksum" in r for r in rows):
        assert all("checksum" in r for r in rows) and len({r["checksum"] for r in rows}) == 1, "unequal workload result"
    assert min(r["elapsed_seconds"] for r in rows) >= minimum, "short interval"


def summarize(rows):
    valid = [r for r in rows if r["phase"] == "measurement" and r["accepted_attempt"]]
    groups = sorted({(r["profile"], r["task"], r["variant"]) for r in valid})
    result = []
    for profile, task, variant in groups:
        group = [r for r in valid if (r["profile"], r["task"], r["variant"]) == (profile, task, variant)]
        base = {r["seed"]: r for r in group if r["label"] == "std"}
        for label in sorted({r["label"] for r in group} - {"std"}):
            selected = sorted((r for r in group if r["label"] == label), key=lambda r: r["seed"])
            ratios = [r["elapsed_seconds"] / base[r["seed"]]["elapsed_seconds"] for r in selected]
            standard = sum(base[r["seed"]]["elapsed_seconds"] for r in selected)
            elapsed = sum(r["elapsed_seconds"] for r in selected)
            result.append(dict(profile=profile, task=task, variant=variant, label=label,
                               seeds=len(selected), operations=sum(r["operations"] for r in selected),
                               elements=sum(r["elements"] for r in selected),
                               std_total_seconds=standard, total_seconds=elapsed,
                               ratio=elapsed / standard, median_seed_ratio=statistics.median(ratios),
                               min_seed_ratio=min(ratios), max_seed_ratio=max(ratios),
                               minimum_recorded_seconds=min(r["elapsed_seconds"] for r in selected),
                               observation="above-15-percent" if elapsed / standard > 1.15 else "within-15-percent"))
    # This three-seed long-run diagnostic does not replace the earlier
    # independent-process bootstrap acceptance protocol with a point-estimate gate.
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", action="append", required=True, help="variant=/absolute/path")
    parser.add_argument("--seeds", default="617,6203,96011")
    parser.add_argument("--tasks", help="mixed tasks; default: all; empty string disables mixed cases")
    parser.add_argument("--small-tasks", default="clone,extract_all,pop")
    parser.add_argument("--small-variants", default="default,align64")
    parser.add_argument("--minimum-seconds", type=float, default=1.0)
    parser.add_argument("--target-seconds", type=float, default=1.2)
    parser.add_argument("--output", type=Path, default=ROOT / "target/verification/throughput-long")
    parser.add_argument("--cpus", help="comma-separated allowed CPUs; default: one CPU; use multiple for threaded scenarios")
    args = parser.parse_args()
    assert args.minimum_seconds >= 1.0 and args.target_seconds > args.minimum_seconds
    binaries = {k: Path(v).resolve() for k, v in (x.split("=", 1) for x in args.binary)}
    assert len(binaries) == len(args.binary)
    seeds = [int(x) for x in args.seeds.split(",")]
    assert len(seeds) == len(set(seeds)) and seeds
    available_cpus = os.sched_getaffinity(0)
    selected_cpus = {int(x) for x in args.cpus.split(",")} if args.cpus else {min(available_cpus)}
    assert selected_cpus and selected_cpus <= available_cpus, "unavailable CPU selection"
    cpu = min(selected_cpus)
    os.sched_setaffinity(0, selected_cpus)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    tasks = subprocess.check_output([str(next(iter(binaries.values()))), "tasks"], text=True).strip().split(",")
    requested = [x for x in args.tasks.split(",") if x] if args.tasks is not None else tasks
    assert set(requested) <= set(tasks)
    small_tasks = args.small_tasks.split(",") if args.small_tasks else []
    assert set(small_tasks) <= set(tasks)
    small_variants = [x for x in args.small_variants.split(",") if x in binaries]
    plan = [("mixed", task, list(binaries)) for task in requested]
    plan += [("small", task, small_variants) for task in small_tasks if small_variants]
    benchmark_info = json.loads(subprocess.check_output([str(next(iter(binaries.values()))), "describe"], text=True)) if tasks[0].startswith(("rc_", "arc_", "btree_", "hash_map_", "hash_set_", "binary_heap_", "deque_", "scratch_", "scenario_")) else {}
    source_files = sorted((ROOT / "src").rglob("*.rs")) + sorted((Path(__file__).parent / "src").rglob("*.rs"))
    source_files += [Path(__file__), Path(__file__).with_name("Cargo.toml"), Path(__file__).with_name("Cargo.lock")]
    source_sha = hashlib.sha256(b"".join(str(p.relative_to(ROOT)).encode() + b"\0" + p.read_bytes() for p in source_files)).hexdigest()
    protocol = dict(cpu=cpu, seeds=seeds, minimum_seconds=args.minimum_seconds, target_seconds=args.target_seconds,
                    plan=[list(x) for x in plan], binaries={k: dict(path=str(p), sha256=digest(p)) for k, p in binaries.items()},
                    source_sha256=source_sha, rustc=subprocess.check_output(["rustc", "-Vv"], text=True),
                    operation_unit=benchmark_info.get("operation_unit", "complete lifecycle except reserved reuse and Box clone_from"),
                    timing="one Instant around all epochs; generation/setup and two warmup epochs excluded",
                    comparisons="same corpus, order, epochs and operations across every variant/label in a seed group",
                    summary="sum of elapsed seconds for matched seed groups; no conversion to time per operation",
                    acceptance=f"observational {len(seeds)}-seed ratios; previous bootstrap gate remains separate",
                    inputs=benchmark_info.get("inputs", "64 pre-generated shuffled cases, log-uniform exponent plus zero/one/maximum; small profile uniform 0..64"))
    if args.cpus:
        protocol["cpus"] = sorted(selected_cpus)
    if "scenarios" in benchmark_info:
        protocol["scenarios"] = benchmark_info["scenarios"]
        protocol["timing"] = "one interval across fixed worker transactions; worker creation, readiness and warmup excluded; start/completion barriers included"
        protocol["acceptance"] = f"observational {len(seeds)}-seed workload ratios; repeated-std mean drift above 10 percent requires a repeat"
    protocol_path = out / "protocol.json"
    if protocol_path.exists():
        assert json.loads(protocol_path.read_text()) == protocol, "resume protocol drift"
    else:
        protocol_path.write_text(json.dumps(protocol, indent=2) + "\n")
    journal = out / "records.jsonl"
    rows = [json.loads(line) for line in journal.read_text().splitlines()] if journal.exists() else []
    # A completed group is committed separately from its rows, after every
    # integrity/duration check passes. Interrupted/incomplete attempts stay raw.
    commits = out / "completed.json"
    completed = json.loads(commits.read_text()) if commits.exists() else []
    committed = {(x["profile"], x["task"], x["seed"]) for x in completed}
    started = time.monotonic()

    def save(row):
        rows.append(row)
        with journal.open("a") as stream:
            stream.write(json.dumps(row, separators=(",", ":")) + "\n")

    def run(variant, label, task, seed, profile, epochs, phase, attempt):
        before_cpu = resource.getrusage(resource.RUSAGE_CHILDREN)
        before_wall = time.monotonic()
        result = json.loads(subprocess.check_output(
            [str(binaries[variant]), "std" if label == "std_control" else label, task, str(seed), str(epochs), profile], text=True))
        after_cpu = resource.getrusage(resource.RUSAGE_CHILDREN)
        result["process_wall_seconds"] = time.monotonic() - before_wall
        result["process_cpu_seconds"] = after_cpu.ru_utime + after_cpu.ru_stime - before_cpu.ru_utime - before_cpu.ru_stime
        if task.startswith("arc_app_"):
            # The child reads its own Linux VmHWM outside timing; cumulative
            # RUSAGE_CHILDREN would retain earlier processes' RSS peaks.
            assert result.get("peak_rss_kib",0) > 0
        assert result["epochs"] == epochs and result["cases"] == 64 and result["elapsed_seconds"] > 0
        result.update(variant=variant, label=label, task=task, seed=seed, profile=profile,
                      phase=phase, attempt=attempt)
        return result

    def write_summary():
        accepted_keys = {(x["profile"], x["task"], x["seed"], x["attempt"]) for x in completed}
        for row in rows:
            row["accepted_attempt"] = row["phase"] == "measurement" and (
                row["profile"], row["task"], row["seed"], row["attempt"]) in accepted_keys
        (out / "summary.json").write_text(json.dumps(summarize(rows), indent=2) + "\n")

    for profile, task, variants in plan:
        labels = labels_for(task)
        for seed_index, seed in enumerate(seeds):
            if (profile, task, seed) in committed:
                continue
            assert all(digest(binaries[v]) == protocol["binaries"][v]["sha256"] for v in variants), "executable changed"
            print(f"CALIBRATE {profile} {task} seed={seed}", flush=True)
            epochs = 1
            for variant in variants:
                for label in labels:
                    if label == "std_control":
                        continue
                    count = 4
                    while True:
                        row = run(variant, label, task, seed, profile, count, "calibration", 0)
                        save(row)
                        if row["elapsed_seconds"] >= 0.05:
                            break
                        factor = min(1024, max(2, math.ceil(0.055 / row["elapsed_seconds"])))
                        count *= factor
                    epochs = max(epochs, math.ceil(count * args.target_seconds / row["elapsed_seconds"]))
            attempt = max((r["attempt"] for r in rows if r["profile"] == profile and r["task"] == task and r["seed"] == seed), default=0)
            while True:
                attempt += 1
                measurements = []
                variant_order = variants[seed_index % len(variants):] + variants[:seed_index % len(variants)]
                for variant in variant_order:
                    shift = (seed_index + variants.index(variant)) % len(labels)
                    order = labels[shift:] + labels[:shift]
                    if seed_index % 2:
                        order = order[::-1]
                    for label in order:
                        row = run(variant, label, task, seed, profile, epochs, "measurement", attempt)
                        save(row)
                        measurements.append(row)
                        print(f"TIME {profile} {task} seed={seed} {variant}/{label} "
                              f"{row['operations']} calls {row['elapsed_seconds']:.6f} s", flush=True)
                # Always check work even if one record was too short.
                validate_equal_work(measurements, labels, variants, 0)
                shortest = min(r["elapsed_seconds"] for r in measurements)
                if shortest >= args.minimum_seconds:
                    validate_equal_work(measurements, labels, variants, args.minimum_seconds)
                    completed.append(dict(profile=profile, task=task, seed=seed, attempt=attempt, epochs=epochs))
                    commits.write_text(json.dumps(completed, indent=2) + "\n")
                    write_summary()
                    print(f"COMPLETE {profile} {task} seed={seed}; minimum={shortest:.6f} s", flush=True)
                    break
                epochs = math.ceil(epochs * max(1.3, args.target_seconds / shortest * 1.1))
                print(f"SHORT attempt retained; rerunning the whole equal-work group at {epochs} epochs", flush=True)
            (out / "progress.json").write_text(json.dumps(dict(
                completed_groups=len(completed), planned_groups=len(plan) * len(seeds),
                elapsed_wall_seconds=time.monotonic() - started, latest=[profile, task, seed]), indent=2) + "\n")
    assert all(digest(binaries[v]) == protocol["binaries"][v]["sha256"] for v in binaries)
    write_summary()
    print(f"DONE {len(completed)} seed groups; raw and summary under {out}", flush=True)


if __name__ == "__main__":
    main()
