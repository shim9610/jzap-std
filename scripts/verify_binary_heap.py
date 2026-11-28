#!/usr/bin/env python3
"""Actual heap kernels and reviewed caller correspondence, without body projection.

The review seals invalidate changed callers; they do not prove Rust semantics.
Direct Verus covers every-word geometry, Kani covers the actual small ownership
kernels. Miri/ASan and compiler checks remain the separate provenance boundary.
"""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
from scanner_tools import scanner
import maintained_callers
import storage_kernel
from heap_callers import BODIES,CALLERS,SITES,RESTATED_MAINTAINED
ROOT=Path(__file__).resolve().parents[1]
FILES=['src/collections/binary_heap.rs', 'src/collections/binary_heap/core.rs', 'src/collections/binary_heap/geometry.rs', 'src/collections/binary_heap/iter.rs']
REVIEWED={'src/collections/binary_heap.rs': '44ddd5b61797f7a1558084ebef99d32578e2c711fa66ea7f715a13d5b758096f', 'src/collections/binary_heap/core.rs': '5cdf53d1f9c40c0884e88eb86c421e17881b9610c97b2b182f26d7158213afa8', 'src/collections/binary_heap/geometry.rs': '40b5ca3551b89f3f89ab657e621b4e71e1cc1c159ed100a979d212be66759179', 'src/collections/binary_heap/iter.rs': '7cd265abcd69b3b489f1f74be4405e30c6e5f0ef0b2e483bf12c9b6d8d7e62e3'}
HARNESSES=['collections::binary_heap::proofs::'+n for n in (
 'geometry_all_words','actual_sifts_preserve_permutation_order_and_bounds',
 'actual_owner_pop_peek_and_drain_protocols','actual_tail_repair_and_sorted_recovery',
 'actual_metered_append_accepts_and_preserves_storage_policy',
 'actual_metered_storage_reuse_refuses_without_owner_or_budget_change')]
def is_binary_heap(file):return file in FILES
MODEL=ROOT/'tools/memory-proof/binary-heap.verus.rs'
RESTATED=[(('collections/binary_heap/core.rs','','fn first_child<'),'heap_first_child',RESTATED_MAINTAINED)]
def correspondence(root=ROOT):
 # The restated body differs from the product only by `zst` standing for the
 # size test and by fully qualified geometry paths; both texts are fixed.
 bodies=dict(BODIES);key=RESTATED[0][0]
 assert BODIES[key]=='ifcore::mem::size_of::<T>()==0{ifpos>=end/2{end}else{children(pos,end).0}}else{child_offset(pos)}','BinaryHeap caller body changed: first_child'
 result=maintained_callers.correspondence(root,MODEL,bodies,CALLERS,SITES,'BinaryHeap',shared=storage_kernel.FILES[3].read_text())
 assert maintained_callers.maintained(MODEL.read_text(),'heap_first_child').endswith('{'+RESTATED_MAINTAINED),'BinaryHeap restated maintained body changed'
 return dict(result,restated=['heap_first_child'])
