#!/usr/bin/env python3
"""Bind the safe set layer and check actual Rust hint arithmetic and controls.

Seals are correspondence checks, not semantic proofs. Map node/slot contracts,
actual Rust replacement checks and final links are composed separately by the
whole-library certificate. Typed memory and callback paths use the memory suite.
"""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
from scanner_tools import scanner
from btree_bindings import SET_FILES as FILES
ROOT=Path(__file__).resolve().parents[1]
REVIEWED={'src/collections/btree_set.rs': 'c00cf5ffe76309390df72f1ed3d71c5ea6c5b1f1ceb51495460fda89e003d7fb', 'src/collections/btree_set/algebra.rs': '916e081f758d5b418ede381f37ecac70c682c543046c285d43e6bc65ec43a89a', 'src/collections/btree_set/iter.rs': 'a5e16d62c24a3c7e9f0ccb414e6d774d79f0899d88d5e7b263596a783ec4bda2'} # Reviewed private ownership, map delegation and finite iterator progress.
def bindings(root=ROOT):
    root=Path(root)
    actual={'src/collections/btree_set.rs',*(str(p.relative_to(root)) for p in (root/'src/collections/btree_set').glob('*.rs') if not p.name.endswith('proofs.rs'))}
    assert actual==set(FILES),'BTreeSet verification domain changed'
    hashes={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in FILES}
    assert hashes==REVIEWED,'BTreeSet source/contract correspondence requires review'
    rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True))
    assert not rows,('set wrapper unexpectedly acquired an unsafe/metadata boundary',rows)
    return hashes
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--mutations',action='store_true');p.add_argument('--kani',action='store_true');args=p.parse_args()
    out=ROOT/'target/verification/btree-set/proofs';out.mkdir(parents=True,exist_ok=True);(out/'summary.json').unlink(missing_ok=True)
    def fingerprint():
        paths=[*sorted((ROOT/'src').rglob('*.rs')),Path(__file__),ROOT/'scripts/btree_bindings.py',ROOT/'tools/memory-proof/btree.verus.rs']
        return hashlib.sha256(b''.join(str(f.relative_to(ROOT)).encode()+b'\0'+f.read_bytes() for f in paths)).hexdigest()
    before=fingerprint();phases=bindings();controls=[];harness='collections::btree_set::proofs::algebra_hint_all_machine_word_lengths'
    merge_harness='collections::btree_set::algebra::merge_proofs::sorted_prefixes_preserve_values_progress_and_hints'
    def run(name,command,cwd=ROOT,env=None):
        with (out/(name+'.log')).open('w') as log:r=subprocess.run(command,cwd=cwd,env=env,stdout=log,stderr=log)
        return r,(out/(name+'.log')).read_text()
    if args.kani:
        result,log=run('set-kani',['cargo','kani','--lib','--harness',harness,'--harness',merge_harness])
        assert result.returncode==0 and 'Complete - 2 successfully verified harnesses, 0 failures, 2 total.' in log,log[-4000:]
    if args.mutations:
        for name,file,old,new in [
            ('replace-wrong-slot','src/collections/btree_map.rs','node::replace_key(c.node, c.index(), key)','node::replace_key(c.node, 11, key)'),
            ('swallow-insert-refusal','src/collections/btree_set.rs','.map_err(rejected)','.or(Ok(true))'),
        ]:
            with tempfile.TemporaryDirectory(prefix='jzap-set-binding-') as tmp:
                root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');path=root/file;s=path.read_text();assert old in s,(name,old);path.write_text(s.replace(old,new,1))
                result,log=run(name+'-compile',['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(root/'probe.rlib')]);assert result.returncode==0,log[-4000:]
                try:
                    if name=='replace-wrong-slot':
                        from btree_bindings import kernel_envelopes
                        kernel_envelopes(root)
                    else:bindings(root)
                except AssertionError:pass
                else:raise AssertionError('compiled defect escaped source boundary: '+name)
                controls.append(dict(name=name,compiled=True,detected_by='reviewed-source-contract-boundary'))
        if args.kani:
            for name,old,new,target in [
                ('hint-overflow','a.checked_add(b)','a.checked_sub(b)',harness),
                ('merge-pending-identity','self.pending = Some((true, left));','self.pending = Some((false, right));',merge_harness),
            ]:
                with tempfile.TemporaryDirectory(prefix='jzap-set-mutant-') as tmp:
                    root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');shutil.copytree(ROOT/'examples',root/'examples')
                    for file in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/file,root/file)
                    path=root/'src/collections/btree_set/algebra.rs';s=path.read_text();assert s.count(old)==1;path.write_text(s.replace(old,new))
                    env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'))
                    result,log=run(name+'-mutant',['cargo','kani','--manifest-path',str(root/'Cargo.toml'),'--lib','--exact','--harness',target],env=env)
                    assert result.returncode and 'VERIFICATION:- FAILED' in log and 'error[E' not in log,log[-4000:]
                    controls.append(dict(name=name,compiled=True,detected_by='actual-Rust-Kani-counterexample',separate_target=True))
    assert fingerprint()==before,'set proof input changed during verification'
    report=dict(status='pass',complete=args.kani and args.mutations,source_sha256=before,source_unchanged=True,phases=phases,kani_harnesses=[harness,merge_harness] if args.kani else [],controls=controls,
        scope='Safe wrapper delegation/progress correspondence, all-word hint arithmetic, and actual generic merge over sorted prefixes of two arbitrary u8 triples. Replacement uses the map certificate; pointer provenance/aliasing/drop use the separately executed memory suite. Source seals are not direct Rust semantic proofs.')
    (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print('BTreeSet bindings and arithmetic checks passed; complete=',report['complete'])
if __name__=='__main__':main()
