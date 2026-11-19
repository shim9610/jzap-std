#!/usr/bin/env python3
"""Reproduce and check x86_64 Linux release assembly, no LTO/inline(always)."""
import hashlib, json, pathlib, re, shutil, subprocess, sys
root = pathlib.Path(__file__).resolve().parents[1]
nightly = '--nightly' in sys.argv
out = root / 'target/verification/box-analysis'
out.mkdir(parents=True, exist_ok=True)
target = root / 'tools/box-analysis/target' / ('nightly' if nightly else '.')
command = ['cargo'] + (['+nightly-2026-10-30'] if nightly else []) + [
    'rustc', '--locked', '--offline', '--manifest-path', 'tools/box-analysis/Cargo.toml',
    '--target-dir', str(target), '--release', '--lib']
if nightly: command += ['--features', 'nightly']
command += ['--', '--emit=asm,llvm-ir']
label = 'nightly' if nightly else 'after'
with (out / f'build-{label}.log').open('w') as log:
    subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True)
asm = max((p for p in (target / 'release').rglob('jzap_box_analysis-*.s')
           if re.search(r'^jzap_new:', p.read_text(), re.M)),
          key=lambda p: p.stat().st_mtime)
shutil.copyfile(asm, out / f'{label}.s')
shutil.copyfile(asm.with_suffix('.ll'), out / f'{label}.ll')

def bodies(text):
    result = dict(re.findall(r'^((?:std|jzap|varlit|nightly)_\w+):\n(.*?)^\.Lfunc_end\d+:', text, re.M | re.S))
    aliases = re.findall(r'^\s*\.set\s+(\w+),\s*(\w+)', text, re.M)
    aliases += re.findall(r'^(\w+) = (\w+)$', text, re.M)
    for alias, name in aliases:
        if name in result: result[alias] = result[name]
    return result

def describe(body):
    return dict(instructions=len(re.findall(r'^\t[a-z][a-z0-9]+\s', body, re.M)),
                saved_registers=re.findall(r'^\tpushq\s+(%\w+)', body, re.M),
                max_cfa_bytes=max([8]+[int(n) for n in re.findall(r'\.cfi_def_cfa_offset (\d+)', body)]),
                calls=re.findall(r'^\t(?:callq|jmpq)\s+(.*)', body, re.M))
text=(out/f'{label}.s').read_text()
functions=bodies(text)
for name in ['jzap_new','jzap_clone','jzap_new_option','jzap_clone_option','jzap_drop','jzap_copy']:
    assert name in functions, name
    assert not re.search('budget|reserve|commit|handle_alloc_error|panic|len_mismatch_fail', functions[name]), name
assert functions['jzap_drop'] == functions['std_drop']
normalize_iteration = lambda body: re.sub(r'\.LBB\d+_\d+', '.LABEL', body)
assert normalize_iteration(functions['jzap_iteration']) == normalize_iteration(functions['std_iteration'])
for kind in ['u8', 'u64', 'three', 'unit']:
    normalize = lambda body: re.sub(r'\.LBB\d+_\d+', '.LABEL', body)
    assert normalize(functions['jzap_layout_' + kind]) == normalize(functions['std_layout_' + kind]), kind
assert 'memcpy' in functions['std_copy'] and 'memcpy' in functions['jzap_copy']
assert 'handle_alloc_error' in functions['std_new']
assert 'memcpy' in functions['jzap_clone_array_4k']
assert 'memset' in functions['jzap_init_array_4k']
for name in ['jzap_clone_array_4k','jzap_init_array_4k']:
    assert describe(functions[name])['max_cfa_bytes'] <= 64, name
if nightly:
    normalized=lambda body: re.sub(r'\.LBB\d+_\d+', '.LABEL', body)
    assert normalized(functions['nightly_try_new']) == normalized(functions['jzap_new_option'])
report = dict(command=command, assembly_sha256=hashlib.sha256(text.encode()).hexdigest(),
              functions={k:describe(v) for k,v in functions.items()})
(out/f'{label}-summary.json').write_text(json.dumps(report,indent=2))
print(f'{label}: allocation/copy/drop codegen checks passed; {len(functions)} wrappers')
