#!/usr/bin/env python3
"""Check that the owner extent and its capacity are closed state types.

The direct Verus target `vec-extent` proves that every constructor of
`src/extent.rs` and `src/capacity.rs` establishes the type invariant and that
every transition preserves it. That closes the types only while nothing else
can write their words. This check reads the declarations of both files from
the syntax tree and fails when:

- a struct, a field, its type or its visibility differs from the pinned form;
- a function is added or removed, or a verified one loses its contract;
- one of the few functions without a contract differs from its pinned body;
- a value is constructed or a field is assigned outside the verified
  constructors and transitions;
- a trait implementation, module, macro, constant or static is added;
- a trusted entry gains a caller.

Compiled defects (`--controls`, on by default) show that each rule rejects a
source that still builds. The proof itself is run by
`scripts/verify_direct_verus.py --target vec-extent --mutations`.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from scanner_tools import scanner

ROOT = Path(__file__).resolve().parents[1]
FILES = ('src/extent.rs', 'src/capacity.rs')
FIELDS = {
    'Extent': {'len': 'Checked < usize >', 'cap': 'Capacity < T >', 'limit': 'L'},
    'Capacity': {'count': 'Checked < usize >', 'element': 'PhantomData < T >'},
}
# Functions with a direct Verus contract: (file, implementing type, name).
VERIFIED = {
    ('src/extent.rs', 'Extent < T , Unlimited >', name) for name in ('unlimited', 'unlimited_clamped')
} | {
    ('src/extent.rs', 'Extent < T , L >', name) for name in (
        'empty', 'try_new', 'from_raw_parts', 'len', 'length', 'cap', 'capacity', 'limit',
        'max_len', 'set_len_within', 'set_len_clamped', 'shorten_to', 'holding', 'resized')
} | {
    ('src/capacity.rs', 'Capacity < T >', name) for name in ('empty', 'admitted', 'from_validated_raw', 'get')
} | {('src/capacity.rs', '', name) for name in ('logical_capacity', 'adopted_capacity')}
# The only functions that construct a value, and the only ones that assign a field.
CONSTRUCTORS = {
    ('src/extent.rs', name) for name in (
        'unlimited', 'unlimited_clamped', 'empty', 'try_new', 'from_raw_parts', 'holding', 'resized')
} | {('src/capacity.rs', name) for name in ('empty', 'admitted', 'from_validated_raw')}
TRANSITIONS = {('src/extent.rs', name) for name in ('set_len_within', 'set_len_clamped', 'shorten_to')}
# Functions without a contract. They can only read the words or call a
# constructor above; their bodies are pinned so that this stays true.
PINNED = {
    ('src/extent.rs', 'Extent < T , L >', 'clone'): '{*self}',
    ('src/capacity.rs', 'Capacity < T >', 'clone'): '{*self}',
    ('src/capacity.rs', 'Capacity < T >', 'try_new'):
        '{letlayout=crate::layout::array::<T>(count)?;Ok((Self::admitted(count)?,layout))}',
    ('src/capacity.rs', 'Capacity < T >', 'for_slice'):
        '{letcount=logical_capacity(mem::size_of::<T>(),values.len(),limit);unsafe{Self::from_validated_raw(count)}}',
    ('src/capacity.rs', 'Capacity < T >', 'layout'):
        '{letbytes=wrapping_mul(self.count.get(),mem::size_of::<T>());'
        'unsafe{Layout::from_size_align_unchecked(bytes,mem::align_of::<T>())}}',
    ('src/capacity.rs', 'Capacity < T >', 'sub'): '{self.count-rhs}',
}
TRAITS = {
    'src/extent.rs': ['Copy for Extent<T, L>', 'Clone for Extent<T, L>'],
    'src/capacity.rs': ['Copy for Capacity<T>', 'Clone for Capacity<T>',
                        'core::ops::Sub<Checked<usize>> for Capacity<T>'],
}
# Trusted entries: the calls outside proof-only code, by file.
TRUSTED = {
    'Extent::from_raw_parts(': {'src/vec/ownership.rs': 1},
    # In src/extent.rs the precondition is proved by Verus from the growth rule.
    'from_validated_raw(': {'src/capacity.rs': 1, 'src/extent.rs': 1, 'src/vec/ownership.rs': 1, 'src/deque.rs': 1},
}
VEC_FIELDS = {'ptr': 'NonNull < T >', 'extent': 'crate :: extent :: Extent < T , L >',
              'policy': 'P', 'marker': 'PhantomData < T >'}


def compact(text):
    return re.sub(r'\s+', '', text)


def product(text):
    """The source without its proof-only tail module."""
    return text.split('\n#[cfg(kani)]\nmod proofs {', 1)[0]


def scan(root, *arguments):
    return json.loads(subprocess.check_output([str(scanner('state_sites')), *arguments], cwd=root, text=True))


def declarations(root):
    root = Path(root)
    rows = scan(root, '--declarations', *FILES)
    structures = [r for r in rows if r['kind'] == 'struct_declaration']
    if sorted(r['detail']['name'] for r in structures) != sorted(FIELDS):
        raise RuntimeError('Closed owner representation changed')
    for row in structures:
        detail = row['detail']
        if ({f['name']: f['type'] for f in detail['fields']} != FIELDS[detail['name']]
                or any(f['visibility'] for f in detail['fields'])):
            raise RuntimeError('Owner state fields must retain their private representation')
    methods = [r for r in rows if r['kind'] == 'declaration']
    found = [(r['file'], r['context'], r['function']) for r in methods]
    if len(found) != len(set(found)) or set(found) != VERIFIED | set(PINNED):
        raise RuntimeError('Owner state declaration domain changed; review added/removed functions')
    for row in methods:
        key = (row['file'], row['context'], row['function'])
        if key in VERIFIED and not row['detail']['direct_contract']:
            raise RuntimeError('Owner state function is missing its direct Verus contract: ' + row['function'])
    for row in rows:
        if row['kind'] == 'module_declaration':
            raise RuntimeError('New owner state module requires domain review')
        if row['kind'] == 'item_macro' and not (
                row['detail']['path'] == 'verus'
                and row['detail']['attributes'] == ['cfg (verus_keep_ghost)']):
            raise RuntimeError('New owner state macro requires domain review')
        if row['kind'] == 'unreviewed_declaration' and row['detail']['item'] != 'trait implementation':
            raise RuntimeError('New owner state declaration requires domain review')
    for file, expected in TRAITS.items():
        text = product((root / file).read_text())
        actual = [m.group(1).strip() for m in re.finditer(
            r'^impl(?:<[^>]*>)? ([A-Za-z_:]+(?:<[^{]*>)? for [^{]+?) \{', text, re.M)]
        if actual != expected:
            raise RuntimeError('Owner state trait implementation set changed: ' + file)
    if sum(r['kind'] == 'unreviewed_declaration' for r in rows) != sum(map(len, TRAITS.values())):
        raise RuntimeError('Owner state trait implementation set changed')
    # Bodies, constructions and assignments from the complete syntax tree.
    sites = scan(root, *FILES)
    bodies = {(r['file'], r['context'], r['function']): compact(r['expression'])
              for r in sites if r['kind'] == 'body'}
    for key, expected in PINNED.items():
        if bodies.get(key) != expected:
            raise RuntimeError('Owner state function without a contract changed: ' + key[2])
    for row in sites:
        where = (row['file'], row['function'])
        if row['kind'] == 'field':
            constructor = compact(row['detail']['constructor'])
            if constructor in ('Self', 'Extent', 'Capacity') and where not in CONSTRUCTORS:
                raise RuntimeError('Owner state constructed outside a verified constructor: ' + row['function'])
        if row['kind'] == 'write' and where not in TRANSITIONS:
            raise RuntimeError('Owner state written outside a verified transition: ' + row['function'])
    # Whole values may be moved, never rebuilt: the struct names appear in an
    # expression position only inside their own files.
    for path in sorted((root / 'src').rglob('*.rs')):
        name = str(path.relative_to(root))
        text = path.read_text()
        if name.endswith('proofs.rs'):
            continue
        text = product(text)
        for entry, expected in TRUSTED.items():
            count = len(re.findall(r'(?<!fn )' + re.escape(entry), text))
            if count != expected.get(name, 0):
                raise RuntimeError('Trusted entry callers changed: ' + entry + ' in ' + name)
    vector = [r for r in scan(root, '--declarations', 'src/vec.rs')
              if r['kind'] == 'struct_declaration' and r['detail']['name'] == 'Vec']
    if len(vector) != 1 or ({f['name']: f['type'] for f in vector[0]['detail']['fields']} != VEC_FIELDS
                            or any(f['visibility'] for f in vector[0]['detail']['fields'])):
        raise RuntimeError('Vec must hold its length, capacity and limit only in the closed extent')
    return [dict(file=r['file'], owner=r['context'], function=r['function'], line=r['line'],
                 contract=(r['file'], r['context'], r['function']) in VERIFIED) for r in methods]


BYPASS = ('impl<T, L: LengthPolicy> Extent<T, L> { pub(crate) fn unchecked_factory(limit: L) -> Self '
          '{ Self { len: Checked::new(usize::MAX), cap: Capacity::empty(0), limit } } }')
SETTER = ('impl<T, L: LengthPolicy> Extent<T, L> { #[cfg_attr(verus_keep_ghost, verus_spec(ensures true,))] '
          'pub(crate) fn publish(&mut self, value: usize) { self.len = Checked::new(value); } }')
CONTROLS = [
    ('new-constructor', 'src/extent.rs', None, '\n' + BYPASS + '\n', 'declaration domain changed'),
    ('new-transition', 'src/extent.rs', None, '\n' + SETTER + '\n', 'declaration domain changed'),
    ('hidden-constructor-macro', 'src/extent.rs', None,
     '\nmacro_rules! hidden { () => { ' + BYPASS + ' }; }\nhidden!();\n', 'macro requires domain review'),
    ('exposed-state-field', 'src/extent.rs',
     '    len: Checked<usize>,\n    cap: Capacity<T>,', '    pub(crate) len: Checked<usize>,\n    cap: Capacity<T>,',
     'private representation'),
    ('exposed-capacity-field', 'src/capacity.rs',
     '    count: Checked<usize>,\n    element: PhantomData<T>,\n}',
     '    pub(crate) count: Checked<usize>,\n    element: PhantomData<T>,\n}', 'private representation'),
    ('new-state-module', 'src/extent.rs', None, '\nmod unchecked_child;\n', 'module requires domain review'),
    ('new-state-constant', 'src/capacity.rs', None,
     '\nconst BAD: Capacity<u64> = Capacity { count: Checked::new(usize::MAX), element: PhantomData };\n',
     'declaration requires domain review'),
    ('removed-contract', 'src/extent.rs',
     '''    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures
            final(self).cap_spec() == old(self).cap_spec(),
            final(self).limit_spec() == old(self).limit_spec(),
            final(self).len_spec()
                == if next <= old(self).len_spec() { next } else { old(self).len_spec() },
    ))]
    pub(crate) fn shorten_to(''', '    pub(crate) fn shorten_to(', 'missing its direct Verus contract'),
    ('unverified-constructor-body', 'src/capacity.rs',
     '        unsafe { Self::from_validated_raw(count) }\n    }\n    #[inline]\n    pub(crate) fn layout',
     '        let _ = count;\n        Self { count: Checked::new(usize::MAX), element: PhantomData }\n    }\n'
     '    #[inline]\n    pub(crate) fn layout', 'without a contract changed'),
    ('new-trait-implementation', 'src/extent.rs', None,
     '\nimpl<T, L: LengthPolicy> core::marker::Unpin for Extent<T, L> {}\n',
     'trait implementation set changed'),
    ('second-trusted-caller', 'src/vec/ownership.rs',
     '    pub fn into_std(self) -> alloc::vec::Vec<T> {',
     '    #[allow(dead_code)]\n    fn unchecked_owner(limit: L) -> crate::extent::Extent<T, L> {\n'
     '        unsafe { crate::extent::Extent::from_raw_parts(usize::MAX, crate::capacity::Capacity::empty(0), limit) }\n    }\n'
     '    pub fn into_std(self) -> alloc::vec::Vec<T> {', 'Trusted entry callers changed'),
    ('separate-length-field', 'src/vec.rs',
     '    extent: crate::extent::Extent<T, L>,\n    policy: P,',
     '    extent: crate::extent::Extent<T, L>,\n    #[allow(dead_code)]\n    shadow: usize,\n    policy: P,',
     'only in the closed extent'),
]


def controls(source_root, out):
    """Each defect compiles and is rejected by the declaration check alone."""
    results = []
    for name, file, old, new, expected in CONTROLS:
        with tempfile.TemporaryDirectory(prefix='jzap-extent-domain-') as temporary:
            work = Path(temporary)
            shutil.copytree(source_root / 'src', work / 'src')
            path = work / file
            text = path.read_text()
            if old is None:
                head, mark, tail = text.partition('\n#[cfg(kani)]\nmod proofs {')
                changed = head + new + mark + tail
            else:
                if text.count(old) != 1:
                    raise RuntimeError('Owner extent control anchor needs review: ' + name)
                changed = text.replace(old, new)
            if name == 'separate-length-field':
                changed = changed.replace('extent: crate::extent::Extent::unlimited(),',
                                          'extent: crate::extent::Extent::unlimited(),\n            shadow: 0,')
                changed = changed.replace('extent: crate::extent::Extent::empty(limit),',
                                          'extent: crate::extent::Extent::empty(limit),\n            shadow: 0,')
                for other in ('src/vec/ownership.rs',):
                    source = (work / other).read_text()
                    (work / other).write_text(re.sub(r'(\n\s+)(policy,\n\s+marker: PhantomData,)',
                                                     r'\1shadow: 0,\1\2', source))
            path.write_text(changed)
            if name == 'new-state-module':
                child = work / 'src/extent/unchecked_child.rs'
                child.parent.mkdir(exist_ok=True)
                child.write_text('use super::*;\n' + BYPASS + '\n')
            command = ['rustc', '--edition=2024', '--crate-type=rlib', '--crate-name=jzap_extent_scope',
                       '--cfg=feature="alloc"', str(work / 'src/lib.rs'), '-o', str(work / 'libjzap.rlib')]
            result = subprocess.run(command, capture_output=True, text=True)
            (out / (name + '.compile.log')).write_text(result.stdout + result.stderr)
            if result.returncode:
                raise RuntimeError('Owner extent control must compile: ' + name)
            try:
                declarations(work)
            except RuntimeError as error:
                if expected not in str(error):
                    raise
                results.append(dict(name=name, detected=True, compiled=True,
                                    method='independent AST scope check', diagnostic=str(error)))
            else:
                raise RuntimeError('Unknown owner extent scope accepted: ' + name)
    return results


def verify(source_root=ROOT, out=None, run_controls=True):
    if not __debug__:
        raise RuntimeError('Verification requires Python assertions')
    source_root = Path(source_root).resolve()
    out = Path(out or ROOT / 'target/verification/owner-extent').resolve()
    out.mkdir(parents=True, exist_ok=True)
    (out / 'summary.json').unlink(missing_ok=True)
    paths = sorted((source_root / 'src').rglob('*.rs')) + [Path(__file__)]
    paths += [ROOT / 'tools/panic-audit' / p for p in ('Cargo.toml', 'Cargo.lock', 'src/bin/state_sites.rs')]
    digest = lambda: hashlib.sha256(b''.join(str(p).encode() + b'\0' + p.read_bytes() for p in paths)).hexdigest()
    before = digest()
    functions = declarations(source_root)
    scope_controls = controls(source_root, out) if run_controls else []
    if digest() != before:
        raise RuntimeError('Owner extent inputs changed')
    report = dict(status='pass', kind='closed-state-declarations', source_unchanged=True, input_sha256=before,
                  functions=functions, contracts=sum(f['contract'] for f in functions),
                  pinned_without_contract=sorted(key[2] + ' (' + key[0] + ')' for key in PINNED),
                  trusted_entries={entry: files for entry, files in TRUSTED.items()},
                  controls=[], controls_run=run_controls, domain_controls=scope_controls,
                  scope='Declarations of src/extent.rs and src/capacity.rs: private fields, the verified '
                        'constructor and transition set, pinned bodies of the functions without a contract, '
                        'construction and assignment sites, trait implementations and the callers of the two '
                        'trusted entries. The invariant proof is the direct Verus target vec-extent.')
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/owner-extent')
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--no-controls', action='store_true', help='Declaration check only')
    args = parser.parse_args()
    report = verify(args.source_root, args.output, not args.no_controls)
    print(str(report['contracts']) + ' owner state functions with a direct contract, '
          + str(len(report['pinned_without_contract'])) + ' pinned without one; '
          + str(len(report['domain_controls'])) + ' compiled scope controls rejected')


if __name__ == '__main__':
    main()
