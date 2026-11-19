#!/usr/bin/env python3
"""Reproduce HashMap direct-source offsets, phase models, actual Rust kernels.

No product body is extracted or rewritten for a positive proof. Mathematical
phase models remain distinct from direct Verus and actual Rust Kani evidence.
"""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
from hash_map_bindings import FILES,check,correspondence,ROOT
from verus_composition import run as compose
import storage_kernel
from verify_direct_verus import verify,executable
HARNESSES=['group_masks_all_bytes','mirror_and_probe_all_machine_word_shapes','checked_layout_representative_sizes_all_capacities','actual_table_slot_publication_and_retirement','actual_insert_probe_keeps_tombstones_until_later_matches','hash_set_replace_preserves_slot_class_length_and_credit']
HARNESSES=['collections::hash_map::proofs::'+h for h in HARNESSES]+['collections::hash_map::iter::scan_proofs::every_control_frontier_preserves_bounds_and_retirement']
SOLVER_CONTROLS=[
 # Table primitives over the storage model: each bound is needed for the
 # documented condition on every table of the mask.
 ('slot_index_past_buckets','fn hash_slot_index(mask: usize, index: usize)\n    requires table_shape(mask), index <= mask,','fn hash_slot_index(mask: usize, index: usize)\n    requires table_shape(mask), index <= mask + 1,'),
 ('control_index_past_mirror','    requires table_shape(mask), index < mask + 17,','    requires table_shape(mask), index <= mask + 17,'),
 ('group_load_past_control_span','fn hash_group_index(mask: usize, pos: usize)\n    requires table_shape(mask), pos <= mask,','fn hash_group_index(mask: usize, pos: usize)\n    requires table_shape(mask), pos <= mask + 2,'),
 ('table_without_mirror_group','    && raw_valid(t.controls) && t.controls.cap == mask + 17 && t.controls.size == 1','    && raw_valid(t.controls) && t.controls.cap == mask + 1 && t.controls.size == 1'),
 ('slot_sub_from_wrong_end','            storage_sub_condition(t.slots, (mask + 1) as usize, (index + 1) as usize);','            storage_sub_condition(t.slots, mask, (index + 1) as usize);'),
 ('retire_mirror_as_slot','    hash_slot_index(mask, index);\n    hash_control_index(mask, index);\n    let mirror = geometry::mirror(index, add(mask, 1));\n    hash_control_index(mask, mirror);','    hash_control_index(mask, index);\n    let mirror = geometry::mirror(index, add(mask, 1));\n    hash_slot_index(mask, mirror);\n    hash_control_index(mask, mirror);'),
 ('resize_target_in_source_table','    hash_slot_index(destination, target);','    hash_slot_index(source, target);'),
 ('groups_one_more','{ 1 } else { add(mask / group::WIDTH, 1) }','{ 1 } else { add(mask / group::WIDTH, 2) }'),
 ('find_insert_without_stride_check','        if stride > mask {\n            return;','        if stride > add(mask, 16) {\n            return;'),
 ('set_control_wrong_buckets','let mirror = geometry::mirror(index, add(mask, 1));\n    assert(index < mask + 1 + 16 && mirror < mask + 1 + 16);\n    assert(mirror >= 16);','let mirror = geometry::mirror(index, mask);\n    assert(index < mask + 1 + 16 && mirror < mask + 1 + 16);\n    assert(mirror >= 16);'),
 ('allocation_wrong_rounding','add(mul(buckets, size), round) & !round;','add(mul(buckets, size), round) & !align;'),
 ('slot_one_past','let back = add(index, 1);','let back = add(index, 2);'),
 ('erase_before_group','add(mask, 1)), group::WIDTH);','add(mask, 1)), 17);'),
 ('insert_credit_swapped','if empty { sub(growth, 1) } else { growth }','if empty { growth } else { sub(growth, 1) }'),
 ('clear_half_group','index = add(base, group::WIDTH);','index = add(base, 8);'),
 ('scan_half_group','            bits = full;\n            group = add(group, group::WIDTH);','            bits = full;\n            group = add(group, 8);'),
 ('scan_past_last_group','            if group > mask {\n                return;\n            }\n            assert(group + 16','            if group > add(mask, 16) {\n                return;\n            }\n            assert(group + 16'),
 ('scan_wrong_base','let index = add(sub(group, group::WIDTH), group::first(bits));','let index = add(group, group::first(bits));'),
 ('vacant_fallback_unchecked','    if fallback < hash_buckets(mask) {\n        assert(fallback <= mask);\n    }','    assert(fallback <= mask);'),
 ('retire_two','    assert(index < mask + 1 + 16 && mirror < mask + 1 + 16);\n    sub(len, 1)\n}','    assert(index < mask + 1 + 16 && mirror < mask + 1 + 16);\n    sub(len, 2)\n}'),
 ('resize_source_mask','let mirror = geometry::mirror(target, add(destination, 1));','let mirror = geometry::mirror(target, add(source, 1));'),
 ('resize_unchecked_credit','    raw::capacity_for(destination).checked_sub(len)\n','    Some(raw::capacity_for(destination).wrapping_sub(len))\n'),
 ('clone_double_count','            len = add(len, 1);\n        }\n        index = add(index, 1);','            len = add(len, 2);\n        }\n        index = add(index, 1);'),
 ('clone_without_source_count','    if len > capacity {\n        return;\n    }\n','    if len > mask {\n        return;\n    }\n'),
 ('growth_reversed','    sub(raw::capacity_for(mask), len)\n}','    sub(len, raw::capacity_for(mask))\n}'),
 ('reset_short_span','let controls = add(buckets, group::WIDTH);\n    assert(offset + controls == bytes);\n    let _filled','let controls = add(buckets, 8);\n    assert(offset + controls == bytes);\n    let _filled'),
]
SOURCE_CONTROLS=[
 ('tag-shift','src/collections/hash_map/raw.rs','    (hash >> 57) as u8','    (hash >> 56) as u8'),
 ('capacity-factor','src/collections/hash_map/raw.rs','mul(add(mask, 1) / 8, 7)','mul(add(mask, 1) / 8, 9)'),
 ('clear-first-or','src/collections/hash_map/group.rs','    bits & bits.saturating_sub(1)','    bits | bits.saturating_sub(1)'),
 ('sizes-offset-rounding','src/collections/hash_map/geometry.rs','let offset = padded & !mask;','let offset = padded & !15usize;'),
]
TEXT_CONTROLS=[
 ('groups-body','src/collections/hash_map/raw.rs','add(self.mask / WIDTH, 1)','add(self.mask / WIDTH, 2)'),
 ('find-probe-arguments','src/collections/hash_map/raw.rs','            groups = sub(groups, 1);\n            if groups != 0 {\n                (pos, stride) = probe_next(pos, stride, self.mask);\n            }\n        }\n        None\n    }\n    /// One bounded','            groups = sub(groups, 1);\n            if groups != 0 {\n                (pos, stride) = probe_next(pos, stride, self.len.get());\n            }\n        }\n        None\n    }\n    /// One bounded'),
 ('set-control-mirror','src/collections/hash_map/raw.rs','let mirror = mirror(index, add(self.mask, 1));','let mirror = mirror(index, add(self.mask, 2));'),
 ('slot-offset','src/collections/hash_map/raw.rs','.sub(add(index, 1)) }','.sub(add(index, 2)) }'),
 ('scan-group-bound','src/collections/hash_map/iter.rs','if self.group > self.mask {','if self.group > self.remaining.get() {'),
 ('scan-base','src/collections/hash_map/iter.rs','let index = add(sub(self.group, group::WIDTH), group::first(self.bits));','let index = add(self.group, group::first(self.bits));'),
 ('get-mut-slot','src/collections/hash_map.rs','        // SAFETY: exact initialized field under exclusive borrow.\n        Some(unsafe { &mut *ptr::addr_of_mut!((*self.table.slot(index)).1) })','        Some(unsafe { &mut *ptr::addr_of_mut!((*self.table.slot(index ^ 1)).1) })'),
 ('extract-slot','src/collections/hash_map/iter.rs','                let p = self.map.table.slot(index);\n                if (self.predicate)','                let p = self.map.table.slot(self.scan.mask);\n                if (self.predicate)'),
 ('entry-index-source','src/collections/hash_map.rs','Entry::Occupied(OccupiedEntry { map: self, index })','Entry::Occupied(OccupiedEntry { map: self, index: hash as usize })'),
 ('resize-control-index','src/collections/hash_map.rs','destination.table.set_control(target, raw::tag(hash));','destination.table.set_control(index, raw::tag(hash));'),
 ('clone-published-tag','src/collections/hash_map.rs','result.table.set_control(index, self.table.control(index));','result.table.set_control(index, group::EMPTY);'),
 ('disjoint-check','src/collections/hash_map.rs','if index.is_some() && tail.contains(index) {','if index.is_none() && tail.contains(index) {'),
 ('rehash-final-credit','src/collections/hash_map/raw.rs','            Checked::new(sub(capacity_for(guard.table.mask), guard.table.len.get()));\n        guard.active = false;','            Checked::new(capacity_for(guard.table.mask));\n        guard.active = false;'),
 ('vacant-entry-index','src/collections/hash_map/entry.rs','                .insert_at(index, self.hash, (self.key, value));\n            Ok(&mut *ptr::addr_of_mut!((*self.map.table.slot(index)).1))','                .insert_at(index, self.hash, (self.key, value));\n            Ok(&mut *ptr::addr_of_mut!((*self.map.table.slot(0)).1))'),
 ('allocation-rounding','src/collections/hash_map/raw.rs','round) & !round;','round) & !align;'),
]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--kani',action='store_true');p.add_argument('--mutations',action='store_true');a=p.parse_args()
 out=ROOT/'target/verification/hash-map/proofs';out.mkdir(parents=True,exist_ok=True);(out/'summary.json').unlink(missing_ok=True)
 paths=[*(ROOT/f for f in FILES),ROOT/'src/clone.rs',ROOT/'scripts/check_btree_bounds.py',ROOT/'src/collections/hash_map/proofs.rs',ROOT/'src/collections/hash_map/scan_proofs.rs',Path(__file__),*storage_kernel.INPUTS,ROOT/'scripts/hash_map_bindings.py',ROOT/'scripts/verus_composition.py',ROOT/'src/lib.rs',ROOT/'src/arithmetic/bounded.rs',ROOT/'tools/memory-proof/hash-map.verus.rs',ROOT/'tools/direct-verus/hash-map.rs']
 fingerprint=lambda:hashlib.sha256(b''.join(str(f.relative_to(ROOT)).encode()+b'\0'+f.read_bytes() for f in paths)).hexdigest()
 before=fingerprint()
 def run(name,cmd,cwd=ROOT,env=None):
  with (out/(name+'.log')).open('w') as log:r=subprocess.run(cmd,cwd=cwd,stdout=log,stderr=log,env=env)
  return r,(out/(name+'.log')).read_text()
 r,log=run('ast-build',['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bin','unsafe_sites','--target-dir','target/panic-inventory']);assert r.returncode==0,log[-2000:]
 phases,bindings=check()
 # The raw-copy route requires a type-enforced, unforgeable Copy capability.
 from check_btree_bounds import CASES
 capability=[c for c in CASES if c[0].startswith(('hash-copy-witness-','array-copy-witness-'))]
 assert len(capability)==6 and sum(c[1] for c in capability)==2,'Copy capability cases changed'
 def capability_checks(root,negative_mutant=False):
  lib=out/('libcapability-mutant.rlib' if negative_mutant else 'libcapability.rlib')
  r,log=run('capability-build'+('-mutant' if negative_mutant else ''),['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(lib)])
  assert r.returncode==0,log[-4000:]
  outcomes=[]
  for name,positive,source in capability:
   probe=out/(name+'.rs');probe.write_text(source)
   r,log=run(name+('-mutant' if negative_mutant else ''),['rustc','--edition=2024','--crate-type=lib','--crate-name=copy_capability','--extern','jzap='+str(lib),str(probe),'-o',str(out/'probe.rlib')])
   if negative_mutant and name=='hash-copy-witness-noncopy':assert r.returncode==0,log[-4000:]
   elif not negative_mutant:assert (r.returncode==0)==positive,(name,log[-4000:])
   outcomes.append(dict(name=name,compiled=r.returncode==0))
  return outcomes
 capability_results=capability_checks(ROOT)
 # Bind core operations to their pinned source protocol, not just symbol names.
 from verify_edit_loops import block
 core=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/core/src'
 compact=lambda s:re.sub(r'\s+','',re.sub(r'//[^\n]*','',s))
 uints=(core/'num/uint_macros.rs').read_text();slices=(core/'slice/mod.rs').read_text()
 assert compact(block(uints,'pub const fn wrapping_shl('))=='unsafe{self.unchecked_shl(rhs&(Self::BITS-1))}'
 chunk=compact(block(slices,'pub const fn split_first_chunk<'))
 assert 'self.split_at_checked(N)' in chunk and 'returnNone' in chunk
 checked=compact(block(slices,'pub const fn split_at_checked('))
 assert 'ifmid<=self.len()' in checked and 'Some(unsafe{self.split_at_unchecked(mid)})' in checked
 unchecked=compact(block(slices,'pub const unsafe fn split_at_unchecked('))
 assert '(mid:usize=mid,len:usize=len)=>mid<=len,' in unchecked
 direct=verify(out=out/'direct',targets=['hash-map'],mutations=a.mutations)
 binary=executable();model=ROOT/'tools/memory-proof/hash-map.verus.rs'
 callers=correspondence();results=[]
 def model_check(path,name,negative=False,root=ROOT):
  # The maintained callers and lemmas are verified together with the product crate.
  env=dict(os.environ,PATH=str(Path(binary).parent)+os.pathsep+os.environ['PATH'])
  command,r=compose(binary,root,path,'jzap_hash_map_proof',env)
  log=r.stdout+r.stderr;(out/(name+'.log')).write_text(log)
  count=re.findall(r'verification results:: (\d+) verified, (\d+) errors',log)
  if negative:assert count and r.returncode!=0 and any(int(e) for _,e in count) and 'error[E' not in log,log[-4000:]
  else:assert count and r.returncode==0 and all(int(e)==0 for _,e in count),log[-4000:]
  results.append(dict(name=name,command=command,exit_code=r.returncode,verification=count,negative_control=negative))
 # The storage kernel is placed in front of the model in every composition.
 kernel=storage_kernel.kernel();composed=out/'model.composed.rs';composed.write_text(kernel+model.read_text())
 model_check(composed,'phase-model')
 if a.kani:
  r,log=run('kani',['cargo','kani','--lib','--harness','collections::hash_map::proofs::','--harness','collections::hash_map::iter::scan_proofs::','-Z','unstable-options','--cbmc-args','--object-bits','12'])
  assert r.returncode==0 and 'Complete - 7 successfully verified harnesses, 0 failures, 7 total.' in log,log[-5000:]
 controls=[]
 if a.mutations:
  with tempfile.TemporaryDirectory(prefix='jzap-copy-bound-') as tmp:
   work=Path(tmp);shutil.copytree(ROOT/'src',work/'src');path=work/'src/clone.rs';text=path.read_text();old='impl<T: Copy> CopyWitness<T> {';assert text.count(old)==1;path.write_text(text.replace(old,'impl<T> CopyWitness<T> {'))
   capability_checks(work,True)
   controls.append(dict(name='copy-bound-removed',detected_by='compiler accepts forbidden non-Copy constructor',compiled=True))

  with tempfile.TemporaryDirectory(prefix='jzap-hash-model-') as tmp:
   path=Path(tmp)/'model.rs';text=model.read_text();old='pending-1+full+1==len';assert old in text;path.write_text(kernel+text.replace(old,'pending+full+1==len'));model_check(path,'pending-control',True)
   controls.append(dict(name='pending-retirement',detected_by='mathematical-model-counterexample'))
   # Weakened maintained callers must be rejected against the product kernels.
   for name,old,new in SOLVER_CONTROLS:
    assert text.count(old)==1,name
    path.write_text(kernel+text.replace(old,new));model_check(path,'control-'+name,True)
    controls.append(dict(name=name,detected_by='Verus rejects the weakened maintained caller'))
  # Changed product kernels must fail their own contracts in the same composition.
  for name,file,old,new in SOURCE_CONTROLS:
   with tempfile.TemporaryDirectory(prefix='jzap-hash-source-') as tmp:
    work=Path(tmp);shutil.copytree(ROOT/'src',work/'src');path=work/file;source=path.read_text();assert source.count(old)==1,name;path.write_text(source.replace(old,new))
    r=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(work/'src/lib.rs'),'-o',str(work/'probe.rlib')],capture_output=True,text=True);assert r.returncode==0,r.stderr[-3000:]
    model_check(composed,'source-'+name,True,work)
    controls.append(dict(name=name,detected_by='Verus rejects the changed product kernel',compiled=True))
  # A changed restated method or caller statement is refused by text comparison.
  for name,file,old,new in TEXT_CONTROLS:
   with tempfile.TemporaryDirectory(prefix='jzap-hash-text-') as tmp:
    work=Path(tmp);shutil.copytree(ROOT/'src',work/'src');path=work/file;source=path.read_text();assert source.count(old)==1,name;path.write_text(source.replace(old,new))
    r=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(work/'src/lib.rs'),'-o',str(work/'probe.rlib')],capture_output=True,text=True);assert r.returncode==0,r.stderr[-3000:]
    try:correspondence(work)
    except AssertionError as e:assert 'HashMap' in str(e)
    else:raise AssertionError('accepted changed product caller: '+name)
    controls.append(dict(name=name,detected_by='source correspondence comparison',compiled=True))
  if a.kani:
   with tempfile.TemporaryDirectory(prefix='jzap-hash-kani-') as tmp:
    work=Path(tmp);shutil.copytree(ROOT/'src',work/'src');shutil.copytree(ROOT/'examples',work/'examples')
    for file in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/file,work/file)
    path=work/'src/collections/hash_map/raw.rs';text=path.read_text();old='            self.slot(index).read()';assert text.count(old)==1;path.write_text(text.replace(old,'            self.slot(add(index, 1) & self.mask).read()'))
    r,log=run('wrong-slot-control',['cargo','kani','--manifest-path',str(work/'Cargo.toml'),'--lib','--harness',HARNESSES[3],'--exact','-Z','unstable-options','--cbmc-args','--object-bits','12'],env=dict(os.environ,CARGO_TARGET_DIR=str(work/'target')))
    assert r.returncode and 'VERIFICATION:- FAILED' in log and 'error[E' not in log,log[-5000:]
    controls.append(dict(name='actual-wrong-slot',detected_by='actual-Rust-Kani-counterexample',compiled=True))
    # The raw clone publishes metadata only after all bytes have been copied.
    # An omitted length publication must be rejected by the actual clone kernel.
    path=work/'src/collections/hash_map/raw.rs';text=(ROOT/'src/collections/hash_map/raw.rs').read_text();old='            len: self.len,';assert text.count(old)==1;path.write_text(text.replace(old,'            len: Checked::new(0),'))
    r,log=run('copy-publication-control',['cargo','kani','--manifest-path',str(work/'Cargo.toml'),'--lib','--harness',HARNESSES[3],'--exact','-Z','unstable-options','--cbmc-args','--object-bits','12'],env=dict(os.environ,CARGO_TARGET_DIR=str(work/'target')))
    assert r.returncode and 'VERIFICATION:- FAILED' in log and 'error[E' not in log,log[-5000:]
    controls.append(dict(name='actual-copy-publication',detected_by='actual-Rust-Kani-counterexample',compiled=True))
 assert before==fingerprint(),'HashMap input changed during verification'
 report=dict(status='pass',complete=a.kani and a.mutations,source_unchanged=True,source_sha256=before,phases=phases,bindings=bindings,capability=capability_results,direct=direct,verified_functions=re.findall(r'^(?:proof )?fn (\w+)',model.read_text(),re.M),callers=callers,results=results,kani_harnesses=HARNESSES if a.kani else [],controls=controls,scope=__doc__)
 (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print('HashMap proof composition passed; complete=',report['complete'])
if __name__=='__main__':main()
