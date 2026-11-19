#!/usr/bin/env python3
"""Paired long-interval Vec edit timing with a same-code control and fixed stopping rule."""
import argparse
import collections
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import random
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
LABELS = {'std', 'old', 'new', 'old_control'}
RESAMPLES = 50000
CASES = ['none-small', 'none-mixed', 'all-small', 'all-mixed',
         'half-small', 'half-mixed', 'early-drop-small', 'early-drop-mixed',
         'prefix-none-small', 'prefix-none-mixed', 'prefix-last-small', 'prefix-last-mixed']
SPLICE_CASES = [f'{shape}-{size}' for shape in (
    'delete', 'shorter', 'exact', 'longer-known', 'longer-unknown', 'staged-excess')
    for size in ('small', 'mixed')]


def digest(paths):
    return hashlib.sha256(b''.join(str(p.relative_to(ROOT)).encode() + b'\0' + p.read_bytes()
                                   for p in paths)).hexdigest()


def inputs():
    return digest(sorted((ROOT / 'src').rglob('*.rs')) + [Path(__file__),
        Path(__file__).with_name('prepare_range.py'),
        Path(__file__).parent / 'experiments/extraction.rs',
        Path(__file__).parent / 'experiments/extraction_stats.rs'])


def validate(rows, plan, cases=12):
    accepted = [r for r in rows if r['phase'] == 'measurement']
    grouped = collections.defaultdict(list)
    seeds = {int(p['block']): int(p['seed']) for p in plan}
    for r in accepted:
        assert int(r['block']) in seeds and int(r['seed']) == seeds[int(r['block'])]
        assert 0 <= int(r['case']) < cases
        assert float(r['wall_seconds']) >= 1 and float(r['cpu_seconds']) > 0, 'short interval'
        assert int(r['operations']) == int(r['epochs']) * 64 > 0
        grouped[int(r['block']), int(r['case'])].append(r)
    assert set(grouped) == {(b, c) for b in seeds for c in range(cases)}, 'incomplete paired blocks'
    corpora = {}
    for group in grouped.values():
        assert len(group) == 4 and {r['label'] for r in group} == LABELS, 'missing/duplicate label'
        assert {int(r['position']) for r in group} == set(range(4)), 'invalid measurement order'
        assert len({tuple(r[k] for k in ('seed', 'epochs', 'operations', 'checksum', 'corpus_digest', 'attempt'))
                    for r in group}) == 1, 'unequal input/work/result'
        key = int(group[0]['seed']), int(group[0]['case'])
        value = group[0]['corpus_digest']
        assert key not in corpora or corpora[key] == value, 'corpus changed across repetitions'
        corpora[key] = value
    return grouped


def paired_bootstrap(pairs, resamples=RESAMPLES, seed=617):
    """Resample paired process blocks within each fixed input-seed stratum."""
    strata = collections.defaultdict(list)
    for input_seed, numerator, denominator in pairs:
        assert numerator > 0 and denominator > 0
        strata[input_seed].append((numerator, denominator))
    rng = random.Random(seed)
    ratios = []
    for _ in range(resamples):
        numerator = denominator = 0.0
        for group in strata.values():
            for _ in group:
                a, b = group[rng.randrange(len(group))]
                numerator += a
                denominator += b
        ratios.append(numerator / denominator)
    ratios.sort()
    quantile = lambda p: ratios[min(len(ratios) - 1, int(p * len(ratios)))]
    # 24 primary comparisons: new/old and new/std across twelve conditions.
    return dict(low95=quantile(.025), high95=quantile(.975),
                family_low=quantile(.05 / 48), family_high=quantile(1 - .05 / 48))


