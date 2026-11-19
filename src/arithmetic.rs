//! Integer arithmetic with explicit failure or explicitly permitted wrapping.
//!
//! ```
//! use jzap::{ArithmeticError, DivisionError, TryArithmetic, WrappingArithmetic};
//! assert_eq!(20usize.try_add(22), Ok(42));
//! assert_eq!(usize::MAX.try_add(1), Err(ArithmeticError::Overflow));
//! assert_eq!(0usize.try_sub(1), Err(ArithmeticError::Overflow));
//! assert_eq!(usize::MAX.try_mul(2), Err(ArithmeticError::Overflow));
//! assert_eq!(usize::MAX.wrap_add(1), 0);
//! assert_eq!(i8::MIN.try_wrap_div(-1), Ok(i8::MIN));
//! // 64-bit integers use `DivisionError64`; `From` converts to the common error.
//! assert_eq!(42usize.try_div(0).map_err(DivisionError::from), Err(DivisionError::DivisionByZero));
//! ```
//!
//! `Overflow` includes results below the type's minimum, including unsigned
//! subtraction below zero. `TryArithmetic` never wraps on error.
//! `WrappingArithmetic` explicitly permits modular results. Neither API allocates
//! or depends on the consumer's overflow-check compiler setting.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// An integer operation cannot produce a result under the selected contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, vstd::prelude::verus_verify)]
pub enum ArithmeticError {
    /// The result is outside the operand type's range.
    Overflow,
}

impl core::fmt::Display for ArithmeticError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("integer arithmetic overflow")
    }
}
impl core::error::Error for ArithmeticError {}

/// Division cannot produce a result under the selected contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, vstd::prelude::verus_verify)]
pub enum DivisionError {
    /// Signed MIN / -1 is outside the operand type's range.
    Overflow,
    /// Integer division has a zero divisor.
    DivisionByZero,
}
impl core::fmt::Display for DivisionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Overflow => "integer division overflow",
            Self::DivisionByZero => "integer division by zero",
        })
    }
}
impl core::error::Error for DivisionError {}

/// Division error for 64-bit integer types (`u64`, `i64`, `usize`, `isize` on
/// 64-bit targets).
///
/// It has the same variants and meaning as [`DivisionError`]. Its 64-bit
/// representation matches the quotient, so a non-inlined
/// `Result<u64, DivisionError64>` is returned in registers like
/// `Option<u64>`. Convert it with `DivisionError::from` or `?` where one common
/// error type is needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, vstd::prelude::verus_verify)]
#[repr(u64)]
pub enum DivisionError64 {
    Overflow,
    DivisionByZero,
}
impl core::fmt::Display for DivisionError64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&DivisionError::from(*self), f)
    }
}
impl core::error::Error for DivisionError64 {}
impl From<DivisionError64> for DivisionError {
    #[inline]
    fn from(error: DivisionError64) -> Self {
        match error {
            DivisionError64::Overflow => Self::Overflow,
            DivisionError64::DivisionByZero => Self::DivisionByZero,
        }
    }
}

pub(crate) mod bounded;
mod checked;
pub use checked::Checked;

mod sealed {
    #[cfg(verus_keep_ghost)]
    use vstd::prelude::*;
    #[cfg(not(verus_keep_ghost))]
    pub trait Sealed {}
    // Numeric vocabulary for the sealed integer implementations; erased by Cargo.
    #[cfg(verus_keep_ghost)]
    vstd::prelude::verus! {
        pub trait Sealed: Copy {
            spec fn numeric_value(self) -> int;
            spec fn numeric_min() -> int;
            spec fn numeric_max() -> int;
            spec fn numeric_from_int(value: int) -> Self;
            proof fn numeric_identity(self)
                ensures Self::numeric_from_int(self.numeric_value()) == self,
                    Self::numeric_min() <= self.numeric_value() <= Self::numeric_max();
        }
    }
}

