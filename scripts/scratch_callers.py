"""Whole-body text of the scratch product functions followed by maintained callers.

Each maintained caller in tools/memory-proof/scratch.verus.rs was written
against exactly this text (comments and whitespace removed). A changed body
needs the caller to be read again; the comparison is not a solver proof.
Keys are (file under src/, unique anchor or '', function head after the anchor).
"""
BODIES={
 ('scratch.rs', '', 'pub fn as_slice('):
  'unsafe{slice::from_raw_parts(self.storage.as_ptr().cast(),self.len())}',
 ('scratch.rs', '', 'pub fn as_mut_slice('):
  'letlen=self.len();unsafe{slice::from_raw_parts_mut(self.storage.as_mut_ptr().cast(),len)}',
 ('scratch.rs', '', 'pub fn try_push('):
  'letend=matchgeometry::admitted_end(self.len(),1,self.capacity()){Some(n)=>n,None=>{returnErr(Rejected{value,error:Error::Capacity,});}};letindex=self.len();letSome(place)=self.storage.get_mut(index)else{returnErr(Rejected{value,error:Error::Capacity,});};self.len=Checked::new(end);Ok(place.write(value))',
 ('scratch.rs', '', 'pub fn pop('):
  'ifself.is_empty(){returnNone;}letindex=geometry::retired_last(self.len());self.len=Checked::new(index);Some(unsafe{self.storage.get_unchecked(index).assume_init_read()})',
 ('scratch.rs', '', 'pub fn truncate('):
  'letold=self.len();iflen>=old{return;}self.len=Checked::new(len);if!mem::needs_drop::<T>(){return;}unsafe{core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(self.storage.as_mut_ptr().cast::<T>().add(len),sub(old,len),));}',
 ('scratch.rs', '', 'unsafe fn copy_admitted('):
  'unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),self.storage.as_mut_ptr().cast::<T>().add(self.len()),source.len(),);}self.len=Checked::new(end);',
 ('scratch.rs', '', 'pub fn try_extend_copy('):
  'letend=geometry::admitted_end(self.len(),source.len(),self.capacity()).ok_or(Error::Capacity)?;unsafe{self.copy_admitted(source,end)};Ok(())',
 ('scratch.rs', '', 'pub fn try_extend_cloned('):
  'letstart=self.len();letend=geometry::admitted_end(start,source.len(),self.capacity()).ok_or(Error::Capacity)?;ifT::copy_witness().is_some(){unsafe{self.copy_admitted(source,end)};returnOk(());}self.try_resize_with_init(end,|index,slot|{source.get(sub(index,start)).ok_or(Error::Bounds)?.try_clone_into(slot)}).map_err(flatten)',
 ('scratch.rs', '', 'pub fn try_resize_with_init<'):
  'ifnew_len<=self.len(){self.truncate(new_len);returnOk(());}geometry::admitted_end(0,new_len,self.capacity()).ok_or(InitError::Storage(Error::Capacity))?;letoriginal=self.len();letguard=Rollback{owner:self,original,};whileguard.owner.len()<new_len{letindex=guard.owner.len();letnext=geometry::initialized_next(index,new_len);letcell=guard.owner.storage.get_mut(index).ok_or(InitError::Storage(Error::Bounds))?;letmutslot=unsafe{InitSlot::new(NonNull::from(cell).cast())};initialize(index,&mutslot).map_err(InitError::Initialize)?;slot.finish().map_err(InitError::Storage)?;guard.owner.len=Checked::new(next);}mem::forget(guard);Ok(())',
 ('scratch.rs', '', 'pub fn try_resize('):
  'ifnew_len<=self.len(){self.truncate(new_len);returnOk(());}geometry::admitted_end(0,new_len,self.capacity()).ok_or(Error::Capacity)?;ifmem::size_of::<T>()==0&&T::copy_witness().is_some(){self.len=Checked::new(new_len);returnOk(());}self.try_resize_with_init(new_len,|_,slot|value.try_clone_into(slot)).map_err(flatten)',
 ('scratch/owned.rs', '', 'unsafe fn copy_admitted('):
  'letstart=self.len();unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),self.values.as_mut_ptr().add(start),source.len(),);self.values.try_set_len(end)}',
 ('scratch/owned.rs', '', 'pub fn try_extend_cloned('):
  'letstart=self.len();letend=geometry::admitted_end(start,source.len(),self.capacity()).ok_or(Error::Capacity)?;ifT::copy_witness().is_some(){returnunsafe{self.copy_admitted(source,end)};}self.try_resize_with_init(end,|index,slot|{source.get(sub(index,start)).ok_or(Error::Bounds)?.try_clone_into(slot)}).map_err(flatten)',
 ('scratch/owned.rs', '', 'pub fn try_resize('):
  'geometry::admitted_end(0,new_len,self.capacity()).ok_or(Error::Capacity)?;ifmem::size_of::<T>()==0&&T::copy_witness().is_some(){returnunsafe{self.values.try_set_len(new_len)};}self.values.try_resize(new_len,value)',
 ('scratch/owned.rs', '', 'fn try_clone_from('):
  'geometry::admitted_end(0,source.len(),self.capacity()).ok_or(Error::Capacity)?;ifsource.is_empty(){self.clear();returnOk(());}ifT::copy_witness().is_some()&&!mem::needs_drop::<T>(){self.clear();returnunsafe{self.copy_admitted(source.as_slice(),source.len())};}letoriginal=self.len();self.try_extend_cloned(source.as_slice())?;drop(self.values.try_drain(..original)?);Ok(())',
}
# Maintained caller -> product calls its text must contain.
CALLERS={
 'scratch_prefix_caller':['slice_caller(s,0,len)'],
 'scratch_push_caller':['crate::slice::geometry::admitted_end(len,1,cap)', 'initialized_caller(s,len,cap)'],
 'scratch_pop_caller':['crate::slice::geometry::retired_last(len)', 'initialized_caller(s,index,cap)'],
 'scratch_truncate_caller':['crate::arithmetic::bounded::wrapping_sub(old,len)', 'offset_storage(s,len)', 'slice_caller(s,len,count)'],
 'scratch_copy_caller':['crate::slice::geometry::admitted_end(len,count,cap)', 'offset_storage(s,len)', 'distinct_nonoverlap_caller(source,s,0,len,count)'],
 'scratch_cloned_caller':['crate::slice::geometry::admitted_end(start,count,cap)', 'crate::arithmetic::bounded::wrapping_sub(index,start)'],
 'scratch_resize_init_caller':['crate::slice::geometry::admitted_end(0,new_len,cap)', 'crate::slice::geometry::initialized_next(index,new_len)', 'scratch_truncate_caller(s,new_len,len,cap)', 'initialized_caller(s,index,cap)'],
 'scratch_admitted_length_caller':['crate::slice::geometry::admitted_end(0,new_len,cap)'],
}
# Unsafe site (file name, function) -> maintained caller. Sites not listed keep
# only their Kani harness: the byte
# storage cast and state-field initialization.
SITES={
 ('scratch.rs', 'as_slice'):'scratch_prefix_caller',
 ('scratch.rs', 'as_mut_slice'):'scratch_prefix_caller',
 ('scratch.rs', 'try_push'):'scratch_push_caller',
 ('scratch.rs', 'pop'):'scratch_pop_caller',
 ('scratch.rs', 'truncate'):'scratch_truncate_caller',
 ('scratch.rs', 'copy_admitted'):'scratch_copy_caller',
 ('scratch.rs', 'try_extend_copy'):'scratch_copy_caller',
 ('scratch.rs', 'try_extend_cloned'):'scratch_cloned_caller',
 ('scratch.rs', 'try_resize_with_init'):'scratch_resize_init_caller',
 ('scratch.rs', 'try_resize'):'scratch_admitted_length_caller',
 ('owned.rs', 'copy_admitted'):'scratch_copy_caller',
 ('owned.rs', 'try_extend_cloned'):'scratch_cloned_caller',
 ('owned.rs', 'try_resize'):'scratch_admitted_length_caller',
 ('owned.rs', 'try_clone_from'):'scratch_admitted_length_caller',
}