# Weakened maintained callers; Verus must report an error for each.
SOLVER_CONTROLS=[
 ('first_child_unbounded_slice',' requires pos<end,!zst ==> end<=isize::MAX,\n ensures child>pos,zst ==> child<=end,',' requires pos<end,\n ensures child>pos,zst ==> child<=end,'),
 ('sift_up_from_end',' requires raw_valid(s),end<=s.cap,pos<end,replies@.len()>=end,\n ensures hole<=pos,',' requires raw_valid(s),end<=s.cap,pos<=end,replies@.len()>=end,\n ensures hole<=pos,'),
 ('descend_inclusive_last',' while child<last\n',' while child<=last\n'),
 ('descend_empty_slice',' requires raw_valid(s),end<=s.cap,pos<end,!zst ==> end<=isize::MAX,choices@.len()>=end,stops@.len()>=end,\n ensures pos<=hole,hole<end,',' requires raw_valid(s),end<=s.cap,pos<=end,!zst ==> end<=isize::MAX,choices@.len()>=end,stops@.len()>=end,\n ensures pos<=hole,hole<end,'),
 ('rebuild_without_progress','  pos=crate::arithmetic::bounded::wrapping_sub(pos,1);\n  let _hole=heap_descend_caller(s,pos,end,zst,true,choices,stops);','  let _hole=heap_descend_caller(s,pos,end,zst,true,choices,stops);'),
 ('rebuild_tail_past_len',' while pos<len\n  invariant pos<=len,replies@.len()>=len,',' while pos<=len\n  invariant pos<=len,replies@.len()>=len,'),
 ('pop_empty',' requires raw_valid(s),len<=s.cap,len>0,!zst ==> len<=isize::MAX,choices@.len()>=len,stops@.len()>=len,\n ensures last+1==len,',' requires raw_valid(s),len<=s.cap,!zst ==> len<=isize::MAX,choices@.len()>=len,stops@.len()>=len,\n ensures last+1==len,'),
 ('pop_single_sift',' if last==0 {\n  return last;\n }\n',''),
 ('push_empty',' requires raw_valid(s),len<=s.cap,len>0,replies@.len()>=len,\n{\n let pos=',' requires raw_valid(s),len<=s.cap,replies@.len()>=len,\n{\n let pos='),
 ('sorted_to_zero',' while end>1\n',' while end>0\n'),
 # The index facts are unchanged in the next six; only the storage call or its premise is.
 ('move_onto_itself','  heap_move_caller(s,p,hole,end);','  heap_move_caller(s,hole,hole,end);'),
 ('move_without_distinct_slots',' requires raw_valid(s),index<end,pos<end,index!=pos,end<=s.cap,',' requires raw_valid(s),index<end,pos<end,end<=s.cap,'),
 ('element_outside_storage',' requires raw_valid(s),index<end,end<=s.cap,\n ensures doc_ptr_add',' requires raw_valid(s),index<end,\n ensures doc_ptr_add'),
 ('pop_replace_past_end',' let _root=replace_caller(s,0);',' let _root=replace_caller(s,len);'),
 ('sorted_swap_at_length','  let _last=heap_element_caller(s,end,len);','  let _last=heap_element_caller(s,len,len);'),
 ('sift_up_storage_shorter_than_slice',' requires raw_valid(s),end<=s.cap,pos<end,replies@.len()>=end,\n ensures hole<=pos,',' requires raw_valid(s),pos<end,replies@.len()>=end,\n ensures hole<=pos,'),
]
# Compiled product changes that the whole-body comparison must refuse.
TEXT_CONTROLS=[
 ('sift-up-bound','collections/binary_heap/core.rs','    let mut hole = unsafe { Hole::new(data, pos) };\n    while hole.pos > start {\n        let p = parent(hole.pos);','    let mut hole = unsafe { Hole::new(data, pos) };\n    while hole.pos >= start {\n        let p = parent(hole.pos);'),
 ('sift-down-right','collections/binary_heap/core.rs','let choose_right = unsafe { hole.get(child) <= hole.get(right) };','let choose_right = unsafe { hole.get(child) <= hole.get(child) };'),
 ('first-child-zst-frontier','collections/binary_heap/core.rs','if pos >= end / 2 {','if pos > end / 2 {'),
 ('rebuild-start','collections/binary_heap/core.rs','let mut pos = end / 2;','let mut pos = end;'),
 ('rebuild-tail-root','collections/binary_heap/core.rs','unsafe { sift_up(data, 0, pos) };','unsafe { sift_up(data, 0, len) };'),
 ('pop-sift-end','collections/binary_heap/core.rs','unsafe { sift_bottom(data.as_mut_slice(), 0, last) };','unsafe { sift_bottom(data.as_mut_slice(), 0, crate::arithmetic::bounded::wrapping_add(last, 1)) };'),
 ('push-position','collections/binary_heap.rs','let pos = crate::arithmetic::bounded::wrapping_sub(self.len(), 1);','let pos = self.len();'),
 ('sorted-loop-bound','collections/binary_heap.rs','while end > 1 {','while end > 0 {'),
]
def inputs(root=ROOT):
 root=Path(root)
 paths=sorted((root/'src').rglob('*.rs'))+sorted((root/'tests').rglob('*.rs'))
 paths += [root/'Cargo.toml',root/'Cargo.lock',root/'scripts/verify_binary_heap.py',root/'scripts/heap_callers.py',root/'scripts/maintained_callers.py',root/'tools/memory-proof/binary-heap.verus.rs',*(root/f.relative_to(ROOT) for f in storage_kernel.INPUTS)]
 return hashlib.sha256(b''.join(str(f.relative_to(root)).encode()+b'\0'+f.read_bytes() for f in paths)).hexdigest()
