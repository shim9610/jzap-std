#!/usr/bin/env python3
"""Compare actual BTree consumers and paired callees, retaining all output in target/."""
import argparse
import collections
import io
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[2]
CONSUMERS = '''extern crate jzap;
use jzap::{collections::btree_map::BTreeMap,budget::Budget,Error};
#[unsafe(no_mangle)] pub fn append(a:&mut BTreeMap<u64,u64>,b:&mut BTreeMap<u64,u64>)->Result<(),Error>{a.try_append(b)}
#[unsafe(no_mangle)] pub fn append_text(a:&mut BTreeMap<String,u64>,b:&mut BTreeMap<String,u64>)->Result<(),Error>{a.try_append(b)}
#[unsafe(no_mangle)] pub fn append_budget<'a>(a:&mut BTreeMap<u64,u64,Budget<'a>>,b:&mut BTreeMap<u64,u64,Budget<'a>>)->Result<(),Error>{a.try_append(b)}
#[unsafe(no_mangle)] pub fn plan(a:&BTreeMap<u64,u64>,b:&BTreeMap<u64,u64>,n:usize)->Result<(usize,usize),Error>{jzap::collections::btree_map::analysis_plan(a,b,n)}
#[unsafe(no_mangle)] pub fn plan_text(a:&BTreeMap<String,u64>,b:&BTreeMap<String,u64>,n:usize)->Result<(usize,usize),Error>{jzap::collections::btree_map::analysis_plan(a,b,n)}
#[unsafe(no_mangle)] pub fn insert(a:&mut BTreeMap<u64,u64>,k:u64,v:u64)->bool{a.try_insert(k,v).is_ok()}
#[unsafe(no_mangle)] pub fn remove(a:&mut BTreeMap<u64,u64>,k:&u64)->Option<u64>{a.remove(k)}
#[unsafe(no_mangle)] pub fn remove_text(a:&mut BTreeMap<String,u64>,k:&String)->Option<u64>{a.remove(k)}
#[unsafe(no_mangle)] pub fn split(a:&mut BTreeMap<u64,u64>,k:&u64)->bool{a.try_split_off(k).is_ok()}
'''
NAMES = re.findall(r'fn (\w+)(?:<[^>]*>)?\(', CONSUMERS)


def read_object(path):
    asm = subprocess.check_output(['objdump', '-dr', str(path)], text=True)
    sections, current = {}, None
    for line in asm.splitlines():
        if line.startswith('Disassembly of section '):
            current = line[23:-1]
            sections[current] = []
        elif current and re.match(r'\s*[0-9a-f]+:', line):
            sections[current].append(re.sub(r'^\s*[0-9a-f]+:', '', line).strip())
    symbols = {}
    for line in subprocess.check_output(['objdump', '-t', str(path)], text=True).splitlines():
        fields = line.split()
        if len(fields) >= 6:
            symbols[fields[-1]] = fields[-3]
    return sections, symbols, asm