/// Fallible addition, subtraction, multiplication and division on primitive integers.
///
/// Implemented for every signed and unsigned primitive integer. Sealing keeps
/// the implementation and its failure behavior within this library's control.
/// Failure is independent of the consumer's overflow-check compiler setting.
#[cfg_attr(verus_keep_ghost, vstd::prelude::verus_verify)]
pub trait TryArithmetic: sealed::Sealed + Copy {
    /// Division and remainder error: [`DivisionError64`] for 64-bit types,
    /// otherwise [`DivisionError`]. Both convert into [`DivisionError`].
    type DivisionError: Copy + Eq + core::fmt::Debug + Into<DivisionError>;
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(value) => value.numeric_value() == self.numeric_value() + rhs.numeric_value(),
            Err(_) => (self.numeric_value() + rhs.numeric_value()) < Self::numeric_min()
                || (self.numeric_value() + rhs.numeric_value()) > Self::numeric_max(),
        },
    ))]
    fn try_add(self, rhs: Self) -> Result<Self, ArithmeticError>;
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(value) => value.numeric_value() == self.numeric_value() - rhs.numeric_value(),
            Err(_) => (self.numeric_value() - rhs.numeric_value()) < Self::numeric_min()
                || (self.numeric_value() - rhs.numeric_value()) > Self::numeric_max(),
        },
    ))]
    fn try_sub(self, rhs: Self) -> Result<Self, ArithmeticError>;
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(value) => value.numeric_value() == self.numeric_value() * rhs.numeric_value(),
            Err(_) => (self.numeric_value() * rhs.numeric_value()) < Self::numeric_min()
                || (self.numeric_value() * rhs.numeric_value()) > Self::numeric_max(),
        },
    ))]
    fn try_mul(self, rhs: Self) -> Result<Self, ArithmeticError>;
    /// Truncates toward zero. Zero divisors and signed `MIN / -1` return errors.
    fn try_div(self, rhs: Self) -> Result<Self, <Self as TryArithmetic>::DivisionError>;
    /// Remainder follows the dividend's sign. Zero and signed MIN % -1 are errors.
    fn try_rem(self, rhs: Self) -> Result<Self, <Self as TryArithmetic>::DivisionError>;
}

/// Explicit wrapping arithmetic on primitive integers.
///
/// Addition, subtraction and multiplication return the low bits modulo `2^BITS`,
/// interpreted in the operand type. They cannot fail and do not check overflow.
/// Division truncates toward zero; signed `MIN / -1` returns `MIN`. A zero
/// divisor still returns an error, so division is named `try_wrap_div`.
/// These operations never mutate their operands or allocate.
pub trait WrappingArithmetic: sealed::Sealed + Copy {
    /// Zero-divisor error of the wrapping division methods; the same type as
    /// [`TryArithmetic::DivisionError`] for each integer.
    type DivisionError: Copy + Eq + core::fmt::Debug + Into<DivisionError>;
    fn wrap_add(self, rhs: Self) -> Self;
    fn wrap_sub(self, rhs: Self) -> Self;
    fn wrap_mul(self, rhs: Self) -> Self;
    fn try_wrap_div(self, rhs: Self) -> Result<Self, <Self as WrappingArithmetic>::DivisionError>;
    /// Signed MIN % -1 is zero; a zero divisor returns an error.
    fn try_wrap_rem(self, rhs: Self) -> Result<Self, <Self as WrappingArithmetic>::DivisionError>;
}

