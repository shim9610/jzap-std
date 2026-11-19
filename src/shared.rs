//! Common strong/weak ownership protocol. Rc and Arc select sealed counters.
use crate::{
    Error, Rejected, TryClone, TryCloneIn,
    budget::{Budget, Unmetered},
    error::InitError,
};
use core::{
    alloc::Layout,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ptr::NonNull,
};
mod convert;
#[doc(hidden)]
pub mod count;
mod pinned;
mod policy;
#[cfg(kani)]
mod proofs;
mod traits;
mod unsize;
mod value;
use count::Counter;
pub use pinned::Pinned;
pub use policy::SharedPolicy;

/// Layout exposed only to support compiler-checked unsizing in exported macros.
#[doc(hidden)]
#[repr(C)]
pub struct Control<C: Counter, P: SharedPolicy, T: ?Sized> {
    strong: C,
    weak: C,
    policy: ManuallyDrop<P>,
    data: T,
}
/// Implementation type; prefer rc::Rc or sync::Arc.
pub struct Strong<T: ?Sized, C: Counter, P: SharedPolicy = Unmetered> {
    ptr: NonNull<Control<C, P, T>>,
    marker: PhantomData<(P, T)>,
}
/// Implementation type; prefer rc::Weak or sync::Weak.
pub struct Weak<T: ?Sized, C: Counter, P: SharedPolicy = Unmetered> {
    ptr: Option<NonNull<Control<C, P, T>>>,
    marker: PhantomData<(P, T)>,
}
struct Allocation<C: Counter, P: SharedPolicy> {
    ptr: NonNull<u8>,
    layout: Layout,
    offset: usize,
    marker: PhantomData<(C, P)>,
}
impl<C: Counter, P: SharedPolicy> Allocation<C, P> {
    fn new(value: Layout, policy: P, strong: usize, zeroed: bool) -> Result<Self, Error> {
        let (layout, offset) = allocation_layout::<C, P>(value)?;
        let ptr = if zeroed {
            crate::storage::allocate_zeroed(layout, policy.budget())?
        } else {
            crate::storage::allocate(layout, policy.budget())?
        };
        let retired = strong == 0 && policy.budget().is_metered();
        // SAFETY: the checked extended layout covers this initialized header;
        // its zero-sized tail writes no payload bytes. Payload stays unexposed.
        unsafe {
            ptr.cast::<Control<C, P, ()>>().as_ptr().write(Control {
                strong: C::new(if retired {
                    count::retired_state(layout.size())
                } else {
                    strong
                }),
                weak: C::new(C::initial_weak(retired)),
                policy: ManuallyDrop::new(policy),
                data: (),
            });
        }
        Ok(Self {
            ptr,
            layout,
            offset,
            marker: PhantomData,
        })
    }
    fn data(&self) -> *mut u8 {
        // SAFETY: checked layout extension establishes offset <= allocation size.
        unsafe { self.ptr.as_ptr().add(self.offset) }
    }
    fn finish<T: ?Sized>(self, ptr: NonNull<Control<C, P, T>>) -> Strong<T, C, P> {
        core::mem::forget(self);
        Strong {
            ptr,
            marker: PhantomData,
        }
    }
}
impl<C: Counter, P: SharedPolicy> Drop for Allocation<C, P> {
    fn drop(&mut self) {
        // SAFETY: initialized header, no payload ownership published. Separate
        // InitSlot/prefix guards finish cleanup before this allocation guard.
        unsafe {
            // The header owns its policy once. Move it out before freeing the
            // header, and keep it alive until the storage charge is refunded.
            let header = self.ptr.cast::<Control<C, P, ()>>().as_ptr();
            let policy = ManuallyDrop::take(&mut (*header).policy);
            self.ptr
                .cast::<Control<C, P, ()>>()
                .as_ptr()
                .drop_in_place();
            crate::storage::deallocate(self.ptr, self.layout, policy.budget());
        }
    }
}
pub(crate) fn allocation_layout<C: Counter, P: SharedPolicy>(
    value: Layout,
) -> Result<(Layout, usize), Error> {
    let (layout, offset) = Layout::new::<Control<C, P, ()>>()
        .extend(value)
        .map_err(|_| Error::Layout)?;
    Ok((layout.pad_to_align(), offset))
}
// SAFETY: caller owns a strong or weak credit, keeping the header allocated.
unsafe fn counts<'a, T: ?Sized, C: Counter, P: SharedPolicy>(
    ptr: NonNull<Control<C, P, T>>,
) -> (&'a C, &'a C) {
    unsafe {
        (
            &*core::ptr::addr_of!((*ptr.as_ptr()).strong),
            &*core::ptr::addr_of!((*ptr.as_ptr()).weak),
        )
    }
}
// SAFETY: valid control pointer and metadata; does not borrow/read payload.
unsafe fn data<T: ?Sized, C: Counter, P: SharedPolicy>(ptr: NonNull<Control<C, P, T>>) -> *mut T {
    unsafe { core::ptr::addr_of_mut!((*ptr.as_ptr()).data) }
}
// SAFETY: the caller has retired the last strong credit but T is still
// initialized; the implicit weak credit keeps its allocation alive.
unsafe fn retire<T: ?Sized, C: Counter, P: SharedPolicy>(ptr: NonNull<Control<C, P, T>>) {
    unsafe {
        let policy = &*core::ptr::addr_of!((*ptr.as_ptr()).policy).cast::<P>();
        if policy.budget().is_metered() {
            let (strong, weak) = counts(ptr);
            strong.retire(weak, Layout::for_value(&*ptr.as_ptr()).size());
        }
    }
}
struct WeakRelease<T: ?Sized, C: Counter, P: SharedPolicy>(NonNull<Control<C, P, T>>);
impl<T: ?Sized, C: Counter, P: SharedPolicy> Drop for WeakRelease<T, C, P> {
    fn drop(&mut self) {
        // SAFETY: this guard owns exactly one weak credit, including during
        // payload unwind. No reference to a dead/uninitialized T is formed.
        unsafe {
            if !counts(self.0).1.release_weak() {
                return;
            }
            let policy = ManuallyDrop::take(&mut (*self.0.as_ptr()).policy);
            let bytes = if policy.budget().is_metered() {
                count::retired_bytes(counts(self.0).0.load())
            } else {
                0
            };
            // Standard Box supplies the stable compiler's DST deallocation glue.
            // ManuallyDrop suppresses a second payload destructor; no allocation
            // or OOM handling occurs on this path.
            drop(alloc::boxed::Box::from_raw(
                self.0.as_ptr() as *mut Control<C, P, ManuallyDrop<T>>
            ));
            policy.budget().release(bytes);
        }
    }
}
impl<T, C: Counter> Strong<T, C> {
    #[inline]
    pub fn try_new(value: T) -> Result<Self, Rejected<T>> {
        Self::try_new_in(value, Unmetered)
    }
    pub fn try_new_uninit() -> Result<Strong<MaybeUninit<T>, C>, Error> {
        Self::try_new_uninit_in(Unmetered)
    }
    pub fn try_new_zeroed() -> Result<Strong<MaybeUninit<T>, C>, Error> {
        Self::try_new_zeroed_in(Unmetered)
    }
    pub fn try_new_cyclic<E>(
        initialize: impl FnOnce(&Weak<T, C>) -> Result<T, E>,
    ) -> Result<Self, InitError<E>> {
        Self::try_new_cyclic_in(Unmetered, initialize)
    }
}
impl<T, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    #[inline]
    pub fn try_new_in(value: T, policy: P) -> Result<Self, Rejected<T>> {
        let allocation = match Allocation::<C, P>::new(Layout::new::<T>(), policy, 1, false) {
            Ok(a) => a,
            Err(error) => return Err(Rejected { value, error }),
        };
        let ptr = allocation.ptr.cast::<Control<C, P, T>>();
        // SAFETY: exact T layout, uninitialized and exclusively owned.
        unsafe {
            allocation.data().cast::<T>().write(value);
        }
        Ok(allocation.finish(ptr))
    }
    pub fn try_init_in<I: crate::init::TryInit<T>>(
        policy: P,
        initialize: I,
    ) -> Result<Self, InitError<I::Error>> {
        let allocation = Allocation::<C, P>::new(Layout::new::<T>(), policy, 1, false)
            .map_err(InitError::Storage)?;
        let ptr = allocation.ptr.cast::<Control<C, P, T>>();
        // SAFETY: the allocation guard outlives the exclusive initialization slot.
        let mut slot = unsafe {
            crate::init::InitSlot::new(NonNull::new_unchecked(allocation.data().cast::<T>()))
        };
        initialize
            .try_init(&mut slot)
            .map_err(InitError::Initialize)?;
        slot.finish().map_err(InitError::Storage)?;
        Ok(allocation.finish(ptr))
    }
    pub fn try_new_with_in<E>(
        policy: P,
        initialize: impl FnOnce() -> Result<T, E>,
    ) -> Result<Self, InitError<E>> {
        Self::try_init_in(policy, |slot: &mut crate::init::InitSlot<'_, T>| {
            slot.write(initialize().map_err(InitError::Initialize)?)
                .map_err(|e| InitError::Storage(e.error))
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(InitError::Storage(e)) => {
                InitError::Storage(e)
            }
            InitError::Initialize(InitError::Initialize(e)) => InitError::Initialize(e),
        })
    }
    pub fn try_new_uninit_in(policy: P) -> Result<Strong<MaybeUninit<T>, C, P>, Error> {
        Strong::try_new_in(MaybeUninit::uninit(), policy).map_err(|e| e.error)
    }
    pub fn try_new_zeroed_in(policy: P) -> Result<Strong<MaybeUninit<T>, C, P>, Error> {
        let a = Allocation::<C, P>::new(Layout::new::<T>(), policy, 1, true)?;
        let p = a.ptr.cast::<Control<C, P, MaybeUninit<T>>>();
        Ok(a.finish(p))
    }
    pub fn try_new_cyclic_in<E>(
        policy: P,
        initialize: impl FnOnce(&Weak<T, C, P>) -> Result<T, E>,
    ) -> Result<Self, InitError<E>> {
        let a = Allocation::<C, P>::new(Layout::new::<T>(), policy, 0, false)
            .map_err(InitError::Storage)?;
        let ptr = a.ptr.cast::<Control<C, P, T>>();
        core::mem::forget(a);
        let weak = Weak {
            ptr: Some(ptr),
            marker: PhantomData,
        };
        let value = match initialize(&weak) {
            Ok(value) => value,
            Err(error) => return Err(InitError::Initialize(error)),
        };
        // SAFETY: the callback returns an owned initialized T. Construction's
        // weak credit keeps checked storage alive through the move. No fallible
        // operation follows the write; release publication exposes the value.
        unsafe {
            data(ptr).write(value);
            let (strong, weak) = counts(ptr);
            strong.publish(weak);
        }
        core::mem::forget(weak);
        Ok(Self {
            ptr,
            marker: PhantomData,
        })
    }
    /// Initialize a cyclic value directly in its final storage. Callback success
    /// is accepted only after InitSlot records complete initialization.
    pub fn try_init_cyclic_in<E>(
        policy: P,
        initialize: impl FnOnce(&Weak<T, C, P>, &mut crate::init::InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<Self, InitError<E>> {
        let a = Allocation::<C, P>::new(Layout::new::<T>(), policy, 0, false)
            .map_err(InitError::Storage)?;
        let ptr = a.ptr.cast::<Control<C, P, T>>();
        core::mem::forget(a);
        let weak = Weak {
            ptr: Some(ptr),
            marker: PhantomData,
        };
        // SAFETY: allocation is held by construction's weak credit. The slot
        // is declared later so initialized payload cleanup precedes weak release.
        let mut slot = unsafe { crate::init::InitSlot::new(NonNull::new_unchecked(data(ptr))) };
        initialize(&weak, &mut slot).map_err(InitError::Initialize)?;
        slot.finish().map_err(InitError::Storage)?;
        // SAFETY: recorded initialization precedes release publication; upgrades
        // acquire it. Construction weak becomes exactly one implicit credit.
        unsafe {
            let (strong, weak) = counts(ptr);
            strong.publish(weak);
        }
        core::mem::forget(weak);
        Ok(Self {
            ptr,
            marker: PhantomData,
        })
    }
    pub fn try_unwrap(this: Self) -> Result<T, Self> {
        // SAFETY: this strong owner keeps header and payload alive.
        if !unsafe { counts(this.ptr).0.take_last() } {
            return Err(this);
        }
        let this = ManuallyDrop::new(this);
        // SAFETY: payload is still initialized, and its implicit weak credit
        // keeps the complete control block alive before retirement publication.
        unsafe {
            retire(this.ptr);
        }
        let _release = WeakRelease(this.ptr);
        // SAFETY: successful 1->0 transition excludes every other strong owner.
        Ok(unsafe { data(this.ptr).read() })
    }
    pub fn into_inner(this: Self) -> Option<T> {
        // Release rather than CAS is required: among concurrent into_inner
        // calls exactly the final owner obtains T, including a failed unwrap race.
        if !unsafe { counts(this.ptr).0.release() } {
            core::mem::forget(this);
            return None;
        }
        let this = ManuallyDrop::new(this);
        // SAFETY: payload is still initialized, and its implicit weak credit
        // keeps the complete control block alive before retirement publication.
        unsafe {
            retire(this.ptr);
        }
        let _release = WeakRelease(this.ptr);
        // SAFETY: last strong credit transferred payload ownership here.
        Some(unsafe { data(this.ptr).read() })
    }
    pub fn try_unwrap_or_clone(this: Self) -> Result<T, Rejected<Self>>
    where
        T: TryClone,
    {
        match Self::try_unwrap(this) {
            Ok(v) => Ok(v),
            Err(value) => match value.as_ref().try_clone() {
                Ok(v) => Ok(v),
                Err(error) => Err(Rejected { value, error }),
            },
        }
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    #[inline]
    pub fn as_ptr(this: &Self) -> *const T {
        unsafe { data(this.ptr) }
    }
    #[inline]
    pub fn allocation_policy(this: &Self) -> &P {
        // SAFETY: immutable header receipt remains initialized until last weak.
        unsafe { &*core::ptr::addr_of!((*this.ptr.as_ptr()).policy).cast::<P>() }
    }
    #[inline]
    pub fn strong_count(this: &Self) -> usize {
        let (strong, weak) = unsafe { counts(this.ptr) };
        strong.live_count(weak)
    }
    #[inline]
    pub fn weak_count(this: &Self) -> usize {
        unsafe { counts(this.ptr).1.external_weaks() }
    }
    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        core::ptr::addr_eq(this.ptr.as_ptr(), other.ptr.as_ptr())
    }
    #[inline]
    pub fn try_clone(this: &Self) -> Result<Self, Error> {
        unsafe {
            counts(this.ptr).0.clone_strong()?;
        }
        Ok(Self {
            ptr: this.ptr,
            marker: PhantomData,
        })
    }
    #[inline]
    pub fn try_downgrade(this: &Self) -> Result<Weak<T, C, P>, Error> {
        unsafe {
            counts(this.ptr).1.clone_weak()?;
        }
        Ok(Weak {
            ptr: Some(this.ptr),
            marker: PhantomData,
        })
    }
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        let (strong, weak) = unsafe { counts(this.ptr) };
        if !C::unique(strong, weak) {
            return None;
        }
        // SAFETY: the weak lock/count protocol excludes all aliases to T.
        Some(unsafe { &mut *data(this.ptr) })
    }
    /// Raw control pointer, including metadata and its original accounting.
    /// Its address differs from as_ptr's payload address.
    pub fn into_raw(this: Self) -> NonNull<Control<C, P, T>> {
        ManuallyDrop::new(this).ptr
    }
    /// # Safety
    /// ptr must own one unreconstructed strong credit returned by into_raw, of
    /// exactly this counter/policy/payload type, with valid metadata and lifetime.
    pub unsafe fn from_raw(ptr: NonNull<Control<C, P, T>>) -> Self {
        Self {
            ptr,
            marker: PhantomData,
        }
    }
    /// # Safety
    /// ptr names a live allocation of this exact type with an existing strong
    /// credit. Success creates one additional credit; failure changes nothing.
    pub unsafe fn try_increment_strong_count(ptr: NonNull<Control<C, P, T>>) -> Result<(), Error> {
        let (strong, weak) = unsafe { counts(ptr) };
        if strong.upgrade(weak)? {
            Ok(())
        } else {
            Err(Error::InvalidStructure)
        }
    }
    /// # Safety
    /// ptr owns one live strong credit of this exact type, which is consumed.
    pub unsafe fn decrement_strong_count(ptr: NonNull<Control<C, P, T>>) {
        drop(unsafe { Self::from_raw(ptr) });
    }
}
/// # Safety
/// The last strong credit has been retired, T is initialized, and its implicit
/// weak credit still owns the allocation. Keep this final-release path outside
/// hot clone/drop loops, following std's drop_slow organization.
#[inline(never)]
unsafe fn drop_slow<T: ?Sized, C: Counter, P: SharedPolicy>(ptr: NonNull<Control<C, P, T>>) {
    // SAFETY: record the layout before the payload's lifetime ends.
    unsafe { retire(ptr) };
    let _release = WeakRelease(ptr);
    // SAFETY: last owner destroys T once; the guard retains storage during unwind.
    unsafe { core::ptr::drop_in_place(data(ptr)) };
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Drop for Strong<T, C, P> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: this handle owns a live strong credit.
        if !unsafe { counts(self.ptr).0.release() } {
            return;
        }
        // SAFETY: release admitted final destruction with its implicit weak held.
        unsafe { drop_slow(self.ptr) };
    }
}
impl<T, C: Counter, P: SharedPolicy> Weak<T, C, P> {
    pub const fn new() -> Self {
        Self {
            ptr: None,
            marker: PhantomData,
        }
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Weak<T, C, P> {
    pub fn try_upgrade(&self) -> Result<Option<Strong<T, C, P>>, Error> {
        let Some(ptr) = self.ptr else {
            return Ok(None);
        };
        let (strong, weak) = unsafe { counts(ptr) };
        if !strong.upgrade(weak)? {
            return Ok(None);
        }
        Ok(Some(Strong {
            ptr,
            marker: PhantomData,
        }))
    }
    pub fn try_clone(&self) -> Result<Self, Error> {
        if let Some(ptr) = self.ptr {
            unsafe {
                counts(ptr).1.clone_weak()?;
            }
        }
        Ok(Self {
            ptr: self.ptr,
            marker: PhantomData,
        })
    }
    pub fn strong_count(&self) -> usize {
        self.ptr.map_or(0, |p| {
            let (strong, weak) = unsafe { counts(p) };
            strong.live_count(weak)
        })
    }
    pub fn weak_count(&self) -> usize {
        let Some(ptr) = self.ptr else {
            return 0;
        };
        let (s, w) = unsafe { counts(ptr) };
        if s.live_count(w) == 0 {
            return 0;
        }
        w.external_weaks()
    }
    pub fn as_ptr(&self) -> Option<*const T> {
        self.ptr.map(|p| unsafe { data(p) as *const T })
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (self.ptr, other.ptr) {
            (None, None) => true,
            (Some(a), Some(b)) => core::ptr::addr_eq(a.as_ptr(), b.as_ptr()),
            _ => false,
        }
    }
    pub fn into_raw(self) -> Option<NonNull<Control<C, P, T>>> {
        ManuallyDrop::new(self).ptr
    }
    /// # Safety
    /// ptr owns one weak credit returned by into_raw with identical type,
    /// accounting and lifetime. None represents an empty weak handle.
    pub unsafe fn from_raw(ptr: Option<NonNull<Control<C, P, T>>>) -> Self {
        Self {
            ptr,
            marker: PhantomData,
        }
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Drop for Weak<T, C, P> {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr {
            drop(WeakRelease(ptr));
        }
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> TryClone for Strong<T, C, P> {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Self::try_clone(self)
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> TryClone for Weak<T, C, P> {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Self::try_clone(self)
    }
}
impl<'a, T: ?Sized, C: Counter, P: SharedPolicy> TryCloneIn<'a> for Strong<T, C, P> {
    type Output = Self;
    fn try_clone_in(&self, _: Budget<'a>) -> Result<Self, Error> {
        Self::try_clone(self)
    }
}
impl<'a, T: ?Sized, C: Counter, P: SharedPolicy> TryCloneIn<'a> for Weak<T, C, P> {
    type Output = Self;
    fn try_clone_in(&self, _: Budget<'a>) -> Result<Self, Error> {
        Self::try_clone(self)
    }
}
#[cfg(target_has_atomic = "ptr")]
// SAFETY: atomic counts synchronize payload retirement and allocation release;
// shared T and the allocation receipt must both be safe across threads.
unsafe impl<T: ?Sized + Send + Sync, P: SharedPolicy + Send + Sync> Send
    for Strong<T, count::AtomicCount, P>
{
}
#[cfg(target_has_atomic = "ptr")]
// SAFETY: all shared access yields &T, and all mutations require uniqueness.
unsafe impl<T: ?Sized + Send + Sync, P: SharedPolicy + Send + Sync> Sync
    for Strong<T, count::AtomicCount, P>
{
}
#[cfg(target_has_atomic = "ptr")]
// SAFETY: upgrade can expose T and last weak release accesses its receipt.
unsafe impl<T: ?Sized + Send + Sync, P: SharedPolicy + Send + Sync> Send
    for Weak<T, count::AtomicCount, P>
{
}
#[cfg(target_has_atomic = "ptr")]
// SAFETY: only atomic header operations are accessible through shared weak refs.
unsafe impl<T: ?Sized + Send + Sync, P: SharedPolicy + Send + Sync> Sync
    for Weak<T, count::AtomicCount, P>
{
}
