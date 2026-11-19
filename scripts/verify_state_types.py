#!/usr/bin/env python3
"""Verify original Compaction/Extraction/Span methods and emit only a caller include.

No product function is extracted, lowered or copied into a numeric state model.
The temporary Rust adapters call actual methods for pending caller projections.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
from scanner_tools import scanner
import re
import shutil
import subprocess
import tempfile
from verus_composition import run

ROOT = Path(__file__).resolve().parents[1]
METHODS = {
    'Compaction': {'full', 'first_kept', 'try_prefix', 'peek', 'read', 'written',
                   'previous_kept', 'remaining', 'restored_len', 'keep_next',
                   'remove_next', 'keep_live', 'remove_live'},
    'Span': {'full', 'try_new', 'clear', 'front', 'len', 'next', 'next_back'},
    'Extraction': {'try_range', 'read', 'written', 'end', 'remaining', 'untouched',
                   'keep_next', 'remove_next', 'stop', 'unvisited_len', 'restored_len'},
}


def declarations(root):
    rows = json.loads(subprocess.check_output([
        str(scanner('state_sites')), '--declarations',
        'src/compaction.rs', 'src/span.rs'], cwd=root, text=True))
    methods = [r for r in rows if r['kind'] == 'declaration']
    expected = {(owner, name) for owner, names in METHODS.items() for name in names}
    actual = {(r['context'], r['function']) for r in methods}
    if actual != expected or len(methods) != len(expected):
        raise RuntimeError('State declaration domain changed; review added/removed methods')
    if not all(r['detail']['direct_contract'] for r in methods):
        raise RuntimeError('State method is missing its direct Verus contract')
    fields = {'Span': {'front': 'Checked < usize >', 'back': 'Checked < usize >'},
              'Compaction': {'read': 'Checked < usize >', 'write': 'Checked < usize >',
                              'original': 'Checked < usize >'},
              'Extraction': {'read': 'Checked < usize >', 'deleted': 'Checked < usize >',
                             'end': 'Checked < usize >', 'total': 'Checked < usize >'}}
    structures = [r for r in rows if r['kind'] == 'struct_declaration']
    if len(structures) != 3 or {r['detail']['name'] for r in structures} != set(fields):
        raise RuntimeError('Closed state representation changed')
    for row in structures:
        detail = row['detail']
        if ({f['name']: f['type'] for f in detail['fields']} != fields[detail['name']]
                or any(f['visibility'] for f in detail['fields'])):
            raise RuntimeError('State fields must retain their private representation')
    for row in rows:
        if row['kind'] == 'unreviewed_declaration':
            raise RuntimeError('New state declaration requires domain review')
        if row['kind'] == 'module_declaration':
            raise RuntimeError('New state module requires domain review')
        if row['kind'] == 'item_macro' and not (
                row['detail']['path'] == 'verus'
                and row['detail']['attributes'] == ['cfg (verus_keep_ghost)']):
            raise RuntimeError('New state macro requires domain review')
    return [dict(file=r['file'], owner=r['context'], method=r['function'], line=r['line'])
            for r in methods]


def domain_controls(source_root, out):
    """Compiled scope defects test the AST gate, independently of solver proofs."""
    source = (source_root / 'src/compaction.rs').read_text()
    bypass = 'impl Compaction { fn unchecked_factory() -> Self { Self { read: Checked::new(usize::MAX), write: Checked::new(0), original: Checked::new(0) } } }'
    cases = [
        ('new-constructor', source + '\n' + bypass, 'declaration domain changed'),
        ('hidden-constructor-macro', source + '\nmacro_rules! hidden { () => { ' + bypass + ' }; }\nhidden!();', 'macro requires domain review'),
        ('exposed-state-field', source.replace('    read: Checked<usize>,', '    pub(crate) read: Checked<usize>,'), 'private representation'),
        ('new-state-module', source + '\nmod unchecked_child;\n', 'module requires domain review'),
        ('new-state-constant', source + '\nconst BAD: Compaction = Compaction { read: Checked::new(usize::MAX), write: Checked::new(0), original: Checked::new(0) };\n', 'declaration requires domain review'),
    ]
    controls = []
    for name, changed, expected in cases:
        with tempfile.TemporaryDirectory(prefix='jzap-state-domain-') as tmp:
            work = Path(tmp)
            shutil.copytree(source_root / 'src', work / 'src')
            (work / 'src/compaction.rs').write_text(changed)
            if name == 'new-state-module':
                child = work / 'src/compaction/unchecked_child.rs'
                child.parent.mkdir(exist_ok=True)
                child.write_text('use super::*;\n' + bypass)
            command = ['rustc', '--edition=2024', '--crate-type=rlib', '--crate-name=jzap_state_scope',
                       '--cfg=feature="alloc"', str(work / 'src/lib.rs'), '-o', str(work / 'libjzap.rlib')]
            result = subprocess.run(command, capture_output=True, text=True)
            (out / (name + '.compile.log')).write_text(result.stdout + result.stderr)
            if result.returncode:
                raise RuntimeError('Scope control must compile: ' + name)
            try:
                declarations(work)
            except RuntimeError as error:
                if expected not in str(error):
                    raise
                controls.append(dict(name=name, detected=True, compiled=True,
                                     method='independent AST scope check', diagnostic=str(error)))
            else:
                raise RuntimeError('Unknown state scope accepted: ' + name)
    return controls


def verify(source_root, out, controls=True):
    if not __debug__:
        raise RuntimeError('Verification requires Python assertions')
    source_root, out = source_root.resolve(), out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    (out / 'summary.json').unlink(missing_ok=True)
    adapter = ROOT / 'tools/direct-verus/state-adapters.rs'
    paths = sorted((source_root / 'src').rglob('*.rs'))
    paths += [adapter, Path(__file__), ROOT / 'scripts/verus_composition.py']
    paths += [ROOT / 'tools/panic-audit' / p for p in
              ('Cargo.toml', 'Cargo.lock', 'src/bin/state_sites.rs')]
    digest = lambda: hashlib.sha256(b''.join(str(p).encode() + b'\0' + p.read_bytes() for p in paths)).hexdigest()
    before = digest()
    subprocess.run(['cargo', 'build', '--locked', '--offline', '--manifest-path',
                    str(ROOT / 'tools/panic-audit/Cargo.toml'), '--bin', 'state_sites',
                    '--target-dir', str(ROOT / 'target/panic-inventory')], check=True)
    methods = declarations(source_root)
    binary = os.environ.get('JZAP_VERUS', '/workspace/.devtools/verus/verus-x86-linux/verus')
    # This input contains a path to maintained Rust calls, never generated bodies.
    calls = out / 'state-calls.rs'
    calls.write_text('include!(' + json.dumps(str(adapter), ensure_ascii=False) + ');\n')

    def check(root, name):
        command, result = run(binary, root, calls, 'jzap_state_contracts')
        log = result.stdout + result.stderr
        (out / (name + '.log')).write_text(log)
        verdict = re.findall(r'verification results:: (\d+) verified, (\d+) errors', log)
        return dict(command=command, composition_input=str(calls), exit_code=result.returncode, verification=verdict), log

    positive, log = check(source_root, 'state-types')
    if positive['exit_code'] or not positive['verification'] or any(int(e) for _, e in positive['verification']):
        raise RuntimeError('Actual state proof failed; see ' + str(out / 'state-types.log'))
    controls_run = controls
    controls = []
    cases = [
        ('compaction-restoration', 'wrapping_add(self.write.get(), self.remaining())', 'self.write.get()'),
        ('extraction-stop', 'self.end = self.read;', 'self.end = self.total;'),
        ('extraction-deletion-retirement',
         '        self.deleted = Checked::new(wrapping_add(self.deleted.get(), 1));',
         '        self.deleted = self.deleted;'),
        ('extraction-retained-deletion',
         '        // No element is removed, so only the scan index advances.\n        self.read = Checked::new(wrapping_add(self.read.get(), 1));',
         '        // Defect: retire a retained value.\n        self.read = Checked::new(wrapping_add(self.read.get(), 1));\n        self.deleted = Checked::new(wrapping_add(self.deleted.get(), 1));'),
        ('one-past-progress', 'if self.read.get() >= self.original.get() {', 'if self.read.get() > self.original.get() {'),
        ('unmodeled-state-panic', 'pub(crate) fn previous_kept(&self) -> usize {', 'pub(crate) fn previous_kept(&self) -> usize {\n        if self.write.get() == 1 { panic!("original state control"); }'),
        ('word-width-32', 'pub(crate) const fn full(original: usize) -> Self {', 'pub(crate) const fn full(original: usize) -> Self {\n        #[cfg(verus_keep_ghost)] proof! { assert(usize::MAX as int == u32::MAX as int); }'),
        ('word-width-64', 'pub(crate) const fn full(original: usize) -> Self {', 'pub(crate) const fn full(original: usize) -> Self {\n        #[cfg(verus_keep_ghost)] proof! { assert(usize::MAX as int == u64::MAX as int); }'),
    ]
    cases += [
        ('span-clear', 'self.front = self.back;', 'self.front = self.front;'),
        ('span-one-past', 'if self.front.get() >= self.back.get() {', 'if self.front.get() > self.back.get() {'),
        ('span-reverse', 'wrapping_sub(self.back.get(), 1)', 'wrapping_add(self.back.get(), 1)'),
    ]
    for name, old, new in cases if controls_run else []:
        with tempfile.TemporaryDirectory(prefix='jzap-state-source-') as tmp:
            work = Path(tmp)
            shutil.copytree(source_root / 'src', work / 'src')
            path = work / ('src/span.rs' if name.startswith('span-') else 'src/compaction.rs')
            text = path.read_text()
            if text.count(old) != (2 if name == 'span-one-past' else 1):
                raise RuntimeError('State control anchor needs review: ' + name)
            path.write_text(text.replace(old, new))
            row, log = check(work, name)
            if not (row['exit_code'] and row['verification'] and
                    any(int(e) for _, e in row['verification']) and 'error[E' not in log):
                raise RuntimeError('Control did not reach a solver failure: ' + name)
            controls.append(dict(name=name, detected=True, result=row))
    scope_controls = domain_controls(source_root, out) if controls_run else []
    if digest() != before:
        raise RuntimeError('Actual state proof inputs changed')
    report = dict(status='pass', kind='direct-product-source', contracts=len(methods),
                  verus_version=subprocess.check_output([binary, '--version'], text=True).strip(),
                  source_unchanged=True, input_sha256=before,
                  source_sha256={str(p.relative_to(source_root)): hashlib.sha256(p.read_bytes()).hexdigest()
                                 for p in (source_root / 'src').rglob('*.rs')},
                  result=positive, methods=methods, controls=controls, controls_run=controls_run,
                  domain_controls=scope_controls,
                  caller_include=str(calls), adapter_sha256=hashlib.sha256(adapter.read_bytes()).hexdigest(),
                  word_domain='Verus Either32Or64',
                  scope='Original Checked representation and all 31 closed compaction/extraction/iterator methods, with actual-call adapters for pending caller models. No product body projection. Payload provenance and the remaining caller projections are separate.')
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/state-types')
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--no-controls', action='store_true', help='Positive proof only, for nested caller-mutation checks; normal runs retain all eleven controls')
    args = parser.parse_args()
    report = verify(args.source_root, args.output, not args.no_controls)
    print(str(report['contracts'])+' original state methods and actual-call adapters verified; '+str(len(report['controls']))+
          ' solver controls and '+str(len(report['domain_controls']))+' compiled scope controls rejected')


if __name__ == '__main__':
    main()
