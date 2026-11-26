use super::*;
#[cfg(all(kani, jzap_loop_contracts))]
#[path = "ownership_proofs.rs"]
mod panic_proofs;
impl<T, P: AllocationPolicy, L: LengthPolicy> Vec<T, P, L> {
    /// Exact storage/policy transfer. Recover only with the matching unsafe contract.
    pub fn into_raw_parts(self) -> (NonNull<T>, usize, usize, P, L) {
        let owner = core::mem::ManuallyDrop::new(self);
        (
            owner.ptr,
            owner.extent.len(),
            owner.extent.cap(),
            // SAFETY: the disarmed owner transfers its policy exactly once.
            unsafe { core::ptr::read(&owner.policy) },
            owner.extent.limit(),
        )
    }
    /// # Safety
    /// Unique globally allocated storage for capacity elements, first len initialized;
    /// valid array layout, len <= capacity <= limit, correct existing budget charge.
    /// For ZSTs, use an aligned non-null pointer and capacity equal to the limit.
    pub unsafe fn from_raw_parts(
        ptr: NonNull<T>,
        len: usize,
        capacity: usize,
        policy: P,
        limit: L,
    ) -> Self {
        Self {
            ptr,
            // SAFETY: the caller's contract is the contract of both trusted
            // entries: a valid array layout and len <= capacity <= limit.
            extent: unsafe {
                crate::extent::Extent::from_raw_parts(
                    len,
                    crate::capacity::Capacity::from_validated_raw(capacity),
                    limit,
                )
            },
            policy,
            marker: PhantomData,
        }
    }
    /// Adopt standard global storage; oversize capacity is rejected without shrinking.
    /// No inner payload budgets are changed by moving an existing value.
    pub fn try_from_std_in(
        mut source: alloc::vec::Vec<T>,
        policy: P,
        limit: L,
    ) -> Result<Self, Rejected<alloc::vec::Vec<T>>> {
        let capacity = match crate::capacity::adopted_capacity(
            mem::size_of::<T>(),
            source.len(),
            source.capacity(),
            limit.max_len(),
        ) {
            Ok(capacity) => capacity,
            Err(error) => {
                return Err(Rejected {
                    value: source,
                    error,
                });
            }
        };
        let (capacity, layout) = match crate::capacity::Capacity::try_new(capacity) {
            Ok(layout) => layout,
            Err(error) => {
                return Err(Rejected {
                    value: source,
                    error,
                });
            }
        };
        // Admitted before the accounting and the transfer: a refusal returns
        // the source unchanged.
        let extent = match crate::extent::Extent::try_new(source.len(), capacity, limit) {
            Ok(extent) => extent,
            Err(error) => {
                return Err(Rejected {
                    value: source,
                    error,
                });
            }
        };
        let ptr = {
            let reservation = match policy.budget().reserve(layout.size()) {
                Ok(reservation) => reservation,
                Err(error) => {
                    return Err(Rejected {
                        value: source,
                        error,
                    });
                }
            };
            // SAFETY: Vec::as_mut_ptr is non-null/aligned, including empty/ZST vectors.
            let ptr = unsafe { NonNull::new_unchecked(source.as_mut_ptr()) };
            if let Some(r) = reservation {
                r.commit().retain();
            }
            ptr
        };
        mem::forget(source);
        Ok(Self {
            ptr,
            extent,
            policy,
            marker: PhantomData,
        })
    }
    /// Explicitly end jzap's accounting and element-limit contract without copying.
    pub fn into_std(self) -> alloc::vec::Vec<T> {
        let bytes = self.layout().size();
        let (ptr, len, cap, policy, _) = self.into_raw_parts();
        policy.budget().release(bytes);
        // SAFETY: same global allocation/layout and initialized prefix; ZST cap is ignored by std.
        unsafe { alloc::vec::Vec::from_raw_parts(ptr.as_ptr(), len, cap) }
    }
    /// Shrink fallibly to an exact Box allocation. Refusal returns this Vec unchanged.
    /// The resulting Box has fixed length and carries the same allocation policy.
    pub fn try_into_boxed_slice(mut self) -> Result<crate::boxed::Box<[T], P>, Rejected<Self>> {
        if let Err(error) = self.try_shrink_to_fit() {
            return Err(Rejected { value: self, error });
        }
        let (ptr, len, _, policy, _) = self.into_raw_parts();
        // SAFETY: exact length-sized allocation (or ZST), all elements initialized, charge transferred.
        Ok(unsafe {
            crate::boxed::Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr, len), policy)
        })
    }
}
impl<T, P: AllocationPolicy> Vec<T, P> {
    pub fn from_boxed_slice(source: crate::boxed::Box<[T], P>) -> Self {
        let len = source.len();
        let capacity = crate::capacity::Capacity::for_slice(source.as_slice(), usize::MAX);
        let (ptr, policy) = crate::boxed::Box::into_raw_parts(source);
        Self {
            ptr: ptr.cast(),
            // The slice capacity holds the slice length, so nothing is clamped.
            extent: crate::extent::Extent::unlimited_clamped(len, capacity),
            policy,
            marker: PhantomData,
        }
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for &'a Vec<T, P, L> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for &'a mut Vec<T, P, L> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
pub struct IntoIter<T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    owner: Vec<T, P, L>,
    span: crate::span::Span,
}
impl<T, P: AllocationPolicy, L: LengthPolicy> IntoIterator for Vec<T, P, L> {
    type Item = T;
    type IntoIter = IntoIter<T, P, L>;
    fn into_iter(mut self) -> Self::IntoIter {
        let back = self.extent.len();
        self.extent.shorten_to(0);
        IntoIter {
            owner: self,
            span: crate::span::Span::full(back),
        }
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> IntoIter<T, P, L> {
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: front..back is precisely the remaining initialized range.
        unsafe {
            core::slice::from_raw_parts(
                self.owner.ptr.as_ptr().add(self.span.front()),
                self.span.len(),
            )
        }
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Iterator for IntoIter<T, P, L> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let index = self.span.next()?;
        // SAFETY: the span removes exactly one initialized index before transfer.
        Some(unsafe { self.owner.ptr.as_ptr().add(index).read() })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.span.len();
        (len, Some(len))
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> DoubleEndedIterator for IntoIter<T, P, L> {
    fn next_back(&mut self) -> Option<T> {
        let index = self.span.next_back()?;
        // SAFETY: the span removes exactly one initialized index before transfer.
        Some(unsafe { self.owner.ptr.as_ptr().add(index).read() })
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> ExactSizeIterator for IntoIter<T, P, L> {
    #[inline]
    fn len(&self) -> usize {
        self.size_hint().0
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> core::iter::FusedIterator for IntoIter<T, P, L> {}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for IntoIter<T, P, L> {
    fn drop(&mut self) {
        let remaining = self.span.len();
        if !mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: only unconsumed elements remain initialized; owner.len=0. The
        // owner field releases storage even if dropping this suffix unwinds.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.owner.ptr.as_ptr().add(self.span.front()),
                remaining,
            ))
        };
    }
}

impl<T, P: AllocationPolicy, L: LengthPolicy> IntoIter<T, P, L> {
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: exclusive borrow of precisely the remaining initialized range.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.owner.ptr.as_ptr().add(self.span.front()),
                self.span.len(),
            )
        }
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> AsRef<[T]> for IntoIter<T, P, L> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> AsMut<[T]> for IntoIter<T, P, L> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: core::fmt::Debug, P: AllocationPolicy, L: LengthPolicy> core::fmt::Debug
    for IntoIter<T, P, L>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}
impl<T: TryClone, P: AllocationPolicy, L: LengthPolicy> TryClone for IntoIter<T, P, L> {
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        let mut replacement = Vec::try_from_slice_in(
            source.as_slice(),
            self.owner.policy.try_clone()?,
            self.owner.extent.limit(),
        )?
        .into_iter();
        mem::swap(self, &mut replacement);
        Ok(())
    }
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Vec::try_from_slice_in(
            self.as_slice(),
            self.owner.policy.try_clone()?,
            self.owner.extent.limit(),
        )?
        .into_iter())
    }
}
impl<T> Default for IntoIter<T> {
    fn default() -> Self {
        Vec::new().into_iter()
    }
}

impl<'a, T: TryCloneIn<'a>, P: AllocationPolicy, L: LengthPolicy> TryCloneIn<'a>
    for IntoIter<T, P, L>
{
    type Output = IntoIter<T::Output, Budget<'a>, L>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        let mut copy = Vec::empty(budget, self.owner.extent.limit());
        copy.try_reserve_exact(self.span.len())?;
        copy.try_resize_with_init(self.span.len(), |i, slot| {
            // SAFETY: i covers exactly the remaining initialized iterator range.
            unsafe { self.as_slice().get_unchecked(i) }.try_clone_into_in(budget, slot)
        })
        .map_err(flatten_error)?;
        Ok(copy.into_iter())
    }
}
