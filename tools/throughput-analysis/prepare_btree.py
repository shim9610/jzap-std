#!/usr/bin/env python3
"""Repeat equal-work std/previous/current BTree workloads in separate processes.

Builds and raw metadata stay in target/. The CSV retains process seed, work,
corpus identity and total continuous time. Input preparation is outside timing.
"""
import argparse
import csv
import hashlib
import io
import json
import math
import random
import shutil
import subprocess
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / 'tools/throughput-analysis/src/bin/btree.rs'
CORPUS = BENCH.parent.parent / 'ordered_corpus.rs'
OPERATIONS = ('build', 'entry', 'remove', 'split', 'append', 'append_disjoint',
              'pop_all', 'remove_cycle')


def inputs():
    paths = [*sorted((ROOT / 'src').rglob('*.rs')), BENCH, CORPUS, Path(__file__).resolve()]
    return hashlib.sha256(b''.join(str(p.relative_to(ROOT)).encode() + b'\0' + p.read_bytes()
                                   for p in paths)).hexdigest()


def main():
    if not __debug__:
        raise RuntimeError('Run without -O/PYTHONOPTIMIZE')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True, help='Pinned Git revision')
    parser.add_argument('--output', type=Path, required=True, help='Directory under target/')
    parser.add_argument('--operation', action='append', choices=OPERATIONS)
    parser.add_argument('--size', action='append', choices=('small', 'mixed'))
    parser.add_argument('--seed', action='append', type=int)
    parser.add_argument('--align32', action='store_true', help='Apply the same optional loop alignment to both builds')
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--build-only', action='store_true')
    mode.add_argument('--run-only', action='store_true', help='Require matching previously built inputs')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        parser.error('Keep generated builds and logs under target/')
    revision = subprocess.check_output(['git', 'rev-parse', args.baseline + '^{commit}'], cwd=ROOT, text=True).strip()
    identity = dict(baseline=revision, current_inputs=inputs(), align32=args.align32)
    metadata = out / 'inputs.json'
    if args.run_only:
        assert json.loads(metadata.read_text()) == identity, 'Build inputs changed; rebuild in a fresh directory'
    else:
        if out.exists():
            parser.error('Choose a fresh output directory, or --run-only for matching builds')
        out.mkdir(parents=True)
        flags = ['--edition=2024', '-O', '-C', 'panic=abort', '-C', 'codegen-units=1']
        if args.align32:
            flags += ['-C', 'llvm-args=-align-all-loops=5']
        for label in ('old', 'new'):
            work = out / label
            work.mkdir()
            if label == 'old':
                data = subprocess.check_output(['git', 'archive', revision, 'src'], cwd=ROOT)
                with tarfile.open(fileobj=io.BytesIO(data)) as archive:
                    archive.extractall(work, filter='data')
            else:
                shutil.copytree(ROOT / 'src', work / 'src')
            library = work / 'libjzap.rlib'
            commands = [
                ['rustc', *flags, '--crate-name', 'jzap', '--crate-type', 'rlib', '--cfg', 'feature="alloc"',
                 str(work / 'src/lib.rs'), '-o', str(library)],
                ['rustc', *flags, '--crate-name', 'btree_workloads', '--extern', 'jzap=' + str(library),
                 str(BENCH), '-o', str(work / 'workloads')],
            ]
            with (work / 'build.log').open('w') as log:
                for command in commands:
                    subprocess.run(command, cwd=ROOT, check=True, stdout=log, stderr=subprocess.STDOUT)
        assert identity['current_inputs'] == inputs(), 'Inputs changed during compilation'
        metadata.write_text(json.dumps(identity, indent=2) + '\n')
    if args.build_only:
        print('Built matching inputs; use --run-only after other compilers/provers finish')
        return
    operations = args.operation or OPERATIONS
    sizes = args.size or ('small', 'mixed')
    seeds = args.seed or (617, 6203, 96011)
    assert len(set(seeds)) == len(seeds) and len(set(operations)) == len(operations)
    modes = {'std': ('old', 'std'), 'old': ('old', 'unlimited'), 'new': ('new', 'unlimited')}

    def sample(label, operation, size, seed, epochs):
        binary, policy = modes[label]
        command = [str(out / binary / 'workloads'), policy, 'btree_' + operation,
                   str(seed), str(epochs), size]
        return json.loads(subprocess.check_output(command, text=True))

    with (out / 'timings.csv').open('w', newline='') as file:
        writer = csv.writer(file)
        writer.writerow(('seed', 'operation', 'size', 'label', 'epochs', 'calls', 'elements',
                         'corpus_digest', 'seconds'))
        for seed in seeds:
            for operation in operations:
                for size in sizes:
                    estimates = []
                    for label in modes:
                        n = 1
                        while True:
                            result = sample(label, operation, size, seed, n)
                            elapsed = result['elapsed_seconds']
                            if elapsed >= 0.1:
                                estimates.append(elapsed / n)
                                break
                            n *= 4
                    n = max(1, math.ceil(2.0 / min(estimates)))
                    order = list(modes)
                    random.Random(f'{seed}:{operation}:{size}').shuffle(order)
                    work = None
                    for label in order:
                        result = sample(label, operation, size, seed, n)
                        assert result['elapsed_seconds'] >= 1.0, ('Short interval; increase calibration work', result)
                        actual = tuple(result[k] for k in ('epochs', 'cases', 'operations', 'elements', 'corpus_digest'))
                        if work is None:
                            work = actual
                        assert actual == work, 'Unequal work or corpus'
                        writer.writerow((seed, operation, size, label, n, result['operations'],
                                         result['elements'], result['corpus_digest'], result['elapsed_seconds']))
                        file.flush()
                    print(seed, operation, size, 'PASS', flush=True)
    assert identity['current_inputs'] == inputs(), 'Inputs changed during timing'
    print(out / 'timings.csv')


if __name__ == '__main__':
    main()
