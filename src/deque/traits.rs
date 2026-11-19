use super::*;
use crate::{TryIntoIterator, TryIterator, error::InitError};
use core::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};
impl<T> Default for VecDeque<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> crate::TryDefault for VecDeque<T> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
impl<T: fmt::Debug, P: AllocationPolicy, L: LengthPolicy> fmt::Debug for VecDeque<T, P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy, Q: AllocationPolicy, M: LengthPolicy>
    PartialEq<VecDeque<U, Q, M>> for VecDeque<T, P, L>
{
    fn eq(&self, other: &VecDeque<U, Q, M>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<T: Eq, P: AllocationPolicy, L: LengthPolicy> Eq for VecDeque<T, P, L> {}
macro_rules! slice_equality {
    ([$($extra:tt)*] $rhs:ty) => {
        impl<T:PartialEq<U>,U,P:AllocationPolicy,L:LengthPolicy,$($extra)*> PartialEq<$rhs> for VecDeque<T,P,L>{
            fn eq(&self,other:&$rhs)->bool{self.len()==other.len()&&self.iter().eq(other.iter())}
        }
    }
}
slice_equality!([] & [U]);
slice_equality!([] &mut [U]);
slice_equality!([const N:usize] [U;N]);
slice_equality!([const N:usize] &[U;N]);
slice_equality!([const N:usize] &mut [U;N]);
slice_equality!([] alloc::vec::Vec<U>);
impl<T: PartialEq<U>, U, P: AllocationPolicy, L: LengthPolicy, Q: AllocationPolicy, M: LengthPolicy>
    PartialEq<Vec<U, Q, M>> for VecDeque<T, P, L>
{
    fn eq(&self, other: &Vec<U, Q, M>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<T: PartialOrd, P: AllocationPolicy, L: LengthPolicy> PartialOrd for VecDeque<T, P, L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}
impl<T: Ord, P: AllocationPolicy, L: LengthPolicy> Ord for VecDeque<T, P, L> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}
impl<T: Hash, P: AllocationPolicy, L: LengthPolicy> Hash for VecDeque<T, P, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for v in self {
            v.hash(state)
        }
    }
}
impl<T: TryClone, P: AllocationPolicy, L: LengthPolicy> VecDeque<T, P, L> {
    #[inline]
    fn clone_to(&self, policy: P, limit: L) -> Result<Self, Error> {
        crate::vec::required_len(0, self.len(), limit.max_len())?;
        let count = if mem::size_of::<T>() == 0 {
            limit.max_len()
        } else {
            self.len()
        };
        let (cap, layout) = Capacity::try_new(count)?;
        let ptr = storage::allocate(layout, policy.budget())?.cast();
        // Construct after allocation. The exact cap/head/length remain visible to
        // LLVM rather than escaping through an opaque grow(&mut owner) call.
        let mut result = Self {
            ptr,
            cap,
            head: Checked::new(0),
            len: Checked::new(0),
            policy,
            limit,
            marker: PhantomData,
        };
        let (a, b) = self.as_slices();
        if T::copy_witness().is_some() {
            // SAFETY: capability permits bitwise duplication, and exact storage was admitted.
            unsafe {
                result.write_clone_slice(0, a);
                result.write_clone_slice(a.len(), b)
            };
            result.len = Checked::new(self.len());
            Ok(result)
        } else {
            // Reuse Vec's tracked in-place initialization, including large arrays and
            // fallible nested fields, without materializing a Result<T> temporary.
            let mut values = result.into_vec();
            values.try_extend_cloned(a)?;
            values.try_extend_cloned(b)?;
            Ok(Self::from_vec(values))
        }
    }
}
impl<T: TryClone, P: AllocationPolicy, L: LengthPolicy> TryClone for VecDeque<T, P, L> {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        self.clone_to(self.policy.try_clone()?, self.limit)
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        crate::vec::required_len(0, source.len(), self.max_len())?;
        if source.is_empty() {
            self.clear();
            return Ok(());
        }
        if T::copy_witness().is_some() && !mem::needs_drop::<T>() {
            if source.len() > self.capacity() {
                self.try_reserve(sub(source.len(), self.len()))?;
            }
            // All failures precede retirement. These old values need no drop;
            // the capability permits duplicating the source into admitted storage.
            let (a, b) = source.as_slices();
            self.len = Checked::new(0);
            self.head = Checked::new(0);
            // SAFETY: distinct owners and two live source segments; destination
            // capacity covers their combined length, with no intervening failure.
            unsafe {
                self.write_clone_slice(0, a);
                self.write_clone_slice(a.len(), b);
            }
            self.len = Checked::new(source.len());
            return Ok(());
        }
        let mut replacement = source.clone_to(self.policy.try_clone()?, self.limit)?;
        mem::swap(self, &mut replacement);
        Ok(())
    }
}
impl<'a, T: TryCloneIn<'a>, P: AllocationPolicy, L: LengthPolicy> TryCloneIn<'a>
    for VecDeque<T, P, L>
{
    type Output = VecDeque<T::Output, Budget<'a>, L>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        let mut values = Vec::try_from_array_in([], budget, self.limit).map_err(|r| r.error)?;
        values
            .try_resize_with_init(self.len(), |i, slot| {
                self.get(i)?.try_clone_into_in(budget, slot)
            })
            .map_err(|e| match e {
                InitError::Storage(e) | InitError::Initialize(e) => e,
            })?;
        Ok(VecDeque::from_vec(values))
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ExtendError<T, E> {
    Source(E),
    Push(Rejected<T>),
}
impl<T, P: AllocationPolicy, L: LengthPolicy> VecDeque<T, P, L> {
    unsafe fn write_clone_slice(&mut self, index: usize, source: &[T]) {
        // SAFETY: caller reserved full prefix and has evidence of bitwise clonability; fresh head zero.
        unsafe {
            ptr::copy_nonoverlapping(source.as_ptr(), self.ptr.as_ptr().add(index), source.len())
        }
    }
    /// Earlier accepted items remain; a refused owned item is returned and the source stops.
    pub fn try_extend<I: TryIntoIterator<Item = T>>(
        &mut self,
        source: I,
    ) -> Result<(), ExtendError<T, I::Error>> {
        let mut source = source.try_into_iter().map_err(ExtendError::Source)?;
        while let Some(value) = source.try_next().map_err(ExtendError::Source)? {
            self.try_push_back(value).map_err(ExtendError::Push)?
        }
        Ok(())
    }
    pub fn try_extend_copy(&mut self, values: &[T]) -> Result<(), Error>
    where
        T: Copy,
    {
        if values.is_empty() {
            return Ok(());
        }
        let len = self.len();
        self.try_reserve(values.len())?;
        if len == 0 {
            self.head = Checked::new(0);
        }
        // SAFETY: Copy source and admitted storage. An empty destination may
        // normalize its head after admission and use one contiguous copy.
        unsafe {
            if len == 0 {
                self.write_clone_slice(0, values);
            } else {
                self.write_slice(len, values);
            }
        }
        self.len = Checked::new(add(len, values.len()));
        Ok(())
    }
    /// Callback/storage errors discard this call's new suffix, preserving the original prefix.
    pub fn try_resize_with<E>(
        &mut self,
        new_len: usize,
        mut f: impl FnMut() -> Result<T, E>,
    ) -> Result<(), InitError<E>> {
        if new_len <= self.len() {
            self.truncate(new_len);
            return Ok(());
        }
        self.try_reserve(sub(new_len, self.len()))?;
        struct Rollback<'a, T, P: AllocationPolicy, L: LengthPolicy> {
            owner: &'a mut VecDeque<T, P, L>,
            original: usize,
        }
        impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Rollback<'_, T, P, L> {
            fn drop(&mut self) {
                self.owner.truncate(self.original)
            }
        }
        let original = self.len();
        let guard = Rollback {
            owner: self,
            original,
        };
        while guard.owner.len() < new_len {
            let value = f().map_err(InitError::Initialize)?;
            guard
                .owner
                .try_push_back(value)
                .map_err(|r| InitError::Storage(r.error))?
        }
        mem::forget(guard);
        Ok(())
    }
    pub fn try_resize(&mut self, new_len: usize, value: &T) -> Result<(), Error>
    where
        T: TryClone,
    {
        if new_len <= self.len() {
            self.truncate(new_len);
            return Ok(());
        }
        if T::copy_witness().is_some() {
            let extra = sub(new_len, self.len());
            self.try_reserve(extra)?;
            if mem::size_of::<T>() != 0 {
                let tail = self.physical(self.len());
                let (first, second) = geometry::split(tail, extra, self.capacity());
                let base = self.ptr.as_ptr();
                for i in 0..first {
                    // SAFETY: capability permits duplication; split bounds this initialized write
                    // to reserved spare storage, disjoint from the immutable external source.
                    unsafe { ptr::copy_nonoverlapping(value, base.add(add(tail, i)), 1) }
                }
                for i in 0..second {
                    // SAFETY: remaining spare prefix; no active element is overwritten.
                    unsafe { ptr::copy_nonoverlapping(value, base.add(i), 1) }
                }
            }
            self.len = Checked::new(new_len);
            return Ok(());
        }
        self.try_resize_with(new_len, || value.try_clone())
            .map_err(|e| match e {
                InitError::Storage(e) | InitError::Initialize(e) => e,
            })
    }
}
impl<T, P: AllocationPolicy> VecDeque<T, P> {
    pub fn try_from_array_in<const N: usize>(
        values: [T; N],
        policy: P,
    ) -> Result<Self, Rejected<[T; N]>> {
        Vec::try_from_array_in(values, policy, Unlimited).map(Self::from_vec)
    }
    pub fn try_from_iter_in<I: TryIntoIterator<Item = T>>(
        source: I,
        policy: P,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        let mut value = Self::new_in(policy);
        value.try_extend(source)?;
        Ok(value)
    }
}
impl<T> VecDeque<T> {
    pub fn try_from_array<const N: usize>(values: [T; N]) -> Result<Self, Rejected<[T; N]>> {
        Self::try_from_array_in(values, Unmetered)
    }
    pub fn try_from_iter<I: TryIntoIterator<Item = T>>(
        source: I,
    ) -> Result<Self, ExtendError<T, I::Error>> {
        Self::try_from_iter_in(source, Unmetered)
    }
}
