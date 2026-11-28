#!/usr/bin/env python3
"""Direct ring kernels and reviewed callers; no Python source-body projection.

Direct Verus checks all-word geometry. Kani checks original bounded movement,
cleanup and refusal kernels. Source seals require re-review on edits; they are
not proofs of aliasing, allocation, arbitrary callbacks or arbitrary heap size.
Miri/ASan and compiler ownership rules cover the separate memory boundary.
"""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
from scanner_tools import scanner
import maintained_callers
import storage_kernel
from deque_callers import BODIES,CALLERS,SITES,RESTATED_PRODUCT,RESTATED_MAINTAINED
ROOT=Path(__file__).resolve().parents[1]
FILES=['src/deque.rs','src/deque/edit.rs','src/deque/geometry.rs','src/deque/iter.rs','src/deque/traits.rs']
REVIEWED={'src/deque.rs': 'aa2cf4b2694dd6e39e8d68075759ea76bb94d9de8ce6008b4b23168d36426373', 'src/deque/edit.rs': '08beeae6c763d33c33b4af9fbe6e0be2ae17fccef484a730d70662832c16947e', 'src/deque/geometry.rs': '039297be39c9b1f96703e3d1de4be6a856d3ac651f44d7dd139be1de6d739fd9', 'src/deque/iter.rs': 'c13c5ed17cbf87eaa63ee3ce56eb0e48b513dc8767eab25b1a316c180c302db5', 'src/deque/traits.rs': '8cff21258c02c1db120b562539ae24220bdda1dc6bb4a12e5e3df9b1688b9445'} # Reviewed bounds, ownership retirement and storage/policy correspondence.
HARNESSES=['deque::proofs::'+n for n in ('geometry_all_words','actual_ring_moves_and_contiguous_growth','actual_insert_remove_and_drain','actual_retention_error_and_split_append','actual_shrink_refusal_restores_ring_and_accounting','actual_clone_limits_and_consuming_fold','actual_copy_clone_from_reuses_admitted_storage','actual_empty_copy_extension_admission_and_wrap','actual_no_drop_truncation_all_word_lengths')]
def is_deque(file):return file in FILES
MODEL=ROOT/'tools/memory-proof/deque.verus.rs'
def correspondence(root=ROOT):
 assert BODIES[('deque.rs','','fn physical(')]==RESTATED_PRODUCT,'VecDeque caller body changed: physical'
 result=maintained_callers.correspondence(root,MODEL,BODIES,CALLERS,{k:v for k,v in SITES.items()},'VecDeque',shared=storage_kernel.FILES[3].read_text())
 assert maintained_callers.maintained(MODEL.read_text(),'deque_physical').endswith('{'+RESTATED_MAINTAINED),'VecDeque restated maintained body changed'
 return dict(result,restated=['deque_physical'])
