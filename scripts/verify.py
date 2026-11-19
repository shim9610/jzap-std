#!/usr/bin/env python3
"""Reproduce selected checks. A partial pass is not whole-library certification."""
import argparse,hashlib,json,os,py_compile,re,shlex,signal,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
NIGHTLY='nightly-2026-10-30'
BTREE_SEEDS='''from pathlib import Path
import sys
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
for count in (12,144,1200):
    data=bytearray()
    for i in range(count):data.extend((255 if i==0 else i%2,i%256,i//256,i%251))
    for op in range(2,9):data.extend((op,53,0,7))
    (out/str(count)).write_bytes(data)
'''
SBOM='''from pathlib import Path
import json,subprocess,sys
source=Path('jzap-sbom.json')
if source.exists():raise FileExistsError('Move the existing jzap-sbom.json before generating a new report')
try:
    subprocess.run(['cargo','cyclonedx','--format','json','--all','--override-filename','jzap-sbom'],check=True)
    data=json.loads(source.read_text())
    if data.get('bomFormat')!='CycloneDX':raise ValueError('No valid CycloneDX result')
    source.replace(sys.argv[1])
finally:source.unlink(missing_ok=True)
'''
AREAS={
    'all':[], 'foundation':['foundation','properties','wrapping_memory','unsafe_paths','iteration'],
    'arithmetic':['arithmetic'], 'box':['box_complete','box_properties'],
    'vec':['vec','vec_properties','iteration'], 'string':['string'],
    'shared':['shared','arc_headroom','owned_budget'],
    'budget':['owned_budget','reallocation_budget'], 'btree':['btree'], 'btree-set':['btree_set'], 'hash-map':['hash_map'], 'hash-set':['hash_set'], 'binary-heap':['binary_heap'], 'deque':['deque'], 'scratch-slice':['scratch_slice'],
}
API={'box':'check_box_compatibility.py','vec':'check_vec_api.py','string':'check_string_api.py',
    'shared':'check_shared_api.py','btree':'check_btree_api.py','btree-set':'check_btree_set_api.py','hash-map':'check_hash_map_api.py','hash-set':'check_hash_set_api.py','binary-heap':'check_binary_heap_api.py','deque':'check_deque_api.py'}
CHECKS={
    'runner':'Verification runner failure/selection tests',
    'format':'Rust formatting', 'api':'Pinned API declarations (not semantic proofs)',
    'tests':'Native tests', 'release':'Release tests', 'overflow':'Overflow-checked release tests',
    'clippy':'Whole-library lint checks', 'docs':'Rustdoc with warnings denied',
    'builds':'Core-only and target compilation', 'miri':'Strict-provenance memory scenarios',
    'miri-32':'i686 memory scenarios', 'miri-tree':'Tree Borrows memory scenarios',
    'asan':'AddressSanitizer scenarios', 'coverage':'LLVM coverage', 'fuzz':'10,000 fuzz runs',
    'verus':'Direct product-source Verus kernels for the selected area',
    'direct-source':'Migrated product-source contracts and actual-source negative controls',
    'models':'Projected Verus models; the translation boundary remains',
    'kani':'Actual Rust model-checking harnesses', 'loom':'Actual atomic schedule models',
    'small-counts':'Reduced-limit shared ownership runtime tests',
    'bounds':'Compiler ownership/thread boundary checks',
    'inventory':'Independent AST site listing and counts (not proof obligations)',
    'unsafe':'Unsafe census and routing',
    'audit':'Locked dependency security/license checks', 'sbom':'CycloneDX software bill of materials',
    'link':'Concrete whole-library call graph; retained guards may need the certificate',
    'certificate':'Fresh global source/model/link composition and negative controls',
}
QUICK=['runner','format','tests','clippy']
FULL=['runner','format','api','tests','release','overflow','clippy','docs','builds',
    'miri','miri-32','miri-tree','asan','coverage','fuzz','small-counts','bounds','unsafe','audit','sbom','certificate']
