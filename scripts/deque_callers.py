"""Whole-body text of the VecDeque product functions followed by maintained callers.

Each maintained caller in tools/memory-proof/deque.verus.rs was written
against exactly this text (comments and whitespace removed). A changed body
needs the caller to be read again; the comparison is not a solver proof.
Keys are (file under src/, unique anchor or '', function head after the anchor).
"""
BODIES={
 ('deque/iter.rs', "Drop for Finish<'_, '_, T, P, L> {", 'fn drop('):
  'forvalueinself.0.by_ref(){drop(value)}letd=&mutself.0;letremoved=sub(d.end,d.start);ifremoved!=0{unsafe{ifd.start<d.tail{d.owner.copy_ring(d.owner.physical(0),d.owner.physical(removed),d.start,true,);ifmem::size_of::<T>()!=0{d.owner.head=Checked::new(geometry::physical(d.owner.head.get(),removed,d.owner.capacity(),))}}elseifd.tail!=0{d.owner.copy_ring(d.owner.physical(d.end),d.owner.physical(d.start),d.tail,false,)}}}d.owner.len=Checked::new(add(d.start,d.tail));',
 ('deque/iter.rs', '', 'fn fold<B, F: FnMut(B, T) -> B>(mut self'):
  "structRetire<'a,T,P:AllocationPolicy,L:LengthPolicy>{owner:&'amutVecDeque<T,P,L>,consumed:usize,}impl<T,P:AllocationPolicy,L:LengthPolicy>DropforRetire<'_,T,P,L>{fndrop(&mutself){ifself.consumed!=0{self.owner.len=Checked::new(sub(self.owner.len(),self.consumed));ifmem::size_of::<T>()!=0{self.owner.head=Checked::new(geometry::physical(self.owner.head.get(),self.consumed,self.owner.capacity(),))}}}}letmutguard=Retire{owner:&mutself.owner,consumed:0,};let(a,b)=guard.owner.as_slices();letcount=&mutguard.consumed;letmutconsume=|acc,value:&T|{*count=add(*count,1);f(acc,unsafe{ptr::read(value)})};letvalue=a.iter().fold(initial,&mutconsume);b.iter().fold(value,consume)",
 ('deque.rs', "impl Drop for Allocation<'_> {", 'fn drop('):
  'unsafe{storage::deallocate(self.0,self.1,self.2)}',
 ('deque.rs', '', 'pub fn into_vec('):
  'self.normalize();letowner=mem::ManuallyDrop::new(self);unsafe{Vec::from_raw_parts(owner.ptr,owner.len(),owner.capacity(),ptr::read(&owner.policy),owner.limit,)}',
 ('deque.rs', '', 'pub fn try_from_std_in('):
  'letcap=ifmem::size_of::<T>()==0{limit.max_len()}else{source.capacity()};ifsource.len()>limit.max_len()||cap>limit.max_len(){returnErr(Rejected{value:source,error:Error::Capacity,});}let(_,layout)=matchCapacity::<T>::try_new(cap){Ok(x)=>x,Err(error)=>{returnErr(Rejected{value:source,error,});}};{letreservation=matchpolicy.budget().reserve(layout.size()){Ok(r)=>r,Err(error)=>{returnErr(Rejected{value:source,error,});}};ifletSome(r)=reservation{r.commit().retain()}}letmutvec=mem::ManuallyDrop::new(alloc::vec::Vec::from(source));letowner=unsafe{Vec::from_raw_parts(NonNull::new_unchecked(vec.as_mut_ptr()),vec.len(),cap,policy,limit,)};Ok(Self::from_vec(owner))',
 ('deque.rs', '', 'fn physical('):
  'ifmem::size_of::<T>()==0{0}else{geometry::allocated_physical(self.head.get(),index,self.capacity())}',
 ('deque.rs', '', 'unsafe fn slot('):
  'unsafe{self.ptr.as_ptr().add(self.physical(index))}',
 ('deque.rs', '', 'pub fn as_slices('):
  'let(a,b)=geometry::split(self.head.get(),self.len(),self.capacity());unsafe{(slice::from_raw_parts(self.ptr.as_ptr().add(self.head.get()),a),slice::from_raw_parts(self.ptr.as_ptr(),b),)}',
 ('deque.rs', '', 'pub fn as_mut_slices('):
  'let(a,b)=geometry::split(self.head.get(),self.len(),self.capacity());unsafe{(slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.head.get()),a),slice::from_raw_parts_mut(self.ptr.as_ptr(),b),)}',
 ('deque.rs', '', 'pub fn get('):
  'ifindex>=self.len(){returnErr(Error::Bounds);}Ok(unsafe{&*self.slot(index)})',
 ('deque.rs', '', 'pub fn get_mut('):
  'ifindex>=self.len(){returnErr(Error::Bounds);}Ok(unsafe{&mut*self.slot(index)})',
 ('deque.rs', '', 'pub fn front('):
  'ifself.is_empty(){returnNone;}Some(unsafe{&*self.ptr.as_ptr().add(self.head.get())})',
 ('deque.rs', '', 'pub fn front_mut('):
  'ifself.is_empty(){returnNone;}Some(unsafe{&mut*self.ptr.as_ptr().add(self.head.get())})',
 ('deque.rs', '', 'pub fn try_push_back('):
  'letlen=self.len();iflen==self.capacity()&&letErr(error)=self.grow(1,false){returnErr(Rejected{value,error});}unsafe{self.slot(len).write(value)};self.len=Checked::new(add(len,1));Ok(())',
 ('deque.rs', '', 'pub fn try_push_front('):
  'ifself.len()==self.capacity()&&letErr(error)=self.grow(1,false){returnErr(Rejected{value,error});}ifmem::size_of::<T>()!=0{self.head=Checked::new(geometry::retreat(self.head.get(),1,self.capacity()))}self.len=Checked::new(add(self.len(),1));unsafe{self.ptr.as_ptr().add(self.head.get()).write(value)};Ok(())',
 ('deque.rs', '', 'pub fn try_push_back_mut('):
  'self.try_push_back(value)?;Ok(unsafe{&mut*self.slot(sub(self.len(),1))})',
 ('deque.rs', '', 'pub fn try_push_front_mut('):
  'self.try_push_front(value)?;Ok(unsafe{&mut*self.ptr.as_ptr().add(self.head.get())})',
 ('deque.rs', '', 'pub fn pop_front('):
  'ifself.is_empty(){returnNone;}letold_head=self.head.get();self.len=Checked::new(geometry::after_pop(self.len(),self.capacity()));ifmem::size_of::<T>()!=0{self.head=Checked::new(geometry::allocated_physical(self.head.get(),1,self.capacity(),))}letspare=self.len()<self.capacity();#[cfg(kani)]assert!(spare,"verifiedpopsparecapacity");unsafe{core::hint::assert_unchecked(spare)};Some(unsafe{self.ptr.as_ptr().add(old_head).read()})',
 ('deque.rs', '', 'pub fn pop_back('):
  'ifself.is_empty(){returnNone;}letlast=geometry::after_pop(self.len(),self.capacity());self.len=Checked::new(last);letspare=self.len()<self.capacity();#[cfg(kani)]assert!(spare,"verifiedpopsparecapacity");unsafe{core::hint::assert_unchecked(spare)};Some(unsafe{self.slot(last).read()})',
 ('deque.rs', '', 'pub fn truncate('):
  'iflen>=self.len(){return;}if!mem::needs_drop::<T>(){self.len=Checked::new(len);return;}whileself.len()>len{drop(self.pop_back())}',
 ('deque.rs', '', 'pub fn make_contiguous('):
  'letcap=self.capacity();letlen=self.len();lethead=self.head.get();let(a,b)=geometry::split(head,len,cap);letkind=geometry::contiguous_kind(head,len,cap);ifkind!=0{letfree=sub(cap,len);letp=self.ptr.as_ptr();unsafe{ifkind==1{ptr::copy(p,p.add(a),b);ptr::copy_nonoverlapping(p.add(head),p,a);self.head=Checked::new(0)}elseifkind==2{ptr::copy(p.add(head),p.add(b),a);ptr::copy_nonoverlapping(p,p.add(add(b,a)),b);self.head=Checked::new(b)}elseifkind==3{iffree!=0{ptr::copy(p,p.add(free),b)}slice::from_raw_parts_mut(p.add(free),len).rotate_left(b);self.head=Checked::new(free);}else{iffree!=0{ptr::copy(p.add(head),p.add(b),a)}slice::from_raw_parts_mut(p,len).rotate_right(a);self.head=Checked::new(0);}}}unsafe{slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.head.get()),len)}',
 ('deque.rs', '', 'fn normalize('):
  'self.make_contiguous();ifself.head.get()!=0{unsafe{ptr::copy(self.ptr.as_ptr().add(self.head.get()),self.ptr.as_ptr(),self.len(),)}self.head=Checked::new(0);}',
 ('deque.rs', '', 'pub fn try_shrink_to('):
  'letnew=min.max(self.len());ifmem::size_of::<T>()==0||new>=self.capacity(){returnOk(());}let(cap,layout)=Capacity::try_new(new)?;lethead=self.head.get();letlen=self.len();let(a,b)=geometry::split(head,len,self.capacity());letmoved=head>sub(new,len);ifmoved{self.normalize()}matchunsafe{storage::reallocate(self.ptr.cast(),self.cap.layout(),layout,self.policy.budget(),)}{Ok(ptr)=>{self.ptr=ptr.cast();self.cap=cap;ifmoved||len==0{self.head=Checked::new(0)};Ok(())}Err(e)=>{ifmoved{unsafe{ifb==0{ptr::copy(self.ptr.as_ptr(),self.ptr.as_ptr().add(head),len)}else{slice::from_raw_parts_mut(self.ptr.as_ptr(),len).rotate_left(a);ptr::copy(self.ptr.as_ptr().add(b),self.ptr.as_ptr().add(head),a);}}self.head=Checked::new(head);}Err(e)}}',
 ('deque.rs', '', 'fn grow('):
  'letneeded=crate::vec::required_len(self.len(),additional,self.max_len())?;letsize=mem::size_of::<T>();letceiling=self.max_len().min((isize::MAXasusize).checked_div(size).unwrap_or(usize::MAX),);ifneeded>ceiling{returnErr(Error::Capacity);}letminimum=ifsize==1{8}elseifsize<=1024{4}else{1};letnew=ifexact{needed}else{crate::vec::growth_capacity(self.capacity(),needed,ceiling,minimum)};let(cap,layout)=Capacity::try_new(new)?;letold=self.capacity();letptr=unsafe{storage::reallocate(self.ptr.cast(),self.cap.layout(),layout,self.policy.budget(),)?}.cast::<T>();ifold>0{lethead=geometry::grown_head(self.head.get(),self.len(),old,new);let(a,b)=geometry::split(self.head.get(),self.len(),old);ifb>0{unsafe{ifhead==self.head.get(){ptr::copy_nonoverlapping(ptr.as_ptr(),ptr.as_ptr().add(old),b)}else{ptr::copy(ptr.as_ptr().add(self.head.get()),ptr.as_ptr().add(head),a)}}}self.head=Checked::new(head);}self.ptr=ptr;self.cap=cap;Ok(())',
 ('deque.rs', '', 'pub fn try_reserve('):
  'ifadditional>sub(self.capacity(),self.len()){self.grow(additional,false)?}Ok(())',
 ('deque.rs', '', 'pub fn try_reserve_exact('):
  'ifadditional>sub(self.capacity(),self.len()){self.grow(additional,true)?}Ok(())',
 ('deque.rs', 'Drop for VecDeque<T, P, L> {', 'fn drop('):
  "structAllocation<'a>(NonNull<u8>,core::alloc::Layout,Accounting<'a>);implDropforAllocation<'_>{fndrop(&mutself){unsafe{storage::deallocate(self.0,self.1,self.2)}}}let_allocation=Allocation(self.ptr.cast(),self.cap.layout(),self.policy.budget());if!mem::needs_drop::<T>(){return;}let(a,b)=geometry::split(self.head.get(),self.len(),self.capacity());structTail<T>(*mut[T]);impl<T>DropforTail<T>{fndrop(&mutself){unsafe{ptr::drop_in_place(self.0)}}}let_tail=Tail(ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(),b));unsafe{ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.ptr.as_ptr().add(self.head.get()),a,))}",
 ('deque/edit.rs', '', 'unsafe fn copy_ring('):
  'ifmem::size_of::<T>()==0||count==0||src==dst{return;}letcap=self.capacity();letp=self.ptr.as_ptr();ifbackward{src=geometry::physical(src,count,cap);dst=geometry::physical(dst,count,cap);whilecount!=0{let(chunk,next_src,next_dst,left)=geometry::backward_chunk(src,dst,count,cap);src=next_src;dst=next_dst;unsafe{ptr::copy(p.add(src),p.add(dst),chunk)};count=left;}}else{whilecount!=0{let(chunk,next_src,next_dst,left)=geometry::forward_chunk(src,dst,count,cap);unsafe{ptr::copy(p.add(src),p.add(dst),chunk)};count=left;src=next_src;dst=next_dst;}}',
 ('deque/edit.rs', '', 'pub fn swap('):
  'ifa>=self.len()||b>=self.len(){returnErr(Error::Bounds);}unsafe{ptr::swap(self.slot(a),self.slot(b))};Ok(())',
 ('deque/edit.rs', '', 'pub fn try_insert_mut('):
  'ifindex>self.len(){returnErr(Rejected{value,error:Error::Bounds,});}ifletErr(error)=self.try_reserve(1){returnErr(Rejected{value,error});}lettail=sub(self.len(),index);unsafe{ifindex<tail{ifmem::size_of::<T>()!=0{self.head=Checked::new(geometry::retreat(self.head.get(),1,self.capacity()))}self.copy_ring(self.physical(1),self.physical(0),index,false);}else{self.copy_ring(self.physical(index),self.physical(add(index,1)),tail,true,)}letp=self.slot(index);p.write(value);self.len=Checked::new(add(self.len(),1));Ok(&mut*p)}',
 ('deque/edit.rs', '', 'pub fn remove('):
  'ifindex>=self.len(){returnNone;}lettail=sub(sub(self.len(),index),1);letvalue=unsafe{self.slot(index).read()};unsafe{ifindex<tail{self.copy_ring(self.physical(0),self.physical(1),index,true);ifmem::size_of::<T>()!=0{self.head=Checked::new(geometry::allocated_physical(self.head.get(),1,self.capacity(),))}}else{self.copy_ring(self.physical(add(index,1)),self.physical(index),tail,false,)}}self.len=Checked::new(sub(self.len(),1));Some(value)',
 ('deque/edit.rs', '', 'pub fn rotate_left('):
  'ifn>self.len(){returnErr(Error::Bounds);}ifn==0||n==self.len()||mem::size_of::<T>()==0{returnOk(());}letright=sub(self.len(),n);ifn>right{returnself.rotate_right(right);}unsafe{ifself.len()!=self.capacity(){self.copy_ring(self.physical(0),self.physical(self.len()),n,false)}}self.head=Checked::new(geometry::allocated_physical(self.head.get(),n,self.capacity(),));Ok(())',
 ('deque/edit.rs', '', 'pub fn rotate_right('):
  'ifn>self.len(){returnErr(Error::Bounds);}ifn==0||n==self.len()||mem::size_of::<T>()==0{returnOk(());}letleft=sub(self.len(),n);ifn>left{returnself.rotate_left(left);}lethead=geometry::retreat(self.head.get(),n,self.capacity());unsafe{ifself.len()!=self.capacity(){self.copy_ring(self.physical(left),head,n,true)}}self.head=Checked::new(head);Ok(())',
 ('deque/edit.rs', '', 'pub fn try_append('):
  'self.try_reserve(other.len())?;letlen=self.len();letmore=other.len();let(a,b)=other.as_slices();unsafe{self.write_slice(len,a);self.write_slice(add(len,a.len()),b);}self.len=Checked::new(add(len,more));other.len=Checked::new(0);Ok(())',
 ('deque/edit.rs', '', 'unsafe fn write_slice('):
  'ifsource.is_empty()||mem::size_of::<T>()==0{return;}letphysical=self.physical(index);letfirst=source.len().min(sub(self.capacity(),physical));unsafe{ptr::copy_nonoverlapping(source.as_ptr(),self.ptr.as_ptr().add(physical),first);ptr::copy_nonoverlapping(source.as_ptr().add(first),self.ptr.as_ptr(),sub(source.len(),first),);}',
 ('deque/edit.rs', '', 'pub fn try_split_off('):
  'ifat>self.len(){returnErr(Error::Bounds);}letcount=sub(self.len(),at);letmutresult=Self::empty(self.policy.try_clone()?,self.limit);result.try_reserve_exact(count)?;letpos=ifcount==0{0}else{self.physical(at)};letfirst=count.min(sub(self.capacity(),pos));unsafe{ptr::copy_nonoverlapping(self.ptr.as_ptr().add(pos),result.ptr.as_ptr(),first);ptr::copy_nonoverlapping(self.ptr.as_ptr(),result.ptr.as_ptr().add(first),sub(count,first),);}result.len=Checked::new(count);self.len=Checked::new(at);Ok(result)',
 ('deque/edit.rs', '', 'pub fn try_retain_mut<'):
  "structRepair<'a,T,P:AllocationPolicy,L:LengthPolicy>{owner:&'amutVecDeque<T,P,L>,state:crate::compaction::Compaction,}impl<T,P:AllocationPolicy,L:LengthPolicy>DropforRepair<'_,T,P,L>{fndrop(&mutself){ifself.state.written()!=self.state.read()&&self.state.remaining()!=0{unsafe{self.owner.copy_ring(self.owner.physical(self.state.read()),self.owner.physical(self.state.written()),self.state.remaining(),false,)}}self.owner.len=Checked::new(self.state.restored_len());}}letstate=crate::compaction::Compaction::full(self.len());letmutguard=Repair{owner:self,state};#[cfg_attr(all(kani,jzap_loop_contracts),kani::loop_invariant(guard.state.valid_for(guard.owner.len())&&guard.owner.len()<=guard.owner.capacity()&&guard.owner.capacity()<=guard.owner.max_len()&&guard.owner.max_len()==on_entry(guard.owner.max_len())&&!guard.owner.probe_ptr().is_null()))]whileletSome(read)=guard.state.peek(){letp=unsafe{guard.owner.slot(read)};iff(unsafe{&mut*p})?{letwrite=guard.state.written();guard.state.keep_live();ifread!=write{unsafe{ptr::copy(p,guard.owner.slot(write),1)}}}else{guard.state.remove_live();unsafe{ptr::drop_in_place(p)}}}Ok(())",
 ('deque/edit.rs', '', 'pub fn binary_search_by('):
  'letmutleft=0;letmutright=self.len();whileleft<right{letmid=add(left,sub(right,left)/2);matchf(unsafe{&*self.slot(mid)}){core::cmp::Ordering::Less=>left=add(mid,1),core::cmp::Ordering::Greater=>right=mid,core::cmp::Ordering::Equal=>returnOk(mid),}}Err(left)',
 ('deque/edit.rs', '', 'pub fn partition_point('):
  'letmutleft=0;letmutright=self.len();whileleft<right{letmid=add(left,sub(right,left)/2);iff(unsafe{&*self.slot(mid)}){left=add(mid,1)}else{right=mid}}left',
 ('deque/traits.rs', '', 'fn clone_to('):
  'crate::vec::required_len(0,self.len(),limit.max_len())?;letcount=ifmem::size_of::<T>()==0{limit.max_len()}else{self.len()};let(cap,layout)=Capacity::try_new(count)?;letptr=storage::allocate(layout,policy.budget())?.cast();letmutresult=Self{ptr,cap,head:Checked::new(0),len:Checked::new(0),policy,limit,marker:PhantomData,};let(a,b)=self.as_slices();ifT::copy_witness().is_some(){unsafe{result.write_clone_slice(0,a);result.write_clone_slice(a.len(),b)};result.len=Checked::new(self.len());Ok(result)}else{letmutvalues=result.into_vec();values.try_extend_cloned(a)?;values.try_extend_cloned(b)?;Ok(Self::from_vec(values))}',
 ('deque/traits.rs', '', 'fn try_clone_from('):
  'crate::vec::required_len(0,source.len(),self.max_len())?;ifsource.is_empty(){self.clear();returnOk(());}ifT::copy_witness().is_some()&&!mem::needs_drop::<T>(){ifsource.len()>self.capacity(){self.try_reserve(sub(source.len(),self.len()))?;}let(a,b)=source.as_slices();self.len=Checked::new(0);self.head=Checked::new(0);unsafe{self.write_clone_slice(0,a);self.write_clone_slice(a.len(),b);}self.len=Checked::new(source.len());returnOk(());}letmutreplacement=source.clone_to(self.policy.try_clone()?,self.limit)?;mem::swap(self,&mutreplacement);Ok(())',
 ('deque/traits.rs', '', 'unsafe fn write_clone_slice('):
  'unsafe{ptr::copy_nonoverlapping(source.as_ptr(),self.ptr.as_ptr().add(index),source.len())}',
 ('deque/traits.rs', '', 'pub fn try_extend_copy('):
  'ifvalues.is_empty(){returnOk(());}letlen=self.len();self.try_reserve(values.len())?;iflen==0{self.head=Checked::new(0);}unsafe{iflen==0{self.write_clone_slice(0,values);}else{self.write_slice(len,values);}}self.len=Checked::new(add(len,values.len()));Ok(())',
 ('deque/traits.rs', '', 'pub fn try_resize('):
  'ifnew_len<=self.len(){self.truncate(new_len);returnOk(());}ifT::copy_witness().is_some(){letextra=sub(new_len,self.len());self.try_reserve(extra)?;ifmem::size_of::<T>()!=0{lettail=self.physical(self.len());let(first,second)=geometry::split(tail,extra,self.capacity());letbase=self.ptr.as_ptr();foriin0..first{unsafe{ptr::copy_nonoverlapping(value,base.add(add(tail,i)),1)}}foriin0..second{unsafe{ptr::copy_nonoverlapping(value,base.add(i),1)}}}self.len=Checked::new(new_len);returnOk(());}self.try_resize_with(new_len,||value.try_clone()).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})',
 ('deque/iter.rs', '> Iterator for Drain<', 'fn next('):
  'self.span.next().map(|i|{unsafe{self.owner.slot(i).read()}})',
 ('deque/iter.rs', '> DoubleEndedIterator for Drain<', 'fn next_back('):
  'self.span.next_back().map(|i|{unsafe{self.owner.slot(i).read()}})',
}
# Maintained caller -> text its specification and body must contain (product
# calls, or the premise for callers that only state a range).
CALLERS={
 'deque_physical':['crate::deque::geometry::allocated_physical(head,index,cap)'],
 'deque_slot_caller':['offset_storage(s,physical)', 'slice_caller(s,physical,1)'],
 'deque_slices_caller':['crate::deque::geometry::split(head,len,cap)', 'offset_storage(s,head)', 'slice_caller(s,head,a)', 'slice_caller(s,0,b)'],
 'deque_get_caller':['deque_physical(head,index,cap,zst)', 'deque_slot_caller(s,out,cap)'],
 'deque_front_caller':['deque_slot_caller(s,head,cap)'],
 'deque_push_back_caller':['deque_physical(head,len,cap,zst)', 'crate::arithmetic::bounded::wrapping_add(len,1)', 'crate::arithmetic::bounded::wrapping_sub(next,1)', 'deque_slot_caller(s,slot,cap)'],
 'deque_push_front_caller':['crate::deque::geometry::retreat(head,1,cap)', 'crate::arithmetic::bounded::wrapping_add(len,1)', 'deque_slot_caller(s,head,cap)'],
 'deque_pop_front_caller':['crate::deque::geometry::after_pop(len,cap)', 'crate::deque::geometry::allocated_physical(head,1,cap)', 'letspare=len<cap;assert(spare);', 'deque_slot_caller(s,old_head,cap)'],
 'deque_pop_back_caller':['crate::deque::geometry::after_pop(len,cap)', 'letspare=last<cap;assert(spare);', 'deque_physical(head,last,cap,zst)', 'deque_slot_caller(s,slot,cap)'],
 'deque_contiguous_caller':['crate::deque::geometry::split(head,len,cap)', 'crate::deque::geometry::contiguous_kind(head,len,cap)', 'crate::arithmetic::bounded::wrapping_sub(cap,len)', 'crate::arithmetic::bounded::wrapping_add(b,a)',
  'overlapping_copy_caller(s,0,a,b)', 'unordered_nonoverlap_caller(s,head,0,a)', 'overlapping_copy_caller(s,head,b,a)', 'unordered_nonoverlap_caller(s,0,third,b)', 'overlapping_copy_caller(s,0,free,b)', 'slice_caller(s,free,len)', 'slice_caller(s,0,len)', 'offset_storage(s,head)', 'slice_caller(s,head,len)'],
 'deque_normalize_caller':['deque_contiguous_caller(s,head,len,cap,zst)', 'overlapping_copy_caller(s,head,0,len)'],
 'deque_shrink_caller':['crate::deque::geometry::split(head,len,cap)', 'crate::arithmetic::bounded::wrapping_sub(new,len)', 'deque_normalize_caller(s,head,len,cap,false)', 'overlapping_copy_caller(s,0,head,len)', 'slice_caller(s,0,len)', 'overlapping_copy_caller(s,b,head,a)'],
 'deque_grow_caller':['crate::deque::geometry::grown_head(head,len,old,new)', 'crate::deque::geometry::split(head,len,old)', 'unordered_nonoverlap_caller(n,0,old,b)', 'overlapping_copy_caller(n,head,moved,a)'],
 'deque_copy_ring_caller':['crate::deque::geometry::physical(src,count,cap)', 'crate::deque::geometry::physical(dst,count,cap)', 'crate::deque::geometry::backward_chunk(src,dst,count,cap)', 'crate::deque::geometry::forward_chunk(src,dst,count,cap)', 'overlapping_copy_caller(s,src,dst,chunk)'],
 'deque_insert_caller':['crate::arithmetic::bounded::wrapping_sub(len,index)', 'crate::deque::geometry::retreat(head,1,cap)', 'deque_copy_ring_caller(s,from,to,index,cap,false)', 'deque_copy_ring_caller(s,from,to,tail,cap,true)', 'crate::arithmetic::bounded::wrapping_add(len,1)', 'deque_slot_caller(s,slot,cap)'],
 'deque_remove_caller':['crate::arithmetic::bounded::wrapping_sub(crate::arithmetic::bounded::wrapping_sub(len,index),1)', 'deque_get_caller(s,head,index,len,cap,zst)', 'deque_copy_ring_caller(s,from,to,index,cap,true)', 'crate::deque::geometry::allocated_physical(head,1,cap)', 'deque_copy_ring_caller(s,from,to,tail,cap,false)', 'crate::arithmetic::bounded::wrapping_sub(len,1)'],
 'deque_rotate_caller':['crate::arithmetic::bounded::wrapping_sub(len,n)', 'crate::deque::geometry::allocated_physical(head,n,cap)', 'crate::deque::geometry::retreat(head,n,cap)', 'deque_copy_ring_caller(s,deque_physical(head,other,cap,false),moved,n,cap,true)'],
 'deque_write_slice_caller':['deque_physical(head,index,cap,zst)', 'crate::arithmetic::bounded::wrapping_sub(cap,physical)', 'crate::arithmetic::bounded::wrapping_sub(count,first)', 'offset_storage(s,physical)', 'distinct_nonoverlap_caller(source,s,0,physical,first)', 'offset_storage(source,first)', 'distinct_nonoverlap_caller(source,s,first,0,rest)'],
 'deque_append_caller':['deque_write_slice_caller(s,first,head,len,a,cap,zst)', 'crate::arithmetic::bounded::wrapping_add(len,a)', 'deque_write_slice_caller(s,second,head,middle,b,cap,zst)'],
 'deque_split_off_caller':['crate::arithmetic::bounded::wrapping_sub(len,at)', 'deque_physical(head,at,cap,zst)', 'crate::arithmetic::bounded::wrapping_sub(cap,pos)', 'crate::arithmetic::bounded::wrapping_sub(count,first)', 'offset_storage(s,pos)', 'distinct_nonoverlap_caller(s,result,pos,0,first)', 'offset_storage(result,first)', 'distinct_nonoverlap_caller(s,result,0,first,rest)'],
 'deque_search_caller':['crate::arithmetic::bounded::wrapping_add(left,crate::arithmetic::bounded::wrapping_sub(right,left)/2)', 'deque_get_caller(s,head,mid,len,cap,zst)', 'crate::arithmetic::bounded::wrapping_add(mid,1)'],
 'deque_retain_caller':['crate::compaction::Compaction::full(len)', 'state.peek()', 'state.keep_live()', 'state.remove_live()', 'deque_get_caller(s,head,read,len,cap,zst)', 'deque_get_caller(s,head,write,len,cap,zst)', 'overlapping_copy_caller(s,from,to,1)', 'deque_copy_ring_caller(s,from,to,state.remaining(),cap,false)', 'state.restored_len()'],
 'deque_drain_drop_caller':['crate::arithmetic::bounded::wrapping_sub(end,start)', 'deque_physical(head,0,cap,zst)', 'deque_physical(head,removed,cap,zst)', 'deque_copy_ring_caller(s,from,to,start,cap,true)', 'crate::deque::geometry::physical(head,removed,cap)', 'deque_physical(head,end,cap,zst)', 'deque_physical(head,start,cap,zst)', 'deque_copy_ring_caller(s,from,to,tail,cap,false)', 'crate::arithmetic::bounded::wrapping_add(start,tail)'],
 'deque_deallocate_caller':['layout_caller(s)'],
 'deque_raw_parts_caller':['representation_caller(s)'],
 'deque_clone_slice_caller':['offset_storage(s,index)', 'distinct_nonoverlap_caller(source,s,0,index,count)'],
 'deque_clone_slices_caller':['deque_clone_slice_caller(s,first,0,a,cap)', 'deque_clone_slice_caller(s,second,a,b,cap)'],
 'deque_extend_copy_caller':['deque_clone_slice_caller(s,source,0,count,cap)', 'deque_write_slice_caller(s,source,head,len,count,cap,zst)', 'crate::arithmetic::bounded::wrapping_add(len,count)'],
 'deque_resize_copy_caller':['crate::arithmetic::bounded::wrapping_sub(new_len,len)', 'deque_physical(head,len,cap,false)', 'crate::deque::geometry::split(tail,extra,cap)', 'crate::arithmetic::bounded::wrapping_add(tail,i)', 'offset_storage(s,at)', 'distinct_nonoverlap_caller(value,s,0,at,1)', 'offset_storage(s,i)', 'distinct_nonoverlap_caller(value,s,0,i,1)'],
}
# VecDeque::physical restated with `zst` for the size test.
RESTATED_PRODUCT='ifmem::size_of::<T>()==0{0}else{geometry::allocated_physical(self.head.get(),index,self.capacity())}'
RESTATED_MAINTAINED='ifzst{0}else{crate::deque::geometry::allocated_physical(head,index,cap)}}'
# Unsafe site (file name, function, context type or None for any) -> maintained
# caller. Sites not listed keep only their Kani harness.
SITES={
 ('deque.rs', 'slot', None):'deque_get_caller',
 ('deque.rs', 'get', None):'deque_get_caller',
 ('deque.rs', 'get_mut', None):'deque_get_caller',
 ('deque.rs', 'as_slices', None):'deque_slices_caller',
 ('deque.rs', 'as_mut_slices', None):'deque_slices_caller',
 ('deque.rs', 'drop', 'VecDeque'):'deque_slices_caller',
 ('deque.rs', 'drop', 'Tail'):'deque_slices_caller',
 ('deque.rs', 'front', None):'deque_front_caller',
 ('deque.rs', 'front_mut', None):'deque_front_caller',
 ('deque.rs', 'try_push_front_mut', None):'deque_front_caller',
 ('deque.rs', 'try_push_back', None):'deque_push_back_caller',
 ('deque.rs', 'try_push_back_mut', None):'deque_push_back_caller',
 ('deque.rs', 'try_push_front', None):'deque_push_front_caller',
 ('deque.rs', 'pop_front', None):'deque_pop_front_caller',
 ('deque.rs', 'pop_back', None):'deque_pop_back_caller',
 ('deque.rs', 'truncate', None):'deque_pop_back_caller',
 ('deque.rs', 'make_contiguous', None):'deque_contiguous_caller',
 ('deque.rs', 'normalize', None):'deque_normalize_caller',
 ('deque.rs', 'try_shrink_to', None):'deque_shrink_caller',
 ('deque.rs', 'grow', None):'deque_grow_caller',
 ('edit.rs', 'copy_ring', None):'deque_copy_ring_caller',
 ('edit.rs', 'swap', None):'deque_get_caller',
 ('edit.rs', 'try_insert_mut', None):'deque_insert_caller',
 ('edit.rs', 'remove', None):'deque_remove_caller',
 ('edit.rs', 'rotate_left', None):'deque_rotate_caller',
 ('edit.rs', 'rotate_right', None):'deque_rotate_caller',
 ('edit.rs', 'try_append', None):'deque_append_caller',
 ('edit.rs', 'write_slice', None):'deque_write_slice_caller',
 ('edit.rs', 'try_split_off', None):'deque_split_off_caller',
 ('edit.rs', 'try_retain_mut', None):'deque_retain_caller',
 ('edit.rs', 'drop', 'Repair'):'deque_retain_caller',
 ('edit.rs', 'binary_search_by', None):'deque_search_caller',
 ('edit.rs', 'partition_point', None):'deque_search_caller',
 ('traits.rs', 'clone_to', None):'deque_clone_slices_caller',
 ('traits.rs', 'try_clone_from', None):'deque_clone_slices_caller',
 ('traits.rs', 'write_clone_slice', None):'deque_clone_slice_caller',
 ('iter.rs', 'drop', 'Finish'):'deque_drain_drop_caller',
 ('iter.rs', 'fold', 'IntoIter'):'deque_slices_caller',
 ('deque.rs', 'drop', 'Allocation'):'deque_deallocate_caller',
 ('deque.rs', 'into_vec', None):'deque_raw_parts_caller',
 ('deque.rs', 'try_from_std_in', None):'deque_raw_parts_caller',
 ('traits.rs', 'try_extend_copy', None):'deque_extend_copy_caller',
 ('traits.rs', 'try_resize', None):'deque_resize_copy_caller',
 ('iter.rs', 'next', 'Drain'):'deque_get_caller',
 ('iter.rs', 'next_back', 'Drain'):'deque_get_caller',
 ('iter.rs', 'fmt', 'Drain'):'deque_get_caller',
}
