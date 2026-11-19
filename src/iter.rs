//! Open, fallible counterparts of the standard iterator traits.
//!
//! Implementors must return supported failures through `Error`, including failures
//! in callbacks and conversion. A `Result` signature is a contract, not a compiler
//! proof: it does not turn a panic, abort, or nonterminating implementation into an
//! error. Check concrete consumer roots with the link-audit infrastructure; see
//! `docs/iteration.md`. Standard interoperability is explicit through `from_std`
//! and `from_results` and retains the source implementation's behavior.
//!
//! ```
//! use jzap::{TryIterator, vec::Vec};
//! # fn main() -> Result<(), jzap::error::InitError<core::convert::Infallible>> {
//! struct One(Option<u8>);
//! impl TryIterator for One {
//!     type Item = u8;
//!     type Error = core::convert::Infallible;
//!     fn try_next(&mut self) -> Result<Option<u8>, Self::Error> {
//!         Ok(self.0.take())
//!     }
//! }
//! let value = Vec::try_from_fallible_iter(One(Some(7)))?;
//! assert_eq!(value.as_slice(), &[7]);
//! # Ok(())
//! # }
//! ```
use crate::Checked;
use core::convert::Infallible;
mod collections;
#[cfg(kani)]
mod proofs;

/// Distinguishes failure to obtain an item from failure to process that item.
#[derive(Debug, PartialEq, Eq)]
pub enum TryFoldError<S, F> {
    Source(S),
    Operation(F),
}

/// An iterator with an explicit failure channel, open to user implementations.
///
/// `Ok(Some(item))` yields, `Ok(None)` ends this traversal, and `Err(error)`
/// reports failure. Unless a stronger contract is documented, neither an error
/// nor `None` promises that a subsequent call is exhausted. Consumers stop on
/// the first error. `try_size_hint` is only a hint, never a safety obligation.
/// Implementors must document consumption/state on error and obey the module's
/// failure contract, including any callbacks and destructors they invoke.
///
/// Ordinary iterators need an explicit interoperability bridge:
/// ```compile_fail
/// let _ = jzap::vec::Vec::<u64>::try_from_fallible_iter(0u64..3);
/// ```
pub trait TryIterator {
    type Item;
    type Error;
    fn try_next(&mut self) -> Result<Option<Self::Item>, Self::Error>;
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Self::Error> {
        Ok((0, None))
    }
    #[inline]
    fn try_nth(&mut self, n: usize) -> Result<Option<Self::Item>, Self::Error> {
        let mut remaining = Checked::new(n);
        while let Ok(next) = remaining - Checked::new(1) {
            if self.try_next()?.is_none() {
                return Ok(None);
            }
            remaining = next;
        }
        self.try_next()
    }
    #[inline]
    fn try_fold<B, E, F>(
        &mut self,
        mut value: B,
        mut f: F,
    ) -> Result<B, TryFoldError<Self::Error, E>>
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> Result<B, E>,
    {
        while let Some(item) = self.try_next().map_err(TryFoldError::Source)? {
            value = f(value, item).map_err(TryFoldError::Operation)?;
        }
        Ok(value)
    }
    #[inline]
    fn try_for_each<E, F>(&mut self, mut f: F) -> Result<(), TryFoldError<Self::Error, E>>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Result<(), E>,
    {
        self.try_fold((), |(), item| f(item))
    }
    #[inline]
    fn try_by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
    /// Limit calls that can yield items; errors also consume one attempt.
    #[inline]
    fn try_take(self, count: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take {
            iter: self,
            remaining: Checked::new(count),
        }
    }
}

/// Conversion may fail before iteration starts. No blanket `IntoIterator` bridge.
pub trait TryIntoIterator {
    type Item;
    type Error;
    type IntoIter: TryIterator<Item = Self::Item, Error = Self::Error>;
    fn try_into_iter(self) -> Result<Self::IntoIter, Self::Error>;
}
impl<I: TryIterator> TryIntoIterator for I {
    type Item = I::Item;
    type Error = I::Error;
    type IntoIter = I;
    #[inline]
    fn try_into_iter(self) -> Result<I, I::Error> {
        Ok(self)
    }
}

pub trait TryDoubleEndedIterator: TryIterator {
    fn try_next_back(&mut self) -> Result<Option<Self::Item>, Self::Error>;
    #[inline]
    fn try_nth_back(&mut self, n: usize) -> Result<Option<Self::Item>, Self::Error> {
        let mut remaining = Checked::new(n);
        while let Ok(next) = remaining - Checked::new(1) {
            if self.try_next_back()?.is_none() {
                return Ok(None);
            }
            remaining = next;
        }
        self.try_next_back()
    }
}
pub trait TryExactSizeIterator: TryIterator {
    fn try_len(&self) -> Result<usize, Self::Error>;
    #[inline]
    fn try_is_empty(&self) -> Result<bool, Self::Error> {
        Ok(self.try_len()? == 0)
    }
}
/// After `Ok(None)`, both ends (if present) keep returning `Ok(None)`.
/// This does not promise exhaustion after an error.
pub trait TryFusedIterator: TryIterator {}

