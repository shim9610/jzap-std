#!/usr/bin/env python3
"""Exercise selection and real process failures without invoking Rust tools."""
import contextlib,io,json,sys,tempfile,unittest
from unittest.mock import patch
from pathlib import Path
from verify import ROOT,AREAS,CHECKS,FULL,execute,plan,prepare_python_modules
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
        with patch('verus_composition.subprocess.run') as invoke:
            command,_=run('verus',ROOT,ROOT/'target/calls.rs','actual_product',{})
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

if __name__=='__main__':unittest.main()
