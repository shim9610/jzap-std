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
    extent=(root/'src/extent.rs').read_text()
    expressions={}
    # The owner holds its words in the closed extent; the empty words are the
    # two verified constructors, which differ only in the limit.
    empty=block(vector,'fn empty(')
    assert compact(empty)=="Self{ptr:NonNull::dangling(),extent:crate::extent::Extent::empty(limit),policy,marker:PhantomData,}"
    expected_new=compact(empty).replace('Extent::empty(limit)','Extent::unlimited()').replace('policy,','policy:Unmetered,')
    assert compact(block(vector[vector.index('impl<T> Vec<T> {'):],'pub const fn new('))==expected_new
    empty_words=compact(block(extent,'pub(crate) fn empty('))
    assert empty_words=='Self{len:Checked::new(0),cap:Capacity::empty(limit.max_len()),limit,}', 'unbound empty owner words'
    assert compact(block(extent,'pub(crate) const fn unlimited('))==empty_words.replace('limit.max_len()','usize::MAX').replace('limit,','limit:Unlimited,'), 'unbound unlimited owner words'
    for marker,expected in [('pub fn new_in(', 'Self::empty(policy,Unlimited)'),('pub fn with_limit_in(', 'Vec::empty(policy,LengthLimit(max_len))')]:
        assert compact(block(vector,marker))==expected
    push=compact(re.sub(r'//[^\n]*','',block(convenience,'pub fn push_within_capacity(')))
    assert push=='letnext=matchgeometry::append_cursor(self.extent.length(),self.extent.cap()){Ok(next)=>next,Err(error)=>{returnErr(Rejected{value,error});}};letplace=unsafe{self.ptr.as_ptr().add(self.extent.len())};unsafe{place.write(value);}ifletErr(error)=self.extent.set_len_within(next.get()){letvalue=unsafe{place.read()};returnErr(Rejected{value,error});}Ok(unsafe{&mut*place})','unbound within-capacity publication'
    truncate=compact(re.sub(r'//[^\n]*','',block(vector,'pub fn truncate(')))
    assert truncate=='letnext=Checked::new(len);letSome(removed)=geometry::removed_suffix(self.extent.length(),next)else{return;};self.extent.shorten_to(next.get());if!mem::needs_drop::<T>(){return;}unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr().add(len),removed.get(),))};','unbound removed-suffix caller'
    setlen=block(convenience,'pub unsafe fn try_set_len(')
    # The admission is the verified length transition itself.
    assert compact(setlen)=='self.extent.set_len_within(new_len)', 'unbound initialized-length admission'
    capacity_source=(root/'src/capacity.rs').read_text()
    cap_empty=block(capacity_source,'pub(crate) const fn empty(')
    assert compact(cap_empty)=='Self{count:Checked::new(logical_capacity(mem::size_of::<T>(),0,limit)),element:PhantomData,}', 'unbound empty capacity arguments'
    cap_slice=block(capacity_source,'pub(crate) fn for_slice(')
    assert compact(re.sub(r'//[^\n]*','',cap_slice))=='letcount=logical_capacity(mem::size_of::<T>(),values.len(),limit);unsafe{Self::from_validated_raw(count)}', 'unbound slice capacity arguments'
    # All single/bulk reserved commits publish precisely the already-proved
    # reservation result, never a newly wrapped sum after a memory transfer.
    for file,methods in [(vector,['try_insert','try_extend_copy','try_append']),
                         (convenience,['try_extend_copy_from_within','try_write_vectored'])]:
        for name in methods:
            text=block(file,'pub fn '+name)
            # The reservation result is admitted by the length transition
            # before the memory transfer; nothing else publishes a length.
            assert text.count('self.extent.set_len_within(needed)')==1 and text.count('set_len_within(')==1,name
            assert text.index('self.reserve_length(')<text.index('self.extent.set_len_within(needed)')<text.index('core::ptr::copy'),name
    expressions['RESERVED_COMMIT']=re.search(r'self.extent.set_len_within\((needed)\)',block(vector,'pub fn try_extend_copy')).group(1)
    expressions['APPEND_SOURCE_COMMIT']=re.search(r'other.extent.shorten_to\((.*?)\)',block(vector,'pub fn try_append')).group(1)
    split=block(vector,'pub fn try_split_off(')
    expressions['SPLIT_OWNER_REJECT']=re.search(r'if (.*?) \{',split).group(1).replace('self.extent.len()','original.len')
    assert 'let mut tail = Self::empty(self.policy.try_clone()?, self.extent.limit());' in split
    assert 'tail.try_reserve_exact(len)?;' in split
    expressions['SPLIT_TAIL_COMMIT']=re.search(r'tail.extent.set_len_within\((.*?)\)',split).group(1).replace('len','count')
    expressions['SPLIT_PREFIX_COMMIT']=re.search(r'self.extent.shorten_to\((.*?)\)',split).group(1)
    adopt=block(ownership,'pub fn try_from_std_in(')
    prefix=compact(adopt[:adopt.index('let (capacity, layout)')])
    expected='letcapacity=matchcrate::capacity::adopted_capacity(mem::size_of::<T>(),source.len(),source.capacity(),limit.max_len(),){Ok(capacity)=>capacity,Err(error)=>{returnErr(Rejected{value:source,error,});}};'
    assert prefix==expected,'unbound standard adoption shape arguments'
    ordered=['Capacity::try_new(capacity)','crate::extent::Extent::try_new(source.len(), capacity, limit)','policy.budget().reserve(layout.size())','let ptr = unsafe','r.commit().retain()','mem::forget(source)','Ok(Self {']
    indices=[adopt.index(token) for token in ordered];assert indices==sorted(indices)
    # The adopted words are the checked constructor's result, published whole.
    assert compact(adopt[adopt.index('Ok(Self {'):])=='Ok(Self{ptr,extent,policy,marker:PhantomData,})' and adopt.count('Extent::')==1
    # Declaration/call binding only. The maintained adapter calls the runtime
    # shape function; neither the body nor its arithmetic is projected.
    flat=compact(re.sub(r'//[^\n]*','',block(convenience,'pub fn try_into_flattened(')))
    expected='letSome((len,cap))=super::geometry::flatten(self.extent.len(),self.extent.cap(),self.max_len(),mem::size_of::<T>(),N,)else{returnErr(Rejected{value:self,error:Error::Capacity,});};'
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
    destination=re.search(r'let mut destination = unsafe \{ self.ptr.as_ptr\(\).add\((.*?)\) \};',vectored).group(1).replace('self.extent.len()','s.len')
    advance=re.search(r'destination = unsafe \{ destination.add\((.*?)\) \};',vectored).group(1).replace('part.len()','part')
    committed=re.search(r'self.extent.set_len_within\((.*?)\)',vectored).group(1)
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