G='crate::deque::geometry::'
# Weakened maintained callers; Verus must report an error for each.
SOLVER_CONTROLS=[
 ('ring_head_at_capacity',' len<=cap && (cap==0 ==> head==0) && (cap>0 ==> head<cap)',' len<=cap && (cap==0 ==> head==0) && (cap>0 ==> head<=cap)'),
 ('get_unchecked_index',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<len,\n ensures out<cap,zst ==> out==0,',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),\n ensures out<cap,zst ==> out==0,'),
 ('push_back_full',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len<cap,\n ensures out.1==len+1,out.1<=cap,zst',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),\n ensures out.1==len+1,out.1<=cap,zst'),
 ('push_front_full',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len<cap,\n ensures out.1==len+1,ring(out.0,out.1,cap,zst),\n{\n let mut head=head;\n if !zst {',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),\n ensures out.1==len+1,ring(out.0,out.1,cap,zst),\n{\n let mut head=head;\n if !zst {'),
 ('pop_front_empty',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len>0,\n ensures out.0==head,ring(',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),\n ensures out.0==head,ring('),
 ('pop_back_spare_before_decrement',' let spare=last<cap;',' let spare=len<cap;'),
 ('contiguous_kind_one_whole_length','   unordered_nonoverlap_caller(s,head,0,a);','   unordered_nonoverlap_caller(s,head,0,len);'),
 ('contiguous_wrong_head','   assert(b<=len);\n   head=free;','   assert(b<=len);\n   head=b;'),
 ('shrink_restore_whole','    overlapping_copy_caller(s,b,head,a);','    overlapping_copy_caller(s,b,head,len);'),
 ('shrink_empty_head_kept',' } else if moved || len==0 {',' } else if moved {'),
 ('grow_same_capacity',' requires raw_valid(n),n.cap==new,ring(head,len,old,false),old>0,old<new,new<=isize::MAX,',' requires raw_valid(n),n.cap==new,ring(head,len,old,false),old>0,old<=new,new<=isize::MAX,'),
 ('copy_ring_source_at_capacity',' requires raw_valid(s),s.cap==cap,cap>0,src<cap,dst<cap,count<=cap,',' requires raw_valid(s),s.cap==cap,cap>0,src<=cap,dst<cap,count<=cap,'),
 ('insert_without_free_slot',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<=len,len<cap,',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<=len,'),
 ('remove_at_length',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<len,\n ensures out.1+1==len,ring(out.0,out.1,cap,zst),',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<=len,\n ensures out.1+1==len,ring(out.0,out.1,cap,zst),'),
 ('rotate_past_length',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,false),0<n,n<len,',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,false),0<n,'),
 ('write_slice_first_unbounded',' let first=if count<room {count} else {room};\n let rest=crate::arithmetic::bounded::wrapping_sub(count,first);\n // `copy_nonoverlapping(source.as_ptr(), ptr.add(physical), first)`,',' let first=count;\n let rest=crate::arithmetic::bounded::wrapping_sub(count,first);\n // `copy_nonoverlapping(source.as_ptr(), ptr.add(physical), first)`,'),
 ('append_without_reservation',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len as int+a as int+b as int<=cap,',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len as int+a as int<=cap,'),
 ('split_off_small_target','  ring(head,len,cap,zst),at<=len,target>=len-at,','  ring(head,len,cap,zst),at<=len,'),
 ('search_inclusive',' while left<right\n',' while left<=right\n'),
 ('retain_foreign_length',' let mut state=crate::compaction::Compaction::full(len);',' let mut state=crate::compaction::Compaction::full(cap);'),
 ('clone_second_segment',' deque_clone_slice_caller(s,second,a,b,cap);',' deque_clone_slice_caller(s,second,b,b,cap);'),
 ('resize_without_reservation','  ring(head,len,cap,false),len<new_len,new_len<=cap,','  ring(head,len,cap,false),len<new_len,'),
 # The index facts are unchanged in the next eight; only the storage call or its premise is.
 ('slices_back_from_head',' let _back=slice_caller(s,0,b);',' let _back=slice_caller(s,head,b);'),
 ('slot_at_capacity',' requires raw_valid(s),s.cap==cap,physical<cap,',' requires raw_valid(s),s.cap==cap,physical<=cap,'),
 ('write_slice_source_inside_ring',' requires raw_valid(s),s.cap==cap,raw_valid(source),source.cap==count,same_element(source,s),apart(source,s),\n  zst || count==0',' requires raw_valid(s),s.cap==cap,raw_valid(source),source.cap==count,same_element(source,s),\n  zst || count==0'),
 ('grow_copy_onto_itself','   unordered_nonoverlap_caller(n,0,old,b);','   unordered_nonoverlap_caller(n,0,0,b);'),
 ('split_off_other_element_type',' requires raw_valid(s),s.cap==cap,raw_valid(result),result.cap==target,same_element(s,result),apart(s,result),',' requires raw_valid(s),s.cap==cap,raw_valid(result),result.cap==target,apart(s,result),'),
 ('retain_copy_two_elements','      overlapping_copy_caller(s,from,to,1);','      overlapping_copy_caller(s,from,to,2);'),
 ('drain_drop_tail_beyond_length',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),start<=end,end<=len,tail==len-end,',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),start<=end,end<=len,'),
 ('drain_drop_without_storage',' requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),start<=end,end<=len,tail==len-end,',' requires s.cap==cap,ring(head,len,cap,zst),start<=end,end<=len,tail==len-end,'),
]
# Compiled product changes that the whole-body comparison must refuse.
TEXT_CONTROLS=[
 ('physical-kernel','deque.rs','geometry::allocated_physical(self.head.get(), index, self.capacity())\n        }\n    }','geometry::physical(self.head.get(), index, self.capacity())\n        }\n    }'),
 ('get-bound','deque.rs','    pub fn get(&self, index: usize) -> Result<&T, Error> {\n        if index >= self.len() {','    pub fn get(&self, index: usize) -> Result<&T, Error> {\n        if index > self.len() {'),
 ('push-back-capacity-test','deque.rs','        let len = self.len();\n        if len == self.capacity()','        let len = self.len();\n        if len > self.capacity()'),
 ('pop-front-spare','deque.rs','let old_head = self.head.get();\n        self.len = Checked::new(geometry::after_pop(self.len(), self.capacity()));','let old_head = self.head.get();\n        self.len = Checked::new(self.len());'),
 ('contiguous-kind-one-copy','deque.rs','ptr::copy_nonoverlapping(p.add(head), p, a);','ptr::copy_nonoverlapping(p.add(head), p, len);'),
 ('shrink-moved','deque.rs','let moved = head > sub(new, len);','let moved = head >= sub(new, len);'),
 ('shrink-empty-head','deque.rs','if moved || len == 0 {\n                    self.head = Checked::new(0)','if moved {\n                    self.head = Checked::new(0)'),
 ('grow-copy-target','deque.rs','ptr::copy_nonoverlapping(ptr.as_ptr(), ptr.as_ptr().add(old), b)','ptr::copy_nonoverlapping(ptr.as_ptr(), ptr.as_ptr().add(new), b)'),
 ('copy-ring-chunk','deque/edit.rs','unsafe { ptr::copy(p.add(src), p.add(dst), chunk) };\n                count = left;\n                src = next_src;','unsafe { ptr::copy(p.add(src), p.add(dst), count) };\n                count = left;\n                src = next_src;'),
 ('insert-tail','deque/edit.rs','self.physical(add(index, 1)),\n                    tail,\n                    true,','self.physical(add(index, 1)),\n                    index,\n                    true,'),
 ('remove-tail','deque/edit.rs','let tail = sub(sub(self.len(), index), 1);','let tail = sub(self.len(), index);'),
 ('rotate-left-head','deque/edit.rs','self.head = Checked::new(geometry::allocated_physical(\n            self.head.get(),\n            n,\n            self.capacity(),\n        ));','self.head = Checked::new(geometry::allocated_physical(\n            self.head.get(),\n            right,\n            self.capacity(),\n        ));'),
 ('write-slice-first','deque/edit.rs','let first = source.len().min(sub(self.capacity(), physical));','let first = source.len();'),
 ('split-off-first','deque/edit.rs','let first = count.min(sub(self.capacity(), pos));','let first = count;'),
 ('search-slot','deque/edit.rs','match f(unsafe { &*self.slot(mid) }) {','match f(unsafe { &*self.slot(right) }) {'),
 ('clone-second-offset','deque/traits.rs','result.write_clone_slice(a.len(), b)','result.write_clone_slice(b.len(), b)'),
 ('resize-tail-offset','deque/traits.rs','base.add(add(tail, i))','base.add(add(tail, first))'),
 ('drain-slot','deque/iter.rs','unsafe { self.owner.slot(i).read() }\n        })\n    }\n    fn size_hint','unsafe { self.owner.slot(add(i, 1)).read() }\n        })\n    }\n    fn size_hint'),
]
def inputs(root=ROOT):
 root=Path(root);paths=sorted((root/'src').rglob('*.rs'))+sorted((root/'tests').rglob('*.rs'))
 paths += [root/'Cargo.toml',root/'Cargo.lock',root/'scripts/verify_deque.py',root/'scripts/deque_callers.py',root/'scripts/maintained_callers.py',root/'tools/memory-proof/deque.verus.rs',*(root/f.relative_to(ROOT) for f in storage_kernel.INPUTS)]
 return hashlib.sha256(b''.join(str(p.relative_to(root)).encode()+b'\0'+p.read_bytes() for p in paths)).hexdigest()
def bindings(root=ROOT):
 root=Path(root)
 actual={'src/deque.rs',*(str(p.relative_to(root)) for p in (root/'src/deque').glob('*.rs') if not p.name.endswith('proofs.rs'))}
 assert actual==set(FILES),'VecDeque verification domain changed'
 hashes={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in FILES}
 assert hashes==REVIEWED,'VecDeque caller/ownership correspondence requires review'
 rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True));result=[]
 for row in rows:
  fn=row['function'];file=row['file']
  if fn in ('try_shrink_to','normalize'):h=HARNESSES[4]
  elif fn in ('try_retain','try_retain_mut','try_split_off','try_append'):h=HARNESSES[3]
  elif fn=='try_clone_from':h=HARNESSES[6]
  elif fn=='try_extend_copy':h=HARNESSES[7]
  elif fn=='truncate':h=HARNESSES[8]
  elif 'Repair' in row['context']:h=HARNESSES[3]
  elif fn in ('try_clone','try_clone_from','try_clone_in','write_clone_slice','try_resize','try_resize_with','try_extend','try_extend_copy','write_slice') or file.endswith('/traits.rs'):h=HARNESSES[5]
  elif file.endswith('/iter.rs') and (fn in ('fold','count','last') or 'Retire' in row['context']):h=HARNESSES[5]
  elif fn in ('try_insert','try_insert_mut','remove','swap','swap_remove_front','swap_remove_back','copy_ring') or file.endswith('/iter.rs'):h=HARNESSES[2]
  else:h=HARNESSES[1]
  base=file.split('/')[-1];kind=row['context'].split('<')[0].strip()
  caller=(SITES.get((base,fn,kind)) or SITES.get((base,fn,None))) if row['kind']!='unsafe_impl' and fn else None
  result.append(dict(site=row,contract=caller or 'kani:'+h,also=['kani:'+h] if caller else [],role='reviewed ring index / initialized segment / admitted move / retired ownership',boundary=__doc__))
 return hashes,result
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--kani',action='store_true');p.add_argument('--mutations',action='store_true');a=p.parse_args()
 out=ROOT/'target/verification/vec-deque/proofs';out.mkdir(parents=True,exist_ok=True);(out/'summary.json').unlink(missing_ok=True);before=inputs()
 def run(name,cmd,env=None):
  with(out/(name+'.log')).open('w')as log:r=subprocess.run(cmd,cwd=ROOT,env=env,stdout=log,stderr=log)
  return r,(out/(name+'.log')).read_text()
 r,log=run('ast-build',['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bin','unsafe_sites','--target-dir','target/panic-inventory']);assert r.returncode==0,log[-3000:]
 hashes,sites=bindings();controls=[];callers=correspondence()
 verus=Path(os.environ.get('JZAP_VERUS','/workspace/.devtools/verus/verus-x86-linux/verus'))
 kernel=storage_kernel.kernel()
 results=[maintained_callers.verify(verus,ROOT,out,'jzap_deque_proof','maintained-callers',kernel+MODEL.read_text())];caller_controls=[]
 if a.mutations:
  more,caller_controls=maintained_callers.controls(verus,ROOT,out,'jzap_deque_proof',MODEL,SOLVER_CONTROLS,TEXT_CONTROLS,correspondence,'VecDeque',kernel);results+=more
 reference=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/alloc/src/collections/vec_deque/mod.rs'
 assert hashlib.sha256(reference.read_bytes()).hexdigest()=='b166418e66bb4069f2e9284dc8c02ae788517888c4980554396bcc53a13ae0f6','Pinned std adapter changed'
 if a.kani:
  # By exact name: deque::proofs::closure (issue #16, unit 4) is run by verify_source.py.
  r,log=run('kani',['cargo','kani','--lib','--exact',*[a for n in HARNESSES for a in ('--harness',n)],'-Z','unstable-options','--cbmc-args','--object-bits','12'])
  assert r.returncode==0 and f'Complete - {len(HARNESSES)} successfully verified harnesses, 0 failures, {len(HARNESSES)} total.' in log,log[-5000:]
 if a.mutations:
  cases=[
   ('pop-spare-fact', 'src/deque.rs', 'let spare = self.len() < self.capacity();', 'let spare = self.len() == self.capacity();', 'differential_lifecycle'),
   ('head-wrap','src/deque/geometry.rs','sub(offset, end)','add(offset, end)','exhaustive_ring_moves'),
   ('growth-copy','src/deque.rs','ptr::copy_nonoverlapping(ptr.as_ptr(), ptr.as_ptr().add(old), b)','ptr::copy_nonoverlapping(ptr.as_ptr().add(1), ptr.as_ptr().add(old), b)','exhaustive_ring_moves'),
   ('shrink-restore','src/deque.rs','self.head = Checked::new(head);','self.head = Checked::new(0);','grow_and_shrink_refusal_preserves_exact_split'),
   ('shrink-empty-head','src/deque.rs','if moved || len == 0 {','if moved {','empty_shrink_to_head_resets_head'),
   ('drain-double-owner','src/deque/iter.rs','self.len = Checked::new(start);','self.len = Checked::new(end);','zero_sized_alignment_and_forget'),
   ('retain-restore','src/deque/edit.rs','self.state.restored_len()','self.state.written()','retain_resize_errors_retire_once'),
   ('copy-clone-from-head','src/deque/traits.rs','self.len = Checked::new(0);\n            self.head = Checked::new(0);','self.len = Checked::new(0);\n            self.head = Checked::new(1);','copy_clone_from_reuses_storage'),
   ('empty-extension-head','src/deque/traits.rs','if len == 0 {\n            self.head = Checked::new(0);','if len == 0 {\n            self.head = Checked::new(1);','empty_copy_extension_admission_and_wrap'),
   ('drain-skip-required-drop','src/deque/iter.rs','if !mem::needs_drop::<T>() {','if true {','retain_resize_errors_retire_once'),
   ('no-drop-truncate-length','src/deque.rs','self.len = Checked::new(len);\n            return;','self.len = Checked::new(0);\n            return;','no_drop_truncation_at_full_word_length'),
  ]
  for name,file,old,new,test in cases:
   with tempfile.TemporaryDirectory(prefix='jzap-deque-')as tmp:
    work=Path(tmp)
    for directory in ('src','examples','tests'):shutil.copytree(ROOT/directory,work/directory)
    for f in ('Cargo.toml','Cargo.lock'):shutil.copyfile(ROOT/f,work/f)
    path=work/file;text=path.read_text();assert old in text,(name,'mutation anchor changed');path.write_text(text.replace(old,new))
    env=dict(os.environ,CARGO_TARGET_DIR=str(work/'target'))
    r,log=run(name,['cargo','test','--manifest-path',str(work/'Cargo.toml'),'--locked','--offline','--all-features','--test','deque',test,'--','--exact'],env)
    assert r.returncode!=0 and ('test result: FAILED' in log or (name=='pop-spare-fact' and 'unsafe precondition(s) violated' in log)) and 'error[E' not in log,log[-3500:]
    controls.append(dict(name=name,compiled=True,detected_by='actual-product-regression'))
 assert inputs()==before,'VecDeque proof inputs changed during execution'
 report=dict(status='pass',complete=a.kani and a.mutations,source_unchanged=True,source_sha256=before,phases=hashes,bindings=sites,kani_harnesses=HARNESSES if a.kani else [],controls=controls,callers=callers,results=results,caller_controls=caller_controls,verified_functions=re.findall(r'^fn (\w+)\(',MODEL.read_text(),re.M),scope=__doc__)
 (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print('VecDeque source/kernel checks passed; complete=',report['complete'])
if __name__=='__main__':main()
