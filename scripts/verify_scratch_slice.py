#!/usr/bin/env python3
"""Direct original Rust kernels and reviewed fixed-storage/checked-view callers.

Source seals require review after edits; they are not ownership proofs. Kani
checks bounded typed memory and all-word endpoints; direct Verus checks numeric
contracts. Compiler ownership, Miri and ASan cover the separate borrow/provenance
boundary. No source body is projected into another proof language.
"""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
from scanner_tools import scanner
from scratch_callers import BODIES,CALLERS,SITES
import storage_kernel
from verify_edit_loops import block
from verus_composition import run as compose
ROOT=Path(__file__).resolve().parents[1]
FILES=['src/scratch.rs','src/scratch/owned.rs','src/slice.rs','src/slice/geometry.rs']
REVIEWED={'src/scratch.rs': '56604f13a85487849fee3e35d1d820abb9de640a7b0dfc50a830f1c027b06106', 'src/scratch/owned.rs': 'cc59c39a1bf9e262fd1e7f64c9640787e68c07d8dd455a9b61d3edbd399d2bff', 'src/slice.rs': 'f8ec7cfc59f1fb35ae72803523942fe271afd813e2dbd4654d16b49aded80774', 'src/slice/geometry.rs': 'ddb4bf94421fca8190c22bdab22cd5936e2baa7fb2cd5e9c30c276e94e1d6600'}
CORE_SLICE='decea260b5d73957cd573e827257d8c00d6614fb1cb10f8001d54e3c264de08d'
HARNESSES=['scratch::proofs::'+n for n in ('actual_borrowed_admission_rollback_and_bytes','actual_owned_storage_never_grows_after_construction','actual_zst_all_word_lengths_retire_without_wrap','actual_noncopy_staging_and_missing_write')]+['slice::proofs::'+n for n in ('endpoints_all_words','actual_swap_and_overlap_preserve_checked_state','actual_bound_normalization_all_words','actual_copy_swap_and_disjoint_partition_guards')]
def is_scratch_slice(file):return file in FILES
MODEL=ROOT/'tools/memory-proof/scratch.verus.rs'
def maintained(text,name):
 start=text.index('\nfn '+name+'(');return re.sub(r'\s+','',re.sub(r'//[^\n]*','',text[start:text.index('\n}\n',start)+3]))
def correspondence(root=ROOT):
 root=Path(root);compact=lambda s:re.sub(r'\s+','',re.sub(r'//[^\n]*','',s));model=MODEL.read_text()
 for name,calls in CALLERS.items():
  text=maintained(model,name)
  for call in calls:assert call in text,('maintained scratch caller lost a product call',name,call)
 for (file,anchor,head),expected in BODIES.items():
  source=(root/'src'/file).read_text();assert source.count(anchor or head)==1,('scratch caller anchor is ambiguous',file,anchor,head)
  if anchor:source=source[source.index(anchor):]
  assert compact(block(source,head))==expected,('Scratch caller body changed',file,anchor,head)
 assert set(SITES.values())<=set(CALLERS),'scratch site names an unknown maintained caller'
 return dict(callers=sorted(CALLERS),bodies=sorted(' '.join(k) for k in BODIES))
