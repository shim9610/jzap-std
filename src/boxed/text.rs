//! Explicit textual conversions and iteration without infallible collection.
use super::*;
use crate::arithmetic::bounded::{wrapping_add, wrapping_sub};
use crate::error::InitError;
use crate::{Error, TryIterator};
#[cfg(not(no_global_oom_handling))]
use alloc::borrow::Cow;
use alloc::string::String;
#[cfg(all(kani, jzap_loop_contracts))]
#[path = "text_proofs.rs"]
mod panic_proofs;

struct CharBytes<I> {
    iter: I,
    bytes: [u8; 4],
    position: usize,
    len: usize,
}
impl<I: TryIterator<Item = char>> TryIterator for CharBytes<I> {
    type Item = u8;
    type Error = I::Error;
    fn try_next(&mut self) -> Result<Option<u8>, I::Error> {
        if self.position == self.len {
            let Some(character) = self.iter.try_next()? else {
                return Ok(None);
            };
            self.len = character.encode_utf8(&mut self.bytes).len();
            self.position = 0;
        }
        let Some(byte) = self.bytes.get(self.position).copied() else {
            return Ok(None);
        };
        self.position = wrapping_add(self.position, 1);
        Ok(Some(byte))
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), I::Error> {
        let remaining = wrapping_sub(self.len, self.position);
        let (low, high) = self.iter.try_size_hint()?;
        Ok((
            low.saturating_add(remaining),
            high.and_then(|h| h.checked_mul(4))
                .and_then(|h| h.checked_add(remaining)),
        ))
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: Clone + crate::TryClone, P: AllocationPolicy> Box<[T], P> {
    #[cfg(not(no_global_oom_handling))]
    pub fn try_from_cow_in<'a>(
        value: Cow<'a, [T]>,
        policy: P,
    ) -> Result<Self, Rejected<Cow<'a, [T]>>> {
        match value {
            Cow::Borrowed(source) => {
                Self::try_clone_from_slice_in(source, policy).map_err(|error| Rejected {
                    value: Cow::Borrowed(source),
                    error,
                })
            }
            Cow::Owned(source) => Self::try_from_std_vec_in(source, policy).map_err(|e| Rejected {
                value: Cow::Owned(e.value),
                error: e.error,
            }),
        }
    }
}
impl<P: AllocationPolicy> Box<str, P> {
    #[cfg(not(no_global_oom_handling))]
    pub fn try_from_cow_in<'a>(
        value: Cow<'a, str>,
        policy: P,
    ) -> Result<Self, Rejected<Cow<'a, str>>> {
        match value {
            Cow::Borrowed(source) => {
                Self::try_from_str_in(source, policy).map_err(|error| Rejected {
                    value: Cow::Borrowed(source),
                    error,
                })
            }
            Cow::Owned(source) => {
                Self::try_from_std_string_in(source, policy).map_err(|e| Rejected {
                    value: Cow::Owned(e.value),
                    error: e.error,
                })
            }
        }
    }
    pub fn try_from_chars_in(
        source: impl IntoIterator<Item = char>,
        policy: P,
    ) -> Result<Self, Error> {
        Self::try_from_fallible_chars_in(crate::iter::from_std(source), policy)
            .map_err(infallible_error)
    }
    pub fn try_from_fallible_chars_in<I: crate::TryIntoIterator<Item = char>>(
        source: I,
        policy: P,
    ) -> Result<Self, InitError<I::Error>> {
        let bytes = Box::<[u8], _>::try_from_fallible_iter_in(
            CharBytes {
                iter: source.try_into_iter().map_err(InitError::Initialize)?,
                bytes: [0; 4],
                position: 0,
                len: 0,
            },
            policy,
        )?;
        // SAFETY: concatenating complete char UTF-8 encodings yields valid UTF-8.
        Ok(unsafe { Self::from_utf8_unchecked(bytes) })
    }
    pub fn try_from_texts_in<S: AsRef<str>>(
        source: impl IntoIterator<Item = S>,
        policy: P,
    ) -> Result<Self, Error> {
        Self::try_from_fallible_texts_in(crate::iter::from_std(source), policy)
            .map_err(infallible_error)
    }
    pub fn try_from_fallible_texts_in<S: AsRef<str>, I: crate::TryIntoIterator<Item = S>>(
        source: I,
        policy: P,
    ) -> Result<Self, InitError<I::Error>> {
        let mut source = source.try_into_iter().map_err(InitError::Initialize)?;
        let mut buffer = super::buffer::Buffer::new(policy);
        while let Some(chunk) = source.try_next().map_err(InitError::Initialize)? {
            // Borrow once: a user's AsRef is a trusted callback boundary.
            buffer.append(chunk.as_ref().as_bytes())?;
        }
        let bytes = buffer.finish()?;
        // SAFETY: complete, individually valid UTF-8 chunks were concatenated.
        Ok(unsafe { Self::from_utf8_unchecked(bytes) })
    }
}

#[derive(Debug)]
struct TextError(String);
impl core::fmt::Display for TextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}
impl core::error::Error for TextError {}
macro_rules! messages { ($($bounds:tt)*) => {
    impl<'a> Box<dyn core::error::Error $($bounds)* + 'a> {
        pub fn try_from_message(source: &str) -> Result<Self, Error> {
            let text = Box::<str>::try_from_str(source)?.into_std_string();
            Self::try_from_owned_message(text).map_err(|e| e.error)
        }
        pub fn try_from_owned_message(source: String) -> Result<Self, Rejected<String>> {
            Box::try_new(TextError(source))
                .map(|boxed| crate::box_unsize!(boxed => dyn core::error::Error $($bounds)* + 'a))
                .map_err(|e| Rejected { value: e.value.0, error: e.error })
        }
    }
}; }
messages!();
messages!(+ Send + Sync);

fn infallible_error(error: InitError<core::convert::Infallible>) -> Error {
    match error {
        InitError::Storage(error) => error,
        InitError::Initialize(never) => match never {},
    }
}
