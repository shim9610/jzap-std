"""Check the source side of the modular numeric/guard certificate.

This is a fail-closed projection checker, not a Rust interpreter. Arithmetic,
loop and credit facts come from the solver-checked projections; this checker
closes their constructor/write/call coverage and validates the actual published
expressions. Ownership, typed-layout and unsafe-caller contracts are explicit
boundaries checked by Rust, the existing memory suites and their controls.
"""
import json,re,subprocess,tempfile
from pathlib import Path
from scanner_tools import scanner

ROOT=Path(__file__).resolve().parents[1]


def compact(s):return re.sub(r'\s+','',s)
def canonical(s):return re.sub(r',(?=\))','',compact(s))


def parsed_operations(rows,out):
    """The syn-parsed calls, methods and macros of each site, in input order."""
    out=Path(out);out.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='operations-',dir=out) as tmp:
        path=Path(tmp)/'input.json';path.write_text(json.dumps(rows))
        parsed=json.loads(subprocess.check_output([str(ROOT/'target/panic-inventory/debug/unsafe_operations'),str(path)],text=True))
    key=lambda r:(r['file'],r['line'],r['column'],r['end_line'],r['end_column'],r['kind'])
    assert list(map(key,parsed))==list(map(key,rows)),'operation scanner changed the site order'
    return parsed


def audit_operations(rows,out):
    """An unsupported unsafe call must fail even without a new unsafe block."""
    parsed=parsed_operations(rows,out)
    calls=set();methods=set();macros=set()
    for site in parsed:
        for op in site.get('operations',[]):
            {'call':calls,'method':methods,'macro':macros}[op['kind']].add(op['operation'])
    allowed_calls={
        'Ok','Some','Err','core::ptr::copy','core::ptr::copy_nonoverlapping','core::ptr::read','core::ptr::drop_in_place','core::ptr::slice_from_raw_parts_mut',
        'core::slice::from_raw_parts','core::slice::from_raw_parts_mut','NonNull::new_unchecked','NonNull::slice_from_raw_parts','NonNull::dangling',
        'core::ptr::NonNull::new_unchecked','core::ptr::NonNull::slice_from_raw_parts',
        'InitSlot::new','crate::init::InitSlot::new','crate::init::InitSlot::from_raw',
        'storage::reallocate','storage::deallocate','alloc::alloc::alloc','alloc::alloc::alloc_zeroed','alloc::alloc::dealloc','alloc::alloc::realloc',
        'Self::from_raw_parts','Self::from_raw','Self::from_bytes_unchecked','Self::from_utf8_unchecked',
        'Box::from_raw_parts','Box::into_std','Vec::from_raw_parts','Self::into_raw','deallocate','crate::boxed::Box::from_raw_parts','crate::boxed::CallRelease::new','crate::boxed::StandardCallOwner::from_raw',
        'alloc::boxed::Box::from_raw','alloc::boxed::Box::into_raw','alloc::vec::Vec::from_raw_parts',
        'alloc::string::String::from_utf8_unchecked','OsString::from_encoded_bytes_unchecked',
        'Pin::new_unchecked','Pin::into_inner_unchecked','crate::boxed::Pinned::into_inner_unchecked',
        'Layout::from_size_align_unchecked','mem::align_of','crate::capacity::Capacity::from_validated_raw','core::mem::MaybeUninit::new',
        'crate::error::InitError::Storage','crate::arithmetic::bounded::wrapping_sub',
        'crate::init::prefix::initialize','prefix::initialize','initialize_one','write_next','crate::init::collection::fill',
    }
    allowed_methods={'as_ptr','as_mut_ptr','as_ref','as_mut','cast','add','read','write','drop_in_place','get_unchecked','get_unchecked_mut',
        'get','len','front','read','written','remaining','unvisited_len','is_aligned','mark_initialized','disarm_field','into_bytes','into_std_vec',
        'as_slice','as_str','budget','layout','size','assume_init'}
    assert calls<=allowed_calls,('unbound unsafe operation',sorted(calls-allowed_calls))
    assert methods<=allowed_methods,('unbound unsafe operation',sorted(methods-allowed_methods))
    assert macros<={'core::ptr::addr_of_mut'},('unbound unsafe operation',sorted(macros-{'core::ptr::addr_of_mut'}))
    return dict(calls=sorted(calls),methods=sorted(methods),macros=sorted(macros),sites=sum('operations' in s for s in parsed))