# Weakened maintained callers; Verus must report an error for each.
SOLVER_CONTROLS=[
 ('push_past_capacity','crate::slice::geometry::admitted_end(len,1,cap) {','crate::slice::geometry::admitted_end(len,0,cap) {'),
 ('pop_empty',' requires raw_valid(s),s.cap==cap,0<len,len<=cap,\n ensures index<cap,index+1==len,',' requires raw_valid(s),s.cap==cap,len<=cap,\n ensures index<cap,index+1==len,'),
 ('truncate_unchecked',' if len>=old {\n  return 0;\n }\n',''),
 ('copy_from_capacity_free_length',' match crate::slice::geometry::admitted_end(len,count,cap) {',' match crate::slice::geometry::admitted_end(0,count,cap) {'),
 ('cloned_index_before_start','   if start<=index && index<end {','   if index<end {'),
 ('resize_without_admission',' if crate::slice::geometry::admitted_end(0,new_len,cap).is_none() {\n  return None;\n }\n',''),
 ('admitted_length_shifted',' crate::slice::geometry::admitted_end(0,new_len,cap)\n}',' crate::slice::geometry::admitted_end(0,new_len,new_len)\n}'),
 # The index facts are unchanged in the next six; only the storage call or its premise is.
 ('truncate_suffix_from_old',' let _suffix=slice_caller(s,len,count);',' let _suffix=slice_caller(s,old,count);'),
 ('copy_without_disjoint_storage','  source.base as int+source.cap as int*source.size as int<=s.base as int\n   || s.base as int+s.cap as int*s.size as int<=source.base as int,\n',''),
 ('copy_destination_at_end','   distinct_nonoverlap_caller(source,s,0,len,count);','   distinct_nonoverlap_caller(source,s,0,end,count);'),
 ('prefix_beyond_capacity',' requires raw_valid(s),len<=s.cap,\n ensures doc_slice_from_raw_parts',' requires raw_valid(s),\n ensures doc_slice_from_raw_parts'),
 ('pop_slot_at_length',' let _slot=initialized_caller(s,index,cap);\n index',' let _slot=initialized_caller(s,len,cap);\n index'),
 ('push_without_valid_storage',' requires raw_valid(s),s.cap==cap,len<=cap,\n ensures match out {Some((index,end))',' requires s.cap==cap,len<=cap,\n ensures match out {Some((index,end))'),
]
# Compiled product changes that the whole-body comparison must refuse.
TEXT_CONTROLS=[
 ('push-count','scratch.rs','geometry::admitted_end(self.len(), 1, self.capacity())','geometry::admitted_end(self.len(), 0, self.capacity())'),
 ('truncate-count','scratch.rs','sub(old, len),','old,'),
 ('copy-destination','scratch.rs','self.storage.as_mut_ptr().cast::<T>().add(self.len()),','self.storage.as_mut_ptr().cast::<T>().add(end),'),
 ('resize-next','scratch.rs','let next = geometry::initialized_next(index, new_len);','let next = geometry::initialized_next(new_len, new_len);'),
 ('cloned-offset','scratch.rs','.get(sub(index, start))','.get(index)'),
 ('owned-copy-start','scratch/owned.rs','self.values.as_mut_ptr().add(start),','self.values.as_mut_ptr().add(end),'),
 ('owned-clone-from-admission','scratch/owned.rs','geometry::admitted_end(0, source.len(), self.capacity()).ok_or(Error::Capacity)?;\n        if source.is_empty() {','geometry::admitted_end(0, 0, self.capacity()).ok_or(Error::Capacity)?;\n        if source.is_empty() {'),
]