def plan(area,checks,out,test_targets=None):
    selected=AREAS[area]
    if test_targets is not None:
        allowed=set(AREAS[area]) if area!='all' else {name for names in AREAS.values() for name in names}
        if not test_targets or len(test_targets)!=len(set(test_targets)) or not set(test_targets)<=allowed:
            raise ValueError('Choose distinct test targets from the selected area')
        if 'certificate' in checks:
            raise ValueError('A global certificate cannot use selected runtime test targets')
        if not set(checks)&{'tests','release','overflow','miri','miri-32','miri-tree','asan','coverage'}:
            raise ValueError('--test-target requires a runtime test or coverage check')
        selected=test_targets
    jobs=[];test_args=["--lib"] if area in ("hash-map","hash-set") else []
    for test in selected:test_args+=['--test',test]
    def add(name,cmd,kind='runtime-check',env=None):
        jobs.append(dict(name=name,command=cmd,kind=kind,environment=env or {}))
    def script(name,file,*args,kind='source-correspondence'):
        add(name,[sys.executable,'scripts/'+file,*args],kind)
    base=['cargo','test','--locked','--offline','--all-features',*test_args]
    for check in checks:
        if check=='runner':script(check,'test_verify.py',kind='verification-runner-tests')
        elif check=='format':add(check,['cargo','fmt','--all','--check'],'formatting')
        elif check=='api':
            names=list(API) if area=='all' else [area]
            if any(a not in API for a in names):raise ValueError('No pinned API inventory for '+area+'; choose tests or verus.')
            for a in names:script('api-'+a,API[a],kind='declaration-inventory')
        elif check in ('tests','release','overflow'):
            add(check,base+(['--release'] if check!='tests' else []),env={'RUSTFLAGS':'-C overflow-checks=yes'} if check=='overflow' else {})
        elif check=='clippy':
            for name,feature in [('all','--all-features'),('core','--no-default-features')]:
                add('clippy-'+name,['cargo','clippy','--locked','--offline','--lib',feature,'--','-D','warnings'],'static-lint')
        elif check=='docs':add(check,['cargo','doc','--locked','--offline','--all-features','--no-deps'],'documentation',{'RUSTDOCFLAGS':'-D warnings'})
        elif check=='builds':
            add('build-core',['cargo','check','--locked','--offline','--no-default-features'],'compile-check')
            for target in ['wasm32-unknown-unknown','thumbv7em-none-eabi']:
                add('build-'+target,['cargo','check','--locked','--offline','--target',target],'compile-check')
            add('build-no-global-oom',['cargo','+'+NIGHTLY,'check','--locked','--offline','--all-features'],'compile-check',{'RUSTFLAGS':'--cfg no_global_oom_handling'})
        elif check.startswith('miri'):
            cmd=['cargo','+'+NIGHTLY,'miri','test','--locked','--offline','--all-features',*test_args]
            if check=='miri-32':cmd+=['--target','i686-unknown-linux-gnu']
            # Isolation keeps host variables from the interpreted program;
            # forward only the proptest settings so they take effect. Under
            # Miri, proptest uses XorShift: interpreting the ChaCha SSE2
            # generator exhausts the 32-bit (i686) address space after two
            # 2048-case properties in one process. Case counts are unchanged.
            forward=''.join(' -Zmiri-env-forward='+name for name in ('PROPTEST_DISABLE_FAILURE_PERSISTENCE','PROPTEST_RNG_SEED','PROPTEST_RNG_ALGORITHM'))
            add(check,cmd,'executed-memory-check',{'MIRIFLAGS':'-Zmiri-strict-provenance'+(' -Zmiri-tree-borrows' if check=='miri-tree' else '')+forward,'PROPTEST_DISABLE_FAILURE_PERSISTENCE':'1','PROPTEST_RNG_SEED':'1','PROPTEST_RNG_ALGORITHM':'xs'})
        elif check=='asan':add(check,['cargo','+'+NIGHTLY,'test','--locked','--offline','--all-features','--target','x86_64-unknown-linux-gnu',*test_args],
            'executed-memory-check',{'RUSTFLAGS':'-Zsanitizer=address','RUSTDOCFLAGS':'-Zsanitizer=address'})
        elif check=='coverage':add(check,['cargo','+'+NIGHTLY,'llvm-cov','--branch','--all-features',*test_args,'--json','--output-path',str(out/'coverage.json')],'coverage')
        elif check=='fuzz':
            mapping={'foundation':'foundation','vec':'vec','string':'string','shared':'shared','btree':'btree','btree-set':'btree_set','hash-map':'hash_map','hash-set':'hash_set','binary-heap':'binary_heap','deque':'deque','scratch-slice':'scratch_slice'}
            if area!='all' and area not in mapping:raise ValueError('No dedicated fuzz target for '+area)
            for target in (list(mapping.values()) if area=='all' else [mapping[area]]):
                corpus=[]
                if target=='btree':
                    corpus=[str(out/'btree-corpus')]
                    add('fuzz-btree-seeds',[sys.executable,'-c',BTREE_SEEDS,*corpus],'fuzz-seed-preparation')
                add('fuzz-'+target,['cargo','+'+NIGHTLY,'fuzz','run',target,*corpus,'--','-runs=10000',
                    '-max_len='+('16384' if target=='btree' else '4096')],'fuzzing')
        elif check=='direct-source':
            groups={'btree':['btree-search-cursors','btree-plan-counts','btree-frontiers','btree-delete-decisions','btree-packed-shape','btree-node-spans'],'btree-set':['btree-search-cursors','btree-plan-counts','btree-frontiers','btree-delete-decisions','btree-packed-shape','btree-node-spans'],'all':['arithmetic','slot','clone','state','budget','vec-geometry','vec-conversion','layout','vec-queries','init-prefix','buffer','owner-metadata','vec-cursors','vec-layout-growth','string-cursors','shared-numeric','btree-search-cursors','btree-plan-counts','btree-frontiers','btree-delete-decisions','btree-packed-shape','btree-node-spans','vec-compaction','vec-compaction-values','vec-extraction','vec-extraction-values','vec-splice-fill','vec-staging'],'foundation':['arithmetic','slot','clone','state','budget','vec-geometry','vec-conversion','layout','vec-queries','init-prefix','buffer','owner-metadata','vec-cursors','vec-layout-growth','vec-compaction','vec-compaction-values','vec-extraction','vec-extraction-values','vec-splice-fill','vec-staging'],'arithmetic':['arithmetic'],'string':['string-cursors'],'shared':['shared-numeric'],'budget':['budget'],'box':['slot','clone','layout','init-prefix','buffer','owner-metadata','vec-cursors','vec-layout-growth'],'vec':['clone','state','vec-geometry','vec-conversion','layout','vec-queries','init-prefix','buffer','owner-metadata','vec-cursors','vec-layout-growth','vec-compaction','vec-compaction-values','vec-extraction','vec-extraction-values','vec-splice-fill','vec-staging']}
            if area not in groups:raise ValueError('No migrated direct-source group registered for '+area+'; use verus or kani for existing domain checks.')
            args=[value for group in groups[area] for value in ('--group',group)]
            script(check,'verify_source.py',*args,'--mutations','--output',str(out/'direct-source'),kind='direct-product-source-proof')
        elif check=='verus':
            if area not in ('all','arithmetic','budget','hash-map','hash-set','binary-heap','deque','scratch-slice','vec','btree','btree-set'):raise ValueError('Direct Verus currently covers arithmetic and budget admission. Collection models remain separate; select models to inspect them.')
            args=[] if area=='all' else ['--target','integer' if area=='arithmetic' else 'btree-search-cursors' if area in ('btree','btree-set') else 'hash-map' if area in ('hash-map','hash-set') else 'binary-heap' if area=='binary-heap' else 'deque' if area=='deque' else 'scratch-slice' if area=='scratch-slice' else 'vec-geometry' if area=='vec' else 'budget-admit']
            if area in ('btree','btree-set'):args += ['--target','btree-plan-counts','--target','btree-frontiers','--target','btree-delete-decisions','--target','btree-packed-shape','--target','btree-node-spans']
            if area=='vec':args += ['--target','vec-compaction','--target','vec-extraction']
            if area=='budget':args += ['--target','local-budget']
            script(check,'verify_direct_verus.py',*args,'--mutations','--output',str(out/'direct-verus'),kind='direct-product-source-proof')
        elif check=='models':
            if area in ('budget','vec'):
                script(area+'-direct','verify_source.py','--group','budget' if area=='budget' else 'vec-cursors','--mutations','--output',str(out/'direct-source'),kind='direct-product-source-proof')
                continue
            if area=='arithmetic':raise ValueError('Arithmetic projections have been removed. Use verus or direct-source; scripts/verify_arithmetic.py also checks code generation and final links.')
            mapping={'foundation':'verify_owner_protocol.py','box':'verify_owner_protocol.py','vec':'verify_owner_protocol.py',
                'string':'verify_string.py','shared':'verify_shared.py','btree':'verify_btree.py','btree-set':'verify_btree.py','hash-map':'verify_hash_map.py','hash-set':'verify_hash_set.py','binary-heap':'verify_binary_heap.py','deque':'verify_deque.py','scratch-slice':'verify_scratch_slice.py'}
            files=list(dict.fromkeys(mapping.values())) if area=='all' else [mapping[area]]
            for file in files:
                script('models-'+file.removesuffix('.py'),file,*(['--kani','--mutations'] if file in ('verify_hash_set.py','verify_binary_heap.py','verify_deque.py','verify_scratch_slice.py') else []),kind='actual-Rust-model-check' if file in ('verify_hash_set.py','verify_binary_heap.py','verify_deque.py','verify_scratch_slice.py') else 'projected-model-proof')
        elif check=='kani':
            patterns={'all':[''],'foundation':['proofs::budget_arithmetic_and_raii','proofs::copy_mismatch_preserves_destination','proofs::scalar_array_clone_preserves_values','proofs::checked_array_layout','iter::proofs::'],'arithmetic':['arithmetic::proofs::'],'box':['proofs::dynamic_slice_initialization_and_budget_cleanup','proofs::collected_slice_reallocation_preserves_values','proofs::struct_fields_require_every_write','boxed::buffer::proofs::','vec::metadata_proofs::boxed_and_vector_unit_owners_transfer_every_word_length_without_moving_storage',
                    # Loop-contract harnesses compile only with this cfg (as in verify_panic_paths.py).
                    # The slice initialization loop is proved by the direct Verus init-prefix
                    # group; its earlier whole-loop Kani harness is the --legacy-kani diagnostic.
                    'loop:panic_proofs::box_iterator_counters_cover_every_machine_word_length',
                    'loop:boxed::slice::panic_proofs::box_iterator_preserves_every_valid_cursor_state','loop:boxed::text::panic_proofs::character_byte_cursor_preserves_every_valid_state'],
                'vec':['proofs::vec_'],'string':['string::proofs::'],'shared':['shared::proofs::','budget::concurrent::proofs::'],
                'budget':['budget::concurrent::proofs::'],'btree':['collections::btree_map::proofs::','collections::btree_map::iter::borrowed_edge_proofs::','collections::btree_map::view::bplus::proofs::'],
                'hash-map':['collections::hash_map::proofs::','collections::hash_map::iter::scan_proofs::'],
                'binary-heap':['collections::binary_heap::proofs::'],'deque':['deque::proofs::'],'scratch-slice':['scratch::proofs::','slice::proofs::'],
                'hash-set':['collections::hash_set::proofs::','collections::hash_map::proofs::hash_set_replace_preserves_slot_class_length_and_credit'],
                'btree-set':['collections::btree_set::proofs::','collections::btree_set::algebra::merge_proofs::','collections::btree_map::proofs::set_replacement_every_initialized_slot','collections::btree_map::iter::borrowed_edge_proofs::']}
            for index,pattern in enumerate(patterns[area]):
                loop=pattern.startswith('loop:');pattern=pattern.removeprefix('loop:')
                cmd=['cargo','kani','--lib']+(['-Z','loop-contracts'] if loop else [])+(['--harness',pattern] if pattern else [])
                if area in ('foundation','box') and not pattern.endswith('::'):cmd+=['--exact']
                if area!='box' and ('btree_map' in pattern or 'hash_map' in pattern or 'hash_set' in pattern or 'binary_heap' in pattern or 'deque::' in pattern or 'scratch::' in pattern or 'slice::' in pattern):cmd+=['-Z','unstable-options','--cbmc-args','--object-bits','12' if 'hash_map' in pattern or 'hash_set' in pattern or 'binary_heap' in pattern or 'deque::' in pattern or 'scratch::' in pattern or 'slice::' in pattern else '10']
                add('kani-'+str(index),cmd,'actual-Rust-model-check',{'RUSTFLAGS':'--cfg jzap_loop_contracts'} if loop else None)
            if area in ('all','budget'):script('kani-local-budget','verify_budget_native.py','--mutations',kind='actual-Rust-model-check')
        elif check=='loom':
            if area in ('hash-map','hash-set'):raise ValueError('Default hash keys are thread-local (docs/random-state.md); no shared seed protocol remains for Loom.')
            if area not in ('all','shared','budget'):raise ValueError('Loom covers shared owners and budgets; this collection has exclusive ownership.')
            add(check,['cargo','test','--locked','--offline','--manifest-path','tools/shared-loom/Cargo.toml','--test','concurrent','--test','headroom'],'actual-implementation-schedules',{'RUSTFLAGS':'--cfg jzap_loom --cfg jzap_small_counts'})
        elif check=='small-counts':
            if area not in ('all','shared'):raise ValueError('Reduced reference-count limits apply to shared ownership.')
            add(check,['cargo','test','--locked','--offline','--all-features','--test','shared'],
                'reduced-limit-runtime-check',{'RUSTFLAGS':'--cfg jzap_small_counts'})
        elif check=='bounds':
            if area in ('all','btree','btree-set','hash-map','hash-set','deque','scratch-slice'):script('bounds-btree','check_btree_bounds.py','--mutations',kind='compiler-boundary')
            script('bounds-common','verify_thread_bounds.py','--mutations',kind='compiler-boundary')
        elif check=='unsafe':script(check,'check_unsafe_inventory.py',kind='census-and-routing')
        elif check=='inventory':
            files=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs') if not p.name.endswith('proofs.rs'))
            add('inventory-build',['cargo','build','--locked','--offline','--manifest-path','tools/panic-audit/Cargo.toml',
                '--bin','unsafe_sites','--target-dir','target/panic-inventory'],'verification-tool-build')
            add(check,['target/panic-inventory/debug/unsafe_sites',*files],'syntactic-source-inventory')
        elif check=='audit':
            add('audit',['cargo','audit','--no-fetch','--file','Cargo.lock'],'dependency-audit')
            add('audit-loom',['cargo','audit','--no-fetch','--file','tools/shared-loom/Cargo.lock'],'dependency-audit')
            add('licenses',['cargo','deny','--offline','check','advisories','licenses','sources'],'dependency-audit')
        elif check=='sbom':add(check,[sys.executable,'-c',SBOM,str(out/'jzap-sbom.json')],'dependency-inventory')
        elif check=='link':
            script(check,'check_link_exits.py','--output',str(out/'link'),kind='concrete-call-graph-check')
            if area in ('all','btree'):
                script('link-bplus-consumer','check_consumer_link.py',
                       '--source','tools/consumer-audit/cases/bplus_pool.rs',
                       '--source-dependency','tools/throughput-analysis/src/bplus_pool.rs',
                       '--require-success','--output',str(out/'bplus-consumer'),
                       kind='concrete-call-graph-check')
            # The no_std fixture cannot link default std entropy; walk that root separately.
            if area in ('all','hash-map','hash-set'):script('link-seed','check_seed_link.py','--profiles','release,release-lto,release-checked','--output',str(out/'link-seed'),kind='concrete-call-graph-check')
        elif check=='certificate':
            if area!='all':raise ValueError('The combined certificate is global. Use --area all --check certificate; partial checks cannot replace it.')
            script(check,'source_certificate.py','--output',str(out/'certificate'),kind='combined-model-source-link-evidence')
    return jobs
