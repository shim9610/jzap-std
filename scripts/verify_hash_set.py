#!/usr/bin/env python3
"""Check safe set delegation plus actual Rust replacement/algebra kernels.

Source seals invalidate reviews; they are not semantic proofs. No positive
Rust body is projected through Python. Pointer/alias/drop scenarios use the
separate Miri/sanitizer suite; table geometry has its existing direct proofs.
"""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
from scanner_tools import scanner
ROOT=Path(__file__).resolve().parents[1]
FILES=['src/collections/hash_set.rs','src/collections/hash_set/algebra.rs','src/collections/hash_set/iter.rs']
REVIEWED={'src/collections/hash_set.rs': 'bece80151dadeecb01477c0e6e0cdeda36173a70d725c5b274da33b62be3c7f8', 'src/collections/hash_set/algebra.rs': '04baaa52ab3882253ecc44ab6bb7b6b878cff6433ecc79a77f0d9881df651033', 'src/collections/hash_set/iter.rs': '0dcd39eb9d7e12bed0b2e5e71f97744b59a3c51e564d3c9490134296f13fe383'} # Reviewed safe delegation and finite iterator progress.
HARNESSES=['collections::hash_map::proofs::hash_set_replace_preserves_slot_class_length_and_credit','collections::hash_set::proofs::algebra_hint_all_machine_word_lengths','collections::hash_set::proofs::algebra_filter_and_chain_preserve_progress_results_and_hints']
def is_hash_set(file):
 # Exact domain only: new files must not inherit the separate set checks.
 return file in FILES
def bindings(root=ROOT):
 root=Path(root)
 actual={'src/collections/hash_set.rs',*(str(p.relative_to(root)) for p in (root/'src/collections/hash_set').glob('*.rs') if not p.name.endswith('proofs.rs'))}
 assert actual==set(FILES),'HashSet verification domain changed'
 hashes={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in FILES}
 assert hashes==REVIEWED,'HashSet delegation/progress correspondence requires review'
 rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True))
 assert not rows,('set wrapper acquired an unsafe/metadata boundary',rows)
 return hashes
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--kani',action='store_true');p.add_argument('--mutations',action='store_true');a=p.parse_args()
 out=ROOT/'target/verification/hash-set/proofs';out.mkdir(parents=True,exist_ok=True);(out/'summary.json').unlink(missing_ok=True)
 paths=[*(ROOT/f for f in FILES),ROOT/'src/collections/hash_set/proofs.rs',ROOT/'src/collections/hash_map.rs',ROOT/'src/collections/hash_map/iter.rs',ROOT/'src/collections/hash_map/raw.rs',ROOT/'src/collections/hash_map/proofs.rs',ROOT/'tests/hash_set.rs',Path(__file__)]
 fingerprint=lambda:hashlib.sha256(b''.join(str(f.relative_to(ROOT)).encode()+b'\0'+f.read_bytes() for f in paths)).hexdigest()
 before=fingerprint()
 def run(name,cmd,env=None):
  with (out/(name+'.log')).open('w') as log:r=subprocess.run(cmd,cwd=ROOT,env=env,stdout=log,stderr=log)
  return r,(out/(name+'.log')).read_text()
 r,log=run('ast-build',['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bin','unsafe_sites','--target-dir','target/panic-inventory']);assert r.returncode==0,log[-3000:]
 phases=bindings();controls=[]
 if a.kani:
  r,log=run('kani',['cargo','kani','--lib','--harness','collections::hash_set::proofs::','--harness',HARNESSES[0],'-Z','unstable-options','--cbmc-args','--object-bits','12'])
  assert r.returncode==0 and 'Complete - 3 successfully verified harnesses, 0 failures, 3 total.' in log,log[-5000:]
 if a.mutations:
  cases=[('swallow-insert-refusal','src/collections/hash_set.rs','.map_err(rejected)','.or(Ok(true))',None),
         ('disjoint-negated','src/collections/hash_set/algebra.rs','self.iter().all(|key| !other.contains(key))','self.iter().all(|key| other.contains(key))',None)]
  if a.kani:cases += [
   ('replace-wrong-slot','src/collections/hash_map/raw.rs','mem::replace(&mut *ptr::addr_of_mut!((*self.slot(index)).0), key)','mem::replace(&mut *ptr::addr_of_mut!((*self.slot(index.wrapping_add(1) & self.mask)).0), key)',HARNESSES[0]),
   ('hint-overflow','src/collections/hash_set/algebra.rs','a.checked_add(b)','a.checked_sub(b)',HARNESSES[1]),
   ('filter-membership','src/collections/hash_set/algebra.rs','iter.find(|item| keep(item))','iter.find(|item| !keep(item))',HARNESSES[2]),
  ]
  for name,file,old,new,harness in cases:
   with tempfile.TemporaryDirectory(prefix='jzap-hash-set-') as tmp:
    work=Path(tmp);shutil.copytree(ROOT/'src',work/'src');shutil.copytree(ROOT/'examples',work/'examples')
    for f in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/f,work/f)
    path=work/file;text=path.read_text();assert text.count(old)==1,(name,text.count(old));path.write_text(text.replace(old,new))
    env=dict(os.environ,CARGO_TARGET_DIR=str(work/'target'))
    if harness:
     cmd=['cargo','kani','--manifest-path',str(work/'Cargo.toml'),'--lib','--harness',harness,'--exact','-Z','unstable-options','--cbmc-args','--object-bits','12']
     r,log=run(name,cmd,env);assert r.returncode and 'VERIFICATION:- FAILED' in log and 'error[E' not in log,log[-4000:]
     detected='actual-Rust-Kani-counterexample'
    else:
     (work/'tests').mkdir();shutil.copyfile(ROOT/'tests/hash_set.rs',work/'tests/hash_set.rs');shutil.copytree(ROOT/'tests/common',work/'tests/common')
     test='disjoint_uses_smaller_input_stops_at_first_match_and_never_allocates' if name=='disjoint-negated' else 'every_growth_refusal_returns_input_and_retains_contents_budget_and_capacity'
     cmd=['cargo','test','--manifest-path',str(work/'Cargo.toml'),'--locked','--offline','--all-features','--test','hash_set',test,'--','--exact']
     r,log=run(name,cmd,env);assert r.returncode and 'test result: FAILED' in log and 'error[E' not in log,log[-4000:]
     detected='actual-disjoint-predicate-runtime-regression' if name=='disjoint-negated' else 'actual-allocation-refusal-runtime-regression'
    controls.append(dict(name=name,compiled=True,detected_by=detected,separate_target=True))
 assert before==fingerprint(),'HashSet verification inputs changed'
 (out/'summary.json').write_text(json.dumps(dict(status='pass',complete=a.kani and a.mutations,source_unchanged=True,source_sha256=before,phases=phases,kani_harnesses=HARNESSES if a.kani else [],controls=controls,scope=__doc__),indent=2)+'\n')
 print('HashSet actual Rust and delegation checks passed; complete=',a.kani and a.mutations)
if __name__=='__main__':main()
