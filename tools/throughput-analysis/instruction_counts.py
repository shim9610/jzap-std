#!/usr/bin/env python3
"""Count executed instructions per benchmark call with callgrind, for every benchmark binary.

Each (binary, task, implementation) runs twice with identical seed and profile,
once with one measured epoch and once with five, with address randomization off. The per-function difference
removes process start, corpus generation, warmup and teardown, leaving four
measured epochs. Dividing by the difference of the reported operation counts
gives instructions per benchmark call. Counts are deterministic for a given
binary and input, so they complement wall-clock timing on noisy shared hosts.
They are not cycles: cache misses, branch mispredictions and code placement
are absent, and callgrind counts every `rep movs/stos` iteration.
"""
import argparse
import collections
import concurrent.futures
import csv
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
RELEASE = ROOT / 'tools/throughput-analysis/target/release'
# Hash benchmarks need the library's std feature; build them in separate target directories.
BINARIES = {
    'main': RELEASE / 'jzap-throughput-analysis',
    'string': RELEASE / 'string',
    'shared': RELEASE / 'shared',
    'arc_scenarios': RELEASE / 'arc_scenarios',
    'btree': RELEASE / 'btree',
    'btree_set': RELEASE / 'btree_set',
    'hash_map': ROOT / 'target/verification/tp-hash-map/release/hash_map',
    'hash_set': ROOT / 'target/verification/tp-hash-set/release/hash_set',
    'binary_heap': RELEASE / 'binary_heap',
    'deque': RELEASE / 'deque',
    'scratch_slice': RELEASE / 'scratch_slice',
}
VEC_SMALL_TASKS = ['clone', 'extract_all', 'pop']
COPY = ('memcpy', 'memmove', 'memset', 'memcmp', 'bcmp')
ALLOC = ('malloc', 'realloc', 'free', 'calloc', 'sbrk', 'brk', 'mremap', 'mmap', 'munmap', 'unlink_chunk',
         'alloc_perturb', 'systrim', 'morecore', 'tcache', '__rdl_', '__rust_alloc', '__rust_dealloc',
         '__rust_realloc')


def labels_for(task):
    """compare.py label selection without the duplicate std control process."""
    if task.startswith(('scratch_borrowed_', 'scratch_get_', 'scratch_range_', 'scratch_copy_within_')):
        return ['std', 'unlimited']
    if task.startswith('arc_app_'):
        return ['std', 'unlimited', 'borrowed', 'owned']
    if task.startswith(('rc_', 'arc_', 'btree_', 'hash_map_', 'hash_set_', 'binary_heap_', 'deque_', 'scratch_')):
        return ['std', 'unlimited', 'local', 'concurrent']
    if task.startswith('box_'):
        return ['std', 'unlimited', 'metered']
    return ['std', 'unlimited', 'limited', 'metered', 'limited_metered']


def category(function):
    if any(word in function for word in COPY):
        return 'copy'
    if any(word in function for word in ALLOC):
        return 'allocator'
    return 'code'


def self_costs(path):
    """Self instructions per function from uncompressed callgrind output."""
    cost = collections.Counter()
    function = None
    skip = False
    for line in path.open():
        if line.startswith('fn='):
            function = line[3:].strip()
        elif line.startswith('calls='):
            skip = True
        elif line[:1].isdigit() or line.startswith('0x'):
            if skip:
                skip = False
                continue
            cost[function] += int(line.split()[-1])
    return cost


def run_once(binary, label, task, seed, epochs, profile, raw):
    # setarch -R disables address randomization so repeated runs execute identical instruction counts.
    command = ['setarch', '-R', 'valgrind', '--tool=callgrind', '--compress-strings=no', '--compress-pos=no',
               f'--callgrind-out-file={raw}', str(binary), label, task, seed, str(epochs), profile]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        return None, result.stderr.strip().splitlines()[-1:] or ['nonzero exit']
    report = json.loads(result.stdout)
    costs = self_costs(raw)
    raw.unlink()
    return (report, costs), None


def measure(item, seed, out):
    group, binary, profile, task, label = item
    stem = f'{group}.{profile}.{task}.{label}'
    first, error = run_once(binary, label, task, seed, 1, profile, out / f'{stem}.epochs1.callgrind')
    if first is None:
        return dict(group=group, profile=profile, task=task, label=label, status='failed', detail=' '.join(error))
    second, error = run_once(binary, label, task, seed, 5, profile, out / f'{stem}.epochs5.callgrind')
    if second is None:
        return dict(group=group, profile=profile, task=task, label=label, status='failed', detail=' '.join(error))
    (r1, c1), (r2, c2) = first, second
    calls = int(r2['operations']) - int(r1['operations'])
    split = collections.Counter()
    for function in set(c1) | set(c2):
        split[category(function)] += c2[function] - c1[function]
    total = sum(split.values())
    if calls <= 0:
        return dict(group=group, profile=profile, task=task, label=label, status='failed', detail='no operations')
    return dict(group=group, profile=profile, task=task, label=label, status='ok', calls=calls,
                per_call=total / calls, code=split['code'] / calls, allocator=split['allocator'] / calls,
                copy=split['copy'] / calls, corpus_digest=r2.get('corpus_digest'), detail='')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/instruction-counts')
    parser.add_argument('--seed', default='617')
    parser.add_argument('--jobs', type=int, default=4)
    parser.add_argument('--groups', help='comma-separated binary groups; default: all')
    parser.add_argument('--tasks', help='comma-separated task filter')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    groups = args.groups.split(',') if args.groups else list(BINARIES)
    plan = []
    for group in groups:
        binary = BINARIES[group]
        tasks = subprocess.check_output([str(binary), 'tasks'], text=True).strip().split(',')
        if args.tasks:
            tasks = [task for task in tasks if task in args.tasks.split(',')]
        for task in tasks:
            profiles = ['mixed', 'small'] if group == 'main' and task in VEC_SMALL_TASKS else ['mixed']
            plan += [(group, binary, profile, task, label) for profile in profiles for label in labels_for(task)]
    rows = []
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        for row in pool.map(lambda item: measure(item, args.seed, out), plan):
            rows.append(row)
            value = f"{row['per_call']:14.1f}" if row['status'] == 'ok' else row['detail'][:60]
            print(f"{row['group']:13s} {row['profile']:5s} {row['task']:40s} {row['label']:16s} {value}", flush=True)
    fields = ['group', 'profile', 'task', 'label', 'status', 'calls', 'per_call', 'code', 'allocator', 'copy',
              'corpus_digest', 'detail']
    with (out / 'instruction-counts.csv').open('w', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)
    metadata = dict(seed=args.seed, method='per-function difference of 5-epoch and 1-epoch callgrind runs without ASLR',
                    source_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                    valgrind=subprocess.check_output(['valgrind', '--version'], text=True).strip(),
                    binaries={group: subprocess.check_output(['sha256sum', str(BINARIES[group])], text=True).split()[0]
                              for group in groups},
                    rows=len(rows), failed=sum(row['status'] != 'ok' for row in rows))
    (out / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(out / 'instruction-counts.csv')


if __name__ == '__main__':
    main()
