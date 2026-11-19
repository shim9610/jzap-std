"""Build source-bound owner transition models; no theorem is assumed here."""
import re
from pathlib import Path
from verify_edit_loops import block


def compact(s): return re.sub(r"\s+", "", s)


def generate(root):
    root=Path(root);vector=(root/'src/vec.rs').read_text()
    convenience=(root/'src/vec/convenience.rs').read_text()
    ownership=(root/'src/vec/ownership.rs').read_text()
    buffer=(root/'src/boxed/buffer.rs').read_text()
    expressions={}
    empty=block(vector,'fn empty(')
    assert compact(empty)=="Self{ptr:NonNull::dangling(),len:Checked::new(0),cap:crate::capacity::Capacity::empty(limit.max_len()),policy,limit,marker:PhantomData,}"
    expected_new=compact(empty).replace('limit.max_len()','usize::MAX').replace('policy,','policy:Unmetered,').replace('limit,','limit:Unlimited,')
    assert compact(block(vector[vector.index('impl<T> Vec<T> {'):],'pub const fn new('))==expected_new
    for marker,expected in [('pub fn new_in(', 'Self::empty(policy,Unlimited)'),('pub fn with_limit_in(', 'Vec::empty(policy,LengthLimit(max_len))')]:
        assert compact(block(vector,marker))==expected
    push=compact(re.sub(r'//[^\n]*','',block(convenience,'pub fn push_within_capacity(')))
    assert push=='letnext=matchgeometry::append_cursor(self.len,self.cap.get()){Ok(next)=>next,Err(error)=>{returnErr(Rejected{value,error});}};letplace=unsafe{self.ptr.as_ptr().add(self.len.get())};unsafe{place.write(value);}self.len=next;Ok(unsafe{&mut*place})','unbound within-capacity publication'
    truncate=compact(re.sub(r'//[^\n]*','',block(vector,'pub fn truncate(')))
    assert truncate=='letnext=Checked::new(len);letSome(removed)=geometry::removed_suffix(self.len,next)else{return;};self.len=next;if!mem::needs_drop::<T>(){return;}unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr().add(len),removed.get(),))};','unbound removed-suffix caller'
    setlen=block(convenience,'pub unsafe fn try_set_len(')
    assert compact(setlen)=='required_len(0,new_len,self.cap.get())?;self.len=Checked::new(new_len);Ok(())', 'unbound initialized-length admission'
    capacity_source=(root/'src/capacity.rs').read_text()
    cap_empty=block(capacity_source,'pub(crate) const fn empty(')
    assert compact(cap_empty)=='Self{count:Checked::new(logical_capacity(mem::size_of::<T>(),0,limit)),element:PhantomData,}', 'unbound empty capacity arguments'
    cap_slice=block(capacity_source,'pub(crate) fn for_slice(')
    assert compact(re.sub(r'//[^\n]*','',cap_slice))=='letcount=logical_capacity(mem::size_of::<T>(),values.len(),limit);Self{count:Checked::new(count),element:PhantomData,}', 'unbound slice capacity arguments'
    # All single/bulk reserved commits publish precisely the already-proved
    # reservation result, never a newly wrapped sum after a memory transfer.
    for file,methods in [(vector,['try_insert','try_extend_copy','try_append']),
                         (convenience,['try_extend_copy_from_within','try_write_vectored'])]:
        for name in methods:
            text=block(file,'pub fn '+name)
            assert text.count('self.len = Checked::new(needed);')==1,name
            assert 'self.reserve_length(' in text,name
    expressions['RESERVED_COMMIT']=re.search(r'self.len = Checked::new\((needed)\)',block(vector,'pub fn try_extend_copy')).group(1)
    expressions['APPEND_SOURCE_COMMIT']=re.search(r'other.len = Checked::new\((.*?)\)',block(vector,'pub fn try_append')).group(1)
    split=block(vector,'pub fn try_split_off(')
    expressions['SPLIT_OWNER_REJECT']=re.search(r'if (.*?) \{',split).group(1).replace('self.len.get()','original.len')
    assert 'let mut tail = Self::empty(self.policy.try_clone()?, self.limit);' in split
    assert 'tail.try_reserve_exact(len)?;' in split
    expressions['SPLIT_TAIL_COMMIT']=re.search(r'tail.len = Checked::new\((.*?)\)',split).group(1).replace('len','count')
    expressions['SPLIT_PREFIX_COMMIT']=re.search(r'self.len = Checked::new\((.*?)\)',split).group(1)
    adopt=block(ownership,'pub fn try_from_std_in(')
    prefix=compact(adopt[:adopt.index('let (capacity, layout)')])
    expected='letcapacity=matchcrate::capacity::adopted_capacity(mem::size_of::<T>(),source.len(),source.capacity(),limit.max_len(),){Ok(capacity)=>capacity,Err(error)=>{returnErr(Rejected{value:source,error,});}};'
    assert prefix==expected,'unbound standard adoption shape arguments'
    ordered=['Capacity::try_new(capacity)','policy.budget().reserve(layout.size())','let ptr = unsafe','r.commit().retain()','mem::forget(source)','Ok(Self {']
    indices=[adopt.index(token) for token in ordered];assert indices==sorted(indices)
    assert 'len: Checked::new(len)' in adopt and 'cap: capacity' in adopt
    # Declaration/call binding only. The maintained adapter calls the runtime
    # shape function; neither the body nor its arithmetic is projected.
    flat=compact(re.sub(r'//[^\n]*','',block(convenience,'pub fn try_into_flattened(')))
    expected='letSome((len,cap))=super::geometry::flatten(self.len.get(),self.cap.get(),self.max_len(),mem::size_of::<T>(),N,)else{returnErr(Rejected{value:self,error:Error::Capacity,});};'
    assert flat.startswith(expected), 'Unbound flattened shape arguments'
    assert flat[len(expected):]=='let(ptr,_,_,policy,limit)=self.into_raw_parts();Ok(unsafe{Vec::from_raw_parts(ptr.cast(),len,cap,policy,limit)})', 'Unbound flattened ownership transfer'
    from init_bindings import check as check_init
    check_init(root)
    vectored=block(convenience,'pub fn try_write_vectored(')
    assert 'let mut total = Checked::new(0usize);' in vectored
    assert vectored.count('for part in parts {')==2
    sum_loop=block(vectored,'for part in parts {')
    assert compact(sum_loop)=='total=(total+Checked::new(part.len())).map_err(|_|Error::Capacity)?;'
    assert 'let needed = self.reserve_length(total.get())?;' in vectored
    assert 'core::ptr::copy_nonoverlapping(part.as_ptr(), destination, part.len());' in vectored
    destination=re.search(r'let mut destination = unsafe \{ self.ptr.as_ptr\(\).add\((.*?)\) \};',vectored).group(1).replace('self.len.get()','s.len')
    advance=re.search(r'destination = unsafe \{ destination.add\((.*?)\) \};',vectored).group(1).replace('part.len()','part')
    committed=re.search(r'self.len = Checked::new\((.*?)\)',vectored).group(1)
    expressions['VECTORED_BODY']='''let mut total=0usize;let mut i=0usize;
        while i<parts.len()
            invariant i<=parts.len(),total as int==piece_sum(parts@,i as int),owner_valid(s,1),
            decreases parts.len()-i,
        {
            let part=parts[i];
            let Some(next)=total.checked_add(part) else{return None;};
            total=next;i+=1;
        }
        let initial_len=s.len;
        let Some((mut next,needed))=reserve_owner(s,1,total,false,accepted) else{return None;};
        let mut destination='''+destination.replace('s.len','initial_len')+''';i=0;
        while i<parts.len()
            invariant i<=parts.len(),owner_valid(next,1),next.len==s.len,next.limit==s.limit,
                needed as int==s.len as int+total as int,needed<=next.cap,
                destination as int==s.len as int+piece_sum(parts@,i as int),
                total as int==piece_sum(parts@,parts.len() as int),
            decreases parts.len()-i,
        {
            let part=parts[i];
            proof {
                piece_sum_monotone(parts@,i as int+1,parts.len() as int);
                memory_span(next.cap,1,destination as int,part as int);
            }
            destination=destination+'''+advance+''';i+=1;
        }
        next.len='''+committed+''';Some((next,total))'''
    template=(root/'tools/memory-proof/owner-state.verus.rs').read_text()
    for key,value in expressions.items():template=re.sub(r'\b'+key+r'\b',lambda _:value,template)
    assert not re.search(r'\b(?:EMPTY|SET_LEN|FLATTEN|BUFFER_RESERVE|RESERVED|APPEND_SOURCE|SPLIT_OWNER|SPLIT_TAIL|SPLIT_PREFIX|ADOPT)_[A-Z_]+\b',template)
    return template,expressions
