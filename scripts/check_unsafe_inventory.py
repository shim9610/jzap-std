#!/usr/bin/env python3
"""Maintain the unsafe census and map macro definition gaps to real expansions.

The census is evidence routing, not a certificate of all properties in a block.
Raw generated code, metadata writes and site records remain outside Git.
"""
import hashlib
import json
import os
from pathlib import Path
from scanner_tools import scanner
import re
import subprocess
from collections import Counter
from verify_edit_loops import block

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'target/verification/unsafe-inventory'
EXPECTED={'G01':15,'G02':20,'G03':17,'G04':35,'G05':31,'G06':10,'G07':20,'G08':4,'G09':22,'G10':3,'G11':5,'G12':2,'G13':0,'G14':14,'G15':98,'G16':131,'G17':72,'G18':42,'G19':57,'G20':13}


def group(row):
    f,fn=row['file'],row['function'];text=re.sub(r'\s+','',row['expression'])
    if f=='src/scratch.rs' or f.startswith('src/scratch/') or f=='src/slice.rs' or f.startswith('src/slice/'):return 'G20'
    if f=='src/deque.rs' or f.startswith('src/deque/'):return 'G19'
    if f=='src/collections/binary_heap.rs' or f.startswith('src/collections/binary_heap/'):return 'G18'
    if f=='src/collections/hash_map.rs' or f.startswith('src/collections/hash_map/'):return 'G17'
    if f=='src/collections/btree_map.rs' or f.startswith('src/collections/btree_map/'):return 'G16'
    if f=='src/shared.rs' or f.startswith('src/shared/') or f=='src/budget/owned.rs':return 'G15'
    if f=='src/string.rs' or f.startswith('src/string/'):return 'G14'
    if row['kind']=='unsafe_impl':return 'G13'
    # The owner extent constructs its capacity from the verified growth rule.
    if f in ('src/storage.rs','src/capacity.rs','src/extent.rs'):return 'G01'
    if f=='src/init/collection.rs':return 'G04'
    if f=='src/init.rs' or f.startswith('src/init/'):return 'G02'
    if f=='src/span.rs' or f=='src/compaction.rs':return 'closed-state'
    if f=='src/clone.rs':return 'G02' if fn in ('drop','array_from_fn') else 'G05' if fn=='try_copy_slice_from' else 'G04'
    if f=='src/boxed/buffer.rs':return {'new':'G01','reserve':'G01','finish':'G01' if 'reallocate' in text else 'G05','drop':'G08','push':'G04','append':'G04','collect':'G04','collect_rest':'G04'}[fn]
    if f=='src/boxed/call.rs':return 'G12'
    if f in ('src/boxed/foreign.rs','src/boxed/string.rs','src/boxed/text.rs'):return 'G09'
    if f=='src/boxed/traits.rs':return 'G10'
    if f=='src/boxed/pinned.rs':return 'G11'
    if f=='src/boxed/owned.rs':return 'G03' if fn in ('as_ref','as_mut','leak') else 'G10' if fn=='box_unsize' else 'G05'
    if f=='src/boxed/slice.rs':
        if fn=='drop':return 'G07' if 'span.front()' in text else 'G02'
        if fn in ('next','next_back','into_iter'):return 'G07'
        if fn=='try_init_slice_in':return 'G02'
        if fn in ('try_clone_from_slice_in','try_clone_in','try_copy_from_slice_in'):return 'G04'
        if fn=='try_from_std_vec_in' and 'reallocate' in text:return 'G01'
        return 'G05'
    if f=='src/boxed.rs':
        if fn=='drop':return 'G01' if 'deallocate' in text else 'G08'
        return {'try_new_in':'G02','try_init_in':'G02','try_clone':'G04','deref':'G03','deref_mut':'G03'}[fn]
    if f=='src/vec/convenience.rs':
        if fn in ('try_push_mut','try_insert_mut','spare_capacity_mut','split_at_spare_mut','leak'):return 'G03'
        if fn=='push_within_capacity':return 'G03' if '&mut' in text else 'G04'
        if fn in ('try_extend_from_within','try_extend_copy_from_within','try_resize_owned','try_write_vectored'):return 'G04'
        return 'G05'
    if f=='src/vec/edit.rs':
        if fn in ('try_retain_mut','try_dedup_by'):return 'G06'
        if fn=='drop' and row['context'].startswith('Compact'):return 'G06'
        if fn in ('as_slice','fmt'):return 'G03'
        return 'G07'
    if f=='src/vec/ownership.rs':
        if fn in ('into_raw_parts','from_raw_parts','try_from_std_in','into_std','try_into_boxed_slice','from_boxed_slice'):return 'G05'
        if fn in ('as_slice','as_mut_slice'):return 'G03'
        if fn in ('next','next_back','drop','into_iter'):return 'G07'
        assert fn=='try_clone_in',fn
        return 'G04'
    if f=='src/vec.rs':
        if fn=='drop':return 'G01' if 'deallocate' in text else 'G08'
        if row['kind']=='metadata_init':return 'G01'
        return {'as_slice':'G03','as_mut_slice':'G03','resize_storage':'G01','try_push':'G01','grow_for_push':'G01','try_insert':'G06','pop':'G06','remove':'G06','swap_remove':'G06','truncate':'G08','try_extend_copy':'G04','try_append':'G04','try_split_off':'G04','try_resize_with_init':'G02','try_extend_cloned':'G04','try_clone':'G05','try_clone_in':'G04'}[fn]
    raise AssertionError(('unrouted product site',row))


