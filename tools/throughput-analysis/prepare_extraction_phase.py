#!/usr/bin/env python3
"""Build typed ExtractIf experiments in ignored copies of a pinned revision.

No product file, compiler default or proof input is changed. Native tests and
allocation observations are optional; this tool performs no elapsed timing.
"""
import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[2]
KERNEL = Path(__file__).parent / 'experiments/extraction_phase.rs'
CONSUMER = Path(__file__).parent / 'experiments/extraction.rs'
VARIANTS = {
    'const-usize': ['jzap_phase_usize'],
    'const-nonzero': [],
    'nonzero-hint': ['jzap_phase_inline_hint'],
    'nonzero-auto': ['jzap_phase_inline_auto'],
    'nonzero-outline': ['jzap_phase_outline'],
    'nonzero-peel': ['jzap_phase_peel'],
    'nonzero-peel-hint': ['jzap_phase_peel', 'jzap_phase_inline_hint'],
    'usize-peel-hint': ['jzap_phase_usize', 'jzap_phase_peel', 'jzap_phase_inline_hint'],
}


def digest(paths):
    return hashlib.sha256(b''.join(p.name.encode() + b'\0' + p.read_bytes()
                                   for p in paths)).hexdigest()


def checked(command, log, *, cwd=ROOT, env=None):
    with log.open('w') as output:
        subprocess.run(command, cwd=cwd, env=env, stdout=output,
                       stderr=subprocess.STDOUT, check=True)


def selected_kernel(cfgs):
    """Materialize the maintained switches; builds use ordinary compiler flags."""
    text = KERNEL.read_text()
    start = text.index('#[cfg_attr(jzap_phase_inline_hint, inline)]')
    end = text.index('fn scan<', start)
    attr = ('#[inline]\n' if 'jzap_phase_inline_hint' in cfgs else
            '#[inline(never)]\n' if 'jzap_phase_outline' in cfgs else
            '' if 'jzap_phase_inline_auto' in cfgs else '#[inline(always)]\n')
    text = text[:start] + attr + text[end:]
    for marker, enabled in [('#[cfg(jzap_phase_peel)]', 'jzap_phase_peel' in cfgs),
                            ('#[cfg(jzap_phase_usize)]', 'jzap_phase_usize' in cfgs),
                            ('#[cfg(not(jzap_phase_usize))]', 'jzap_phase_usize' not in cfgs)]:
        assert text.count(marker) == 1
        start = text.index(marker)
        if enabled:
            text = text[:start] + text[start + len(marker):]
        else:
            opening = text.index('{', start)
            depth = 1
            end = opening + 1
            while depth:
                depth += (text[end] == '{') - (text[end] == '}')
                end += 1
            text = text[:start] + text[end:]
    assert 'jzap_phase_' not in text, 'Unreviewed experimental switch'
    return text


def install(source, cfgs):
    path = source / 'src/vec/compaction_driver.rs'
    text = path.read_text()
    start = text.index('pub(crate) fn extract_next<')
    opening = ') -> Result<Option<usize>, E> {'
    end = text.index(opening, start) + len(opening)
    tail = text[end:]
    assert tail.count('while state.read() < end') == 1 and tail.strip().endswith('}')
    assert tail.count('Ok(None)') == 1 and tail.count('state.untouched()') == 1
    signature = text[start:end].replace('mut step: F', 'step: F').replace('mut act: G', 'act: G')
    path.write_text(text[:start] + signature + '\n    extraction_phase::run(state, step, act)\n}\n'
                    '\n#[path = "extraction_phase.rs"]\nmod extraction_phase;\n')
    (source / 'src/vec/extraction_phase.rs').write_text(selected_kernel(cfgs))


def archive_from_repository(script):
    # The ignored source copy is below the repository, not itself a Git root.
    # Keep the benchmark runner unchanged except for this read-only archive cwd.
    text = script.read_text()
    old = "archive = subprocess.check_output(['git', 'archive', revision, 'src'], cwd=ROOT)"
    new = ("repository = subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], cwd=ROOT, text=True).strip()\n"
           "            archive = subprocess.check_output(['git', 'archive', revision, 'src'], cwd=repository)")
    assert text.count(old) == 1
    script.write_text(text.replace(old, new))


