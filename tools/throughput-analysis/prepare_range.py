#!/usr/bin/env python3
"""Compare actual range normalization, flattening or initialization with a revision and std.

Analysis only: archives/copies product Rust and appends a call-only visibility
shim. It generates no proof input. All output stays in ignored target/ storage.
"""
import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[2]
BENCHES = {name: ROOT / ('tools/throughput-analysis/experiments/' + name + '.rs') for name in ('range', 'flatten', 'initialize', 'collect', 'setter', 'btree-search', 'compaction', 'extraction')}
BENCHES['splice-fill'] = BENCHES['extraction']
SHIM = '''
#[inline]
pub fn proof_codegen_range<R: core::ops::RangeBounds<usize>>(
    r: R, n: usize,
) -> Result<(usize, usize), Error> { vec::edit::checked_range(r, n) }
'''


def digest(paths):
    return hashlib.sha256(b''.join(p.name.encode() + b'\0' + p.read_bytes() for p in paths)).hexdigest()


def selected_profiles(profile):
    return {'default', 'aligned'} if profile == 'both' else {profile}


def main():
    if not __debug__:
        raise RuntimeError('Run without -O/PYTHONOPTIMIZE; result assertions must remain enabled')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--benchmark', choices=BENCHES, default='range')
    parser.add_argument('--baseline', required=True, help='Pinned Git commit')
    parser.add_argument('--output', type=Path, required=True, help='Fresh directory under target/')
    parser.add_argument('--profile', choices=['default', 'aligned', 'loops32', 'both'], default='both')
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--build-only', action='store_true')
    mode.add_argument('--entry-only', action='store_true', help='Count stage_excess entries in disposable instrumented libraries; splice-fill only, no timing or proof')
    mode.add_argument('--memory-only', action='store_true', help='Fresh build and isolated allocation observations; no timing runs')
    mode.add_argument('--run-only', action='store_true', help='Run matching previously built inputs without compiling')
    parser.add_argument('--memory', action='store_true', help='Separate Linux System allocation observer for collection/edit comparisons')
    parser.add_argument('--overflow-checks', action='store_true', help='Apply overflow-checks=yes to both libraries and the comparison consumer')
    args = parser.parse_args()
    if args.entry_only and args.benchmark != 'splice-fill':
        raise ValueError('--entry-only requires --benchmark splice-fill')
    args.memory = args.memory or args.memory_only
    if args.entry_only and args.memory:
        raise ValueError('--entry-only cannot be combined with --memory')
    if args.memory and args.benchmark not in ('collect','compaction','extraction','splice-fill'):
        raise ValueError('--memory requires --benchmark collect, compaction, extraction or splice-fill')
    bench = BENCHES[args.benchmark]
    consumer_flags = ['--cfg','jzap_splice_fill_probe'] if args.benchmark == 'splice-fill' else []
    cases = {'range': 10, 'flatten': 4, 'initialize': 6, 'collect': 6, 'setter': 3, 'btree-search': 6, 'compaction': 8, 'extraction': 12, 'splice-fill': 12}[args.benchmark]
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        raise ValueError('Use ignored target/ storage')
    if not args.run_only:
        out.mkdir(parents=True, exist_ok=False)
    revision = subprocess.check_output(['git', 'rev-parse', args.baseline + '^{commit}'], cwd=ROOT, text=True).strip()
    paths = sorted((ROOT / 'src').rglob('*.rs'))
    before = digest(paths)
    metadata = dict(baseline=revision, current_source_sha256=before,
                    overflow_checks=args.overflow_checks, benchmark=args.benchmark, benchmark_sha256=digest([bench]),
                    compiler=subprocess.check_output(['rustc', '-vV'], text=True),
                    batches=os.environ.get('JZAP_RANGE_BATCHES', 'auto:fastest-to-2s' if args.benchmark in ('btree-search', 'compaction', 'extraction', 'splice-fill') else '250000' if args.benchmark == 'setter' else '50000' if args.benchmark in ('initialize', 'collect') else '156250'),
                    processes_per_profile=3 if args.benchmark in ('initialize', 'collect', 'setter', 'btree-search', 'compaction', 'extraction', 'splice-fill') else 1, profiles=[])
    # Build every binary before timing any binary. Cargo/product builds remain
    # stable Rust. Bootstrap is confined to the diagnostic std::slice::range API.
    if args.run_only:
        recorded = json.loads((out / 'summary.json').read_text())
        for field in ('baseline','current_source_sha256','benchmark','benchmark_sha256','compiler','overflow_checks','batches'):
            assert recorded[field] == metadata[field], ('Build inputs changed', field)
        expected = selected_profiles(args.profile)
        assert {p['name'] for p in recorded['profiles']} == expected, 'Profile selection changed'
        for profile in recorded['profiles']:
            assert profile['binary_sha256'] == digest([out / profile['name']]), 'Binary changed'
        metadata = recorded
    else:
        with tempfile.TemporaryDirectory(prefix='jzap-range-') as temporary:
            work = Path(temporary)
            old, new = work / 'old', work / 'new'
            old.mkdir()
            new.mkdir()
            shutil.copytree(ROOT / 'src', new / 'src')
            archive = subprocess.check_output(['git', 'archive', revision, 'src'], cwd=ROOT)
            with tarfile.open(fileobj=io.BytesIO(archive)) as stream:
                stream.extractall(old, filter='data')
            common = ['rustc', '--edition=2024', '-O', '-C', 'panic=abort', '-C', 'codegen-units=1']
            if args.overflow_checks:
                common += ['-C', 'overflow-checks=yes']
            for label, source in [('old', old), ('new', new)]:
                file = source / 'src/lib.rs'
                file.write_text(file.read_text() + (SHIM if args.benchmark == 'range' else ''))
                if args.entry_only:
                    file.write_text(file.read_text()+'\npub static STAGE_ENTRIES: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);\n')
                    stage=source/'src/vec/edit.rs'
                    text=stage.read_text()
                    anchor='        // Stage only excess beyond the drained hole.'
                    assert text.count(anchor)==1, 'Staging entry instrumentation anchor changed'
                    stage.write_text(text.replace(anchor,'        crate::STAGE_ENTRIES.fetch_add(1,core::sync::atomic::Ordering::Relaxed);\n'+anchor))
                command = [*common, '--crate-name', 'jzap_' + label, '--crate-type', 'rlib',
                           '--cfg', 'feature="alloc"', str(file), '-o', str(out / ('libjzap_' + label + '.rlib'))]
                subprocess.run(command, check=True)
            for profile, flags in [('default', []), ('aligned', ['-C', 'llvm-args=-align-all-functions=6']), ('loops32', ['-C', 'llvm-args=-align-loops=32'])]:
                if args.memory_only or args.entry_only or profile not in selected_profiles(args.profile):
                    continue
                command = [*common, '--crate-name', 'range_timing', *flags, *consumer_flags]
                for label in ('old', 'new'):
                    command += ['--extern', f'jzap_{label}={out}/libjzap_{label}.rlib']
                subprocess.run([*command, str(bench), '-o', str(out / profile)], check=True,
                               env=dict(os.environ, RUSTC_BOOTSTRAP='1') if args.benchmark == 'range' else dict(os.environ))
                metadata['profiles'].append(dict(name=profile, flags=flags, binary_sha256=digest([out / profile])))
            if args.entry_only:
                command=[*common,'--crate-name','stage_entries','--cfg','jzap_entry_probe',*consumer_flags]
                for label in ('old','new'):
                    command+=['--extern',f'jzap_{label}={out}/libjzap_{label}.rlib']
                subprocess.run([*command,str(bench),'-o',str(out/'entries')],check=True)
            if args.memory:
                command=[*common,'--crate-name','collection_memory','--cfg','jzap_memory_probe',*consumer_flags]
                for label in ('old','new'):
                    command += ['--extern',f'jzap_{label}={out}/libjzap_{label}.rlib']
                subprocess.run([*command,str(bench),'-o',str(out/'memory')],check=True)
    if not args.build_only and not args.memory_only and not args.entry_only:
        for profile in metadata['profiles']:
            file = out / (profile['name'] + '.csv')
            with file.open('w') as output:
                if args.benchmark in ('initialize', 'collect', 'setter', 'btree-search', 'compaction', 'extraction', 'splice-fill'):
                    # Independent processes keep allocator state and process
                    # history from crossing between corpus seeds.
                    for seed in range(1, 4):
                        result = subprocess.run([str(out / profile['name'])],
                            capture_output=True, text=True, check=True,
                            env=dict(os.environ, JZAP_RANGE_SEED=str(seed)))
                        lines = result.stdout.splitlines(keepends=True)
                        assert lines[0] == 'seed,case,implementation,seconds,operations,checksum\n'
                        output.writelines(lines if seed == 1 else lines[1:])
                else:
                    subprocess.run([str(out / profile['name'])], stdout=output, check=True)
            rows = list(csv.DictReader(file.open()))
            assert len(rows) == cases * 9 and all(float(r['seconds']) >= 1 for r in rows)
            for seed in range(1, 4):
                for case in range(cases):
                    group = [r for r in rows if int(r['seed']) == seed and int(r['case']) == case]
                    assert {r['implementation'] for r in group} == {'std', 'old', 'new'}
                    assert len({r['operations'] for r in group}) == len({r['checksum'] for r in group}) == 1
    if args.entry_only:
        with (out/'entries.csv').open('w') as log:
            subprocess.run([str(out/'entries')],stdout=log,check=True)
        rows=list(csv.DictReader((out/'entries.csv').open()))
        assert len(rows)==72
        for row in rows:
            expected=int(row['kind'])==5 and int(row['length'])>0
            assert int(row['stage_entries'])==int(expected), ('Uncovered staging condition',row)
        metadata['entry_observations']=len(rows)
        metadata['instrumentation']='AtomicUsize increment at original stage_excess entry in disposable library copies; not timing or proof input'
    if args.memory and not args.build_only:
        with (out/'memory.csv').open('w') as log:
            subprocess.run([str(out/'memory')],stdout=log,check=True)
        observed=list(csv.DictReader((out/'memory.csv').open()))
        assert len(observed)==(108 if args.benchmark in ('extraction','splice-fill') else 72 if args.benchmark=='compaction' else 216)
        assert all(int(r['after'])==int(r['usable_after'])==0 for r in observed)
        metadata['memory_observations']=len(observed)
    assert digest(paths) == before, 'Product sources changed during comparison'
    metadata['status'] = 'built' if args.build_only else 'pass'
    metadata['completed_phases'] = ['build'] if args.build_only else ['entry'] if args.entry_only else ['memory'] if args.memory_only else ['timing','memory'] if args.memory else ['timing']
    (out / 'summary.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(out / 'summary.json')


if __name__ == '__main__':
    main()