def bindings(root=ROOT):
 root=Path(root)
 actual={'src/scratch.rs','src/slice.rs',*(str(p.relative_to(root)) for d in ('scratch','slice') for p in (root/'src'/d).glob('*.rs') if not p.name.endswith('proofs.rs'))}
 assert actual==set(FILES),'Scratch/slice verification domain changed'
 hashes={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in FILES}
 assert hashes==REVIEWED,'Fixed storage / checked-view callers require review'
 rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True));result=[]
 for row in rows:
  fn=row['function'];file=row['file']
  if fn=='try_resize': h=HARNESSES[2]
  elif fn=='try_clone_from': h=HARNESSES[3]
  elif file.endswith('/owned.rs'):h=HARNESSES[1]
  else:h=HARNESSES[0]
  caller=SITES.get((file.split('/')[-1],fn))
  result.append(dict(site=row,contract=caller or 'kani:'+h,also=['kani:'+h] if caller else [],role='reviewed fixed suffix / initialized prefix / retired ownership / unchanged backing',boundary=__doc__))
 return hashes,result

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--kani',action='store_true');p.add_argument('--mutations',action='store_true');a=p.parse_args()
 out=ROOT/'target/verification/scratch-slice/proofs';out.mkdir(parents=True,exist_ok=True);(out/'summary.json').unlink(missing_ok=True)
 paths=sorted((ROOT/'src').rglob('*.rs'))+sorted((ROOT/'tests').rglob('*.rs'))+[Path(__file__),MODEL,ROOT/'scripts/scratch_callers.py',ROOT/'Cargo.toml',ROOT/'Cargo.lock',*storage_kernel.INPUTS]
 fingerprint=lambda:hashlib.sha256(b''.join(str(f.relative_to(ROOT)).encode()+b'\0'+f.read_bytes() for f in paths)).hexdigest()
 before=fingerprint()
 def run(name,cmd,env=None):
  with(out/(name+'.log')).open('w')as log:r=subprocess.run(cmd,cwd=ROOT,env=env,stdout=log,stderr=log)
  return r,(out/(name+'.log')).read_text()
 r,log=run('ast-build',['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bin','unsafe_sites','--target-dir','target/panic-inventory']);assert r.returncode==0,log[-2000:]
 hashes,sites=bindings();controls=[];callers=correspondence();results=[];caller_controls=[]
 verus=Path(os.environ.get('JZAP_VERUS','/workspace/.devtools/verus/verus-x86-linux/verus'));model=storage_kernel.kernel()+MODEL.read_text()
 def verify(name,text,negative=False):
  file=out/(name+'.generated.rs');file.write_text(text);command,r=compose(verus,ROOT,file,'jzap_scratch_proof')
  log=r.stdout+r.stderr;(out/(name+'.verus.log')).write_text(log);counts=re.findall(r'verification results:: (\d+) verified, (\d+) errors',log)
  assert counts and 'error[E' not in log and ((r.returncode!=0 and any(int(e) for _,e in counts)) if negative else (r.returncode==0 and all(int(e)==0 for _,e in counts))),(name,log[-4000:])
  results.append(dict(name=name,exit_code=r.returncode,verification=counts,negative_control=negative))
 verify('maintained-callers',model)
 if a.mutations:
  for name,old,new in SOLVER_CONTROLS:assert model.count(old)==1,('scratch solver control pattern is not unique',name,model.count(old))
  for name,old,new in SOLVER_CONTROLS:
   verify('caller-control-'+name,model.replace(old,new),True);caller_controls.append(dict(name=name,detected_by='Verus rejects the weakened maintained caller'))
  for name,file,old,new in TEXT_CONTROLS:
   with tempfile.TemporaryDirectory(prefix='jzap-scratch-text-') as tmp:
    root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');path=root/'src'/file;text=path.read_text();assert text.count(old)==1,(name,text.count(old));path.write_text(text.replace(old,new))
    built=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(root/'probe.rlib')],capture_output=True,text=True);assert built.returncode==0,(name,built.stderr[-2000:])
    try:correspondence(root)
    except AssertionError as error:assert 'Scratch caller body changed' in str(error),str(error)
    else:raise AssertionError(('changed scratch caller body accepted',name))
    caller_controls.append(dict(name=name,compiled=True,detected_by='source correspondence comparison'))
 core=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/core/src/slice/mod.rs'
 assert hashlib.sha256(core.read_bytes()).hexdigest()==CORE_SLICE,'Pinned core slice predicates changed'
 if a.kani:
  for prefix,count in [('scratch',4),('slice',4)]:
   r,log=run('kani-'+prefix,['cargo','kani','--lib','--harness',prefix+'::proofs::','-Z','unstable-options','--cbmc-args','--object-bits','12'])
   assert r.returncode==0 and f'Complete - {count} successfully verified harnesses, 0 failures, {count} total.' in log,log[-4000:]
 if a.mutations:
  cases=[
   ('owned-refusal','src/scratch/owned.rs','self.values.push_within_capacity(value)','self.values.try_push(value).map(|()| self.values.as_mut_slice().last_mut().unwrap())','fixed_owned_storage_and_failure_injection'),
   ('borrowed-copy-end','src/scratch.rs','self.len = Checked::new(end);','self.len = Checked::new(sub(end, 1));','bytes_remain_valid_after_failure_pop_and_clear'),
   ('rollback-retirement','src/scratch.rs','self.owner.truncate(self.original)','self.owner.truncate(self.owner.len())','borrowed_initialization_rolls_back_each_slot_once'),
   ('byte-offset','src/scratch.rs','self.storage.as_mut_ptr().cast::<T>().add(self.len()),','self.storage.as_mut_ptr().cast::<T>(),','bytes_remain_valid_after_failure_pop_and_clear'),
   ('drop-retirement','src/scratch.rs','sub(old, len),','sub(sub(old, len), 1),','destructor_unwind_retires_suffix_before_cleanup'),
   ('copy-length-admission','src/slice.rs','if self.len() != source.len() {','if self.len() == source.len() {','exhaustive_checked_slices_and_overlapping_copy'),
  ]
  for name,file,old,new,test in cases:
   with tempfile.TemporaryDirectory(prefix='jzap-scratch-') as tmp:
    work=Path(tmp)
    for d in ('src','examples','tests'):shutil.copytree(ROOT/d,work/d)
    for f in ('Cargo.toml','Cargo.lock'):shutil.copyfile(ROOT/f,work/f)
    path=work/file;text=path.read_text();assert old in text,(name,'anchor changed');path.write_text(text.replace(old,new))
    env=dict(os.environ,CARGO_TARGET_DIR=str(ROOT/'target/verification/scratch-slice/mutation-build'))
    r,log=run(name,['cargo','test','--locked','--offline','--manifest-path',str(work/'Cargo.toml'),'--all-features','--test','scratch_slice',test,'--','--exact'],env)
    assert r.returncode!=0 and 'test result: FAILED' in log and 'error[E' not in log,(name,log[-3500:])
    controls.append(dict(name=name,compiled=True,detected_by='actual-product-regression'))
 assert fingerprint()==before,'Proof inputs changed during execution'
 report=dict(status='pass',complete=a.kani and a.mutations,source_unchanged=True,source_sha256=before,phases=hashes,bindings=sites,kani_harnesses=HARNESSES if a.kani else [],controls=controls,callers=callers,results=results,caller_controls=caller_controls,verified_functions=re.findall(r'^fn (\w+)\(',MODEL.read_text(),re.M),scope=__doc__)
 (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print('Scratch/slice checks passed; complete=',report['complete'])
if __name__=='__main__':main()
