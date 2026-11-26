#!/usr/bin/env python3
"""Bind shared ownership, budget and core-guard models to current Rust sources."""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
from shared_bindings import check,compact,is_shared
from arithmetic_projection import replace_once
from verus_composition import run as compose
import storage_kernel
from shared_bindings import CALLERS
from verify_edit_loops import block
from callsite_projection import PINNED_BODIES,STATE_CALLERS
from verify_source import verify as direct
from premise_status import verified_function_names
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'target/verification/shared/proofs'
VERUS=Path(os.environ.get('JZAP_VERUS','/workspace/.devtools/verus/verus-x86-linux/verus'))

def core_bindings():
    version=subprocess.check_output(['rustc','--version'],text=True).strip()
    assert version.startswith('rustc 1.95.0 '),version
    sysroot=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())
    core=sysroot/'lib/rustlib/src/rust/library/core/src/sync/atomic.rs'
    text=core.read_text()
    for expression in ['Release => panic!("there is no such thing as a release load")',
       'AcqRel => panic!("there is no such thing as an acquire-release load")',
       'Acquire => panic!("there is no such thing as an acquire store")',
       '(_, Release) => panic!("there is no such thing as a release failure ordering")',
       'Relaxed => panic!("there is no such thing as a relaxed fence")']:
        assert expression in text,'core atomic guard changed'
    calls=[]
    for file in ['src/shared/count.rs','src/budget/concurrent.rs']:
        source=(ROOT/file).read_text()
        # The complete function phase binding additionally rejects dynamic or
        # shadowed order expressions, new primitives and altered retry routing.
        for match in re.finditer(r'Ordering::(Relaxed|Acquire|Release|AcqRel|SeqCst)',source):
            calls.append(dict(file=file,ordering=match[1]))
    assert calls and {r['ordering']for r in calls}<={'Relaxed','Acquire','Release'}
    return dict(rustc=version,path=str(core),sha256=hashlib.sha256(core.read_bytes()).hexdigest(),order_arguments=calls)

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--kani',action='store_true');p.add_argument('--mutations',action='store_true');p.add_argument('--loom',action='store_true');a=p.parse_args()
    OUT.mkdir(parents=True,exist_ok=True);(OUT/'summary.json').unlink(missing_ok=True)
    files=sorted((ROOT/'src').rglob('*.rs'))+[ROOT/'scripts/shared_bindings.py',ROOT/'scripts/premise_status.py',ROOT/'scripts/callsite_projection.py',ROOT/'scripts/verify_source.py',ROOT/'scripts/verify_direct_verus.py',ROOT/'scripts/verus_composition.py',Path(__file__),ROOT/'tools/memory-proof/shared.verus.rs',*storage_kernel.INPUTS,ROOT/'tools/panic-audit/Cargo.toml',ROOT/'tools/panic-audit/Cargo.lock']+sorted((ROOT/'tools/panic-audit/src').rglob('*.rs'))+sorted(f for f in (ROOT/'tools/shared-loom').rglob('*') if f.is_file() and 'target' not in f.relative_to(ROOT).parts)
    digest=lambda:hashlib.sha256(b''.join(str(f.relative_to(ROOT)).encode()+b'\0'+f.read_bytes()for f in files)).hexdigest()
    before=digest();results=[]
    direct_numeric=direct(['shared-numeric'],OUT/'direct-numeric',a.mutations)
    def run(name,command,expected=False,extra=None):
        r=subprocess.run(command,cwd=ROOT,env=dict(os.environ,**(extra or {})),text=True,capture_output=True)
        log=r.stdout+r.stderr;(OUT/(name+'.log')).write_text(log)
        row=dict(name=name,command=list(map(str,command)),exit_code=r.returncode);results.append(row)
        if not expected:assert r.returncode==0,log[-5000:]
        return row,log
    run('parsers',['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml','--bin','state_sites','--bin','unsafe_sites','--target-dir','target/panic-inventory'])
    bindings=check();core=core_bindings()
    # The storage kernel is placed in front of the model in every composition.
    template=storage_kernel.kernel()+(ROOT/'tools/memory-proof/shared.verus.rs').read_text()
    squeeze=lambda text:re.sub(r'\s+','',re.sub(r'//[^\n]*','',text))
    for name,calls in CALLERS.items():
        start=template.index('\nfn '+name+'(');body=squeeze(template[start:template.index('\n}\n',start)+3])
        for call in calls:assert call in body,('maintained shared caller lost a product call',name,call)
    def model(name,source,negative=False):
        path=OUT/(name+'.calls.rs');path.write_text(source)
        command,r=compose(VERUS,ROOT,path,'jzap_shared_proof',dict(os.environ,PATH=str(VERUS.parent)+os.pathsep+os.environ['PATH']))
        log=r.stdout+r.stderr;(OUT/(name+'.log')).write_text(log)
        verdict=re.findall(r'verification results:: (\d+) verified, (\d+) errors',log)
        assert verdict and 'error[E' not in log and (r.returncode!=0 and any(int(e)>0 for _,e in verdict) if negative else r.returncode==0 and all(int(e)==0 for _,e in verdict)),log[-4000:]
        results.append(dict(name=name,command=command,exit_code=r.returncode,verification=verdict,detected=negative))
    # The admission caller must call the product kernel and not restate it.
    # `shared_clone_weak_caller` uses `checked_add` as its product body does.
    admission=template[template.index('\nfn concurrent_admit('):];admission=admission[:admission.index('\n}\n')]
    assert 'crate::budget::admission::admit(limit,used,bytes)' in admission and 'checked_add' not in admission,'shared admission must call the product kernel'
    assert 'crate::init::driver::run(&mut state,length,' in template,'shared slice prefix must run the product driver'
    # The driver chain between the shared callers and `ptr.add(index)`.
    prefix_source=(ROOT/'src/init/prefix.rs').read_text();strip=lambda text:re.sub(r'\s+','',re.sub(r'//[^\n]*','',text))
    assert '{'+strip(block(prefix_source,'pub(crate) unsafe fn initialize<'))+'}'==PINNED_BODIES[('src/init/prefix.rs','initialize','')],'prefix initialize body changed'
    assert '{'+strip(block(prefix_source,'unsafe fn initialize_one<'))+'}'==STATE_CALLERS[('src/init/prefix.rs','initialize_one','')][1],'prefix initialize_one body changed'
    model('actual-numeric-calls-and-phase-models',template)
    if a.kani:
        for name,pattern,count in [('kani','shared::proofs::',20),('kani-budget','budget::concurrent::proofs::',2)]:
            _,log=run(name,['cargo','kani','--lib','--harness',pattern])
            assert f'Complete - {count} successfully verified harnesses, 0 failures, {count} total.'in log
        reduced=['reduced_model_guard_matches_every_bounded_raw_reservation',
            'buffered_increment_rolls_back_exactly_its_own_credit',
            'flagged_weak_clone_release_preserves_layout_and_lock_separation',
            'atomic_publication_and_retirement_distinguish_all_valid_layouts',
            'temporary_high_count_reports_refusal_without_resurrection_or_death',
            'retirement_preserves_every_valid_weak_credit']
        command=['cargo','kani','--lib']
        for name in reduced:command+=['--harness','shared::proofs::'+name]
        _,log=run('kani-reduced-model',command,extra={'RUSTFLAGS':'--cfg jzap_small_counts'})
        assert f'Complete - {len(reduced)} successfully verified harnesses, 0 failures, {len(reduced)} total.' in log
    if a.mutations:
        model('mutation-prefix-overrun',replace_once(template,'requires index<length,raw_valid(p)','requires index<=length,raw_valid(p)'),True)
        # Weakened maintained callers; Verus must report an error for each.
        for name,old,new in [
          ('increment-stores-old-word','        Ok(next) => Some(next),\n        Err(_) => None,\n    }\n}\n// LocalCount::release','        Ok(next) => Some(old),\n        Err(_) => None,\n    }\n}\n// LocalCount::release'),
          ('release-without-credit','    requires old>=1,\n    ensures out.0 as int==old as int-1','    ensures out.0 as int==old as int-1'),
          ('buffered-guard-on-any-word','    requires old as int+1<2*(crate::shared::count::MAX_COUNT as int+1),\n',''),
          ('weak-clone-without-limit','    if old & !crate::shared::count::RETIRED>=limit {\n        return None;\n    }\n',''),
          ('data-beyond-allocation','    requires raw_valid(a),a.size==1,offset as int+payload as int<=a.cap,\n    ensures doc_ptr_add','    requires raw_valid(a),a.size==1,\n    ensures doc_ptr_add'),
          ('bytes-copy-source-inside-allocation','source.cap==bytes,same_element(source,a),apart(source,a),','source.cap==bytes,same_element(source,a),'),
          ('slice-copy-shifted','    distinct_nonoverlap_caller(source,p,0,0,len);','    distinct_nonoverlap_caller(source,p,0,1,len);'),
          ('value-on-any-storage','    requires raw_valid(p),p.cap==1,\n','    requires raw_valid(p),\n'),
          ('prefix-source-shorter','requires raw_valid(p),p.cap==length,raw_valid(source),source.cap==length,\n{\n    let mut state','requires raw_valid(p),p.cap==length,raw_valid(source),\n{\n    let mut state'),
        ]:model('mutation-caller-'+name,replace_once(template,old,new),True)
        model('mutation-admission-arguments',replace_once(template,'crate::budget::admission::admit(limit,used,bytes)','crate::budget::admission::admit(used,limit,bytes)'),True)
        model('mutation-unbounded-debt',replace_once(template,'0<=pending<maximum','0<=pending'),True)
        changes=[
          ('owning-ledger-identity','src/budget/owned.rs',
           'pub fn ledger(&self) -> &ConcurrentMemoryBudget {\n        self.inner.as_ref()\n    }',
           'pub fn ledger(&self) -> &ConcurrentMemoryBudget { static OTHER: ConcurrentMemoryBudget = ConcurrentMemoryBudget::new(usize::MAX); &OTHER }'),
          ('owning-copy-domain','src/budget/owned.rs',
           'crate::sync::Arc::try_clone(&self.inner)?',
           'crate::sync::Arc::try_new(ConcurrentMemoryBudget::new(self.limit())).map_err(|e|e.error)?'),
          ('header-repr','src/shared.rs','#[repr(C)]\npub struct Control','pub struct Control'),
          ('counter-sealing','src/shared/count.rs','mod sealed {','pub mod sealed {'),
          ('pinned-alias','src/shared/pinned.rs','    owner: Strong<T, C, P>,','    pub owner: Strong<T, C, P>,'),
          ('reservation-clone','src/budget/concurrent.rs',"pub struct ConcurrentReservation<'a>","#[derive(Clone)]\npub struct ConcurrentReservation<'a>"),
          ('layout-pointer','src/shared.rs','allocation.data().cast::<T>().write(value);','allocation.data().add(1).cast::<T>().write(value);'),
          ('implicit-credit','src/shared.rs','weak: C::new(C::initial_weak(retired)),','weak: C::new(2),'),
          ('trait-default-credit','src/shared/count.rs','fn clone_strong(&self) -> Result<(), Error>;','fn clone_strong(&self) -> Result<(), Error> { Ok(()) }'),
          ('buffer-native-without-bound','src/shared/count.rs','pub const BUFFERED_STRONG: bool = cfg!(any(\n    jzap_small_counts,\n    all(\n        target_os = \"linux\",\n        target_arch = \"x86_64\",\n        target_pointer_width = \"64\"\n    )\n));','pub const BUFFERED_STRONG: bool = true;'),
          ('slice-clone-count','src/shared/value.rs','crate::init::prefix::initialize(ptr, self.len(), &mut initialize)','crate::init::prefix::initialize(ptr, self.len().wrapping_add(1), &mut initialize)'),
          ('slice-init-pointer','src/shared/value.rs','NonNull::new_unchecked(data(owner.ptr).cast::<T>())','NonNull::new_unchecked(data(owner.ptr).cast::<T>().add(1))'),
          ('budget-compare','src/budget/concurrent.rs','.compare_exchange(used, next, Ordering::Relaxed, Ordering::Relaxed)','.compare_exchange(next, next, Ordering::Relaxed, Ordering::Relaxed)'),
        ]
        for name,file,old,new in changes:
            with tempfile.TemporaryDirectory(prefix='shared-source-',dir=OUT)as temporary:
                root=Path(temporary);shutil.copytree(ROOT/'src',root/'src');path=root/file;path.write_text(replace_once(path.read_text(),old,new))
                run('mutation-compile-'+name,['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(root/'libjzap.rlib')])
                try:check(root)
                except AssertionError as error:results.append(dict(name='mutation-'+name,detected=True,mechanism='compiled Rust rejected by phase/argument binding',diagnostic=str(error)[:500]))
                else:raise AssertionError('accepted compiled mutation '+name)
    if a.loom:
        loom_env={'RUSTFLAGS':'--cfg jzap_loom --cfg jzap_small_counts'}
        command=['cargo','test','--locked','--offline','--manifest-path',str(ROOT/'tools/shared-loom/Cargo.toml'),'--test','concurrent']
        _,log=run('loom',command,extra=loom_env)
        assert 'test result: ok. 12 passed;'in log
        _,log=run('loom-headroom',command[:-1]+['headroom'],extra=loom_env)
        assert 'test result: ok. 6 passed;' in log
        if a.mutations:
            for name,file,old,new,test in [
              ('missing-release','src/shared/count.rs','if self.0.fetch_sub(1, Ordering::Release) != 1','if self.0.fetch_sub(1, Ordering::Relaxed) != 1','last_release_acquires_previous_owner_writes'),
              ('missing-refund','src/budget/concurrent.rs','self.used.fetch_sub(bytes, Ordering::Relaxed)','self.used.fetch_sub(0, Ordering::Relaxed)','budget_reservation_commit_cancellation')]:
                with tempfile.TemporaryDirectory(prefix='shared-loom-',dir=OUT)as temporary:
                    root=Path(temporary);shutil.copytree(ROOT/'src',root/'src')
                    shutil.copytree(ROOT/'tools/shared-loom',root/'tools/shared-loom',ignore=shutil.ignore_patterns('target'))
                    path=root/file;path.write_text(replace_once(path.read_text(),old,new))
                    metadata=json.loads(subprocess.check_output(['cargo','metadata','--offline','--format-version=1','--no-deps','--manifest-path',str(root/'tools/shared-loom/Cargo.toml')],cwd=ROOT,text=True))
                    assert Path(metadata['packages'][0]['targets'][0]['src_path']).resolve()==(root/'src/lib.rs').resolve(), 'Loom mutant resolved the wrong source'
                    row,log=run('loom-mutation-'+name,['cargo','test','--locked','--offline','--manifest-path',str(root/'tools/shared-loom/Cargo.toml'),'--target-dir',str(root/'target'),'--test','concurrent',test,'--','--exact'],True,loom_env)
                    assert row['exit_code'] and 'test result: FAILED'in log and 'error[E'not in log and 'Model exceeded'not in log and ('Causality violation'in log or 'assertion'in log),log[-4000:]
                    row['detected']=True
            for name,old,new,test in [
                ('headroom-missing-rollback','self.0.fetch_sub(1, Ordering::Release);\n            Err(Error::RefCount)','self.0.fetch_sub(0, Ordering::Release);\n            Err(Error::RefCount)','simultaneous_rejected_clones_preserve_every_existing_owner'),
                ('headroom-wrong-guard','wrapping_add(old, 1) & guard != 0','wrapping_add(old, 1) & guard == 0','saturated_live_arc_upgrade_must_not_report_dead'),
                ('headroom-retired-alias','if word <= MAX_COUNT {','if word > MAX_COUNT { return None; } if word <= MAX_COUNT {','saturated_live_arc_upgrade_must_not_report_dead'),
                ('headroom-sole-weak-blind-store','if weak.0.load(Ordering::Acquire) == 1 {','if weak.0.load(Ordering::Acquire) >= 1 {','weak_created_by_another_strong_owner_survives_final_retirement'),
                ('headroom-missing-final-acquire','if self.0.fetch_sub(1, Ordering::Release) != 1 {\n            return false;\n        }\n        fence(Ordering::Acquire);','if self.0.fetch_sub(1, Ordering::Release) != 1 {\n            return false;\n        }','weak_created_by_another_strong_owner_survives_final_retirement'),
                ('headroom-missing-retired-flag','weak.0.fetch_or(RETIRED, Ordering::Release);','weak.0.fetch_or(0, Ordering::Release);','retired_flag_and_weak_clone_race_refund_the_same_domain_once'),
                ('headroom-unordered-cyclic-clear','weak.0.fetch_and(!RETIRED, Ordering::Release);','weak.0.fetch_and(!RETIRED, Ordering::Relaxed);','metered_cyclic_initialization_publishes_flag_and_payload_in_order'),
            ]:
                with tempfile.TemporaryDirectory(prefix='headroom-loom-',dir=OUT) as temporary:
                    root=Path(temporary);shutil.copytree(ROOT/'src',root/'src')
                    shutil.copytree(ROOT/'tools/shared-loom',root/'tools/shared-loom',ignore=shutil.ignore_patterns('target'))
                    path=root/'src/shared/count.rs';path.write_text(replace_once(path.read_text(),old,new))
                    metadata=json.loads(subprocess.check_output(['cargo','metadata','--offline','--format-version=1','--no-deps','--manifest-path',str(root/'tools/shared-loom/Cargo.toml')],cwd=ROOT,text=True))
                    assert Path(metadata['packages'][0]['targets'][0]['src_path']).resolve()==(root/'src/lib.rs').resolve(), 'Headroom mutant resolved the wrong source'
                    # A disposable package must get its own target directory.
                    # Reusing the product target returned a cached unmutated
                    # test binary in the negative-control preflight.
                    row,log=run('loom-mutation-'+name,['cargo','test','--locked','--offline','--manifest-path',str(root/'tools/shared-loom/Cargo.toml'),'--target-dir',str(root/'target'),'--test','headroom',test,'--','--exact'],True,loom_env)
                    # Custom assert! messages do not contain "assertion".
                    # Require a panic at the test oracle, never a compiler,
                    # model-limit, or internal library diagnostic.
                    semantic_failure=re.search(r'panicked at [^\n]*tests/headroom\.rs:\d+:\d+',log) is not None
                    assert row['exit_code'] and 'test result: FAILED' in log and 'error[E' not in log and 'Model exceeded' not in log and semantic_failure,log[-4000:]
                    row['detected']=True
    assert digest()==before,'shared source changed during verification'
    report=dict(status='pass',source_sha256=before,source_unchanged=True,bindings=bindings,core=core,results=results,kani_run=a.kani,loom_run=a.loom,
        direct_numeric=direct_numeric,word_domain='Verus Either32Or64; no word-type body rewriting',
        verified_functions=sorted(verified_function_names(template)),
        kani_harnesses=['shared::proofs::'+n for n in re.findall(r'#\[kani::proof\](?:\s*#\[[^\n]+\])*\s*fn (\w+)\(', (ROOT/'src/shared/proofs.rs').read_text())]+['budget::concurrent::proofs::concurrent_budget_admission_all_machine_words','budget::concurrent::proofs::actual_concurrent_reservation_commit_and_cancel'],
        platform_refinement=dict(native_fast_target='Linux x86-64, 64-bit pointers, four/five-level paging native ABI',
            assumptions=['at most positive-i32 native task IDs','distinct suspended native activations occupy nonempty valid storage below the 2^56-byte user address ceiling'],
            pending_bound='less than 2^56 + 2^31, hence less than isize::MAX',
            other_targets='CAS',reduced_model='at most 3 running native-model threads, guard 8'),
        scope='Actual count/layout/budget Rust Kani and numeric/ghost-credit kernels; complete source-phase/argument binding. Native pending-count refinement trusts documented Linux task/address-space and valid native ABI constraints; it is not a proof of Linux or arbitrary foreign context-switching assembly. Pointer provenance/alias/drop use compiler, Miri, sanitizers and actual-implementation Loom; public unsafe and returning callbacks remain explicit boundaries.')
    (OUT/'summary.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'{len(bindings)} shared source bindings, direct numeric kernels, retained phase/core models and controls pass; Kani={a.kani}')
if __name__=='__main__':main()
