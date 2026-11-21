"""Generate core-precondition queries using actual unsafe-call arguments.

The environments are the phase invariants established by the source-derived
owner/loop/state models. All offset/count expressions are extracted afresh.
Representation-only operations use the explicitly stated typed-storage ABI
contract; no all-type pointer-provenance proof is inferred.
"""
import re
from arithmetic_projection import bounded_projection
from verify_wrapping_memory import call_args


def compact(s):return re.sub(r'\s+','',s)


# Operations that convert or pass a typed pointer and carry no element offset
# or count of their own: pointer casts and dereferences, owner constructors
# from raw parts, initialization slots, pinning, and the allocator entry
# points whose byte accounting is bound separately. A site made only of these
# is routed to the maintained `representation_caller`; no proof text is
# generated for it. Any other operation requires an offset or count query.
REPRESENTATION={('method',name) for name in (
    'as_ptr','as_mut_ptr','cast','as_ref','as_mut','drop_in_place','budget','get','into_std_vec','into_bytes',
    'assume_init','is_aligned','mark_initialized','disarm_field','size','layout')}|{('call',name) for name in (
    'NonNull::new_unchecked','core::ptr::NonNull::new_unchecked','Box::from_raw_parts','crate::boxed::Box::from_raw_parts',
    'Self::from_raw_parts','Self::from_raw','Self::into_raw','Self::from_utf8_unchecked','Self::from_bytes_unchecked',
    'InitSlot::new','crate::init::InitSlot::new','crate::init::InitSlot::from_raw',
    'storage::reallocate','storage::deallocate','deallocate',
    # Vec push growth passes the owner's unique storage words, as reallocate does.
    'grow_for_push',
    'alloc::alloc::alloc','alloc::alloc::alloc_zeroed','alloc::alloc::dealloc','alloc::alloc::realloc',
    'Pin::new_unchecked','Pin::into_inner_unchecked','crate::boxed::Pinned::into_inner_unchecked',
    'crate::init::collection::fill','crate::init::prefix::initialize','prefix::initialize','initialize_one','write_next',
    'alloc::boxed::Box::into_raw','alloc::boxed::Box::from_raw','Box::into_std',
    'alloc::vec::Vec::from_raw_parts','Vec::from_raw_parts','alloc::string::String::from_utf8_unchecked',
    'OsString::from_encoded_bytes_unchecked','crate::boxed::CallRelease::new','crate::boxed::StandardCallOwner::from_raw',
    'crate::capacity::Capacity::from_validated_raw','crate::error::InitError::Storage','Err')}|{('macro','core::ptr::addr_of_mut')}


# Callers whose offsets and counts come from product kernels. The maintained
# adapter in raw-owner.verus.rs calls the same kernel; the unsafe block must be
# exactly the reviewed one. No assumption or query is generated for them.
KERNEL_CALLERS={
    ('src/vec.rs','try_insert'):('vec_insert_caller','unsafe{letplace=self.ptr.as_ptr().add(index);core::ptr::copy(place,place.add(1),shifted.get());place.write(value);}'),
    ('src/vec.rs','remove'):('vec_remove_caller','unsafe{letplace=self.ptr.as_ptr().add(index);letvalue=place.read();core::ptr::copy(place.add(1),place,shifted.get());self.len=next;Ok(value)}'),
    ('src/vec.rs','swap_remove'):('vec_swap_remove_caller','unsafe{letplace=self.ptr.as_ptr().add(index);letvalue=place.read();core::ptr::copy(self.ptr.as_ptr().add(self.len.get()),place,1);Ok(value)}'),
}


# Callers that need only the owner invariant `len <= cap`, or one complete
# typed value. Each whole unsafe
# block must equal the reviewed text; the named maintained function in
# raw-owner.verus.rs states the offsets and counts. Nothing is lowered from the
# block and every entry must be consumed exactly once.
PREFIX='owner_prefix_caller'
SCALAR='owner_scalar_caller'
INVARIANT_CONDITIONS={SCALAR:['raw_valid(a)','1<=a.cap'],'owner_empty_caller':['raw_valid(a)'],'layout_caller':['raw_valid(a)']}
INVARIANT_CALLERS={
    ('src/boxed/buffer.rs','drop','unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(),self.len.get(),))}'):PREFIX,
    ('src/boxed/foreign.rs','try_into_bytes_with_nul','unsafe{Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr.cast::<u8>(),len),policy)}'):PREFIX,
    ('src/boxed/slice.rs','empty_in','unsafe{Self::from_raw_parts(NonNull::slice_from_raw_parts(NonNull::dangling(),0),policy,)}'):'owner_empty_caller',
    ('src/boxed/slice.rs','try_init_slice_in','unsafe{Self::from_raw_parts(NonNull::slice_from_raw_parts(ptr,len),policy)}'):PREFIX,
    ('src/boxed/slice.rs','try_new_uninit_slice_in','unsafe{Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr,len),policy)}'):PREFIX,
    ('src/boxed/slice.rs','try_new_zeroed_slice_in','unsafe{Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr,len),policy)}'):PREFIX,
    ('src/boxed/slice.rs','assume_init','unsafe{Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr.cast::<T>(),ptr.len()),policy,)}'):PREFIX,
    ('src/boxed/slice.rs','try_from_std_vec_in','unsafe{Self::from_raw_parts(NonNull::slice_from_raw_parts(ptr.cast(),len),policy)}'):PREFIX,
    ('src/capacity.rs','layout','unsafe{Layout::from_size_align_unchecked(bytes,mem::align_of::<T>())}'):'layout_caller',
    ('src/init/prefix.rs','drop','unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(),self.state.get(),));}'):PREFIX,
    ('src/vec.rs','as_slice','unsafe{core::slice::from_raw_parts(self.ptr.as_ptr(),self.len.get())}'):PREFIX,
    ('src/vec.rs','as_mut_slice','unsafe{core::slice::from_raw_parts_mut(self.ptr.as_ptr(),self.len.get())}'):PREFIX,
    ('src/vec.rs','drop','unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(),self.len.get(),))}'):PREFIX,
    ('src/vec/convenience.rs','spare_capacity_mut','unsafe{core::slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.len.get()).cast(),crate::arithmetic::bounded::wrapping_sub(self.cap.get(),self.len.get()),)}'):'owner_spare_caller',
    ('src/vec/convenience.rs','split_at_spare_mut','unsafe{(core::slice::from_raw_parts_mut(self.ptr.as_ptr(),self.len.get()),core::slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.len.get()).cast(),crate::arithmetic::bounded::wrapping_sub(self.cap.get(),self.len.get()),),)}'):'owner_split_spare_caller',
    ('src/vec/convenience.rs','leak','unsafe{core::slice::from_raw_parts_mut(ptr.as_ptr(),len)}'):PREFIX,
    ('src/vec/ownership.rs','try_into_boxed_slice','unsafe{crate::boxed::Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr,len),policy)}'):PREFIX,
    # One complete value in its own typed place: a scalar Box or InitSlot
    # payload, or a policy field read through a reference. The typed layout is
    # the stated ABI fact; the block carries no offset or count.
    ('src/boxed.rs','try_new_in','unsafe{ptr.as_ptr().write(value);}'):SCALAR,
    ('src/boxed.rs','try_clone','unsafe{ptr.as_ptr().write(self.ptr.as_ptr().read())}'):SCALAR,
    ('src/boxed/buffer.rs','finish','unsafe{core::ptr::read(&owner.policy)}'):SCALAR,
    ('src/boxed/owned.rs','into_raw_parts','unsafe{core::ptr::read(&this.policy)}'):SCALAR,
    ('src/boxed/owned.rs','into_inner','unsafe{ptr.as_ptr().read()}'):SCALAR,
    ('src/boxed/owned.rs','into_boxed_slice','unsafe{Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr,1),policy)}'):SCALAR,
    ('src/boxed/owned.rs','take','unsafe{(ptr.as_ptr().read(),Box::from_raw_parts(ptr.cast(),policy))}'):SCALAR,
    ('src/boxed/owned.rs','write','unsafe{ptr.cast::<T>().as_ptr().write(value);Box::from_raw_parts(ptr.cast(),policy)}'):SCALAR,
    ('src/init.rs','write','unsafe{self.ptr.as_ptr().write(value)}'):SCALAR,
    ('src/vec/ownership.rs','into_raw_parts','unsafe{core::ptr::read(&owner.policy)}'):SCALAR,
}
# Functions with an entry above keep no other offset-bearing unsafe block.
INVARIANT_FUNCTIONS={(f,fn) for f,fn,_ in INVARIANT_CALLERS}


