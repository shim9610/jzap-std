#!/usr/bin/env python3
"""Walk the linked call graph of default hash-key acquisition (std feature).

The no_std link fixture (check_link_exits.py) cannot link the std entropy path,
so this check builds tools/seed-link-audit, disassembles the executable and
follows every direct call/jump and GOT-resolved indirect call reachable from
`seed_link_root`. It reports reachable panic, allocation and process-exit
targets, trap instructions and unresolved indirect calls. libc entry points
(PLT/GOT symbols) are leaves: they are the trusted OS boundary.

A product profile passes when nothing forbidden is reachable and no indirect
call is unresolved. Each control must be reported. A pass covers this concrete
root and these profiles only; it is not a proof about foreign code, UB or
nontermination. Outputs remain under target/verification/.
"""
import argparse
import bisect
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tools/seed-link-audit'
ROOT_SYMBOL = 'seed_link_root'
PROFILES = {
    'release': ('release', {}),
    'release-lto': ('release', {'CARGO_PROFILE_RELEASE_LTO': 'fat'}),
    'release-checked': ('release', {'CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS': 'true'}),
    'debug': ('dev', {'CARGO_PROFILE_DEV_DEBUG': '1'}),
}
CONTROLS = ['panic-control', 'alloc-control', 'std-file-control']
PANIC = ('core::panicking::', 'std::panicking::', 'core::slice::index::slice_index_fail',
         'core::option::unwrap_failed', 'core::option::expect_failed', 'core::result::unwrap_failed',
         'alloc::alloc::handle_alloc_error', 'std::alloc::rust_oom', 'std::process::abort',
         'std::sys::abort_internal', 'rust_panic', '__rust_start_panic', 'rust_begin_unwind')
ALLOCATION = ('__rust_alloc', '__rust_alloc_zeroed', '__rust_realloc', '__rustc::__rust_alloc',
              '__rustc::__rust_alloc_zeroed', '__rustc::__rust_realloc', 'malloc', 'calloc',
              'realloc', 'posix_memalign', 'aligned_alloc')
EXITS = ('abort', 'exit', '_exit', '_Exit', 'quick_exit', 'pthread_exit', '__assert_fail')


def leaf(name):
    return name.split('@')[0]


def classify(name):
    base = leaf(name)
    if base.startswith(PANIC) or '::panicking::' in base:
        return 'panic'
    if base in ALLOCATION or base.startswith(('__rust_alloc', '__rustc::__rust_alloc', '__rust_realloc')):
        return 'allocation'
    if base in EXITS:
        return 'exit'
    return None


