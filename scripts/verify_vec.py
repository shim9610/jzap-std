#!/usr/bin/env python3
"""Run maintained Vec verification; raw outputs stay outside Git."""
import argparse,json,os,pathlib,subprocess
root=pathlib.Path(__file__).resolve().parents[1];out=root/'target/verification/vec';out.mkdir(parents=True,exist_ok=True)
p=argparse.ArgumentParser();p.add_argument('--enhanced-kani',action='store_true');p.add_argument('--bench',action='store_true');args=p.parse_args();results=[]
def run(name,command,extra=None,timeout=None):
 env=os.environ.copy();env.update(extra or {})
 with (out/(name+'.log')).open('w') as log:
  try:r=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=timeout);code=r.returncode
  except subprocess.TimeoutExpired:code=124
 results.append(dict(name=name,command=command,environment=extra or {},exit_code=code));(out/'status.json').write_text(json.dumps(results,indent=2));print(name,code,flush=True)
 return code
run('api',['python3','scripts/check_vec_api.py'])
run('fmt',['cargo','fmt','--all','--check'])
run('tests',['cargo','test','--locked','--all-features'],{'PROPTEST_RNG_SEED':'123456789'})
run('checked-release-tests',['cargo','test','--locked','--release','--all-features'],{'RUSTFLAGS':'-C overflow-checks=yes','PROPTEST_RNG_SEED':'123456789'})
run('arithmetic-policy',['python3','scripts/verify_arithmetic_policy.py'])
run('panic-certificate',['python3','scripts/verify_panic_paths.py'])
run('unsafe-inventory',['python3','scripts/check_unsafe_inventory.py'])
run('thread-bounds',['python3','scripts/verify_thread_bounds.py','--mutations'])
run('memory-controls',['python3','scripts/check_memory_controls.py'])
run('consumer-link',['python3','scripts/check_consumer_link.py'])
run('core',['cargo','check','--locked','--no-default-features'])
run('rustdoc',['cargo','doc','--locked','--all-features','--no-deps'],{'RUSTDOCFLAGS':'-D warnings'})
for name,flags in [('miri',{}),('miri-tree',{'MIRIFLAGS':'-Zmiri-tree-borrows -Zmiri-strict-provenance'})]:
 run(name,['cargo','+nightly-2026-10-30','miri','test','--locked','--all-features','--test','vec','--test','iteration','--test','wrapping_memory','--test','unsafe_paths'],flags)
run('asan',['cargo','+nightly-2026-10-30','test','--locked','--all-features','--target','x86_64-unknown-linux-gnu','--test','vec','--test','wrapping_memory','--test','unsafe_paths'],{'RUSTFLAGS':'-Zsanitizer=address'})
for release in [False,True]:run('small-stack-'+str(release),['cargo','run','--locked']+(['--release'] if release else [])+['--example','vec_small_stack'])
run('abort',['cargo','run','--locked','--release','--example','vec_abort_failure'],{'RUSTFLAGS':'-C panic=abort'})
run('fuzz',['cargo','+nightly-2026-10-30','fuzz','run','vec','--','-runs=10000','-max_len=128'])
for target in ['wasm32-unknown-unknown','thumbv7em-none-eabi']:run(target,['cargo','check','--locked','--target',target])
run('no-oom',['cargo','+nightly-2026-10-30','-Z','build-std=core,alloc','check','--locked','--lib','--target','x86_64-unknown-linux-gnu'],{'RUSTFLAGS':'--cfg no_global_oom_handling'})
harnesses=['vec_required_length_checks_all_overflow_and_limit_cases','vec_growth_proposal_fits_limit_without_rejecting_fitting_length','vec_bounded_push_pop_preserves_values_and_budget','vec_insert_remove_and_iterator_transfer','vec_drain_preserves_retained_order','vec_extract_preserves_retained_order','vec_extract_long_prefix_preserves_unvisited_ownership','vec_retain_error_restores_suffix','vec_splice_checks_limit_and_keeps_tail','vec_unhinted_splice_restores_tail_and_budget']
harnesses += ['take_preserves_limit_and_propagates_every_reply','take_clamps_even_dishonest_hints_without_overflow','nth_stops_at_first_error_or_end_for_any_skip_count','fold_distinguishes_source_and_operation_failure']
for h in harnesses:
 run(h,['timeout','180','cargo','kani','--harness',h])
 if args.enhanced_kani:run(h+'-extra',['timeout','180','cargo','kani','--harness',h,'-Z','uninit-checks','-Z','valid-value-checks'])
run('direct-cursors',['python3','scripts/verify_source.py','--group','vec-cursors','--mutations'])
run('mutations',['python3','scripts/mutation_check.py'])
run('coverage',['cargo','+nightly-2026-10-30','llvm-cov','--branch','--all-features','--test','vec','--test','vec_properties','--json','--output-path',str(out/'coverage.json')])
if args.bench:
 codegen=run('codegen',['python3','scripts/analyze_vec_codegen.py'])
 timing_build=run('bench-build',['cargo','build','--locked','--manifest-path','tools/vec-analysis/Cargo.toml','--release'])
 memory_build=run('memory-build',['cargo','build','--locked','--manifest-path','tools/vec-analysis/Cargo.toml','--release','--features','memory','--target-dir','tools/vec-analysis/target/memory'])
 if not (codegen or timing_build or memory_build):run('performance',['python3','scripts/compare_vec.py','--trials','30','--ops','grow,reserved,copy,clone,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all,insert_remove,resize,split_off,append,shrink,pop','--memory-ops','grow,copy,retain,dedup,drain,extract,splice,within,splice_unknown,splice_no_hint,extract_none,extract_all,insert_remove,resize,split_off,append,shrink,pop'])
raise SystemExit(int(any(r['exit_code'] for r in results)))