def save(path,data):
    path.parent.mkdir(parents=True,exist_ok=True);tmp=path.with_suffix('.tmp');tmp.write_text(json.dumps(data,indent=2)+'\n');tmp.replace(path)
def terminate(process):
    os.killpg(process.pid,signal.SIGTERM)
    try:process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid,signal.SIGKILL);process.wait()
def source_identity():
    names=subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'],cwd=ROOT).decode().split('\0')
    prefixes=('src/','tests/','scripts/','tools/memory-proof/','tools/panic-audit/',
        'tools/link-audit/','tools/seed-link-audit/','tools/shared-loom/','tools/direct-verus/','fuzz/','.cargo/')
    files=sorted({n for n in names if n and (ROOT/n).is_file() and
        (n.startswith(prefixes) or n in ('Cargo.toml','Cargo.lock','clippy.toml'))})
    digest=hashlib.sha256()
    for name in files:digest.update(name.encode()+b'\0'+(ROOT/name).read_bytes())
    return dict(commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT)),input_sha256=digest.hexdigest(),files=len(files))
def prepare_python_modules(root=ROOT):
    if not __debug__:
        raise RuntimeError('Verification requires Python assertions; remove -O/PYTHONOPTIMIZE')
    for source in (root/'scripts').glob('*.py'):
        py_compile.compile(str(source),doraise=True,
            invalidation_mode=py_compile.PycInvalidationMode.CHECKED_HASH)