def analyse(binary):
    asm = subprocess.check_output(['objdump', '-dC', '--no-show-raw-insn', str(binary)], text=True)
    relocations = subprocess.check_output(['objdump', '-R', str(binary)], text=True)
    got = {}
    for slot, kind, target in re.findall(r'^([0-9a-f]+)\s+(\S+)\s+(\S+)$', relocations, re.M):
        got[int(slot, 16)] = target
    functions = {}
    order = []
    current = None
    recent = []
    for line in asm.splitlines():
        header = re.match(r'^([0-9a-f]+) <(.+)>:$', line)
        if header:
            current = dict(start=int(header[1], 16), name=header[2], edges=[], indirect=[],
                           loaded_pointers=0, jump_tables=0, traps=0)
            functions[current['start']] = current
            order.append(current['start'])
            recent = []
            continue
        if current is None or not re.match(r'^\s+[0-9a-f]+:', line):
            continue
        recent = (recent + [line])[-6:]
        # int3 is inter-function padding; ud2 is a reachable trap.
        if re.search(r'\tud2\b', line):
            current['traps'] += 1
        branch = re.search(r'\b(call|jmp)\s+(.*)$', line)
        reference = re.search(r'# ([0-9a-f]+) <', line)
        if not branch:
            # A RIP-relative load of a GOT slot or a function address can feed a
            # later register call: follow it (sound over-approximation).
            if reference:
                address = int(reference[1], 16)
                if address in got:
                    target = got[address]
                    current['edges'].append(int(target[len('*ABS*+0x'):], 16) if target.startswith('*ABS*+0x')
                                            else ('symbol', target))
                    current['loaded_pointers'] += 1
                elif re.search(r'\blea\b', line):
                    current['edges'].append(address)
                    current['loaded_pointers'] += 1
            continue
        operand = branch[2]
        direct = re.match(r'([0-9a-f]+) <(.+)>', operand)
        if direct:
            current['edges'].append(int(direct[1], 16))
            continue
        if reference and int(reference[1], 16) in got:
            target = got[int(reference[1], 16)]
            current['edges'].append(int(target[len('*ABS*+0x'):], 16) if target.startswith('*ABS*+0x')
                                    else ('symbol', target))
            continue
        if operand.startswith('*'):
            # A switch table: an indexed 32-bit offset load added to a table base,
            # then an indirect jump inside the same function.
            if branch[1] == 'jmp' and any(re.search(r'\bmovslq\s+\(%\w+,%\w+,4\)', r) for r in recent):
                current['jump_tables'] += 1
            else:
                current['indirect'].append(line.strip())
    order.sort()

    def owner(address):
        index = bisect.bisect_right(order, address) - 1
        return order[index] if index >= 0 else None

    roots = [start for start, f in functions.items() if f['name'] == ROOT_SYMBOL]
    if len(roots) != 1:
        raise SystemExit(f'expected one {ROOT_SYMBOL}, found {len(roots)}')
    seen, stack = set(), list(roots)
    findings, libc, indirect, traps = [], set(), [], []
    while stack:
        start = stack.pop()
        if start in seen:
            continue
        seen.add(start)
        function = functions[start]
        name = function['name']
        kind = classify(name)
        if kind:
            findings.append(dict(kind=kind, target=name))
            continue
        if name.endswith('@plt'):
            libc.add(leaf(name))
            continue
        if function['traps']:
            traps.append(dict(function=name, count=function['traps']))
        indirect += [dict(function=name, instruction=i, loaded_pointers=function['loaded_pointers'])
                     for i in function['indirect']]
        for edge in function['edges']:
            if isinstance(edge, tuple):
                kind = classify(edge[1])
                if kind:
                    findings.append(dict(kind=kind, target=edge[1], caller=name))
                else:
                    libc.add(leaf(edge[1]))
                continue
            target = owner(edge)
            if target is not None and target != start:
                stack.append(target)
    reachable = sorted(functions[s]['name'] for s in seen)
    # A register call in a function whose every RIP-relative function pointer is
    # already followed is covered by those edges; one without any is unresolved.
    covered = [i for i in indirect if i['loaded_pointers']]
    unresolved = [i for i in indirect if not i['loaded_pointers']]
    return dict(reachable_functions=reachable, findings=findings, libc_leaves=sorted(libc),
                pointer_covered_indirect_calls=covered, unresolved_indirect_calls=unresolved,
                trap_functions=traps), asm


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profiles', default=','.join(PROFILES))
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/seed-link')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    sources = sorted([*(ROOT / 'src').rglob('*.rs'), *FIXTURE.rglob('*.rs'), FIXTURE / 'Cargo.toml', Path(__file__)])
    source_sha256 = hashlib.sha256(b''.join(str(p.relative_to(ROOT)).encode() + b'\0' + p.read_bytes()
                                            for p in sources)).hexdigest()
    results = []
    for profile in args.profiles.split(','):
        cargo_profile, environment = PROFILES[profile]
        for control in [None, *CONTROLS]:
            work = args.output / profile / (control or 'product')
            work.mkdir(parents=True, exist_ok=True)
            command = ['cargo', 'build', '--locked', '--offline', '--manifest-path', str(FIXTURE / 'Cargo.toml'),
                       '--profile', cargo_profile, '--target-dir', str(args.output / 'cargo' / profile)]
            if control:
                command += ['--features', control]
            env = dict(**__import__('os').environ, **environment)
            with (work / 'build.log').open('w') as log:
                built = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=log).returncode
            row = dict(profile=profile, control=control, build_exit=built)
            if built == 0:
                directory = 'release' if cargo_profile == 'release' else 'debug'
                binary = args.output / 'cargo' / profile / directory / 'jzap-seed-link-audit'
                report, asm = analyse(binary)
                (work / 'disassembly.txt').write_text(asm)
                (work / 'reachable.json').write_text(json.dumps(report, indent=2) + '\n')
                run = subprocess.run([str(binary)], capture_output=True, text=True, timeout=30)
                row.update(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                           reachable=len(report['reachable_functions']), libc_leaves=report['libc_leaves'],
                           findings=report['findings'], unresolved_indirect_calls=len(report['unresolved_indirect_calls']),
                           trap_functions=report['trap_functions'], run_exit=run.returncode, run_stdout=run.stdout.strip())
                clean = not report['findings'] and not report['unresolved_indirect_calls'] and not report['trap_functions']
                if control:
                    row['status'] = 'control-detected' if not clean else 'control-missed'
                else:
                    row['status'] = 'pass' if clean and run.returncode == 0 else 'retained-edge'
            else:
                row['status'] = 'build-error'
            results.append(row)
            print(profile, control or 'product', row['status'], len(row.get('findings', [])), flush=True)
            (args.output / 'summary.json').write_text(json.dumps(dict(
                source_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                source_sha256=source_sha256,
                rustc=subprocess.check_output(['rustc', '-Vv'], text=True).strip(),
                scope=__doc__, results=results), indent=2) + '\n')
    return int(any(r['status'] not in ('pass', 'control-detected') for r in results))


if __name__ == '__main__':
    raise SystemExit(main())