# Each row describes publications proved by the named model, not a list of
# hashes/symbols to excuse. Different publications require different contracts.
WRITES={
 ('src/init/collection.rs','drop'):('kani:local_cursor_publication_preserves_every_word',['*self.output=*self.current']),
 ('src/init/prefix.rs','initialize'):('prefix_storage_connection',['prefix.state=Checked::new(0)']),
 ('src/init/driver.rs','run'):('admitted_cursor',['*state=Checked::new(wrapping_add(index,1))']),
 ('src/boxed/buffer.rs','reserve'):('buffer_reserve',['self.ptr=ptr.cast()','self.cap=Checked::new(proposed)','self.layout=layout']),
 ('src/boxed/buffer.rs','finish'):('buffer_finish',['self.ptr=ptr.cast()','self.layout=layout']),
 ('src/boxed/buffer.rs','push'):('buffer_push',['self.len=Checked::new(needed)']),
 ('src/boxed/buffer.rs','append'):('reservation_before_bulk_write',['self.len=Checked::new(needed)']),
 ('src/boxed/text.rs','try_next'):('utf8_byte_cursor',['self.len=character.encode_utf8(&mutself.bytes).len()','self.position=0','self.position=wrapping_add(self.position,1)']),
 ('src/compaction.rs','keep_next'):('c_keep_next',['self.read=next_read','self.write=next_write']),
 ('src/compaction.rs','remove_next'):('c_remove_next',['self.read=next']),
 ('src/compaction.rs','keep_live'):('c_keep_live',['self.read=next_read','self.write=next_write']),
 ('src/compaction.rs','remove_live'):('c_remove_live',['self.read=next']),
 ('src/compaction.rs','stop'):('e_stop',['self.end=self.read']),
 ('src/compaction.rs','keep_next','Extraction'):('e_keep_next',['self.read=Checked::new(wrapping_add(self.read.get(),1))']),
 ('src/compaction.rs','remove_next','Extraction'):('e_remove_next',['self.read=Checked::new(wrapping_add(read,1))','self.deleted=Checked::new(wrapping_add(self.deleted.get(),1))']),
 ('src/init.rs','try_init_array'):('prefix_storage_connection',['self.initialized=true']),
 ('src/span.rs','clear'):('kani:both_transitions_preserve_order_and_exact_length',['self.front=self.back']),
 ('src/span.rs','next'):('kani:both_transitions_preserve_order_and_exact_length',['self.front=next']),
 ('src/span.rs','next_back'):('kani:both_transitions_preserve_order_and_exact_length',['self.back=next']),
 ('src/vec.rs','resize_storage'):('resized_owner',['self.ptr=ptr.cast()','self.cap=capacity']),
 ('src/vec.rs','try_insert'):('insertion_before_moves',['self.len=Checked::new(needed)']),
 ('src/vec.rs','pop'):('shortened_owner',['self.len=next']),
 ('src/vec.rs','remove'):('removal_before_moves',['self.len=next']),
 ('src/vec.rs','swap_remove'):('swap_remove_before_moves',['self.len=next']),
 ('src/vec.rs','truncate'):('shortened_owner',['self.len=next']),
 ('src/vec.rs','try_extend_copy'):('commit_reserved_owner',['self.len=Checked::new(needed)']),
 ('src/vec.rs','try_extend_cloned'):('commit_reserved_owner',['self.len=Checked::new(needed)']),
 ('src/vec.rs','try_append'):('append_owner',['self.len=Checked::new(needed)','other.len=Checked::new(0)']),
 ('src/vec.rs','try_split_off'):('split_owner',['tail.len=Checked::new(len)','self.len=Checked::new(at)']),
 ('src/vec.rs','try_resize_with_init'):('initialized_operation',['guard.owner.len=next']),
 ('src/vec.rs','try_clone'):('empty_owner',['copy.len=Checked::new(0)']),
 ('src/vec/convenience.rs','push_within_capacity'):('kani:within_capacity_push_preserves_every_numeric_owner',['self.len=next']),
 ('src/vec/convenience.rs','try_set_len'):('checked_owner_length',['self.len=Checked::new(new_len)']),
 ('src/vec/convenience.rs','try_extend_from_within'):('clone_within_operation',['guard.owner.len=next']),
 ('src/vec/convenience.rs','try_extend_copy_from_within'):('internal_copy_before_append',['self.len=Checked::new(needed)']),
 ('src/vec/convenience.rs','try_resize_owned'):('owned_resize_operation',['guard.owner.len=next','guard.owner.len=next']),
 ('src/vec/convenience.rs','try_into_array'):('owner_to_array',['self.len=Checked::new(0)']),
 ('src/vec/convenience.rs','try_from_array_in'):('array_import',['result.len=Checked::new(N)']),
 ('src/vec/convenience.rs','try_write_vectored'):('vectored_owner',['total=(total+Checked::new(part.len())).map_err(|_|Error::Capacity)?','destination=unsafe{destination.add(part.len())}','self.len=Checked::new(needed)']),
 ('src/vec/edit.rs','try_retain_mut'):('retained_loop',['g.owner.len=Checked::new(g.state.written())']),
 ('src/vec/edit.rs','try_dedup_by'):('dedup_loop',['g.owner.len=Checked::new(g.state.written())']),
 ('src/vec/edit.rs','try_drain'):('drain_constructor',['self.len=Checked::new(start)']),
 ('src/vec/edit.rs','try_extract_if'):('e_try_range',['self.len=Checked::new(0)']),
 ('src/vec/edit.rs','restore_tail'):('hole_restore',['self.owner.len=Checked::new(crate::arithmetic::bounded::wrapping_add(self.owner.len.get(),self.tail_len,))','self.tail_len=0']),
 ('src/vec/edit.rs','move_tail'):('hole_move',['self.drain.tail_start=next_tail.get()']),
 ('src/vec/edit.rs','try_finish'):('splice_operation',['self.drain.owner.len=next']),
 ('src/vec/edit.rs','stage_push'):('kani:staging_push_preserves_every_admitted_word_owner',['extra.len=next']),
 ('src/vec/staging_driver.rs','run'):('staged_collection_loop',['len=new_len','cap=new_cap','pending=value']),
 ('src/vec/edit.rs','stage_excess'):('stage_operation',['self.drain.tail_start=next_tail.get()','self.drain.owner.len=next_len','extra.len=Checked::new(0)']),
 ('src/vec/ownership.rs','into_iter'):('cursor_before_read',['self.len=Checked::new(0)']),
}