macro_rules! integers {
    ($(($ty:ty, $err:ident)),+ $(,)?) => {$(
        #[cfg(not(verus_keep_ghost))]
        impl sealed::Sealed for $ty {}
        #[cfg(verus_keep_ghost)]
        verus! {
            impl sealed::Sealed for $ty {
                open spec fn numeric_value(self) -> int { self as int }
                open spec fn numeric_min() -> int { <$ty>::MIN as int }
                open spec fn numeric_max() -> int { <$ty>::MAX as int }
                open spec fn numeric_from_int(value: int) -> Self { value as $ty }
                proof fn numeric_identity(self) {}
            }
        }
        #[cfg_attr(verus_keep_ghost, vstd::prelude::verus_verify)]
        impl TryArithmetic for $ty {
            type DivisionError = $err;
            #[cfg_attr(verus_keep_ghost, vstd::prelude::verus_spec(out =>
                ensures match out {
                    Ok(value) => value as int == self as int + rhs as int,
                    Err(_) => (self as int + rhs as int) < (<$ty>::MIN as int)
                        || (self as int + rhs as int) > (<$ty>::MAX as int),
                },
            ))]
            #[inline]
            fn try_add(self, rhs: Self) -> Result<Self, ArithmeticError> {
                self.checked_add(rhs).ok_or(ArithmeticError::Overflow)
            }
            #[cfg_attr(verus_keep_ghost, vstd::prelude::verus_spec(out =>
                ensures match out {
                    Ok(value) => value as int == self as int - rhs as int,
                    Err(_) => (self as int - rhs as int) < (<$ty>::MIN as int)
                        || (self as int - rhs as int) > (<$ty>::MAX as int),
                },
            ))]
            #[inline]
            fn try_sub(self, rhs: Self) -> Result<Self, ArithmeticError> {
                self.checked_sub(rhs).ok_or(ArithmeticError::Overflow)
            }
            #[cfg_attr(verus_keep_ghost, vstd::prelude::verus_spec(out =>
                ensures match out {
                    Ok(value) => value as int == self as int * rhs as int,
                    Err(_) => (self as int * rhs as int) < (<$ty>::MIN as int)
                        || (self as int * rhs as int) > (<$ty>::MAX as int),
                },
            ))]
            #[inline]
            fn try_mul(self, rhs: Self) -> Result<Self, ArithmeticError> {
                self.checked_mul(rhs).ok_or(ArithmeticError::Overflow)
            }
            #[inline]
            fn try_div(self, rhs: Self) -> Result<Self, $err> {
                if rhs == 0 {
                    Err($err::DivisionByZero)
                } else {
                    self.checked_div(rhs).ok_or($err::Overflow)
                }
            }
            #[inline]
            fn try_rem(self, rhs: Self) -> Result<Self, $err> {
                if rhs == 0 {
                    Err($err::DivisionByZero)
                } else {
                    self.checked_rem(rhs).ok_or($err::Overflow)
                }
            }
        }
        impl WrappingArithmetic for $ty {
            type DivisionError = $err;
            #[inline]
            fn wrap_add(self, rhs: Self) -> Self {
                self.wrapping_add(rhs)
            }
            #[inline]
            fn wrap_sub(self, rhs: Self) -> Self {
                self.wrapping_sub(rhs)
            }
            #[inline]
            fn wrap_mul(self, rhs: Self) -> Self {
                self.wrapping_mul(rhs)
            }
            #[inline]
            fn try_wrap_div(self, rhs: Self) -> Result<Self, $err> {
                if rhs == 0 {
                    Err($err::DivisionByZero)
                } else {
                    // The only nonzero-divisor failure is signed MIN / -1.
                    // checked_div avoids wrapping_div's redundant panic path
                    // in unoptimized builds as well.
                    Ok(self.checked_div(rhs).unwrap_or(Self::MIN))
                }
            }
            #[inline]
            fn try_wrap_rem(self, rhs: Self) -> Result<Self, $err> {
                if rhs == 0 {
                    Err($err::DivisionByZero)
                } else {
                    Ok(self.checked_rem(rhs).unwrap_or(0))
                }
            }
        }
    )+};
}
// 64-bit quotients use a 64-bit error so their Result keeps the register ABI.
#[cfg(target_pointer_width = "64")]
integers!(
    (u8, DivisionError),
    (u16, DivisionError),
    (u32, DivisionError),
    (u64, DivisionError64),
    (u128, DivisionError),
    (usize, DivisionError64),
    (i8, DivisionError),
    (i16, DivisionError),
    (i32, DivisionError),
    (i64, DivisionError64),
    (i128, DivisionError),
    (isize, DivisionError64)
);
#[cfg(not(target_pointer_width = "64"))]
integers!(
    (u8, DivisionError),
    (u16, DivisionError),
    (u32, DivisionError),
    (u64, DivisionError64),
    (u128, DivisionError),
    (usize, DivisionError),
    (i8, DivisionError),
    (i16, DivisionError),
    (i32, DivisionError),
    (i64, DivisionError64),
    (i128, DivisionError),
    (isize, DivisionError)
);

#[cfg(kani)]
#[path = "arithmetic_proofs.rs"]
mod proofs;
