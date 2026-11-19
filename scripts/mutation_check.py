#!/usr/bin/env python3
"""Deliberately break accounting and both partial cleanup paths; require test failures."""
from arithmetic_projection import replace_once
import os,pathlib,shutil,subprocess,tempfile,json
root=pathlib.Path(__file__).resolve().parents[1]
(root/'target/verification').mkdir(parents=True,exist_ok=True)
mutations=[('budget_release','src/budget.rs','self.occupied.set(wrapping_sub(self.occupied.get(), bytes));','self.occupied.set(wrapping_add(self.occupied.get(), bytes));','interleaved_reservations_and_charges_preserve_limit'),('array_cleanup','src/init/prefix.rs','                self.state.get(),','                0,','partial_array_clone_drops_only_completed_prefix')]
mutations.append(('array_slot_cleanup','src/init/prefix.rs','                self.state.get(),','                0,','array_initialization_cleans_each_failure_position'))
results=[]
mutations=[(*m,'foundation') for m in mutations]
mutations.append(('slice_cleanup','src/init/prefix.rs','                self.state.get(),','                0,','slice_every_partial_failure_is_cleaned_and_budget_restored','box_complete'))
mutations += [
 ('vec_limit','src/vec.rs','if needed > limit {','if needed >= limit {','limit_creation_exact_boundary_growth_clamping_and_reuse','vec'),
 ('vec_growth','src/vec.rs','.min(ceiling)','.max(ceiling)','limit_creation_exact_boundary_growth_clamping_and_reuse','vec'),
 ('vec_rollback','src/vec.rs','self.owner.truncate(self.original);','self.owner.truncate(0);','clone_and_resize_partial_failure_drop_suffix_and_preserve_prefix','vec'),
]
mutations += [
 ('vec_drain_tail','src/vec/edit.rs','wrapping_add(self.owner.len.get(), self.tail_len)','wrapping_add(self.owner.len.get(), 0)','complete::drain_every_range_and_consumption_matches_std','vec'),
 ('vec_extract_stop','src/compaction.rs','self.end = self.read;','self.end = self.total;','complete::callback_errors_preserve_unvisited_suffix_and_mutations','vec'),
 ('vec_spare_limit','src/vec/convenience.rs','required_len(0, new_len, self.cap.get())?;','required_len(0, new_len, usize::MAX)?;','complete::spare_storage_checked_length_and_reference_returning_edits','vec'),
 ('try_take_attempts','src/iter.rs','self.remaining = next;','self.remaining = Checked::new(0);','take_zero_does_not_call_source_and_errors_consume_one_attempt','iteration'),
 ('try_source_error','src/vec.rs','crate::TryIterator::try_next(&mut iter).map_err(InitError::Initialize)?','crate::TryIterator::try_next(&mut iter).unwrap_or(None)','source_failure_rolls_back_suffix_and_leaves_borrowed_source_resumable','iteration'),
]
for name,file,old,new,test,test_target in mutations:
 with tempfile.TemporaryDirectory(prefix='jzap-mutation-') as tmp:
  dst=pathlib.Path(tmp)
  for d in ['src','tests','examples']:shutil.copytree(root/d,dst/d)
  for f in ['Cargo.toml','Cargo.lock']:shutil.copy2(root/f,dst/f)
  p=dst/file;s=p.read_text();assert s != replace_once(s,old,new),(name,s.count(old));p.write_text(replace_once(s,old,new))
  env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target/mutations'))
  r=subprocess.run(['cargo','test','--locked','--offline','--test',test_target,test,'--','--exact'],cwd=dst,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  (root/f'target/verification/mutation-{name}.log').write_text(r.stdout)
  detected=r.returncode!=0 and 'test result: FAILED' in r.stdout and 'error[E' not in r.stdout
  results.append(dict(name=name,detected=detected,exit_code=r.returncode));assert detected,r.stdout
(root/'target/verification/mutations.json').write_text(json.dumps(results,indent=2));print(results)
