#!/usr/bin/env python3
"""Repeat existing BTree binaries with process CPU/scheduling observations; never rebuild them."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import random
import resource
import re
import subprocess

from prepare_btree import ROOT, OPERATIONS, inputs


def inspect_code(source, out):
    """Compare exact linked instruction rows, including addresses and bytes."""
    functions = []
    for label in ('old', 'new'):
        asm = subprocess.check_output(['objdump', '-d', '--demangle',
            str(source/label/'workloads')], text=True)
        (out/(label+'.asm')).write_text(asm)
        table, current = {}, None
        for line in asm.splitlines():
            match = re.fullmatch(r'[0-9a-f]+ <(.+)>:', line)
            if match:
                current = match[1]
                ordinal = 0
                while current in table:
                    ordinal += 1
                    current = match[1] + '#' + str(ordinal)
                table[current] = []
            elif current and re.match(r'\s*[0-9a-f]+:', line):
                table[current].append(line)
        functions.append(table)
    old, new = functions
    shared = old.keys() & new.keys()
    changed = sorted(name for name in shared if old[name] != new[name])
    report = dict(changed=changed, added=sorted(new.keys()-old.keys()),
        removed=sorted(old.keys()-new.keys()),
        run_functions=sum(name.startswith('btree_workloads::run') for name in shared),
        epochs_functions=sum(name.startswith('btree_workloads::epochs') for name in shared),
        changed_workload_functions=[name for name in changed if name.startswith(
            ('btree_workloads::run', 'btree_workloads::epochs'))])
    (out/'code.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report, indent=2))


def main():
    if not __debug__:
        raise RuntimeError('Run without -O/PYTHONOPTIMIZE')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binaries', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--code-only', action='store_true', help='Compare exact linked functions without timing')
    parser.add_argument('--operation', choices=OPERATIONS, default='entry')
    parser.add_argument('--size', choices=('small', 'mixed'), default='mixed')
    parser.add_argument('--rounds', type=int, default=3)
    parser.add_argument('--work-multiplier', type=int, default=2)
    parser.add_argument('--cpu', type=int, help='Pin this process and its children to an allowed CPU')
    args = parser.parse_args()
    if args.rounds < 1 or args.work_multiplier < 1:
        parser.error('Use positive rounds and work multiplier')
    source = args.binaries.resolve()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'target') or out.exists():
        parser.error('Choose a fresh output directory under target/')
    metadata = json.loads((source / 'inputs.json').read_text())
    if metadata['current_inputs'] != inputs():
        parser.error('Recorded build inputs differ from current source')
    if args.code_only:
        out.mkdir(parents=True)
        inspect_code(source, out)
        return
    rows = list(csv.DictReader((source / 'timings.csv').open()))
    work = {}
    for row in rows:
        if row['operation'] == args.operation and row['size'] == args.size:
            seed = int(row['seed'])
            n = int(row['epochs'])
            if seed in work:
                assert work[seed] == n
            work[seed] = n
    if not work:
        parser.error('No completed input condition found')
    allowed = sorted(os.sched_getaffinity(0))
    if args.cpu is not None and args.cpu not in allowed:
        parser.error('CPU must be one of ' + str(allowed))
    if args.cpu is not None:
        os.sched_setaffinity(0, {args.cpu})
    out.mkdir(parents=True)
    provenance = dict(build=metadata, operation=args.operation, size=args.size,
                      rounds=args.rounds, multiplier=args.work_multiplier,
                      affinity=sorted(os.sched_getaffinity(0)),
                      binaries={name: hashlib.sha256((source/name/'workloads').read_bytes()).hexdigest()
                                for name in ('old', 'new')})
    (out/'inputs.json').write_text(json.dumps(provenance, indent=2)+'\n')
    modes = {'std': ('old', 'std'), 'old': ('old', 'unlimited'), 'new': ('new', 'unlimited')}
    with (out/'timings.csv').open('w', newline='') as file:
        writer = csv.writer(file)
        writer.writerow(('round', 'seed', 'operation', 'size', 'label', 'epochs', 'calls',
                         'seconds', 'process_cpu_seconds', 'off_cpu_lower_bound_seconds',
                         'involuntary_switches', 'voluntary_switches', 'major_faults', 'corpus_digest'))
        for round_id in range(args.rounds):
            for seed, base in sorted(work.items()):
                epochs = base * args.work_multiplier
                order = list(modes)
                random.Random(f'{round_id}:{seed}').shuffle(order)
                expected = None
                for label in order:
                    binary, policy = modes[label]
                    before = resource.getrusage(resource.RUSAGE_CHILDREN)
                    row = json.loads(subprocess.check_output([str(source/binary/'workloads'), policy,
                        'btree_'+args.operation, str(seed), str(epochs), args.size], text=True))
                    after = resource.getrusage(resource.RUSAGE_CHILDREN)
                    elapsed = row['elapsed_seconds']
                    assert elapsed >= 1.0, ('Short interval; repeat the whole condition with more work', row)
                    actual = tuple(row[k] for k in ('epochs','cases','operations','elements','corpus_digest'))
                    if expected is None:
                        expected = actual
                    assert expected == actual
                    cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
                    # Whole-process CPU includes preparation and teardown. Subtracting
                    # it from the timed interval gives a conservative off-CPU bound.
                    writer.writerow((round_id,seed,args.operation,args.size,label,epochs,row['operations'],
                        elapsed,cpu,max(0.0,elapsed-cpu),after.ru_nivcsw-before.ru_nivcsw,
                        after.ru_nvcsw-before.ru_nvcsw,after.ru_majflt-before.ru_majflt,row['corpus_digest']))
                    file.flush()
                print(round_id,seed,'PASS',flush=True)
    assert metadata['current_inputs'] == inputs(), 'Build inputs changed during timing'
    print(out/'timings.csv')


if __name__ == '__main__':
    main()
