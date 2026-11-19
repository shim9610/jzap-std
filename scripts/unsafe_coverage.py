#!/usr/bin/env python3
"""List the unsafe blocks that no runtime test executes.

Miri and the sanitizers only check the code a test runs. This script builds the
integration tests with LLVM source coverage, runs them natively and reports
every unsafe block in src/ whose lines have no executed region. The same tests
run under Miri with smaller sizes, so an unexecuted block here is also outside
the executed-memory checks.

    python3 scripts/unsafe_coverage.py [--test NAME ...] [--fail-on-unexecuted]

Output: target/verification/unsafe-coverage/summary.json and unexecuted.txt.
A block counts as executed when any of its lines has a positive count; this
does not show that every branch inside the block ran.
"""
import argparse, json, os, shutil, subprocess, sys
from collections import Counter
from pathlib import Path
from scanner_tools import scanner

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'target/verification/unsafe-coverage'


def tool(name):
    sysroot = subprocess.check_output(['rustc', '--print', 'sysroot'], cwd=ROOT, text=True).strip()
    host = subprocess.check_output(['rustc', '-vV'], cwd=ROOT, text=True).split('host: ')[1].split()[0]
    path = Path(sysroot) / 'lib/rustlib' / host / 'bin' / name
    if not path.exists():
        sys.exit(f'{name} is missing: install the llvm-tools component of the active toolchain')
    return str(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--test', action='append', default=[], help='integration test target; default: all')
    parser.add_argument('--fail-on-unexecuted', action='store_true')
    args = parser.parse_args()
    shutil.rmtree(OUT, ignore_errors=True)
    (OUT / 'profiles').mkdir(parents=True)
    env = dict(os.environ, RUSTFLAGS='-C instrument-coverage', CARGO_TARGET_DIR=str(OUT / 'target'),
               LLVM_PROFILE_FILE=str(OUT / 'profiles/%p-%m.profraw'))
    select = [x for name in args.test for x in ('--test', name)] or ['--tests']
    base = ['cargo', 'test', '--locked', '--offline', '--all-features', *select]
    built = subprocess.run([*base, '--no-run', '--message-format=json'], cwd=ROOT, env=env, text=True,
                           stdout=subprocess.PIPE, stderr=(OUT / 'build.log').open('w'))
    if built.returncode:
        sys.exit('coverage build failed: see ' + str(OUT / 'build.log'))
    binaries = [m['executable'] for m in map(json.loads, filter(None, built.stdout.splitlines()))
                if m.get('reason') == 'compiler-artifact' and m.get('executable') and m['profile']['test']]
    with (OUT / 'run.log').open('w') as log:
        ran = subprocess.run(base, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    if ran.returncode:
        sys.exit('a test failed under coverage: see ' + str(OUT / 'run.log'))
    profiles = sorted(str(p) for p in (OUT / 'profiles').glob('*.profraw'))
    subprocess.run([tool('llvm-profdata'), 'merge', '-sparse', *profiles, '-o', str(OUT / 'merged.profdata')], check=True)
    objects = [x for b in binaries[1:] for x in ('--object', b)]
    lcov = subprocess.check_output([tool('llvm-cov'), 'export', '--format=lcov', '--instr-profile',
                                    str(OUT / 'merged.profdata'), binaries[0], *objects,
                                    '--sources', str(ROOT / 'src')], text=True)
    counts, current = {}, None
    for line in lcov.splitlines():
        if line.startswith('SF:'):
            current = counts.setdefault(str(Path(line[3:]).relative_to(ROOT)), {})
        elif line.startswith('DA:'):
            number, count = line[3:].split(',')[:2]
            current[int(number)] = max(current.get(int(number), 0), int(count))
    files = sorted(str(p.relative_to(ROOT)) for p in (ROOT / 'src').rglob('*.rs') if not p.name.endswith('proofs.rs'))
    sites = [r for r in json.loads(subprocess.check_output([str(scanner('unsafe_sites')), *files], cwd=ROOT, text=True))
             if r['kind'] in ('unsafe_block', 'macro_unsafe_block')]
    rows = []
    for site in sites:
        lines = counts.get(site['file'], {})
        seen = [lines[n] for n in range(site['line'], site['end_line'] + 1) if n in lines]
        state = 'executed' if any(seen) else 'unexecuted' if seen else 'no-coverage-record'
        rows.append(dict(file=site['file'], line=site['line'], end_line=site['end_line'], context=site['context'],
                         function=site['function'], state=state))
    tally = Counter(r['state'] for r in rows)
    missing = [r for r in rows if r['state'] != 'executed']
    summary = dict(tests=args.test or 'all integration tests', test_binaries=len(binaries), unsafe_blocks=len(rows),
                   **{k: tally.get(k, 0) for k in ('executed', 'unexecuted', 'no-coverage-record')}, not_executed=missing)
    (OUT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    (OUT / 'unexecuted.txt').write_text(''.join(
        f"{r['file']}:{r['line']} {r['state']} {r['context']}::{r['function']}\n" for r in missing))
    print(f"unsafe blocks {len(rows)}: executed {tally.get('executed', 0)}, unexecuted {tally.get('unexecuted', 0)}, "
          f"no coverage record {tally.get('no-coverage-record', 0)}")
    by_file = Counter(r['file'] for r in missing)
    for name, count in sorted(by_file.items()):
        print(f'  {name}: {count}')
    if missing and args.fail_on_unexecuted:
        sys.exit(1)


if __name__ == '__main__':
    main()
