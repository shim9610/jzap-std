#!/usr/bin/env python3
"""Exercise selection and real process failures without invoking Rust tools."""
import contextlib,io,json,sys,tempfile,unittest
from unittest.mock import patch
from pathlib import Path
from verify import ROOT,AREAS,CHECKS,FULL,execute,plan,prepare_python_modules
import re,shutil
def compact_text(text):
    return re.sub(r'\s+', '', text)
class ReplayTests(unittest.TestCase):
    def test_throughput_profiles_keep_loop_alignment_opt_in(self):
        import importlib.util
        spec=importlib.util.spec_from_file_location('prepare_range',ROOT/'tools/throughput-analysis/prepare_range.py')
        module=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertEqual(module.selected_profiles('both'),{'default','aligned'})
        for profile in ('default','aligned','loops32'):
            self.assertEqual(module.selected_profiles(profile),{profile})

    def test_box_kani_names_every_box_harness_and_enables_loop_contracts(self):
        jobs=plan('box',['kani'],ROOT/'target/probe')
        self.assertEqual(len(jobs),8)
        for job in jobs:
            self.assertIn('--exact' if not job['command'][job['command'].index('--harness')+1].endswith('::') else '--harness',job['command'])
            loop='loop-contracts' in job['command']
            self.assertEqual(loop,job['environment'].get('RUSTFLAGS')=='--cfg jzap_loop_contracts')
        self.assertEqual(sum('loop-contracts' in job['command'] for job in jobs),3)

    def test_explicit_runtime_targets_remain_partial_and_isolated(self):
        for area in ('vec','all'):
            for job in plan(area,['tests','miri','miri-32','miri-tree','asan'],ROOT/'target/probe',['vec']):
                self.assertEqual(job['command'].count('--test'),1)
                self.assertIn('vec',job['command'])
                self.assertNotIn('vec_properties',job['command'])
                if job['name'].startswith('miri'):
                    self.assertEqual(job['environment']['PROPTEST_DISABLE_FAILURE_PERSISTENCE'],'1')
                    self.assertEqual(job['environment']['PROPTEST_RNG_SEED'],'1')
                    self.assertNotIn('disable-isolation',job['environment']['MIRIFLAGS'])
                    self.assertEqual(job['environment']['PROPTEST_RNG_ALGORITHM'],'xs')
                    for name in ('PROPTEST_DISABLE_FAILURE_PERSISTENCE','PROPTEST_RNG_SEED','PROPTEST_RNG_ALGORITHM'):
                        self.assertIn('-Zmiri-env-forward='+name,job['environment']['MIRIFLAGS'])
        for targets,checks in [(['shared'],['miri']),(['vec','vec'],['tests']),([],['tests']),(['vec'],['certificate']),(['vec'],['verus'])]:
            with self.assertRaises(ValueError):plan('vec',checks,ROOT/'target/probe',targets)

    def test_state_composition_compiles_the_original_crate(self):
        from verus_composition import run
        # Patching subprocess.run replaces it for every module, including the
        # rustc queries of the std specification build, so that build is
        # replaced too and only its import flags are checked here.
        flags=['--import','jzap_verus_std_specs=specs.vir']
        with patch('verus_composition.verus_std_specs.build',return_value=flags) as specs, \
             patch('verus_composition.subprocess.run') as invoke:
            command,_=run('verus',ROOT,ROOT/'target/calls.rs','actual_product',{})
            specs.assert_called_once_with('verus')
            self.assertEqual(command[-len(flags):],flags)
            self.assertIn(str(ROOT/'src/lib.rs'),command)
            self.assertIn('--no-cheating',command)
            self.assertNotIn('--verify-module',command)
            self.assertEqual(invoke.call_args.kwargs['env']['JZAP_PROOF_COMPOSITION'],str(ROOT/'target/calls.rs'))
            self.assertEqual(invoke.call_args.kwargs['cwd'],ROOT)

    def test_migrated_direct_groups_are_selectable_without_legacy_models(self):
        job=plan('foundation',['direct-source'],ROOT/'target/probe')[0]
        self.assertEqual(job['kind'],'direct-product-source-proof')
        self.assertIn('verify_source.py',' '.join(job['command']))
        self.assertIn('slot',job['command'])
        self.assertIn('init-prefix',plan('box',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('clone',job['command'])
        self.assertIn('vec-cursors',job['command'])
        self.assertEqual(plan('vec',['models'],ROOT/'target/probe')[0]['kind'],'direct-product-source-proof')
        self.assertIn('budget',job['command'])
        self.assertIn('vec-geometry',plan('vec',['direct-source'],ROOT/'target/probe')[0]['command'])
        for group in ('vec-compaction','vec-compaction-values','vec-extraction','vec-extraction-values','vec-staging'):
            self.assertIn(group,plan('vec',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('vec-geometry',plan('vec',['verus'],ROOT/'target/probe')[0]['command'])
        self.assertIn('budget',plan('budget',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertNotIn('verify_verus.py',' '.join(job['command']))
        self.assertNotIn('source_certificate.py',' '.join(job['command']))
        self.assertIn('btree-search-cursors',plan('btree',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('btree-plan-counts',plan('btree',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('btree-frontiers',plan('btree',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('btree-delete-decisions',plan('btree',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('btree-packed-shape',plan('btree',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('btree-node-spans',plan('btree',['direct-source'],ROOT/'target/probe')[0]['command'])
        self.assertIn('btree-search-cursors',plan('btree',['verus'],ROOT/'target/probe')[0]['command'])
        self.assertIn('btree-plan-counts',plan('btree',['verus'],ROOT/'target/probe')[0]['command'])
        for area in ('btree','btree-set'):
            self.assertIn('btree-node-spans',plan(area,['verus'],ROOT/'target/probe')[0]['command'])
        with self.assertRaises(ValueError):plan('hash-map',['direct-source'],ROOT/'target/probe')

    def test_certificate_runs_every_closure_group_and_pins_its_controls(self):
        import verify_source
        from source_certificate import FOUNDATION_CONTROLS, FOUNDATION_GROUPS
        self.assertIn('vec-element-loops', FOUNDATION_GROUPS)
        self.assertTrue(set(FOUNDATION_GROUPS) <= set(verify_source.GROUPS))
        self.assertEqual(FOUNDATION_CONTROLS,
                         sum(row[1] in FOUNDATION_GROUPS for row in verify_source.CONTROLS))
        # Every group that holds a closure harness is one the certificate runs.
        self.assertTrue(set(verify_source.CLOSURE) | set(verify_source.EXTRA) <= set(FOUNDATION_GROUPS))
        for area in ('vec', 'foundation', 'all'):
            self.assertIn('vec-element-loops', plan(area, ['direct-source'], ROOT/'target/probe')[0]['command'])
        for area in ('deque', 'foundation', 'all'):
            self.assertIn('deque-closure', plan(area, ['direct-source'], ROOT/'target/probe')[0]['command'])
        for area in ('binary-heap', 'foundation', 'all'):
            self.assertIn('heap-closure', plan(area, ['direct-source'], ROOT/'target/probe')[0]['command'])

    def test_deque_model_harnesses_are_selected_by_exact_name(self):
        # The closure harnesses of deque::proofs::closure (issue #16, unit 4)
        # must not change the selection or the count of the nine model harnesses.
        import verify
        import verify_deque
        import verify_source
        self.assertEqual(['deque::proofs::' + n for n in verify.DEQUE_MODEL_HARNESSES], verify_deque.HARNESSES)
        [job] = plan('deque', ['kani'], ROOT/'target/probe')
        self.assertIn('--exact', job['command'])
        self.assertEqual([job['command'][i + 1] for i, a in enumerate(job['command']) if a == '--harness'],
                         verify_deque.HARNESSES)
        closure = verify_source.CLOSURE['deque-closure']
        self.assertTrue(closure and all(n.startswith('deque::proofs::closure::') for n in closure))
        self.assertFalse(set(closure) & set(verify_deque.HARNESSES))

    def test_heap_model_harnesses_are_selected_by_exact_name(self):
        # The closure harnesses of collections::binary_heap::proofs::closure
        # (issue #16, unit 7) must not change the selection or count of the six.
        import verify
        import verify_binary_heap
        import verify_source
        self.assertEqual(['collections::binary_heap::proofs::' + n for n in verify.HEAP_MODEL_HARNESSES],
                         verify_binary_heap.HARNESSES)
        [job] = plan('binary-heap', ['kani'], ROOT/'target/probe')
        self.assertIn('--exact', job['command'])
        self.assertEqual([job['command'][i + 1] for i, a in enumerate(job['command']) if a == '--harness'],
                         verify_binary_heap.HARNESSES)
        closure = verify_source.CLOSURE['heap-closure']
        self.assertTrue(closure and all(n.startswith('collections::binary_heap::proofs::closure::') for n in closure))
        self.assertFalse(set(closure) & set(verify_binary_heap.HARNESSES))
        source = (ROOT / 'scripts/verify_binary_heap.py').read_text()
        self.assertIn("'--exact',*[a for n in HARNESSES for a in ('--harness',n)]", source)

    def test_actual_source_negative_control_requires_a_solver_result(self):
        from types import SimpleNamespace
        from verify_source import kani
        for diagnostic in ('error[E0000]: compilation failure',
                           'Complete - 0 successfully verified harnesses, 1 failures, 1 total.'):
            with self.subTest(diagnostic=diagnostic),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp)
                def failed_compile(*args,**kwargs):
                    kwargs['stdout'].write(diagnostic)
                    return SimpleNamespace(returncode=1)
                with patch('verify_source.subprocess.run',side_effect=failed_compile):
                    with self.assertRaises(RuntimeError):kani(root,root,['actual::harness'],failure=True)

    def test_same_size_same_second_edits_use_current_helper(self):
        import os,py_compile,subprocess
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);directory=root/'scripts';directory.mkdir()
            source=directory/'cache_fixture.py'
            def write(value):
                source.write_text('VALUE='+str(value)+'\n')
                os.utime(source,(1700000000,1700000000))
            command=[sys.executable]
            if sys.pycache_prefix is not None:
                command+=['-X','pycache_prefix='+sys.pycache_prefix]
            command+=['-c','import sys;sys.path.insert(0,sys.argv[1]);import cache_fixture;print(cache_fixture.VALUE)',str(directory)]
            write(1)
            py_compile.compile(str(source),doraise=True,invalidation_mode=py_compile.PycInvalidationMode.TIMESTAMP)
            write(2)
            self.assertEqual(subprocess.check_output(command,text=True).strip(),'1')
            prepare_python_modules(root)
            self.assertEqual(subprocess.check_output(command,text=True).strip(),'2')
            write(3)
            self.assertEqual(subprocess.check_output(command,text=True).strip(),'3')

    def test_optimized_python_cannot_claim_verification(self):
        import subprocess
        r=subprocess.run([sys.executable,'-O',str(ROOT/'scripts/verify.py'),'--check','runner'],
            cwd=ROOT,capture_output=True,text=True)
        self.assertNotEqual(r.returncode,0)
        self.assertIn('Verification requires Python assertions',r.stderr)

    def test_deque_selection_and_closed_review_domain(self):
        import shutil
        from verify_deque import FILES,bindings,is_deque
        for job in plan('deque',['tests','miri-tree','verus','kani'],ROOT/'target/probe'):
            self.assertIn('deque',' '.join(job['command']))
            self.assertNotIn('source_certificate.py',' '.join(job['command']))
        with tempfile.TemporaryDirectory() as tmp:
            work=Path(tmp)
            shutil.copytree(ROOT/'src/deque',work/'src/deque')
            shutil.copyfile(ROOT/'src/deque.rs',work/'src/deque.rs')
            self.assertTrue(all(is_deque(f) for f in FILES))
            self.assertFalse(is_deque('src/deque/new_owner.rs'))
            with patch('verify_deque.subprocess.check_output',return_value='[]'):
                self.assertEqual(len(bindings(work)[0]),5)
                new=work/'src/deque/new_owner.rs';new.write_text('fn changed() {}')
                with self.assertRaisesRegex(AssertionError,'domain changed'):bindings(work)
                new.unlink();source=work/'src/deque.rs';source.write_text(source.read_text()+'\nfn changed() {}\n')
                with self.assertRaisesRegex(AssertionError,'requires review'):bindings(work)
    def test_heap_receipt_includes_shared_source_and_control_dependencies(self):
        from verify_binary_heap import inputs
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            for name in ['src/collections/binary_heap.rs','src/vec.rs','src/budget.rs',
                         'tests/binary_heap.rs','tests/common/mod.rs',
                         'Cargo.toml','Cargo.lock','scripts/verify_binary_heap.py','scripts/heap_callers.py',
                         'scripts/maintained_callers.py','tools/memory-proof/binary-heap.verus.rs',
                         'tools/memory-proof/core-contracts.verus.rs','tools/memory-proof/pointer-checks.verus.rs',
                         'tools/memory-proof/extent.verus.rs','tools/memory-proof/storage.verus.rs',
                         'scripts/pointer_predicates.py','scripts/storage_kernel.py']:
                p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('original')
            before=inputs(root)
            for name in ['src/vec.rs','src/budget.rs','tests/common/mod.rs','Cargo.lock','scripts/heap_callers.py',
                         'tools/memory-proof/binary-heap.verus.rs','tools/memory-proof/storage.verus.rs',
                         'tools/memory-proof/core-contracts.verus.rs','scripts/storage_kernel.py']:
                p=root/name;p.write_text('changed')
                self.assertNotEqual(inputs(root),before,name)
                p.write_text('original');self.assertEqual(inputs(root),before)
            new=root/'src/new_dependency.rs';new.write_text('new')
            self.assertNotEqual(inputs(root),before)
            new.unlink()
            ignored=root/'target/verification/run.log';ignored.parent.mkdir(parents=True);ignored.write_text('log')
            self.assertEqual(inputs(root),before)
    def test_memory_census_delegates_only_registered_heap_modules(self):
        from verify_binary_heap import FILES
        from verify_wrapping_memory import inventory,coverage
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            unknown='src/collections/binary_heap/unreviewed.rs'
            for name in [*FILES,unknown,'src/span.rs']:
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True)
                path.write_text('')
            with patch('verify_wrapping_memory.subprocess.check_output',return_value='[]') as parse:
                inventory(root)
                selected=set(parse.call_args.args[0][1:])
            self.assertEqual(selected,{unknown,'src/span.rs'})
            row=dict(file=unknown,kind='modular_call',context='',function='rogue',expression='wrapping_add(n,1)')
            with self.assertRaisesRegex(AssertionError,'modular call has no verified contract'):
                coverage([row])

    def test_memory_census_delegates_only_registered_scratch_modules(self):
        from verify_scratch_slice import FILES
        from verify_wrapping_memory import inventory,coverage
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            unknown='src/scratch/unreviewed.rs'
            for name in [*FILES,unknown,'src/span.rs']:
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True)
                path.write_text('')
            with patch('verify_wrapping_memory.subprocess.check_output',return_value='[]') as parse:
                inventory(root)
                selected=set(parse.call_args.args[0][1:])
            self.assertEqual(selected,{unknown,'src/span.rs'})
            row=dict(file=unknown,kind='modular_call',context='',function='rogue',expression='wrapping_add(n,1)')
            with self.assertRaisesRegex(AssertionError,'modular call has no verified contract'):
                coverage([row])
    def test_hash_set_separate_domain_rejects_new_files_and_changed_delegation(self):
        import shutil
        from verify_hash_set import bindings, is_hash_set
        self.assertTrue(is_hash_set('src/collections/hash_set.rs'))
        self.assertFalse(is_hash_set('src/collections/hash_set/new_owner.rs'))
        with tempfile.TemporaryDirectory() as tmp:
            work=Path(tmp);dest=work/'src/collections'
            dest.mkdir(parents=True)
            shutil.copyfile(ROOT/'src/collections/hash_set.rs',dest/'hash_set.rs')
            shutil.copytree(ROOT/'src/collections/hash_set',dest/'hash_set')
            with patch('verify_hash_set.subprocess.check_output',return_value='[]'):
                self.assertEqual(len(bindings(work)),3)
                new=dest/'hash_set/new_owner.rs';new.write_text('pub struct NewOwner;')
                with self.assertRaisesRegex(AssertionError,'domain changed'):bindings(work)
                new.unlink()
                source=dest/'hash_set.rs';source.write_text(source.read_text()+'\nfn unreviewed() {}\n')
                with self.assertRaisesRegex(AssertionError,'requires review'):bindings(work)
    def test_selected_memory_scope_and_no_implicit_full_proof(self):
        jobs=plan('btree-set',['tests','miri'],ROOT/'target/probe')
        self.assertEqual(len(jobs),2)
        for job in jobs:
            self.assertIn('btree_set',job['command'])
            self.assertNotIn('source_certificate.py',' '.join(job['command']))
            self.assertNotIn('--test shared',' '.join(job['command']))
    def test_direct_and_model_evidence_are_distinct(self):
        direct=plan('budget',['verus'],ROOT/'target/probe')[0]
        model=plan('string',['models'],ROOT/'target/probe')[0]
        self.assertEqual(direct['kind'],'direct-product-source-proof')
        self.assertEqual(model['kind'],'projected-model-proof')
        self.assertIn('budget-admit',direct['command'])
        self.assertIn('local-budget',direct['command'])
        self.assertIn('btree-search-cursors',plan('btree-set',['verus'],ROOT/'target/probe')[0]['command'])
        with self.assertRaises(ValueError):plan('box',['verus'],ROOT/'target/probe')
        with self.assertRaises(ValueError):plan('vec',['certificate'],ROOT/'target/probe')
        self.assertIn('integer',plan('arithmetic',['verus'],ROOT/'target/probe')[0]['command'])
        self.assertIn('arithmetic',plan('arithmetic',['direct-source'],ROOT/'target/probe')[0]['command'])
        with self.assertRaisesRegex(ValueError,'projections have been removed'):
            plan('arithmetic',['models'],ROOT/'target/probe')
    def test_loom_plan_instruments_actual_atomics_and_reduced_limits(self):
        job=plan('shared',['loom'],ROOT/'target/probe')[0]
        self.assertEqual(job['environment']['RUSTFLAGS'],'--cfg jzap_loom --cfg jzap_small_counts')
    def test_inventory_is_independent_of_solver_and_review_seals(self):
        job=plan('btree-set',['inventory'],ROOT/'target/probe')[-1]
        self.assertTrue(job['command'][0].endswith('/unsafe_sites'))
        self.assertIn('src/collections/btree_set/algebra.rs',job['command'])
        self.assertNotIn('source_certificate.py',' '.join(job['command']))
    def test_consolidated_drivers_keep_unique_checks(self):
        self.assertTrue({'small-counts','sbom'}<=set(FULL))
        job=plan('shared',['small-counts'],ROOT/'target/probe')[0]
        self.assertEqual(job['environment']['RUSTFLAGS'],'--cfg jzap_small_counts')
        self.assertIn('tools/shared-loom/Cargo.lock',' '.join(str(j['command']) for j in plan('all',['audit'],ROOT/'target/probe')))
    def test_btree_deep_fuzz_seeds_are_reproduced(self):
        import subprocess
        with tempfile.TemporaryDirectory() as tmp:
            jobs=plan('btree',['fuzz'],Path(tmp))
            subprocess.run(jobs[0]['command'],check=True)
            self.assertIn('-max_len=16384',jobs[1]['command'])
            self.assertIn(str(Path(tmp)/'btree-corpus'),jobs[1]['command'])
            for count in (12,144,1200):
                data=(Path(tmp)/'btree-corpus'/str(count)).read_bytes()
                self.assertEqual(len(data),count*4+28)
                self.assertEqual(data[:4],bytes([255,0,0,0]))
                self.assertEqual(data[-4:],bytes([8,53,0,7]))
    def test_every_selectable_plan_resolves_script_paths(self):
        for area in AREAS:
            for check in CHECKS:
                try:jobs=plan(area,[check],ROOT/'target/probe')
                except ValueError:continue
                for job in jobs:
                    for token in job['command']:
                        if token.startswith('scripts/'):self.assertTrue((ROOT/token).is_file(),(area,check,token))
    def test_sbom_requires_a_new_result_and_preserves_existing_files(self):
        import os,subprocess
        with tempfile.TemporaryDirectory() as tmp:
            work=Path(tmp);out=work/'out';out.mkdir()
            cargo=work/'cargo';cargo.write_text('#!/bin/sh\nexit 0\n');cargo.chmod(0o755)
            command=plan('all',['sbom'],out)[0]['command']
            def run():return subprocess.run(command,cwd=work,env=dict(os.environ,PATH=str(work)),capture_output=True)
            self.assertNotEqual(run().returncode,0)
            self.assertFalse((out/'jzap-sbom.json').exists())
            existing=work/'jzap-sbom.json';existing.write_text('original')
            self.assertNotEqual(run().returncode,0)
            self.assertEqual(existing.read_text(),'original')
    def run_jobs(self,commands,kind='test-fixture',**kwargs):
        jobs=[dict(name=str(i),command=cmd,kind=kind,environment={}) for i,cmd in enumerate(commands)]
        with tempfile.TemporaryDirectory(prefix='jzap-replay-test-') as tmp:
            out=Path(tmp)
            with contextlib.redirect_stdout(io.StringIO()):code=execute(jobs,out,**kwargs)
            return code,json.loads((out/'summary.json').read_text())
    def test_failed_command_leaves_remaining_work_unrun(self):
        code,r=self.run_jobs([[sys.executable,'-c','pass'],[sys.executable,'-c','raise SystemExit(7)'],[sys.executable,'-c','pass']])
        self.assertEqual(code,1);self.assertFalse(r['passed_all_selected'])
        self.assertEqual([x['status'] for x in r['results']],['passed','failed','not-run'])
        self.assertEqual(r['results'][1]['exit_code'],7)
    def test_keep_going_does_not_hide_a_failure(self):
        code,r=self.run_jobs([[sys.executable,'-c','raise SystemExit(2)'],[sys.executable,'-c','pass']],keep_going=True)
        self.assertEqual(code,1);self.assertEqual([x['status'] for x in r['results']],['failed','passed'])
    def test_missing_tool_and_timeout_are_not_passes(self):
        code,r=self.run_jobs([['/nonexistent/jzap-verification-tool']]);self.assertEqual(code,1);self.assertEqual(r['results'][0]['status'],'missing-tool')
        code,r=self.run_jobs([[sys.executable,'-c','import time;time.sleep(30)']],timeout=0.05)
        self.assertEqual(code,1);self.assertEqual(r['results'][0]['status'],'timed-out')
    def test_all_selected_successes(self):
        code,r=self.run_jobs([[sys.executable,'-c','print("checked")']]);self.assertEqual(code,0);self.assertTrue(r['passed_all_selected'])
    def test_kani_zero_exit_without_proofs_is_not_success(self):
        from types import SimpleNamespace
        for output in ['', 'Complete - 0 successfully verified harnesses, 0 failures, 0 total.',
                       'Complete - 1 successfully verified harnesses, 1 failures, 2 total.']:
            with patch('verify.source_identity',return_value={'input_sha256':'fixed'}),patch('verify.subprocess.Popen') as popen:
                def process(*args,**kwargs):
                    kwargs['stdout'].write(output);kwargs['stdout'].flush()
                    return SimpleNamespace(wait=lambda timeout=None:0)
                popen.side_effect=process
                code,r=self.run_jobs([['cargo','kani']],kind='actual-Rust-model-check')
            self.assertEqual(code,1);self.assertEqual(r['results'][0]['status'],'no-verification-result')
    def test_timeout_escalates_for_process_ignoring_termination(self):
        code,r=self.run_jobs([[sys.executable,'-c','import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);print("ready",flush=True);time.sleep(30)']],timeout=0.2)
        self.assertEqual(code,1);self.assertEqual(r['results'][0]['status'],'timed-out')
    def test_failed_direct_proof_cannot_retain_an_old_success(self):
        from verify_direct_verus import verify
        with tempfile.TemporaryDirectory(prefix='jzap-direct-receipt-') as tmp:
            out=Path(tmp);summary=out/'summary.json';summary.write_text('{"status":"pass"}')
            with patch('verify_direct_verus.executable',side_effect=RuntimeError('tool unavailable')):
                with self.assertRaises(RuntimeError):verify(out=out,targets=['bounded'])
            self.assertFalse(summary.exists())
    def test_changed_source_invalidates_otherwise_successful_run(self):
        with patch('verify.source_identity',side_effect=[{'input_sha256':'before'},{'input_sha256':'after'}]):
            code,r=self.run_jobs([[sys.executable,'-c','pass']])
        self.assertEqual(code,1);self.assertFalse(r['source_unchanged']);self.assertFalse(r['passed_all_selected'])
class CampaignTests(unittest.TestCase):
    def test_resumed_campaign_runs_each_step_once_and_rejects_changed_evidence(self):
        import source_certificate as certificate
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw);out=root/'out';out.mkdir();receipt=root/'receipt.json'
            step=[sys.executable,'-c','import pathlib,time;pathlib.Path("receipt.json").write_text(str(time.time_ns()))']
            with patch.dict(certificate.RECEIPTS,{'probe':['receipt.json']}),contextlib.redirect_stdout(io.StringIO()):
                first=certificate.Campaign(out,'a',False,root=root)
                self.assertEqual(first.run('probe',step),0);written=receipt.read_text()
                second=certificate.Campaign(out,'a',True,root=root)
                self.assertEqual(second.run('probe',step),0)
                self.assertEqual(receipt.read_text(),written)
                self.assertEqual(second.steps[0]['invocation'],1)
                self.assertEqual(len(second.state['invocations']),2)
                with self.assertRaises(AssertionError):second.run('probe',step+['other'])
                with self.assertRaises(AssertionError):certificate.Campaign(out,'b',True,root=root)
                (out/'probe.log').write_text('edited')
                with self.assertRaises(AssertionError):certificate.Campaign(out,'a',True,root=root).run('probe',step)
                (out/'probe.log').write_text('')
                receipt.write_text('edited')
                with self.assertRaises(AssertionError):certificate.Campaign(out,'a',True,root=root).run('probe',step)
                fresh=certificate.Campaign(out,'a',False,root=root)
                self.assertEqual(fresh.run('probe',step),0)
                self.assertNotEqual(receipt.read_text(),'edited')
                self.assertEqual(fresh.steps[0]['invocation'],1)
                killed=[sys.executable,'-c','import os,signal;os.kill(os.getpid(),signal.SIGKILL)']
                self.assertLess(fresh.run('killed',killed),0)
                self.assertNotIn('killed',fresh.state['steps'])

class DistributedCampaignTests(unittest.TestCase):
    def setUp(self):
        from certificate_artifacts import collect, publish
        from source_certificate import Campaign

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        self.directory = root / 'evidence'
        self.bundle = self.directory / 'certificate-test'
        self.manifest = self.bundle / 'manifest.json'
        self.context = dict(commit='abc', source_sha256='fingerprint', repository='owner/repo',
                            run_id='1000', run_attempt='1')
        self.stages = {'test': ('', ('probe',))}
        self.receipts = {'probe': ['receipt.json']}
        self.command = [sys.executable, '-c', 'from pathlib import Path; Path("receipt.json").write_text("{}")']
        output = root / 'output'
        output.mkdir()
        with patch.dict('source_certificate.RECEIPTS', self.receipts), contextlib.redirect_stdout(io.StringIO()):
            campaign = Campaign(output, self.context['source_sha256'], False, root=root)
            self.assertEqual(campaign.run('probe', self.command), 0)
        publish(self.bundle, 'test', self.context, campaign.steps, root, output)
        self.collect = lambda: collect(self.directory, self.context, self.stages, self.receipts,
                                       {'probe': (self.command, {})}, ('release',))

    def change_manifest(self, change):
        manifest = json.loads(self.manifest.read_text())
        change(manifest)
        self.manifest.write_text(json.dumps(manifest))

    def test_collects_fresh_evidence_without_repeating_step(self):
        steps, files = self.collect()
        self.assertEqual(steps['probe']['exit_code'], 0)
        self.assertEqual(files['checkout/receipt.json'].read_text(), '{}')
        self.assertIn('output/probe.log', files)

    def test_rejects_different_commit_fingerprint_or_run_attempt(self):
        for key in self.context:
            with self.subTest(key=key), patch.dict(self.context, {key: 'different'}):
                with self.assertRaisesRegex(AssertionError, 'stale or mismatched'):
                    self.collect()

    def test_rejects_missing_job_or_receipt(self):
        self.stages['missing'] = ('', ('absent',))
        with self.assertRaisesRegex(AssertionError, 'stage artifacts'):
            self.collect()
        del self.stages['missing']
        self.change_manifest(lambda manifest: manifest['steps'][0]['files'].pop('checkout/receipt.json'))
        with self.assertRaisesRegex(AssertionError, 'evidence files'):
            self.collect()

    def test_rejects_modified_receipt(self):
        (self.bundle / 'checkout/receipt.json').write_text('modified')
        with self.assertRaisesRegex(AssertionError, 'digest differs'):
            self.collect()

    def test_rejects_failed_step(self):
        self.change_manifest(lambda manifest: manifest['steps'][0].update(exit_code=1))
        with self.assertRaisesRegex(AssertionError, 'failed step'):
            self.collect()

    def test_rejects_changed_command(self):
        self.change_manifest(lambda manifest: manifest['steps'][0]['command'].append('--reuse-models'))
        with self.assertRaisesRegex(AssertionError, 'unexpected command'):
            self.collect()

    def test_rejects_unlisted_artifact_file(self):
        (self.bundle / 'unlisted.txt').touch()
        with self.assertRaisesRegex(AssertionError, 'unlisted artifact files'):
            self.collect()

class LinkCampaignTests(unittest.TestCase):
    def test_bplus_route_rejects_changed_ancestor_publication(self):
        from bplus_bindings import LOOKUP, SOURCES, check

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            for name in SOURCES:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes((ROOT / name).read_bytes())
            check(root)
            lookup = root / LOOKUP
            original = lookup.read_text()
            self.assertIn('slot.write(id);', original)
            lookup.write_text(original.replace('slot.write(id);', ''))
            with self.assertRaisesRegex(AssertionError, r'unbound B\+ source'):
                check(root)

    def test_retained_bplus_guard_requires_reviewed_callers(self):
        from link_call_graph import PAIR_WINDOW_NEXT, PAIR_WINDOW_PATH, SLICE_INDEX_FAIL, bind_pair_window_edges
        from source_certificate import classify

        edge = dict(function=f'10 <{PAIR_WINDOW_NEXT}>:',
                    target=SLICE_INDEX_FAIL, location='/rust/core/src/slice/index.rs:569')
        assembly = [
            edge['function'],
            f'20 <{PAIR_WINDOW_PATH[0]}>:',
            f'   20: call 10 <{PAIR_WINDOW_NEXT}>',
            f'30 <{PAIR_WINDOW_PATH[1]}>:',
            f'   30: call 20 <{PAIR_WINDOW_PATH[0]}>',
            f'40 <{PAIR_WINDOW_PATH[2]}>:',
            f'   40: call 30 <{PAIR_WINDOW_PATH[1]}>',
        ]
        with self.assertRaisesRegex(AssertionError, r'unbound B\+ pair-window caller'):
            classify(edge)
        bind_pair_window_edges([edge], '\n'.join(assembly), '')
        self.assertEqual(classify(edge), ('bplus-pair-window-advance', ['pair_window_advance']))
        with self.assertRaisesRegex(ValueError, 'unexpected callers'):
            bind_pair_window_edges([edge], '\n'.join([*assembly, '50 <unexpected>:', *assembly[-1:]]), '')


class PremiseTests(unittest.TestCase):
    def test_spec_functions_are_not_verified_names(self):
        from premise_status import verified_function_names
        text = """verus! {
            spec fn a() -> bool { true }
            pub open spec fn b() -> bool { true }
            pub(crate) closed spec fn c() -> bool { true }
            spec(checked) fn d() -> bool { true }
            proof fn e() {}
            pub broadcast proof fn f() {}
            fn g() {}
            pub(crate) fn h<T>(x: T) {}
            exec fn i() {}
        }"""
        self.assertEqual(verified_function_names(text), {'e', 'f', 'g', 'h', 'i'})

    def test_guarded_increments_are_pinned_next_to_their_guards(self):
        from premise_status import premise
        from verify_wrapping_memory import FAMILIES, guarded_stores
        files = ('src/init/driver.rs', 'src/vec/staging_driver.rs')
        texts = {name: (ROOT / name).read_text() for name in files}
        guarded_stores(texts)
        self.assertEqual(premise('guarded_increment')['basis'], 'no-premise')
        for name in files:
            self.assertEqual(FAMILIES[(name, '', 'run')], (1, ['guarded_increment']))
        # The functions' own numeric requires stays a stated premise.
        self.assertEqual(premise('admitted_cursor')['status'], 'stated-not-proved')
        self.assertEqual(premise('staged_collection_extent')['status'], 'stated-not-proved')
        for name, old, new in [
                (files[0], 'while state.get() < end {', 'while state.get() <= end {'),
                (files[0], 'let index = state.get();', 'let index = state.get(); let index = end;'),
                (files[1], 'if len >= limit {', 'if len > limit {'),
                (files[1], 'let next = wrapping_add(len, 1);', 'let len = limit; let next = wrapping_add(len, 1);')]:
            self.assertIn(old, texts[name])
            with self.assertRaises(AssertionError):
                guarded_stores({**texts, name: texts[name].replace(old, new)})

    def test_disposition_requires_a_verified_contract_and_an_established_premise(self):
        from premise_status import PROVED, STATED_NOT_PROVED, UNPROVED, disposition
        verified = {'c_keep_next', 'c_keep_live', 'resized_owner', 'direct:bplus-partial', 'lookup_path_read', 'unknown_contract'}
        self.assertEqual(disposition('c_keep_next', verified), PROVED)
        # A second, unverified caller keeps a verified-caller premise stated.
        self.assertEqual(disposition('c_keep_live', verified), STATED_NOT_PROVED)
        self.assertEqual(disposition('direct:bplus-partial', verified), PROVED)
        self.assertEqual(disposition('resized_owner', verified), STATED_NOT_PROVED)
        # A projection-only backing is never reported as connected.
        self.assertEqual(disposition('lookup_path_read', verified), STATED_NOT_PROVED)
        self.assertEqual(disposition('btree_cursor', verified), UNPROVED)
        with self.assertRaisesRegex(AssertionError, 'contract has no recorded premise status'):
            disposition('unknown_contract', verified)

    def test_every_modular_family_contract_has_a_premise_status(self):
        from premise_status import premise
        from verify_wrapping_memory import FAMILIES
        for _, contracts in FAMILIES.values():
            for contract in contracts:
                self.assertIn(premise(contract)['status'], ('established', 'stated-not-proved'))

    def test_modular_ledger_reports_each_call_premise(self):
        from verify_wrapping_memory import FAMILIES, coverage
        census = [dict(file=file, context=context, function=function, kind='modular_call', expression='')
                  for (file, context, function), (count, _) in FAMILIES.items() for _ in range(count)]
        premises = {(row['site']['file'], row['site']['function']): row['premise'] for row in coverage(census)}
        self.assertEqual(premises[('src/compaction.rs', 'keep_live')], 'stated-not-proved')
        self.assertEqual(premises[('src/collections/btree_map/view/bplus/partial.rs', 'pass_over')], 'established')
        self.assertEqual(premises[('src/vec/edit.rs', 'restore_tail')], 'stated-not-proved')
        # One stated contract makes the whole call stated.
        self.assertEqual(premises[('src/span.rs', 'next')], 'stated-not-proved')

    def test_a_verified_function_does_not_discharge_its_own_requires(self):
        from premise_status import premise, row_premise
        # Both stores follow a guard inside a directly verified function, but
        # that function's numeric requires is met only by unverified callers.
        for contract, function, requires in [
                ('admitted_cursor', 'init::driver::run', '`old(state)@ <= end`'),
                ('staged_collection_extent', 'vec::staging_driver::run', '`initial_cap <= limit`')]:
            entry = premise(contract)
            self.assertEqual(entry['status'], 'stated-not-proved')
            for text in (function, requires, 'not verified'):
                self.assertIn(text, entry['evidence'])
            self.assertEqual(row_premise([contract]), 'stated-not-proved')
        for file, function, target in [('src/init/driver.rs', 'run', 'init-prefix'),
                                       ('src/vec/staging_driver.rs', 'run', 'vec-staging')]:
            source = (ROOT / file).read_text()
            head = source.index('pub(crate) fn ' + function + '<')
            spec = source[source.rfind('verus_spec(', 0, head):head]
            # The requires the evidence names is the one on the product function.
            self.assertIn('requires', spec)
            from verify_direct_verus import TARGETS
            self.assertIn(target, TARGETS)

    def test_certificate_rows_have_no_fixed_disposition(self):
        from source_certificate import BPLUS_ANCESTOR, obligation_counts, obligations
        from premise_status import PROVED, STATED_NOT_PROVED, UNPROVED
        site = dict(file='src/x.rs', function='f')
        owner = dict(source_bindings=dict(bindings=[
            dict(site=site, contract='c_keep_next', role='state-publication'),
            dict(site=site, contract='resized_owner', role='state-publication'),
            dict(site=site, contract='kani:within_capacity_push_preserves_every_numeric_owner', role='state-publication')]))
        string = dict(bindings=[dict(site=site, contract='UTF-8-owner-transfer')])
        area = dict(bindings=[dict(site=site, contract='btree_cursor', role='cursor')])
        lookup = (ROOT / BPLUS_ANCESTOR['file']).read_text()
        verified = {'c_keep_next', 'resized_owner', 'btree_cursor', 'lookup_path_read'}
        rows = obligations(owner, string, (area,), verified, lookup)
        self.assertEqual([r['disposition'] for r in rows],
                         [PROVED, STATED_NOT_PROVED, UNPROVED, UNPROVED, STATED_NOT_PROVED, STATED_NOT_PROVED])
        self.assertEqual(rows[-1]['contract'], 'lookup_path_read')
        from premise_status import INSTANTIATED, TRUSTED
        self.assertEqual(obligation_counts(rows), {PROVED: 1, INSTANTIATED: 0, 'instantiated-kani+local': 0, TRUSTED: 0, STATED_NOT_PROVED: 3, UNPROVED: 2})
        # Without a verified lemma the B+ ancestor row is unproved, not connected.
        self.assertEqual(obligations(owner, string, (area,), verified - {'lookup_path_read'}, lookup)[-1]['disposition'], UNPROVED)
        with self.assertRaisesRegex(AssertionError, 'B\\+ ancestor read moved'):
            obligations(owner, string, (area,), verified, lookup.replace(BPLUS_ANCESTOR['expression'], 'ancestor.read()'))

    def test_instantiated_kani_needs_a_closed_owner_and_the_exact_invariant(self):
        import shutil
        from premise_status import (INSTANTIATED, INSTANTIATED_KANI, STATED_NOT_PROVED, TRUSTED, TRUST_BOUNDARY,
                                    check_instantiated, disposition, premise)
        from source_certificate import BPLUS_ANCESTOR, obligation_counts, obligations
        closed = {'Buffer': dict(closed=True), 'Vec': dict(closed=True), 'BinaryHeap': dict(closed=True)}
        vec = {'checked_owner_length', 'split_owner', 'shortened_owner', 'owner_to_array',
               'memory_reserve', 'insertion_before_moves', 'internal_copy_before_append', 'drain_constructor'}
        self.assertEqual(set(INSTANTIATED_KANI), {'buffer_reserve', 'buffer_push', 'buffer_finish', 'buffer_collect'} | vec)
        self.assertEqual({c for c, r in INSTANTIATED_KANI.items() if r.get('form') == 'expanded'},
                         {'memory_reserve', 'insertion_before_moves', 'internal_copy_before_append', 'drain_constructor'})
        self.assertEqual(TRUST_BOUNDARY, {})
        verified = set(INSTANTIATED_KANI) | {'buffer_span', 'reservation_before_bulk_write', 'resized_owner'}
        for contract in INSTANTIATED_KANI:
            self.assertEqual(premise(contract)['status'], INSTANTIATED)
            self.assertEqual(disposition(contract, verified, closed), INSTANTIATED)
            owner = INSTANTIATED_KANI[contract]['owner']
            for closure in (None, {**closed, owner: dict(closed=False)}, {}):
                with self.assertRaisesRegex(AssertionError, 'owner is not closed'):
                    disposition(contract, verified, closure)
        # Premises that are not exactly an owner invariant stay stated.
        stays = ('buffer_span', 'reservation_before_bulk_write', 'resized_owner', 'commit_reserved_owner',
                 'append_owner', 'exchanged_owners', 'raw_parts_owner', 'clone_within_operation',
                 'owned_resize_operation', 'initialized_operation', 'complete_splice')
        verified |= set(stays)
        for contract in stays:
            self.assertEqual(disposition(contract, verified, closed), STATED_NOT_PROVED)
        # 12 instantiated-kani contracts and 11 instantiated-kani+local heap contracts.
        self.assertEqual(len(check_instantiated(ROOT, closed)), 23)
        with self.assertRaisesRegex(AssertionError, 'owner is not closed'):
            check_instantiated(ROOT, {'Buffer': dict(closed=False), 'Vec': dict(closed=True)})
        with self.assertRaisesRegex(AssertionError, 'owner is not closed'):
            check_instantiated(ROOT, {'Buffer': dict(closed=True), 'Vec': dict(closed=False)})
        vsite = dict(file='src/vec/edit.rs', function='try_drain', context='Vec < T , P , L >')
        check_instantiated(ROOT, closed, [(vsite, 'drain_constructor')])
        for other in (dict(vsite, file='src/clone.rs'), dict(vsite, context='Vec < u8 , P , L >')):
            with self.assertRaisesRegex(AssertionError, 'outside a function of the owner'):
                check_instantiated(ROOT, closed, [(other, 'drain_constructor')])
        site = dict(file='src/boxed/buffer.rs', function='push', context='Buffer < T , P >')
        check_instantiated(ROOT, closed, [(site, 'buffer_push')])
        for other in (dict(site, file='src/vec.rs'), dict(site, context='Buffer < u8 , P >')):
            with self.assertRaisesRegex(AssertionError, 'outside a function of the owner'):
                check_instantiated(ROOT, closed, [(other, 'buffer_push')])
        with tempfile.TemporaryDirectory() as tmp:
            shutil.copytree(ROOT / 'tools/memory-proof', Path(tmp) / 'tools/memory-proof')
            shutil.copytree(ROOT / 'src', Path(tmp) / 'src')
            path = Path(tmp) / 'tools/memory-proof/operation-closure.verus.rs'
            text = path.read_text()
            old = 'fn buffer_finish(len:usize,cap:usize,size:usize,accepted:bool)->(r:Option<OwnerState>)\n    requires len<=cap,byte_valid(cap,size),'
            self.assertEqual(text.count(old), 1)
            path.write_text(text.replace(old, old + 'len>0,'))
            with self.assertRaisesRegex(AssertionError, 'not exactly the owner invariant'):
                check_instantiated(tmp, closed)
            path.write_text(text)
            # The second accepted text (the expanded Vec invariant) is exact too:
            # a weaker or a stronger expanded requires is refused.
            path = Path(tmp) / 'tools/memory-proof/wrapping-memory.verus.rs'
            text = path.read_text()
            old = 'fn memory_reserve(len: usize, cap: usize, limit: usize'
            self.assertEqual(text.count(old), 1)
            head = text.index(old)
            clause = text.index('requires', head)
            line_end = text.index('\n', clause)
            for changed in ('requires len <= cap, byte_valid(cap, size),', 'requires len <= cap <= limit, byte_valid(cap, size), len > 0,'):
                path.write_text(text[:clause] + changed + text[line_end:])
                with self.assertRaisesRegex(AssertionError, 'not exactly the owner invariant'):
                    check_instantiated(tmp, closed)
            path.write_text(text)
            check_instantiated(tmp, closed)
        owner = dict(source_bindings=dict(bindings=[
            dict(site=site, contract='buffer_push', role='state-publication'),
            dict(site=site, contract='buffer_span', role='state-lend')]))
        lookup = (ROOT / BPLUS_ANCESTOR['file']).read_text()
        import local_acceptance
        verified |= set(local_acceptance.CONTRACTS) | set(local_acceptance.ENTRIES)
        rows = obligations(owner, dict(bindings=[]), (), verified | {'lookup_path_read'}, lookup, closed)
        self.assertEqual(obligation_counts(rows)[INSTANTIATED], 1)
        self.assertEqual(obligation_counts(rows)[TRUSTED], 0)
        with self.assertRaisesRegex(AssertionError, 'owner is not closed'):
            obligations(owner, dict(bindings=[]), (), verified | {'lookup_path_read'}, lookup, {'Buffer': dict(closed=False)})

    def test_heap_contracts_take_only_the_local_rule(self):
        # Issue #16, unit 7: no heap caller is accepted as plain instantiated-kani
        # (its requires is not exactly the Vec invariant); 11 are
        # instantiated-kani+local (rule A), two stay stated.
        import premise_status
        from premise_status import INSTANTIATED_LOCAL, STATED_NOT_PROVED, check_instantiated, disposition
        import local_acceptance
        heap = list(local_acceptance.verify_binary_heap_contracts())
        closed = {'Buffer': dict(closed=True), 'Vec': dict(closed=True), 'BinaryHeap': dict(closed=True)}
        self.assertEqual(set(local_acceptance.LEFT), {'heap_peek_caller', 'heap_restore_caller'})
        for contract in heap:
            expected = STATED_NOT_PROVED if contract in local_acceptance.LEFT else INSTANTIATED_LOCAL
            self.assertEqual(disposition(contract, set(heap), closed), expected)
            if expected == INSTANTIATED_LOCAL:
                for open_owner in ('Vec', 'BinaryHeap'):
                    with self.assertRaisesRegex(AssertionError, 'owner is not closed'):
                        disposition(contract, set(heap), {**closed, open_owner: dict(closed=False)})
            for form in ('requires', 'expanded'):
                row = dict(owner='Vec', maintained='tools/memory-proof/binary-heap.verus.rs', form=form,
                           words=dict(s='s', len='len', cap='cap', limit='limit'), call_requires=())
                with patch.dict(premise_status.INSTANTIATED_KANI, {contract: row}):
                    with self.assertRaisesRegex(AssertionError, 'not exactly the owner invariant'):
                        check_instantiated(ROOT, closed)
        site = dict(file='src/collections/binary_heap.rs', function='pop', context='BinaryHeap < T , P , L >')
        with self.assertRaisesRegex(AssertionError, 'outside a function of the owner'):
            check_instantiated(ROOT, closed, [(site, 'shortened_owner')])

    def test_local_rule_refuses_each_unsound_case(self):
        # Rule A (approved 2026-11-28): the controls the owner asked for.
        import local_acceptance as la
        import heap_callers
        closed = {'Buffer': dict(closed=True), 'Vec': dict(closed=True), 'BinaryHeap': dict(closed=True)}
        self.assertEqual(set(la.check(ROOT, closed)), set(la.CONTRACTS))

        def copy(tmp):
            shutil.copytree(ROOT / 'tools/memory-proof', Path(tmp) / 'tools/memory-proof')
            shutil.copytree(ROOT / 'src', Path(tmp) / 'src')
            return Path(tmp)

        def edit(root, file, old, new):
            path = root / file
            text = path.read_text()
            self.assertEqual(text.count(old), 1, (file, old))
            path.write_text(text.replace(old, new))
        # (1) An extra, undischarged conjunct in an entry caller's requires.
        with tempfile.TemporaryDirectory() as tmp:
            root = copy(tmp)
            edit(root, la.MODEL, 'paired_storage(s,cap,size),len>0,\n{', 'paired_storage(s,cap,size),len>0,len>1,\n{')
            with self.assertRaisesRegex(AssertionError, 'undischarged or missing local fact'):
                la.check(root, closed)
        # ... and a caller-discharged contract whose requires gains a conjunct
        # that no accepted caller establishes: the verified set refuses it.
        with self.assertRaisesRegex(AssertionError, 'not verified'):
            la.check(ROOT, closed, verified=set(la.CONTRACTS) - {'heap_sift_up_caller'} | set(la.ENTRIES))
        # (2) A discharging harness removed from its group, its stub weakened,
        # its stub attribute removed, or a discharging caller that no longer
        # reaches the contract.
        import owner_closure
        groups = owner_closure.harness_groups()
        groups.pop(la.H + 'push_establishes_its_local_fact')
        with self.assertRaisesRegex(AssertionError, 'not in a group'):
            la.check(ROOT, closed, groups=groups)
        for old, new, message in [('        assert!(data.len() > 0);\n    }', '        assert!(true);\n    }', 'precondition stub changed'),
                                  ('    #[kani::stub(crate::collections::binary_heap::core::sift_up, sift_up_precondition)]\n', '',
                                   'does not stub the row callee'),
                                  ('        let result = heap.try_push(());', '        let result: Result<(), ()> = Ok(());',
                                   'does not run the product function'),
                                  ('        let mut heap = closure_heap(Unmetered, limit);\n        let result = heap.try_push(());',
                                   '        let mut heap = closure_heap(Unmetered, limit);\n        kani::assume(heap.len() > 0);\n        let result = heap.try_push(());',
                                   'not admissible')]:
            with self.subTest(message=message), tempfile.TemporaryDirectory() as tmp:
                root = copy(tmp)
                edit(root, la.PROOF, old, new)
                with self.assertRaisesRegex(AssertionError, message):
                    la.check(root, closed)
        weakened = dict(heap_callers.CALLERS, heap_push_caller=['crate::arithmetic::bounded::wrapping_sub(len,1)'])
        with patch.object(heap_callers, 'CALLERS', weakened), patch('verify_binary_heap.CALLERS', weakened):
            with self.assertRaisesRegex(AssertionError, 'does not reach the contract'):
                la.check(ROOT, closed)
        # (3) The owner or the wrapper not closed.
        for open_owner in ('Vec', 'BinaryHeap'):
            with self.assertRaisesRegex(AssertionError, 'owner is not closed'):
                la.check(ROOT, {**closed, open_owner: dict(closed=False)})
        # (4) A row outside the pinned functions, or a block that does not call
        # the product function (the PeekMut::pop set_len block).
        good = dict(file='src/collections/binary_heap/core.rs', function='pop_nonempty', context='', expression='unsafe { x }')
        la.check(ROOT, closed, [(good, 'heap_pop_caller')])
        for site in (dict(good, function='pop_unrelated'), dict(good, file='src/vec.rs'),
                     dict(file='src/collections/binary_heap.rs', function='pop', context="PeekMut < '_ , T , P , L >",
                          expression='unsafe { core :: set_len (& mut this . heap . data , len . get ()) }')):
            with self.assertRaisesRegex(AssertionError, 'outside its pinned functions'):
                la.check(ROOT, closed, [(site, 'heap_pop_caller')])
        # The pinned call table refuses a new product call of a kernel.
        with tempfile.TemporaryDirectory() as tmp:
            root = copy(tmp)
            edit(root, 'src/collections/binary_heap.rs', '    pub fn clear(&mut self) {\n        self.data.clear()',
                 '    pub fn clear(&mut self) {\n        core::rebuild(self.data.as_mut_slice());\n        self.data.clear()')
            with self.assertRaisesRegex(AssertionError, 'heap files changed|call table changed'):
                la.check(root, closed)

class OwnerWordTests(unittest.TestCase):
    @staticmethod
    def borrow(file, function, expression, kind='field_borrow'):
        members = expression.replace('&mut', '').split('.')[1:]
        return dict(file=file, function=function, context='', kind=kind, expression=expression,
                    detail=dict(member=members[-1], members=members, raw=False))

    def product(self):
        from caller_bindings import LENDS
        return [self.borrow(file, function, expression)
                for (file, function), (_, expressions) in LENDS.items() for expression in expressions]

    def test_every_owner_word_borrow_is_bound_and_stays_stated(self):
        from caller_bindings import lend_bindings
        from premise_status import premise
        bindings, other = lend_bindings(self.product(), lambda name: name)
        self.assertEqual(len(bindings), 10)
        self.assertEqual({b['role'] for b in bindings}, {'state-lend'})
        self.assertEqual({b['contract'] for b in bindings}, {'buffer_span', 'splice_fill_loop', 'exchanged_owners'})
        for binding in bindings:
            self.assertEqual(premise(binding['contract'])['status'], 'stated-not-proved')
        self.assertEqual(other, {})

    def test_an_unbound_owner_word_channel_is_refused(self):
        from caller_bindings import lend_bindings
        proof = lambda name: name
        rows = self.product()
        for extra, message in [
                # A new lend in a function without a binding, and a second one in a bound function.
                (self.borrow('src/vec.rs', 'clear', '&mutself.len'), 'unbound owner-word borrow'),
                (self.borrow('src/boxed/buffer.rs', 'collect', '&mutself.cap'), 'unbound owner-word borrow'),
                # A borrow of a part of an owner word is a borrow of the word.
                (self.borrow('src/vec.rs', 'clear', '&mutself.len.0'), 'unbound owner-word borrow'),
                # No area filter may drop the row.
                (self.borrow('src/deque.rs', 'clear', '&mutself.len'), 'owner-word borrow in a separately bound area'),
                (self.borrow('src/string.rs', 'clear', '&mutself.bytes.len'), 'owner-word borrow in a separately bound area'),
                (self.borrow('src/vec.rs', 'clear', '&mutself.len', kind='field_pattern'), 'unbound owner-word pattern binding'),
        ]:
            with self.assertRaisesRegex(AssertionError, message):
                lend_bindings(rows + [extra], proof)
        with self.assertRaisesRegex(AssertionError, 'missing expected owner-word borrow'):
            lend_bindings(rows[1:], proof)
        with self.assertRaisesRegex(AssertionError, 'unbound owner-word borrow'):
            lend_bindings(list(reversed(rows)), proof)

    def test_lend_harnesses_repeat_the_pinned_product_statements(self):
        from caller_bindings import LEND_HARNESSES, LENDS, lend_harness_texts
        rows = lend_harness_texts(ROOT)
        self.assertEqual([(r['file'], r['function']) for r in rows],
                         [('src/boxed/buffer.rs', 'collect'), ('src/boxed/buffer.rs', 'collect_rest')])
        self.assertTrue(set(LEND_HARNESSES) <= set(LENDS))
        files = ('src/boxed/buffer.rs', 'src/boxed/buffer_proofs.rs')
        for file, old, new, message in [
                # The product lend changes its end, its pointer or its order.
                (files[0], 'fill(self.ptr, &mut self.len, reserved_end, &mut iter)',
                 'fill(self.ptr, &mut self.len, reserved_end.wrapping_add(0), &mut iter)', 'exactly once'),
                (files[0], 'fill(self.ptr, &mut self.len, self.cap.get(), &mut iter)',
                 'fill(self.ptr, &mut self.len, self.len.get(), &mut iter)', 'exactly once'),
                (files[0], 'let reserved_end = self.reserve(lower, lower)', 'let reserved_end = self.reserve(lower, 0)',
                 'exactly once'),
                # The harness lends something else, or lends twice.
                (files[1], 'fill(owner.ptr, &mut owner.len, owner.cap.get(), &mut iter)',
                 'fill(owner.ptr, &mut owner.len, owner.cap.get().wrapping_sub(0), &mut iter)', 'exactly once'),
                (files[1], 'let result = unsafe {\n        crate::init::collection::fill(owner.ptr, &mut owner.len, reserved_end, &mut iter)\n    };',
                 'let result = unsafe {\n        crate::init::collection::fill(owner.ptr, &mut owner.len, reserved_end, &mut iter);\n'
                 '        crate::init::collection::fill(owner.ptr, &mut owner.len, reserved_end, &mut iter)\n    };',
                 'exactly once'),
                # The reservation moves after the lend in the harness.
                (files[1], '    let Ok(reserved_end) = owner.reserve(lower, lower) else {',
                 '    let _ = crate::init::collection::fill(owner.ptr, &mut owner.len, reserved_end, &mut iter);\n'
                 '    let Ok(reserved_end) = owner.reserve(lower, lower) else {', 'exactly once')]:
            with tempfile.TemporaryDirectory() as tmp:
                for name in files:
                    (Path(tmp) / name).parent.mkdir(parents=True, exist_ok=True)
                    (Path(tmp) / name).write_text((ROOT / name).read_text())
                path = Path(tmp) / file
                text = path.read_text()
                self.assertEqual(text.count(old), 1, old)
                path.write_text(text.replace(old, new))
                with self.assertRaisesRegex(AssertionError, message):
                    lend_harness_texts(tmp)

    def test_owner_closure_of_the_product(self):
        import owner_closure
        result = owner_closure.report(ROOT, ROOT / 'target/verification/test-owner-closure')
        self.assertTrue(result['Buffer']['closed'])
        self.assertEqual(result['Buffer']['counts'], {'harness': 8})
        # Issue #16, unit 4: Vec and VecDeque close together (the joint closure).
        self.assertTrue(result['Vec']['closed'])
        self.assertTrue(result['VecDeque']['closed'])
        self.assertEqual((result['Vec']['depends_on'], result['VecDeque']['depends_on']), (['VecDeque'], ['Vec']))
        self.assertEqual(result['VecDeque']['counts'], {'harness': 42, 'unobservable': 2})
        self.assertEqual(sum(bool(s.get('dead')) for s in result['VecDeque']['sites']), 15)
        calls = {s['function']: s for s in result['Vec']['sites'] if s['kind'] == 'call' and s['file'] == 'src/deque.rs'}
        self.assertEqual(sorted(calls), ['into_vec', 'try_from_std_in'])
        self.assertEqual(calls['try_from_std_in']['disposition'], 'harness')
        self.assertIn('from_vec', calls['try_from_std_in']['observed'])
        finish = [s for s in result['Vec']['sites'] if s['function'] == 'try_finish']
        self.assertEqual(len(finish), 1)
        self.assertEqual(finish[0]['path_assumption']['assumption'], 'tail_len != 0')
        # Issue #16, unit 7: BinaryHeap is a wrapper of one Vec word, closed with Vec.
        heap = result['BinaryHeap']
        self.assertTrue(heap['closed'])
        self.assertEqual((heap['wrapper_of'], heap['depends_on']), ('Vec', ['Vec']))
        self.assertEqual(heap['counts'], {'harness': 12, 'inner-closure': 4})
        self.assertEqual(sorted({s['kind'] for s in heap['sites']}), ['constructor', 'lend', 'swap'])

    def test_owner_closure_refuses_stale_entries_and_extra_assumptions_elsewhere(self):
        import owner_closure
        out = ROOT / 'target/verification/test-owner-closure'
        push = ('Buffer', 'write', 'src/boxed/buffer.rs', 'Buffer', 'push', 'self.len=Checked::new(needed)', 1)
        pop = ('Vec', 'write', 'src/vec.rs', 'Vec', 'pop', 'self.len=next', 1)
        table = dict(owner_closure.SITES)
        table[push] = owner_closure.h(owner_closure.B + 'collect_preserves_the_invariant_for_every_unit_owner_and_two_value_source')
        table[pop] = owner_closure.h(owner_closure.E + 'splice_finish_with_a_tail_preserves_the_invariant',
                                     path_assumption='tail_len != 0')
        result = owner_closure.report(ROOT, out, table=table)
        rows = {(s['function'], s['expression']): s for o in result.values() for s in o['sites']}
        # An extra-assumption harness is evidence for no other site, and a path
        # assumption is accepted only for the store it is checked for.
        self.assertEqual(rows[('push', 'self.len=Checked::new(needed)')]['disposition'], 'uncovered')
        self.assertFalse(result['Buffer']['closed'])
        self.assertEqual(rows[('pop', 'self.len=next')]['disposition'], 'uncovered')
        sites = owner_closure.SITES
        for change, message in [
                ({**sites, ('Vec', 'write', 'src/vec.rs', 'Vec', 'clear', 'self.len=next', 1):
                  owner_closure.h(owner_closure.M + 'pop_preserves_the_invariant')}, 'entry without a site'),
                ({k: v for k, v in sites.items() if k != pop}, 'site without an entry'),
                ({**sites, pop: owner_closure.d('trust-boundary', 'x', item='public-unsafe-caller')},
                 'trust-boundary only for a constructor'),
                ({**sites, pop: owner_closure.h('vec::metadata_proofs::not_a_harness')}, 'not in a group')]:
            with self.assertRaisesRegex(AssertionError, message):
                owner_closure.report(ROOT, out, table=change)

    def test_owner_closure_checks_the_deque_pins(self):
        import owner_closure
        site = lambda file, line, function, expression: dict(file=file, line=line, function=function,
                                                           expression=expression, key=(file, function, expression))
        files = ('src/deque.rs', 'src/deque/iter.rs', 'src/vec/ownership.rs', 'src/vec.rs')
        for file, old, new, check, message in [
                # A dead store whose guard is dropped, or a return that no longer precedes it.
                ('src/deque.rs', '        if mem::size_of::<T>() != 0 {\n            self.head = Checked::new(geometry::retreat(',
                 '        if true {\n            self.head = Checked::new(geometry::retreat(',
                 ('dead', 'try_push_front', 'self.head=Checked::new(geometry::retreat(self.head.get(),1,self.capacity()))',
                  owner_closure.ZST_GUARD), 'not inside its guard'),
                ('src/deque.rs', '        if mem::size_of::<T>() == 0 || new >= self.capacity() {',
                 '        if new >= self.capacity() {',
                 ('dead', 'try_shrink_to', 'self.cap=cap', owner_closure.ZST_RETURN), 'no early return'),
                # IntoIter::fold uses self after the guard.
                ('src/deque/iter.rs', '        b.iter().fold(value, consume)\n    }',
                 '        let _ = self.len();\n        b.iter().fold(value, consume)\n    }', ('fold',), 'shape changed'),
                # from_vec no longer copies the words verbatim.
                ('src/deque.rs', '            len: Checked::new(len),\n            head: Checked::new(0),',
                 '            len: Checked::new(len.min(1)),\n            head: Checked::new(0),',
                 ('observed',), 'pinned body changed'),
                # A Vec getter that closure_vec_owner reads changes.
                ('src/vec.rs', '    pub fn capacity(&self) -> usize {\n        self.cap.get()',
                 '    pub fn capacity(&self) -> usize {\n        self.len.get()', ('getters',), 'Vec getter')]:
            with tempfile.TemporaryDirectory() as tmp:
                shutil.copytree(ROOT / 'src', Path(tmp) / 'src')
                path = Path(tmp) / file
                text = path.read_text()
                self.assertEqual(text.count(old), 1, old)
                path.write_text(text.replace(old, new))
                lines = (ROOT / file).read_text().split('\n')
                with self.assertRaisesRegex(AssertionError, message):
                    if check[0] == 'dead':
                        start = next(i for i, l in enumerate(lines) if 'fn ' + check[1] + '(' in l)
                        line = next(i + 1 for i, l in enumerate(lines) if i > start and compact_text(l).startswith(check[2][:12]))
                        owner_closure.check_dead(tmp, site(file, line, check[1], check[2]), check[3])
                    elif check[0] == 'fold':
                        owner_closure.check_retire_fold(tmp)
                    elif check[0] == 'observed':
                        line = next(i + 1 for i, l in enumerate(lines) if 'pub fn try_from_std_in(' in l) + 5
                        owner_closure.check_observed(tmp, site(file, line, 'try_from_std_in', 'Vec::from_raw_parts'), 'from_vec')
                    else:
                        owner_closure.check_fixtures(tmp)

    def test_owner_closure_is_a_joint_fixpoint(self):
        # An other-unit entry for the into_vec call, or the retain store
        # without a harness, opens both owners.
        import owner_closure
        out = ROOT / 'target/verification/test-owner-closure'
        table = dict(owner_closure.SITES)
        result = owner_closure.report(ROOT, out, table={**table, owner_closure.RETAIN_SITE: owner_closure.d('uncovered', 'x')})
        self.assertFalse(result['Vec']['closed'] or result['VecDeque']['closed'])
        table[('Vec', 'call', 'src/deque.rs', 'VecDeque', 'into_vec', 'Vec::from_raw_parts', 1)] = owner_closure.d('other-unit:4', 'x')
        result = owner_closure.report(ROOT, out, table=table)
        self.assertFalse(result['Vec']['closed'] or result['VecDeque']['closed'])

    def test_owner_closure_defect_controls_are_detected(self):
        import owner_closure
        results = owner_closure.controls(ROOT, ROOT / 'target/verification/test-owner-closure')
        self.assertEqual([r['name'] for r in results],
                         ['unmapped-write', 'harness-assumption', 'harness-out-of-group', 'other-unit-site',
                          'deque-unmapped-write', 'retain-harness-assumption', 'into-vec-harness-assumption',
                          'from-vec-harness-assumption', 'retain-uncovered', 'heap-inner-not-closed',
                          'heap-harness-assumption', 'heap-unmapped-lend', 'heap-second-field', 'heap-field-write',
                          'heap-helper-changed'])
        self.assertTrue(all(r['detected'] for r in results), results)

    def test_borrows_of_other_fields_are_counted_not_bound(self):
        from caller_bindings import lend_bindings
        rows = self.product() + [self.borrow('src/vec/edit.rs', 'try_retain_mut', '&mutg.state'),
                                 self.borrow('src/deque/iter.rs', 'next', '&mutself.state'),
                                 self.borrow('src/deque/iter.rs', 'next', '&mutself.inner', kind='field_pattern')]
        bindings, other = lend_bindings(rows, lambda name: name)
        self.assertEqual(len(bindings), 10)
        self.assertEqual(other, {'deque': 2, 'foundation': 1})

    def test_scanner_records_borrows_and_pattern_bindings(self):
        import json, subprocess, tempfile
        from scanner_tools import scanner
        source = '''struct V { len: usize, cap: usize, items: [u8; 2] }
impl V {
    fn f(&mut self, other: &mut V) {
        core::mem::swap(&mut self.len, &mut other.len);
        let _ = core::mem::replace(&mut (self.cap), 3);
        let _ = core::mem::take(&mut (*other).len);
        let _ = &raw mut self.len;
        let _ = &mut self.len.0;
        let _ = &self.len;
        let _ = &raw const self.cap;
        let _ = &mut self.items[0];
        let V { len, cap: ref mut c, .. } = self;
        let V { len: _, .. } = other;
    }
}
'''
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'sample.rs'
            path.write_text(source)
            rows = json.loads(subprocess.check_output([str(scanner('state_sites')), 'sample.rs'], cwd=tmp, text=True))
        compact = lambda text: ''.join(text.split())
        borrows = [(r['line'], compact(r['expression']), r['detail']['members'], r['detail']['raw'])
                   for r in rows if r['kind'] == 'field_borrow']
        self.assertEqual(borrows, [
            (4, '&mutself.len', ['len'], False), (4, '&mutother.len', ['len'], False),
            (5, '&mut(self.cap)', ['cap'], False), (6, '&mut(*other).len', ['len'], False),
            (7, '&rawmutself.len', ['len'], True), (8, '&mutself.len.0', ['len', '0'], False)])
        patterns = [(r['line'], r['detail']['structure'], r['detail']['member'], r['detail']['ref_mut'])
                    for r in rows if r['kind'] == 'field_pattern']
        self.assertEqual(patterns, [(12, 'V', 'len', False), (12, 'V', 'cap', True)])


def load_verifast_check():
    import importlib.util
    spec=importlib.util.spec_from_file_location('verifast_check',ROOT/'tools/verifast/check.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module


class VeriFastTests(unittest.TestCase):
    """Manifest, identity and comment-strip logic of tools/verifast/check.py (no VeriFast run)."""
    def setUp(self):
        self.vf=load_verifast_check()

    def fixture(self,tmp,annotated,product='pub fn f(x: usize) -> usize {\n    x\n}\n// tail\nfn g() {}\n',cut=None):
        import hashlib
        tmp=Path(tmp);(tmp/'src').mkdir();(tmp/'proof'/'verified').mkdir(parents=True)
        (tmp/'src'/'m.rs').write_text(product);(tmp/'proof'/'lib.rs').write_text('mod m;\n')
        (tmp/'proof'/'verified'/'m.rs').write_text(annotated)
        data=product.encode();original={'kind':'product'}
        if cut:
            lines=data.splitlines(keepends=True)
            original={'kind':'product-cut','keep_lines':[1,cut],'dropped_sha256':hashlib.sha256(b''.join(lines[cut:])).hexdigest()}
        manifest={'root':{'path':'lib.rs','stored':'lib.rs'},'modules':[{'path':'m.rs','product':'src/m.rs',
            'product_sha256':hashlib.sha256(data).hexdigest(),'original':original,
            'verified':{'kind':'annotations-only','stored':'verified/m.rs'}}]}
        return tmp,manifest

    def test_identity_and_strip_accept_annotations_only(self):
        annotated='pub fn f(x: usize) -> usize \n//@ req x < 10;\n/*@ ens result == x; @*/\n{\n    x\n}\n// tail\nfn g() {}\n'
        with tempfile.TemporaryDirectory() as tmp:
            tmp,manifest=self.fixture(tmp,annotated)
            sides=self.vf.assemble(tmp/'out',manifest,product_root=tmp,proof=tmp/'proof')
            self.assertEqual(sides['original']['m.rs'],(tmp/'src'/'m.rs').read_bytes())
            self.assertEqual(self.vf.check_identity(tmp/'out',manifest,product_root=tmp),[])
            self.assertTrue(self.vf.check_strip(manifest,sides)['m.rs']['passed'])

    def test_changed_product_fails_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp,manifest=self.fixture(tmp,'pub fn f(x: usize) -> usize {\n    x\n}\n// tail\nfn g() {}\n')
            (tmp/'src'/'m.rs').write_text('pub fn f(x: usize) -> usize {\n    x + 0\n}\n// tail\nfn g() {}\n')
            self.vf.assemble(tmp/'out',manifest,product_root=tmp,proof=tmp/'proof')
            errors=self.vf.check_identity(tmp/'out',manifest,product_root=tmp)
            self.assertEqual(len(errors),1);self.assertIn('changed',errors[0])

    def test_cut_keeps_a_prefix_and_pins_the_dropped_part(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp,manifest=self.fixture(tmp,'pub fn f(x: usize) -> usize {\n    x\n}\n',cut=3)
            sides=self.vf.assemble(tmp/'out',manifest,product_root=tmp,proof=tmp/'proof')
            self.assertEqual(sides['original']['m.rs'],b'pub fn f(x: usize) -> usize {\n    x\n}\n')
            self.assertTrue(self.vf.check_strip(manifest,sides)['m.rs']['passed'])
            manifest['modules'][0]['original']['dropped_sha256']='0'*64
            with self.assertRaises(self.vf.CheckError):self.vf.original_side(manifest['modules'][0],tmp)

    def test_strip_rejects_code_changes(self):
        same=self.vf.annotation_only_equal
        original='fn f() -> u8 { let s = "a"; s.len() as u8 } // note\n'
        self.assertTrue(same('fn f() -> u8 /*@ req true; @*/ { let s = "a";\n s.len() as u8 } // note\n',original))
        for edited in ['#[inline(never)] fn f() -> u8 { let s = "a"; s.len() as u8 } // note\n',
                       'fn f() - > u8 { let s = "a"; s.len() as u8 } // note\n',
                       'fn f() -> u8 { let s = "a/*@ x @*/"; s.len() as u8 } // note\n',
                       'fn f() -> u8 { let s = "a"; s.len() as u8 } // changed note\n',
                       'fn f() -> u8 { let s = "a"; /*@ /* nested */ @*/ s.len() as u8 + 0 } // note\n']:
            self.assertFalse(same(edited,original),edited)
        # Block comments nest as in rustc: the whole nested annotation is removed, and text
        # after the point where rustc closes the comment is code.
        self.assertTrue(same('fn f() -> u8 /*@ a /* b */ c @*/ { let s = "a"; s.len() as u8 } // note\n',original))
        self.assertFalse(same('fn f() -> u8 /*@ a /* b @*/ c */ @*/ { let s = "a"; s.len() as u8 } // note\n',original))
        self.assertEqual(self.vf.strip_annotations("let c = '/'; let r = r#\"//@\"#; //@ x\n"),"let c = '/'; let r = r#\"//@\"#; \n")

    def test_function_inventory_and_trusted_assumes(self):
        text='fn a() {}\nstruct S(PhantomData<fn() -> u8>);\n// fn hidden()\nconst X: &str = "fn quoted";\nmacro_rules! m { () => { fn $name() {} } }\nfn a2<\'x>(c: char) { let _ = \'f\'; }\n'
        self.assertEqual(self.vf.fn_names(text),{'a':1,'$name':1,'a2':1})
        ghost='/*@\nlem ok()\n    req true;\n    ens true;\n{ assume(false); }\nlem other() req true; ens true; {}\n@*/\nfn f() { /*@ assume(x); @*/ }\n'
        self.assertEqual(self.vf.assumes_in_lemmas(ghost),['ok',None])

    def test_repository_manifest_is_complete(self):
        manifest=self.vf.load_manifest()
        proof=ROOT/'tools/verifast/proofs/capacity'
        kinds={m['verified']['kind'] for m in manifest['modules']}
        self.assertLessEqual(kinds,{'product','annotations-only','rewrite'})
        for module in manifest['modules']:
            self.assertTrue(module['product'].startswith('src/'))
            self.assertFalse((proof/module['path']).exists(),'no copies of product files beside the manifest')
            if module['verified']['kind']!='product':self.assertTrue((proof/module['verified']['stored']).is_file())
        classes={f['class'] for f in manifest['functions']}
        self.assertLessEqual(classes,{'verified','verified-rewrite-refined','skipped-specless','not-compiled'})
        self.assertTrue(all(('vf_name' in f)==f['class'].startswith('verified') for f in manifest['functions']))
        controls=json.loads((proof/'controls.json').read_text())
        self.assertTrue({'refinement','verifast','strip','identity'}<={c for row in controls for c in row['expect']})

    def test_verifast_check_is_local_and_outside_the_profiles(self):
        from verify import QUICK
        self.assertNotIn('verifast',FULL);self.assertNotIn('verifast',QUICK)
        job=plan('all',['verifast'],ROOT/'target/probe')[0]
        self.assertEqual(job['command'][1],'tools/verifast/check.py');self.assertTrue((ROOT/job['command'][1]).is_file())
        with self.assertRaises(ValueError):plan('btree',['verifast'],ROOT/'target/probe')


if __name__=='__main__':unittest.main()