def summarize(rows, plan, cases=CASES):
    grouped = validate(rows, plan, len(cases))
    results = []
    for case, name in enumerate(cases):
        groups = [{r['label']: r for r in grouped[int(p['block']), case]} for p in plan]
        seconds = lambda label, metric: sum(float(g[label][metric]) for g in groups)
        calls = sum(int(g['old']['operations']) for g in groups)
        for metric in ('wall_seconds', 'cpu_seconds'):
            aa = paired_bootstrap([(int(g['old']['seed']), float(g['old_control'][metric]),
                                    float(g['old'][metric])) for g in groups])
            stable = aa['low95'] >= .95 and aa['high95'] <= 1.05
            for denominator in ('old', 'std'):
                ci = paired_bootstrap([(int(g['old']['seed']), float(g['new'][metric]),
                                        float(g[denominator][metric])) for g in groups])
                ratio = seconds('new', metric) / seconds(denominator, metric)
                effect = ('lower' if ci['family_high'] < 1 else
                          'higher' if ci['family_low'] > 1 else 'unresolved')
                if not stable:
                    effect = 'unresolved-control-drift'
                results.append(dict(case=case, condition=name, metric=metric,
                    comparison='new/' + denominator, blocks=len(groups), operations=calls,
                    new_ns=seconds('new', metric) / calls * 1e9,
                    reference_ns=seconds(denominator, metric) / calls * 1e9,
                    ratio=ratio, **ci, control_ratio=seconds('old_control', metric) / seconds('old', metric),
                    control_low95=aa['low95'], control_high95=aa['high95'], control_stable=stable,
                    effect=effect,
                    std_15_gate=('not-applicable' if denominator != 'std' else
                                 'unresolved-control-drift' if not stable else
                                 'within' if ci['family_high'] <= 1.15 else
                                 'above' if ci['family_low'] > 1.15 else 'unresolved')))
    return results