def compare(old, new):
    a, roots_a, _ = old
    b, roots_b, _ = new
    queue = collections.deque((roots_a[name], roots_b[name]) for name in NAMES)
    mapping, reverse, failures, reordered = {}, {}, [], []
    while queue:
        left, right = queue.popleft()
        if left in mapping:
            if mapping[left] != right:
                failures.append(['inconsistent callee mapping', left, right])
            continue
        if right in reverse and reverse[right] != left:
            failures.append(['non-bijective callee mapping', left, right])
            continue
        mapping[left], reverse[right] = right, left
        x, y = a[left], b[right]
        def instructions(rows, section):
            return [row.replace('<' + section.removeprefix('.text.'), '<SELF')
                    for row in rows if not row.startswith('R_')]
        ix, iy = instructions(x, left), instructions(y, right)
        if collections.Counter(ix) != collections.Counter(iy):
            failures.append(['instruction multiset changed', left, right])
        elif ix != iy:
            reordered.append(left)
        rx = [row for row in x if row.startswith('R_')]
        ry = [row for row in y if row.startswith('R_')]
        if len(rx) != len(ry):
            failures.append(['relocation count changed', left, right])
            continue
        for old_relocation, new_relocation in zip(rx, ry):
            pattern = r'(R_[A-Z0-9_]+)\s+(.+?)([+-]0x[0-9a-f]+)?'
            om, nm = re.fullmatch(pattern, old_relocation), re.fullmatch(pattern, new_relocation)
            if not om or not nm or (om[1], om[3]) != (nm[1], nm[3]):
                failures.append(['relocation kind/addend changed', old_relocation, new_relocation])
                continue
            ot, nt = om[2], nm[2]
            ol, nl = ot if ot in a else '.text.' + ot, nt if nt in b else '.text.' + nt
            if ol in a and nl in b:
                queue.append((ol, nl))
            elif ot != nt:
                failures.append(['external relocation changed', ot, nt])
    return dict(roots=NAMES, paired_sections=len(mapping), mapping=mapping,
                reordered=reordered, failures=failures,
                instruction_multisets_and_targets_preserved=not failures)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'target') or out.exists():
        parser.error('Choose a fresh directory under target/')
    out.mkdir(parents=True)
    revision = subprocess.check_output(['git', 'rev-parse', args.baseline + '^{commit}'], cwd=ROOT, text=True).strip()
    objects = {}
    for label in ('previous', 'current'):
        work = out / label
        work.mkdir()
        if label == 'previous':
            data = subprocess.check_output(['git', 'archive', revision, 'src'], cwd=ROOT)
            with tarfile.open(fileobj=io.BytesIO(data)) as archive:
                archive.extractall(work, filter='data')
        else:
            shutil.copytree(ROOT / 'src', work / 'src')
        # Both visibility adapters only invoke the original private planner.
        for file, text in [
            ('bulk.rs', '#[inline] pub(super) fn analysis_plan<K:Ord,V>(a:&BTreeMap<K,V>,b:&BTreeMap<K,V>,n:usize)->Result<(usize,usize),Error>{let c=a.append_plan(b,n)?;Ok((c.leaves,c.branches))}'),
            ('../btree_map.rs', '#[inline] pub fn analysis_plan<K:Ord,V>(a:&BTreeMap<K,V>,b:&BTreeMap<K,V>,n:usize)->Result<(usize,usize),Error>{bulk::analysis_plan(a,b,n)}'),
        ]:
            path = work / 'src/collections/btree_map' / file
            path.write_text(path.read_text() + '\n' + text + '\n')
        (work / 'consumers.rs').write_text(CONSUMERS)
        for profile, extra in [('release', []), ('checked', ['-C', 'overflow-checks=yes'])]:
            flags = ['rustc', '--edition=2024', '-O', '-C', 'panic=abort', '-C', 'codegen-units=1', *extra]
            library, obj = work / ('lib-' + profile + '.rlib'), work / (profile + '.o')
            subprocess.run([*flags, '--crate-name', 'jzap', '--crate-type', 'rlib', '--cfg', 'feature="alloc"',
                            str(work / 'src/lib.rs'), '-o', str(library)], check=True)
            subprocess.run([*flags, '--crate-name', 'node_consumers', '--crate-type', 'lib', '--extern', 'jzap=' + str(library),
                            str(work / 'consumers.rs'), '--emit=obj', '-o', str(obj)], check=True)
            objects[label, profile] = read_object(obj)
            (work / (profile + '.asm')).write_text(objects[label, profile][2])
    reports = {}
    for profile in ('release', 'checked'):
        reports[profile] = compare(objects['previous', profile], objects['current', profile])
        r = reports[profile]
        print(profile, 'paired', r['paired_sections'], 'reordered', len(r['reordered']), 'differences', len(r['failures']))
    (out / 'summary.json').write_text(json.dumps(dict(baseline=revision, profiles=reports), indent=2) + '\n')


if __name__ == '__main__':
    main()