# Iterators whose positions come from the product `Span` state type. The
# maintained function in raw-owner.verus.rs calls the same Span methods, whose
# contracts are verified on src/span.rs. The whole function body must equal
# the reviewed text, so the index or count used by the unsafe block cannot come
# from anywhere else. The one stated premise is `span.back <= cap`.
SLOT='reserved_slot_caller'
RESIZE_WITH_INIT="{ifnew_len<=self.len.get(){self.truncate(new_len);returnOk(());}letadditional=(Checked::new(new_len)-self.len).map_err(|_|InitError::Storage(Error::Capacity))?;self.try_reserve(additional.get()).map_err(InitError::Storage)?;letoriginal=self.len.get();letguard=SuffixGuard{owner:self,original,};#[cfg_attr(all(kani,jzap_loop_contracts),kani::loop_invariant(original<=guard.owner.len.get()&&guard.owner.len.get()<=new_len&&guard.original==original&&guard.owner.ptr==on_entry(guard.owner.ptr)&&guard.owner.cap.get()==on_entry(guard.owner.cap.get())))]whileguard.owner.len.get()<new_len{letindex=guard.owner.len.get();letnext=(guard.owner.len+Checked::new(1)).map_err(|_|InitError::Storage(Error::Capacity))?;letptr=unsafe{NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(index))};letmutslot=unsafe{InitSlot::new(ptr)};initialize(index,&mutslot).map_err(InitError::Initialize)?;slot.finish().map_err(InitError::Storage)?;guard.owner.len=next;}mem::forget(guard);Ok(())}"
RESIZE_OWNED="{ifnew_len<=self.len.get(){self.truncate(new_len);returnOk(());}letlast=matchChecked::new(new_len)-Checked::new(1){Ok(last)=>last.get(),Err(_)=>{returnErr(Rejected{value,error:Error::Capacity,});}};letadditional=matchChecked::new(new_len)-self.len{Ok(additional)=>additional,Err(_)=>{returnErr(Rejected{value,error:Error::Capacity,});}};ifletErr(error)=self.try_reserve(additional.get()){returnErr(Rejected{value,error});}letoriginal=self.len.get();letguard=SuffixGuard{owner:self,original,};whileguard.owner.len.get()<last{letnext=matchguard.owner.len+Checked::new(1){Ok(next)=>next,Err(_)=>{returnErr(Rejected{value,error:Error::Capacity,});}};letptr=unsafe{NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(guard.owner.len.get()))};letmutslot=unsafe{InitSlot::new(ptr)};ifletErr(error)=value.try_clone_into(&mutslot){returnErr(Rejected{value,error});}ifletErr(error)=slot.finish(){returnErr(Rejected{value,error});}guard.owner.len=next;}letnext=matchguard.owner.len+Checked::new(1){Ok(next)=>next,Err(_)=>{returnErr(Rejected{value,error:Error::Capacity,});}};unsafe{guard.owner.ptr.as_ptr().add(guard.owner.len.get()).write(value);}guard.owner.len=next;mem::forget(guard);Ok(())}"
DRIVER='prefix_driver_caller';REMAINING='span_remaining_caller';NEXT='span_next_caller';NEXT_BACK='span_next_back_caller'
STATE_CALLERS={
    ('src/vec/ownership.rs','as_slice','IntoIter'):(REMAINING,'{unsafe{core::slice::from_raw_parts(self.owner.ptr.as_ptr().add(self.span.front()),self.span.len(),)}}'),
    ('src/vec/ownership.rs','as_mut_slice','IntoIter'):(REMAINING,'{unsafe{core::slice::from_raw_parts_mut(self.owner.ptr.as_ptr().add(self.span.front()),self.span.len(),)}}'),
    ('src/vec/ownership.rs','drop','IntoIter'):(REMAINING,'{letremaining=self.span.len();if!mem::needs_drop::<T>(){return;}unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.owner.ptr.as_ptr().add(self.span.front()),remaining,))};}'),
    ('src/vec/ownership.rs','next','IntoIter'):(NEXT,'{letindex=self.span.next()?;Some(unsafe{self.owner.ptr.as_ptr().add(index).read()})}'),
    ('src/vec/ownership.rs','next_back','IntoIter'):(NEXT_BACK,'{letindex=self.span.next_back()?;Some(unsafe{self.owner.ptr.as_ptr().add(index).read()})}'),
    ('src/boxed/slice.rs','drop','IntoIter'):(REMAINING,'{let_allocation=AllocationGuard(self.ptr.cast(),self.layout,self.policy.budget());letremaining=self.span.len();if!core::mem::needs_drop::<T>(){return;}unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr().add(self.span.front()),remaining,))}}'),
    ('src/boxed/slice.rs','next','IntoIter'):(NEXT,'{leti=self.span.next()?;Some(unsafe{self.ptr.as_ptr().add(i).read()})}'),
    ('src/boxed/slice.rs','next_back','IntoIter'):(NEXT_BACK,'{leti=self.span.next_back()?;Some(unsafe{self.ptr.as_ptr().add(i).read()})}'),
    ('src/vec/edit.rs','as_slice','Drain'):(REMAINING,'{unsafe{core::slice::from_raw_parts(self.owner.ptr.as_ptr().add(self.span.front()),self.span.len(),)}}'),
    ('src/vec/edit.rs','clear_remaining','Drain'):('span_clear_caller','{letstart=self.span.front();letcount=self.span.len();self.span.clear();if!mem::needs_drop::<T>(){return;}unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.owner.ptr.as_ptr().add(start),count,));}}'),
    ('src/vec/edit.rs','next','Drain'):(NEXT,'{leti=self.span.next()?;Some(unsafe{self.owner.ptr.as_ptr().add(i).read()})}'),
    ('src/vec/edit.rs','next_back','Drain'):(NEXT_BACK,'{leti=self.span.next_back()?;Some(unsafe{self.owner.ptr.as_ptr().add(i).read()})}'),
    # Indices handed out by `init::prefix::initialize`. It runs the verified
    # `init::driver::run` from zero to the element count; `initialize_one`
    # offsets the storage by that index and the clone closures read the source
    # of the same length at it. PINNED_BODIES fixes every link of that chain.
    ('src/init/prefix.rs','initialize_one',''):(DRIVER,'{letpointer=unsafe{NonNull::new_unchecked(ptr.as_ptr().add(index))};letmutslot=unsafe{InitSlot::new(pointer)};initialize(index,&mutslot).map_err(InitError::Initialize)?;slot.finish().map_err(InitError::Storage)}','unsafe{NonNull::new_unchecked(ptr.as_ptr().add(index))}'),
    ('src/clone.rs','try_clone','[T ; N]'):(DRIVER,'{array_from_fn(|i|{unsafe{self.get_unchecked(i)}.try_clone()})}'),
    ('src/clone.rs','try_clone_into','[T ; N]'):(DRIVER,'{slot.try_init_array(|i,element|{unsafe{self.get_unchecked(i)}.try_clone_into(element)}).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}'),
    ('src/clone.rs','try_clone_in','[T ; N]'):(DRIVER,'{array_from_fn(|i|{unsafe{self.get_unchecked(i)}.try_clone_in(b)})}'),
    ('src/clone.rs','try_clone_into_in','[T ; N]'):(DRIVER,'{slot.try_init_array(|i,element|{unsafe{self.get_unchecked(i)}.try_clone_into_in(budget,element)}).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}'),
    ('src/boxed/slice.rs','try_clone_from_slice_in','Box'):(DRIVER,'{Self::try_init_slice_in(source.len(),policy,|index,slot|{unsafe{source.get_unchecked(index)}.try_clone_into(slot)}).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}'),
    ('src/boxed/slice.rs','try_clone_in','Box'):(DRIVER,'{Box::<[T::Output],_>::try_init_slice_in(self.len(),budget,|index,slot|{unsafe{self.as_slice().get_unchecked(index)}.try_clone_into_in(budget,slot)}).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}'),
    # Indices handed out by `init::collection::fill`, which runs the same driver
    # from the initialized length to the reserved end.
    ('src/init/collection.rs','write_next',''):('collection_driver_caller','{letvalue=iter.try_next().map_err(Stop::Source)?.ok_or(Stop::End)?;unsafe{ptr.as_ptr().add(index).write(value)};Ok(())}'),
    # Positions computed by product kernels or the checked operator.
    ('src/vec.rs','pop','Vec'):('vec_pop_caller','{letnext=(self.len-Checked::new(1)).ok()?;self.len=next;Some(unsafe{self.ptr.as_ptr().add(next.get()).read()})}'),
    ('src/vec.rs','truncate','Vec'):('vec_truncate_caller','{letnext=Checked::new(len);letSome(removed)=geometry::removed_suffix(self.len,next)else{return;};self.len=next;if!mem::needs_drop::<T>(){return;}unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr().add(len),removed.get(),))};}'),
    ('src/vec/convenience.rs','push_within_capacity','Vec'):('vec_push_within_caller','{letnext=matchgeometry::append_cursor(self.len,self.cap.get()){Ok(next)=>next,Err(error)=>{returnErr(Rejected{value,error});}};letplace=unsafe{self.ptr.as_ptr().add(self.len.get())};unsafe{place.write(value);}self.len=next;Ok(unsafe{&mut*place})}',
        ('unsafe{self.ptr.as_ptr().add(self.len.get())}','unsafe{place.write(value);}')),
    ('src/vec/convenience.rs','try_insert_mut','Vec'):('vec_insert_mut_caller','{self.try_insert(index,value)?;Ok(unsafe{&mut*self.ptr.as_ptr().add(index)})}'),
    # One slot at the initialized length after a successful reservation. The
    # stated premise `len < cap` is the reservation result, which the pending
    # growth composition has to establish; no expression is lowered here.
    ('src/boxed/buffer.rs','push','Buffer'):('buffer_push_caller','{letneeded=self.reserve(1,hint)?;unsafe{self.ptr.as_ptr().add(self.len.get()).write(value)};self.len=Checked::new(needed);Ok(())}'),
    ('src/vec/convenience.rs','try_push_mut','Vec'):(SLOT,'{letindex=self.len.get();self.try_push(value)?;Ok(unsafe{&mut*self.ptr.as_ptr().add(index)})}'),
    ('src/vec.rs','try_resize_with_init','Vec'):(SLOT,RESIZE_WITH_INIT,('unsafe{NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(index))}',)),
    ('src/vec/convenience.rs','try_resize_owned','Vec'):(SLOT,RESIZE_OWNED,
        ('unsafe{NonNull::new_unchecked(guard.owner.ptr.as_ptr().add(guard.owner.len.get()))}','unsafe{guard.owner.ptr.as_ptr().add(guard.owner.len.get()).write(value);}')),
    # Reserved-suffix copies, fresh-storage copies, compaction and Drain tail
    # moves. Each maintained function runs the product kernels the caller
    # runs; the capacity after a reservation is the published kernel result
    # (the reservation bodies are pinned below).
    ('src/boxed/buffer.rs','append','Buffer'):('buffer_append_caller','{letneeded=self.reserve(values.len(),values.len())?;unsafe{core::ptr::copy_nonoverlapping(values.as_ptr(),self.ptr.as_ptr().add(self.len.get()),values.len(),)};self.len=Checked::new(needed);Ok(())}'),
    ('src/boxed/slice.rs','try_copy_from_slice_in','Box'):('whole_copy_caller','{letlayout=crate::layout::array::<T>(source.len())?;letptr=storage::allocate(layout,policy.budget())?.cast::<T>();unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),ptr.as_ptr(),source.len());Ok(Self::from_raw_parts(NonNull::slice_from_raw_parts(ptr,source.len()),policy,))}}'),
    ('src/boxed/string.rs','try_clone_from','Box'):('whole_copy_caller','{ifself.len()==source.len(){unsafe{core::ptr::copy_nonoverlapping(source.as_str().as_ptr(),self.ptr.as_ptr()as*mutu8,self.len(),)};returnOk(());}letreplacement=Self::try_from_str_in(source.as_str(),self.policy.try_clone()?)?;*self=replacement;Ok(())}'),
    ('src/clone.rs','try_copy_slice',''):('whole_copy_caller','{ifsource.len()!=destination.len(){returnErr(Error::Bounds);}unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),destination.as_mut_ptr(),source.len());}Ok(())}'),
    ('src/vec.rs','try_extend_copy','Vec'):('vec_extend_copy_caller','{letneeded=self.reserve_length(source.len())?;unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),self.ptr.as_ptr().add(self.len.get()),source.len(),)};self.len=Checked::new(needed);Ok(())}'),
    ('src/vec.rs','try_append','Vec'):('vec_extend_copy_caller','{letneeded=self.reserve_length(other.len.get())?;unsafe{core::ptr::copy_nonoverlapping(other.ptr.as_ptr(),self.ptr.as_ptr().add(self.len.get()),other.len.get(),)};self.len=Checked::new(needed);other.len=Checked::new(0);Ok(())}'),
    ('src/vec.rs','try_split_off','Vec'):('vec_split_off_caller','{ifat>self.len.get(){returnErr(Error::Bounds);}letlen=(self.len-Checked::new(at)).map_err(|_|Error::Bounds)?.get();letmuttail=Self::empty(self.policy.try_clone()?,self.limit);tail.try_reserve_exact(len)?;unsafe{core::ptr::copy_nonoverlapping(self.ptr.as_ptr().add(at),tail.ptr.as_ptr(),len)};tail.len=Checked::new(len);self.len=Checked::new(at);Ok(tail)}'),
    ('src/vec.rs','try_extend_cloned','Vec'):('vec_extend_copy_caller','{letend=required_len(self.len.get(),source.len(),self.max_len())?;ifletSome(result)=T::try_extend_vec(source,self){returnresult;}ifT::copy_witness().is_some(){letneeded=self.reserve_length(source.len())?;unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),self.ptr.as_ptr().add(self.len.get()),source.len(),)};self.len=Checked::new(needed);returnOk(());}letstart=self.len.get();self.try_resize_with_init(end,|index,slot|{letoffset=(Checked::new(index)-Checked::new(start)).map_err(|_|Error::Bounds)?.get();unsafe{source.get_unchecked(offset)}.try_clone_into(slot)}).map_err(flatten_error)}',('unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),self.ptr.as_ptr().add(self.len.get()),source.len(),)}',)),
    ('src/vec/convenience.rs','try_extend_copy_from_within','Vec'):('vec_extend_within_caller','{let(start,end)=edit::checked_range(range,self.len.get())?;letcount=(Checked::new(end)-Checked::new(start)).map_err(|_|Error::Bounds)?;letneeded=self.reserve_length(count.get())?;unsafe{core::ptr::copy_nonoverlapping(self.ptr.as_ptr().add(start),self.ptr.as_ptr().add(self.len.get()),count.get(),);}self.len=Checked::new(needed);Ok(())}'),
    ('src/vec/convenience.rs','try_into_array','Vec'):('vec_into_array_caller','{ifself.len.get()!=N{returnErr(Rejected{value:self,error:Error::Bounds,});}self.len=Checked::new(0);Ok(unsafe{self.ptr.as_ptr().cast::<[T;N]>().read()})}'),
    ('src/vec/convenience.rs','try_from_array_in','Vec'):('vec_from_array_caller','{letmutresult=Self::empty(policy,limit);ifletErr(error)=result.try_reserve_exact(N){returnErr(Rejected{value:values,error,});}letvalues=mem::ManuallyDrop::new(values);unsafe{core::ptr::copy_nonoverlapping(values.as_ptr(),result.ptr.as_ptr(),N);}result.len=Checked::new(N);Ok(result)}'),
    ('src/vec/edit.rs','drop','Compact'):('compact_drop_caller','{ifself.state.read()==self.state.written(){return;}letremaining=self.state.remaining();unsafe{core::ptr::copy(self.owner.ptr.as_ptr().add(self.state.read()),self.owner.ptr.as_ptr().add(self.state.written()),remaining,);}self.owner.len=Checked::new(self.state.restored_len());}'),
    ('src/vec/edit.rs','drop','ExtractIf'):('extract_drop_caller','{ifself.state.written()!=self.state.read(){unsafe{core::ptr::copy(self.owner.ptr.as_ptr().add(self.state.read()),self.owner.ptr.as_ptr().add(self.state.written()),self.state.unvisited_len(),);}}self.owner.len=Checked::new(self.state.restored_len());}'),
    ('src/vec/edit.rs','fmt','ExtractIf'):('extract_peek_caller','{letpeek=ifself.state.read()<self.state.end(){Some(unsafe{&*self.owner.ptr.as_ptr().add(self.state.read())})}else{None};f.debug_struct("ExtractIf").field("peek",&peek).finish_non_exhaustive()}'),
    ('src/vec/edit.rs','restore_tail','Drain'):('drain_restore_tail_caller','{unsafe{core::ptr::copy(self.owner.ptr.as_ptr().add(self.tail_start),self.owner.ptr.as_ptr().add(self.owner.len.get()),self.tail_len,);}self.owner.len=Checked::new(crate::arithmetic::bounded::wrapping_add(self.owner.len.get(),self.tail_len,));self.tail_len=0;}'),
    ('src/vec/edit.rs','move_tail','Splice'):('splice_move_tail_caller','{letrequired=required_len(self.drain.tail_len,additional,usize::MAX)?;letnext_tail=(Checked::new(self.drain.tail_start)+Checked::new(additional)).map_err(|_|Error::Capacity)?;self.drain.owner.try_reserve(required)?;unsafe{core::ptr::copy(self.drain.owner.ptr.as_ptr().add(self.drain.tail_start),self.drain.owner.ptr.as_ptr().add(next_tail.get()),self.drain.tail_len,);}self.drain.tail_start=next_tail.get();Ok(())}'),
    ('src/vec/edit.rs','try_finish','Splice'):('reserved_slot_caller','{letinitial_capacity=self.drain.owner.cap.get();self.drain.clear_remaining();ifself.drain.tail_len==0{whileletSome(value)=self.replacement.try_next().map_err(InitError::Initialize)?{self.drain.owner.try_push(value).map_err(|e|e.error)?;}returnOk(());}if!self.fill()?{returnOk(());}letSome(first)=self.replacement.try_next().map_err(InitError::Initialize)?else{returnOk(());};letavailable=((Checked::new(self.drain.owner.max_len())-self.drain.owner.len).map_err(|_|Error::Capacity)?-Checked::new(self.drain.tail_len)).map_err(|_|Error::Capacity)?.get();ifavailable==0{returnErr(InitError::Storage(Error::Capacity));}letlower=self.replacement.try_size_hint().map_err(InitError::Initialize)?.0;letadditional=iflower==0{letreserve=(Checked::new(self.drain.tail_len)+Checked::new(1)).map_err(|_|Error::Capacity)?;self.drain.owner.try_reserve(reserve.get())?;((self.drain.owner.cap-self.drain.owner.len).map_err(|_|Error::Capacity)?-Checked::new(self.drain.tail_len)).map_err(|_|Error::Capacity)?.get()}else{lower.saturating_add(1).min(available)};self.move_tail(additional)?;letnext=(self.drain.owner.len+Checked::new(1)).map_err(|_|Error::Capacity)?;unsafe{self.drain.owner.ptr.as_ptr().add(self.drain.owner.len.get()).write(first);}self.drain.owner.len=next;if!self.fill()?{returnOk(());}letSome(first)=self.replacement.try_next().map_err(InitError::Initialize)?else{returnOk(());};self.stage_excess(first,initial_capacity)}'),
}
STATE_CONDITIONS={DRIVER:['raw_valid(a)','n<=a.cap'],'collection_driver_caller':['raw_valid(a)','initial<=end<=a.cap'],
    'vec_pop_caller':['raw_valid(a)','len<=a.cap'],'vec_truncate_caller':['raw_valid(a)','len<=a.cap'],
    'vec_push_within_caller':['raw_valid(a)','len<=a.cap'],'vec_insert_mut_caller':['raw_valid(a)','len<a.cap'],
    SLOT:['raw_valid(a)','len<a.cap'],
    'vec_extend_copy_caller':['raw_valid(a)','raw_valid(b)','additional<=b.cap','a and b disjoint'],
    'vec_extend_within_caller':['raw_valid(a)','start<=end<=len'],
    'vec_split_off_caller':['raw_valid(a)','raw_valid(b)','len<=a.cap','a and b disjoint'],
    'vec_from_array_caller':['raw_valid(a)','raw_valid(b)','n<=b.cap','a and b disjoint'],
    'vec_into_array_caller':['raw_valid(a)','len<=a.cap'],
    'buffer_append_caller':['raw_valid(a)','raw_valid(b)','additional<=b.cap','len<=cap before','a and b disjoint'],
    'buffer_push_caller':['raw_valid(a)','len<=cap before'],
    'whole_copy_caller':['raw_valid(a)','raw_valid(b)','n<=a.cap','n<=b.cap','a and b disjoint'],
    'compact_drop_caller':['raw_valid(a)','state.wf()','state@.original<=a.cap'],
    'extract_drop_caller':['raw_valid(a)','state.wf()','state@.total<=a.cap'],
    'extract_peek_caller':['raw_valid(a)','state.wf()','state@.total<=a.cap'],
    'drain_restore_tail_caller':['raw_valid(a)','len<=tail_start','tail_start+tail_len<=a.cap'],
    'splice_move_tail_caller':['raw_valid(a)','len==tail_start','tail_start+tail_len<=cap before']}
