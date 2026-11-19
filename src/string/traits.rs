use super::*;
use core::{
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

impl Default for String {
    fn default() -> Self {
        Self::new()
    }
}
impl crate::TryDefault for String {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> TryClone for String<P, L> {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self {
            bytes: self.bytes.try_clone()?,
        })
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        self.try_replace_range(.., source.as_str())
    }
}
impl<'a, P: AllocationPolicy, L: LengthPolicy> TryCloneIn<'a> for String<P, L> {
    type Output = String<Budget<'a>, L>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        Ok(String {
            bytes: self.bytes.try_clone_in(budget)?,
        })
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> fmt::Debug for String<P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> fmt::Display for String<P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> AsRef<str> for String<P, L> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> AsMut<str> for String<P, L> {
    fn as_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> AsRef<[u8]> for String<P, L> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> Borrow<str> for String<P, L> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> BorrowMut<str> for String<P, L> {
    fn borrow_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> Hash for String<P, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
impl<P: AllocationPolicy, L: LengthPolicy, Q: AllocationPolicy, M: LengthPolicy>
    PartialEq<String<Q, M>> for String<P, L>
{
    fn eq(&self, other: &String<Q, M>) -> bool {
        self.as_str() == other.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> PartialEq<str> for String<P, L> {
    fn eq(&self, s: &str) -> bool {
        self.as_str() == s
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> PartialEq<&str> for String<P, L> {
    fn eq(&self, s: &&str) -> bool {
        self.as_str() == *s
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> Eq for String<P, L> {}
impl<P: AllocationPolicy, L: LengthPolicy> PartialEq<String<P, L>> for str {
    fn eq(&self, other: &String<P, L>) -> bool {
        self == other.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> PartialEq<String<P, L>> for &str {
    fn eq(&self, other: &String<P, L>) -> bool {
        *self == other.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> PartialEq<alloc::string::String> for String<P, L> {
    fn eq(&self, other: &alloc::string::String) -> bool {
        self.as_str() == other.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> PartialEq<String<P, L>> for alloc::string::String {
    fn eq(&self, other: &String<P, L>) -> bool {
        self.as_str() == other.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> PartialOrd for String<P, L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> Ord for String<P, L> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}
