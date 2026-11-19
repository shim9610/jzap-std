#!/usr/bin/env python3
"""Check direct search/cursor code, remaining node projections and phase bindings.

Native Kani exercises the real finite node kernels. Verus checks arbitrary-word
geometry and traversal/recycling credit transitions. Compiler/Miri/ASan retain
the separately agreed typed allocation, provenance, aliasing and Drop role.
"""
import argparse,hashlib,json,os,re,subprocess,tempfile,shutil
from pathlib import Path
from arithmetic_projection import validate_bounded,replace_once
from btree_bindings import FILES,is_btree,check as bindings,correspondence
from btree_core import projection as core_projection
from verus_composition import run as compose
from verify_source import verify as direct
import storage_kernel
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'target/verification/btree/proofs'
def remaining_models(root=ROOT,core_root=None):
    # Only pinned core/ownership/credit models remain here. Numeric product
    # contracts are maintained call-only Rust adapters over executable source.
    core,_,_=core_projection(root,core_root)
    template=(ROOT/'tools/memory-proof/btree.verus.rs').read_text();at=template.rindex('\n}')
    # The storage kernel comes first: the node primitives of the adapters are
    # proved from its conditions.
    return storage_kernel.kernel()+template[:at]+'\n'+core+template[at:]+'\n'+(ROOT/'tools/direct-verus/btree-adapters.rs').read_text()