# Functions between those callers and the driver. Their bodies are compared
# whole: the element count and the index reach the callback unchanged.
PINNED_BODIES={
    ('src/clone.rs','try_copy_slice_from','[T ; N]'):'{letlen=matchgeometry::flatten_count(source.len(),destination.len(),N){Ok(len)=>len,Err(error)=>returnSome(Err(error)),};let(source,destination)=unsafe{(core::slice::from_raw_parts(source.as_ptr().cast::<T>(),len),core::slice::from_raw_parts_mut(destination.as_mut_ptr().cast::<T>(),len),)};T::try_copy_slice_from(source,destination)}',
    ('src/init/prefix.rs','initialize',''):'{letmutprefix=Prefix{ptr,state:Checked::new(0),};super::driver::run(&mutprefix.state,length,|index|unsafe{initialize_one(ptr,index,initialize)})?;prefix.state=Checked::new(0);Ok(())}',
    # Reservation bodies: the capacity they publish is the kernel result.
    ('src/vec.rs', 'reserve_length', 'Vec'):'{letneeded=required_len(self.len.get(),additional,self.max_len())?;ifneeded>self.cap.get(){self.grow_to(needed,false)?;}Ok(needed)}',
    ('src/vec.rs', 'grow_to', 'Vec'):'{letcap=geometry::growth_for_layout(self.cap.get(),needed,self.max_len(),mem::size_of::<T>(),exact,)?;self.resize_storage(cap)}',
    ('src/vec.rs', 'grow', 'Vec'):'{letneeded=required_len(self.len.get(),additional,self.max_len())?;self.grow_to(needed,exact)}',
    # Push growth: the same admission, proposal and layout as grow(1, false), by value.
    ('src/vec.rs', 'try_push', 'Vec'):'{ifself.len.get()>=self.cap.get(){let(ptr,cap,len,max_len)=(self.ptr,self.cap,self.len.get(),self.max_len());letbudget=self.policy.budget();matchunsafe{grow_for_push(ptr,cap,len,max_len,budget)}{Ok((ptr,cap))=>{self.ptr=ptr;self.cap=cap;}Err(error)=>returnErr(Rejected{value,error}),}}self.push_within_capacity(value).map(|_|())}',
    ('src/vec.rs', 'grow_for_push', ''):'{letneeded=required_len(len,1,max_len)?;letgrown=geometry::growth_for_layout(cap.get(),needed,max_len,mem::size_of::<T>(),false)?;let(capacity,new_layout)=crate::capacity::Capacity::try_new(grown)?;letptr=ifcap.get()==0{storage::allocate(new_layout,budget)?}else{unsafe{storage::reallocate(ptr.cast(),cap.layout(),new_layout,budget)}?};Ok((ptr.cast(),capacity))}',
    ('src/vec.rs', 'resize_storage', 'Vec'):'{let(capacity,new_layout)=crate::capacity::Capacity::try_new(cap)?;letptr=ifself.cap.get()==0{storage::allocate(new_layout,self.policy.budget())?}else{unsafe{storage::reallocate(self.ptr.cast(),self.layout(),new_layout,self.policy.budget(),)}?};self.ptr=ptr.cast();self.cap=capacity;Ok(())}',
    ('src/vec.rs', 'try_reserve', 'Vec'):'{letspare=(self.cap-self.len).map_err(|_|Error::Capacity)?;ifadditional>spare.get(){self.grow(additional,false)?;}Ok(())}',
    ('src/vec.rs', 'try_reserve_exact', 'Vec'):'{letspare=(self.cap-self.len).map_err(|_|Error::Capacity)?;ifadditional>spare.get(){self.grow(additional,true)?;}Ok(())}',
    ('src/boxed/buffer.rs', 'reserve', 'Buffer'):'{let(needed,proposed)=super::buffer_numeric::reservation(self.len.get(),self.cap.get(),additional,hint)?;ifneeded<=self.cap.get(){returnOk(needed);}letlayout=crate::layout::array::<T>(proposed)?;letptr=unsafe{storage::reallocate(self.ptr.cast(),self.layout,layout,self.policy.budget())}?;self.ptr=ptr.cast();self.cap=Checked::new(proposed);self.layout=layout;Ok(needed)}',
    ('src/init/collection.rs','fill',''):'{letmutcurrent=*state;letprogress=Commit{output:state,current:&mutcurrent,};finished(super::driver::run(progress.current,end,|index|unsafe{write_next(ptr,index,iter)}))}',
    ('src/init/collection.rs','drop','Commit'):'{*self.output=*self.current;}',
    ('src/clone.rs','array_from_fn',''):"{letmutstorage=core::mem::MaybeUninit::<[T;N]>::uninit();letptr=core::ptr::NonNull::from(&mutstorage).cast::<T>();letmutinitialize=|index,slot:&mutcrate::init::InitSlot<'_,T>|{slot.write(f(index)?).map(|_|()).map_err(|e|e.error)};unsafe{crate::init::prefix::initialize(ptr,N,&mutinitialize)}.map_err(|e|matche{crate::error::InitError::Storage(e)|crate::error::InitError::Initialize(e)=>e,})?;Ok(unsafe{storage.assume_init()})}",
    ('src/init.rs','try_init_array','InitSlot'):'{usecrate::error::InitError;ifself.initialized{returnErr(InitError::Storage(Error::AlreadyInitialized));}unsafe{prefix::initialize(self.ptr.cast::<T>(),N,&mutinitialize)}?;self.initialized=true;Ok(())}',
    ('src/boxed/slice.rs','try_init_slice_in','Box'):'{letlayout=crate::layout::array::<T>(len).map_err(InitError::Storage)?;letptr=storage::allocate(layout,policy.budget()).map_err(InitError::Storage)?.cast::<T>();letallocation=AllocationGuard(ptr.cast(),layout,policy.budget());unsafe{crate::init::prefix::initialize(ptr,len,&mutinitialize)}?;core::mem::forget(allocation);Ok(unsafe{Self::from_raw_parts(NonNull::slice_from_raw_parts(ptr,len),policy)})}',
}


