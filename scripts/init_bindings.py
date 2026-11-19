"""Read-only runtime glue binding; proof inputs are original Rust, never these strings."""
from pathlib import Path
from verify_edit_loops import block
from caller_bindings import compact

BODIES = {
    ('src/init/prefix.rs','fn drop(&mut self)'): """if !core::mem::needs_drop::<T>() { return; }
        unsafe { core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
            self.ptr.as_ptr(), self.state.get(), )); }""",
    ('src/init/prefix.rs','unsafe fn initialize_one<T, E>('): """
        let pointer = unsafe { NonNull::new_unchecked(ptr.as_ptr().add(index)) };
        let mut slot = unsafe { InitSlot::new(pointer) };
        initialize(index, &mut slot).map_err(InitError::Initialize)?;
        slot.finish().map_err(InitError::Storage)""",
    ('src/init/prefix.rs','pub(crate) unsafe fn initialize<T, E>('): """
        let mut prefix = Prefix { ptr, state: Checked::new(0), };
        super::driver::run(&mut prefix.state, length, |index| unsafe {
            initialize_one(ptr, index, initialize)
        })?;
        prefix.state = Checked::new(0); Ok(())""",
    ('src/init.rs','pub fn try_init_array<E>('): """
        use crate::error::InitError;
        if self.initialized { return Err(InitError::Storage(Error::AlreadyInitialized)); }
        unsafe { prefix::initialize(self.ptr.cast::<T>(), N, &mut initialize) }?;
        self.initialized = true; Ok(())""",
    ('src/boxed/slice.rs','pub fn try_init_slice_in<E>('): """
        let layout = crate::layout::array::<T>(len).map_err(InitError::Storage)?;
        let ptr = storage::allocate(layout, policy.budget()).map_err(InitError::Storage)?.cast::<T>();
        let allocation = AllocationGuard(ptr.cast(), layout, policy.budget());
        unsafe { crate::init::prefix::initialize(ptr, len, &mut initialize) }?;
        core::mem::forget(allocation);
        Ok(unsafe { Self::from_raw_parts(NonNull::slice_from_raw_parts(ptr, len), policy) })""",
    ('src/clone.rs','pub(crate) fn array_from_fn<T, const N: usize>('): """
        let mut storage = core::mem::MaybeUninit::<[T; N]>::uninit();
        let ptr = core::ptr::NonNull::from(&mut storage).cast::<T>();
        let mut initialize = |index, slot: &mut crate::init::InitSlot<'_, T>| {
            slot.write(f(index)?).map(|_| ()).map_err(|e| e.error)
        };
        unsafe { crate::init::prefix::initialize(ptr, N, &mut initialize) }.map_err(|e| match e {
            crate::error::InitError::Storage(e) | crate::error::InitError::Initialize(e) => e,
        })?;
        Ok(unsafe { storage.assume_init() })""",
}

BODIES.update({
    ('src/vec/edit.rs','fn fill('): """
        let start=self.drain.owner.len;
        let _=(Checked::new(self.drain.tail_start)-start).map_err(|_| Error::Bounds)?;
        unsafe {crate::init::collection::fill(self.drain.owner.ptr,&mut self.drain.owner.len,self.drain.tail_start,&mut self.replacement,)}
        .map_err(InitError::Initialize)""",
    ('src/vec/edit.rs','fn stage_batch<'): """
        stage_push(extra,value).map_err(InitError::Storage)?;
        let end=extra.capacity();
        if !unsafe {crate::init::collection::fill(extra.ptr,&mut extra.len,end,replacement)}
            .map_err(InitError::Initialize)? {return Ok(None);}
        replacement.try_next().map_err(InitError::Initialize)""",
    ('src/init/collection.rs','unsafe fn write_next<T, I: TryIterator<Item = T>>('): """
        let value=iter.try_next().map_err(Stop::Source)?.ok_or(Stop::End)?;
        unsafe {ptr.as_ptr().add(index).write(value)}; Ok(())""",
    ('src/init/collection.rs','pub(crate) unsafe fn fill<T, I: TryIterator<Item = T>>('): """
        let mut current=*state; let progress=Commit {output:state,current:&mut current,};
        finished(super::driver::run(progress.current,end,|index| unsafe {write_next(ptr,index,iter)}))""",
    ('src/init/collection.rs','fn drop(&mut self)'): """
        *self.output=*self.current;""",
    ('src/boxed/buffer.rs','fn reserve('): """
        let (needed,proposed)=super::buffer_numeric::reservation(self.len.get(),self.cap.get(),additional,hint)?;
        if needed<=self.cap.get() {return Ok(needed);}
        let layout=crate::layout::array::<T>(proposed)?;
        let ptr=unsafe {storage::reallocate(self.ptr.cast(),self.layout,layout,self.policy.budget())}?;
        self.ptr=ptr.cast();self.cap=Checked::new(proposed);self.layout=layout;Ok(needed)""",
    ('src/boxed/buffer.rs','fn collect<E>('): """
        let lower=iter.try_size_hint().map_err(InitError::Initialize)?.0;
        let reserved_end=self.reserve(lower,lower).map_err(InitError::Storage)?;
        if !unsafe {crate::init::collection::fill(self.ptr,&mut self.len,reserved_end,&mut iter)}
            .map_err(InitError::Initialize)? {return Ok(());}
        let Some(first)=iter.try_next().map_err(InitError::Initialize)? else {return Ok(());};
        self.collect_rest(first,iter)""",
    ('src/boxed/buffer.rs','fn collect_rest<E>('): """
        self.push(first,iter.try_size_hint().map_err(InitError::Initialize)?.0.saturating_add(1),)
            .map_err(InitError::Storage)?;
        loop {
            if !unsafe {crate::init::collection::fill(self.ptr,&mut self.len,self.cap.get(),&mut iter)}
                .map_err(InitError::Initialize)? {return Ok(());}
            let Some(item)=iter.try_next().map_err(InitError::Initialize)? else {return Ok(());};
            self.push(item,iter.try_size_hint().map_err(InitError::Initialize)?.0.saturating_add(1),)
                .map_err(InitError::Storage)?;
        }""",
})

def check(root):
    import re
    root=Path(root)
    for (file,signature),expected in BODIES.items():
        actual=block((root/file).read_text(),signature)
        actual=re.sub(r'//[^\n]*','',actual)
        assert compact(actual)==compact(expected), ('unbound collection extent' if file=='src/boxed/buffer.rs' and 'collect' in signature else 'initialization caller binding requires review',file,signature)
