use super::*;
use crate::{TryIntoIterator, TryIterator, error::InitError};
use core::{fmt, ptr::NonNull, str::Utf8Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Utf16Error {
    Storage(Error),
    UnpairedSurrogate(u16),
}
impl From<Error> for Utf16Error {
    fn from(e: Error) -> Self {
        Self::Storage(e)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatError {
    Storage(Error),
    Formatter,
}
impl fmt::Display for Utf16Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(e) => e.fmt(f),
            Self::UnpairedSurrogate(_) => f.write_str("unpaired UTF-16 surrogate"),
        }
    }
}
impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(e) => e.fmt(f),
            Self::Formatter => f.write_str("formatter failed"),
        }
    }
}
impl core::error::Error for Utf16Error {}
impl core::error::Error for FormatError {}

/// Borrow valid input without allocation; own only repaired text.
#[derive(Debug)]
pub enum Utf8Lossy<'a, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    Borrowed(&'a str),
    Owned(String<P, L>),
}
impl<P: AllocationPolicy, L: LengthPolicy> Utf8Lossy<'_, P, L> {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(s) => s,
            Self::Owned(s) => s.as_str(),
        }
    }
    pub fn try_to_owned_in<Q: AllocationPolicy, M: LengthPolicy>(
        &self,
        policy: Q,
        limit: M,
    ) -> Result<String<Q, M>, Error> {
        String::try_from_str_in(self.as_str(), policy, limit)
    }
}