def generate(rows,operations,census):
    output=[];bindings=[];consumed=set();expected_blocks={}
    def body(file,function,context):
        matches=[r for r in census if r['kind']=='body' and r['file']==file and r['function']==function and r['context'].split('<')[0].strip()==context]
        assert len(matches)==1,('state caller body lookup',file,function,context,len(matches))
        return compact(matches[0]['expression'])
    assert len(operations)==len(rows)
    for key,expected in PINNED_BODIES.items():
        assert body(*key)==expected,('state caller body changed',key)
    for row,parsed in zip(rows,operations):
        if row['kind'] not in ('unsafe_block','macro_unsafe_block'):continue
        text=compact(row['expression']);f,fn,ctx=row['file'],row['function'],row['context']
        if (f,fn,ctx.split('<')[0].strip())==('src/clone.rs','try_copy_slice_from','[T ; N]'):
            # The original flatten_count result establishes the flattened extent
            # in array-caller.rs. The whole-body pin is correspondence only.
            bindings.append(dict(site=row,function='array_slice_caller',phase_conditions=[],queries=[],kind='direct-array-layout-adapter'))
            continue
        if f=='src/vec/edit.rs' and (fn in ('try_retain_mut','try_dedup_by') or (fn=='next' and ctx.startswith('ExtractIf'))):
            # These five leaves are composed with the actual executable driver
            # in maintained adapters. The caller audit checks their exact routing;
            # no pointer expression or control body is projected for this group.
            bindings.append(dict(site=row,function='retained_loop' if fn=='try_retain_mut' else 'dedup_loop' if fn=='try_dedup_by' else 'extract_next_loop',
                phase_conditions=[],queries=[],kind='direct-driver-adapter'))
            continue
        if f=='src/vec/edit.rs' and fn in ('stage_push', 'stage_batch'):
            bindings.append(dict(site=row,function='staged_collection_loop',phase_conditions=[],queries=[],kind='direct-owning-stage-leaf'))
            continue
        if f=='src/vec/edit.rs' and fn=='fill':
            bindings.append(dict(site=row,function='splice_fill_loop',phase_conditions=[],queries=[],kind='direct-initialization-adapter'))
            continue
        # Both storages hold the same element type: one size and one alignment.
        if (f,fn) in KERNEL_CALLERS:
            adapter,block=KERNEL_CALLERS[(f,fn)]
            assert text==block,('kernel caller unsafe block',row)
            bindings.append(dict(site=row,function=adapter,phase_conditions=[],queries=[],kind='direct-kernel-adapter'))
            continue
        state_key=(f,fn,ctx.split('<')[0].strip())
        if state_key in STATE_CALLERS:
            adapter,expected,*block=STATE_CALLERS[state_key]
            assert body(*state_key)==expected,('state caller body changed',state_key)
            assert text in expected and (block or expected.count('unsafe{')==1),('state caller unsafe block',row)
            # A function with several unsafe blocks names the one bound here;
            # its other blocks continue to the ordinary classification.
            blocks=() if not block else (block[0],) if isinstance(block[0],str) else block[0]
            if not blocks or text in blocks:
                assert (state_key,text) not in consumed,('state caller matched twice',row)
                consumed.add((state_key,text));expected_blocks[state_key]=max(len(blocks),1)
                bindings.append(dict(site=row,function=adapter,
                    phase_conditions=STATE_CONDITIONS.get(adapter,['raw_valid(a)','span.wf()','span@.back<=a.cap']),
                    queries=[],kind='direct-state-adapter'))
                continue
        if (f,fn,text) in INVARIANT_CALLERS:
            assert (f,fn,text) not in consumed,('invariant caller matched twice',row)
            consumed.add((f,fn,text))
            bindings.append(dict(site=row,function=INVARIANT_CALLERS[(f,fn,text)],
                phase_conditions=INVARIANT_CONDITIONS.get(INVARIANT_CALLERS[(f,fn,text)],['raw_valid(a)','len<=a.cap']),
                queries=[],kind='direct-owner-invariant'))
            continue
        guarded=(f,fn) in INVARIANT_FUNCTIONS
        assumptions=['len<=a.cap','b.size==a.size','b.align==a.align','source_len<=b.cap']
        replacements={
            'self.owner.len.get()':'len','self.drain.owner.len.get()':'len',
            'guard.owner.len.get()':'len','g.owner.len.get()':'len','self.len.get()':'len',
            'self.owner.cap.get()':'a.cap','self.cap.get()':'a.cap',
            'self.span.front()':'front','self.span.len()':'back-front',
            'self.state.read()':'read','self.state.written()':'write',
            'self.state.unvisited_len()':'original-read','self.state.remaining()':'original-read',
            'self.tail_start':'tail_start','self.tail_len':'tail_len',
            'self.drain.tail_start':'tail_start','self.drain.tail_len':'tail_len',
            'self.drain.owner.len':'len','self.owner.len':'len','self.len':'len',
            'self.cap':'a.cap','ptr.len()':'len','self.len()':'len',
            'self.state.get()':'len','self.data':'data','p.len':'prefix','prefix.len.get()':'prefix','prefix.len':'prefix',
            'source.len()':'source_len','values.len()':'source_len','part.len()':'source_len',
            'next.get()':'next','next_tail.get()':'next_tail','shifted.get()':'count',
            'removed.get()':'count','count.get()':'count','start.get()':'start',
            'initialized.len.get()':'prefix','initialized.len':'prefix','N':'n',
        }
        if f=='src/boxed/buffer.rs':
            if fn in ('push','collect','collect_rest'):assumptions+=['len<a.cap']
            if fn=='append':assumptions+=['len as int+source_len as int<=a.cap as int']
        if f=='src/boxed/slice.rs':
            if fn=='try_init_slice_in':assumptions+=['prefix<target<=a.cap']
            if fn in ('try_clone_from_slice_in','try_clone_in'):assumptions+=['index<source_len<=a.cap']
            if fn=='try_copy_from_slice_in':assumptions+=['source_len<=a.cap']
            if fn=='try_into_array':assumptions+=['n==len']
        if f=='src/clone.rs':
            if fn=='array_from_fn':assumptions+=['prefix<n<=a.cap'] if '.add(' in text else ['n<=a.cap']
            elif fn.startswith('try_clone'):assumptions+=['index<n<=a.cap']
            elif fn=='try_copy_slice':assumptions+=['len<=a.cap','len<=b.cap','source_len<=len']
        if f=='src/init.rs' and fn=='try_init_array':assumptions+=['prefix<n<=a.cap']
        if f=='src/init.rs' and fn=='drop':assumptions+=['len<=a.cap']
        if f=='src/init/prefix.rs' and fn=='initialize_one':assumptions+=['index<a.cap']
        if f=='src/init/collection.rs' and fn=='write_next':assumptions+=['index<target<=a.cap']
        if f in ('src/vec.rs','src/vec/convenience.rs'):
            if fn in ('try_resize_with_init','try_resize_owned','push_within_capacity'):
                assumptions+=['len<a.cap','index==len']
            elif fn=='try_push_mut':assumptions+=['index<len']
            elif fn=='try_insert_mut':assumptions+=['index<len']
            elif fn=='try_clone_in':assumptions+=['index<source_len<=a.cap']
            elif fn=='try_extend_cloned' and 'copy_nonoverlapping' in text:
                # Copy-witnessed bulk branch: same reserved-suffix copy as try_extend_copy.
                assumptions+=['len as int+source_len as int<=a.cap as int']
            elif fn=='try_extend_cloned':
                assumptions+=['start<=index<target','source_len as int==target as int-start as int']
            elif fn=='try_extend_from_within':
                assumptions+=['index<original<=len','len<a.cap']
            elif fn=='try_extend_copy_from_within':
                assumptions+=['start<=end<=len','count==end-start','len as int+count as int<=a.cap as int']
            elif fn in ('try_extend_copy','try_append'):
                assumptions+=['len as int+source_len as int<=a.cap as int']
                replacements['other.len.get()']='source_len'
            elif fn=='try_split_off':
                assumptions+=['start<=original<=a.cap','count==original-start','count<=b.cap']
                replacements['at']='start';replacements['len']='count'
            elif fn=='try_from_array_in':assumptions+=['n<=a.cap','n<=b.cap']
            elif fn=='try_into_array':assumptions+=['n==original<=a.cap']
            elif fn=='try_write_vectored':
                assumptions+=['destination as int+source_len as int<=a.cap as int']
            elif fn=='pop':assumptions+=['0<len','next==len-1']
            elif fn=='truncate':
                assumptions+=['start<=original<=a.cap','count==original-start'];replacements['len']='start'
        if f=='src/vec/edit.rs':
            if ctx.startswith(('ExtractIf','Compact')):
                assumptions+=['write<=read<=original<=a.cap']
                if fn in ('next','fmt'):assumptions+=['read<original']
                if 'copy_nonoverlapping' in text:assumptions+=['write<read']
                replacements['remaining']='original-read'
            elif fn=='move_tail':
                assumptions+=['len==tail_start','tail_start as int+tail_len as int<=a.cap as int','next_tail as int+tail_len as int<=a.cap as int']
            elif fn=='try_finish':assumptions+=['len<tail_start','tail_start as int+tail_len as int<=a.cap as int']
            elif fn=='stage_excess':
                if 'extra.ptr' in text and 'self.drain.owner' not in text:
                    assumptions+=['len<a.cap'];replacements['extra.len.get()']='len'
                else:
                    assumptions+=['len==tail_start','len as int+tail_len as int+source_len as int<=a.cap as int','next_tail as int==tail_start as int+source_len as int']
                    replacements['extra.len.get()']='source_len'
            else:
                assumptions+=['front<=back<=tail_start','tail_start as int+tail_len as int<=a.cap as int','len<=tail_start']
                if fn in ('next','next_back'):assumptions+=['index<a.cap']
                if fn=='clear_remaining':replacements['start']='front';replacements['count']='back-front'
        if f in ('src/vec/ownership.rs','src/boxed/slice.rs') and fn in ('as_slice','as_mut_slice','next','next_back','drop') and ('span.' in text or '.add(i)' in text or '.add(index)' in text):
            assumptions+=['front<=back<=a.cap']
            if fn in ('next','next_back'):assumptions+=['index<a.cap']
            replacements['remaining']='back-front'
        if f=='src/vec/ownership.rs' and fn=='try_clone_in':assumptions+=['index<source_len<=a.cap']
        if fn=='try_clone_from' and f in ('src/boxed/string.rs','src/boxed/foreign.rs'):assumptions+=['source_len==len']

        def scalar(value):
            value=bounded_projection(value)
            for old,new in sorted(replacements.items(),key=lambda p:-len(p[0])):
                value=re.sub(r'\b'+re.escape(old)+r'\b',lambda _:new,value) if re.fullmatch(r'\w+',old) else value.replace(old,new)
            value=re.sub(r'\bi\b','index',value)
            value=value.replace('self.state.total','original')
            assert re.fullmatch(r'[\w.()+* \-]+',value),('unhandled source scalar',row,value)
            allowed={'a','cap','len','source_len','index','read','write','previous','original','start','end','front','back','tail_start','tail_len','next_tail','next','prefix','target','count','n','destination','offset'}
            assert set(re.findall(r'\b[a-zA-Z_]\w*\b',value))<=allowed,('unbound source scalar',row,value)
            return value

        def pointer(value):
            if value in ('item','current'):return 'a','read'
            if value=='place':return 'a','len' if fn=='push_within_capacity' else 'index'
            if value=='destination':return 'a','destination'
            if value.startswith('place.add('):return 'a','index+'+scalar(call_args(value,'place.add')[0])
            other=any(x in value for x in ('source.as_ptr()','source.as_str().as_ptr()','source.as_c_str().to_bytes_with_nul().as_ptr()','source.as_os_str()','values.as_ptr()','part.as_ptr()','other.ptr','tail.ptr','extra.ptr'))
            allocation='b' if other else 'a'
            offset='0'
            if '.add(' in value:offset=scalar(call_args(value,value[:value.index('.add(')+4])[0])
            if '.add(' not in value and allocation=='a' and value not in ('ptr','pointer','self.ptr.as_ptr()'):
                # Casts and borrowed/owner base pointers are typed ABI transfers.
                assert any(token in value for token in ('.as_ptr()','.as_mut_ptr()','.cast',' as*','as*','__ptr','__coerced')),('unbound pointer base',row,value)
            return allocation,offset

        statements=[];addresses=[]
        # .add calls carry the exact offset argument from each actual block.
        for m in re.finditer(r'\.add\(',text):
            arg=call_args(text[m.start():],'.add')[0]
            offset=scalar(arg)
            # A chained place.add(1) starts at the already indexed place.
            prefix_text=text[:m.start()]
            if prefix_text.endswith('place'):offset='index+('+offset+')'
            allocation='b' if prefix_text.endswith('extra.ptr.as_ptr()') and fn=='stage_excess' and 'self.drain.owner' in text else 'a'
            if prefix_text.endswith('tail.ptr.as_ptr()'):allocation='b'
            statements.append('let _=offset_storage('+allocation+','+offset+');')
            addresses.append((allocation,offset))
        for marker in ['core::slice::from_raw_parts','core::slice::from_raw_parts_mut','core::ptr::slice_from_raw_parts_mut']:
            for m in re.finditer(re.escape(marker)+r'\(',text):
                ptr,count=call_args(text[m.start():],marker);allocation,offset=pointer(ptr)
                statements.append('let _=slice_caller('+allocation+','+offset+','+scalar(count)+');')
        for m in re.finditer(r'NonNull::slice_from_raw_parts\(',text):
            ptr,count=call_args(text[m.start():],'NonNull::slice_from_raw_parts')
            extent=scalar(count)
            assert extent!='1',('unreviewed scalar value access',row)
            statements.append('let _=slice_caller(a,0,'+extent+');')
        for m in re.finditer(r'\.get_unchecked\(',text):
            arg=scalar(call_args(text[m.start():],'.get_unchecked')[0])
            limit='n' if f=='src/clone.rs' else 'source_len'
            # Clone offsets use the checked local subtraction, whose exact
            # expression is also proved by clone_input_offset in the base.
            if arg=='offset':arg='index-start'
            statements.append('let accepted=unchecked_index_precondition('+arg+','+limit+');assert(accepted);')
        # A pointer may reach the one-past position, but a read/write may not.
        # Check the actual access as well as its preceding .add operation.
        access_text=re.sub(r'(?:self|g)\.state\.read\(\)','',text)
        direct_access=('.read(' in access_text or '.write(' in access_text or 'core::ptr::read(' in access_text)
        if direct_access:
            if '.cast::<[T;N]>().read()' in text:
                statements.append('let _=slice_caller(a,0,n);')
            elif 'place.write(' in text or 'place.read(' in text:
                offset='len' if fn=='push_within_capacity' else 'index'
                statements.append('let _=slice_caller(a,'+offset+',1);')
            elif addresses:
                allocation,offset=addresses[-1]
                statements.append('let _=slice_caller('+allocation+','+offset+',1);')
            elif 'core::ptr::read(current)' in text:
                statements.append('let _=slice_caller(a,read,1);')
            else:
                # One complete typed value: only the reviewed blocks in
                # INVARIANT_CALLERS may rest on that layout fact.
                raise AssertionError(('unreviewed scalar value access',row))
        for marker,overlap in [('core::ptr::copy_nonoverlapping',False),('core::ptr::copy',True)]:
            for m in re.finditer(re.escape(marker)+r'\(',text):
                src,dst,count=call_args(text[m.start():],marker)
                source,source_index=pointer(src);dest,dest_index=pointer(dst);count=scalar(count)
                if source==dest:
                    statements.append('overlapping_copy_caller(a,'+source_index+','+dest_index+','+count+');')
                    if not overlap:
                        # A disjoint logical gap is the only same-owner
                        # nonoverlapping move; use its ordered span theorem.
                        statements.append('inside_nonoverlap_caller(a,'+dest_index+','+source_index+','+count+');' if fn=='next' else 'inside_nonoverlap_caller(a,'+source_index+','+dest_index+','+count+');')
                else:
                    assumptions+=['a.base as int+a.cap as int*a.size as int<=b.base as int || b.base as int+b.cap as int*b.size as int<=a.base as int']
                    statements.append('distinct_nonoverlap_caller('+source+','+dest+','+source_index+','+dest_index+','+count+');')
        # Two independent classifications must agree: the parsed operations of
        # the block, and the absence of any offset or count query above.
        site_operations={(op['kind'],op['operation']) for op in parsed.get('operations',[])}
        if site_operations<=REPRESENTATION:
            assert not statements,('representation-only operations with an offset or count query',row,statements)
            bindings.append(dict(site=row,function='representation_caller',phase_conditions=['raw_valid(a)'],
                queries=['representation_caller(a);'],kind='direct-representation'))
            continue
        assert statements,('unsafe operation without an offset or count query',row,sorted(site_operations-REPRESENTATION))
        assert not guarded,('unreviewed offset-bearing block in an owner-invariant function',row)
        number=len(bindings);name='actual_caller_'+str(number)
        parameters='a:RawStorage,b:RawStorage,len:usize,source_len:usize,index:usize,read:usize,write:usize,previous:usize,original:usize,start:usize,end:usize,front:usize,back:usize,tail_start:usize,tail_len:usize,next_tail:usize,next:usize,prefix:usize,target:usize,count:usize,n:usize,destination:usize'
        output.append('fn '+name+'('+parameters+')\n requires raw_valid(a),raw_valid(b),'+','.join(dict.fromkeys(assumptions))+',\n{'+''.join(statements)+'}\n')
        bindings.append(dict(site=row,function=name,phase_conditions=list(dict.fromkeys(assumptions)),queries=statements))
    assert {k for k in consumed if len(k)==3}==set(INVARIANT_CALLERS),('owner-invariant caller text changed or missing',sorted(set(INVARIANT_CALLERS)-consumed))
    bound={key:sum(1 for k in consumed if len(k)==2 and k[0]==key) for key in STATE_CALLERS}
    assert set(expected_blocks)==set(STATE_CALLERS) and bound==expected_blocks,('state caller body changed',sorted(k for k in STATE_CALLERS if bound[k]!=expected_blocks.get(k)))
    return 'verus! {\n'+''.join(output)+'}\n',bindings