def execute(jobs,out,keep_going=False,timeout=0):
    prepare_python_modules()
    out.mkdir(parents=True,exist_ok=True)
    before=source_identity()
    receipt=dict(scope='Selected checks only; not full product certification.',passed_all_selected=False,source=before,source_unchanged=False,
        results=[dict(job,status='not-run') for job in jobs]);save(out/'summary.json',receipt)
    for row in receipt['results']:
        row.update(status='running',log=str(out/(row['name']+'.log')));save(out/'summary.json',receipt)
        print('RUN '+row['name']+' ['+row['kind']+']',flush=True);started=time.monotonic();process=None
        try:
            with Path(row['log']).open('w') as log:
                process=subprocess.Popen(row['command'],cwd=ROOT,env=dict(os.environ,**row['environment']),stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
                code=process.wait(timeout=timeout or None)
            row.update(exit_code=code,status='passed' if code==0 else 'failed')
            if code==0 and row['kind']=='actual-Rust-model-check' and row['command'][:2]==['cargo','kani']:
                summaries=re.findall(r'Complete - (\d+) successfully verified harnesses, (\d+) failures, (\d+) total\.',Path(row['log']).read_text())
                if not summaries or any(int(total)==0 or int(passed)!=int(total) or int(failed)!=0 for passed,failed,total in summaries):
                    row.update(status='no-verification-result',error='No complete nonempty Kani proof result.')
            if code==0 and row['kind']=='syntactic-source-inventory':
                sites=json.loads(Path(row['log']).read_text())
                counts={kind:sum(s['kind']==kind for s in sites) for kind in sorted({s['kind'] for s in sites})}
                row.update(sites=len(sites),counts=counts,scope='Whole-source AST sites, not a count of independent proofs or discharged obligations.')
                print('AST sites: '+json.dumps(counts,sort_keys=True),flush=True)
        except FileNotFoundError as error:row.update(status='missing-tool',error=str(error))
        except (OSError,ValueError,KeyError,TypeError) as error:row.update(status='invalid-result',error=str(error))
        except subprocess.TimeoutExpired:
            terminate(process);row.update(status='timed-out',exit_code=124)
        except KeyboardInterrupt:
            if process is not None:terminate(process)
            row.update(status='cancelled');save(out/'summary.json',receipt);return 130
        row['elapsed_seconds']=time.monotonic()-started;save(out/'summary.json',receipt)
        print(row['status'].upper()+' '+row['name']+' -> '+row['log'],flush=True)
        if row['status']!='passed' and not keep_going:break
    receipt['source_unchanged']=source_identity()['input_sha256']==before['input_sha256']
    receipt['passed_all_selected']=receipt['source_unchanged'] and bool(jobs) and all(r['status']=='passed' for r in receipt['results']);save(out/'summary.json',receipt)
    print('Selected checks: '+('PASS' if receipt['passed_all_selected'] else 'INCOMPLETE/FAILED')+'; '+str(out/'summary.json'),flush=True)
    return 0 if receipt['passed_all_selected'] else 1
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--area',choices=AREAS,default='all');p.add_argument('--check',choices=CHECKS,action='append',help='repeat to select checks; overrides profile')
    p.add_argument('--test-target',action='append',help='repeat to select named Cargo integration tests within the area; runtime checks only')
    p.add_argument('--profile',choices=['quick','memory','full'],default='quick',help='quick default; full requires area all')
    p.add_argument('--list',action='store_true');p.add_argument('--plan',action='store_true',help='commands only; no checks run and no pass claimed')
    p.add_argument('--keep-going',action='store_true');p.add_argument('--timeout',type=float,default=0,help='seconds per command; 0 imposes no timeout')
    p.add_argument('--output',type=Path,help='fresh directory under ignored target/')
    a=p.parse_args()
    if a.list:
        print('Areas: '+', '.join(AREAS))
        for name,description in CHECKS.items():print(f'{name:12} {description}')
        return 0
    if a.timeout<0:p.error('timeout must be nonnegative')
    checks=list(dict.fromkeys(a.check or (FULL if a.profile=='full' else ['tests','miri','miri-32','miri-tree','asan'] if a.profile=='memory' else QUICK)))
    out=(a.output or ROOT/'target/verification/replay'/time.strftime('%Y%m%d-%H%M%S')).resolve()
    if not out.is_relative_to(ROOT/'target'):p.error('Keep generated logs under ignored target/.')
    try:jobs=plan(a.area,checks,out,a.test_target)
    except ValueError as error:p.error(str(error))
    if a.plan:
        print('PLAN ONLY: no checks executed.')
        for job in jobs:print(job['name']+' ['+job['kind']+']\n  '+shlex.join([*(k+'='+v for k,v in job['environment'].items()),*job['command']]))
        return 0
    if (out/'summary.json').exists():p.error('This output already has a receipt; choose a fresh directory.')
    return execute(jobs,out,a.keep_going,a.timeout)
if __name__=='__main__':raise SystemExit(main())
