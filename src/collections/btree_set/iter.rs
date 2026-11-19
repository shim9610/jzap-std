use super::*;
use crate::{TryDoubleEndedIterator, TryExactSizeIterator, TryFusedIterator};
use core::{convert::Infallible, iter::FusedIterator};

pub struct Iter<'a, T>(pub(super) btree_map::Iter<'a, T, ()>);
pub struct Range<'a, T>(pub(super) btree_map::Range<'a, T, ()>);
pub struct IntoIter<T, P: AllocationPolicy = Unmetered>(btree_map::IntoIter<T, (), P>);
pub struct ExtractIf<'a, T, P: AllocationPolicy, F, E>(
    pub(super) btree_map::ExtractIf<'a, T, (), P, F, E>,
);
macro_rules! borrowed {
    ($name:ident) => {
        impl<'a, T> Iterator for $name<'a, T> {
            type Item = &'a T;
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|(k, _)| k)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }
        impl<T> DoubleEndedIterator for $name<'_, T> {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map(|(k, _)| k)
            }
        }
        impl<T> FusedIterator for $name<'_, T> {}
        impl<T> Clone for $name<'_, T> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }
        impl<T> Default for $name<'_, T> {
            fn default() -> Self {
                Self(Default::default())
            }
        }
        impl<T: core::fmt::Debug> core::fmt::Debug for $name<'_, T> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_list().entries(self.clone()).finish()
            }
        }
        impl<T> TryIterator for $name<'_, T> {
            type Item = <Self as Iterator>::Item;
            type Error = Infallible;
            fn try_next(&mut self) -> Result<Option<Self::Item>, Infallible> {
                Ok(self.next())
            }
            fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
                Ok(self.size_hint())
            }
        }
        impl<T> TryDoubleEndedIterator for $name<'_, T> {
            fn try_next_back(&mut self) -> Result<Option<Self::Item>, Infallible> {
                Ok(self.next_back())
            }
        }
        impl<T> TryFusedIterator for $name<'_, T> {}
    };
}
borrowed!(Iter);
borrowed!(Range);
impl<T> ExactSizeIterator for Iter<'_, T> {
    fn len(&self) -> usize {
        self.0.size_hint().0
    }
}
impl<T> TryExactSizeIterator for Iter<'_, T> {
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(self.len())
    }
}
impl<T, P: AllocationPolicy> IntoIterator for BTreeSet<T, P> {
    type Item = T;
    type IntoIter = IntoIter<T, P>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.map.into_iter())
    }
}
impl<'a, T, P: AllocationPolicy> IntoIterator for &'a BTreeSet<T, P> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T, P: AllocationPolicy> IntoIterator for &'a mut BTreeSet<T, P> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T, P: AllocationPolicy> IntoIter<T, P> {
    pub fn as_iter(&self) -> Iter<'_, T> {
        Iter(self.0.as_iter())
    }
}
impl<T, P: AllocationPolicy> Iterator for IntoIter<T, P> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.next().map(|(k, _)| k)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T, P: AllocationPolicy> DoubleEndedIterator for IntoIter<T, P> {
    fn next_back(&mut self) -> Option<T> {
        self.0.next_back().map(|(k, _)| k)
    }
}
impl<T, P: AllocationPolicy> ExactSizeIterator for IntoIter<T, P> {
    fn len(&self) -> usize {
        self.0.size_hint().0
    }
}
impl<T, P: AllocationPolicy> FusedIterator for IntoIter<T, P> {}
impl<T> Default for IntoIter<T> {
    fn default() -> Self {
        BTreeSet::new().into_iter()
    }
}
impl<T: core::fmt::Debug, P: AllocationPolicy> core::fmt::Debug for IntoIter<T, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_iter().fmt(f)
    }
}
impl<T, P: AllocationPolicy> TryIterator for IntoIter<T, P> {
    type Item = T;
    type Error = Infallible;
    fn try_next(&mut self) -> Result<Option<T>, Infallible> {
        Ok(self.next())
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(self.size_hint())
    }
}
impl<T, P: AllocationPolicy> TryDoubleEndedIterator for IntoIter<T, P> {
    fn try_next_back(&mut self) -> Result<Option<T>, Infallible> {
        Ok(self.next_back())
    }
}
impl<T, P: AllocationPolicy> TryExactSizeIterator for IntoIter<T, P> {
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(self.len())
    }
}
impl<T, P: AllocationPolicy> TryFusedIterator for IntoIter<T, P> {}
impl<T, P: AllocationPolicy, E, F: FnMut(&T, &mut ()) -> Result<bool, E>> TryIterator
    for ExtractIf<'_, T, P, F, E>
{
    type Item = T;
    type Error = E;
    fn try_next(&mut self) -> Result<Option<T>, E> {
        self.0.try_next().map(|p| p.map(|(k, _)| k))
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), E> {
        self.0.try_size_hint()
    }
}
impl<T, P: AllocationPolicy, E, F: FnMut(&T, &mut ()) -> Result<bool, E>> TryFusedIterator
    for ExtractIf<'_, T, P, F, E>
{
}
impl<T: core::fmt::Debug, P: AllocationPolicy, F, E> core::fmt::Debug
    for ExtractIf<'_, T, P, F, E>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
