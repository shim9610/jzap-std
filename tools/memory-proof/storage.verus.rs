// Address and count facts of one storage of equally sized elements, and the
// documented core condition (core-contracts.verus.rs) of each pointer
// operation on it. Every function is proved for all word-sized addresses,
// capacities, element sizes and alignments admitted by `raw_valid`. Callers:
// the Vec owner model (raw-owner.verus.rs) and the area models, which pass the
// index or range computed by the product kernels. That a product allocation is
// described by a `raw_valid` storage comes from the allocator result or a valid
// Rust borrow and is a stated premise; provenance, liveness, initialization
// and aliasing stay with the Kani and Miri checks.
verus! {
// A copyable descriptor of address/layout facts, not a unique ownership token.
#[derive(Clone,Copy)]
struct RawStorage { base:usize, cap:usize, size:usize, align:usize }
spec fn raw_valid(s:RawStorage)->bool {
    s.base>0 && is_pow2(s.align as int) && s.align>0
        && s.base as int % s.align as int==0
        && s.size as int % s.align as int==0
        && byte_valid(s.cap,s.size)
        && s.base as int+s.cap as int*s.size as int<=usize::MAX as int
}

// The storage descriptor as one allocation of the documented conditions.
spec fn allocation_of(s:RawStorage)->Allocation {
    Allocation{base:s.base,bytes:(s.cap as int*s.size as int) as usize}
}
proof fn storage_is_an_allocation(s:RawStorage)
    requires raw_valid(s),
    ensures allocation_wf(allocation_of(s)),
        allocation_of(s).bytes as int==s.cap as int*s.size as int,
{
    assert(0<=s.cap as int*s.size as int) by(nonlinear_arith);
}

fn dangling_storage(cap:usize,size:usize,align:usize)->(r:RawStorage)
    requires is_pow2(align as int),align>0,size as int%align as int==0,
        cap==0 || size==0,
    ensures raw_valid(r),r.cap==cap,r.size==size,r.align==align,
{
    proof {
        lemma_fundamental_div_mod(align as int,align as int);
        // Stated explicitly: the product of an empty extent is zero. Leaving it
        // to the solver's nonlinear search made this proof order-dependent.
        assert(cap as int*size as int==0) by(nonlinear_arith) requires cap==0 || size==0;
    }
    RawStorage{base:align,cap,size,align}
}

fn offset_storage(s:RawStorage,index:usize)->(address:usize)
    requires raw_valid(s),index<=s.cap,
    ensures address as int==s.base as int+index as int*s.size as int,
        address>0,address as int%s.align as int==0,
        doc_ptr_add(allocation_of(s),s.base,index,s.size),
        doc_non_null_new_unchecked(address),
{
    proof {
        storage_is_an_allocation(s);
        memory_span(s.cap,s.size,index as int,0);
        assert(0<=index as int*s.size as int) by(nonlinear_arith);
        assert(index as int*s.size as int<=s.cap as int*s.size as int) by(nonlinear_arith)
            requires index<=s.cap,0<=s.size;
    }
    let permitted=pointer_add_precondition(Ghost(allocation_of(s)),s.base,index,s.size);
    assert(permitted);
    let bytes=index*s.size;
    assert(bytes as int<=s.cap as int*s.size as int) by(nonlinear_arith)
        requires index<=s.cap,bytes as int==index as int*s.size as int,0<=s.size;
    let address=s.base+bytes;
    proof {
        lemma_fundamental_div_mod(s.base as int,s.align as int);
        lemma_fundamental_div_mod(s.size as int,s.align as int);
        let factor=s.base as int/s.align as int+index as int*(s.size as int/s.align as int);
        assert(address as int==s.align as int*factor) by(nonlinear_arith)
            requires s.base as int==s.align as int*(s.base as int/s.align as int),
                s.size as int==s.align as int*(s.size as int/s.align as int),
                address as int==s.base as int+index as int*s.size as int,
                factor==s.base as int/s.align as int+index as int*(s.size as int/s.align as int);
        lemma_mod_multiples_vanish(factor,0,s.align as int);
    }
    let aligned=alignment_mask_precondition(address,s.align);
    assert(aligned);
    address
}

// A typed pointer conversion, or a call whose arguments carry no element
// offset or count: the address is the non-null, aligned start of the storage.
fn representation_caller(s:RawStorage)->(address:usize)
    requires raw_valid(s),
    ensures address==s.base,aligned(address,s.align),
        doc_non_null_new_unchecked(address),
{
    offset_storage(s,0)
}

fn slice_caller(s:RawStorage,start:usize,count:usize)->(address:usize)
    requires raw_valid(s),start as int+count as int<=s.cap as int,
    ensures address>0,address as int%s.align as int==0,
        address as int==s.base as int+start as int*s.size as int,
        count as int*s.size as int<=isize::MAX as int,
        doc_slice_from_raw_parts(allocation_of(s),address,count,s.size,s.align),
        doc_write_bytes(allocation_of(s),address,count,s.size,s.align),
        doc_non_null_new_unchecked(address),
{
    proof {memory_span(s.cap,s.size,start as int,count as int);}
    let address=offset_storage(s,start);
    proof {
        storage_is_an_allocation(s);
        assert(0<=count as int*s.size as int) by(nonlinear_arith);
        assert(0<=start as int*s.size as int) by(nonlinear_arith);
        assert(start as int*s.size as int+count as int*s.size as int
            ==(start as int+count as int)*s.size as int) by(nonlinear_arith);
    }
    let sized=slice_size_precondition(Ghost(allocation_of(s)),address,s.size,count,s.align);
    assert(sized);
    address
}

fn layout_caller(s:RawStorage)->(bytes:usize)
    requires raw_valid(s),
    ensures bytes as int==s.cap as int*s.size as int,
        vstd::layout::valid_layout(bytes,s.align),
        doc_layout_from_size_align(bytes,s.align),
{
    aligned_array_layout(s.cap,s.size,s.align)
}

fn inside_nonoverlap_caller(s:RawStorage,left:usize,right:usize,count:usize)
    requires raw_valid(s),left as int+count as int<=right as int,
        right as int+count as int<=s.cap as int,
{
    let src=slice_caller(s,left,count);
    let dst=slice_caller(s,right,count);
    proof {ordered_elements_do_not_overlap(s.size,left,count,right);}
    proof {
        assert(src as int+count as int*s.size as int<=dst as int) by(nonlinear_arith)
            requires src as int==s.base as int+left as int*s.size as int,
                dst as int==s.base as int+right as int*s.size as int,
                (left as int+count as int)*s.size as int<=right as int*s.size as int;
    }
    proof {storage_is_an_allocation(s);}
    let accepted=copy_nonoverlap_precondition(Ghost(allocation_of(s)),Ghost(allocation_of(s)),src,dst,s.size,count,s.align);
    assert(accepted);
}

// `copy_nonoverlapping` between two ranges of one storage in either order.
fn unordered_nonoverlap_caller(s:RawStorage,source:usize,destination:usize,count:usize)
    requires raw_valid(s),source as int+count as int<=s.cap as int,
        destination as int+count as int<=s.cap as int,
        source as int+count as int<=destination as int
            || destination as int+count as int<=source as int,
{
    let src=slice_caller(s,source,count);
    let dst=slice_caller(s,destination,count);
    proof {
        storage_is_an_allocation(s);
        if source as int+count as int<=destination as int {
            ordered_elements_do_not_overlap(s.size,source,count,destination);
            assert(src as int+count as int*s.size as int<=dst as int) by(nonlinear_arith)
                requires src as int==s.base as int+source as int*s.size as int,
                    dst as int==s.base as int+destination as int*s.size as int,
                    (source as int+count as int)*s.size as int<=destination as int*s.size as int;
        } else {
            ordered_elements_do_not_overlap(s.size,destination,count,source);
            assert(dst as int+count as int*s.size as int<=src as int) by(nonlinear_arith)
                requires src as int==s.base as int+source as int*s.size as int,
                    dst as int==s.base as int+destination as int*s.size as int,
                    (destination as int+count as int)*s.size as int<=source as int*s.size as int;
        }
    }
    let accepted=copy_nonoverlap_precondition(Ghost(allocation_of(s)),Ghost(allocation_of(s)),src,dst,s.size,count,s.align);
    assert(accepted);
}

// `ptr::replace` of the element at `index`.
fn replace_caller(s:RawStorage,index:usize)->(address:usize)
    requires raw_valid(s),index<s.cap,
    ensures doc_replace(allocation_of(s),address,s.size,s.align),
{
    let address=slice_caller(s,index,1);
    proof {storage_is_an_allocation(s);}
    let accepted=replacement_address_precondition(Ghost(allocation_of(s)),address,s.size,s.align,s.size==0);
    assert(accepted);
    address
}

// Two storages of one element type, and two storages that share no byte.
spec fn same_element(a:RawStorage,b:RawStorage)->bool {
    a.size==b.size && a.align==b.align
}
spec fn apart(a:RawStorage,b:RawStorage)->bool {
    a.base as int+a.cap as int*a.size as int<=b.base as int
        || b.base as int+b.cap as int*b.size as int<=a.base as int
}

fn distinct_nonoverlap_caller(source:RawStorage,destination:RawStorage,
    start:usize,write:usize,count:usize)
    requires raw_valid(source),raw_valid(destination),source.size==destination.size,
        source.align==destination.align,
        start as int+count as int<=source.cap as int,
        write as int+count as int<=destination.cap as int,
        source.base as int+source.cap as int*source.size as int<=destination.base as int
            || destination.base as int+destination.cap as int*destination.size as int<=source.base as int,
{
    let src=slice_caller(source,start,count);
    let dst=slice_caller(destination,write,count);
    proof {
        memory_span(source.cap,source.size,start as int,count as int);
        memory_span(destination.cap,destination.size,write as int,count as int);
        assert(src as int+count as int*source.size as int<=source.base as int+source.cap as int*source.size as int) by(nonlinear_arith)
            requires src as int==source.base as int+start as int*source.size as int,
                (start as int+count as int)*source.size as int<=source.cap as int*source.size as int;
        assert(dst as int+count as int*source.size as int<=destination.base as int+destination.cap as int*destination.size as int) by(nonlinear_arith)
            requires source.size==destination.size,
                dst as int==destination.base as int+write as int*destination.size as int,
                (write as int+count as int)*destination.size as int<=destination.cap as int*destination.size as int;
    }
    proof {storage_is_an_allocation(source);storage_is_an_allocation(destination);}
    let accepted=copy_nonoverlap_precondition(Ghost(allocation_of(source)),Ghost(allocation_of(destination)),src,dst,source.size,count,source.align);
    assert(accepted);
}

fn cursor_successor(index:usize,end:usize)->(next:usize)
    requires index<end,
    ensures next as int==index as int+1,next<=end,
        doc_unchecked_add_usize(index,1),
{
    index+1
}

// Field pointers may be unaligned (packed structs are explicitly rejected).
// is_aligned_to itself must return false rather than panic in that case: only
// its alignment argument, not the tested address, must be a power of two.
fn alignment_argument_caller(base:usize,align:usize)->(aligned:bool)
    requires doc_alignment_new_unchecked(align),
{
    proof {lemma_fundamental_div_mod(align as int,align as int);}
    let valid=alignment_mask_precondition(align,align);assert(valid);
    (base & (align-1))==0
}

fn initialized_caller(s:RawStorage,index:usize,len:usize)->(address:usize)
    requires raw_valid(s),index<len<=s.cap,
    ensures address>0,address as int%s.align as int==0,
{
    let accepted=unchecked_index_precondition(index,len);assert(accepted);
    slice_caller(s,index,1)
}

fn span_caller(s:RawStorage,front:usize,back:usize,len:usize)->(address:usize)
    requires raw_valid(s),front<=back<=len<=s.cap,
    ensures address>0,address as int%s.align as int==0,
{
    let accepted=unchecked_index_range_precondition(front,back,len);assert(accepted);
    let count=back-front;
    slice_caller(s,front,count)
}

fn overlapping_copy_caller(s:RawStorage,source:usize,destination:usize,count:usize)
    requires raw_valid(s),source as int+count as int<=s.cap as int,
        destination as int+count as int<=s.cap as int,
    ensures doc_copy(allocation_of(s),allocation_of(s),
        (s.base as int+source as int*s.size as int) as usize,
        (s.base as int+destination as int*s.size as int) as usize,count,s.size,s.align),
{
    let _=slice_caller(s,source,count);
    let _=slice_caller(s,destination,count);
}

// Owning iterators over a storage whose live interval is a product `Span`
// (Vec IntoIter and Drain, the binary heap Drain). The callers run the product
// Span methods; `span@.back <= cap` is the stated premise.
// IntoIter/Drain::as_slice, as_mut_slice and the IntoIter drops:
// `ptr.add(span.front())` with `span.len()` elements.
fn span_remaining_caller(a:RawStorage,span:crate::span::Span)
    requires raw_valid(a),span.wf(),span@.back<=a.cap,
{
    let count=span.len();
    let front=span.front();
    let _=offset_storage(a,front);
    let _=slice_caller(a,front,count);
}

// Drain::clear_remaining: front and length are read before `span.clear()`.
fn span_clear_caller(a:RawStorage,span:crate::span::Span)
    requires raw_valid(a),span.wf(),span@.back<=a.cap,
{
    let mut span=span;
    let start=span.front();
    let count=span.len();
    span.clear();
    let _=offset_storage(a,start);
    let _=slice_caller(a,start,count);
}

// next: `ptr.add(index).read()` for the index removed by `span.next()`.
fn span_next_caller(a:RawStorage,span:crate::span::Span)
    requires raw_valid(a),span.wf(),span@.back<=a.cap,
{
    let mut span=span;
    if let Some(index)=span.next() {
        let _=offset_storage(a,index);
        let _=slice_caller(a,index,1);
    }
}

// next_back: `ptr.add(index).read()` for the index removed by `span.next_back()`.
fn span_next_back_caller(a:RawStorage,span:crate::span::Span)
    requires raw_valid(a),span.wf(),span@.back<=a.cap,
{
    let mut span=span;
    if let Some(index)=span.next_back() {
        let _=offset_storage(a,index);
        let _=slice_caller(a,index,1);
    }
}

// The conditions above as statements about every storage, for callers that
// are proved once for all storages of a shape (the B-tree node arrays, the
// hash table arrays) instead of for one storage passed as an argument.
spec fn slot_address(s:RawStorage,index:usize)->usize {
    (s.base as int+index as int*s.size as int) as usize
}
proof fn storage_range_condition(s:RawStorage,start:usize,count:usize)
    requires raw_valid(s),start as int+count as int<=s.cap as int,
    ensures slot_address(s,start) as int==s.base as int+start as int*s.size as int,
        slot_address(s,start)!=0,aligned(slot_address(s,start),s.align),
        doc_ptr_add(allocation_of(s),s.base,start,s.size),
        doc_slice_from_raw_parts(allocation_of(s),slot_address(s,start),count,s.size,s.align),
        doc_write_bytes(allocation_of(s),slot_address(s,start),count,s.size,s.align),
        doc_non_null_new_unchecked(slot_address(s,start)),
{
    storage_is_an_allocation(s);
    memory_span(s.cap,s.size,start as int,count as int);
    assert(0<=start as int*s.size as int) by(nonlinear_arith);
    assert(0<=count as int*s.size as int) by(nonlinear_arith);
    assert(start as int*s.size as int+count as int*s.size as int
        ==(start as int+count as int)*s.size as int) by(nonlinear_arith);
    lemma_fundamental_div_mod(s.base as int,s.align as int);
    lemma_fundamental_div_mod(s.size as int,s.align as int);
    let factor=s.base as int/s.align as int+start as int*(s.size as int/s.align as int);
    assert(s.base as int+start as int*s.size as int==s.align as int*factor) by(nonlinear_arith)
        requires s.base as int==s.align as int*(s.base as int/s.align as int),
            s.size as int==s.align as int*(s.size as int/s.align as int),
            factor==s.base as int/s.align as int+start as int*(s.size as int/s.align as int);
    lemma_mod_multiples_vanish(factor,0,s.align as int);
}
// `ptr::copy` between ranges of two storages of one element type (or of one
// storage): overlap is permitted.
proof fn storage_copy_condition(a:RawStorage,b:RawStorage,from:usize,to:usize,count:usize)
    requires raw_valid(a),raw_valid(b),same_element(a,b),
        from as int+count as int<=a.cap as int,to as int+count as int<=b.cap as int,
    ensures doc_copy(allocation_of(a),allocation_of(b),slot_address(a,from),slot_address(b,to),count,a.size,a.align),
{
    storage_range_condition(a,from,count);
    storage_range_condition(b,to,count);
}
// `ptr::replace` of one element.
proof fn storage_replace_condition(s:RawStorage,index:usize)
    requires raw_valid(s),index<s.cap,
    ensures doc_replace(allocation_of(s),slot_address(s,index),s.size,s.align),
{
    storage_range_condition(s,index,1);
    assert(1*s.size as int==s.size as int);
}
// One range of a storage: the address of its first element and `count`
// elements from there. A read, write or drop of one element is a range of one.
spec fn storage_range(s:RawStorage,start:usize,count:usize)->bool {
    doc_ptr_add(allocation_of(s),s.base,start,s.size)
    && doc_slice_from_raw_parts(allocation_of(s),slot_address(s,start),count,s.size,s.align)
}
proof fn storage_sub_condition(s:RawStorage,from:usize,count:usize)
    requires raw_valid(s),count<=from,from<=s.cap,
    ensures doc_ptr_sub(allocation_of(s),slot_address(s,from),count,s.size),
        slot_address(s,from) as int-count as int*s.size as int==slot_address(s,(from-count) as usize) as int,
{
    storage_range_condition(s,from,0);
    storage_range_condition(s,(from-count) as usize,count);
    assert(from as int*s.size as int-count as int*s.size as int==(from as int-count as int)*s.size as int) by(nonlinear_arith);
    assert(0<=count as int*s.size as int) by(nonlinear_arith);
}
// `ptr.sub(count)` from the address of element `from`.
fn sub_storage(s:RawStorage,from:usize,count:usize)->(address:usize)
    requires raw_valid(s),count<=from,from<=s.cap,
    ensures doc_ptr_sub(allocation_of(s),slot_address(s,from),count,s.size),
        address==slot_address(s,(from-count) as usize),
{
    proof {storage_sub_condition(s,from,count);storage_is_an_allocation(s);storage_range_condition(s,from,0);}
    let start=offset_storage(s,from);
    let accepted=pointer_sub_precondition(Ghost(allocation_of(s)),start,count,s.size);
    assert(accepted);
    offset_storage(s,from-count)
}
}