def main():
    if not __debug__:
        raise RuntimeError('Assertions must remain enabled')
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--benchmark', choices=['extraction', 'splice-fill'], default='extraction')
    p.add_argument('--baseline', required=True)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--rounds', type=int, default=4)
    p.add_argument('--seeds', default='1,2,3')
    p.add_argument('--target-seconds', type=float, default=2)
    p.add_argument('--cpu', type=int)
    mode = p.add_mutually_exclusive_group()
    mode.add_argument('--build-only', action='store_true')
    mode.add_argument('--run-only', action='store_true')
    args = p.parse_args()
    cases = SPLICE_CASES if args.benchmark == 'splice-fill' else CASES
    seeds = [int(s) for s in args.seeds.split(',')]
    assert len(set(seeds)) == len(seeds) >= 3 and all(s > 0 for s in seeds)
    assert args.rounds >= 4 and args.target_seconds >= 1.5
    out = args.output.resolve()
    assert out.is_relative_to(ROOT / 'target'), 'Use ignored target/ storage'
    allowed = sorted(os.sched_getaffinity(0))
    cpu = min(allowed) if args.cpu is None else args.cpu
    assert cpu in allowed
    revision = subprocess.check_output(['git', 'rev-parse', args.baseline + '^{commit}'], text=True).strip()
    plan = []
    for round_id in range(args.rounds):
        order = seeds.copy()
        random.Random(617 + round_id).shuffle(order)
        for seed in order:
            plan.append(dict(block=len(plan), round=round_id, seed=seed))
    protocol = dict(baseline=revision, source_inputs=inputs(), plan=plan, cpu=cpu,
        target_seconds=args.target_seconds, minimum_seconds=1, cases=cases,
        compiler=subprocess.check_output(['rustc', '-vV'], text=True),
        bootstrap_resamples=RESAMPLES, bootstrap_seed=617,
        bootstrap_unit='paired process blocks, stratified by three fixed input seeds',
        primary_metric='wall_seconds; thread CPU seconds are supporting diagnostics',
        primary_family='24 ratios: new/old and new/std across 12 conditions; Bonferroni bootstrap bounds',
        control='old_control calls the identical old function; its 95% interval must lie within [0.95,1.05]',
        stopping='fixed process count; no significance-based stopping or outlier removal',
        timing='complete clone/edit/observe/drop; preparation, calibration and reporting excluded',
        cpu_model=next((s.split(':', 1)[1].strip() for s in Path('/proc/cpuinfo').read_text().splitlines()
                        if s.startswith('model name')), 'unknown'))
    if not args.run_only:
        out.mkdir(parents=True, exist_ok=False)
        subprocess.run(['python3', str(Path(__file__).with_name('prepare_range.py')),
            '--baseline', revision, '--benchmark', args.benchmark, '--profile', 'default',
            '--build-only', '--output', str(out / 'build')], check=True, cwd=ROOT)
        command = ['rustc', '--edition=2024', '-O', '-C', 'panic=abort', '-C', 'codegen-units=1',
            '--crate-name', 'vec_edit_statistics', '--emit=asm,link', '--cfg', 'jzap_stat_probe']
        if args.benchmark == 'splice-fill':
            command += ['--cfg', 'jzap_splice_fill_probe']
        for label in ('old', 'new'):
            command += ['--extern', f'jzap_{label}={out}/build/libjzap_{label}.rlib']
        command += [str(ROOT / 'tools/throughput-analysis/experiments/extraction.rs'), '-o', str(out / 'consumer')]
        subprocess.run(command, check=True, cwd=ROOT)
        protocol['build_command'] = command
        protocol['binary_sha256'] = hashlib.sha256((out / 'consumer').read_bytes()).hexdigest()
        symbols = subprocess.check_output(['nm', '-S', str(out / 'consumer')], text=True)
        protocol['consumer_symbols'] = [s for s in symbols.splitlines()
                                       if re.search(r'(?:7current|8previous|8standard)17h', s)]
        (out / 'protocol.json').write_text(json.dumps(protocol, indent=2) + '\n')
    else:
        stored = json.loads((out / 'protocol.json').read_text())
        assert all(stored[k] == v for k, v in protocol.items()), 'protocol/build input drift'
        protocol = stored
    if args.build_only:
        print('BUILT', out, flush=True)
        return
    assert not (out / 'records.csv').exists(), 'Choose a fresh run; never overwrite observations'
    assert inputs() == protocol['source_inputs']
    assert hashlib.sha256((out / 'consumer').read_bytes()).hexdigest() == protocol['binary_sha256']
    os.sched_setaffinity(0, {cpu})
    all_rows = []
    header = None
    with (out / 'records.csv').open('w', newline='') as raw:
        for item in plan:
            print('BLOCK', item, flush=True)
            with (out / f'block-{item["block"]}.stderr.log').open('w') as errors:
                child = subprocess.Popen([str(out / 'consumer'), str(item['seed']), str(item['block']),
                                          str(args.target_seconds)], stdout=subprocess.PIPE,
                                         stderr=errors, text=True, cwd=ROOT)
                reader = csv.DictReader(child.stdout)
                if header is None:
                    header = reader.fieldnames
                    writer = csv.DictWriter(raw, fieldnames=header)
                    writer.writeheader()
                assert header == reader.fieldnames
                for row in reader:
                    writer.writerow(row)
                    raw.flush()
                    all_rows.append(row)
                    if row['phase'] == 'measurement' and int(row['position']) == 3:
                        print('MEASURED block', item['block'], 'case', row['case'], flush=True)
                assert child.wait() == 0, f'Incomplete block {item}; retain raw observations'
    assert inputs() == protocol['source_inputs'], 'source changed during measurement'
    assert hashlib.sha256((out / 'consumer').read_bytes()).hexdigest() == protocol['binary_sha256']
    results = summarize(all_rows, plan, cases)
    with (out / 'summary.csv').open('w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=results[0]); writer.writeheader(); writer.writerows(results)
    (out / 'summary.json').write_text(json.dumps(dict(status='pass', protocol=protocol,
        source_unchanged=True, completed_processes=len(plan), results=results), indent=2) + '\n')
    print('COMPLETE', out, flush=True)


if __name__ == '__main__':
    main()
