#!/usr/bin/env python3
"""Compose fresh owner, loop and core-precondition models with source mutations.

A model pass is distinct from a complete source-to-final-link certificate. The
receipt records that distinction rather than accepting residual exits by name.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
from scanner_tools import scanner
import re
import shutil
import subprocess
import tempfile
from arithmetic_projection import replace_once
from verus_composition import run as compose
from verify_edit_loops import composition as loops
from owner_state_projection import generate as owners
from closure_projection import generate as closure
from credit_bindings import check as credit_bindings
from callsite_projection import generate as callers
from verify_wrapping_memory import inventory as memory_inventory
from check_unsafe_inventory import group
from verify_string import is_string
from shared_bindings import is_shared,separate
from btree_bindings import is_btree
from bplus_bindings import check as check_bplus, is_bplus
from hash_map_bindings import is_hash_map
from verify_binary_heap import is_binary_heap
from verify_deque import is_deque
from verify_scratch_slice import is_scratch_slice,bindings as scratch_bindings
from caller_bindings import check as bind_callers,inventory as state_inventory,audit_operations,parsed_operations

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'target/verification/owner-protocol'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mutations',action='store_true')
    parser.add_argument('--reuse-generated',action='store_true',help='Development only; passing receipt is explicitly conditional on reused prerequisites')
    args=parser.parse_args()
    OUT.mkdir(parents=True,exist_ok=True)
    lock=(OUT.parent/'.owner-protocol.lock').open('a')
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    OUT.mkdir(parents=True,exist_ok=True);(OUT/'summary.json').unlink(missing_ok=True)
    verus=Path(os.environ.get('JZAP_VERUS','/workspace/.devtools/verus/verus-x86-linux/verus'))
    env=dict(os.environ,PATH=str(verus.parent)+os.pathsep+os.environ['PATH'])
    files=sorted((ROOT/'src').rglob('*.rs'))+sorted((ROOT/'tools/memory-proof').glob('*.verus.rs'))
    files+=sorted((ROOT/'tools/direct-verus').glob('*.rs'))+[ROOT/'scripts/verus_composition.py']
    files+=sorted((ROOT/'tools/panic-audit/src').rglob('*.rs'))
    files+=[ROOT/'scripts/verify_string.py',ROOT/'scripts/shared_bindings.py',ROOT/'scripts/btree_bindings.py',ROOT/'scripts/hash_map_bindings.py',ROOT/'scripts/verify_hash_set.py',ROOT/'scripts/verify_binary_heap.py',ROOT/'scripts/verify_deque.py',ROOT/'scripts/verify_scratch_slice.py']
    files+=[ROOT/'scripts'/name for name in ['init_bindings.py','premise_status.py','verify_owner_protocol.py','owner_state_projection.py','closure_projection.py','credit_bindings.py','callsite_projection.py','caller_bindings.py','bplus_bindings.py','check_unsafe_inventory.py','verify_edit_loops.py','verify_wrapping_memory.py','verify_panic_state.py','verify_state_types.py','verify_pointer_checks.py','verify_source.py','arithmetic_projection.py']]
    fingerprint=lambda:hashlib.sha256(b''.join(str(p.relative_to(ROOT)).encode()+b'\0'+p.read_bytes() for p in files)).hexdigest()
    before=fingerprint()
    check_bplus(ROOT)
    with (OUT/'unsafe-operations-build.log').open('w') as log:
        subprocess.run(['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bin','unsafe_operations','--target-dir','target/panic-inventory'],cwd=ROOT,stdout=log,stderr=log,check=True)
    if not args.reuse_generated:
        for name,command in [('memory',['python3','scripts/verify_wrapping_memory.py']),('pointer',['python3','scripts/verify_pointer_checks.py'])]:
            if args.mutations:command+=['--mutations']
            with (OUT/(name+'-prerequisites.log')).open('w') as log:
                result=subprocess.run(command,cwd=ROOT,env=env,stdout=log,stderr=log)
            assert result.returncode==0,(name,result.returncode)
    stage_output=OUT/'staging-source'
    command=['python3','scripts/verify_source.py','--group','vec-staging','--output',str(stage_output)]
    if args.mutations:command+=['--mutations']
    with (OUT/'staging-source.log').open('w') as log:
        subprocess.run(command,cwd=ROOT,env=env,stdout=log,stderr=log,check=True)
    stage_report=json.loads((stage_output/'summary.json').read_text())
    assert stage_report['status']=='pass' and stage_report['source_unchanged']
    assert len(stage_report['groups'])==1 and stage_report['groups'][0]['group']=='vec-staging'
    stage_harnesses=[name.rsplit('::',1)[-1] for name in stage_report['groups'][0]['kani']['harnesses']]
    assert len(stage_harnesses)==3
    strip=lambda text:re.sub(r'fn main\(\)\s*\{\}','',text)
    base=strip((ROOT/'target/verification/wrapping-memory/memory.generated.rs').read_text())
    pointer=strip((ROOT/'target/verification/pointer-checks/pointer-checks.generated.rs').read_text())
    raw=(ROOT/'tools/memory-proof/storage.verus.rs').read_text()+(ROOT/'tools/memory-proof/raw-owner.verus.rs').read_text()+(ROOT/'tools/direct-verus/array-caller.rs').read_text()
    edit=(ROOT/'src/vec/edit.rs').read_text()

    def program(source_root=ROOT,edit_source=edit):
        owner,expressions=owners(source_root)
        composed,bindings=closure(source_root)
        credit_expressions=credit_bindings(source_root)
        credit='include!('+json.dumps(str(ROOT/'tools/direct-verus/budget-ledger.rs'),ensure_ascii=False)+');\n'
        credit+=(ROOT/'tools/memory-proof/owner-credit.verus.rs').read_text()
        source_files=sorted(str(p.relative_to(source_root)) for p in (source_root/'src').rglob('*.rs') if not p.name.endswith('proofs.rs') and not is_string(str(p.relative_to(source_root))) and not is_shared(str(p.relative_to(source_root))) and not is_btree(str(p.relative_to(source_root))) and not is_bplus(str(p.relative_to(source_root))) and not is_hash_map(str(p.relative_to(source_root))) and not is_binary_heap(str(p.relative_to(source_root))) and not is_deque(str(p.relative_to(source_root))) and not is_scratch_slice(str(p.relative_to(source_root))))
        unsafe_rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*source_files],cwd=source_root,text=True))
        for site in unsafe_rows:site['group']=group(site)
        operation_audit=audit_operations(unsafe_rows,OUT/'operations')
        calls,caller_bindings=callers(unsafe_rows,parsed_operations(unsafe_rows,OUT/'operations'),memory_inventory(source_root))
        expressions=dict(expressions,operation_bindings=bindings,credit_expressions=credit_expressions)
        expressions['caller_bindings']=caller_bindings
        expressions['unsafe_operation_audit']=operation_audit
        return base+pointer+loops()+owner+raw+composed+credit+calls+'\nfn main(){}\n',expressions

    results=[]
    def verify(name,text):
        path=OUT/(name+'.generated.rs');path.write_text(text)
        command,r=compose(verus,ROOT,path,'jzap_owner_protocol',env)
        log=r.stdout+r.stderr;(OUT/(name+'.log')).write_text(log)
        row=dict(name=name,command=command,composition_input=str(path),exit_code=r.returncode,
                 generated_sha256=hashlib.sha256(text.encode()).hexdigest(),
                 verification=re.findall(r'verification results:: (\d+) verified, (\d+) errors',log))
        results.append(row);return row,log

    text,expressions=program()
    row,log=verify('owner-protocol',text);assert row['exit_code']==0,log[-5000:]
    # The whole inventory: the binding check filters each area itself, after it
    # has refused an owner-word borrow in any of them.
    state_rows=state_inventory(ROOT,OUT)
    source_files=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs') if not p.name.endswith('proofs.rs') and not is_string(str(p.relative_to(ROOT))))
    unsafe_rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*source_files],cwd=ROOT,text=True))
    for site in unsafe_rows:site['group']=group(site)
    memory_report=json.loads((ROOT/'target/verification/wrapping-memory/summary.json').read_text())
    checked_harnesses=memory_report['kani_harnesses']+memory_report['direct_cursor_harnesses']+stage_harnesses
    source_bindings=bind_callers(ROOT,text,state_rows,unsafe_rows,checked_harnesses)
    if args.mutations:
        for name,file,old,new in [
            # A representation-only site that gains an element offset leaves the
            # maintained representation contract and must be proved as an offset.
            ('representation_gains_offset','src/boxed.rs','unsafe { self.ptr.as_ref() }','unsafe { &*self.ptr.as_ptr().add(1) }'),
        ]:
            with tempfile.TemporaryDirectory(prefix='jzap-owner-model-') as tmp:
                source=Path(tmp);shutil.copytree(ROOT/'src',source/'src')
                shutil.copytree(ROOT/'tools/memory-proof',source/'tools/memory-proof')
                path=source/file;path.write_text(replace_once(path.read_text(),old,new))
                mutated,_=program(source)
            row,log=verify('mutation-'+name,mutated)
            assert row['exit_code'] and row['verification'] and any(int(n) for _,n in row['verification']) and 'error[E' not in log,log[-5000:]
            row['detected']=True
        # Owner-invariant callers are bound by whole-block text. A changed
        # block is refused before any proof text is composed.
        for name,file,old,new in [
            ('expose_spare_as_initialized','src/vec.rs','unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len.get()) }','unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.cap.get()) }'),
            ('scalar_gains_offset','src/boxed/owned.rs','unsafe { ptr.as_ptr().read() }','unsafe { ptr.as_ptr().add(1).read() }'),
            ('into_iter_stale_index','src/vec/ownership.rs','let index = self.span.next()?;','let index = self.span.next()?; let index = self.span.len() + index;'),
            ('drain_clear_after_count','src/vec/edit.rs','let start = self.span.front();\n        let count = self.span.len();\n        self.span.clear();','self.span.clear();\n        let start = self.span.front();\n        let count = self.owner.cap.get();'),
            ('array_clone_shifted_index','src/clone.rs','unsafe { self.get_unchecked(i) }.try_clone()','unsafe { self.get_unchecked(i + 1) }.try_clone()'),
            ('prefix_driver_longer_count','src/init/prefix.rs','super::driver::run(&mut prefix.state, length, |index| unsafe {','super::driver::run(&mut prefix.state, length + 1, |index| unsafe {'),
            ('pop_one_past','src/vec.rs','Some(unsafe { self.ptr.as_ptr().add(next.get()).read() })','Some(unsafe { self.ptr.as_ptr().add(self.cap.get()).read() })'),
            ('truncate_from_capacity','src/vec.rs','self.ptr.as_ptr().add(len),\n                removed.get(),','self.ptr.as_ptr().add(removed.get()),\n                len,'),
            ('collection_write_shifted','src/init/collection.rs','unsafe { ptr.as_ptr().add(index).write(value) };','unsafe { ptr.as_ptr().add(index + 1).write(value) };'),
            ('collection_publication_omitted','src/init/collection.rs','*self.output = *self.current;','*self.output = *self.output;'),
            ('append_from_capacity','src/vec.rs','other.ptr.as_ptr(),\n                self.ptr.as_ptr().add(self.len.get()),\n                other.len.get(),','other.ptr.as_ptr(),\n                self.ptr.as_ptr().add(self.cap.get()),\n                other.len.get(),'),
            ('reserve_without_growth','src/vec.rs','if needed > self.cap.get() {\n            self.grow_to(needed, false)?;\n        }\n        Ok(needed)','Ok(needed)'),
            ('compact_drop_whole','src/vec/edit.rs','let remaining = self.state.remaining();','let remaining = self.owner.cap.get();'),
            ('empty_slice_length','src/boxed/slice.rs','NonNull::slice_from_raw_parts(NonNull::dangling(), 0)','NonNull::slice_from_raw_parts(NonNull::dangling(), 1)'),
            ('drop_whole_capacity','src/vec.rs','core::ptr::slice_from_raw_parts_mut(\n                self.ptr.as_ptr(),\n                self.len.get(),\n            ))','core::ptr::slice_from_raw_parts_mut(\n                self.ptr.as_ptr(),\n                self.cap.get(),\n            ))'),
        ]:
            with tempfile.TemporaryDirectory(prefix='jzap-owner-model-') as tmp:
                source=Path(tmp);shutil.copytree(ROOT/'src',source/'src')
                shutil.copytree(ROOT/'tools/memory-proof',source/'tools/memory-proof')
                path=source/file;path.write_text(replace_once(path.read_text(),old,new))
                try:program(source)
                except AssertionError as error:
                    diagnostic=str(error)
                    # Either the exact-text routing or an earlier binding audit of the
                    # same source refuses the mutant; any other failure is not accepted.
                    assert any(reason in diagnostic for reason in (
                        'owner-invariant caller text changed or missing','unreviewed offset-bearing block in an owner-invariant function',
                        'unreviewed scalar value access','state caller body changed',
                        'initialization caller binding requires review','unbound removed-suffix caller')),(name,diagnostic)
                else:raise AssertionError(('source mutant accepted',name))
            results.append(dict(name='mutation-'+name,detected=True,mechanism='changed caller body refused by exact-text routing',diagnostic=diagnostic[:400]))
        # The maintained owner-invariant functions themselves are solver-checked.
        for name,old,new in [
            ('owner_prefix_without_invariant','fn owner_prefix_caller(a:RawStorage,len:usize)\n    requires raw_valid(a),len<=a.cap,','fn owner_prefix_caller(a:RawStorage,len:usize)\n    requires raw_valid(a),'),
            ('owner_scalar_without_element','fn owner_scalar_caller(a:RawStorage)\n    requires raw_valid(a),1<=a.cap,','fn owner_scalar_caller(a:RawStorage)\n    requires raw_valid(a),'),
            ('span_without_storage_bound','fn span_next_caller(a:RawStorage,span:crate::span::Span)\n    requires raw_valid(a),span.wf(),span@.back<=a.cap,','fn span_next_caller(a:RawStorage,span:crate::span::Span)\n    requires raw_valid(a),span.wf(),'),
            ('span_remaining_count','    let _=slice_caller(a,front,count);\n}','    let _=slice_caller(a,front,count+1);\n}'),
            ('prefix_driver_one_past','        let accepted=unchecked_index_precondition(index,n);','        let accepted=unchecked_index_precondition(index+1,n);'),
            ('pop_without_subtraction','    let Ok(next)=crate::Checked::new(len)-crate::Checked::new(1usize) else {return;};\n    let next=next.get();','    let next=len;'),
            ('truncate_whole_length','    let _=slice_caller(a,requested,removed);','    let _=slice_caller(a,requested,len);'),
            ('collection_driver_past_end','        requires raw_valid(a),end<=a.cap,index<end,\n    {\n        let _=offset_storage(a,index);\n        let _=slice_caller(a,index,1);','        requires raw_valid(a),end<=a.cap,index<end,\n    {\n        let _=offset_storage(a,index);\n        let _=slice_caller(a,index,2);'),
            ('reserved_slot_without_capacity','fn reserved_slot_caller(a:RawStorage,len:usize)\n    requires raw_valid(a),len<a.cap,','fn reserved_slot_caller(a:RawStorage,len:usize)\n    requires raw_valid(a),len<=a.cap,'),
            ('reserve_length_keeps_capacity','        let Ok(grown)=crate::vec::geometry::growth_for_layout(cap,needed,limit,size,false) else {return None;};\n        Some((needed,grown))','        Some((needed,cap))'),
            ('extend_copy_without_published_capacity','    let Some((_needed,after))=vec_reserve_length(len,before,limit,a.size,additional) else {return;};\n    if a.cap!=after {return;}\n    let _=offset_storage(a,len);\n    distinct_nonoverlap_caller(b,a,0,len,additional);','    let Some((_needed,after))=vec_reserve_length(len,before,limit,a.size,additional) else {return;};\n    let _=offset_storage(a,len);\n    distinct_nonoverlap_caller(b,a,0,len,additional);'),
            ('buffer_append_old_capacity','    if a.cap!=proposed {return;}\n    let _=offset_storage(a,len);\n    distinct_nonoverlap_caller(b,a,0,len,additional);','    if a.cap!=before {return;}\n    let _=offset_storage(a,len);\n    distinct_nonoverlap_caller(b,a,0,len,additional);'),
            ('extract_drop_to_end_only','        let count=state.unvisited_len();','        let count=state.unvisited_len()+1;'),
            ('move_tail_without_gap_fill','    requires raw_valid(a),len==tail_start,tail_start as int+tail_len as int<=before as int,','    requires raw_valid(a),tail_start as int+tail_len as int<=before as int,'),
            ('owner_spare_count','    let _=slice_caller(a,len,spare);','    let _=slice_caller(a,len,a.cap);'),
            # The restated Drain words: a length above tail_start is admitted
            # only with an empty tail and inside the storage; the fill takes the
            # product's rejection branch; an empty-tail push leaves tail_start.
            ('drain_restore_without_length_bound','        len<=tail_start || (tail_len==0 && len<=a.cap),\n{\n    let _=offset_storage(a,tail_start);','        len<=tail_start || tail_len==0,\n{\n    let _=offset_storage(a,tail_start);'),
            ('hole_without_length_bound','        && (h.len<=h.tail || (h.tail_len==0 && h.len<=h.cap))','        && (h.len<=h.tail || h.tail_len==0)'),
            ('hole_fill_without_admission','    if h.tail.checked_sub(h.len).is_none() {return 2;}\n',''),
            ('hole_push_moves_tail','    h.len=next;\n    true','    h.len=next;h.tail=next;\n    true'),
            # Exchanged storage words keep each owner's own limit.
            ('exchange_without_equal_limits','    if a.len>=b.len || a.limit!=b.limit || declined {return None;}','    if a.len>=b.len || declined {return None;}'),
        ]:
            assert text.count(old)==1,name
            row,log=verify('mutation-'+name,text.replace(old,new))
            assert row['exit_code'] and row['verification'] and any(int(n) for _,n in row['verification']) and 'error[E' not in log,log[-5000:]
            row['detected']=True
        # Structural controls exercise the source-to-model boundary itself.
        # These mutants must compile as real Rust, then fail a specific binding
        # check; a compiler or solver failure is not accepted as that detection.
        for name,file,old,new,expected in [
            ('staging_size','src/vec/edit.rs',
             '            mem::size_of::<T>(),\n            extra.capacity(),',
             '            0,\n            extra.capacity(),',
             'staging controller arguments and error routing'),
            ('splice_fill_end','src/vec/edit.rs','                self.drain.tail_start,\n                &mut self.replacement,','                self.drain.owner.cap.get(),\n                &mut self.replacement,','initialization caller binding requires review'),
            ('staging_fill_end','src/vec/edit.rs','    let end = extra.capacity();','    let end = extra.len.get();','initialization caller binding requires review'),
            ('extraction_error_routing','src/vec/edit.rs','Err(error) => self.fail(error),','Err(error) => Some(Err(error)),','extraction adapter statements'),
            ('extraction_source_routing','src/vec/edit.rs','filter(unsafe { &mut *pointer.as_ptr().add(read) })','filter(unsafe { &mut *pointer.as_ptr() })','extraction physical leaf routing'),
            ('shadowed_compaction_index','src/vec/edit.rs','keep(unsafe { &mut *pointer.as_ptr().add(read) })','let read = original; keep(unsafe { &mut *pointer.as_ptr().add(read) })','compaction adapter statements'),
            ('unbound_compaction_call', 'src/vec.rs', 'self.truncate(0);', 'let mut cursor=crate::compaction::Compaction::full(0); let _:Result<(),()>=crate::vec::compaction_driver::run::<false,(),_,_>(&mut cursor,1,|_,_|Ok(true),|_,_,_|{}); self.truncate(0);', 'unbound compaction driver entry'),
            ('escaped_compaction_entry', 'src/vec.rs', 'self.truncate(0);', 'let _=crate::vec::compaction_driver::run::<false,(),fn(usize,usize)->Result<bool,()>,fn(usize,usize,bool)>; self.truncate(0);', 'unbound compaction driver entry'),
            ('aliased_compaction_entry', 'src/vec.rs', 'self.truncate(0);', 'use crate::vec::compaction_driver::run as escaped; let _=escaped::<false,(),fn(usize,usize)->Result<bool,()>,fn(usize,usize,bool)>; self.truncate(0);', 'unbound compaction driver import'),
            ('retain_destination','src/vec/edit.rs','pointer.as_ptr().add(write),','pointer.as_ptr().add(read),','compaction physical leaf routing'),
            ('retain_input_route','src/vec/edit.rs','keep(unsafe { &mut *pointer.as_ptr().add(read) })','keep(unsafe { &mut *pointer.as_ptr().add(original) })','compaction predicate routing'),
            ('dedup_initial_state','src/vec/edit.rs','state: Compaction::first_kept(original)','state: Compaction::full(original)','compaction adapter statements'),
            ('dedup_driver_route','src/vec/edit.rs','compaction_driver::run::<false, E, _, _>(\n            &mut g.state,\n            original,\n            |read, write| {','compaction_driver::run::<true, E, _, _>(\n            &mut g.state,\n            original,\n            |read, write| {','compaction driver routing'),
            ('duplicate_release','src/storage.rs','budget.release(layout.size());','budget.release(layout.size()); budget.release(layout.size());','accounting argument/order mismatch'),
            ('unbound_capacity_caller','src/vec.rs','self.truncate(0);','let _ = self.resize_storage(0); self.truncate(0);','unbound private-capacity caller'),
            ('wrong_array_reservation','src/vec/convenience.rs','result.try_reserve_exact(N)','result.try_reserve_exact(0)','unbound array reservation'),
            ('unknown_unsafe_intrinsic','src/boxed.rs','ptr.as_ptr().write(value);','core::arch::asm!(""); ptr.as_ptr().write(value);','unbound unsafe operation'),
            ('wrong_release_layout','src/boxed/owned.rs','AllocationGuard(ptr.cast(), Layout::new::<T>(), policy.budget())','AllocationGuard(ptr.cast(), Layout::new::<u8>(), policy.budget())','allocation guard identity/layout mismatch'),
            ('buffer_initial_end','src/boxed/buffer.rs','fill(self.ptr, &mut self.len, reserved_end, &mut iter)','fill(self.ptr, &mut self.len, self.cap.get(), &mut iter)','unbound collection extent'),
            ('buffer_growth_end','src/boxed/buffer.rs','fill(self.ptr, &mut self.len, self.cap.get(), &mut iter)','fill(self.ptr, &mut self.len, 0, &mut iter)','unbound collection extent'),
            ('bulk_clone_without_append','src/clone.rs','None\n    }\n    /// Clone into existing storage','Some(Ok(()))\n    }\n    /// Clone into existing storage','bulk clone hook changed'),
            ('unbound_live_transition','src/vec.rs','self.truncate(0);','crate::compaction::Compaction::full(0).keep_live(); self.truncate(0);','unbound admitted cursor step'),
            # Every mutation channel of an owner word is pinned: an assignment,
            # a mutable borrow (also as an argument of mem::swap or mem::replace)
            # and a pattern binding, with or without `ref mut`. A borrow in a
            # file of another area is refused by the same check; test_verify.py
            # exercises that on rows, because VecDeque pins its files whole and
            # refuses a source mutant earlier.
            ('unbound_owner_write','src/vec/edit.rs','        self.span.clear();\n        if !mem::needs_drop::<T>() {','        self.span.clear();\n        self.owner.len = Checked::new(0);\n        if !mem::needs_drop::<T>() {','unbound state write'),
            ('unbound_owner_lend','src/vec.rs','self.truncate(0);','core::mem::swap(&mut self.len, &mut Checked::new(0)); self.truncate(0);','unbound owner-word borrow'),
            ('unbound_owner_take','src/vec.rs','self.truncate(0);','let _ = core::mem::replace(&mut self.cap, crate::capacity::Capacity::empty(0)); self.truncate(0);','unbound owner-word borrow'),
            ('owner_ref_mut_pattern','src/vec.rs','self.truncate(0);','{ let Self { len: ref mut hidden, .. } = *self; core::mem::swap(hidden, &mut Checked::new(0)); } self.truncate(0);','unbound owner-word pattern binding'),
            ('owner_default_binding_pattern','src/vec.rs','self.truncate(0);','{ let Self { len, .. } = &mut *self; core::mem::swap(len, &mut Checked::new(0)); } self.truncate(0);','unbound owner-word pattern binding'),
            ('exchange_without_limit_check','src/vec.rs','            || self.max_len() != other.max_len()\n','','unbound owner exchange'),
        ]:
            with tempfile.TemporaryDirectory(prefix='jzap-source-binding-') as tmp:
                source=Path(tmp);shutil.copytree(ROOT/'src',source/'src')
                path=source/file;original_source=path.read_text()
                if name=='retain_destination':
                    boundary=original_source.index('pub fn try_dedup_by<')
                    changed=replace_once(original_source[:boundary],old,new)+original_source[boundary:]
                else:changed=replace_once(original_source,old,new)
                path.write_text(changed)
                build=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(source/'src/lib.rs'),'-o',str(source/'libjzap.rlib')],cwd=ROOT,capture_output=True,text=True)
                assert build.returncode==0,build.stderr
                mutated_states=state_inventory(source,OUT/('mutation-'+name))
                mutated_unsafe=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*source_files],cwd=source,text=True))
                for site in mutated_unsafe:site['group']=group(site)
                try:
                    audit_operations([r for r in mutated_unsafe if not separate(r) and not is_btree(r['file']) and not is_bplus(r['file']) and not is_hash_map(r['file']) and not is_binary_heap(r['file']) and not is_deque(r['file']) and not is_scratch_slice(r['file'])],OUT/('mutation-'+name))
                    if name.startswith('buffer_'):closure(source)
                    bind_callers(source,text,mutated_states,mutated_unsafe,memory_report['kani_harnesses']+memory_report['direct_cursor_harnesses']+stage_harnesses)
                except AssertionError as error:
                    diagnostic=str(error);assert expected in diagnostic,(name,diagnostic)
                else:raise AssertionError(('source mutant accepted',name))
                results.append(dict(name='mutation-'+name,detected=True,mechanism='compiled Rust rejected by source binding',diagnostic=diagnostic))
    assert fingerprint()==before,'verification inputs changed during run'
    report=dict(staging_source=str(stage_output/'summary.json'),staging_harnesses=stage_harnesses,status='pass',source_sha256=before,source_unchanged=True,reused_prerequisites=args.reuse_generated,
                results=results,owner_expressions=expressions,
                source_bindings=source_bindings,
                # The actual-Rust harnesses each `kani:` binding was checked against.
                kani_harnesses=checked_harnesses,
                state_adapter_sha256=hashlib.sha256((ROOT/'tools/direct-verus/state-adapters.rs').read_bytes()).hexdigest(),
                word_domain='Verus Either32Or64; no usize-to-u32 body rewriting',
                scope='Original Compaction/Extraction methods composed with pending projected owner/loop/credit/core caller contracts in the same product crate. Numeric word domain is Either32Or64; final link and executed memory targets are separate. Heap/alias behavior is covered separately by actual-source checks.',
                source_closure_complete=source_bindings['unsafe_callers']==181 and len(expressions['caller_bindings'])==181,
                boundary='The final link certificate is produced separately by verify_panic_paths.py; these source-model results do not certify arbitrary user callbacks or Rust provenance.')
    (OUT/'summary.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Owner/loop/credit models, actual core-call queries and exhaustive source publications pass; final-link attachment is checked by verify_panic_paths.py.')


if __name__=='__main__':main()