def compaction_leaves(root, unsafe_rows, state_rows):
    """Read-only correspondence checks; no proof program is generated here."""
    from verify_edit_loops import block
    source=(Path(root)/'src/vec/edit.rs').read_text()
    transfer=canonical("""unsafe { if retain {
        core::ptr::copy_nonoverlapping(pointer.as_ptr().add(read),pointer.as_ptr().add(write),1);
    } else {core::ptr::drop_in_place(pointer.as_ptr().add(read));}}""")
    for name,paired in [('try_retain_mut','false'),('try_dedup_by','true')]:
        body=canonical(block(source,'pub fn '+name+'<'))
        assert body.count('super::compaction_driver::run::<'+paired+',E,_,_>(&mutg.state,original,')==1, ('compaction driver routing',name)
        assert body.count('letpointer=g.owner.ptr;')==1, ('compaction allocation identity',name)
        actual=[canonical(row['expression']) for row in unsafe_rows
                if row['file']=='src/vec/edit.rs' and row['function']==name and row['kind']=='unsafe_block']
        expected=[canonical('unsafe {&mut*pointer.as_ptr().add(read)}')]
        if paired=='true':
            expected.append(canonical('unsafe {&mut*pointer.as_ptr().add(previous)}'))
            assert 'letprevious=crate::arithmetic::bounded::wrapping_sub(write,1);' in body
            assert '.map(|duplicate|!duplicate)' in body
        else:
            assert 'keep(unsafe{&mut*pointer.as_ptr().add(read)})' in body, ('compaction predicate routing',name)
        assert actual==expected+[transfer], ('compaction physical leaf routing',name,actual)
        # The thin unsafe adapter must contain only the reviewed initializers,
        # exact leaves, driver invocation and length publication. This rejects
        # index shadowing or extra owner mutations without projecting a body.
        bodies=[r for r in state_rows if r['file']=='src/vec/edit.rs' and r['function']==name and r['kind']=='body']
        assert len(bodies)==1, ('compaction adapter body',name)
        normalize=lambda value:re.sub(r',(?=[)}])','',canonical(value))
        scan='|read,_write|{keep(unsafe{&mut*pointer.as_ptr().add(read)})}'
        if paired=='true':
            scan='|read,write|{let previous=crate::arithmetic::bounded::wrapping_sub(write,1);same(unsafe{&mut*pointer.as_ptr().add(read)},unsafe{&mut*pointer.as_ptr().add(previous)}).map(|duplicate|!duplicate)}'
        action='|read,write,retain|{'+transfer+'}'
        shape=['let original=self.len.get();']
        if paired=='true':shape+=['if original<2{return Ok(());}']
        constructor='full' if paired=='false' else 'first_kept'
        shape+=['let mut g=Compact{owner:self,state:Compaction::'+constructor+'(original)};',
                'let pointer=g.owner.ptr;',
                'super::compaction_driver::run::<'+paired+',E,_,_>(&mut g.state,original,'+scan+','+action+')?;',
                'g.owner.len=Checked::new(g.state.written());','mem::forget(g);','Ok(())']
        assert list(map(normalize,bodies[0]['detail']['statements']))==list(map(normalize,shape)), ('compaction adapter statements',name)
    extraction=source[source.index('for ExtractIf'):]
    adapter=canonical(block(extraction,'fn next('))
    assert adapter.count('super::compaction_driver::extract_next::<E,_,_>')==1, 'extraction driver routing'
    actual=[canonical(row['expression']) for row in unsafe_rows
            if row['file']=='src/vec/edit.rs' and row['function']=='next' and row['context'].startswith('ExtractIf') and row['kind']=='unsafe_block']
    transfer='unsafe {core::ptr::copy_nonoverlapping(pointer.as_ptr().add(read),pointer.as_ptr().add(write),1);}'
    expected=['unsafe {&mut *pointer.as_ptr().add(read)}',transfer,'unsafe {core::ptr::read(pointer.as_ptr().add(read))}']
    assert actual==list(map(canonical,expected)), ('extraction physical leaf routing',actual)
    bodies=[r for r in state_rows if r['file']=='src/vec/edit.rs' and r['function']=='next' and r['context'].startswith('ExtractIf') and r['kind']=='body']
    shape=['let pointer=self.owner.ptr;','let filter=&mut self.filter;',
           'let result=super::compaction_driver::extract_next::<E,_,_>(&mut self.state,|read|{filter(unsafe{&mut *pointer.as_ptr().add(read)})},|read,write|{'+transfer+'});',
           'match result {Ok(Some(read))=>Some(Ok(unsafe{core::ptr::read(pointer.as_ptr().add(read))})),Ok(None)=>None,Err(error)=>self.fail(error)}']
    assert len(bodies)==1 and list(map(normalize,bodies[0]['detail']['statements']))==list(map(normalize,shape)), 'extraction adapter statements'
    assert canonical(block(source[source.index('impl<T, F, P: AllocationPolicy, L: LengthPolicy> ExtractIf'):],'fn fail<'))=='self.state.stop();Some(Err(error))', 'extraction cold error routing'
    stage_bodies=[r for r in state_rows if r['file']=='src/vec/edit.rs' and r['function']=='stage_excess' and r['kind']=='body']
    assert len(stage_bodies)==1, 'staging controller AST body'
    stage=canonical(stage_bodies[0]['expression'])[1:-1].split('lettotal',1)[0]
    prefix='let available = ((Checked::new(self.drain.owner.max_len()) - self.drain.owner.len).map_err(|_| Error::Capacity)? - Checked::new(self.drain.tail_len)).map_err(|_| Error::Capacity)?.get();\nlet mut extra = Vec::<T, P>::with_limit_in(available, self.drain.owner.policy.try_clone()?);\nlet (staged, _, _) = super::staging_driver::run(first, available, mem::size_of::<T>(), extra.capacity(), |_index, _cap, value| { let reply = stage_batch(&mut extra, &mut self.replacement, value); (reply, extra.len.get(), extra.capacity()) },);\nif !staged? { return Err(InitError::Storage(Error::Capacity)); }'
    assert stage==canonical(prefix), 'staging controller arguments and error routing'
    leaf_bodies=[r for r in state_rows if r['file']=='src/vec/edit.rs' and r['function']=='stage_push' and r['kind']=='body']
    assert len(leaf_bodies)==1, 'owning staging leaf AST body'
    leaf=canonical(leaf_bodies[0]['expression'])[1:-1]
    expected='let next = (extra.len + Checked::new(1)).map_err(|_| Error::Capacity)?;\nextra.try_reserve(1)?;\nunsafe { extra.ptr.as_ptr().add(extra.len.get()).write(value) };\nextra.len = next;\nOk(extra.capacity())'
    assert leaf==canonical(expected), 'owning staging leaf and publication'
    guard=canonical(block(source[source.index('Drop for Compact'):],'fn drop('))
    assert guard.startswith('ifself.state.read()==self.state.written(){return;}')


