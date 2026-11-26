"""Source projections for complete initialization, buffer and splice operations.

Checked arithmetic is lowered to checked machine operations. Actual pointer
arguments are checked by the prerequisite span projections. New/unhandled source
syntax fails generation; a source digest alone never supplies a caller fact.
"""
import re
from pathlib import Path
from arithmetic_projection import bounded_projection, replace_once
from verify_edit_loops import block


def clean(text):
    text=re.sub(r'//[^\n]*','',text)
    return re.sub(r'#\[cfg_attr\([^\n]*\)\]','',text)


def compact(text):return re.sub(r'\s+','',clean(text))


def generate(root):
    root=Path(root);edit=(root/'src/vec/edit.rs').read_text()
    vector=(root/'src/vec.rs').read_text();buffer=(root/'src/boxed/buffer.rs').read_text()
    values={};bindings={}
    def bind(name,file,method,source):
        bindings[name]=dict(file=file,function=method,source=source)

    available=block(edit,'fn stage_excess(')
    formula=re.search(r'let available = (.*?)\s*\.get\(\);',available,re.S).group(1)
    available_formula=formula
    assert compact(formula)=='((Checked::new(self.drain.owner.max_len())-self.drain.owner.len).map_err(|_|Error::Capacity)?-Checked::new(self.drain.tail_len)).map_err(|_|Error::Capacity)?'
    values['HOLE_AVAILABLE']='h.limit-h.len-h.tail_len'
    bind('hole_available','src/vec/edit.rs','stage_excess',formula)

    move=clean(block(edit,'fn move_tail('))
    move=replace_once(move,'let required = required_len(self.drain.tail_len, additional, usize::MAX)?;',
        'let Ok(required)=needed_length(h.tail_len,additional,usize::MAX) else{return false;};')
    move=replace_once(move,'let next_tail = (Checked::new(self.drain.tail_start) + Checked::new(additional)).map_err(|_| Error::Capacity)?;',
        'let Some(next_tail)=h.tail.checked_add(additional) else{return false;};')
    move=replace_once(move,'self.drain.owner.try_reserve(required)?;',
        'if !hole_reserve(h,size,required,false,accepted){return false;}')
    pointer=block(move,'unsafe')
    assert compact(pointer)=='core::ptr::copy(self.drain.owner.ptr.as_ptr().add(self.drain.tail_start),self.drain.owner.ptr.as_ptr().add(next_tail.get()),self.drain.tail_len,);'
    move=replace_once(move,'unsafe {'+pointer+'}',
        'proof {memory_span(h.cap,size,h.tail as int,h.tail_len as int); memory_span(h.cap,size,next_tail as int,h.tail_len as int);}')
    move=replace_once(move,'self.drain.tail_start = next_tail.get();','h.tail=next_tail;')
    move=replace_once(move,'Ok(())','true')
    values['HOLE_MOVE']=move;bind('hole_move','src/vec/edit.rs','move_tail',move)

    restore=block(edit,'fn restore_tail(')
    formula=re.search(r'self.owner.len = Checked::new\((.*?)\);',restore,re.S).group(1)
    values['HOLE_RESTORE']=bounded_projection(formula).replace('self.owner.len.get()','h.len').replace('self.tail_len','h.tail_len')
    bind('hole_restore','src/vec/edit.rs','restore_tail',formula)

    drain=block(edit,'pub fn try_drain(')
    assert 'checked_range(range, self.len.get())?' in drain
    values['DRAIN_TAIL']=re.search(r'let tail_len = \(self.len - Checked::new\((.*?)\)\)',drain).group(1)
    values['DRAIN_TAIL']='len-('+values['DRAIN_TAIL']+')'
    values['DRAIN_LENGTH']=re.search(r'self.len = Checked::new\((.*?)\)',drain).group(1)
    values['DRAIN_START']=re.search(r'tail_start:\s*(\w+)',drain).group(1)
    bind('drain_constructor','src/vec/edit.rs','try_drain',drain)

    stage=clean(block(edit,'fn stage_excess('))
    # Keep the entire real post-collection control flow, including the exact
    # growth condition and all state publications after the two transfers.
    stage=stage[stage.index('let total = required_len('):]
    stage=replace_once(stage,'let total = required_len(self.drain.owner.len.get(),self.drain.tail_len,self.drain.owner.max_len(),)?;',
        'let Ok(total)=needed_length(h.len,h.tail_len,h.limit) else{return false;};')
    stage=replace_once(stage,'let needed = required_len(total, extra.len.get(), self.drain.owner.max_len())?;',
        'let Ok(needed)=needed_length(total,extra_len,h.limit) else{return false;};')
    stage=replace_once(stage,'self.drain.owner.grow((Checked::new(needed)-self.drain.owner.len).map_err(|_|Error::Capacity)?.get(),self.drain.owner.cap.get()!=initial_capacity,)?;',
        'let additional=needed-h.len; if !hole_reserve(h,size,additional,h.cap!=initial_capacity,accepted){return false;}')
    stage=replace_once(stage,'let next_tail = (Checked::new(self.drain.tail_start) + extra.len).map_err(|_| Error::Capacity)?;',
        'let Some(next_tail)=h.tail.checked_add(extra_len) else{return false;};')
    stage=replace_once(stage,'let next_len = (self.drain.owner.len + extra.len).map_err(|_| Error::Capacity)?;',
        'let Some(next_len)=h.len.checked_add(extra_len) else{return false;};')
    pointer=block(stage,'unsafe')
    assert compact(pointer)=='core::ptr::copy(self.drain.owner.ptr.as_ptr().add(self.drain.tail_start),self.drain.owner.ptr.as_ptr().add(next_tail.get()),self.drain.tail_len,);core::ptr::copy_nonoverlapping(extra.ptr.as_ptr(),self.drain.owner.ptr.as_ptr().add(self.drain.owner.len.get()),extra.len.get(),);'
    stage=replace_once(stage,'unsafe {'+pointer+'}',
        'staged_splice_before_moves(h.len,h.tail,h.tail_len,extra_len,h.cap,size); proof {memory_span(extra_cap,size,0,extra_len as int);}')
    stage=stage.replace('self.drain.owner.cap.get()','h.cap').replace('self.drain.tail_start = next_tail.get();','h.tail=next_tail;')
    stage=stage.replace('self.drain.owner.len = next_len;','h.len=next_len;')
    stage=replace_once(stage,'extra.len = Checked::new(0);','let extra_len=0usize;')
    stage=replace_once(stage,'Ok(())','true')
    values['STAGE_COMMIT']=stage;bind('stage_commit','src/vec/edit.rs','stage_excess',stage)

    finish=clean(block(edit,'pub fn try_finish('))
    finish=replace_once(finish,'self.drain.clear_remaining();','assert(hole_valid(*h,size));')
    loop=block(finish,'while let Some(value)')
    assert compact(loop)=='self.drain.owner.try_push(value).map_err(|e|e.error)?;'
    finish=replace_once(finish,'while let Some(value) = self.replacement.try_next().map_err(InitError::Initialize)? {'+loop+'}',
        '''while h.len<usize::MAX
            invariant hole_valid(*h,size),h.tail_len==0,old(h).tail_len==0,h.limit==old(h).limit,
                forall|i:usize| call_requires(*cb,(i,)),forall|i:usize| call_requires(*alloc,(i,)),
            decreases usize::MAX-h.len,
        {
            let present=match cb(h.len){Ok(p)=>p,Err(_)=>return false};
            if !present{return true;}
            if !hole_push(h,size,alloc(h.len)){return false;}
        }
        let present=match cb(h.len){Ok(p)=>p,Err(_)=>return false};
        if present{return hole_push(h,size,alloc(h.len));}''')
    finish=finish.replace('self.fill()?','(match hole_fill(h,size,cb){0=>true,1=>false,_=>return false})')
    next_pattern=r'let Some\(first\) = self.replacement.try_next\(\).map_err\(InitError::Initialize\)\? else \{\s*return Ok\(\(\)\);\s*\};'
    finish,n=re.subn(next_pattern,'let present=match cb(h.len){Ok(p)=>p,Err(_)=>return false}; if !present{return true;}',finish)
    assert n==2
    start=finish.index('let available =');end=finish.index('.get();',start)+len('.get();')
    assert compact(finish[start:end].removeprefix('let available =').removesuffix(';'))==compact(available_formula+'.get()')
    finish=finish[:start]+'let available=hole_available(*h,size);'+finish[end:]
    hint_start=finish.index('let lower =');hint_end=finish.index('.0;',hint_start)+3
    assert compact(finish[hint_start:hint_end])=='letlower=self.replacement.try_size_hint().map_err(InitError::Initialize)?.0;'
    finish=finish[:hint_start]+'if !hint_ok{return false;} let lower=hint;'+finish[hint_end:]
    finish=replace_once(finish,'let reserve = (Checked::new(self.drain.tail_len) + Checked::new(1)).map_err(|_| Error::Capacity)?;',
        'let Some(reserve)=h.tail_len.checked_add(1) else{return false;};')
    finish=replace_once(finish,'self.drain.owner.try_reserve(reserve.get())?;',
        'if !hole_reserve(h,size,reserve,false,alloc(h.len)){return false;}')
    finish=replace_once(finish,'((self.drain.owner.cap - self.drain.owner.len).map_err(|_| Error::Capacity)? - Checked::new(self.drain.tail_len)).map_err(|_| Error::Capacity)?.get()',
        'h.cap-h.len-h.tail_len')
    finish=replace_once(finish,'self.move_tail(additional)?;',
        'if !hole_move(h,size,additional,move_accepts){return false;}')
    finish=replace_once(finish,'let next = (self.drain.owner.len + Checked::new(1)).map_err(|_| Error::Capacity)?;',
        'let Some(next)=h.len.checked_add(1) else{return false;};')
    pointer=block(finish,'unsafe')
    assert compact(pointer)=='self.drain.owner.ptr.as_ptr().add(self.drain.owner.len.get()).write(first);'
    finish=replace_once(finish,'unsafe {'+pointer+'}','let _=observed_element(h.cap,size,h.len);')
    finish=finish.replace('self.drain.owner.cap.get()','h.cap').replace('self.drain.tail_len','h.tail_len')
    finish=finish.replace('self.drain.owner.len = next;','h.len=next;')
    finish=replace_once(finish,'self.stage_excess(first, initial_capacity)',
        'stage_operation(h,size,initial_capacity,cb,alloc,stage_accepts)')
    finish=finish.replace('return Ok(());','return true;').replace('return Err(InitError::Storage(Error::Capacity));','return false;')
    values['SPLICE_OPERATION']=finish;bind('splice_operation','src/vec/edit.rs','try_finish',finish)

    initialize=clean(block(vector,'pub fn try_resize_with_init'))
    loop=block(initialize,'while guard.owner.len.get() < new_len')
    loop=replace_once(loop,'let index = guard.owner.len.get();','let index=state.len;')
    loop=replace_once(loop,'let next = (guard.owner.len + Checked::new(1)).map_err(|_| InitError::Storage(Error::Capacity))?;',
        'let Some(next)=state.len.checked_add(1) else{state.len=original;return (false,state);};')
    loop=replace_once(loop,'let ptr = unsafe { NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(index)) };',
        'let ptr=observed_element(state.cap,size,index);')
    loop=replace_once(loop,'let mut slot = unsafe { InitSlot::new(ptr) };','let slot=ptr;')
    loop=replace_once(loop,'initialize(index, &mut slot).map_err(InitError::Initialize)?;',
        'let reply=cb(index); if reply!=1{state.len=original;return (false,state);}')
    loop=replace_once(loop,'slot.finish().map_err(InitError::Storage)?;','assert(reply==1);')
    loop=replace_once(loop,'guard.owner.len = next;','state.len=next;')
    values['INITIALIZED_OPERATION']='''if target<=s.len{return (true,OwnerState{len:target,..s});}
        let additional=target-s.len;let original=s.len;
        let Some((mut state,needed))=reserve_owner(s,size,additional,false,accepted) else{return (false,s);};
        assert(needed==target);
        while state.len<target
            invariant owner_valid(state,size),original==s.len,original<=state.len<=target<=state.cap,
                state.limit==s.limit,forall|i:usize| call_requires(*cb,(i,)),
            decreases target-state.len,
        {'''+loop+'''} (true,state)'''
    assert 'guard.owner.truncate' not in initialize and 'mem::forget(guard);' in initialize
    assert compact(block(vector[vector.index('Drop for SuffixGuard'):],'fn drop('))=='self.owner.truncate(self.original);'
    bind('initialized_operation','src/vec.rs','try_resize_with_init',initialize)

    # The spans execute the original shared cursor; only the surrounding growth
    # orchestration remains a separate caller model.
    from init_bindings import check as check_init
    check_init(root)
    for method in ['collect','collect_rest']:
        bind('buffer_'+method,'src/boxed/buffer.rs',method,block(buffer,'fn '+method+'<'))
    values['BUFFER_REST']='''let Some((mut len,mut cap))=buffer_push(initial,initial_cap,size,hint,alloc(initial)) else{return (false,initial,initial_cap);};
        while len<usize::MAX
            invariant initial<=len<=cap,byte_valid(cap,size),
                forall|i:usize| call_requires(*cb,(i,)),forall|i:usize| call_requires(*alloc,(i,)),
            decreases usize::MAX-len,
        {
            let before=len;let spare=cap-len;
            let (code,next)=buffer_span(len,cap,size,spare,cb);len=next;
            if code==2{return (false,len,cap);}
            if len<cap{return (true,len,cap);}
            let present=match cb(len){Ok(p)=>p,Err(_)=>return (false,len,cap)};
            if !present{return (true,len,cap);}
            let Some((next,next_cap))=buffer_push(len,cap,size,hint,alloc(len)) else{return (false,len,cap);};
            len=next;cap=next_cap;assert(before<len);
        }
        let present=match cb(len){Ok(p)=>p,Err(_)=>return (false,len,cap)};
        if !present{return (true,len,cap);}
        let result=buffer_push(len,cap,size,hint,alloc(len));assert(result.is_none());
        (false,len,cap)'''
    values['BUFFER_COLLECT']='''let Some((needed,cap))=buffer_reserve(initial,initial_cap,size,hint,hint,alloc(initial)) else{return (false,initial,initial_cap);};
        let (code,len)=buffer_span(initial,cap,size,hint,cb);
        if code==2{return (false,len,cap);}
        if len<needed{return (true,len,cap);}
        let present=match cb(len){Ok(p)=>p,Err(_)=>return (false,len,cap)};
        if !present{return (true,len,cap);}
        buffer_collect_rest(len,cap,size,hint,cb,alloc)'''
    for name,value in values.items():
        assert not re.search(r'\bunsafe\b|self\.|\?|Checked|InitError|\.ptr|\.get\(',value),(name,value)
    template=(root/'tools/memory-proof/operation-closure.verus.rs').read_text()
    for name,value in values.items():template=re.sub(r'\b'+name+r'\b',lambda _:value,template)
    assert not re.search(r'\b(?:HOLE|STAGE|SPLICE|DRAIN|INITIALIZED|BUFFER)_[A-Z_]+\b',template)
    return template,bindings