impl<I: TryIterator + ?Sized> TryIterator for &mut I {
    type Item = I::Item;
    type Error = I::Error;
    #[inline]
    fn try_next(&mut self) -> Result<Option<I::Item>, I::Error> {
        (**self).try_next()
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), I::Error> {
        (**self).try_size_hint()
    }
    #[inline]
    fn try_nth(&mut self, n: usize) -> Result<Option<I::Item>, I::Error> {
        (**self).try_nth(n)
    }
}
impl<I: TryDoubleEndedIterator + ?Sized> TryDoubleEndedIterator for &mut I {
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<I::Item>, I::Error> {
        (**self).try_next_back()
    }
}
impl<I: TryExactSizeIterator + ?Sized> TryExactSizeIterator for &mut I {
    #[inline]
    fn try_len(&self) -> Result<usize, I::Error> {
        (**self).try_len()
    }
}
impl<I: TryFusedIterator + ?Sized> TryFusedIterator for &mut I {}

#[derive(Debug)]
pub struct Take<I> {
    iter: I,
    remaining: Checked<usize>,
}
impl<I: TryIterator> TryIterator for Take<I> {
    type Item = I::Item;
    type Error = I::Error;
    #[inline]
    fn try_next(&mut self) -> Result<Option<I::Item>, I::Error> {
        let Ok(next) = self.remaining - Checked::new(1) else {
            return Ok(None);
        };
        self.remaining = next;
        self.iter.try_next()
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), I::Error> {
        let remaining = self.remaining.get();
        if remaining == 0 {
            return Ok((0, Some(0)));
        }
        let (low, high) = self.iter.try_size_hint()?;
        Ok((
            low.min(remaining),
            Some(high.unwrap_or(remaining).min(remaining)),
        ))
    }
}
impl<I: TryFusedIterator> TryFusedIterator for Take<I> {}

/// Explicit interoperability adapter. It adds no checks inside the std iterator.
#[derive(Debug)]
pub struct FromStd<I> {
    iter: I,
}
#[inline]
pub fn from_std<I: IntoIterator>(source: I) -> FromStd<I::IntoIter> {
    FromStd {
        iter: source.into_iter(),
    }
}
impl<I: Iterator> TryIterator for FromStd<I> {
    type Item = I::Item;
    type Error = Infallible;
    #[inline]
    fn try_next(&mut self) -> Result<Option<I::Item>, Infallible> {
        Ok(self.iter.next())
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Infallible> {
        Ok(self.iter.size_hint())
    }
    #[inline]
    fn try_nth(&mut self, n: usize) -> Result<Option<I::Item>, Infallible> {
        Ok(self.iter.nth(n))
    }
    #[inline]
    fn try_fold<B, E, F>(&mut self, value: B, mut f: F) -> Result<B, TryFoldError<Infallible, E>>
    where
        F: FnMut(B, I::Item) -> Result<B, E>,
    {
        self.iter
            .try_fold(value, |b, item| f(b, item).map_err(TryFoldError::Operation))
    }
}
impl<I: DoubleEndedIterator> TryDoubleEndedIterator for FromStd<I> {
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<I::Item>, Infallible> {
        Ok(self.iter.next_back())
    }
}
impl<I: ExactSizeIterator> TryExactSizeIterator for FromStd<I> {
    #[inline]
    fn try_len(&self) -> Result<usize, Infallible> {
        Ok(self.iter.len())
    }
}
impl<I: core::iter::FusedIterator> TryFusedIterator for FromStd<I> {}

/// Explicitly promote result-valued std items into the iteration error channel.
#[derive(Debug)]
pub struct FromResults<I> {
    iter: I,
}
#[inline]
pub fn from_results<T, E, I: IntoIterator<Item = Result<T, E>>>(
    source: I,
) -> FromResults<I::IntoIter> {
    FromResults {
        iter: source.into_iter(),
    }
}
impl<T, E, I: Iterator<Item = Result<T, E>>> TryIterator for FromResults<I> {
    type Item = T;
    type Error = E;
    #[inline]
    fn try_next(&mut self) -> Result<Option<T>, E> {
        self.iter.next().transpose()
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), E> {
        Ok(self.iter.size_hint())
    }
    #[inline]
    fn try_fold<B, O, F>(&mut self, value: B, mut f: F) -> Result<B, TryFoldError<E, O>>
    where
        F: FnMut(B, T) -> Result<B, O>,
    {
        self.iter.try_fold(value, |b, item| {
            f(b, item.map_err(TryFoldError::Source)?).map_err(TryFoldError::Operation)
        })
    }
}
impl<T, E, I: DoubleEndedIterator<Item = Result<T, E>>> TryDoubleEndedIterator for FromResults<I> {
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<T>, E> {
        self.iter.next_back().transpose()
    }
}
impl<T, E, I: core::iter::FusedIterator<Item = Result<T, E>>> TryFusedIterator for FromResults<I> {}