def check(root,generated,rows,unsafe_rows,kani_harnesses):
    from verify_string import is_string
    from shared_bindings import separate, check as check_shared
    from btree_bindings import is_btree
    from bplus_bindings import check as check_bplus, is_bplus
    from hash_map_bindings import is_hash_map,is_copy_capability_site
    from verify_hash_set import is_hash_set,bindings as check_hash_set
    from verify_binary_heap import is_binary_heap,bindings as check_binary_heap
    from verify_deque import is_deque,bindings as check_deque
    from verify_scratch_slice import is_scratch_slice,bindings as scratch_bindings
    from init_bindings import check as check_init
    check_init(root)
    check_binary_heap(root)
    check_deque(root)
    scratch_bindings(root)
    check_shared(root, rows)
    # Set fields own an already-validated map, not independent raw metadata.
    # Require its exact reviewed safe delegation and empty unsafe inventory
    # before handing that domain to the separate actual-Rust set checks.
    check_hash_set(root)
    # The B+ audit verifies the source-bound initialized prefix and snapshot separately.
    check_bplus(root)
    rows=[r for r in rows if not is_string(r['file']) and not separate(r) and not is_btree(r['file']) and not is_bplus(r['file']) and not is_hash_map(r['file']) and not is_hash_set(r['file']) and not is_copy_capability_site(r) and not is_binary_heap(r['file']) and not is_deque(r['file']) and not is_scratch_slice(r['file'])]
    unsafe_rows=[r for r in unsafe_rows if not is_string(r['file']) and not separate(r) and not is_btree(r['file']) and not is_bplus(r['file']) and not is_hash_map(r['file']) and not is_hash_set(r['file']) and not is_copy_capability_site(r) and not is_binary_heap(r['file']) and not is_deque(r['file']) and not is_scratch_slice(r['file'])]
    compaction_leaves(root,unsafe_rows,rows)
    root=Path(root);theorems=set(re.findall(r'\b(?:proof\s+)?fn\s+(\w+)\b',generated))
    for filename in ('state-adapters.rs','budget-adapters.rs','budget-ledger.rs','vec-adapters.rs','layout-adapters.rs','init-adapters.rs'):
        adapter=ROOT/'tools/direct-verus'/filename
        if 'include!('+json.dumps(str(adapter),ensure_ascii=False)+')' in generated:
            # Namespace routing only. Verus checks the maintained include in the
            # actual product crate; no declaration name is accepted as a proof.
            theorems |= set(re.findall(r'\b(?:proof\s+)?fn\s+(\w+)\b',adapter.read_text()))
    assert not [r for r in rows if r['kind']=='explicit_exit'],('explicit product exit', [r for r in rows if r['kind']=='explicit_exit'])
    seen={};bindings=[]
    def proof(name):
        if name.startswith('kani:'):assert name[5:] in kani_harnesses,name
        else:assert name in theorems,('missing verified contract',name)
        return name
    for row in rows:
        if row['kind']!='write':continue
        f,fn,ctx=row['file'],row['function'],row['context'];e=compact(row['expression'])
        key=(f,fn,ctx.split('<')[0].strip())
        if key not in WRITES:key=(f,fn)
        if key in WRITES:
            contract,expected=WRITES[key];position=seen.get(key,0)
            assert position<len(expected) and e==expected[position],('unbound publication',row,expected)
            seen[key]=position+1
        elif f=='src/vec/edit.rs' and fn=='drop':
            assert ctx.startswith(('ExtractIf','Compact')), ('unbound edit guard publication',row)
            contract='kani:extraction_drop_restores_every_numeric_cursor_exactly' if ctx.startswith('ExtractIf') else 'c_restored_len'
            expected='self.owner.len=Checked::new(self.state.restored_len())'
            assert e==expected,('unbound guard publication',row)
        elif f=='src/init.rs' and fn in ('write','finish','mark_initialized','disarm_field'):
            expected='self.initialized='+('true' if fn in ('write','mark_initialized') else 'false')
            assert e==expected,('unbound initialization state',row)
            contract='prefix_storage_connection'
        elif e=='*self=replacement':
            # A complete Rust owner/value is moved, not a raw numeric state.
            # New owners are covered by the constructor/transition closure.
            contract='replaced_owner'
        elif f=='src/iter.rs':
            # No allocation/pointer state: defaults use checked counters. Take
            # is additionally proved on actual Rust in the prerequisite suite.
            assert e in ('remaining=next','self.remaining=next','value=f(value,item).map_err(TryFoldError::Operation)?'),row
            contract='buffer_span'
        elif ctx.startswith('Writer'):
            assert e=='self.error=Some(e)',row
            contract='extend_operation'
        else:raise AssertionError(('unbound state write',row))
        bindings.append(dict(site=row,contract=proof(contract),role='state-publication'))
    for key,(contract,expected) in WRITES.items():assert seen.get(key)==len(expected),('missing expected publication',key)
    assert not [r for r in rows if r['kind']=='compaction_driver_import'], 'unbound compaction driver import'
    entries=[r for r in rows if r['kind']=='compaction_driver_reference']
    expected_entries={
        ('try_retain_mut','super::compaction_driver::run::<false,E,_,_>'):'retained_loop',
        ('try_dedup_by','super::compaction_driver::run::<true,E,_,_>'):'dedup_loop',
        ('next','super::compaction_driver::extract_next::<E,_,_>'):'extract_next_loop',
        ('stage_excess','super::staging_driver::run'):'staged_collection_loop',
    }
    found_entries=[]
    for row in entries:
        key=(row['function'],canonical(row['expression']))
        assert row['file']=='src/vec/edit.rs' and key in expected_entries, ('unbound compaction driver entry',row)
        if row['function']=='next':assert row['context'].startswith('ExtractIf'), 'unbound extraction driver context'
        found_entries.append(key)
        bindings.append(dict(site=row,contract=proof(expected_entries[key]),role='direct-compaction-entry'))
    assert len(found_entries)==len(expected_entries) and set(found_entries)==set(expected_entries), 'compaction driver entry set changed'

    # Only the directly verified control driver may call pre-admitted steps.
    # Reject references/escapes and route each actual call to the direct adapter.
    admitted={'keep_live':1,'remove_live':1}
    admitted_seen={name:0 for name in admitted}
    assert not [r for r in rows if r['kind']=='admitted_reference'], 'unbound admitted-step reference'
    for row in rows:
        if row['kind']!='admitted_step':continue
        expression=canonical(row['expression']);name=row['detail']['method']
        assert row['file']=='src/vec/compaction_driver.rs' and row['function']=='compact_one' and expression=='state.'+name+'()', 'unbound admitted cursor step'
        assert name in admitted, 'unbound admitted cursor method'
        admitted_seen[name]+=1
        bindings.append(dict(site=row,contract=proof('dedup_loop'),role='direct-admitted-cursor-step'))
    assert admitted_seen==admitted,('admitted step set changed',admitted_seen)

    # Accounting calls must use the source owner's policy and exact old/new
    # layout sizes. Extra debits, reservation swaps and duplicate releases are
    # rejected, even if the enclosing function already has another binding.
    protocol={
      ('src/boxed/buffer.rs','push'):('buffer_push',['self.reserve(1,hint)']),
      ('src/boxed/buffer.rs','append'):('buffer_reserve',['self.reserve(values.len(),values.len())']),
      ('src/boxed/buffer.rs','collect'):('buffer_collect',['self.reserve(lower,lower)']),
      ('src/boxed/call.rs','drop'):('released_credit',['self.policy.budget().release(self.bytes)']),
      ('src/boxed/owned.rs','try_from_std_in'):('allocation_credit',['policy.budget().reserve(layout.size())','r.commit().retain()','r.commit()']),
      ('src/boxed/slice.rs','try_from_std_vec_in'):('allocation_credit',['policy.budget().reserve(new.size())','r.commit().retain()','r.commit()']),
      ('src/budget.rs','reserve'):('ledger_reserve',['self.available.set(available)']),
      ('src/budget.rs','release'):('ledger_release',['self.occupied.set(budget_numeric::retire(self.occupied.get(),bytes))','self.available.set(budget_numeric::credit(self.available.get(),bytes))']),
      ('src/budget.rs','commit'):('ledger_commit',['self.budget.occupied.set(budget_numeric::credit(self.budget.occupied.get(),bytes))']),
      ('src/budget.rs','drop'):('ledger_cancel',['self.budget.available.set(budget_numeric::credit(self.budget.available.get(),self.bytes))','self.budget.release(self.bytes)']),
      ('src/storage.rs','allocate_impl'):('allocation_credit',['budget.reserve(layout.size())','r.commit().retain()','r.commit()']),
      ('src/storage.rs','deallocate'):('released_credit',['budget.release(layout.size())']),
      ('src/storage.rs','reallocate'):('reallocation_credit',['budget.reserve(new.size())','reservation.commit().retain()','reservation.commit()','budget.release(old.size())']),
      ('src/vec/ownership.rs','try_from_std_in'):('allocation_credit',['policy.budget().reserve(layout.size())','r.commit().retain()','r.commit()']),
      ('src/vec/ownership.rs','into_std'):('released_credit',['policy.budget().release(bytes)']),
    }
    protocol_seen={}
    for row in rows:
        if row['kind']!='protocol_method':continue
        key=(row['file'],row['function']);assert key in protocol,('unbound accounting call',row)
        contract,expected=protocol[key];n=protocol_seen.get(key,0)
        assert n<len(expected) and canonical(row['expression'])==canonical(expected[n]),('accounting argument/order mismatch',row,expected)
        protocol_seen[key]=n+1
        bindings.append(dict(site=row,contract=proof(contract),role='accounting-call'))
    for key,(_,expected) in protocol.items():assert protocol_seen.get(key)==len(expected),('missing accounting call',key)

    storage_calls={
      ('src/boxed.rs','try_new_in'):['storage::allocate(Layout::new::<T>(),policy.budget())'],
      ('src/boxed.rs','try_init_in'):['storage::allocate(Layout::new::<T>(),policy.budget())'],
      ('src/boxed.rs','try_clone'):['storage::allocate(Layout::new::<T>(),policy.budget())'],
      ('src/boxed.rs','drop'):['storage::deallocate(self.0,self.1,self.2)'],
      ('src/boxed/buffer.rs','reserve'):['storage::reallocate(self.ptr.cast(),self.layout,layout,self.policy.budget())'],
      ('src/boxed/buffer.rs','finish'):['storage::reallocate(self.ptr.cast(),self.layout,layout,self.policy.budget())'],
      ('src/boxed/owned.rs','try_new_uninit_in'):['storage::allocate(Layout::new::<T>(),policy.budget())'],
      ('src/boxed/owned.rs','try_new_zeroed_in'):['storage::allocate_zeroed(Layout::new::<T>(),policy.budget())'],
      ('src/boxed/slice.rs','try_init_slice_in'):['storage::allocate(layout,policy.budget())'],
      ('src/boxed/slice.rs','try_copy_from_slice_in'):['storage::allocate(layout,policy.budget())'],
      ('src/boxed/slice.rs','try_new_uninit_slice_in'):['storage::allocate(layout,policy.budget())'],
      ('src/boxed/slice.rs','try_new_zeroed_slice_in'):['storage::allocate_zeroed(layout,policy.budget())'],
      ('src/boxed/slice.rs','try_from_std_vec_in'):['storage::reallocate(ptr.cast(),old,new,crate::budget::Accounting::Unmetered)'],
      ('src/storage.rs','reallocate'):['allocate(new,budget)','deallocate(ptr,old,budget)'],
      ('src/vec.rs','resize_storage'):['storage::allocate(new_layout,self.policy.budget())','storage::reallocate(self.ptr.cast(),self.layout(),new_layout,self.policy.budget())'],
      ('src/vec.rs','drop'):['storage::deallocate(self.0,self.1,self.2)'],
    }
    storage_seen={}
    for row in rows:
        if row['kind']!='protocol_call':continue
        callee=compact(row['detail']['callee']);last=callee.split('::')[-1]
        if last not in ('allocate','allocate_zeroed','reallocate','deallocate'):continue
        key=(row['file'],row['function']);assert key in storage_calls,('unbound storage call',row)
        n=storage_seen.get(key,0);expected=storage_calls[key]
        assert n<len(expected) and canonical(row['expression'])==canonical(expected[n]),('storage policy/layout mismatch',row,expected)
        storage_seen[key]=n+1
        contract='reallocation_credit' if last=='reallocate' else 'released_credit' if last=='deallocate' else 'allocation_credit'
        bindings.append(dict(site=row,contract=proof(contract),role='storage-call'))
    for key,expected in storage_calls.items():assert storage_seen.get(key)==len(expected),('missing storage call',key)
    # Tuple guards retain the exact allocation/layout/policy through callbacks
    # and Drop. They are constructors too, despite having no named fields.
    guard_calls={
      ('src/boxed.rs','try_init_in'):'AllocationGuard(ptr.cast(),Layout::new::<T>(),policy.budget())',
      ('src/boxed.rs','drop'):'AllocationGuard(self.ptr.cast(),layout,self.policy.budget())',
      ('src/boxed/buffer.rs','drop'):'AllocationGuard(self.ptr.cast(),self.layout,self.policy.budget())',
      ('src/boxed/owned.rs','into_inner'):'AllocationGuard(ptr.cast(),Layout::new::<T>(),policy.budget())',
      ('src/boxed/slice.rs','try_init_slice_in'):'AllocationGuard(ptr.cast(),layout,policy.budget())',
      ('src/boxed/slice.rs','drop'):'AllocationGuard(self.ptr.cast(),self.layout,self.policy.budget())',
      ('src/vec.rs','drop'):'Allocation(self.ptr.cast(),self.layout(),self.policy.budget())',
    }
    guards_seen=set()
    for row in rows:
        if row['kind']!='protocol_call' or row['detail']['callee'] not in ('AllocationGuard','Allocation'):continue
        key=(row['file'],row['function'])
        assert key in guard_calls and key not in guards_seen and canonical(row['expression'])==guard_calls[key],('allocation guard identity/layout mismatch',row)
        guards_seen.add(key);bindings.append(dict(site=row,contract=proof('transferred_credit'),role='allocation-guard'))
    assert guards_seen==set(guard_calls),('missing allocation guard',set(guard_calls)-guards_seen)

    # Constructor field expressions are checked separately from writes: new
    # fields and alternate factories cannot inherit an existing certificate.
    factories={
      ('Buffer','new') : ('empty_buffer',dict(ptr='NonNull::dangling()',len='Checked::new(0)',cap='Checked::new(0)',layout='Layout::new::<[T;0]>()',policy='policy')),
      ('MemoryBudget','new'):('ledger_new',dict(limit='limit',occupied='Cell::new(0)',available='Cell::new(limit)')),
      ('MemoryBudget','reserve'):('ledger_reserve',dict(budget='self',bytes='bytes')),
      ('Reservation','commit'):('ledger_commit',dict(budget='self.budget',bytes='bytes')),
      ('CallRelease','new'):('transferred_credit',dict(policy='policy',bytes='bytes')),
      ('InitSlot','new'):('prefix_storage_connection',dict(ptr='ptr',initialized='false',marker='PhantomData')),
      ('Compaction','full'):('c_full',dict(read='Checked::new(0)',write='Checked::new(0)',original='Checked::new(original)')),
      ('Compaction','first_kept'):('c_first_kept',dict(read='Checked::new(first)',write='Checked::new(first)',original='Checked::new(original)')),
      ('Compaction','try_prefix'):('c_try_prefix',dict(read='Checked::new(start)',write='Checked::new(start)',original='Checked::new(original)')),
      ('Extraction','try_range'):('e_try_range',dict(read='Checked::new(start)',deleted='Checked::new(0)',end='Checked::new(end)',total='Checked::new(total)')),
      ('Span','full'):('kani:full_constructor_establishes_order',dict(front='Checked::new(0)',back='Checked::new(end)')),
      ('Span','try_new'):('kani:both_transitions_preserve_order_and_exact_length',dict(front='Checked::new(front)',back='Checked::new(back)')),
    }
    fields={}
    for row in rows:
        if row['kind']!='field':continue
        d=row['detail'];ctor=compact(d['constructor']);ctx=compact(row['context']).split('<')[0];fn=row['function']
        if ctor in ('Rejected','Error::Allocation','Error::Budget'):continue
        identity=(row['file'],row['context'],fn,ctor)
        fields.setdefault(identity,[]).append(row)
    for (f,context,fn,ctor),items in fields.items():
        ctx=compact(context).split('<')[0]
        actual={compact(x['detail']['field']):compact(x['detail']['value']) for x in items}
        assert len(actual)==len(items),('duplicate fields',items)
        if ctor in ('Self','Reservation','Charge') and (ctx,fn) in factories:
            contract,expected=factories[(ctx,fn)];assert actual==expected,(f,ctx,fn,actual,expected)
        elif ctx=='Vec' and ctor=='Self':
            if fn in ('new','empty'):contract='empty_owner'
            elif fn=='from_raw_parts':contract='resized_owner'
            elif fn=='try_from_std_in':contract='adopted_owner'
            elif fn=='from_boxed_slice':contract='boxed_to_owner'
            else:raise AssertionError(('unbound Vec factory',items))
            assert set(actual)=={'ptr','len','cap','policy','limit','marker'},items
            assert actual['marker']=='PhantomData',items
            assert actual['policy']==('Unmetered' if fn=='new' else 'policy'),items
            assert actual['limit']==('Unlimited' if fn in ('new','from_boxed_slice') else 'limit'),items
            if fn in ('new','empty'):
                assert actual['ptr']=='NonNull::dangling()' and actual['len']=='Checked::new(0)',items
            else:
                assert actual['ptr']==('ptr.cast()' if fn=='from_boxed_slice' else 'ptr'),items
                assert actual['len']=='Checked::new(len)',items
                assert actual['cap']==('unsafe{crate::capacity::Capacity::from_validated_raw(capacity)}' if fn=='from_raw_parts' else 'capacity'),items
        elif ctx=='Box' and ctor=='Self':
            assert fn in ('try_new_in','try_init_in','try_clone','from_raw_parts') and actual==dict(ptr='ptr',policy='policy',marker='PhantomData'),items
            contract='allocation_credit' if fn!='from_raw_parts' else 'transferred_credit'
        elif ctx=='Capacity' and ctor=='Self':
            assert fn in ('empty','try_new','for_slice','from_validated_raw'),items
            assert set(actual)=={'count','element'} and actual['element']=='PhantomData',items
            expected='Checked::new(logical_capacity(mem::size_of::<T>(),0,limit))' if fn=='empty' else 'Checked::new(count)'
            assert actual['count']==expected,items
            contract='empty_owner' if fn=='empty' else 'boxed_to_owner' if fn=='for_slice' else 'capacity_bytes'
        elif ctor == 'Prefix':
            assert f=='src/init/prefix.rs' and fn=='initialize', items
            assert actual==dict(ptr='ptr',state='Checked::new(0)'), items
            contract='prefix_storage_connection'
        elif ctor == 'Commit':
            assert f=='src/init/collection.rs' and fn=='fill' and not ctx, items
            assert actual==dict(output='state',current='&mutcurrent'), items
            contract='kani:local_cursor_publication_preserves_every_word'
        elif ctor=='SuffixGuard':
            assert actual==dict(owner='self',original='original'),items
            assert fn in ('try_resize_with_init','try_extend_fallible','try_extend_from_within','try_resize_owned','try_write_fmt'),items
            contract='kani:suffix_guard_restores_every_admitted_original_length'
        elif ctor=='Compact':
            assert fn in ('try_retain_mut','try_dedup_by'),items
            expected='Compaction::full(original)' if fn=='try_retain_mut' else 'Compaction::first_kept(original)'
            assert actual==dict(owner='self',state=expected),('compaction initial state',items)
            contract='retained_loop' if fn=='try_retain_mut' else 'dedup_loop'
        elif ctor=='Drain':
            assert actual==dict(owner='self',span='span',tail_start='end',tail_len='tail_len') and fn=='try_drain',items
            contract='drain_constructor'
        elif ctor=='ExtractIf':
            assert actual==dict(owner='self',filter='filter',state='state') and fn=='try_extract_if',items
            contract='e_try_range'
        elif ctor=='Splice':
            assert actual==dict(drain='self.try_drain(range)?',replacement='replacement'),items
            contract='complete_splice'
        elif ctor=='IntoIter':
            assert fn=='into_iter',items
            assert actual==dict(owner='self',span='crate::span::Span::full(back)') or actual==dict(ptr='ptr.cast()',span='crate::span::Span::full(ptr.len())',layout='layout',policy='policy',marker='PhantomData'),items
            contract='cursor_before_read'
        elif ctor=='CharBytes':
            assert actual==dict(iter='source.try_into_iter().map_err(InitError::Initialize)?',bytes='[0;4]',position='0',len='0'),items
            contract='utf8_byte_cursor'
        elif ctor=='Writer':
            assert actual==dict(owner='guard.owner',error='None'),items
            contract='extend_operation'
        elif ctor in ('Take','FromStd','FromResults'):
            assert set(actual)<= {'iter','remaining'},items
            if ctor=='Take':assert actual['remaining']=='Checked::new(count)',items
            contract='buffer_span'
        elif ctor in ('Pinned','Self') and ctx in ('Box','Pinned'):
            assert set(actual)=={'inner'},items
            contract='transferred_credit'
        else:raise AssertionError(('unbound state constructor',items))
        for row in items:bindings.append(dict(site=row,contract=proof(contract),role='state-construction'))
    # Close the entry conditions of the two private capacity helpers. All other
    # public numeric operations accept arbitrary words and return a refusal.
    helper_expected={
      ('src/vec.rs','grow'):['self.grow_to(needed,exact)'],
      ('src/vec.rs','reserve_length'):['self.grow_to(needed,false)'],
      ('src/vec.rs','grow_to'):['self.resize_storage(cap)'],
      ('src/vec.rs','try_shrink_to'):['self.resize_storage(cap)'],
    }
    helper_seen={}
    for row in rows:
        if row['kind']!='private_helper':continue
        key=(row['file'],row['function']);assert key in helper_expected,('unbound private-capacity caller',row)
        n=helper_seen.get(key,0);expected=helper_expected[key]
        assert n<len(expected) and canonical(row['expression'])==expected[n],('private helper argument mismatch',row)
        helper_seen[key]=n+1
        bindings.append(dict(site=row,contract=proof('memory_reserve' if row['detail']['method']=='grow_to' else 'resized_owner'),role='private-helper'))
    for key,expected in helper_expected.items():assert helper_seen.get(key)==len(expected),('missing helper caller',key)

    # These phase facts are producer postconditions, not fresh assumptions at a
    # memory call. Check the real producer/branch and reject variable shadowing.
    from verify_edit_loops import block
    def body(file,marker):return compact(re.sub(r'//[^\n]*','',block((root/file).read_text(),marker)))
    v=(root/'src/vec.rs').read_text()
    assert body('src/vec.rs','fn grow(')=='letneeded=required_len(self.len.get(),additional,self.max_len())?;self.grow_to(needed,exact)'
    shrink=body('src/vec.rs','pub fn try_shrink_to(')
    assert 'letcap=self.len.get().max(minimum_capacity);ifcap<self.cap.get(){self.resize_storage(cap)?;}' in shrink
    array=body('src/vec/convenience.rs','pub fn try_from_array_in')
    assert 'letmutresult=Self::empty(policy,limit);' in array and 'result.try_reserve_exact(N)' in array,'unbound array reservation'
    imported=body('src/vec/ownership.rs','pub fn from_boxed_slice(')
    assert 'letlen=source.len();' in imported and 'letcapacity=crate::capacity::Capacity::for_slice(source.as_slice(),usize::MAX);' in imported
    for method,argument in [('try_init_slice_in','len'),('try_copy_from_slice_in','source.len()'),
                            ('try_new_uninit_slice_in','len'),('try_new_zeroed_slice_in','len')]:
        source=body('src/boxed/slice.rs','pub fn '+method)
        assert 'letlayout=crate::layout::array::<T>('+argument+')' in source,('unbound allocated extent',method)
        assert source.count('letlayout=')==1,('shadowed allocation layout',method)
    reserve=body('src/boxed/buffer.rs','fn reserve(')
    assert 'letlayout=crate::layout::array::<T>(proposed)?;' in reserve and reserve.count('letlayout=')==1
    finish=body('src/boxed/buffer.rs','fn finish(')
    assert 'ifself.cap.get()!=self.len.get(){letlayout=crate::layout::array::<T>(self.len.get())?;' in finish
    assert finish.count('self.cap.get()')==1 and finish.count('letlayout=')==1
    adopt=body('src/boxed/owned.rs','pub fn try_from_std_in(')
    assert 'letlayout=Layout::for_value(value.as_ref());' in adopt and adopt.count('letlayout=')==1
    adopt_slice=body('src/boxed/slice.rs','pub fn try_from_std_vec_in(')
    assert 'letlen=value.len();' in adopt_slice and 'letold=matchcrate::layout::array::<T>(value.capacity())' in adopt_slice
    assert 'letnew=matchcrate::layout::array::<T>(len)' in adopt_slice
    exported=body('src/vec/ownership.rs','pub fn into_std(')
    assert 'letbytes=self.layout().size();' in exported and exported.count('letbytes=')==1
    call=compact((root/'src/boxed/call.rs').read_text())
    assert 'let__bytes=$crate::boxed::Box::allocation_layout(&__owner).size();' in call
    assert 'let(__ptr,__policy)=$crate::boxed::Box::into_raw_parts(__owner);' in call
    # The sealed built-in policies and scoped references retain their identity. A new policy implementation
    # must establish its own fixed-limit/accounting-identity contract.
    budget=(root/'src/budget.rs').read_text()
    assert 'letavailable=budget_numeric::reserve(self.available(),bytes)?;' in compact(budget),'unbound local admission result'
    assert compact(block(budget,'mod sealed'))=='pubtraitSealed{}implSealedforsuper::Budget<\'_>{}implSealedforsuper::Unmetered{}#[cfg(feature="alloc")]implSealedforsuper::OwnedBudget{}#[cfg(all(feature="alloc",target_has_atomic="ptr"))]implSealedforsuper::OwnedConcurrentBudget{}impl<P:super::AllocationPolicy>Sealedfor&P{}#[cfg(target_has_atomic="ptr")]implSealedforsuper::ConcurrentBudget<\'_>{}'
    limited_policy=budget[budget.index("impl AllocationPolicy for Budget<'_>"):]
    unmetered_policy=budget[budget.index('impl AllocationPolicy for Unmetered'):]
    assert compact(block(limited_policy,'fn budget('))=='match*self{Budget::Limited(b)=>Accounting::Local(b),Budget::Unmetered=>Accounting::Unmetered,}','allocation policy identity changed'
    assert compact(block(unmetered_policy,'fn budget('))=='Accounting::Unmetered','unmetered policy changed'
    assert compact(block(v,'mod sealed'))=='#[cfg(verus_keep_ghost)]usevstd::prelude::*;#[cfg_attr(verus_keep_ghost,verus_verify)]pubtraitSealed{}#[cfg_attr(verus_keep_ghost,verus_verify)]implSealedforsuper::Unlimited{}#[cfg_attr(verus_keep_ghost,verus_verify)]implSealedforsuper::LengthLimit{}'
    unlimited_impl=v[v.index('impl LengthPolicy for Unlimited'):]
    assert compact(block(unlimited_impl,'fn max_len(self)'))=='usize::MAX'
    length_impl=v[v.index('impl LengthPolicy for LengthLimit'):]
    assert compact(block(length_impl,'fn max_len(self)'))=='self.0'
    for getter,value in [('len','self.len.get()'),('capacity','self.cap.get()'),('max_len','self.limit.max_len()')]:
        assert body('src/vec.rs','pub fn '+getter+'(&self)')==value,('unbound Vec getter',getter)
    # Primitive bulk cloning delegates to the already proved Copy append. No
    # callback success can publish an uninitialized prefix. Other user TryClone
    # implementations remain returning-callback contracts, as for clone_into.
    cloning=(root/'src/clone.rs').read_text()
    hooks=list(re.finditer(r'fn\s+try_extend_vec\s*<',cloning))
    assert len(hooks)==3, 'bulk clone hook set changed'
    hook_bodies=[compact(block(cloning[m.start():],'fn try_extend_vec')) for m in hooks]
    assert hook_bodies==['None','Some(destination.try_extend_copy(source))','Some(destination.try_extend_copy(source))'], 'bulk clone hook changed'
    dispatch=body('src/vec.rs','pub fn try_extend_cloned')
    assert dispatch.startswith('letend=required_len(self.len.get(),source.len(),self.max_len())?;ifletSome(result)=T::try_extend_vec(source,self){returnresult;}'), 'bulk clone dispatch changed'
    bindings.append(dict(site=dict(file='src/clone.rs',function='try_extend_vec'),
        contract=proof('commit_reserved_owner'),role='safe-bulk-clone-dispatch'))
    capacity=(root/'src/capacity.rs').read_text()
    assert compact(block(capacity,'fn get(self)'))=='self.count.get()'
    slices=(root/'src/boxed/slice.rs').read_text()
    assert compact(block(slices,'pub fn len(&self)'))=='self.ptr.len()'
    boxed=(root/'src/boxed.rs').read_text()
    assert 'letlayout=Layout::for_value(self.as_ref());' in compact(block(boxed[boxed.index('Drop for Box'):],'fn drop('))
    assert 'letlayout=Layout::for_value(self.as_slice());' in compact(block(slices[slices.index('IntoIterator for Box'):],'fn into_iter('))
    for row in unsafe_rows:
        if row['kind']!='unsafe_block':continue
        containing=[r for r in rows if r['kind']=='body' and r['file']==row['file']
            and r['context']==row['context'] and r['function']==row['function']
            and (r['line'],r['column'])<=(row['line'],row['column'])
            and (row['end_line'],row['end_column'])<=(r['end_line'],r['end_column'])]
        assert len(containing)==1,('missing caller body',row)
        source=canonical(containing[0]['expression']);fn=row['function'];f=row['file']
        if fn in ('next','next_back') and 'self.span.' in compact(row['expression']):
            producer='self.span.'+('next_back' if fn=='next_back' else 'next')+'()?'
            assert re.search(r'let(?:i|index)='+re.escape(producer)+';',source),('unbound span index producer',row)
            assert len(re.findall(r'let(?:i|index)=',source))==1,('shadowed span index',row)
        if f=='src/vec/edit.rs' and fn=='fmt':
            assert 'ifself.state.read()<self.state.end(){' in source,('missing extraction peek guard',row)
        if f=='src/vec/convenience.rs' and fn=='try_push_mut':
            assert 'letindex=self.len.get();self.try_push(value)?;' in source and source.count('letindex=')==1
        if f=='src/vec/convenience.rs' and fn=='try_insert_mut':assert 'self.try_insert(index,value)?;' in source
        if fn=='try_into_array':
            expected='self.len.get()!=N' if f=='src/vec/convenience.rs' else 'self.len()!=N'
            assert 'if'+expected+'{' in source,('missing checked array length',row)
    # Every unsafe site must be routed. Numeric sites select the actual scalar
    # proof; representation-only sites use the typed-layout/ownership boundary.
    for row in unsafe_rows:
        if row['kind'] not in ('unsafe_block','macro_unsafe_block'):continue
        f,fn=row['file'],row['function'];group=row.get('group','')
        if (f,fn) in WRITES:contract=WRITES[(f,fn)][0]
        elif group in ('G10','G11','G12','G09','G05'):contract='transferred_credit'
        elif f=='src/vec/edit.rs' and fn=='fill':contract='splice_fill_loop'
        elif f=='src/vec/edit.rs':
            contract='extraction_before_move' if row['context'].startswith('ExtractIf') else 'compaction_before_move' if row['context'].startswith('Compact') else 'cursor_before_read'
        elif f in ('src/vec/ownership.rs','src/boxed/slice.rs') and fn in ('next','next_back','drop','as_slice','as_mut_slice'):contract='cursor_before_read'
        elif f=='src/vec/ownership.rs' and fn=='try_clone_in':contract='prefix_storage_connection'
        elif f=='src/storage.rs':contract='allocation_credit' if fn=='allocate_impl' else 'released_credit' if fn=='deallocate' else 'reallocation_credit'
        elif f=='src/capacity.rs':contract='capacity_bytes'
        elif f=='src/init/collection.rs':contract='buffer_span'
        elif f in ('src/clone.rs','src/init.rs','src/init/prefix.rs'):contract='prefix_storage_connection'
        elif f in ('src/boxed.rs','src/boxed/owned.rs','src/boxed/slice.rs'):contract='boxed_to_owner'
        elif f=='src/boxed/buffer.rs':contract='buffer_finish' if fn=='finish' else 'buffer_collect'
        elif f in ('src/vec.rs','src/vec/convenience.rs'):contract='checked_index_before_access' if fn=='try_extend_cloned' and 'copy_nonoverlapping' in row['expression'] else 'initialized_operation' if fn in ('try_extend_cloned','try_clone_in') else 'spare_view_before_slice' if fn in ('spare_capacity_mut','split_at_spare_mut') else 'checked_index_before_access'
        else:raise AssertionError(('unbound unsafe caller',row))
        bindings.append(dict(site=row,contract=proof(contract),role='typed-memory-boundary'))
    return dict(bindings=bindings,state_writes=sum(x['role']=='state-publication' for x in bindings),
                constructor_fields=sum(x['role']=='state-construction' for x in bindings),
                unsafe_callers=sum(x['role']=='typed-memory-boundary' for x in bindings),
                boundary='Source-projection numeric/guard certificate under valid typed storage, allocator, safe Rust ownership and explicit public-unsafe/callback contracts; not a Rust provenance theorem.')


def inventory(root,out):
    root=Path(root);out=Path(out);out.mkdir(parents=True,exist_ok=True)
    with (out/'state-parser.log').open('w') as log:
        subprocess.run(['cargo','build','--locked','--offline','--manifest-path',str(ROOT/'tools/panic-audit/Cargo.toml'),
                        '--bin','state_sites','--target-dir',str(ROOT/'target/panic-inventory')],cwd=ROOT,stdout=log,stderr=log,check=True)
    files=sorted(str(p.relative_to(root)) for p in (root/'src').rglob('*.rs') if not p.name.endswith('proofs.rs'))
    rows=json.loads(subprocess.check_output([str(scanner('state_sites')),*files],cwd=root,text=True))
    (out/'state-sites.json').write_text(json.dumps(rows,indent=2)+'\n')
    return rows
