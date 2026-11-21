"""Reviewed ownership-phase correspondence for the B-tree numeric projections.

Review seals include declarations, layouts, macros, trait bounds and bodies.
They stop unknown source from inheriting a result; a seal is not a solver proof.
verify_btree composes direct executable span contracts, native node kernels,
remaining credit models and deliberate compiled/semantic controls.
"""
import hashlib,json,re,subprocess
from pathlib import Path
from scanner_tools import scanner
from verify_edit_loops import block
from btree_callers import BODIES,CALLERS,RESTATED,SITES
ROOT=Path(__file__).resolve().parents[1]
FILES=['src/collections/btree_map.rs',*('src/collections/btree_map/'+f+'.rs' for f in ['node','iter','entry','operations','bulk','view','geometry'])]
SET_FILES=['src/collections/btree_set.rs','src/collections/btree_set/algebra.rs','src/collections/btree_set/iter.rs']
def is_btree(file):
    # These domains have independent source/contract checks in the global
    # composition. Local set references named left/right are not owner indices.
    # Unknown files must not silently inherit either domain's verification.
    return file in FILES or file in SET_FILES
# Reviewed packed-prefix construction and native-word borrowed edges. Node
# partitions/owning cursors stay unchanged; actual Kani and memory runs are separate.
REVIEWED_PHASES={'src/collections/btree_map.rs': 'c7a19eb82835dd90bec626956ce82432c8c2d74566ee0369a06c27559ed97ee4', 'src/collections/btree_map/node.rs': '350079ef5e7ca8c6f3d8ebb0533e2ab43ea5957651c36a33cea1740d78cd563b', 'src/collections/btree_map/iter.rs': 'a251f50a7f4e1bfe98bff93e42fd86fecb4496640bce5b77051bd77264e55b19', 'src/collections/btree_map/entry.rs': '209eba61d9c18c9f07d61b9b85eb7e725294c8f0cc172cf0955b699e2493d1be', 'src/collections/btree_map/operations.rs': 'c13a4094b92833cb3a1787d80b4a92229e6136171d72f3ceba7dbd64cf6f5405', 'src/collections/btree_map/bulk.rs': 'aea1fc92a5c8f2c8a47ae2984916de80790b40cfcdabf8edd7f4b9a937581a3b', 'src/collections/btree_map/view.rs': 'f54c222e8fe8cbb36db872aff538f7f993f837e27a0b75ae5c10fd1c25915a25'}
def kernel_envelopes(root):
    source=(root/'src/collections/btree_map/node.rs').read_text()
    compact=lambda s:re.sub(r'\s+','',re.sub(r'//[^\n]*','',s))
    owner=(root/'src/collections/btree_map.rs').read_text()
    bulk=compact((root/'src/collections/btree_map/bulk.rs').read_text())
    assert 'letmutpool=Pool::<_,_,_,false>{' in bulk, 'BTree append must retain preallocated-only node admission'
    assert 'letmutpool=Pool::<_,_,_,true>{' in bulk, 'BTree private sorted construction uses checked on-demand allocation'
    sorted_body=compact(block((root/'src/collections/btree_map/bulk.rs').read_text(),'fn try_from_sorted_source('))
    assert 'ifresult.len().checked_add(1).is_none(){returnErr(Error::Capacity);}' in sorted_body, 'BTree sorted-prefix length admission changed'
    assert sorted_body.index('checked_add(1)')<sorted_body.index('result.bulk_push('), 'BTree sorted-prefix admission must precede mutation'
    replacement=compact(block(owner,'fn try_replace_key('))
    assert replacement=='matchself.search(&key){Search::Found(c)=>{letold=unsafe{node::replace_key(c.node,c.index(),key)};drop(value);Ok(Some(old))}Search::Vacant(at)=>{self.try_insert_at(at,key,value)?;Ok(None)}}', 'BTree set replacement must use the found initialized key slot and transactional vacant insertion'
    expected={
        'replace_key':'unsafe{ptr::replace(key(node,i),replacement)}',
        'set_child':'unsafe{edge(node,i).write(child);(*child.as_ptr()).parent=Some(node);(*child.as_ptr()).parent_idx=iasu16;}',
        'fix':'letmuti=from;whilei<=to{unsafe{set_child(node,i,child(node,i))};i=add(i,1);}',
        'set_len':'unsafe{(*node.as_ptr()).len=Count(nasu16)};',
        'pairs':'unsafe{ptr::copy(key(src,from),key(dst,to),n);ptr::copy(val(src,from),val(dst,to),n);}',
        'edges':'unsafe{ptr::copy(edge(src,from),edge(dst,to),n)};',
        'child':'unsafe{edge(node,i).read()}',
        'free':'unsafe{storage::deallocate(node.cast(),layout::<K,V>(height),policy.budget())};',
    }
    for fn,body in expected.items():
        assert compact(block(source,'fn '+fn+'<'))==body,('BTree kernel ownership envelope changed',fn)
    # Branch shares the leaf prefix; field access must never borrow an entire
    # node containing previously yielded mutable references.
    for fn,field,ty in [('key','keys','K'),('val','vals','V')]:
        assert compact(block(source,'fn '+fn+'<'))=='unsafe{addr_of_mut!((*node.as_ptr()).'+field+').cast::<'+ty+'>().add(i)}'
    iterator=(root/'src/collections/btree_map/iter.rs').read_text()
    canonical=iterator[iterator.index('impl<K, V> BorrowState<K, V> {'):]
    new_body=compact(block(canonical,'fn new<P: AllocationPolicy>'))
    assert re.match(r'letSome\(root\)=map.rootelse\{returnSelf::empty\(\);?\};',new_body)
    assert 'front:Self::edge(root,map.height.get(),false)' in new_body
    assert 'back:Self::edge(root,map.height.get(),true)' in new_body
    assert 'remaining:Checked::new(map.len())' in new_body
    borrowed=iterator[iterator.index('impl<K, V> BorrowEdge<K, V> {'):]
    assert compact(block(borrowed,'fn index(self)'))=='self.index'
    assert compact(block(borrowed,'fn height(self)'))=='self.height'
    assert compact(block(borrowed,'fn set_index('))=='self.index=index;'
    assert compact(block(borrowed,'fn from_cursor('))=='Self::new(cursor.node,cursor.index(),cursor.height())'
    leaf=compact(block(iterator,'unsafe fn take<const EXACT: bool>'))
    assert 'let(i,edge)=super::geometry::leaf_edge(c.index(),node::len(c.node),back);next=BorrowEdge::new(c.node,edge,0);c.set_index(i);' in leaf, 'BTree borrowed leaf frontier arguments/publication changed'
    cleanup=compact(block(iterator,'fn drop_next('))
    assert 'super::geometry::drop_skip(c.index(),node::len(c.node),self.state.remaining.get(),' in cleanup, 'BTree cleanup frontier arguments changed'
    assert 'c.set_index(add(c.index(),skip));self.state.front=Some(c);self.state.remaining=Checked::new(sub(self.state.remaining.get(),skip));' in cleanup, 'BTree cleanup frontier publication changed'
    owner=iterator[iterator.index('impl<K, V, P: AllocationPolicy> IntoIter<K, V, P> {'):]
    assert compact(block(owner,'fn take('))=='letaccounting=self.policy.budget();Self::take_state(&mutself.state,back,|n,h|{unsafe{crate::storage::deallocate(n.cast(),node::layout::<K,V>(h),accounting)};})', 'BTree owning retirement layout/accounting mismatch'
