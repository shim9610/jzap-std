"""Whole-body text of the binary heap product functions followed by maintained callers.

Each maintained caller in tools/memory-proof/binary-heap.verus.rs was written
against exactly this text (comments and whitespace removed). A changed body
needs the caller to be read again; the comparison is not a solver proof.
Keys are (file under src/, unique anchor or '', function head after the anchor).
"""
BODIES={
 ('collections/binary_heap/core.rs', '', 'unsafe fn new('):
  'letvalue=ManuallyDrop::new(unsafe{ptr::read(data.as_ptr().add(pos))});Self{data,value,pos}',
 ('collections/binary_heap/core.rs', "impl<T> Drop for Hole<'_, T> {", 'fn drop('):
  'unsafe{ptr::copy_nonoverlapping(&*self.value,self.data.as_mut_ptr().add(self.pos),1)};',
 ('collections/binary_heap.rs', '', 'fn deref('):
  'unsafe{&*self.heap.data.as_ptr()}',
 ('collections/binary_heap.rs', '', 'fn deref_mut('):
  'ifself.original_len.is_none()&&self.heap.len()>1{self.original_len=NonZeroUsize::new(self.heap.len());unsafe{core::set_len(&mutself.heap.data,1)};}unsafe{&mut*self.heap.data.as_mut_ptr()}',
 ('collections/binary_heap.rs', "Drop for PeekMut<'_, T, P, L> {", 'fn drop('):
  'ifletSome(len)=self.original_len.take(){unsafe{core::set_len(&mutself.heap.data,len.get())};unsafe{core::sift_down(self.heap.data.as_mut_slice(),0,len.get())};}',
 ('collections/binary_heap/iter.rs', '', 'fn remaining('):
  'unsafe{::core::slice::from_raw_parts(self.heap.data.as_ptr().add(self.span.front()),self.span.len(),)}',
 ('collections/binary_heap/iter.rs', '', 'fn next('):
  'self.span.next().map(|index|{unsafe{self.heap.data.as_ptr().add(index).read()}})',
 ('collections/binary_heap/iter.rs', '', 'fn next_back('):
  'self.span.next_back().map(|index|{unsafe{self.heap.data.as_ptr().add(index).read()}})',
 ('collections/binary_heap/iter.rs', "Drop for Drain<'_, T, P, L> {", 'fn drop('):
  'letfront=self.span.front();letlen=self.span.len();self.span.clear();unsafe{::core::ptr::drop_in_place(::core::ptr::slice_from_raw_parts_mut(self.heap.data.as_mut_ptr().add(front),len,))};',
 ('collections/binary_heap/core.rs', '', 'fn first_child<'):
  'ifcore::mem::size_of::<T>()==0{ifpos>=end/2{end}else{children(pos,end).0}}else{child_offset(pos)}',
 ('collections/binary_heap/core.rs', '', 'unsafe fn sift_up<'):
  'letmuthole=unsafe{Hole::new(data,pos)};whilehole.pos>start{letp=parent(hole.pos);if&*hole.value<=unsafe{hole.get(p)}{break;}unsafe{hole.move_to(p)};}',
 ('collections/binary_heap/core.rs', '', 'unsafe fn sift_down<'):
  'letmuthole=unsafe{Hole::new(data,pos)};letlast=sub(end,1);letmutchild=first_child::<T>(pos,end);whilechild<last{letright=crate::arithmetic::bounded::wrapping_add(child,1);letchoose_right=unsafe{hole.get(child)<=hole.get(right)};child=selected(child,end,choose_right);if&*hole.value>=unsafe{hole.get(child)}{return;}unsafe{hole.move_to(child)};child=first_child::<T>(hole.pos,end);}ifchild==last{if&*hole.value<unsafe{hole.get(child)}{unsafe{hole.move_to(child)};}}',
 ('collections/binary_heap/core.rs', '', 'unsafe fn sift_bottom<'):
  'letstart=pos;letmuthole=unsafe{Hole::new(data,pos)};letlast=sub(end,1);letmutchild=first_child::<T>(pos,end);whilechild<last{letright=crate::arithmetic::bounded::wrapping_add(child,1);child=selected(child,end,unsafe{hole.get(child)<=hole.get(right)});unsafe{hole.move_to(child)};child=first_child::<T>(hole.pos,end);}ifchild==last{unsafe{hole.move_to(child)};}whilehole.pos>start{letp=parent(hole.pos);if&*hole.value<=unsafe{hole.get(p)}{break;}unsafe{hole.move_to(p)};}',
 ('collections/binary_heap/core.rs', '', 'fn rebuild<'):
  'letend=data.len();letmutpos=end/2;whilepos>0{pos=sub(pos,1);unsafe{sift_down(data,pos,end)};}',
 ('collections/binary_heap/core.rs', '', 'fn rebuild_tail<'):
  'letlen=data.len();ifstart>=len{return;}lettail=sub(len,start);letrebuild_all=ifstart<tail{true}else{letlevels=iflen<=2048{usize::BITS.saturating_sub(1).saturating_sub(start.leading_zeros())asusize}else{11};len.saturating_mul(2)<tail.saturating_mul(levels)};ifrebuild_all{rebuild(data);}else{letmutpos=start;whilepos<len{unsafe{sift_up(data,0,pos)};pos=crate::arithmetic::bounded::wrapping_add(pos,1);}}',
 ('collections/binary_heap/core.rs', '', 'unsafe fn pop_nonempty<'):
  'letlast=sub(data.len(),1);letvalue=unsafe{data.as_ptr().add(last).read()};unsafe{set_len(data,last)};iflast==0{returnvalue;}letresult=unsafe{data.as_mut_ptr().replace(value)};unsafe{sift_bottom(data.as_mut_slice(),0,last)};result',
 ('collections/binary_heap/core.rs', '', 'unsafe fn move_to('):
  'letdata=self.data.as_mut_ptr();unsafe{ptr::copy_nonoverlapping(data.add(index),data.add(self.pos),1)};self.pos=index;',
 ('collections/binary_heap/core.rs', '', 'unsafe fn get('):
  'unsafe{&*self.data.as_ptr().add(index)}',
 ('collections/binary_heap.rs', '', 'pub fn try_push('):
  'self.data.try_push(value)?;letpos=crate::arithmetic::bounded::wrapping_sub(self.len(),1);unsafe{core::sift_up(self.data.as_mut_slice(),0,pos)};Ok(())',
 ('collections/binary_heap.rs', '', 'pub fn pop(&mut self)'):
  'ifself.is_empty(){None}else{Some(unsafe{core::pop_nonempty(&mutself.data)})}',
 ('collections/binary_heap.rs', '', 'pub fn into_sorted_vec('):
  'letmutend=self.len();whileend>1{end=crate::arithmetic::bounded::wrapping_sub(end,1);letptr=self.data.as_mut_ptr();unsafe{::core::ptr::swap(ptr,ptr.add(end))};unsafe{core::sift_down(self.data.as_mut_slice(),0,end)};}self.data',
 # Issue #16, unit 7 (rule A): the product functions that enter the kernels with a Vec.
 ('collections/binary_heap.rs', '', 'pub fn from_vec('):
  'core::rebuild(data.as_mut_slice());Self{data}',
 ('collections/binary_heap.rs', '', 'pub fn try_append('):
  'letreused=self.data.try_reuse_larger_buffer(&mutother.data)?;letswapped=!reused&&self.len()<other.len()&&!self.allocation_policy().budget().is_metered()&&!other.allocation_policy().budget().is_metered()&&self.max_len()==other.max_len();ifswapped{::core::mem::swap(self,other);}letstart=self.len();ifletErr(error)=self.data.try_append(&mutother.data){ifswapped{::core::mem::swap(self,other);}returnErr(error);}core::rebuild_tail(self.data.as_mut_slice(),start);Ok(())',
 ('collections/binary_heap.rs', 'pub fn try_retain<E>(', 'fn drop('):
  'ifself.start<self.heap.len(){core::rebuild_tail(self.heap.data.as_mut_slice(),self.start);}',
 ('collections/binary_heap.rs', 'pub fn try_extend<I: TryIntoIterator<Item = T>>(', 'fn drop('):
  'core::rebuild_tail(self.heap.data.as_mut_slice(),self.start);',
}
# Maintained caller -> product calls its text must contain.
CALLERS={
 'heap_restore_caller':['offset_storage(s,pos)', 'distinct_nonoverlap_caller(value,s,0,pos,1)'],
 'heap_peek_caller':['heap_element_caller(s,0,len)'],
 'heap_peek_drop_caller':['heap_descend_caller(s,0,len,size==0,true,choices,stops)'],
 'span_remaining_caller':['span.len()', 'span.front()', 'offset_storage(a,front)', 'slice_caller(a,front,count)'],
 'span_next_caller':['span.next()', 'offset_storage(a,index)', 'slice_caller(a,index,1)'],
 'span_next_back_caller':['span.next_back()', 'offset_storage(a,index)', 'slice_caller(a,index,1)'],
 'span_clear_caller':['span.front()', 'span.len()', 'span.clear()', 'offset_storage(a,start)', 'slice_caller(a,start,count)'],
 'heap_first_child':['crate::collections::binary_heap::geometry::children(pos,end).0', 'crate::collections::binary_heap::geometry::child_offset(pos)'],
 'heap_element_caller':['offset_storage(s,index)', 'slice_caller(s,index,1)'],
 'heap_move_caller':['offset_storage(s,index)', 'offset_storage(s,pos)', 'unordered_nonoverlap_caller(s,index,pos,1)'],
 'heap_sift_up_caller':['crate::collections::binary_heap::geometry::parent(hole)', 'heap_element_caller(s,pos,end)', 'heap_element_caller(s,p,end)', 'heap_move_caller(s,p,hole,end)'],
 'heap_descend_caller':['crate::arithmetic::bounded::wrapping_sub(end,1)', 'heap_first_child(pos,end,zst)', 'crate::arithmetic::bounded::wrapping_add(child,1)', 'crate::collections::binary_heap::geometry::selected(child,end,pick(choices,child))', 'heap_first_child(hole,end,zst)', 'heap_element_caller(s,pos,end)', 'heap_element_caller(s,child,end)', 'heap_element_caller(s,right,end)', 'heap_move_caller(s,child,hole,end)'],
 'heap_sift_bottom_caller':['heap_descend_caller(s,pos,end,zst,false,choices,stops)', 'heap_sift_up_caller(s,pos,hole,end,stops)'],
 'heap_rebuild_caller':['crate::arithmetic::bounded::wrapping_sub(pos,1)', 'heap_descend_caller(s,pos,end,zst,true,choices,stops)'],
 'heap_rebuild_tail_caller':['crate::arithmetic::bounded::wrapping_sub(len,start)', 'heap_rebuild_caller(s,len,zst,choices,stops)', 'heap_sift_up_caller(s,0,pos,len,stops)', 'crate::arithmetic::bounded::wrapping_add(pos,1)'],
 'heap_pop_caller':['crate::arithmetic::bounded::wrapping_sub(len,1)', 'heap_sift_bottom_caller(s,0,last,zst,choices,stops)', 'heap_element_caller(s,last,len)', 'replace_caller(s,0)'],
 'heap_push_caller':['crate::arithmetic::bounded::wrapping_sub(len,1)', 'heap_sift_up_caller(s,0,pos,len,replies)'],
 'heap_sorted_caller':['crate::arithmetic::bounded::wrapping_sub(end,1)', 'heap_descend_caller(s,0,end,zst,true,choices,stops)', 'heap_element_caller(s,0,len)', 'heap_element_caller(s,end,len)'],
 # Issue #16, unit 7 (rule A): callers of the product functions that enter the kernels with a Vec.
 'heap_from_vec_caller':['heap_rebuild_caller(s,len,size==0,choices,stops)'],
 'heap_append_caller':['heap_rebuild_tail_caller(s,start,len,size==0,choices,stops)'],
 'heap_retain_repair_caller':['heap_rebuild_tail_caller(s,start,len,size==0,choices,stops)'],
 'heap_extend_repair_caller':['heap_rebuild_tail_caller(s,start,len,size==0,choices,stops)'],
}
# Product body restated with `zst` for `size_of::<T>() == 0`; the maintained
# suffix is fixed here and the product text is fixed in BODIES.
RESTATED_PRODUCT='ifcore::mem::size_of::<T>()==0{ifpos>=end/2{end}else{children(pos,end).0}}else{child_offset(pos)}'
RESTATED_MAINTAINED='ifzst{ifpos>=end/2{end}else{crate::collections::binary_heap::geometry::children(pos,end).0}}else{crate::collections::binary_heap::geometry::child_offset(pos)}}'
# Unsafe site (file name, function) -> maintained caller. Sites not listed keep
# only their Kani harness: the
# set_len wrapper and the Drain constructor.
SITES={
 ('core.rs', 'new'):'heap_element_caller',
 ('core.rs', 'drop'):'heap_restore_caller',
 ('binary_heap.rs', 'deref'):'heap_peek_caller',
 ('binary_heap.rs', 'deref_mut'):'heap_peek_caller',
 ('binary_heap.rs', 'drop'):'heap_peek_drop_caller',
 ('iter.rs', 'remaining'):'span_remaining_caller',
 ('iter.rs', 'next'):'span_next_caller',
 ('iter.rs', 'next_back'):'span_next_back_caller',
 ('iter.rs', 'drop'):'span_clear_caller',
 ('core.rs', 'get'):'heap_element_caller',
 ('core.rs', 'move_to'):'heap_move_caller',
 ('core.rs', 'sift_up'):'heap_sift_up_caller',
 ('core.rs', 'sift_down'):'heap_descend_caller',
 ('core.rs', 'sift_bottom'):'heap_sift_bottom_caller',
 ('core.rs', 'rebuild'):'heap_rebuild_caller',
 ('core.rs', 'rebuild_tail'):'heap_rebuild_tail_caller',
 ('core.rs', 'pop_nonempty'):'heap_pop_caller',
 ('binary_heap.rs', 'try_push'):'heap_push_caller',
 ('binary_heap.rs', 'pop'):'heap_pop_caller',
 ('binary_heap.rs', 'into_sorted_vec'):'heap_sorted_caller',
}
