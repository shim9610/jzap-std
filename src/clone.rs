//! Explicit field-by-field cloning. Adapted from the sources in reference/.
use crate::{Error, budget::Budget, error::InitError, init::InitSlot};
#[cfg(not(verus_keep_ghost))]
mod geometry;
#[cfg(verus_keep_ghost)]
pub(crate) mod geometry;
#[cfg(kani)]
#[path = "clone/proofs.rs"]
mod proofs;
/// Evidence that `T` permits bitwise duplication. The private field prevents
/// safe construction for non-Copy values. This is a memory-safety capability;
/// an opting-in TryClone implementation also promises identical clone semantics.
pub struct CopyWitness<T> {
    marker: core::marker::PhantomData<fn() -> T>,
}
impl<T> CopyWitness<T> {
    /// Compose the capability: an array of bitwise-clonable elements has the
    /// same property. This retains the element's opt-in clone semantics.
    #[inline]
    pub const fn for_array<const N: usize>(self) -> CopyWitness<[T; N]> {
        CopyWitness {
            marker: core::marker::PhantomData,
        }
    }
}
impl<T: Copy> CopyWitness<T> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            marker: core::marker::PhantomData,
        }
    }
}
impl<T: Copy> Default for CopyWitness<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Fallible clone using the source's budget policy. No blanket Clone delegation.
///
/// ```compile_fail
/// use jzap::TryClone;
/// fn requires<T: TryClone>() {}
/// requires::<std::string::String>();
/// ```
pub trait TryClone: Sized {
    fn try_clone(&self) -> Result<Self, Error>;
    /// Optional bitwise clone capability. Safe callers cannot create this token
    /// for a non-Copy type. Returning Some opts into clones without callbacks;
    /// None retains ordinary fallible per-element cloning.
    #[doc(hidden)]
    #[inline]
    fn copy_witness() -> Option<CopyWitness<Self>> {
        None
    }