def main():
    OUT.mkdir(parents=True,exist_ok=True);(OUT/'summary.json').unlink(missing_ok=True)
    files=sorted(p for p in (ROOT/'src').rglob('*.rs') if not p.name.endswith('proofs.rs'))
    inputs=files+[Path(__file__),ROOT/'tests/unsafe_paths.rs',ROOT/'tools/panic-audit/src/bin/unsafe_sites.rs',ROOT/'tools/panic-audit/src/bin/state_sites.rs',ROOT/'scripts/scanner_tools.py']
    fingerprint=lambda:hashlib.sha256(b''.join(str(p.relative_to(ROOT)).encode()+b'\0'+p.read_bytes() for p in inputs)).hexdigest()
    before=fingerprint()
    with (OUT/'parser-build.log').open('w') as log:
        subprocess.run(['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bins','--target-dir','target/panic-inventory'],cwd=ROOT,stdout=log,stderr=log,check=True)
    rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*(str(p.relative_to(ROOT)) for p in files)],cwd=ROOT,text=True))
    for row in rows:
        assert row['kind'] in ('unsafe_block','macro_unsafe_block','unsafe_fn','macro_unsafe_fn','unsafe_impl','metadata_write','metadata_init'),('unsupported unsafe syntax',row)
        row['group']=group(row)
    blocks=Counter(row['group'] for row in rows if row['kind'] in ('unsafe_block','macro_unsafe_block'))
    assert {k:blocks[k] for k in EXPECTED}==EXPECTED and sum(blocks.values())==611,blocks
    assert sum(row['kind']=='unsafe_fn' for row in rows)==119
    assert sum(row['kind']=='macro_unsafe_fn' for row in rows)==2
    assert sum(row['kind']=='unsafe_impl' for row in rows)==42
    (OUT/'sites.json').write_text(json.dumps(rows,indent=2)+'\n')
    command=['cargo','+nightly-2026-10-30','rustc','--locked','--offline','--all-features','--test','unsafe_paths','--','-Zunpretty=expanded']
    with (OUT/'expansion-build.log').open('w') as log:
        result=subprocess.run(command,cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=log)
    assert result.returncode==0
    (OUT/'expanded-paths.rs').write_text(result.stdout)
    expanded=block(result.stdout,'fn macro_expansions_preserve_field_pin_and_call_ownership(')
    tokens=re.sub(r'\s+','',expanded)
    markers=['::jzap::boxed::CallRelease::new','::jzap::boxed::StandardCallOwner::from_raw',
             '::jzap::boxed::Box::from_raw_parts','::jzap::boxed::Pinned::into_inner_unchecked',
             '::jzap::init::InitSlot::from_raw','__parent.mark_initialized()','.disarm_field()']
    for marker in markers:assert marker in tokens,('missing actual macro expansion',marker)
    # These definitions map to a single dedicated positive test executed by both
    # Miri modes; its passing receipt is recorded by the maintained main driver.
    gaps=[row for row in rows if row['kind']=='macro_unsafe_block' and row['function'] in ('box_call_once','box_unsize','pin_box_unsize','try_init_struct')]
    assert len(gaps)==7 # all seven blocks, including the two already mapped before
    from verify_string import bindings
    assert len(bindings())==14
    from shared_bindings import check as shared_bindings
    assert len(shared_bindings())==172
    from btree_bindings import check as btree_bindings
    _,btree_sites=btree_bindings()
    # Canonical borrowed construction adds one unsafe boundary and its two
    # front/back metadata initializers; each routes to the leaf-edge contract.
    assert len(btree_sites)==225
    from verify_binary_heap import bindings as heap_bindings
    heap_phases,heap_sites=heap_bindings()
    assert len(heap_sites)==50
    from verify_deque import bindings as deque_bindings
    _,deque_sites=deque_bindings()
    assert len(deque_sites)==100
    from verify_scratch_slice import bindings as scratch_bindings
    _,scratch_sites=scratch_bindings()
    assert len(scratch_sites)==25
    assert fingerprint()==before
    report=dict(status='pass',source_sha256=before,blocks=dict(blocks),
        unsafe_functions=119,macro_unsafe_functions=2,unsafe_impls=42,
        metadata_writes=sum(r['kind']=='metadata_write' for r in rows),
        metadata_initializers=sum(r['kind']=='metadata_init' for r in rows),
        macro_expansion=dict(command=command,markers=markers,blocks=len(gaps),test='macro_expansions_preserve_field_pin_and_call_ownership'),
        scope='Complete source census and expansion-to-test routing; not a per-site formal or dynamic coverage certificate.')
    (OUT/'summary.json').write_text(json.dumps(report,indent=2)+'\n')
    print('611 blocks, 119 ordinary + 2 macro unsafe signatures and 42 impls routed; String/shared/BTreeMap/HashMap have separate source-connected certificates.')


if __name__=='__main__':main()