# Weakened maintained callers. Each must make Verus report an error, because a
# product call inside the caller loses its precondition or an index bound.
SOLVER_CONTROLS=[
 # Node primitives over the storage model: each index bound is needed for
 # the documented condition on every valid node.
 ('pair-index-at-capacity','fn btree_pair_index(i:usize)\n requires i<11,','fn btree_pair_index(i:usize)\n requires i<=11,'),
 ('edge-index-at-capacity','fn btree_edge_index(i:usize)\n requires i<12,','fn btree_edge_index(i:usize)\n requires i<=12,'),
 ('pair-span-past-array',' requires from as int+count as int<=11,to as int+count as int<=11,',' requires from as int+count as int<=11,'),
 ('edge-span-past-array',' requires from as int+count as int<=12,to as int+count as int<=12,',' requires to as int+count as int<=12,'),
 ('node-edge-array-short',' && raw_valid(n.edges) && n.edges.cap==12',' && raw_valid(n.edges) && n.edges.cap==11'),
 ('pair-copy-other-element-type',' ensures forall|a:BtreeNode,b:BtreeNode| btree_node(a) && btree_node(b) && btree_same_map(a,b)\n   ==> #[trigger] btree_pair_copy(a,from,b,to,count),',' ensures forall|a:BtreeNode,b:BtreeNode| btree_node(a) && btree_node(b)\n   ==> #[trigger] btree_pair_copy(a,from,b,to,count),'),
 ('replace-at-capacity','fn btree_replace_index(i:usize)\n requires i<11,','fn btree_replace_index(i:usize)\n requires i<=11,'),
 ('view-prefix-past-array','fn btree_view_index(count:usize)\n requires count<=11,','fn btree_view_index(count:usize)\n requires count<=12,'),
 ('count-wider-than-word','fn btree_count_index(n:usize)\n requires n<=11,','fn btree_count_index(n:usize)\n'),
 ('insert-shift-whole-node',' btree_pairs_index(i,after,tail);',' btree_pairs_index(i,after,old);'),
 ('merge-right-pairs-by-parent-count',' btree_pairs_index(0,start,right_old);',' btree_pairs_index(0,start,parent_old);'),
 ('steal-right-edge-source-shifted',' btree_edges_index(1,0,right_old);',' btree_edges_index(2,0,right_old);'),
    ('fix_twelve',' requires to<=11,\n{\n let mut i=from;',' requires to<=12,\n{\n let mut i=from;'),
    ('drain_leaf_children',' requires children<=12,pairs<=11,children>0 ==> height>0,',' requires children<=12,pairs<=11,'),
    ('destroy_two_children','wrapping_add(n,1)};\n btree_drain_caller(children,n,height);','wrapping_add(n,2)};\n btree_drain_caller(children,n,height);'),
    ('search_inclusive',' while i<count\n  invariant i<=count,count<=11,h<usize::BITS,replies@.len()>=count,',' while i<=count\n  invariant i<=count,count<=11,h<usize::BITS,replies@.len()>=count,'),
    ('commit_fit_full','  if count<11 {\n   btree_actual_insert(count,index);','  if count<=11 {\n   btree_actual_insert(count,index);'),
    ('split_right_index','   crate::arithmetic::bounded::wrapping_add(mid,1));\n  btree_actual_insert(right,index);','   crate::arithmetic::bounded::wrapping_add(mid,0));\n  btree_actual_insert(right,index);'),
    ('plan_without_height_guard','  if height>=crate::arithmetic::bounded::wrapping_sub(usize::BITS as usize,1) {\n   return 0;\n  }\n',''),
    ('rebalance_empty_parent',' requires count<5,0<parent<=11,i<=parent,',' requires count<5,parent<=11,i<=parent,'),
    ('rebalance_full_child',' requires count<5,0<parent<=11,i<=parent,',' requires count<=11,0<parent<=11,i<=parent,'),
    ('remove_empty_map','pred_count<=11,length>0,','pred_count<=11,'),
    ('shrink_leaf_root',' requires height>0,\n ensures next<height,',' ensures next<=height,'),
    ('descend_wide_node','forall|j:int| 0<=j<counts@.len() ==> #[trigger] counts@[j]<=11,\n ensures','forall|j:int| 0<=j<counts@.len() ==> #[trigger] counts@[j]<=12,\n ensures'),
    ('step_edge_cursor',' requires index<count,count<=11,level<=height,height<usize::BITS,\n    parent_index<=parent_count,parent_count<=11,climbed<height,\n ensures next.0',' requires index<=count,count<=11,level<=height,height<usize::BITS,\n    parent_index<=parent_count,parent_count<=11,climbed<height,\n ensures next.0'),
    ('take_empty_interval',' requires remaining>0,\n ensures next as int==remaining as int-1,',' ensures next as int==remaining as int-1,'),
    ('retire_above_root',' requires level<height,height<usize::BITS,\n ensures next<=height,',' requires level<=height,height<usize::BITS,\n ensures next<=height,'),
    ('borrow_take_past_end',"  yielded=(true,i);\n } else {","  yielded=(true,edge);\n } else {"),
    ('fix_border_empty_node',' requires 0<count<=11,level>0,a<=11,b<=11,',' requires count<=11,level>0,a<=11,b<=11,'),
    ('count_without_limit',"   assert(count as int+btree_prefix_sum(children@,i as int+1)<=limit);\n",''),
    ('split_off_foreign_slot','slot as usize<=count,count<=11,level<=height,moved<=old,','count<=11,level<=height,moved<=old,'),
    ('clone_extra_child','   invariant children<=n+1,n<=11,height>0,','   invariant children<=n+2,n<=11,height>0,'),
    ('bulk_push_full_root','  let pillar=crate::arithmetic::bounded::wrapping_sub(target_height,1);','  let pillar=crate::arithmetic::bounded::wrapping_sub(height,1);'),
    ('packed_border_short_left',' requires 0<count<=11,level>0,left==11,right<=11,',' requires 0<count<=11,level>0,left<=11,right<=11,'),
    ('view_child_unchecked',' if index>count {\n  return false;\n }\n',''),
]
# Compiled product changes that the whole-body comparison must refuse.
TEXT_CONTROLS=[
    ('search-scan-bound','main','while i < count {','while i <= count {'),
    ('fit-capacity-test','main','if unsafe { node::len(at.node) } < CAP {','if unsafe { node::len(at.node) } <= CAP {'),
    ('commit-right-index','main','(right, sub(cur.index(), add(mid, 1)))','(right, sub(cur.index(), mid))'),
    ('plan-height-guard','main','if self.height.get() >= sub(usize::BITS as usize, 1) {','if self.height.get() > sub(usize::BITS as usize, 1) {'),
    ('remove-merge-separator','main','let sep = if i > 0 { sub(i, 1) } else { 0 };','let sep = i;'),
    ('fix-loop-bound','node','while i <= to {','while i < to {'),
    ('destroy-children','node','children: if height == 0 { 0 } else { add(n, 1) },','children: if height == 0 { 0 } else { add(n, 2) },'),
    ('drain-height','node','sub(self.height, 1),','self.height,'),
    ('merge-fix-range','node','fix(left, start, super::geometry::merge_span(l, r).1);','fix(left, 0, super::geometry::merge_span(l, r).1);'),
    ('cursor-last-index','iter','Some(Self::new(node, sub(n, 1), 0))','Some(Self::new(node, n, 0))'),
    ('cursor-next-bound','iter','if i < node::len(p) {\n                    return Some(Self::new(p, i, h));','if i <= node::len(p) {\n                    return Some(Self::new(p, i, h));'),
    ('borrow-take-child','iter','node::child(parent, if back { at } else { add(at, 1) }),','node::child(parent, if back { at } else { add(at, 2) }),'),
    ('occupied-slot','entry','unsafe { &*node::val(self.at.node, self.at.index()) }','unsafe { &*node::val(self.at.node, super::add(self.at.index(), 1)) }'),
    ('border-merge-test','operations','if add(add(node::len(a), 1), node::len(b)) <= CAP {','if add(node::len(a), node::len(b)) <= CAP {'),
    ('clone-child-bound','operations','while guard.children <= n {','while guard.children <= add(n, 1) {'),
    ('bulk-pillar-height','bulk','Self::pillar_from_pool(sub(target_height, 1), &self.policy, pool)','Self::pillar_from_pool(target_height, &self.policy, pool)'),
    ('packed-border-minimum','bulk','while node::len(right) < MIN {','while node::len(right) <= MIN {'),
    ('view-child-check','view','if index > n {','if index > super::add(n, 1) {'),
]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--numeric-only',action='store_true');p.add_argument('--mutations',action='store_true');p.add_argument('--kani',action='store_true');args=p.parse_args()
    OUT.mkdir(parents=True,exist_ok=True);(OUT/'summary.json').unlink(missing_ok=True);validate_bounded(ROOT)
    source=lambda:hashlib.sha256(b''.join(str(x.relative_to(ROOT)).encode()+b'\0'+x.read_bytes() for x in [*sorted((ROOT/'src').rglob('*.rs')),ROOT/'tools/memory-proof/btree.verus.rs',Path(__file__),ROOT/'scripts/btree_bindings.py',ROOT/'scripts/btree_callers.py',ROOT/'scripts/arithmetic_projection.py',ROOT/'tools/direct-verus/btree-adapters.rs',*storage_kernel.INPUTS])).hexdigest()
    _,core,core_paths=core_projection(ROOT)
    product_source=source
    source=lambda:hashlib.sha256(product_source().encode()+b''.join(p.read_bytes() for p in core_paths)+(ROOT/'scripts/btree_core.py').read_bytes()).hexdigest()
    before=source();direct_source=direct(['btree-node-spans','btree-search-cursors','btree-plan-counts','btree-frontiers','btree-delete-decisions','btree-packed-shape'],OUT/'direct-source',args.mutations);phases,sites=({},[]) if args.numeric_only else bindings();text=remaining_models();results=[]
    verus=Path(os.environ.get('JZAP_VERUS','/workspace/.devtools/verus/verus-x86-linux/verus'))
    def verify(name,text,negative=False,source_root=ROOT):
        file=OUT/(name+'.generated.rs');file.write_text(text)
        command,r=compose(verus,source_root,file,'jzap_btree_proof')
        log=r.stdout+r.stderr;(OUT/(name+'.log')).write_text(log);counts=re.findall(r'verification results:: (\d+) verified, (\d+) errors',log)
        assert counts and 'error[E' not in log and ((r.returncode!=0 and any(int(e) for _,e in counts)) if negative else (r.returncode==0 and all(int(e)==0 for _,e in counts))),log[-5000:]
        results.append(dict(name=name,command=command,exit_code=r.returncode,verification=counts,negative_control=negative))
    verify('runtime-calls-and-remaining-models',text)
    callers=correspondence();caller_controls=[]
    if args.mutations:
        for name,old,new in SOLVER_CONTROLS:assert text.count(old)==1,('BTree solver control pattern is not unique',name,text.count(old))
        for name,old,new in SOLVER_CONTROLS:
            verify('caller-control-'+name,text.replace(old,new),True)
            caller_controls.append(dict(name=name,detected_by='Verus rejects the weakened maintained caller'))
        for name,file,old,new in TEXT_CONTROLS:
            with tempfile.TemporaryDirectory(prefix='jzap-btree-text-') as tmp:
                root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');path=root/('src/collections/btree_map.rs' if file=='main' else 'src/collections/btree_map/'+file+'.rs');path.write_text(replace_once(path.read_text(),old,new))
                built=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(root/'probe.rlib')],capture_output=True,text=True);assert built.returncode==0,(name,built.stderr[-2000:])
                try:correspondence(root)
                except AssertionError as error:diagnostic=str(error);assert 'BTree caller body changed' in diagnostic,diagnostic
                else:raise AssertionError(('changed BTree caller body accepted',name))
                caller_controls.append(dict(name=name,compiled=True,detected_by='source correspondence comparison',diagnostic=diagnostic))
    if args.mutations:
        # A returning formatter error may leave pending=true, but result=Err
        # suppresses later guards. A successful value must clear the flag.
        with tempfile.TemporaryDirectory(prefix='jzap-btree-core-') as tmp:
            core_root=Path(tmp);(core_root/'fmt').mkdir();(core_root/'num').mkdir()
            for path in core_paths:
                destination=core_root/path.parent.name/path.name;shutil.copyfile(path,destination)
            path=core_root/'fmt/builders.rs';path.write_text(replace_once(path.read_text(),'self.has_key = false;','self.has_key = true;'))
            mutant=remaining_models(ROOT,core_root);verify('mutation-core-map-value-flag',mutant,True)
        with tempfile.TemporaryDirectory(prefix='jzap-btree-format-') as tmp:
            root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');path=root/'src/collections/btree_map.rs'
            path.write_text(replace_once(path.read_text(),'f.debug_map().entries(self.iter()).finish()','f.debug_map().key(&self.len()).finish()'))
            built=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(root/'probe.rlib')],capture_output=True,text=True);assert built.returncode==0,built.stderr
            try:core_projection(root)
            except AssertionError as error:assert 'BTree debug_map call protocol changed' in str(error)
            else:raise AssertionError('unpaired formatter call accepted')
            results.append(dict(name='mutation-unpaired-format-entry',compiled=True,detected_by='core/product protocol correspondence'))
    kani=[h for row in direct_source['groups'] for h in row['kani']['harnesses']];native_controls=[]
    if args.kani:
        command=['cargo','kani','--lib','--harness','collections::btree_map::iter::borrowed_edge_proofs::native_words_and_packed_conversion_preserve_indices','-Z','unstable-options','--cbmc-args','--object-bits','10']
        with (OUT/'kani.log').open('w') as log:r=subprocess.run(command,cwd=ROOT,stdout=log,stderr=log)
        log=(OUT/'kani.log').read_text();assert r.returncode==0 and 'Complete - 1 successfully verified harnesses, 0 failures, 1 total.' in log,log[-5000:]
        word_harnesses=re.findall(r'Checking harness (.*?)\.\.\.',log);assert len(word_harnesses)==1
        kani+=word_harnesses
        if args.mutations:
            # A source seal alone cannot demonstrate the leaf proof's sensitivity.
            # Compile a separate actual Rust variant and require a counterexample.
            with tempfile.TemporaryDirectory(prefix='jzap-btree-leaf-') as tmp:
                root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');shutil.copytree(ROOT/'examples',root/'examples')
                for file in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/file,root/file)
                path=root/'src/collections/btree_map/iter.rs'
                path.write_text(replace_once(path.read_text(),'front: Self::edge(root, map.height.get(), false),','front: Self::edge(root, map.height.get(), true),'))
                target='collections::btree_map::iter::borrowed_edge_proofs::borrowed_leaf_interval_preserves_each_slot_and_ends'
                command=['cargo','kani','--manifest-path',str(root/'Cargo.toml'),'--lib','--exact','--harness',target,'-Z','unstable-options','--cbmc-args','--object-bits','10']
                with (OUT/'leaf-start-mutant.log').open('w') as log:
                    result=subprocess.run(command,cwd=ROOT,env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target')),stdout=log,stderr=log)
                log=(OUT/'leaf-start-mutant.log').read_text()
                assert result.returncode and 'VERIFICATION:- FAILED' in log and 'error[E' not in log,log[-4000:]
                native_controls.append(dict(name='borrowed-starts-at-back',compiled=True,detected_by='actual-Rust-Kani-counterexample',separate_target=True))
    controls=[]
    if args.mutations and not args.numeric_only:
        for name,file,old,new in [
            ('delete-left-arguments','main','geometry::consider_left(i)','geometry::consider_left(0)'),
            ('delete-right-arguments','main','geometry::consider_right(i, pn)','geometry::consider_right(1, pn)'),
            ('parent-edge-identity','node','edge(node, i).write(child);','edge(node, 0).write(child);'),
            ('iterator-free-kind','iter','node::layout::<K, V>(h)','node::layout::<K, V>(0)'),
            ('free-kind','node','storage::deallocate(node.cast(), layout::<K, V>(height), policy.budget())','storage::deallocate(node.cast(), layout::<K, V>(0), policy.budget())'),
            ('tracked-cursor','main','if t.node == c.node && t.index() > c.index() { t.set_index(sub(t.index(), 1)); }','if t.node == c.node && t.index() > c.index() { t.set_index(add(t.index(), 1)); }'),
            ('unknown-intrinsic','node','edge(node, i).write(child);','core::arch::asm!(""); edge(node, i).write(child);'),
            ('append-reallocates-after-commit','bulk','let mut pool = Pool::<_, _, _, false> {','let mut pool = Pool::<_, _, _, true> {'),
            ('sorted-prefix-length-admission','bulk','if result.len().checked_add(1).is_none() {','if false {'),
        ]:
            with tempfile.TemporaryDirectory(prefix='jzap-btree-phase-') as tmp:
                root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');path=root/('src/collections/btree_map.rs' if file=='main' else 'src/collections/btree_map/'+file+'.rs');path.write_text(replace_once(path.read_text(),old,new))
                built=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(root/'probe.rlib')],capture_output=True,text=True);assert built.returncode==0,built.stderr
                try:bindings(root)
                except AssertionError as error:diagnostic=str(error);assert 'BTree' in diagnostic,diagnostic
                else:raise AssertionError(('unreviewed phase accepted',name))
                controls.append(dict(name=name,compiled=True,detected_by='source ownership phase binding',diagnostic=diagnostic))
    assert before==source(),'BTree proof inputs changed during verification'
    summary=dict(status='numeric-pass' if args.numeric_only else 'pass',complete=not args.numeric_only and args.kani and args.mutations,source_sha256=before,source_unchanged=True,core=core,direct_source=direct_source,word_domain="Verus Either32Or64; no positive word-type rewriting; retained ownership/core models remain",results=results,kani_harnesses=kani,native_controls=native_controls,phase_controls=controls,phases=phases,bindings=sites,callers=callers,caller_controls=caller_controls,
        verified_functions=re.findall(r'\b(?:proof\s+)?fn\s+(\w+)\s*\(',text),
        scope='Direct executable BTree search/cursor, planning, frontier, deletion, packed-shape and node-span kernels and actual Rust callers; remaining ownership/core and logical credit models. Typed ownership, actual aliasing, initialization and destructors require the compiler/Miri/ASan suites separately.')
    (OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('BTree geometry/credit proofs passed; complete=',summary['complete'])
if __name__=='__main__':main()