    /// Build the replacement before dropping the destination. A returned error
    /// preserves the old destination; source cloning determines new nested policies.
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        let replacement = source.try_clone()?;
        *self = replacement;
        Ok(())
    }
    /// Optional allocation-free replacement for a complete equal-length slice.
    /// `None` leaves the destination unchanged and requests the transactional
    /// fallback. `Some(Err(_))` must also preserve it. This hook lets known Copy
    /// implementations reuse storage without specialization or TypeId guesses.
    #[doc(hidden)]
    fn try_copy_slice_from(
        _source: &[Self],
        _destination: &mut [Self],
    ) -> Option<Result<(), Error>> {
        None
    }
    /// Optional bulk append using the destination's allocation and length policy.
    /// `None` leaves it untouched and selects per-element cloning. `Some(Ok(()))`
    /// appends exactly the source; `Some(Err(_))` preserves its initialized prefix
    /// and length (capacity may grow). Implementations must match `try_clone`.
    /// Safe Vec operations maintain initialization and memory bounds even for a
    /// user implementation; callback success is never used to publish raw slots.
    #[cfg(feature = "alloc")]
    #[doc(hidden)]
    #[inline]
    fn try_extend_vec<P: crate::budget::AllocationPolicy, L: crate::vec::LengthPolicy>(
        _source: &[Self],
        _destination: &mut crate::vec::Vec<Self, P, L>,
    ) -> Option<Result<(), Error>> {
        None
    }
    /// Clone into existing storage; arrays override this to avoid stack temporaries.
    fn try_clone_into(&self, slot: &mut InitSlot<'_, Self>) -> Result<(), Error> {
        if slot.get().is_some() {
            return Err(Error::AlreadyInitialized);
        }
        slot.write(self.try_clone()?)
            .map_err(|rejected| rejected.error)
    }
}
/// Clone all newly owned storage into `budget`, including nested owned fields.
/// Output carries the target lifetime rather than retaining the source budget.
pub trait TryCloneIn<'a> {
    type Output;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error>;
    fn try_clone_into_in(
        &self,
        budget: Budget<'a>,
        slot: &mut InitSlot<'_, Self::Output>,
    ) -> Result<(), Error> {
        if slot.get().is_some() {
            return Err(Error::AlreadyInitialized);
        }
        slot.write(self.try_clone_in(budget)?)
            .map_err(|rejected| rejected.error)
    }
}
macro_rules! scalar {($($t:ty),*$(,)?)=>{$(
 impl TryClone for $t {
 #[inline]fn copy_witness()->Option<CopyWitness<Self>>{Some(CopyWitness::new())}
 #[inline]fn try_clone(&self)->Result<Self,Error>{Ok(*self)}
 #[inline]fn try_copy_slice_from(source:&[Self],destination:&mut[Self])->Option<Result<(),Error>>{Some(try_copy_slice(source,destination))}
 #[cfg(feature="alloc")]
 #[inline]fn try_extend_vec<P:crate::budget::AllocationPolicy,L:crate::vec::LengthPolicy>(source:&[Self],destination:&mut crate::vec::Vec<Self,P,L>)->Option<Result<(),Error>>{Some(destination.try_extend_copy(source))}
 }
 impl<'a> TryCloneIn<'a> for $t {type Output=Self;#[inline]fn try_clone_in(&self,_:Budget<'a>)->Result<Self,Error>{Ok(*self)}}
)*};}
scalar!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64
);
impl<T: ?Sized> TryClone for &T {
    #[inline]
    fn copy_witness() -> Option<CopyWitness<Self>> {
        Some(CopyWitness::new())
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn try_extend_vec<P: crate::budget::AllocationPolicy, L: crate::vec::LengthPolicy>(
        source: &[Self],
        destination: &mut crate::vec::Vec<Self, P, L>,
    ) -> Option<Result<(), Error>> {
        Some(destination.try_extend_copy(source))
    }
    fn try_copy_slice_from(source: &[Self], destination: &mut [Self]) -> Option<Result<(), Error>> {
        Some(try_copy_slice(source, destination))
    }
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(*self)
    }
}
impl<'a, 's, T: ?Sized> TryCloneIn<'a> for &'s T {
    type Output = &'s T;
    fn try_clone_in(&self, _: Budget<'a>) -> Result<Self::Output, Error> {
        Ok(*self)
    }
}
impl<T: TryClone> TryClone for Option<T> {
    fn try_clone(&self) -> Result<Self, Error> {
        self.as_ref().map(TryClone::try_clone).transpose()
    }
}
impl<'a, T: TryCloneIn<'a>> TryCloneIn<'a> for Option<T> {
    type Output = Option<T::Output>;
    fn try_clone_in(&self, b: Budget<'a>) -> Result<Self::Output, Error> {
        self.as_ref().map(|x| x.try_clone_in(b)).transpose()
    }
}
impl<T: TryClone, E: TryClone> TryClone for Result<T, E> {
    fn try_clone(&self) -> Result<Self, Error> {
        match self {
            Ok(v) => Ok(Ok(v.try_clone()?)),
            Err(e) => Ok(Err(e.try_clone()?)),
        }
    }
}
impl<'a, T: TryCloneIn<'a>, E: TryCloneIn<'a>> TryCloneIn<'a> for Result<T, E> {
    type Output = Result<T::Output, E::Output>;
    fn try_clone_in(&self, b: Budget<'a>) -> Result<Self::Output, Error> {
        match self {
            Ok(v) => Ok(Ok(v.try_clone_in(b)?)),
            Err(e) => Ok(Err(e.try_clone_in(b)?)),
        }
    }
}
macro_rules! tuple {($($t:ident:$i:tt),+)=>{
 impl<$($t:TryClone),+> TryClone for ($($t,)+){fn try_clone(&self)->Result<Self,Error>{Ok(($(self.$i.try_clone()?,)+))}}
 impl<'a,$($t:TryCloneIn<'a>),+> TryCloneIn<'a> for ($($t,)+){type Output=($($t::Output,)+);fn try_clone_in(&self,b:Budget<'a>)->Result<Self::Output,Error>{Ok(($(self.$i.try_clone_in(b)?,)+))}}
};}
tuple!(A:0);
tuple!(A:0,B:1);
tuple!(A:0,B:1,C:2);
tuple!(A:0,B:1,C:2,D:3);
tuple!(A:0,B:1,C:2,D:3,E:4);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7,I:8);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7,I:8,J:9);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7,I:8,J:9,K:10);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7,I:8,J:9,K:10,L:11);
pub(crate) fn array_from_fn<T, const N: usize>(
    mut f: impl FnMut(usize) -> Result<T, Error>,
) -> Result<[T; N], Error> {
    let mut storage = core::mem::MaybeUninit::<[T; N]>::uninit();
    let ptr = core::ptr::NonNull::from(&mut storage).cast::<T>();
    let mut initialize = |index, slot: &mut crate::init::InitSlot<'_, T>| {
        slot.write(f(index)?).map(|_| ()).map_err(|e| e.error)
    };
    // SAFETY: local storage exclusively owns all N unwritten element slots.
    unsafe { crate::init::prefix::initialize(ptr, N, &mut initialize) }.map_err(|e| match e {
        crate::error::InitError::Storage(e) | crate::error::InitError::Initialize(e) => e,
    })?;
    // SAFETY: every element is initialized and the helper transferred ownership.
    Ok(unsafe { storage.assume_init() })
}
impl<T: TryClone, const N: usize> TryClone for [T; N] {
    #[inline]
    fn copy_witness() -> Option<CopyWitness<Self>> {
        T::copy_witness().map(CopyWitness::for_array)
    }
    fn try_copy_slice_from(source: &[Self], destination: &mut [Self]) -> Option<Result<(), Error>> {
        let len = match geometry::flatten_count(source.len(), destination.len(), N) {
            Ok(len) => len,
            Err(error) => return Some(Err(error)),
        };
        // SAFETY: arrays have contiguous T layout and alignment. Original slices
        // establish storage bounds, checked multiplication also handles ZST counts.
        // The two borrows remain disjoint and preserve their original lifetimes.
        let (source, destination) = unsafe {
            (
                core::slice::from_raw_parts(source.as_ptr().cast::<T>(), len),
                core::slice::from_raw_parts_mut(destination.as_mut_ptr().cast::<T>(), len),
            )
        };
        T::try_copy_slice_from(source, destination)
    }
    fn try_clone(&self) -> Result<Self, Error> {
        array_from_fn(|i| {
            // SAFETY: array_from_fn supplies only indices in 0..N.
            unsafe { self.get_unchecked(i) }.try_clone()
        })
    }
    fn try_clone_into(&self, slot: &mut InitSlot<'_, Self>) -> Result<(), Error> {
        slot.try_init_array(|i, element| {
            // SAFETY: try_init_array calls only for i in 0..N.
            unsafe { self.get_unchecked(i) }.try_clone_into(element)
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
impl<'a, T: TryCloneIn<'a>, const N: usize> TryCloneIn<'a> for [T; N] {
    type Output = [T::Output; N];
    fn try_clone_in(&self, b: Budget<'a>) -> Result<Self::Output, Error> {
        array_from_fn(|i| {
            // SAFETY: array_from_fn supplies only indices in 0..N.
            unsafe { self.get_unchecked(i) }.try_clone_in(b)
        })
    }
    fn try_clone_into_in(
        &self,
        budget: Budget<'a>,
        slot: &mut InitSlot<'_, Self::Output>,
    ) -> Result<(), Error> {
        slot.try_init_array(|i, element| {
            // SAFETY: try_init_array calls only for i in 0..N.
            unsafe { self.get_unchecked(i) }.try_clone_into_in(budget, element)
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
/// Explicit Copy bulk path. Length mismatch leaves destination unchanged.
#[inline]
pub fn try_copy_slice<T: Copy>(source: &[T], destination: &mut [T]) -> Result<(), Error> {
    if source.len() != destination.len() {
        return Err(Error::Bounds);
    }
    // SAFETY: references guarantee valid, disjoint regions; equal lengths checked.
    unsafe {
        core::ptr::copy_nonoverlapping(source.as_ptr(), destination.as_mut_ptr(), source.len());
    }
    Ok(())
}

// Reverse changes ordering only; fallible cloning retains the inner contract.
impl<T: TryClone> TryClone for core::cmp::Reverse<T> {
    fn try_clone(&self) -> Result<Self, Error> {
        self.0.try_clone().map(Self)
    }
}
impl<'a, T: TryCloneIn<'a>> TryCloneIn<'a> for core::cmp::Reverse<T> {
    type Output = core::cmp::Reverse<T::Output>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        self.0.try_clone_in(budget).map(core::cmp::Reverse)
    }
}
