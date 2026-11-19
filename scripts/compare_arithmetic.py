#!/usr/bin/env python3
"""Repeated scalar arithmetic timings; run after verify_arithmetic.py finishes."""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import random
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/arithmetic')
parser.add_argument('--trials', type=int, default=30)
parser.add_argument('--operations', type=int, default=1 << 20)
args = parser.parse_args()
out = args.output.resolve()
verification = json.loads((out / 'verification.json').read_text())
assert verification['source_sha256'] == hashlib.sha256((ROOT / 'src/arithmetic.rs').read_bytes()).hexdigest()
assert all(hashlib.sha256((ROOT/p).read_bytes()).hexdigest()==sha for p,sha in verification.get('extra_source_sha256',{}).items())
source_files = [ROOT / 'src/arithmetic.rs', ROOT / 'src/arithmetic/checked.rs', ROOT / 'src/lib.rs', ROOT / 'tools/arithmetic-analysis/src/lib.rs', ROOT / 'tools/arithmetic-analysis/src/main.rs']
profiles = ['release', 'release-checked']
executables = {p: out / p / 'build/release/jzap-arithmetic-analysis' for p in profiles}
for exe in executables.values():
    assert exe.exists()
    assert all(p.stat().st_mtime <= exe.stat().st_mtime for p in source_files), f'Rebuild {exe}'
cpu = min(os.sched_getaffinity(0))
os.sched_setaffinity(0, {cpu})
metadata = dict(trials=args.trials, operations_per_sample=args.operations,
    retained_samples=15, warmups=2, cpu=cpu, type='usize', bits=64,
    comparison='Identical successful inputs. Plain release may wrap on other inputs; checked APIs return errors.',
    scope='Amortized scalar iterations including identical input barriers and XOR bookkeeping; not isolated CPU instruction latency.',
    criterion='Compare wrapper to native checked operations using identical Result ABI and assembly. Report raw-operator cost without imposing a fictitious zero-cost error check.',
    source_sha256=verification['source_sha256'],
    binaries={p: dict(path=str(exe), sha256=hashlib.sha256(exe.read_bytes()).hexdigest()) for p,exe in executables.items()})
(out / 'timing-metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
labels = ['plain', 'plain_control', 'native', 'jzap', 'wrapping']
rows = []
for profile, exe in executables.items():
    for scenario in ['opaque', 'bounded', 'call']:
        for op in ['add', 'sub', 'mul', 'div']:
            for trial in range(args.trials):
                shift = trial % len(labels)
                order = labels[shift:] + labels[:shift]
                if trial % 2:
                    order.reverse()
                for label in order:
                    result = subprocess.check_output([str(exe), 'plain' if label == 'plain_control' else label,
                        op, scenario, str(args.operations)], text=True)
                    row = json.loads(result)
                    row.update(profile=profile, scenario=scenario, op=op, label=label, trial=trial)
                    rows.append(row)
            (out / 'timings.json').write_text(json.dumps(rows, indent=2) + '\n')
            print(profile, scenario, op, 'done', flush=True)

rng = random.Random(9610)
summary = []
for profile in profiles:
    for scenario in ['opaque', 'bounded', 'call']:
        for op in ['add', 'sub', 'mul', 'div']:
            def values(label):
                return [statistics.median(r['samples_ns']) / r['operations'] for r in rows
                        if (r['profile'],r['scenario'],r['op'],r['label']) == (profile,scenario,op,label)]
            base = values('plain')
            native = values('native')
            for label in labels[1:]:
                samples = values(label)
                ratios, differences = [], []
                for _ in range(5000):
                    a = statistics.median(rng.choices(samples, k=len(samples)))
                    b = statistics.median(rng.choices(base, k=len(base)))
                    ratios.append(a / b)
                    differences.append(a - b)
                ratios.sort(); differences.sort()
                summary.append(dict(profile=profile, scenario=scenario, op=op, label=label,
                    plain_ns=statistics.median(base), ns=statistics.median(samples),
                    delta_ns=statistics.median(samples)-statistics.median(base),
                    ratio=statistics.median(samples)/statistics.median(base),
                    ratio95=[ratios[125],ratios[4874]], delta95=[differences[125],differences[4874]],
                    native_ns=statistics.median(native), vs_native=statistics.median(samples)/statistics.median(native)))
(out / 'timing-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
for row in summary:
    if row['label'] in ('jzap', 'wrapping'):
        print(row, flush=True)