def bindings(root=ROOT):
 root=Path(root)
 actual={'src/collections/binary_heap.rs',*(str(p.relative_to(root)) for p in (root/'src/collections/binary_heap').glob('*.rs') if not p.name.endswith('proofs.rs'))}
 assert actual==set(FILES),'BinaryHeap domain changed'
 phases={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in FILES}
 assert phases==REVIEWED,'BinaryHeap caller/ownership correspondence requires review'
 rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True))
 result=[]
 for row in rows:
  fn=row['function']
  if fn in ('set_len','pop_nonempty','peek_mut','deref','deref_mut','pop') or row['file'].endswith('/iter.rs') or (row['file']=='src/collections/binary_heap.rs' and fn=='drop'):
   contract=HARNESSES[2]
  elif fn in ('rebuild_tail','into_sorted_vec','try_append','try_extend'):contract=HARNESSES[3]
  else:contract=HARNESSES[1]
  caller=SITES.get((row['file'].split('/')[-1],fn)) if row['kind']!='unsafe_impl' else None
  result.append(dict(site=row,contract=caller or 'kani:'+contract,also=['kani:'+contract] if caller else [],role='reviewed guarded heap frontier / initialized hole / exclusive peek / retired drain',boundary=__doc__))
 return phases,result
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--kani',action='store_true');p.add_argument('--mutations',action='store_true');a=p.parse_args()
 out=ROOT/'target/verification/binary-heap/proofs';out.mkdir(parents=True,exist_ok=True);(out/'summary.json').unlink(missing_ok=True)
 before=inputs()
 def run(name,cmd,env=None):
  with (out/(name+'.log')).open('w') as log:r=subprocess.run(cmd,cwd=ROOT,env=env,stdout=log,stderr=log)
  return r,(out/(name+'.log')).read_text()
 r,log=run('ast-build',['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bin','unsafe_sites','--target-dir','target/panic-inventory']);assert r.returncode==0,log[-3000:]
 phases,sites=bindings();controls=[];callers=correspondence()
 verus=Path(os.environ.get('JZAP_VERUS','/workspace/.devtools/verus/verus-x86-linux/verus'))
 kernel=storage_kernel.kernel()
 results=[maintained_callers.verify(verus,ROOT,out,'jzap_heap_proof','maintained-callers',kernel+MODEL.read_text())];caller_controls=[]
 if a.mutations:
  more,caller_controls=maintained_callers.controls(verus,ROOT,out,'jzap_heap_proof',MODEL,SOLVER_CONTROLS,TEXT_CONTROLS,correspondence,'BinaryHeap',kernel);results+=more
 if a.kani:
  # By exact name: the closure harnesses of proofs::closure (issue #16, unit 7) are in the heap-closure group.
  r,log=run('kani',['cargo','kani','--lib','--exact',*[a for n in HARNESSES for a in ('--harness',n)],'-Z','unstable-options','--cbmc-args','--object-bits','12'])
  expected=f'Complete - {len(HARNESSES)} successfully verified harnesses, 0 failures, {len(HARNESSES)} total.'
  assert r.returncode==0 and expected in log,log[-5000:]
 if a.mutations:
  cases=[('hole-provenance','src/collections/binary_heap/core.rs','let data = self.data.as_mut_ptr();\n        unsafe { ptr::copy_nonoverlapping(data.add(index), data.add(self.pos), 1) };','unsafe { ptr::copy_nonoverlapping(self.data.as_ptr().add(index), self.data.as_mut_ptr().add(self.pos), 1) };','allocation_refusal_returns_input_and_preserves_both_owners'),
   ('sift-reversed','src/collections/binary_heap/core.rs','&*hole.value <= unsafe { hole.get(p) }','&*hole.value >= unsafe { hole.get(p) }','differential_random_lifecycle'),
   ('peek-hidden-tail','src/collections/binary_heap.rs','core::set_len(&mut self.heap.data, len.get())','core::set_len(&mut self.heap.data, 1)','peek_mut_pop_read_mutation_forget_and_drain'),
   ('drain-double-owner','src/collections/binary_heap/iter.rs','super::core::set_len(&mut heap.data, 0)','super::core::set_len(&mut heap.data, 1)','peek_mut_pop_read_mutation_forget_and_drain'),
   ('append-ledger-identity','src/budget.rs','=> core::ptr::eq(a, b)', '=> true','metered_append_preserves_policy_handles_and_concurrent_domains'),
   ('append-policy-handle','src/vec.rs','mem::swap(&mut self.cap, &mut other.cap);','mem::swap(&mut self.cap, &mut other.cap); mem::swap(&mut self.policy, &mut other.policy);', 'metered_append_preserves_policy_handles_and_concurrent_domains'),
   ('append-exact-headroom','src/vec.rs','other.try_reserve_exact(self.len())?;','other.try_reserve(self.len())?;', 'metered_append_exact_growth_preserves_admission_and_refusal'),
   ('append-original-spare','src/vec.rs','if self.capacity() >= needed {','if false {', 'metered_append_keeps_original_spare_capacity_and_distinct_limits'),
   ('append-error-propagation','src/collections/binary_heap.rs','let reused = self.data.try_reuse_larger_buffer(&mut other.data)?;', 'let reused = self.data.try_reuse_larger_buffer(&mut other.data).unwrap_or(false);', 'metered_append_exact_growth_preserves_admission_and_refusal')]
  for name,file,old,new,test in cases:
   with tempfile.TemporaryDirectory(prefix='jzap-heap-') as tmp:
    work=Path(tmp);shutil.copytree(ROOT/'src',work/'src');shutil.copytree(ROOT/'examples',work/'examples');shutil.copytree(ROOT/'tests',work/'tests')
    for f in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/f,work/f)
    path=work/file;text=path.read_text();assert old in text,(name,'mutation anchor changed');path.write_text(text.replace(old,new))
    env=dict(os.environ,CARGO_TARGET_DIR=str(work/'target'))
    r,log=run(name,['cargo',*(['+nightly-2026-10-30','miri'] if name=='hole-provenance' else []),'test','--manifest-path',str(work/'Cargo.toml'),'--locked','--offline','--all-features','--test','binary_heap',test,'--','--exact'],env)
    assert r.returncode and ('Undefined Behavior' if name=='hole-provenance' else 'test result: FAILED') in log and 'error[E' not in log,log[-4000:]
    controls.append(dict(name=name,compiled=True,detected_by='actual-product-Miri-provenance' if name=='hole-provenance' else 'actual-product-runtime-regression'))
 assert before==inputs(),'BinaryHeap inputs changed'
 report=dict(status='pass',complete=a.kani and a.mutations,source_unchanged=True,source_sha256=before,phases=phases,bindings=sites,kani_harnesses=HARNESSES if a.kani else [],controls=controls,callers=callers,results=results,caller_controls=caller_controls,verified_functions=re.findall(r'^fn (\w+)\(',MODEL.read_text(),re.M),scope=__doc__)
 (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print('BinaryHeap source/kernel checks passed; complete=',report['complete'])
if __name__=='__main__':main()