impl String {
    pub fn try_from_char(ch: char) -> Result<Self, Error> {
        Self::try_from_str(ch.encode_utf8(&mut [0; 4]))
    }
    #[cfg(not(no_global_oom_handling))]
    pub fn try_from_cow(
        source: alloc::borrow::Cow<'_, str>,
    ) -> Result<Self, Rejected<alloc::borrow::Cow<'_, str>>> {
        Self::try_from_cow_in(source, Unmetered, Unlimited)
    }
    pub fn try_from_utf8_lossy(source: &[u8]) -> Result<Utf8Lossy<'_>, Error> {
        Self::try_from_utf8_lossy_in(source, Unmetered, Unlimited)
    }
    pub fn try_from_utf16(source: &[u16]) -> Result<Self, Utf16Error> {
        Self::try_from_utf16_in(source, Unmetered, Unlimited)
    }
    pub fn try_from_utf16_lossy(source: &[u16]) -> Result<Self, Error> {
        Self::try_from_utf16_lossy_in(source, Unmetered, Unlimited)
    }
    pub fn try_from_chars(source: impl IntoIterator<Item = char>) -> Result<Self, Error> {
        let mut result = Self::new();
        result.try_extend_chars(source)?;
        Ok(result)
    }
    pub fn try_from_fallible_chars<I: TryIntoIterator<Item = char>>(
        source: I,
    ) -> Result<Self, InitError<I::Error>> {
        let mut result = Self::new();
        result.try_extend_fallible_chars(source)?;
        Ok(result)
    }
    pub fn try_from_texts<S: AsRef<str>>(
        source: impl IntoIterator<Item = S>,
    ) -> Result<Self, Error> {
        let mut result = Self::new();
        result.try_extend_texts(source)?;
        Ok(result)
    }
    pub fn try_format(args: fmt::Arguments<'_>) -> Result<Self, FormatError> {
        let mut result = Self::new();
        result.try_write_fmt(args)?;
        Ok(result)
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> String<P, L> {
    #[cfg(not(no_global_oom_handling))]
    pub fn try_from_cow_in(
        source: alloc::borrow::Cow<'_, str>,
        policy: P,
        limit: L,
    ) -> Result<Self, Rejected<alloc::borrow::Cow<'_, str>>> {
        use alloc::borrow::Cow;
        match source {
            Cow::Borrowed(text) => {
                Self::try_from_str_in(text, policy, limit).map_err(|error| Rejected {
                    value: Cow::Borrowed(text),
                    error,
                })
            }
            Cow::Owned(text) => Self::try_from_std_in(text, policy, limit).map_err(|e| Rejected {
                value: Cow::Owned(e.value),
                error: e.error,
            }),
        }
    }
    pub fn try_from_utf8(bytes: Vec<u8, P, L>) -> Result<Self, Rejected<Vec<u8, P, L>, Utf8Error>> {
        if let Err(error) = core::str::from_utf8(bytes.as_slice()) {
            return Err(Rejected {
                value: bytes,
                error,
            });
        }
        Ok(Self { bytes })
    }
    /// # Safety
    /// The complete initialized byte prefix must be valid UTF-8.
    pub unsafe fn from_utf8_unchecked(bytes: Vec<u8, P, L>) -> Self {
        Self { bytes }
    }
    pub fn try_from_utf8_lossy_in(
        source: &[u8],
        policy: P,
        limit: L,
    ) -> Result<Utf8Lossy<'_, P, L>, Error> {
        let mut chunks = source.utf8_chunks();
        let Some(first) = chunks.next() else {
            return Ok(Utf8Lossy::Borrowed(""));
        };
        if first.invalid().is_empty() {
            // No owner is created, so the selected owner limit does not constrain
            // a borrowed result. It is checked if an owned copy is requested.
            return Ok(Utf8Lossy::Borrowed(first.valid()));
        }
        let mut result = Self::try_from_str_in("", policy, limit)?;
        result.try_reserve_exact(source.len())?;
        result.try_push_str(first.valid())?;
        result.try_push('\u{fffd}')?;
        for chunk in chunks {
            result.try_push_str(chunk.valid())?;
            if !chunk.invalid().is_empty() {
                result.try_push('\u{fffd}')?;
            }
        }
        Ok(Utf8Lossy::Owned(result))
    }
    pub fn try_from_utf16_in(source: &[u16], policy: P, limit: L) -> Result<Self, Utf16Error> {
        let mut result = Self::try_from_str_in("", policy, limit)?;
        result.try_reserve_exact(source.len().min(result.max_len()))?;
        // Do not reject based on UTF-16 unit count alone: the byte ceiling is
        // enforced by actual complete UTF-8 scalar appends.
        for ch in core::char::decode_utf16(source.iter().copied()) {
            let ch = ch.map_err(|e| Utf16Error::UnpairedSurrogate(e.unpaired_surrogate()))?;
            result.try_push(ch)?;
        }
        Ok(result)
    }
    pub fn try_from_utf16_lossy_in(source: &[u16], policy: P, limit: L) -> Result<Self, Error> {
        let mut result = Self::try_from_str_in("", policy, limit)?;
        result.try_reserve_exact(source.len().min(result.max_len()))?;
        for ch in core::char::decode_utf16(source.iter().copied()) {
            result.try_push(ch.unwrap_or(core::char::REPLACEMENT_CHARACTER))?;
        }
        Ok(result)
    }
    /// Explicitly end jzap budget/ceiling tracking, without copying storage.
    pub fn into_std(self) -> alloc::string::String {
        // SAFETY: String invariant; Vec transfers the existing global allocation.
        unsafe { alloc::string::String::from_utf8_unchecked(self.bytes.into_std()) }
    }
    pub fn try_from_std_in(
        source: alloc::string::String,
        policy: P,
        limit: L,
    ) -> Result<Self, Rejected<alloc::string::String>> {
        match Vec::try_from_std_in(source.into_bytes(), policy, limit) {
            Ok(bytes) => Ok(Self { bytes }),
            Err(e) => Err(Rejected {
                // SAFETY: rejected bytes came from a String; adoption did not modify them.
                value: unsafe { alloc::string::String::from_utf8_unchecked(e.value) },
                error: e.error,
            }),
        }
    }
    pub fn into_raw_parts(self) -> (NonNull<u8>, usize, usize, P, L) {
        self.bytes.into_raw_parts()
    }
    /// # Safety
    /// All Vec::from_raw_parts allocation, initialized-prefix, limit and budget
    /// requirements apply. The initialized prefix must additionally be valid UTF-8.
    pub unsafe fn from_raw_parts(
        ptr: NonNull<u8>,
        len: usize,
        cap: usize,
        policy: P,
        limit: L,
    ) -> Self {
        Self {
            bytes: unsafe { Vec::from_raw_parts(ptr, len, cap, policy, limit) },
        }
    }
    /// # Safety
    /// Restore valid UTF-8 before this exclusive borrow ends or any String API is
    /// used, including unwinding. Preserve the selected policy and byte ceiling.
    pub unsafe fn as_mut_vec(&mut self) -> &mut Vec<u8, P, L> {
        &mut self.bytes
    }
    pub fn try_into_boxed_str(self) -> Result<crate::boxed::Box<str, P>, Rejected<Self>> {
        match self.bytes.try_into_boxed_slice() {
            // SAFETY: conversion preserves all initialized valid UTF-8 bytes.
            Ok(bytes) => Ok(unsafe { crate::boxed::Box::from_utf8_unchecked(bytes) }),
            Err(e) => Err(Rejected {
                value: Self { bytes: e.value },
                error: e.error,
            }),
        }
    }
    pub fn leak<'a>(self) -> &'a mut str
    where
        P: 'a,
        L: 'a,
    {
        // SAFETY: initialized valid UTF-8; leaked storage retains its budget charge.
        unsafe { core::str::from_utf8_unchecked_mut(self.bytes.leak()) }
    }
    pub fn try_write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), FormatError> {
        // Vec's guarded writer appends only complete str chunks and restores
        // original length on every error/unwind, including ignored sink errors.
        self.bytes.try_write_fmt(args).map_err(|e| match e {
            Error::InvalidStructure => FormatError::Formatter,
            e => FormatError::Storage(e),
        })
    }
    pub fn try_extend_fallible_chars<I: TryIntoIterator<Item = char>>(
        &mut self,
        source: I,
    ) -> Result<(), InitError<I::Error>> {
        let mut source = source.try_into_iter().map_err(InitError::Initialize)?;
        let original = self.len();
        let guard = Append {
            owner: self,
            original,
        };
        while let Some(ch) = source.try_next().map_err(InitError::Initialize)? {
            guard.owner.try_push(ch)?;
        }
        core::mem::forget(guard);
        Ok(())
    }
    pub fn try_extend_chars(
        &mut self,
        source: impl IntoIterator<Item = char>,
    ) -> Result<(), Error> {
        self.try_extend_fallible_chars(crate::iter::from_std(source))
            .map_err(infallible)
    }
    pub fn try_extend_fallible_texts<S: AsRef<str>, I: TryIntoIterator<Item = S>>(
        &mut self,
        source: I,
    ) -> Result<(), InitError<I::Error>> {
        let mut source = source.try_into_iter().map_err(InitError::Initialize)?;
        let original = self.len();
        let guard = Append {
            owner: self,
            original,
        };
        while let Some(text) = source.try_next().map_err(InitError::Initialize)? {
            guard.owner.try_push_str(text.as_ref())?;
        }
        core::mem::forget(guard);
        Ok(())
    }
    pub fn try_extend_texts<S: AsRef<str>>(
        &mut self,
        source: impl IntoIterator<Item = S>,
    ) -> Result<(), Error> {
        self.try_extend_fallible_texts(crate::iter::from_std(source))
            .map_err(infallible)
    }
}
impl<P: AllocationPolicy> String<P> {
    pub fn from_boxed_str(source: crate::boxed::Box<str, P>) -> Self {
        Self {
            bytes: Vec::from_boxed_slice(source.into_bytes()),
        }
    }
}
struct Append<'a, P: AllocationPolicy, L: LengthPolicy> {
    owner: &'a mut String<P, L>,
    original: usize,
}
impl<P: AllocationPolicy, L: LengthPolicy> Drop for Append<'_, P, L> {
    fn drop(&mut self) {
        self.owner.bytes.truncate(self.original);
    }
}
fn infallible(e: InitError<core::convert::Infallible>) -> Error {
    match e {
        InitError::Storage(e) => e,
        InitError::Initialize(never) => match never {},
    }
}
