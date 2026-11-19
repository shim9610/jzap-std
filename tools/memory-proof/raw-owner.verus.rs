// Common caller facts for core's debug precondition checks. Live allocation
// address/alignment bounds come from the allocator or a valid Rust borrow;
// capacity/offset bounds are established by the source-linked owner models.
verus! {
// The single-element moves of Vec. Each body calls the product kernel that the
// caller uses, so the offsets and counts below follow from the kernel's proved
// result rather than from a stated assumption. callsite_projection.py compares
// the caller's unsafe block with the operations named here.

// Vec::try_insert after `reserve_length(1)`: `place = ptr.add(index)`,
// `copy(place, place.add(1), shifted)`, `place.write(value)`.
fn vec_insert_caller(a:RawStorage,len:usize,index:usize)
    requires raw_valid(a),len<a.cap,
{
    let Ok(shifted)=crate::vec::geometry::insertion(crate::Checked::new(len),index) else {return;};
    let shifted=shifted.get();
    let _=offset_storage(a,index);
    let _=offset_storage(a,index+1);
    overlapping_copy_caller(a,index,index+1,shifted);
    let _=slice_caller(a,index,1);
}

// Vec::remove: `place = ptr.add(index)`, `place.read()`,
// `copy(place.add(1), place, shifted)`.
fn vec_remove_caller(a:RawStorage,len:usize,index:usize)
    requires raw_valid(a),len<=a.cap,
{
    let Ok((next,shifted))=crate::vec::geometry::removal(crate::Checked::new(len),index) else {return;};
    let shifted=shifted.get();
    let _=offset_storage(a,index);
    let _=slice_caller(a,index,1);
    let _=offset_storage(a,index+1);
    overlapping_copy_caller(a,index+1,index,shifted);
}

// Vec::swap_remove after `self.len = next`: `place = ptr.add(index)`,
// `place.read()`, `copy(ptr.add(self.len), place, 1)`.
fn vec_swap_remove_caller(a:RawStorage,len:usize,index:usize)
    requires raw_valid(a),len<=a.cap,
{
    let Ok(next)=crate::vec::geometry::shortened(crate::Checked::new(len),index) else {return;};
    let next=next.get();
    let _=offset_storage(a,index);
    let _=slice_caller(a,index,1);
    let _=offset_storage(a,next);
    overlapping_copy_caller(a,next,index,1);
}

// Callers that rest on the owner invariant alone: the initialized length never
// exceeds the capacity of the storage (`len <= cap`, closed by the state
// publication audit). No further phase condition is stated for them.
// callsite_projection.py compares each caller's whole unsafe block with the
// reviewed text and names one of these functions for it.

// A slice or slice pointer over the first `len` elements of the storage.
fn owner_prefix_caller(a:RawStorage,len:usize)
    requires raw_valid(a),len<=a.cap,
{
    let _=slice_caller(a,0,len);
}

// The empty boxed slice: a dangling pointer with zero elements.
fn owner_empty_caller(a:RawStorage)
    requires raw_valid(a),
{
    let _=slice_caller(a,0,0);
}

// One complete value in its own typed place: the payload of a scalar Box or
// InitSlot, or a field read through a reference. The storage holds at least
// one element; that is the typed-layout fact, not an arithmetic result.
fn owner_scalar_caller(a:RawStorage)
    requires raw_valid(a),1<=a.cap,
{
    let _=slice_caller(a,0,1);
}

// The spare capacity: `ptr.add(len)` and `wrapping_sub(cap, len)` elements. The
// count is the product subtraction itself, whose contract needs `len <= cap`.
fn owner_spare_caller(a:RawStorage,len:usize)
    requires raw_valid(a),len<=a.cap,
{
    let spare=crate::arithmetic::bounded::wrapping_sub(a.cap,len);
    let _=offset_storage(a,len);
    let _=slice_caller(a,len,spare);
}

// Vec::split_at_spare_mut: the initialized prefix and the spare capacity.
fn owner_split_spare_caller(a:RawStorage,len:usize)
    requires raw_valid(a),len<=a.cap,
{
    owner_prefix_caller(a,len);
    owner_spare_caller(a,len);
}

// Iterators over the product `Span`. Each function runs the same Span methods
// as the caller, in the caller's order, so the offset and the count are the
// results proved on src/span.rs. The stated premise is that the span ends
// inside the storage.

// Indices handed out by `init::prefix::initialize(ptr, n, callback)`. The body
// runs the product driver itself, so `index < n` in the callback is the
// driver's verified call condition, not a stated premise. `initialize_one`
// forms `ptr.add(index)`; the clone callbacks read a source of the same `n`
// elements with `get_unchecked(index)`.
fn prefix_driver_caller(a:RawStorage,n:usize)
    requires raw_valid(a),n<=a.cap,
{
    let mut state=crate::Checked::new(0usize);
    let _=crate::init::driver::run(&mut state,n,|index:usize| -> (out:Result<(),()>)
        requires raw_valid(a),n<=a.cap,index<n,
    {
        let _=offset_storage(a,index);
        let accepted=unchecked_index_precondition(index,n);
        assert(accepted);
        Ok(())
    });
}

// Indices handed out by `init::collection::fill(ptr, state, end, iter)`, which
// runs the same driver from the initialized length to the reserved end.
// `write_next` writes one value at `ptr.add(index)`.
fn collection_driver_caller(a:RawStorage,initial:usize,end:usize)
    requires raw_valid(a),initial<=end<=a.cap,
{
    let mut state=crate::Checked::new(initial);
    let _=crate::init::driver::run(&mut state,end,|index:usize| -> (out:Result<(),()>)
        requires raw_valid(a),end<=a.cap,index<end,
    {
        let _=offset_storage(a,index);
        let _=slice_caller(a,index,1);
        Ok(())
    });
}

// Vec::pop: the checked subtraction refuses an empty vector; the read is at
// its result.
fn vec_pop_caller(a:RawStorage,len:usize)
    requires raw_valid(a),len<=a.cap,
{
    let Ok(next)=crate::Checked::new(len)-crate::Checked::new(1usize) else {return;};
    let next=next.get();
    let _=offset_storage(a,next);
    let _=slice_caller(a,next,1);
}

// Vec::truncate: `removed_suffix` gives the dropped count; the suffix starts
// at the requested length.
fn vec_truncate_caller(a:RawStorage,len:usize,requested:usize)
    requires raw_valid(a),len<=a.cap,
{
    let Some(removed)=crate::vec::geometry::removed_suffix(crate::Checked::new(len),crate::Checked::new(requested)) else {return;};
    let removed=removed.get();
    let _=offset_storage(a,requested);
    let _=slice_caller(a,requested,removed);
}

// Vec::push_within_capacity: `append_cursor` admits the spare slot at `len`.
fn vec_push_within_caller(a:RawStorage,len:usize)
    requires raw_valid(a),len<=a.cap,
{
    let Ok(_next)=crate::vec::geometry::append_cursor(crate::Checked::new(len),a.cap) else {return;};
    let _=offset_storage(a,len);
    let _=slice_caller(a,len,1);
}

// Vec::try_insert_mut after a successful `try_insert(index, value)`: the same
// kernel admitted `index`, and the slot at it is now initialized.
fn vec_insert_mut_caller(a:RawStorage,len:usize,index:usize)
    requires raw_valid(a),len<a.cap,
{
    let Ok(_shifted)=crate::vec::geometry::insertion(crate::Checked::new(len),index) else {return;};
    let _=offset_storage(a,index);
    let _=slice_caller(a,index,1);
}

// One slot at the initialized length after a successful reservation.
fn reserved_slot_caller(a:RawStorage,len:usize)
    requires raw_valid(a),len<a.cap,
{
    let _=offset_storage(a,len);
    let _=slice_caller(a,len,1);
}

// Reservation results. Each function runs the product kernels in the order of
// the reservation body it names and returns the capacity that body publishes.
// callsite_projection.py pins those bodies whole.

// Vec::reserve_length with grow_to and resize_storage: the final length and
// the capacity afterwards.
fn vec_reserve_length(len:usize,cap:usize,limit:usize,size:usize,additional:usize)->(out:Option<(usize,usize)>)
    ensures match out {
        Some((needed,after)) => needed as int==len as int+additional as int && needed<=after && cap<=after,
        None => true,
    },
{
    let Ok(needed)=crate::vec::required_len(len,additional,limit) else {return None;};
    if needed>cap {
        let Ok(grown)=crate::vec::geometry::growth_for_layout(cap,needed,limit,size,false) else {return None;};
        Some((needed,grown))
    } else {
        Some((needed,cap))
    }
}

// Vec::try_reserve and try_reserve_exact with grow, grow_to and
// resize_storage: the capacity afterwards.
fn vec_try_reserve(len:usize,cap:usize,limit:usize,size:usize,additional:usize,exact:bool)->(out:Option<usize>)
    ensures match out {
        Some(after) => len as int+additional as int<=after as int && cap<=after,
        None => true,
    },
{
    let Ok(spare)=crate::Checked::new(cap)-crate::Checked::new(len) else {return None;};
    if additional>spare.get() {
        let Ok(needed)=crate::vec::required_len(len,additional,limit) else {return None;};
        let Ok(grown)=crate::vec::geometry::growth_for_layout(cap,needed,limit,size,exact) else {return None;};
        Some(grown)
    } else {
        Some(cap)
    }
}

// Vec::try_extend_copy, try_append and the bulk branch of try_extend_cloned:
// `copy_nonoverlapping(source, ptr.add(len), additional)` after
// `reserve_length(additional)`. `a` is the storage after the reservation.
fn vec_extend_copy_caller(a:RawStorage,b:RawStorage,len:usize,before:usize,limit:usize,additional:usize)
    requires raw_valid(a),raw_valid(b),b.size==a.size,b.align==a.align,additional<=b.cap,
        a.base as int+a.cap as int*a.size as int<=b.base as int || b.base as int+b.cap as int*b.size as int<=a.base as int,
{
    let Some((_needed,after))=vec_reserve_length(len,before,limit,a.size,additional) else {return;};
    if a.cap!=after {return;}
    let _=offset_storage(a,len);
    distinct_nonoverlap_caller(b,a,0,len,additional);
}

// Vec::try_extend_copy_from_within: `count = end - start`, then
// `copy_nonoverlapping(ptr.add(start), ptr.add(len), count)` after
// `reserve_length(count)`. `start <= end <= len` is the range validator result.
fn vec_extend_within_caller(a:RawStorage,len:usize,before:usize,limit:usize,start:usize,end:usize)
    requires raw_valid(a),start<=end<=len,
{
    let Ok(count)=crate::Checked::new(end)-crate::Checked::new(start) else {return;};
    let count=count.get();
    let Some((_needed,after))=vec_reserve_length(len,before,limit,a.size,count) else {return;};
    if a.cap!=after {return;}
    let _=offset_storage(a,start);
    let _=offset_storage(a,len);
    overlapping_copy_caller(a,start,len,count);
    inside_nonoverlap_caller(a,start,len,count);
}

// Vec::try_split_off: `count = len - at`, an empty tail reserved exactly, then
// `copy_nonoverlapping(ptr.add(at), tail.ptr, count)`. `b` is the tail storage.
fn vec_split_off_caller(a:RawStorage,b:RawStorage,len:usize,limit:usize,at:usize)
    requires raw_valid(a),raw_valid(b),b.size==a.size,b.align==a.align,len<=a.cap,
        a.base as int+a.cap as int*a.size as int<=b.base as int || b.base as int+b.cap as int*b.size as int<=a.base as int,
{
    let Ok(count)=crate::Checked::new(len)-crate::Checked::new(at) else {return;};
    let count=count.get();
    let Some(after)=vec_try_reserve(0,0,limit,b.size,count,true) else {return;};
    if b.cap!=after {return;}
    let _=offset_storage(a,at);
    distinct_nonoverlap_caller(a,b,at,0,count);
}

// Vec::try_from_array_in: an empty vector reserved exactly for `n`, then
// `copy_nonoverlapping(values, result.ptr, n)`. `b` is the array of `n`.
fn vec_from_array_caller(a:RawStorage,b:RawStorage,limit:usize,n:usize)
    requires raw_valid(a),raw_valid(b),b.size==a.size,b.align==a.align,n<=b.cap,
        a.base as int+a.cap as int*a.size as int<=b.base as int || b.base as int+b.cap as int*b.size as int<=a.base as int,
{
    let Some(after)=vec_try_reserve(0,0,limit,a.size,n,true) else {return;};
    if a.cap!=after {return;}
    distinct_nonoverlap_caller(b,a,0,0,n);
}

// Vec::try_into_array: the length equals `N`, then one read of `[T; N]`.
fn vec_into_array_caller(a:RawStorage,len:usize,n:usize)
    requires raw_valid(a),len<=a.cap,
{
    if len!=n {return;}
    let _=slice_caller(a,0,n);
}

// Buffer::append and Buffer::push after `reserve`: the capacity afterwards is
// the `proposed` result of the product reservation in both of its branches.
// `len <= before` is the owner invariant on entry.
fn buffer_append_caller(a:RawStorage,b:RawStorage,len:usize,before:usize,additional:usize,hint:usize)
    requires raw_valid(a),raw_valid(b),b.size==a.size,b.align==a.align,additional<=b.cap,len<=before,
        a.base as int+a.cap as int*a.size as int<=b.base as int || b.base as int+b.cap as int*b.size as int<=a.base as int,
{
    let Ok((_needed,proposed))=crate::boxed::buffer_numeric::reservation(len,before,additional,hint) else {return;};
    if a.cap!=proposed {return;}
    let _=offset_storage(a,len);
    distinct_nonoverlap_caller(b,a,0,len,additional);
}
fn buffer_push_caller(a:RawStorage,len:usize,before:usize,hint:usize)
    requires raw_valid(a),len<=before,
{
    let Ok((_needed,proposed))=crate::boxed::buffer_numeric::reservation(len,before,1,hint) else {return;};
    if a.cap!=proposed {return;}
    let _=offset_storage(a,len);
    let _=slice_caller(a,len,1);
}

// A copy of `n` elements between two storages that each hold `n`: a fresh
// allocation of `layout::array(n)` filled from a slice, or two references whose
// equal length the caller checked. The extents are the stated layout facts.
fn whole_copy_caller(a:RawStorage,b:RawStorage,n:usize)
    requires raw_valid(a),raw_valid(b),b.size==a.size,b.align==a.align,n<=a.cap,n<=b.cap,
        a.base as int+a.cap as int*a.size as int<=b.base as int || b.base as int+b.cap as int*b.size as int<=a.base as int,
{
    let _=slice_caller(a,0,n);
    distinct_nonoverlap_caller(b,a,0,0,n);
}

// Compact::drop: the unvisited suffix moves down to the write position.
fn compact_drop_caller(a:RawStorage,state:crate::compaction::Compaction)
    requires raw_valid(a),state.wf(),state@.original<=a.cap,
{
    if state.read()==state.written() {return;}
    let remaining=state.remaining();
    let read=state.read();
    let written=state.written();
    let _=offset_storage(a,read);
    let _=offset_storage(a,written);
    overlapping_copy_caller(a,read,written,remaining);
}

// ExtractIf::drop: the unvisited part up to the total length moves down.
fn extract_drop_caller(a:RawStorage,state:crate::compaction::Extraction)
    requires raw_valid(a),state.wf(),state@.total<=a.cap,
{
    if state.written()!=state.read() {
        let read=state.read();
        let written=state.written();
        let count=state.unvisited_len();
        let _=offset_storage(a,read);
        let _=offset_storage(a,written);
        overlapping_copy_caller(a,read,written,count);
    }
}

// ExtractIf::fmt: a reference to the next unvisited element, if any.
fn extract_peek_caller(a:RawStorage,state:crate::compaction::Extraction)
    requires raw_valid(a),state.wf(),state@.total<=a.cap,
{
    if state.read()<state.end() {
        let read=state.read();
        let _=offset_storage(a,read);
        let _=slice_caller(a,read,1);
    }
}

// Drain::restore_tail: the retained tail moves down to the current length.
// The Drain invariant is stated: the gap precedes a tail inside the storage.
fn drain_restore_tail_caller(a:RawStorage,len:usize,tail_start:usize,tail_len:usize)
    requires raw_valid(a),len<=tail_start,tail_start as int+tail_len as int<=a.cap as int,
{
    let _=offset_storage(a,tail_start);
    let _=offset_storage(a,len);
    overlapping_copy_caller(a,tail_start,len,tail_len);
}

// Splice::move_tail: `required = tail_len + additional`,
// `next_tail = tail_start + additional`, `try_reserve(required)`, then the
// tail moves up. The filled gap is stated: `len == tail_start`.
fn splice_move_tail_caller(a:RawStorage,len:usize,before:usize,limit:usize,tail_start:usize,tail_len:usize,additional:usize)
    requires raw_valid(a),len==tail_start,tail_start as int+tail_len as int<=before as int,
{
    let Ok(required)=crate::vec::required_len(tail_len,additional,usize::MAX) else {return;};
    let Ok(next_tail)=crate::Checked::new(tail_start)+crate::Checked::new(additional) else {return;};
    let next_tail=next_tail.get();
    let Some(after)=vec_try_reserve(len,before,limit,a.size,required,false) else {return;};
    if a.cap!=after {return;}
    let _=offset_storage(a,tail_start);
    let _=offset_storage(a,next_tail);
    overlapping_copy_caller(a,tail_start,next_tail,tail_len);
}

fn utf8_output_caller(width:usize,buffer:usize)->(fits:bool)
    requires width<=4,buffer==4,
    ensures fits,
{
    width<=buffer
}

}
