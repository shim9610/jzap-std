#!/usr/bin/env python3
"""Build default Splice consumers and model their actual fill loops; no timing.

Loop selection is structural and fail-closed: each implementation must contain
two scalar replacement loops and two four-element SSE2 loops, or one or two
of each when only the first fill is requested for older consumers. Review
the emitted assembly when a compiler or source change alters that structure.
"""
import argparse
import csv
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
CPUS = ('skylake', 'alderlake', 'znver3', 'znver4', 'znver5')


def run(command, **kwargs):
    return subprocess.run(command, check=True, text=True, capture_output=True, **kwargs)


def loops(assembly, name, first_only=False):
    symbol = re.search(r'(?m)^(_ZN\w*' + name + r'17h\w+E):\n', assembly)
    if symbol is None:
        raise ValueError('Missing consumer: ' + name)
    body = assembly[symbol.start():assembly.index('\n\t.cfi_endproc', symbol.start())]
    pieces = re.split(r'(?m)^(\.LBB\d+_\d+):\n', body)
    found = {'scalar': [], 'vector': []}
    for index in range(1, len(pieces), 2):
        label = pieces[index]
        instructions = [line for line in pieces[index + 1].splitlines()
                        if re.match(r'^\t[a-z]', line)]
        backedge = next((i for i, line in enumerate(instructions)
                         if re.fullmatch(r'\tjne\t' + re.escape(label), line)), None)
        if backedge is None:
            continue
        instructions = instructions[:backedge + 1]
        if any('\tpsllq\t' in line for line in instructions):
            kind = 'vector'
        elif any('\txorq\t' in line for line in instructions):
            kind = 'scalar'
        else:
            continue
        found[kind].append((label, instructions))
    counts = (1, 2) if first_only else (2,)
    if any(len(value) not in counts for value in found.values()):
        raise ValueError('Changed fill structure: ' + name + ' ' + str(found))
    if first_only:
        found = {kind: paths[:1] for kind, paths in found.items()}
    return symbol.group(1), found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--output', type=Path, required=True, help='Fresh ignored target directory')
    parser.add_argument('--mca', type=Path, required=True)
    parser.add_argument('--first-fill-only', action='store_true',
                        help='Analyze only the original hole fill; accepts older one-loop consumers')
    args = parser.parse_args()
    out = args.output.resolve()
    compiler = run(['rustc', '-vV']).stdout
    if 'release: 1.95.0' not in compiler or 'LLVM version: 22.1.2' not in compiler:
        raise ValueError('Review selectors for this compiler; expected Rust 1.95.0 / LLVM 22.1.2')
    run(['python3', str(ROOT / 'tools/throughput-analysis/prepare_range.py'),
         '--benchmark', 'splice-fill', '--baseline', args.baseline, '--profile',
         'default', '--build-only', '--output', str(out)], cwd=ROOT)
    command = ['rustc', '--edition=2024', '-O', '-C', 'panic=abort', '-C',
               'codegen-units=1', '--crate-name', 'vec_edit_analysis', '--emit=asm,link',
               '--cfg', 'jzap_splice_fill_probe']
    for label in ('old', 'new'):
        command += ['--extern', f'jzap_{label}={out}/libjzap_{label}.rlib']
    command += [str(ROOT / 'tools/throughput-analysis/experiments/extraction.rs'),
                '-o', str(out / 'consumer')]
    run(command, cwd=ROOT)
    assembly = (out / 'consumer.s').read_text()
    sizes = {line.split()[-1]: int(line.split()[1], 16)
             for line in run(['nm', '-S', str(out / 'consumer')]).stdout.splitlines()
             if len(line.split()) == 4}
    rows = []
    symbols = {}
    for name in ('previous', 'current', 'standard'):
        symbol, selected = loops(assembly, name, args.first_fill_only)
        symbols[name] = dict(symbol=symbol, bytes=sizes[symbol])
        for kind, paths in selected.items():
            for phase, (label, instructions) in enumerate(paths, 1):
                # MCA repeats this known path linearly; jump outcomes are fixed.
                targets = {line.split()[-1] for line in instructions
                           if re.match(r'\tj\w+\t\.LBB', line)}
                trace = '\n'.join(instructions + [target + ':' for target in sorted(targets)]) + '\n'
                trace_name = f'{name}-{kind}-{phase}'
                (out / (trace_name + '.s')).write_text(trace)
                for cpu in CPUS:
                    result = run([str(args.mca.resolve()), '-mtriple=x86_64',
                                  '-mcpu=' + cpu, '-iterations=10000'], input=trace)
                    (out / f'{trace_name}-{cpu}.txt').write_text(result.stdout + result.stderr)
                    values = {key: re.search(r'^' + re.escape(key) + r':\s*(.+)',
                                            result.stdout, re.M).group(1)
                              for key in ('Total Cycles', 'Total uOps', 'Block RThroughput')}
                    rows.append(dict(implementation=name, path=kind, fill=phase, cpu=cpu,
                                     elements=4 if kind == 'vector' else 1,
                                     instructions=len(instructions),
                                     conditional_branches=sum(bool(re.match(r'\tj(?!mp)\w+', line))
                                                              for line in instructions),
                                     cycles_per_iteration=int(values['Total Cycles']) / 10000,
                                     uops_per_iteration=int(values['Total uOps']) / 10000,
                                     block_throughput=float(values['Block RThroughput'])))
    with (out / 'model.csv').open('w') as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    (out / 'analysis.json').write_text(json.dumps(dict(
        compiler=compiler, mca=run([str(args.mca.resolve()), '--version']).stdout,
        consumer_command=command, first_fill_only=args.first_fill_only,
        symbols=symbols, rows=rows), indent=2) + '\n')
    print(out / 'analysis.json')


if __name__ == '__main__':
    main()