def maintained(text,name):
    # Specification and body of one maintained function, up to its closing brace.
    start=text.index('\nfn '+name+'(');return re.sub(r'\s+','',re.sub(r'//[^\n]*','',text[start:text.index('\n}\n',start)+3]))
def correspondence(root=ROOT):
    root=Path(root);compact=lambda s:re.sub(r'\s+','',re.sub(r'//[^\n]*','',s))
    model=(ROOT/'tools/direct-verus/btree-adapters.rs').read_text()
    for name,calls in CALLERS.items():
        text=maintained(model,name)
        for call in calls:assert call in text,('maintained BTree caller lost a product call',name,call)
    for (file,anchor,head),expected in BODIES.items():
        source=(root/'src/collections'/('btree_map/'+file if file!='btree_map.rs' else file)).read_text()
        assert source.count(anchor or head)==1,('BTree caller anchor is ambiguous',file,anchor,head)
        if anchor:source=source[source.index(anchor):]
        assert compact(block(source,head))==expected,('BTree caller body changed',file,anchor,head)
    for key,name,suffix in RESTATED:
        assert BODIES[key]+'}'==suffix,('BTree restated product body changed',key)
        assert maintained(model,name).endswith('{'+suffix),('BTree restated maintained body changed',name)
    assert set(SITES.values())<=set(CALLERS),'BTree site names an unknown maintained caller'
    return dict(callers=sorted(CALLERS),bodies=sorted(' '.join(k) for k in BODIES),restated=[r[1] for r in RESTATED])