def main():
    if not __debug__:
        raise RuntimeError('Keep assertions enabled for experiment checks')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--checks', action='store_true', help='Debug/release Vec tests and allocation observations for four main type/entry controls')
    parser.add_argument('--timing-workspace', action='store_true', help='Prepare an ignored source copy for paired timing of nonzero-peel-hint')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to(ROOT / 'target'), 'Use ignored target/ storage'
    before = digest([Path(__file__), KERNEL, CONSUMER,
                     Path(__file__).parent / 'src/string_memory.rs'])
    out.mkdir(parents=True, exist_ok=False)
    revision = subprocess.check_output(['git', 'rev-parse', args.baseline + '^{commit}'], cwd=ROOT, text=True).strip()
    archive = subprocess.check_output(['git', 'archive', revision, 'src', 'tests',
                                       'examples', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT)
    baseline = out / 'baseline'
    baseline.mkdir()
    with tarfile.open(fileobj=io.BytesIO(archive)) as stream:
        stream.extractall(baseline, filter='data')
    common = ['rustc', '--edition=2024', '-O', '-Cpanic=abort', '-Ccodegen-units=1']
    checked([*common, '--crate-type=rlib', '--crate-name=jzap_old', '--cfg',
             'feature="alloc"', str(baseline / 'src/lib.rs'), '-o', str(out / 'libjzap_old.rlib')],
            out / 'baseline-build.log')
    results = []
    for name, cfgs in VARIANTS.items():
        work = out / name
        shutil.copytree(baseline, work)
        install(work, cfgs)
        source_paths = sorted((work / 'src').rglob('*.rs'))
        source_hash = digest(source_paths)
        checked([*common, '--crate-type=rlib', '--crate-name=jzap_new', '--cfg',
                 'feature="alloc"', str(work / 'src/lib.rs'), '-o', str(work / 'libjzap_new.rlib')],
                work / 'library.log')
        externs = ['--extern', f'jzap_old={out}/libjzap_old.rlib',
                   '--extern', f'jzap_new={work}/libjzap_new.rlib']
        checked([*common, '--crate-name=extraction_phase_consumer', '--emit=asm,llvm-ir,obj',
                 '--out-dir', str(work), *externs, str(CONSUMER)], work / 'consumer.log')
        assembly = (work / 'extraction_phase_consumer.s').read_text()
        symbols = subprocess.check_output(['nm', '-S', str(work / 'extraction_phase_consumer.o')], text=True)
        (work / 'symbols.txt').write_text(symbols)
        row = dict(variant=name, source_selection=cfgs, source_sha256=source_hash, functions={})
        for label, token in [('current', '7current'), ('previous', '8previous'), ('standard', '8standard')]:
            match = re.search(r'^(_ZN[^\n]*' + token + r'17h[^\n]*):\n(.*?)(?=^\s*\.size)',
                              assembly, re.M | re.S)
            assert match, ('consumer missing', name, label)
            (work / (label + '.s')).write_text(match[0])
            symbol = next(line.split() for line in symbols.splitlines() if line.endswith(' ' + match[1]))
            row['functions'][label] = dict(symbol=match[1], bytes=int(symbol[1], 16))
        row['outlined_scan_bytes'] = sum(int(line.split()[1], 16) for line in symbols.splitlines()
                                          if 'extraction_phase4scan17h' in line)
        if args.checks and name in ('const-usize', 'const-nonzero', 'nonzero-peel-hint', 'usize-peel-hint'):
            env = dict(os.environ, CARGO_TARGET_DIR=str(out / 'native-build'))
            profiles = {}
            for profile, options in [('debug', []), ('release', ['--release'])]:
                log = work / ('native-' + profile + '.log')
                checked(['cargo', 'test', '--locked', '--offline', *options, '--test', 'vec'],
                        log, cwd=work, env=env)
                counts = re.findall(r'test result: ok\. (\d+) passed', log.read_text())
                assert len(counts) == 1 and int(counts[0]) > 0
                profiles[profile] = int(counts[0])
            row['native'] = dict(status='pass', profiles=profiles)
            checked([*common, '--crate-name=extraction_phase_memory', '--cfg=jzap_memory_probe',
                     *externs, str(CONSUMER), '-o', str(work / 'memory')], work / 'memory-build.log')
            checked([str(work / 'memory')], work / 'memory.csv')
            observations = list(csv.DictReader((work / 'memory.csv').open()))
            assert len(observations) == 108
            grouped = {}
            for item in observations:
                grouped.setdefault((item['kind'], item['length']), []).append(item)
            for group in grouped.values():
                assert len(group) == 3 and {x['implementation'] for x in group} == {'old', 'new', 'std'}
                assert len({tuple(v for k, v in x.items() if k != 'implementation') for x in group}) == 1
                assert all(x['after'] == '0' and x['usable_after'] == '0' for x in group)
            row['allocation'] = dict(status='pass', observations=108, matching_triples=len(grouped))
        assert digest(source_paths) == source_hash, 'Experimental source changed during checks'
        row['source_unchanged'] = True
        results.append(row)
        print(name, row['functions']['current']['bytes'], row.get('native', {}), flush=True)
    if args.timing_workspace:
        work = out / 'timing-work'
        shutil.copytree(baseline, work)
        install(work, VARIANTS['nonzero-peel-hint'])
        shutil.copytree(Path(__file__).parent, work / 'tools/throughput-analysis',
                        ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
        archive_from_repository(work / 'tools/throughput-analysis/prepare_range.py')
    assert digest([Path(__file__), KERNEL, CONSUMER,
                   Path(__file__).parent / 'src/string_memory.rs']) == before
    same = (out / 'const-usize/current.s').read_bytes() == (out / 'const-nonzero/current.s').read_bytes()
    report = dict(status='complete', baseline=revision, compiler=subprocess.check_output(['rustc', '-vV'], text=True),
                  experiment_inputs_sha256=before, source_unchanged=True, native_allocation_checks=args.checks,
                  primary_assembly_identical=same, results=results,
                  timing_workspace=str(out / 'timing-work') if args.timing_workspace else None,
                  scope='Experimental library copies; no product proof, elapsed timing or adoption claim')
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    print(out / 'summary.json', flush=True)


if __name__ == '__main__':
    main()
