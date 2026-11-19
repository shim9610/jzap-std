#!/usr/bin/env python3
"""Fixed post-codegen protocol; timings use the default allocator without hooks."""
import argparse, hashlib, json, os, pathlib, random, statistics, subprocess

root = pathlib.Path(__file__).resolve().parents[1]
exe = root / 'tools/box-analysis/target/release/jzap-box-analysis'
parser = argparse.ArgumentParser()
parser.add_argument('--barriers', choices=['phase', 'item'], default='phase')
parser.add_argument('--output', type=pathlib.Path, default=root/'target/verification/box-analysis')
args = parser.parse_args()
out = args.output
out.mkdir(parents=True, exist_ok=True)
cpu = min(os.sched_getaffinity(0))
os.sched_setaffinity(0, {cpu})
metadata = dict(barriers=args.barriers, cpu=cpu, processes=15, warmup_cycles=2, retained_cycles=30,
                binary_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),
                sizes=json.loads(subprocess.check_output([str(exe), 'sizes'], text=True)))
(out / 'timing-metadata.json').write_text(json.dumps(metadata, indent=2))
rows = []
for n in [10_000, 100_000]:
    for trial in range(15):
        order = ['std', 'std_control', 'std_handle16', 'varlit', 'unmetered', 'metered']
        shift = trial % len(order)
        order = order[shift:] + order[:shift]
        if trial % 2: order.reverse()
        for label in order:
            row = json.loads(subprocess.check_output(
                [str(exe), 'std' if label == 'std_control' else label, str(n), args.barriers], text=True))
            row.update(label=label, trial=trial)
            rows.append(row)
        # Preserve every completed trial even if a later process fails.
        (out / 'timings.json').write_text(json.dumps(rows, indent=2))
    print(f'completed n={n}', flush=True)
rng = random.Random(617)
summary = []
for n in [10_000, 100_000]:
    for label in ['std_control', 'std_handle16', 'varlit', 'unmetered', 'metered']:
        for metric, key in enumerate(['create', 'clone', 'drop']):
            def values(which):
                return [statistics.median(s[metric] for s in r['samples_ns'])
                        for r in rows if r['n'] == n and r['label'] == which]
            a, b = values(label), values('std')
            ratios = sorted(statistics.median(rng.choices(a, k=15)) /
                            statistics.median(rng.choices(b, k=15)) for _ in range(5000))
            low, high = ratios[125], ratios[4874]
            summary.append(dict(n=n, label=label, metric=key,
                ratio=statistics.median(a)/statistics.median(b), low95=low, high95=high,
                status='pass' if high <= 1.15 else ('fail' if low > 1.15 else 'inconclusive')))
for row in summary:
    control = next(r for r in summary if r['n'] == row['n'] and
                   r['metric'] == row['metric'] and r['label'] == 'std_control')
    row['control_valid'] = control['status'] == 'pass'
(out / 'timing-summary.json').write_text(json.dumps(summary, indent=2))
for row in summary: print(row)
# The 16-byte std wrapper is diagnostic, not a product acceptance target.
raise SystemExit(int(any(r['label']!='std_handle16' and (r['status'] != 'pass' or not r['control_valid']) for r in summary)))