def check(root=ROOT):
    root=Path(root);phases={file:hashlib.sha256((root/file).read_bytes()).hexdigest() for file in FILES if not file.endswith('/geometry.rs')}
    geometry=(root/'src/collections/btree_map/geometry.rs').read_text()
    for name in ('middle','advance','pack','index','height','replace_index','allocation_deficit','retired_successor','drop_skip','leaf_edge','consider_left','consider_right','borrow_required','insert_span','split_span','suffix_span','remove_span','merge_span','steal_left_span','steal_right_span'):
        declaration=re.search(r'pub\(crate\) fn '+name+r'(?:[<(])',geometry)
        assert declaration, ('BTree direct function missing',name)
        header=geometry[geometry.rfind('#[inline]',0,declaration.start()):declaration.start()]
        assert '#[cfg_attr(verus_keep_ghost, verus_spec(' in header, ('BTree direct contract declaration missing',name)
    bulk=(root/'src/collections/btree_map/bulk.rs').read_text()
    assert '#[cfg_attr(verus_keep_ghost, verus_verify)]\nstruct NodeCounts' in bulk, 'BTree NodeCounts type verification disabled'
    assert '#[cfg_attr(verus_keep_ghost, verus_verify)]\nimpl NodeCounts' in bulk, 'BTree NodeCounts state verification disabled'
    for name in ('record_retired','observe'):
        at=bulk.index('fn '+name+'(')
        assert '#[cfg_attr(verus_keep_ghost, verus_spec(' in bulk[bulk.rfind('#[inline]',0,at):at], ('BTree NodeCounts contract missing',name)
    compact=lambda text:re.sub(r'\s+','',text)
    planning=compact(block(bulk,'fn append_plan('))
    assert planning.count('needed.observe(leaves,branches,&retired);')==1, 'BTree allocation deficit caller arguments changed'
    assert 'let(leaves,branches,_)=output_shape(output)?;needed.observe(leaves,branches,&retired);' in planning, 'BTree output shape must precede deficit observation'
    retirement=compact(block(bulk,'fn next(&mut self, retired: &mut NodeCounts)'))
    assert 'self.state.advance_owned(c,false,|_,h|{retired.record_retired(h);});' in retirement, 'BTree retirement height routing changed'
    owner=(root/'src/collections/btree_map.rs').read_text()
    deleting=re.sub(r'\s+','',block(owner,'unsafe fn remove_at('))
    assert 'ifgeometry::consider_left(i)&&geometry::borrow_required(node::len(node::child(p,sub(i,1))),node::len(n)){' in deleting, 'BTree left deletion decision argument/guard routing changed'
    assert 'ifgeometry::consider_right(i,pn)&&geometry::borrow_required(node::len(node::child(p,add(i,1))),node::len(n)){' in deleting, 'BTree right deletion decision argument/guard routing changed'
    node_source=(root/'src/collections/btree_map/node.rs').read_text()
    assert '#[cfg_attr(verus_keep_ghost, verus_verify)]\npub(super) const CAP: usize = 11;' in node_source, 'BTree capacity constant verification changed'
    shape_at=bulk.index('fn output_shape(')
    assert '#[cfg_attr(verus_keep_ghost, verus_spec(' in bulk[bulk.rfind('/// Upper bound',0,shape_at):shape_at], 'BTree packed-shape direct contract disabled'
    for name,call in {'insert': 'let(after,tail,_,_)=super::geometry::insert_span(n,i);', 'split': 'let(start,n,edge_count)=super::geometry::split_span(len(node),mid);', 'move_suffix': 'let(n,edge_start)=super::geometry::suffix_span(len(left),index);', 'remove_leaf': 'let(after,tail,_)=super::geometry::remove_span(n,i);', 'merge': 'letstart=super::geometry::merge_span(l,r).0;', 'steal_left': 'letl=super::geometry::steal_left_span(left_count,r).0;', 'steal_right': 'set_len(left,super::geometry::steal_right_span(l,r).0);'}.items():
        assert call in compact(block(node_source,"fn "+name+"<")), ("BTree node span routing changed",name)
    kernel_envelopes(root)
    correspondence(root)
    assert phases==REVIEWED_PHASES,'BTree phase correspondence requires review; numeric-only is not a complete source certificate'
    rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True))
    bindings=[]
    for row in rows:
        fn=row['function'];file=row['file'];ctx=row['context'];contract='btree_cursor_state';role='typed-borrow-and-source-phase'
        if file.endswith('/node.rs'):
            contract={'key':'btree_layout_slot','val':'btree_layout_slot','edge':'btree_layout_edge','child':'btree_layout_edge','set_child':'btree_layout_edge',
                'insert':'btree_actual_insert','split':'btree_actual_split','move_suffix':'btree_actual_move_suffix','remove_leaf':'btree_actual_remove_leaf',
                'merge':'btree_actual_merge','steal_left':'btree_actual_steal_left','steal_right':'btree_actual_steal_right',
                'set_len':'btree_closed_node_count','len':'btree_closed_node_count','fix':'btree_fix_cursor','drain':'btree_cleanup_prefix','drop_pair':'btree_cleanup_prefix',
                'destroy':'btree_cleanup_prefix','drop':'btree_cleanup_prefix','allocate':'btree_initial_state','free':'btree_layout_slot',
                'replace_key':'btree_set_replace_slot','pairs':'btree_layout_slot','edges':'btree_layout_edge','parent':'btree_cursor_state','reserve':'btree_initial_state',
                'take':'btree_closed_node_count','recycle':'btree_initial_state','new':'btree_initial_state'}[fn]
            role='closed-node-count-and-initialized-partition'
        elif file.endswith('/bulk.rs'):
            contract='btree_actual_retired_count' if fn=='next' and ctx.startswith('PlanningStream') else 'btree_planned_allocation' if fn in ('try_append','retire','next','take') else 'btree_actual_leaves_deficit' if fn=='append_plan' else 'btree_packed_branch_balance'
            role='shared-retirement-kernel-and-prefix-allocation-deficit'
        elif fn=='try_replace_key':contract='btree_set_replace_slot';role='equal-key-initialized-slot-replacement-without-structural-change'
        elif fn=='clone_node':contract='btree_clone_prefix';role='tracked-field-initialization'
        elif fn in ('try_clone','try_clone_in','try_clone_from'):contract='btree_clone_prefix';role='clone-owner-publication'
        elif fn in ('plan_insert','commit_insert'):contract='btree_plan_consume';role='preallocation-and-commit-frontier'
        elif fn in ('commit_fit','try_insert_at','try_insert_with'):contract='btree_checked_length';role='checked-total-length-before-leaf-insert'
        elif fn=='remove_at':contract='btree_delete_decision';role='allocation-free-delete-and-cursor-relocation'
        elif fn in ('shrink_root','fix_border','try_split_off','count_subtree'):contract='btree_repair';role='split-pillar-and-border-repair'
        elif fn in ('take_with','take_state','advance_owned'):contract='btree_forest_branch';role='read-parent-before-retirement'
        elif fn=='drop_next':contract='btree_actual_drop_skip';role='count-capped-skip-and-deferred-in-place-drop'
        elif fn=='drop':contract='btree_cleanup_prefix';role='progress-before-user-destructor'
        elif fn=='set_index':contract='btree_actual_cursor_update';role='single-scalar-index-update-preserves-height'
        elif 'BorrowState' in ctx:contract='btree_actual_leaf_edge';role='native-word-leaf-edge-frontier-and-last-item-before-retired-child'
        elif fn in ('first','last','next','next_back','prev','take','try_next'):contract='btree_cursor';role='bounded-iteration-frontier'
        if file.endswith('/view.rs'):
            contract='btree_external_depth' if fn in ('provide','advance','new','try_range') else 'btree_layout_slot';role='checked-visited-slice-structure'
        # Maintained callers: the function calls the product geometry and
        # bounded arithmetic with the arguments of this product body.
        base=file.split('/')[-1];kind=ctx.split('<')[0].strip()
        caller=SITES.get((base,fn,kind),SITES.get((base,fn,None))) if row['kind']!='unsafe_impl' and fn else None
        if caller:contract=caller;role='maintained-caller-over-product-kernels-with-stated-tree-premises'
        bindings.append(dict(site=row,contract=contract,role=role,boundary='typed valid allocations, balanced private ownership graph and user callback contracts; source transition/argument correspondence plus compiler/Miri/ASan'))
    return phases,bindings
