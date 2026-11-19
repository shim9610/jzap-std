#!/usr/bin/env python3
"""Verify all integer arithmetic implementations, emitted ABI, and exit-free links."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'tools/arithmetic-analysis/Cargo.toml'
TYPES = ['u8', 'u16', 'u32', 'u64', 'u128', 'usize', 'i8', 'i16', 'i32', 'i64', 'i128', 'isize']
OPS = {'add': '+', 'sub': '-', 'mul': '*'}


def run(command, log, env=None, timeout=240):
    with log.open('w') as output:
        try:
            result = subprocess.run(command, cwd=ROOT, env=env, stdout=output,
                                    stderr=output, timeout=timeout)
            return result.returncode
        except subprocess.TimeoutExpired:
            return 124


def bodies(text):
    found = dict(re.findall(r'^(\w+):\n(.*?)^\.Lfunc_end\d+:', text, re.M | re.S))
    aliases = re.findall(r'^(\w+) = (\w+)$', text, re.M)
    aliases += re.findall(r'^\s*\.set\s+(\w+),\s*(\w+)', text, re.M)
    for _ in aliases:
        for alias, target in aliases:
            if target in found:
                found[alias] = found[target]
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/arithmetic')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    (out / 'verification.json').unlink(missing_ok=True)
    path = ROOT / 'src/arithmetic.rs'
    source = path.read_text()
    sha = hashlib.sha256(source.encode()).hexdigest()
    extra_sources = {p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest()
                     for p in ['src/arithmetic/checked.rs']}
    # Each target-width invocation registers (type, division error) pairs.
    invocations = re.findall(r'integers!\(\s*(.*?)\);', source, re.S)
    assert invocations
    for registered in invocations:
        assert re.findall(r'\(\s*(\w+)\s*,\s*DivisionError(?:64)?\s*\)', registered) == TYPES
    # Verify the executable crate and its actual-Rust harnesses. No operation
    # body is extracted or substituted into a generated arithmetic program.
    source_command = ['python3', 'scripts/verify_source.py', '--group', 'arithmetic',
                      '--mutations', '--output', str(out / 'source')]
    assert run(source_command, out / 'source.log', timeout=600) == 0
    direct = json.loads((out / 'source/summary.json').read_text())
    assert direct['status'] == 'pass' and direct['source_unchanged'] and direct['mutations_run']
    group = direct['groups'][0]
    assert group['group'] == 'arithmetic' and len(group['kani']['harnesses']) == 11
    actual = group['verus']['targets'][0]
    assert actual['target'] == 'integer' and actual['verification']['success']
    assert len(group['verus']['controls']) == 7 and len(direct['controls']) == 6
    env = os.environ.copy()
    env.pop('RUSTFLAGS', None)
    env.pop('CARGO_ENCODED_RUSTFLAGS', None)
    report = dict(source_sha256=sha, extra_source_sha256=extra_sources, types=TYPES,
        kani_source_sha256=hashlib.sha256((ROOT / 'src/arithmetic_proofs.rs').read_bytes()).hexdigest(),
        wrapping_contract='Modulo 2^BITS; division rejects zero and wraps signed MIN / -1.',
        base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        rustc=subprocess.check_output(['rustc', '-Vv'], text=True).strip(),
        direct_source=direct, mathematical_add_sub_mul_contracts=36, generic_checked_add_sub_mul_contracts=3, profiles=[])
    normalize = lambda body: re.sub(r'\.LBB\d+_\d+', '.LABEL', body)
    for profile in ['release', 'release-checked', 'debug']:
        profile_env = env.copy()
        profile_env.pop('CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS', None)
        profile_env.pop('CARGO_PROFILE_RELEASE_LTO', None)
        if profile == 'release-checked':
            profile_env['CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS'] = 'true'
        if profile == 'debug':
            profile_env['CARGO_PROFILE_DEV_DEBUG'] = '1'
        work = out / profile
        work.mkdir(parents=True, exist_ok=True)
        target = work / 'build'
        flags = [] if profile == 'debug' else ['--release']
        build = ['cargo', 'rustc', '--locked', '--offline', '--manifest-path', str(MANIFEST),
                 '--target-dir', str(target), '--lib', *flags, '--', '--emit=asm']
        assert run(build, work / 'codegen-build.log', profile_env) == 0
        directory = target / ('debug' if profile == 'debug' else 'release')
        # Library and executable share a Cargo package prefix. Select the actual
        # exported wrapper definitions even after bin assembly was requested.
        asm = max((p for p in (directory / 'deps').glob('jzap_arithmetic_analysis-*.s')
                   if re.search(r'^(?:jzap_add:|jzap_add =|\s*\.set\s+jzap_add,)', p.read_text(), re.M)), key=lambda p: p.stat().st_mtime)
        parsed = bodies(asm.read_text())
        if profile != 'debug':
            for op in OPS:
                assert normalize(parsed['native_' + op]) == normalize(parsed['jzap_' + op]), (profile, op)
                assert normalize(parsed['bounded_plain_' + op]) == normalize(parsed['bounded_jzap_' + op]), (profile, op)
                assert normalize(parsed['native_wrap_' + op]) == normalize(parsed['jzap_wrap_' + op]), (profile, op, 'wrapping')
                if profile == 'release':
                    assert normalize(parsed['plain_' + op]) == normalize(parsed['jzap_wrap_' + op]), (profile, op, 'plain wrapping')
            for op in [*OPS, 'div']:
                assert normalize(parsed['native_' + op]) == normalize(parsed['jzap_' + op]), (profile, op, 'checked')
            assert normalize(parsed['native_wrap_div']) == normalize(parsed['jzap_wrap_div']), profile
            for op in [*OPS, 'div', 'rem']:
                assert normalize(parsed['native_' + op]) == normalize(parsed['checked_type_' + op]), (profile,op,'Checked ABI')
            assert normalize(parsed['native_rem']) == normalize(parsed['jzap_rem']), profile
        # Build all timing executables before any measurements start.
        if profile != 'debug':
            timing = ['cargo', 'build', '--locked', '--offline', '--manifest-path', str(MANIFEST),
                      '--target-dir', str(target), '--bin', 'jzap-arithmetic-analysis', '--release']
            assert run(timing, work / 'timing-build.log', profile_env) == 0
        link_build = ['cargo', 'build', '--locked', '--offline', '--manifest-path', str(MANIFEST),
                      '--target-dir', str(work / 'link-build'), '--features', 'link-proof', '--lib', *flags]
        assert run(link_build, work / 'link-build.log', profile_env) == 0
        archive = work / 'link-build' / ('debug' if profile == 'debug' else 'release') / 'libjzap_arithmetic_analysis.a'
        link = ['cc', '-O2', '-ffunction-sections', '-fdata-sections',
                'tools/arithmetic-analysis/driver.c', str(archive), '-Wl,--gc-sections', '-o', str(work / 'audit')]
        assert run(link, work / 'link.log') == 0, (profile, (work / 'link.log').read_text())
        assert run([str(work / 'audit')], work / 'runtime.log') == 0
        negative = link[:-1] + [str(work / 'negative'), '-DARITHMETIC_NEGATIVE_CONTROL']
        assert run(negative, work / 'negative-link.log') != 0
        assert 'jzap_arithmetic_panic_reachable' in (work / 'negative-link.log').read_text()
        imports = subprocess.check_output(['nm', '-u', str(work / 'audit')], text=True)
        assert not re.search(r'\b(?:malloc|calloc|realloc|free|abort|exit|jzap_arithmetic_panic_reachable)(?:@|\b)', imports)
        report['profiles'].append(dict(name=profile, link='pass', negative_control='detected',
            cases=1088, imports=imports.splitlines(),
            assembly_sha256=hashlib.sha256(asm.read_bytes()).hexdigest(),
            functions={name: dict(instructions=len(re.findall(r'^\t[a-z]', parsed[name], re.M)),
                        body=parsed[name]) for op in [*OPS, 'div'] for name in ('plain_'+op, 'native_'+op, 'jzap_'+op, 'native_wrap_'+op, 'jzap_wrap_'+op)}))
        print(profile, 'link/codegen pass', flush=True)
    assert hashlib.sha256(path.read_bytes()).hexdigest() == sha
    assert all(hashlib.sha256((ROOT/p).read_bytes()).hexdigest() == sha for p,sha in extra_sources.items())
    (out / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
