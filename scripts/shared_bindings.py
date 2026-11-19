"""Fail-closed phase and actual-argument bindings for shared ownership.

Solver contracts establish numeric/credit facts. These reviewed phase envelopes
bind private constructors, counter admission, payload publication and cleanup to
those contracts; typed ownership/provenance remain compiler/Miri boundaries.
A changed expression requires a reviewed re-projection, not a hash allowlist.
"""
import json,re,subprocess
from pathlib import Path
from scanner_tools import scanner
ROOT=Path(__file__).resolve().parents[1]
def compact(s):return re.sub(r',(?=\))','',re.sub(r'\s+','',s))
def is_shared(file):return file=='src/shared.rs' or file.startswith('src/shared/') or file.startswith('src/budget/')
def separate(row):return is_shared(row['file']) or (row['file']=='src/budget.rs' and (row['context'].startswith('Accounting') or row['function']=='budget' or (row['context']=='MemoryBudget' and row['function']=='limit')))
# Reviewed policy ownership migration: one ManuallyDrop header owner, borrow
# on retirement, take only on final release, and duplicate before commit.
PHASES = {('src/budget.rs', '&P', 'budget'): ['{P::budget(self)}'],
 # Pure ledger identity query for heap storage transfer: no charge or counter
 # transition. Actual heap kernels and compiled domain controls check its use.
 ('src/budget.rs', "Accounting<'a>", 'same_domain'): ['{match(self,other){(Self::Unmetered,Accounting::Unmetered)=>true,(Self::Local(a),Accounting::Local(b))=>core::ptr::eq(a,b),#[cfg(target_has_atomic="ptr")](Self::Concurrent(a),Accounting::Concurrent(b))=>core::ptr::eq(a,b),_=>false,}}'],
 ('src/budget.rs', "Accounting<'a>", 'is_metered'): ['{!matches!(self,Self::Unmetered)}'],
 ('src/budget.rs', "Accounting<'a>", 'release'): ['{matchself{Self::Unmetered=>{}Self::Local(b)=>b.release(bytes),#[cfg(target_has_atomic="ptr")]Self::Concurrent(b)=>b.release(bytes),}}'],
 ('src/budget.rs', "Accounting<'a>", 'reserve'): ['{matchself{Self::Unmetered=>Ok(None),Self::Local(b)=>b.reserve(bytes).map(|r|Some(AccountingReservation::Local(r))),#[cfg(target_has_atomic="ptr")]Self::Concurrent(b)=>b.reserve(bytes).map(|r|Some(AccountingReservation::Concurrent(r))),}}'],
 ('src/budget.rs', "AccountingCharge<'_>", 'retain'): ['{matchself{Self::Local(c)=>c.retain(),#[cfg(target_has_atomic="ptr")]Self::Concurrent(c)=>c.retain(),}}'],
 ('src/budget.rs', "AccountingReservation<'a>", 'commit'): ['{matchself{Self::Local(r)=>AccountingCharge::Local(r.commit()),#[cfg(target_has_atomic="ptr")]Self::Concurrent(r)=>AccountingCharge::Concurrent(r.commit()),}}'],
 ('src/budget.rs', "Budget<'_>", 'budget'): ['{match*self{Budget::Limited(b)=>Accounting::Local(b),Budget::Unmetered=>Accounting::Unmetered,}}'],
 ('src/budget.rs', "ConcurrentBudget<'_>", 'budget'): ['{Accounting::Concurrent(self.0)}'],
 ('src/budget.rs', 'MemoryBudget', 'limit'): ['{self.limit}'],
 ('src/budget.rs', 'Unmetered', 'budget'): ['{Accounting::Unmetered}'],
 ('src/budget/admission.rs', '', 'admit'): ['{letavailable=limit.checked_sub(used).ok_or(Error::Capacity)?;ifbytes>available{returnErr(Error::Budget{requested:bytes,available,});}used.checked_add(bytes).ok_or(Error::Capacity)}'],
 ('src/budget/concurrent.rs', "ConcurrentBudget<'_>", 'try_clone'): ['{Ok(*self)}'],
 ('src/budget/concurrent.rs', "ConcurrentCharge<'_>", 'bytes'): ['{self.bytes}'],
 ('src/budget/concurrent.rs', "ConcurrentCharge<'_>", 'drop'): ['{self.budget.release(self.bytes);}'],
 ('src/budget/concurrent.rs', "ConcurrentCharge<'_>", 'retain'): ['{core::mem::forget(self);}'],
 ('src/budget/concurrent.rs', 'ConcurrentMemoryBudget', 'available'): ['{self.limit.saturating_sub(self.used())}'],
 ('src/budget/concurrent.rs', 'ConcurrentMemoryBudget', 'limit'): ['{self.limit}'],
 ('src/budget/concurrent.rs', 'ConcurrentMemoryBudget', 'new'): ['{Self{limit,used:AtomicUsize::new(0),}}', '{Self{limit,used:AtomicUsize::new(0),}}'],
 ('src/budget/concurrent.rs', 'ConcurrentMemoryBudget', 'release'): ['{self.used.fetch_sub(bytes,Ordering::Relaxed);}'],
 ('src/budget/concurrent.rs', 'ConcurrentMemoryBudget', 'reserve'): ['{letmutused=self.used.load(Ordering::Relaxed);loop{letnext=admit(self.limit,used,bytes)?;matchself.used.compare_exchange(used,next,Ordering::Relaxed,Ordering::Relaxed){Ok(_)=>{returnOk(ConcurrentReservation{budget:self,bytes,});}Err(observed)=>used=observed,}}}'],
 ('src/budget/concurrent.rs', 'ConcurrentMemoryBudget', 'used'): ['{self.used.load(Ordering::Relaxed)}'],
 ('src/budget/concurrent.rs', "ConcurrentReservation<'_>", 'drop'): ['{self.budget.release(self.bytes);}'],
 ('src/budget/concurrent.rs', "ConcurrentReservation<'a>", 'bytes'): ['{self.bytes}'],
 ('src/budget/concurrent.rs', "ConcurrentReservation<'a>", 'commit'): ['{letcharge=ConcurrentCharge{budget:self.budget,bytes:self.bytes,};core::mem::forget(self);charge}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'available'): ['{self.ledger().available()}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'budget'): ['{Accounting::Local(self.ledger())}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'ledger'): ['{self.inner.as_ref()}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'limit'): ['{self.ledger().limit()}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'occupied'): ['{self.ledger().occupied()}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'reserve'): ['{self.ledger().reserve(bytes)}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'reserved'): ['{self.ledger().reserved()}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'try_clone'): ['{Ok(Self{inner:crate::rc::Rc::try_clone(&self.inner)?,})}'],
 ('src/budget/owned.rs', 'OwnedBudget', 'try_new'): ['{crate::rc::Rc::try_new(MemoryBudget::new(limit)).map(|inner|Self{inner}).map_err(|rejected|rejected.error)}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'available'): ['{self.ledger().available()}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'budget'): ['{Accounting::Concurrent(self.ledger())}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'ledger'): ['{self.inner.as_ref()}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'limit'): ['{self.ledger().limit()}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'reserve'): ['{self.ledger().reserve(bytes)}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'try_clone'): ['{Ok(Self{inner:crate::sync::Arc::try_clone(&self.inner)?,})}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'try_new'): ['{crate::sync::Arc::try_new(ConcurrentMemoryBudget::new(limit)).map(|inner|Self{inner}).map_err(|rejected|rejected.error)}'],
 ('src/budget/owned.rs', 'OwnedConcurrentBudget', 'used'): ['{self.ledger().used()}'],
 ('src/shared.rs', '', 'allocation_layout'): ['{let(layout,offset)=Layout::new::<Control<C,P,()>>().extend(value).map_err(|_|Error::Layout)?;Ok((layout.pad_to_align(),offset))}'],
 ('src/shared.rs', '', 'counts'): ['{unsafe{(&*core::ptr::addr_of!((*ptr.as_ptr()).strong),&*core::ptr::addr_of!((*ptr.as_ptr()).weak))}}'],
 ('src/shared.rs', '', 'data'): ['{unsafe{core::ptr::addr_of_mut!((*ptr.as_ptr()).data)}}'],
 ('src/shared.rs', '', 'drop_slow'): ['{unsafe{retire(ptr)};let_release=WeakRelease(ptr);unsafe{core::ptr::drop_in_place(data(ptr))};}'],
 ('src/shared.rs', '', 'retire'): ['{unsafe{letpolicy=&*core::ptr::addr_of!((*ptr.as_ptr()).policy).cast::<P>();ifpolicy.budget().is_metered(){let(strong,weak)=counts(ptr);strong.retire(weak,Layout::for_value(&*ptr.as_ptr()).size());}}}'],
 ('src/shared.rs', 'Allocation<C,P>', 'data'): ['{unsafe{self.ptr.as_ptr().add(self.offset)}}'],
 ('src/shared.rs', 'Allocation<C,P>', 'drop'): ['{unsafe{letheader=self.ptr.cast::<Control<C,P,()>>().as_ptr();letpolicy=ManuallyDrop::take(&mut(*header).policy);self.ptr.cast::<Control<C,P,()>>().as_ptr().drop_in_place();crate::storage::deallocate(self.ptr,self.layout,policy.budget());}}'],
 ('src/shared.rs', 'Allocation<C,P>', 'finish'): ['{core::mem::forget(self);Strong{ptr,marker:PhantomData,}}'],
 ('src/shared.rs', 'Allocation<C,P>', 'new'): ['{let(layout,offset)=allocation_layout::<C,P>(value)?;letptr=ifzeroed{crate::storage::allocate_zeroed(layout,policy.budget())?}else{crate::storage::allocate(layout,policy.budget())?};letretired=strong==0&&policy.budget().is_metered();unsafe{ptr.cast::<Control<C,P,()>>().as_ptr().write(Control{strong:C::new(ifretired{count::retired_state(layout.size())}else{strong}),weak:C::new(C::initial_weak(retired)),policy:ManuallyDrop::new(policy),data:(),});}Ok(Self{ptr,layout,offset,marker:PhantomData,})}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'allocation_policy'): ['{unsafe{&*core::ptr::addr_of!((*this.ptr.as_ptr()).policy).cast::<P>()}}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'as_ptr'): ['{unsafe{data(this.ptr)}}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'decrement_strong_count'): ['{drop(unsafe{Self::from_raw(ptr)});}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'drop'): ['{if!unsafe{counts(self.ptr).0.release()}{return;}unsafe{drop_slow(self.ptr)};}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'from_raw'): ['{Self{ptr,marker:PhantomData,}}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'get_mut'): ['{let(strong,weak)=unsafe{counts(this.ptr)};if!C::unique(strong,weak){returnNone;}Some(unsafe{&mut*data(this.ptr)})}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'into_inner'): ['{if!unsafe{counts(this.ptr).0.release()}{core::mem::forget(this);returnNone;}letthis=ManuallyDrop::new(this);unsafe{retire(this.ptr);}let_release=WeakRelease(this.ptr);Some(unsafe{data(this.ptr).read()})}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'into_raw'): ['{ManuallyDrop::new(this).ptr}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'ptr_eq'): ['{core::ptr::addr_eq(this.ptr.as_ptr(),other.ptr.as_ptr())}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'strong_count'): ['{let(strong,weak)=unsafe{counts(this.ptr)};strong.live_count(weak)}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_clone'): ['{unsafe{counts(this.ptr).0.clone_strong()?;}Ok(Self{ptr:this.ptr,marker:PhantomData,})}', '{Self::try_clone(self)}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_clone_in'): ['{Self::try_clone(self)}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_downgrade'): ['{unsafe{counts(this.ptr).1.clone_weak()?;}Ok(Weak{ptr:Some(this.ptr),marker:PhantomData,})}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_increment_strong_count'): ['{let(strong,weak)=unsafe{counts(ptr)};ifstrong.upgrade(weak)?{Ok(())}else{Err(Error::InvalidStructure)}}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_init_cyclic_in'): ['{leta=Allocation::<C,P>::new(Layout::new::<T>(),policy,0,false).map_err(InitError::Storage)?;letptr=a.ptr.cast::<Control<C,P,T>>();core::mem::forget(a);letweak=Weak{ptr:Some(ptr),marker:PhantomData,};letmutslot=unsafe{crate::init::InitSlot::new(NonNull::new_unchecked(data(ptr)))};initialize(&weak,&mutslot).map_err(InitError::Initialize)?;slot.finish().map_err(InitError::Storage)?;unsafe{let(strong,weak)=counts(ptr);strong.publish(weak);}core::mem::forget(weak);Ok(Self{ptr,marker:PhantomData,})}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_init_in'): ['{letallocation=Allocation::<C,P>::new(Layout::new::<T>(),policy,1,false).map_err(InitError::Storage)?;letptr=allocation.ptr.cast::<Control<C,P,T>>();letmutslot=unsafe{crate::init::InitSlot::new(NonNull::new_unchecked(allocation.data().cast::<T>()))};initialize.try_init(&mutslot).map_err(InitError::Initialize)?;slot.finish().map_err(InitError::Storage)?;Ok(allocation.finish(ptr))}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_new_cyclic_in'): ['{leta=Allocation::<C,P>::new(Layout::new::<T>(),policy,0,false).map_err(InitError::Storage)?;letptr=a.ptr.cast::<Control<C,P,T>>();core::mem::forget(a);letweak=Weak{ptr:Some(ptr),marker:PhantomData,};letvalue=matchinitialize(&weak){Ok(value)=>value,Err(error)=>returnErr(InitError::Initialize(error)),};unsafe{data(ptr).write(value);let(strong,weak)=counts(ptr);strong.publish(weak);}core::mem::forget(weak);Ok(Self{ptr,marker:PhantomData,})}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_new_in'): ['{letallocation=matchAllocation::<C,P>::new(Layout::new::<T>(),policy,1,false){Ok(a)=>a,Err(error)=>returnErr(Rejected{value,error}),};letptr=allocation.ptr.cast::<Control<C,P,T>>();unsafe{allocation.data().cast::<T>().write(value);}Ok(allocation.finish(ptr))}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_new_uninit_in'): ['{Strong::try_new_in(MaybeUninit::uninit(),policy).map_err(|e|e.error)}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_new_with_in'): ["{Self::try_init_in(policy,|slot:&mutcrate::init::InitSlot<'_,T>|{slot.write(initialize().map_err(InitError::Initialize)?).map_err(|e|InitError::Storage(e.error))}).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(InitError::Storage(e))=>{InitError::Storage(e)}InitError::Initialize(InitError::Initialize(e))=>InitError::Initialize(e),})}"],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_new_zeroed_in'): ['{leta=Allocation::<C,P>::new(Layout::new::<T>(),policy,1,true)?;letp=a.ptr.cast::<Control<C,P,MaybeUninit<T>>>();Ok(a.finish(p))}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_unwrap'): ['{if!unsafe{counts(this.ptr).0.take_last()}{returnErr(this);}letthis=ManuallyDrop::new(this);unsafe{retire(this.ptr);}let_release=WeakRelease(this.ptr);Ok(unsafe{data(this.ptr).read()})}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'try_unwrap_or_clone'): ['{matchSelf::try_unwrap(this){Ok(v)=>Ok(v),Err(value)=>matchvalue.as_ref().try_clone(){Ok(v)=>Ok(v),Err(error)=>Err(Rejected{value,error}),},}}'],
 ('src/shared.rs', 'Strong<T,C,P>', 'weak_count'): ['{unsafe{counts(this.ptr).1.external_weaks()}}'],
 ('src/shared.rs', 'Strong<T,C>', 'try_new'): ['{Self::try_new_in(value,Unmetered)}'],
 ('src/shared.rs', 'Strong<T,C>', 'try_new_cyclic'): ['{Self::try_new_cyclic_in(Unmetered,initialize)}'],
 ('src/shared.rs', 'Strong<T,C>', 'try_new_uninit'): ['{Self::try_new_uninit_in(Unmetered)}'],
 ('src/shared.rs', 'Strong<T,C>', 'try_new_zeroed'): ['{Self::try_new_zeroed_in(Unmetered)}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'as_ptr'): ['{self.ptr.map(|p|unsafe{data(p)as*constT})}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'drop'): ['{ifletSome(ptr)=self.ptr{drop(WeakRelease(ptr));}}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'from_raw'): ['{Self{ptr,marker:PhantomData,}}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'into_raw'): ['{ManuallyDrop::new(self).ptr}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'new'): ['{Self{ptr:None,marker:PhantomData,}}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'ptr_eq'): ['{match(self.ptr,other.ptr){(None,None)=>true,(Some(a),Some(b))=>core::ptr::addr_eq(a.as_ptr(),b.as_ptr()),_=>false,}}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'strong_count'): ['{self.ptr.map_or(0,|p|{let(strong,weak)=unsafe{counts(p)};strong.live_count(weak)})}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'try_clone'): ['{ifletSome(ptr)=self.ptr{unsafe{counts(ptr).1.clone_weak()?;}}Ok(Self{ptr:self.ptr,marker:PhantomData,})}', '{Self::try_clone(self)}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'try_clone_in'): ['{Self::try_clone(self)}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'try_upgrade'): ['{letSome(ptr)=self.ptrelse{returnOk(None);};let(strong,weak)=unsafe{counts(ptr)};if!strong.upgrade(weak)?{returnOk(None);}Ok(Some(Strong{ptr,marker:PhantomData,}))}'],
 ('src/shared.rs', 'Weak<T,C,P>', 'weak_count'): ['{letSome(ptr)=self.ptrelse{return0;};let(s,w)=unsafe{counts(ptr)};ifs.live_count(w)==0{return0;}w.external_weaks()}'],
 ('src/shared.rs', 'WeakRelease<T,C,P>', 'drop'): ['{unsafe{if!counts(self.0).1.release_weak(){return;}letpolicy=ManuallyDrop::take(&mut(*self.0.as_ptr()).policy);letbytes=ifpolicy.budget().is_metered(){count::retired_bytes(counts(self.0).0.load())}else{0};drop(alloc::boxed::Box::from_raw(self.0.as_ptr()as*mutControl<C,P,ManuallyDrop<T>>));policy.budget().release(bytes);}}'],
 ('src/shared/convert.rs', '', 'control_from_value'): ['{letoriginal=(core::ptr::from_ref(source)as*mutControl<C,P,T>).with_addr(base.as_ptr().addr()^1);letmutresult=original;unsafe{core::ptr::addr_of_mut!(result).cast::<*mutu8>().write(base.as_ptr());}ifresult.addr()!=base.as_ptr().addr()||!core::ptr::eq(result.with_addr(original.addr()),original){returnErr(Error::Layout);}NonNull::new(result).ok_or(Error::Layout)}'],
 ('src/shared/convert.rs', 'Pinned<T,C>', 'try_default'): ['{Strong::try_pin_init_in(Unmetered,T::try_default_into).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}'],
 ('src/shared/convert.rs', 'Strong<T,C,P>', 'source'): ['{self.as_ref().source()}'],
 ('src/shared/convert.rs', 'Strong<T,C,P>', 'try_from_box_in'): ['{letpolicy=matchcrate::boxed::Box::policy(&value).try_clone(){Ok(policy)=>policy,Err(error)=>{returnErr(Rejected{value,error});}};Self::try_from_box_with(value,policy)}'],
 ('src/shared/convert.rs', 'Strong<T,C,P>', 'try_from_box_with'): ['{letlayout=Layout::for_value(value.as_ref());leta=matchAllocation::<C,P>::new(layout,policy,1,false){Ok(a)=>a,Err(error)=>returnErr(Rejected{value,error}),};letptr=matchunsafe{control_from_value::<T,C,P>(value.as_ref(),a.ptr)}{Ok(p)=>p,Err(error)=>returnErr(Rejected{value,error}),};let(source,source_policy)=crate::boxed::Box::into_raw_parts(value);unsafe{core::ptr::copy_nonoverlapping(source.as_ptr().cast::<u8>(),a.data(),layout.size());crate::storage::deallocate(source.cast(),layout,source_policy.budget());}Ok(a.finish(ptr))}'],
 ('src/shared/convert.rs', 'Strong<T,C,P>', 'try_from_cow_in'): ['{matchSelf::try_from_ref_in(value.as_ref(),policy){Ok(a)=>Ok(a),Err(error)=>Err(Rejected{value,error}),}}'],
 ('src/shared/convert.rs', 'Strong<T,C,P>', 'try_from_ref_in'): ['{if!source.compatible(){returnErr(Error::Layout);}leta=Allocation::<C,P>::new(Layout::for_value(source),policy,1,false)?;letptr=unsafe{source.control::<C,P>(a.ptr)};unsafe{source.clone_into(data(ptr))?;}Ok(a.finish(ptr))}'],
 ('src/shared/convert.rs', 'Strong<T,C,P>', 'try_from_std_box_in'): ['{Self::try_from_box_with(crate::boxed::Box::from_std(value),policy).map_err(|e|Rejected{value:crate::boxed::Box::into_std(e.value),error:e.error,})}'],
 ('src/shared/convert.rs', 'Strong<T,C>', 'try_from_cow'): ['{Self::try_from_cow_in(value,Unmetered)}'],
 ('src/shared/convert.rs', 'Strong<T,C>', 'try_from_ref'): ['{Self::try_from_ref_in(source,Unmetered)}'],
 ('src/shared/convert.rs', 'Strong<T,C>', 'try_from_std_box'): ['{Self::try_from_std_box_in(value,Unmetered)}'],
 ('src/shared/convert.rs', 'Strong<[T;N],C,P>', 'into_slice'): ['{letraw=Self::into_raw(this);letpointer:*mutControl<C,P,[T]>=raw.as_ptr();unsafe{Strong::from_raw(NonNull::new_unchecked(pointer))}}'],
 ('src/shared/convert.rs', 'Strong<[T],C,P>', 'try_from_array_in'): ['{Strong::<[T;N],C,P>::try_new_in(value,policy).map(Strong::into_slice)}'],
 ('src/shared/convert.rs', 'Strong<[T],C,P>', 'try_from_std_vec_in'): ['{leta=matchAllocation::<C,P>::new(Layout::for_value(value.as_slice()),policy,1,false){Ok(a)=>a,Err(error)=>returnErr(Rejected{value,error}),};letptr=unsafe{value.as_slice().control::<C,P>(a.ptr)};unsafe{core::ptr::copy_nonoverlapping(value.as_ptr(),a.data().cast::<T>(),value.len());value.set_len(0);}Ok(a.finish(ptr))}'],
 ('src/shared/convert.rs', 'Strong<[T],C,P>', 'try_into_array'): ['{ifthis.len()!=N{returnErr(this);}letptr=Self::into_raw(this).cast::<Control<C,P,[T;N]>>();Ok(unsafe{Strong::from_raw(ptr)})}'],
 ('src/shared/convert.rs', 'Strong<[T],C>', 'try_default'): ['{letempty=Self::try_new_uninit_slice(0)?;Ok(unsafe{Strong::<[MaybeUninit<T>],C>::assume_init(empty)})}'],
 ('src/shared/convert.rs', 'Strong<[T],C>', 'try_from_array'): ['{Self::try_from_array_in(value,Unmetered)}'],
 ('src/shared/convert.rs', 'Strong<[T],C>', 'try_from_std_vec'): ['{Self::try_from_std_vec_in(value,Unmetered)}'],
 ('src/shared/convert.rs', 'Strong<str,C,P>', 'into_bytes'): ['{letraw=Self::into_raw(this);unsafe{Strong::from_raw(NonNull::new_unchecked(raw.as_ptr()as*mutControl<C,P,[u8]>))}}'],
 ('src/shared/convert.rs', 'Strong<str,C,P>', 'try_from_std_string_in'): ['{matchSelf::try_from_str_in(value.as_str(),policy){Ok(a)=>Ok(a),Err(error)=>Err(Rejected{value,error}),}}'],
 ('src/shared/convert.rs', 'Strong<str,C,P>', 'try_from_string_in'): ['{letpolicy=matchvalue.allocation_policy().try_clone(){Ok(policy)=>policy,Err(error)=>{returnErr(Rejected{value,error});}};matchSelf::try_from_str_in(value.as_str(),policy){Ok(a)=>Ok(a),Err(error)=>Err(Rejected{value,error}),}}'],
 ('src/shared/convert.rs', 'Strong<str,C>', 'try_default'): ['{Self::try_from_str("")}'],
 ('src/shared/convert.rs', 'Strong<str,C>', 'try_from_std_string'): ['{Self::try_from_std_string_in(value,Unmetered)}'],
 ('src/shared/count.rs', 'AtomicCount', 'increment'): ['{letmutold=self.0.load(Ordering::Relaxed);loop{iflive&&old==0{returnOk(false);}ifold==LOCK{#[cfg(not(jzap_loom))]core::hint::spin_loop();#[cfg(jzap_loom)]loom::thread::yield_now();old=self.0.load(Ordering::Relaxed);continue;}letnext=admitted_count(old)?;matchself.0.compare_exchange(old,next,Ordering::Acquire,Ordering::Relaxed){Ok(_)=>returnOk(true),Err(observed)=>old=observed,}}}'],
 ('src/shared/count.rs', 'AtomicCount', 'load'): ['{self.0.load(Ordering::Relaxed)}'],
 ('src/shared/count.rs', 'AtomicCount', 'new'): ['{Self(AtomicUsize::new(value))}'],
 ('src/shared/count.rs', 'AtomicCount', 'publish'): ['{ifweak.0.load(Ordering::Relaxed)&RETIRED!=0{self.0.store(0,Ordering::Relaxed);weak.0.fetch_and(!RETIRED,Ordering::Release);}self.0.store(1,Ordering::Release);}'],
 ('src/shared/count.rs', 'AtomicCount', 'release'): ['{ifself.0.fetch_sub(1,Ordering::Release)!=1{returnfalse;}fence(Ordering::Acquire);true}'],
 ('src/shared/count.rs', 'AtomicCount', 'retire'): ['{ifweak.0.load(Ordering::Acquire)==1{weak.0.store(RETIRED|1,Ordering::Release);}else{weak.0.fetch_or(RETIRED,Ordering::Release);}self.0.store(retired_state(bytes),Ordering::Release);}'],
 ('src/shared/count.rs', 'AtomicCount', 'take_last'): ['{self.0.compare_exchange(1,0,Ordering::Acquire,Ordering::Relaxed).is_ok()}'],
 ('src/shared/count.rs', 'AtomicCount', 'unique'): ['{ifweak.0.compare_exchange(1,LOCK,Ordering::Acquire,Ordering::Relaxed).is_err(){returnfalse;}letunique=strong.0.load(Ordering::Acquire)==1;weak.0.store(1,Ordering::Release);unique}'],
 ('src/shared/count.rs', 'LocalCount', 'increment'): ['{letold=self.0.get();iflive&&(old==0||old>MAX_COUNT){returnOk(false);}self.0.set(admitted_count(old)?);Ok(true)}'],
 ('src/shared/count.rs', 'LocalCount', 'load'): ['{self.0.get()}'],
 ('src/shared/count.rs', 'LocalCount', 'new'): ['{Self(Cell::new(value))}'],
 ('src/shared/count.rs', 'LocalCount', 'publish'): ['{self.0.set(1);}'],
 ('src/shared/count.rs', 'LocalCount', 'release'): ['{letold=self.0.get();self.0.set(crate::arithmetic::bounded::wrapping_sub(old,1));old==1}'],
 ('src/shared/count.rs', 'LocalCount', 'retire'): ['{self.0.set(retired_state(bytes));}'],
 ('src/shared/count.rs', 'LocalCount', 'take_last'): ['{ifself.0.get()!=1{returnfalse;}self.0.set(0);true}'],
 ('src/shared/count.rs', 'LocalCount', 'unique'): ['{strong.load()==1&&weak.load()==1}'],
 ('src/shared/pinned.rs', 'Pinned<T,C,P>', 'as_ref'): ['{unsafe{Pin::new_unchecked(self.owner.as_ref())}}'],
 ('src/shared/pinned.rs', 'Pinned<T,C,P>', 'into_inner'): ['{self.owner}'],
 ('src/shared/pinned.rs', 'Pinned<T,C,P>', 'into_inner_unchecked'): ['{self.owner}'],
 ('src/shared/pinned.rs', 'Pinned<T,C,P>', 'try_clone'): ['{Ok(Self{owner:Strong::try_clone(&self.owner)?,})}'],
 ('src/shared/pinned.rs', 'Strong<T,C,P>', 'into_pin'): ['{Pinned{owner:this}}'],
 ('src/shared/pinned.rs', 'Strong<T,C,P>', 'into_pin_unchecked'): ['{Pinned{owner:this}}'],
 ('src/shared/pinned.rs', 'Strong<T,C,P>', 'try_into_pin'): ['{let(strong,weak)=unsafe{counts(this.ptr)};if!C::unique(strong,weak){returnErr(this);}Ok(Pinned{owner:this})}'],
 ('src/shared/pinned.rs', 'Strong<T,C,P>', 'try_pin_in'): ['{Self::try_new_in(value,policy).map(|owner|Pinned{owner})}'],
 ('src/shared/pinned.rs', 'Strong<T,C,P>', 'try_pin_init_in'): ['{Self::try_init_in(policy,initialize).map(|owner|Pinned{owner})}'],
 ('src/shared/pinned.rs', 'Strong<T,C>', 'try_pin'): ['{Self::try_pin_in(value,Unmetered)}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'as_ref'): ['{unsafe{&*data(self.ptr)}}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'borrow'): ['{self.as_ref()}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'cmp'): ['{self.as_ref().cmp(other.as_ref())}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'deref'): ['{self.as_ref()}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'eq'): ['{self.as_ref().eq(other.as_ref())}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'fmt'): ['{self.as_ref().fmt(f)}', '{self.as_ref().fmt(f)}', '{Self::as_ptr(self).fmt(f)}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'hash'): ['{self.as_ref().hash(h)}'],
 ('src/shared/traits.rs', 'Strong<T,C,P>', 'partial_cmp'): ['{self.as_ref().partial_cmp(other.as_ref())}'],
 ('src/shared/traits.rs', 'Strong<T,C>', 'try_default'): ['{Self::try_init_in(Unmetered,T::try_default_into).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}'],
 ('src/shared/traits.rs', 'Weak<T,C,P>', 'default'): ['{Self::new()}'],
 ('src/shared/traits.rs', 'Weak<T,C,P>', 'fmt'): ['{f.write_str("(Weak)")}'],
 ('src/shared/traits.rs', 'Weak<T,C,P>', 'try_default'): ['{Ok(Self::new())}'],
 ('src/shared/value.rs', '', 'os_bytes'): ['{s.as_encoded_bytes()}'],
 ('src/shared/value.rs', '', 'path_bytes'): ['{s.as_os_str().as_encoded_bytes()}'],
 ('src/shared/value.rs', 'Strong<MaybeUninit<T>,C,P>', 'assume_init'): ['{letptr=Self::into_raw(this).cast::<Control<C,P,T>>();unsafe{Strong::from_raw(ptr)}}'],
 ('src/shared/value.rs', 'Strong<T,C,P>', 'try_clone_payload_with'): ['{if!this.as_ref().compatible(){returnErr(Error::Layout);}leta=Allocation::<C,Q>::new(Layout::for_value(this.as_ref()),policy,1,false)?;letptr=unsafe{this.as_ref().control::<C,Q>(a.ptr)};unsafe{this.as_ref().clone_into(data(ptr))?;}Ok(a.finish(ptr))}'],
 ('src/shared/value.rs', 'Strong<T,C,P>', 'try_deep_clone_in'): ["{Strong::try_init_in(budget,|slot:&mutcrate::init::InitSlot<'_,T::Output>|{this.as_ref().try_clone_into_in(budget,slot)}).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}"],
 ('src/shared/value.rs', 'Strong<T,C,P>', 'try_make_mut'): ['{let(strong,weak)=unsafe{counts(this.ptr)};ifstrong.load()==1&&weak.load()==1&&C::unique(strong,weak){returnOk(unsafe{&mut*data(this.ptr)});}if!this.as_ref().compatible(){returnErr(Error::Layout);}letold=this.ptr;letlayout=Layout::for_value(this.as_ref());leta=Allocation::<C,P>::new(layout,Self::allocation_policy(this).try_clone()?,1,false)?;letptr=unsafe{this.as_ref().control::<C,P>(a.ptr)};ifunsafe{counts(old).0.take_last()}{unsafe{retire(old);}unsafe{core::ptr::copy_nonoverlapping(data(old).cast::<u8>(),a.data(),layout.size());}this.ptr=ptr;core::mem::forget(a);drop(WeakRelease(old));}else{unsafe{this.as_ref().clone_into(data(ptr))?;}*this=a.finish(ptr);}Ok(unsafe{&mut*data(this.ptr)})}'],
 ('src/shared/value.rs', 'Strong<[MaybeUninit<T>],C,P>', 'assume_init'): ['{letraw=Self::into_raw(this);letptr=unsafe{NonNull::new_unchecked(raw.as_ptr()as*mutControl<C,P,[T]>)};unsafe{Strong::from_raw(ptr)}}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'as_slice'): ['{self.as_ref()}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'get'): ['{self.as_ref().get(index)}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'is_empty'): ['{self.as_ref().is_empty()}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'len'): ['{self.as_ref().len()}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_copy_from_slice_in'): ['{leta=Allocation::<C,P>::new(Layout::for_value(source),policy,1,false)?;letptr=unsafe{source.control::<C,P>(a.ptr)};unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),a.data().cast::<T>(),source.len());}Ok(a.finish(ptr))}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_deep_clone_in'): ['{Strong::try_init_slice_in(this.len(),budget,|i,slot|{this.as_slice().get(i).ok_or(Error::Bounds)?.try_clone_into_in(budget,slot)}).map_err(|e|matche{InitError::Storage(e)|InitError::Initialize(e)=>e,})}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_from_fallible_iter_in'): ['{letvalues=crate::vec::Vec::try_from_fallible_iter_in(source,policy,crate::vec::Unlimited)?;Self::try_from_vec_in(values).map_err(|e|InitError::Storage(e.error))}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_from_iter_in'): ['{Self::try_from_fallible_iter_in(crate::iter::from_std(source),policy).map_err(|e|matche{InitError::Storage(e)=>e,InitError::Initialize(n)=>matchn{},})}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_from_slice_in'): ['{leta=Allocation::<C,P>::new(Layout::for_value(source),policy,1,false)?;letptr=unsafe{source.control::<C,P>(a.ptr)};unsafe{source.clone_into(data(ptr))?;}Ok(a.finish(ptr))}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_from_vec_in'): ['{letpolicy=matchsource.allocation_policy().try_clone(){Ok(policy)=>policy,Err(error)=>{returnErr(Rejected{value:source,error,});}};leta=matchAllocation::<C,P>::new(Layout::for_value(source.as_slice()),policy,1,false){Ok(a)=>a,Err(error)=>{returnErr(Rejected{value:source,error,});}};letptr=unsafe{source.as_slice().control::<C,P>(a.ptr)};unsafe{core::ptr::copy_nonoverlapping(source.as_ptr(),a.data().cast::<T>(),source.len());source.try_set_len(0).map_err(|error|Rejected{value:source,error,})?;}Ok(a.finish(ptr))}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_init_slice_in'): ['{letowner=Self::try_new_uninit_slice_in(len,policy).map_err(InitError::Storage)?;letptr=unsafe{NonNull::new_unchecked(data(owner.ptr).cast::<T>())};unsafe{crate::init::prefix::initialize(ptr,len,&mutinitialize)}?;Ok(unsafe{Strong::<[MaybeUninit<T>],C,P>::assume_init(owner)})}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_new_uninit_slice_in'): ['{Self::uninit_slice(len,policy,false)}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'try_new_zeroed_slice_in'): ['{Self::uninit_slice(len,policy,true)}'],
 ('src/shared/value.rs', 'Strong<[T],C,P>', 'uninit_slice'): ['{letlayout=crate::layout::array::<T>(len)?;leta=Allocation::<C,P>::new(layout,policy,1,zeroed)?;letraw=NonNull::slice_from_raw_parts(a.ptr.cast::<MaybeUninit<T>>(),len);letptr=unsafe{NonNull::new_unchecked(raw.as_ptr()as*mutControl<C,P,[MaybeUninit<T>]>)};Ok(a.finish(ptr))}'],
 ('src/shared/value.rs', 'Strong<[T],C>', 'try_copy_from_slice'): ['{Self::try_copy_from_slice_in(source,Unmetered)}'],
 ('src/shared/value.rs', 'Strong<[T],C>', 'try_from_iter'): ['{Self::try_from_iter_in(source,Unmetered)}'],
 ('src/shared/value.rs', 'Strong<[T],C>', 'try_from_slice'): ['{Self::try_from_slice_in(source,Unmetered)}'],
 ('src/shared/value.rs', 'Strong<[T],C>', 'try_new_uninit_slice'): ['{Self::try_new_uninit_slice_in(len,Unmetered)}'],
 ('src/shared/value.rs', 'Strong<[T],C>', 'try_new_zeroed_slice'): ['{Self::try_new_zeroed_slice_in(len,Unmetered)}'],
 ('src/shared/value.rs', 'Strong<core::ffi::CStr,C>', 'try_default'): ['{Self::try_from_ref(c"")}'],
 ('src/shared/value.rs', 'Strong<std::ffi::OsStr,C>', 'try_default'): ['{Self::try_from_ref(std::ffi::OsStr::new(""))}'],
 ('src/shared/value.rs', 'Strong<std::path::Path,C>', 'try_default'): ['{Self::try_from_ref(std::path::Path::new(""))}'],
 ('src/shared/value.rs', 'Strong<str,C,P>', 'as_bytes'): ['{self.as_ref().as_bytes()}'],
 ('src/shared/value.rs', 'Strong<str,C,P>', 'as_str'): ['{self.as_ref()}'],
 ('src/shared/value.rs', 'Strong<str,C,P>', 'is_empty'): ['{self.as_ref().is_empty()}'],
 ('src/shared/value.rs', 'Strong<str,C,P>', 'len'): ['{self.as_ref().len()}'],
 ('src/shared/value.rs', 'Strong<str,C,P>', 'try_deep_clone_in'): ['{Strong::try_from_str_in(this.as_str(),budget)}'],
 ('src/shared/value.rs', 'Strong<str,C,P>', 'try_from_str_in'): ['{leta=Allocation::<C,P>::new(Layout::for_value(source),policy,1,false)?;letptr=unsafe{source.control::<C,P>(a.ptr)};unsafe{source.clone_into(data(ptr))?;}Ok(a.finish(ptr))}'],
 ('src/shared/value.rs', 'Strong<str,C>', 'try_from_str'): ['{Self::try_from_str_in(source,Unmetered)}'],
 ('src/shared/value.rs', 'T', 'clone_into'): ['{letmutslot=unsafe{crate::init::InitSlot::new(NonNull::new_unchecked(destination))};self.try_clone_into(&mutslot)?;slot.finish()}'],
 ('src/shared/value.rs', 'T', 'control'): ['{base.cast()}'],
 ('src/shared/value.rs', '[T]', 'clone_into'): ["{letptr=unsafe{NonNull::new_unchecked(destination.cast::<T>())};letmutinitialize=|index:usize,slot:&mutcrate::init::InitSlot<'_,T>|{unsafe{self.get_unchecked(index)}.try_clone_into(slot)};unsafe{crate::init::prefix::initialize(ptr,self.len(),&mutinitialize)}.map_err(|e|{matche{InitError::Storage(e)|InitError::Initialize(e)=>e,}})}"],
 ('src/shared/value.rs', '[T]', 'control'): ['{letp=NonNull::slice_from_raw_parts(base.cast::<T>(),self.len());unsafe{NonNull::new_unchecked(p.as_ptr()as*mutControl<C,P,[T]>)}}'],
 ('src/shared/value.rs', 'core::ffi::CStr', 'clone_into'): ['{unsafe{core::ptr::copy_nonoverlapping((selfas*constSelf).cast::<u8>(),dst.cast::<u8>(),Layout::for_value(self).size());}Ok(())}'],
 ('src/shared/value.rs', 'core::ffi::CStr', 'compatible'): ['{letbytes=self.to_bytes_with_nul();Layout::for_value(self)==Layout::for_value(bytes)&&(selfas*constSelfas*const[u8]).len()==bytes.len()&&(selfas*constSelf).cast::<u8>()==bytes.as_ptr()}'],
 ('src/shared/value.rs', 'core::ffi::CStr', 'control'): ['{letp=NonNull::slice_from_raw_parts(base,self.to_bytes_with_nul().len());unsafe{NonNull::new_unchecked(p.as_ptr()as*mutControl<C,P,Self>)}}'],
 ('src/shared/value.rs', 'str', 'clone_into'): ['{unsafe{core::ptr::copy_nonoverlapping(self.as_ptr(),destination.cast::<u8>(),self.len());}Ok(())}'],
 ('src/shared/value.rs', 'str', 'control'): ['{letp=NonNull::slice_from_raw_parts(base,self.len());unsafe{NonNull::new_unchecked(p.as_ptr()as*mutControl<C,P,str>)}}'],
 ('src/shared/count.rs', 'LocalCount', 'clone_strong'): ['{self.increment(false).map(|_|())}'],
 ('src/shared/count.rs', 'LocalCount', 'clone_weak'): ['{self.increment(false).map(|_|())}'],
 ('src/shared/count.rs', 'LocalCount', 'upgrade'): ['{self.increment(true)}'],
 ('src/shared/count.rs', 'LocalCount', 'initial_weak'): ['{1}'],
 ('src/shared/count.rs', 'LocalCount', 'live_count'): ['{letn=self.load();ifn<=MAX_COUNT{n}else{0}}'],
 ('src/shared/count.rs', 'LocalCount', 'external_weaks'): ['{matchself.load(){LOCK=>0,n=>n.saturating_sub(1),}}'],
 ('src/shared/count.rs', 'LocalCount', 'release_weak'): ['{self.release()}'],
 ('src/shared/count.rs', 'AtomicCount', 'buffered_clone_strong'): ['{letold=self.0.fetch_add(1,Ordering::Relaxed);letguard=MAX_COUNT.saturating_add(1);ifcrate::arithmetic::bounded::wrapping_add(old,1)&guard!=0{self.0.fetch_sub(1,Ordering::Release);Err(Error::RefCount)}else{Ok(())}}'],
 ('src/shared/count.rs', 'AtomicCount', 'observed_live_word'): ['{letmutword=self.0.load(Ordering::Relaxed);loop{ifword==0{returnNone;}ifword<=MAX_COUNT{returnSome(word);}letacquired=self.0.load(Ordering::Acquire);ifacquired!=word{word=acquired;continue;}ifweak.0.load(Ordering::Acquire)&RETIRED!=0{returnNone;}letrefreshed=self.0.load(Ordering::Acquire);ifrefreshed!=word{word=refreshed;continue;}ifweak.0.load(Ordering::Acquire)&RETIRED!=0{returnNone;}returnSome(word);}}'],
 ('src/shared/count.rs', 'AtomicCount', 'clone_strong'): ['{if!BUFFERED_STRONG{returnself.increment(false).map(|_|());}self.buffered_clone_strong()}'],
 ('src/shared/count.rs', 'AtomicCount', 'clone_weak'): ['{letmutold=self.0.load(Ordering::Relaxed);loop{ifold==LOCK{#[cfg(not(jzap_loom))]core::hint::spin_loop();#[cfg(jzap_loom)]loom::thread::yield_now();old=self.0.load(Ordering::Relaxed);continue;}ifold&!RETIRED>=WEAK_MAX_COUNT{returnErr(Error::RefCount);}letnext=old.checked_add(1).ok_or(Error::RefCount)?;matchself.0.compare_exchange(old,next,Ordering::Acquire,Ordering::Relaxed){Ok(_)=>returnOk(()),Err(observed)=>old=observed,}}}'],
 ('src/shared/count.rs', 'AtomicCount', 'upgrade'): ['{loop{letSome(old)=self.observed_live_word(weak)else{returnOk(false);};letnext=admitted_count(old)?;ifself.0.compare_exchange(old,next,Ordering::Acquire,Ordering::Relaxed).is_ok(){returnOk(true);}}}'],
 ('src/shared/count.rs', 'AtomicCount', 'initial_weak'): ['{ifretired{RETIRED|1}else{1}}'],
 ('src/shared/count.rs', 'AtomicCount', 'live_count'): ['{self.observed_live_word(weak).map_or(0,|n|n.min(MAX_COUNT))}'],
 ('src/shared/count.rs', 'AtomicCount', 'external_weaks'): ['{letword=self.0.load(Ordering::Relaxed);ifword==LOCK||word&RETIRED!=0{0}else{word.saturating_sub(1)}}'],
 ('src/shared/count.rs', 'AtomicCount', 'release_weak'): ['{letold=self.0.fetch_sub(1,Ordering::Release);ifold&!RETIRED!=1{returnfalse;}fence(Ordering::Acquire);true}']}
MACRO_ARGUMENTS = {('src/shared/traits.rs', 'downcast'): ['{Strong::from_raw(ptr)}'],
 ('src/shared/unsize.rs', 'arc_unsize'): ['{$crate::sync::Arc::from_raw(core::ptr::NonNull::new_unchecked(__coerced))}'],
 ('src/shared/unsize.rs', 'arc_weak_unsize'): ['{core::ptr::NonNull::new_unchecked(__p)}', '{$crate::sync::Weak::from_raw(__coerced)}'],
 ('src/shared/unsize.rs', 'pin_arc_unsize'): ['{$crate::shared::Pinned::into_inner_unchecked(__pinned)}',
                                              '{$crate::sync::Arc::into_pin_unchecked($crate::arc_unsize!(__owner=>$target))}'],
 ('src/shared/unsize.rs', 'pin_rc_unsize'): ['{$crate::shared::Pinned::into_inner_unchecked(__pinned)}',
                                             '{$crate::rc::Rc::into_pin_unchecked($crate::rc_unsize!(__owner=>$target))}'],
 ('src/shared/unsize.rs', 'rc_unsize'): ['{$crate::rc::Rc::from_raw(core::ptr::NonNull::new_unchecked(__coerced))}'],
 ('src/shared/unsize.rs', 'rc_weak_unsize'): ['{core::ptr::NonNull::new_unchecked(__p)}', '{$crate::rc::Weak::from_raw(__coerced)}'],
 ('src/shared/value.rs', 'native_shape'): ['{NonNull::new_unchecked(p.as_ptr()as*mutControl<C,P,Self>)}',
                                           '{core::ptr::copy_nonoverlapping((selfas*constSelf).cast::<u8>(),dst.cast::<u8>(),Layout::for_value(self).size());}']}

# Maintained callers in tools/memory-proof/shared.verus.rs and the product
# calls each must contain.
CALLERS={
 'concurrent_admit':['crate::budget::admission::admit(limit,used,bytes)'],
 'shared_increment_caller':['crate::shared::count::admitted_count(old)'],
 'shared_release_caller':['crate::arithmetic::bounded::wrapping_sub(old,1)'],
 'shared_buffered_clone_caller':['crate::arithmetic::bounded::wrapping_add(crate::shared::count::MAX_COUNT,1)','crate::arithmetic::bounded::wrapping_add(old,1)','next&guard!=0'],
 'shared_clone_weak_caller':['crate::arithmetic::bounded::wrapping_sub(crate::shared::count::MAX_COUNT,1)','old&!crate::shared::count::RETIRED>=limit','old.checked_add(1)'],
 'shared_source_retirement':['crate::shared::count::retired_state(bytes)','crate::shared::count::retired_bytes(state)'],
 'shared_prefix_driver':['crate::init::driver::run(&mutstate,length,','offset_storage(p,index)','slice_caller(p,index,1)','initialized_caller(source,index,length)'],
 'shared_data_caller':['offset_storage(a,offset)'],
 'shared_value_caller':['representation_caller(p)','slice_caller(p,0,1)'],
 'shared_header_caller':['representation_caller(h)'],
 'shared_bytes_copy_caller':['shared_data_caller(a,offset,bytes)','distinct_nonoverlap_caller(source,a,0,offset,bytes)'],
 'shared_slice_copy_caller':['representation_caller(p)','distinct_nonoverlap_caller(source,p,0,0,len)'],
}
def maintained_caller(row):
    """The maintained caller of one unsafe block or counter body, or None.

    Chosen from the operation in the block. A block that only transfers
    ownership (from_raw, assume_init, pinning) or changes a counter by a
    constant has no numeric or address statement and keeps its protocol lemma.
    """
    e=compact(row['expression']);fn=row['function'];ctx=row['context'];file=row['file']
    if row['kind']=='body':
        if file=='src/budget/concurrent.rs' and ctx=='ConcurrentMemoryBudget' and fn=='reserve':return 'concurrent_admit'
        if file!='src/shared/count.rs':return None
        # The three numeric kernels carry their own contracts on the product source.
        if ctx=='' and fn in('admitted_count','retired_state','retired_bytes'):return 'direct:shared-numeric'
        if fn in('retire',):return 'shared_source_retirement'
        if ctx=='LocalCount':
            return {'increment':'shared_increment_caller','clone_strong':'shared_increment_caller','clone_weak':'shared_increment_caller',
                    'upgrade':'shared_increment_caller','release':'shared_release_caller','release_weak':'shared_release_caller'}.get(fn)
        if ctx=='AtomicCount':
            return {'increment':'shared_increment_caller','upgrade':'shared_increment_caller','clone_strong':'shared_buffered_clone_caller',
                    'buffered_clone_strong':'shared_buffered_clone_caller','clone_weak':'shared_clone_weak_caller'}.get(fn)
        return None
    if 'prefix::initialize' in e or 'get_unchecked' in e:return 'shared_prefix_driver'
    if 'copy_nonoverlapping(' in e:
        return 'shared_slice_copy_caller' if '.cast::<T>()' in e else 'shared_bytes_copy_caller'
    if fn=='data' and ctx.startswith('Allocation'):return 'shared_data_caller'
    if fn=='retire' or 'retire(' in e:return 'shared_source_retirement'
    if fn in('new','drop') and ctx.startswith('Allocation'):return 'shared_value_caller'
    if '.release()' in e or '.release_weak()' in e:return 'shared_release_caller'
    if '.clone_strong()' in e or '.clone_weak()' in e or '.upgrade(' in e:return 'shared_increment_caller'
    if '.write(value)' in e or '.read()' in e or 'drop_in_place(data(' in e or '&mut*data(' in e or '&*data(' in e or 'InitSlot::new(' in e:
        return 'shared_value_caller'
    if 'NonNull::new_unchecked(' in e or '.control::<' in e or 'counts(' in e or 'data(' in e or fn in('counts','data','allocation_policy'):return 'shared_header_caller'
    return None

def check(root=ROOT, state_rows=None):
    root=Path(root)
    if state_rows is None:
        files=sorted(str(p.relative_to(root)) for p in (root/'src').rglob('*.rs') if not p.name.endswith('proofs.rs'))
        state_rows=json.loads(subprocess.check_output([str(scanner('state_sites')),*files],cwd=root,text=True))
    rows=[r for r in state_rows if separate(r)]
    phases={};direct_scalars={'admitted_count','retired_bytes','retired_state'};seen_scalars=set()
    for row in rows:
        assert row['kind']!='explicit_exit',('shared exit',row)
        if row['kind']=='body':
            key=(row['file'],compact(row['context']),row['function'])
            if key[0]=='src/shared/count.rs' and key[1]=='' and key[2] in direct_scalars:
                seen_scalars.add(key[2]);continue
            phases.setdefault(key,[]).append(compact(row['expression']))
    assert seen_scalars==direct_scalars, 'shared direct numeric domain changed'
    # Declaration audit only. Numeric bodies are checked by direct Verus/Kani;
    # these strings are never a positive proof input.
    declarations=json.loads(subprocess.check_output([str(scanner('state_sites')),'--declarations','src/shared/count.rs'],cwd=root,text=True))
    scalar_declarations=[r for r in declarations if r['kind']=='declaration' and r['function'] in direct_scalars]
    assert len(scalar_declarations)==3 and all(r['detail']['direct_contract'] for r in scalar_declarations), 'shared direct numeric contract disabled'
    assert phases==PHASES,('shared phase/actual-argument binding changed',[(k,v)for k,v in phases.items()if PHASES.get(k)!=v],set(PHASES)-set(phases))
    owner=compact((root/'src/shared.rs').read_text())
    for fields in [
        '#[repr(C)]pubstructControl<C:Counter,P:SharedPolicy,T:?Sized>{strong:C,weak:C,policy:ManuallyDrop<P>,data:T,}',
        'pubstructStrong<T:?Sized,C:Counter,P:SharedPolicy=Unmetered>{ptr:NonNull<Control<C,P,T>>,marker:PhantomData<(P,T)>,}',
        'pubstructWeak<T:?Sized,C:Counter,P:SharedPolicy=Unmetered>{ptr:Option<NonNull<Control<C,P,T>>>,marker:PhantomData<(P,T)>,}',
    ]:assert fields in owner,'unbound shared layout/encapsulation'
    owned=compact((root/'src/budget/owned.rs').read_text())
    for declaration in ['pubstructOwnedBudget{inner:crate::rc::Rc<MemoryBudget>,}',
                        'pubstructOwnedConcurrentBudget{inner:crate::sync::Arc<ConcurrentMemoryBudget>,}']:
        assert declaration in owned, 'owning policy ledger representation changed'
    assert not re.search(r'#\[derive\([^]]*\b(?:Clone|Copy)\b',owned), 'implicit owning policy duplication'
    shapes=compact((root/'src/shared/value.rs').read_text())
    assert 'fncompatible(&self)->bool{true}' in shapes
    assert 'native_shape!(std::ffi::OsStr,os_bytes);' in shapes and 'native_shape!(std::path::Path,path_bytes);' in shapes
    pinned=compact((root/'src/shared/pinned.rs').read_text())
    assert 'pubstructPinned<T:?Sized,C:Counter,P:SharedPolicy=Unmetered>{owner:Strong<T,C,P>,}' in pinned, 'pinned owner must remain private'
    # Compiler-generated Clone/Copy bodies are not ordinary source phases.
    # Shared owners/counters cannot gain an implicit count/refund bypass.
    shared_files=[root/'src/shared.rs',*sorted((root/'src/shared').glob('*.rs'))]
    for file in shared_files:
        if not file.name.endswith('proofs.rs'):
            assert not re.search(r'#\[derive\([^]]*\b(?:Clone|Copy)\b', file.read_text()), 'implicit shared clone/copy derive'
    concurrent=compact((root/'src/budget/concurrent.rs').read_text())
    for declaration in [
        'pubstructConcurrentMemoryBudget{limit:usize,used:AtomicUsize,}',
        "pubstructConcurrentReservation<'a>{budget:&'aConcurrentMemoryBudget,bytes:usize,}",
        "pubstructConcurrentCharge<'a>{budget:&'aConcurrentMemoryBudget,bytes:usize,}",
    ]: assert declaration in concurrent, 'concurrent ledger/token fields must remain private'
    policy_derive="#[derive(Clone,Copy,Debug)]pubstructConcurrentBudget<'a>(pub&'aConcurrentMemoryBudget);"
    assert policy_derive in concurrent
    assert not re.search(r'#\[derive\([^]]*(?:Clone|Copy)',concurrent.replace(policy_derive,'')), 'duplicated accounting token'
    assert 'pubtraitSharedPolicy:AllocationPolicy' in compact((root/'src/shared/policy.rs').read_text())
    assert re.search(r'pubfninto_pin\(this:Self\)->Pinned<T,C,P>whereT:Unpin,?\{',pinned),'unpinned alias conversion'
    counters=compact((root/'src/shared/count.rs').read_text())
    traits=[r for r in rows if r['kind']=='counter_trait']
    assert len(traits)==1, 'missing sealed counter trait AST'
    declaration=compact(traits[0]['expression']).split('pubtraitCounter',1)[1]
    assert declaration == ':sealed::Sealed{#[doc="#Safety"]#[doc="Establishandmaintainavalidcountstateforeverysubsequentprotocol"]#[doc="operation.Countersareconstructedonlyinsideanownedcontrolblock."]unsafefnnew(value:usize)->Self;fnload(&self)->usize;fnincrement(&self,live:bool)->Result<bool,Error>;fnclone_strong(&self)->Result<(),Error>;fnclone_weak(&self)->Result<(),Error>;fnupgrade(&self,weak:&Self)->Result<bool,Error>;fninitial_weak(retired:bool)->usize;fnlive_count(&self,weak:&Self)->usize;fnexternal_weaks(&self)->usize;fnrelease(&self)->bool;fnrelease_weak(&self)->bool;fntake_last(&self)->bool;fnpublish(&self,weak:&Self);fnretire(&self,weak:&Self,bytes:usize);fnunique(strong:&Self,weak:&Self)->bool;}', 'unbound counter method/default body'
    assert 'pubstructLocalCount(Cell<usize>);' in counters and 'pubstructAtomicCount(AtomicUsize);' in counters
    counter_source=(root/'src/shared/count.rs').read_text()
    assert re.search(r'(?m)^mod sealed \{',counter_source) and not re.search(r'pub\s+use\b',counter_source), 'counter sealing changed'
    assert '#[cfg(not(jzap_small_counts))]pubconstMAX_COUNT:usize=isize::MAXasusize;' in counters.replace('#[cfg_attr(verus_keep_ghost,verus_verify)]','')
    assert '#[cfg(jzap_small_counts)]pubconstMAX_COUNT:usize=7;' in counters.replace('#[cfg_attr(verus_keep_ghost,verus_verify)]','')
    assert 'pubconstLOCK:usize=usize::MAX;' in counters
    assert 'pubconstWEAK_MAX_COUNT:usize=MAX_COUNT.saturating_sub(1);' in counters
    assert 'pubconstBUFFERED_STRONG:bool=cfg!(any(jzap_small_counts,all(target_os="linux",target_arch="x86_64",target_pointer_width="64")));' in counters, 'buffered admission platform applicability changed'
    files=['src/shared.rs',*sorted(str(p.relative_to(root)) for p in (root/'src/shared').glob('*.rs') if not p.name.endswith('proofs.rs'))]
    unsafe=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*files],cwd=root,text=True))
    blocks=[r for r in unsafe if r['kind']in('unsafe_block','macro_unsafe_block')]
    assert len(blocks)==98
    macro_signatures=[(r['file'],r['function'],compact(r['expression'])) for r in unsafe if r['kind']=='macro_unsafe_fn']
    assert macro_signatures==[('src/shared/value.rs','native_shape','fncontrol<C:Counter,P:SharedPolicy>(&self,base:NonNull<u8>)->NonNull<Control<C,P,Self>>'),('src/shared/value.rs','native_shape','fnclone_into(&self,dst:*mutSelf)->Result<(),Error>')], 'unbound macro unsafe signature'
    signatures=[r for r in unsafe if r['kind']=='unsafe_fn'];assert len(signatures)==23
    assert sorted(r['function']for r in signatures)==['assume_init', 'assume_init', 'clone_into', 'clone_into', 'clone_into', 'clone_into', 'control', 'control', 'control', 'control', 'control_from_value', 'counts', 'data', 'decrement_strong_count', 'drop_slow', 'from_raw', 'from_raw', 'into_inner_unchecked', 'into_pin_unchecked', 'new', 'new', 'retire', 'try_increment_strong_count']
    impls=[r for r in unsafe if r['kind']=='unsafe_impl'];assert len(impls)==4
    assert all('AtomicCount' in r['context']for r in impls)
    macro={}
    bindings=[]
    for row in blocks:
        if row['kind']=='macro_unsafe_block':macro.setdefault((row['file'],row['function']),[]).append(compact(row['expression']))
        else:
            containing=[r for r in rows if r['kind']=='body' and r['file']==row['file'] and r['context']==row['context'] and r['function']==row['function']
                and (r['line'],r['column'])<=(row['line'],row['column']) and (row['end_line'],row['end_column'])<=(r['end_line'],r['end_column'])]
            assert len(containing)==1,('unbound unsafe phase',row)
        fn=row['function']
        contract='shared_payload_extent'
        if fn in('counts','strong_count','weak_count','try_clone','try_downgrade','try_upgrade','get_mut','try_increment_strong_count'):contract='shared_count_credit'
        if fn in ('try_new_cyclic_in','try_init_cyclic_in') and 'publish' in row['expression']:contract='shared_cyclic_publish'
        if fn=='control_from_value':contract='shared_pointer_frame'
        if fn=='retire':contract='shared_retired_word'
        if fn=='try_make_mut':contract='shared_cow_retirement'
        if fn in('try_unwrap','into_inner','drop_slow'):contract='shared_last_strong'
        if fn=='drop' and row['context'].startswith('Strong'):contract='shared_last_strong' if 'drop_slow' in row['expression'] else 'shared_release_credit'
        if fn=='drop' and row['context'].startswith('WeakRelease'):contract='shared_last_weak'
        if fn in('control','data','new','uninit_slice'):contract='shared_layout_extent'
        if fn in('assume_init','from_raw','decrement_strong_count','as_ref','downcast','rc_unsize','arc_unsize','rc_weak_unsize','arc_weak_unsize'):contract='shared_typed_transfer'
        if 'prefix::initialize' in row['expression'].replace(' ','') or 'get_unchecked' in row['expression']:contract='shared_prefix_driver'
        caller=maintained_caller(row);also=[contract] if caller else []
        bindings.append(dict(site=row,contract=caller or contract,also=also,role='shared-source-phase',boundary='typed allocation/provenance/unsafe-caller contracts; compiler/Miri/Loom'))
    assert macro==MACRO_ARGUMENTS,'shared coercion arguments changed'
    # Counter and receipt operations are also proof obligations even when they
    # contain no unsafe block. The phase list above prevents new safe factories
    # or unadmitted credit changes from inheriting this certificate.
    for row in rows:
        if row['kind']=='body' and (row['file']=='src/shared/count.rs' or row['file'].startswith('src/budget/') or row['context'].startswith('Accounting') or (row['file']=='src/budget.rs' and row['context']=='MemoryBudget' and row['function']=='limit')):
            if row['file']=='src/budget/owned.rs' or (row['file']=='src/budget.rs' and row['context']=='MemoryBudget'):
                contract='shared_count_credit' if row['function']=='try_clone' else 'shared_layout_extent' if row['function']=='try_new' else 'shared_typed_transfer'
                if row['function']=='reserve':
                    contract='kani:budget::panic_proofs::reservation_and_commit_preserve_every_valid_state' if row['context']=='OwnedBudget' else 'kani:budget::concurrent::proofs::actual_concurrent_reservation_commit_and_cancel'
                bindings.append(dict(site=row,contract=contract,role='owning-policy-checked-forwarding'))
            else:
                contract='shared_retired_word' if row['function'] in ('retire','retired_state','retired_bytes') else 'shared_release_credit' if row['function']=='release' else 'shared_count_credit' if row['file']=='src/shared/count.rs' else 'concurrent_commit_preserves_tokens'
                if row['function']=='buffered_clone_strong':contract='buffered_reservation_extent'
                if row['function']=='clone_weak':contract='shared_weak_credit'
                if row['function']=='upgrade':contract='shared_upgrade'
                if row['function']=='publish':contract='shared_cyclic_publish'
                if row['function']=='release_weak':contract='shared_last_weak'
                if row['context']=='AtomicCount' and row['function']=='retire':contract='shared_sole_weak_retirement'
                if row['file']=='src/budget/admission.rs':contract='direct:budget-admit'
                caller=maintained_caller(row);also=[contract] if caller else []
                bindings.append(dict(site=row,contract=caller or contract,also=also,role='shared-credit-transition'))
    return bindings
