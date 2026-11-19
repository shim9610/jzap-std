//! Actual Result ABI wrappers for assembly inspection and scalar-call timing.
#![cfg_attr(feature = "link-proof", no_std)]
use jzap::{ArithmeticError, Checked, DivisionError64, TryArithmetic, WrappingArithmetic};

macro_rules! operations {
    ($plain:ident, $native:ident, $fallible:ident, $operator:tt, $checked:ident, $try:ident) => {
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $plain(a: usize, b: usize) -> usize { a $operator b }

        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $native(a: usize, b: usize) -> Result<usize, ArithmeticError> {
            a.$checked(b).ok_or(ArithmeticError::Overflow)
        }

        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $fallible(a: usize, b: usize) -> Result<usize, ArithmeticError> {
            a.$try(b)
        }
    };
}
operations!(plain_add, native_add, jzap_add, +, checked_add, try_add);
operations!(plain_sub, native_sub, jzap_sub, -, checked_sub, try_sub);
operations!(plain_mul, native_mul, jzap_mul, *, checked_mul, try_mul);

#[unsafe(no_mangle)]
#[inline(never)]
pub fn plain_div(a: usize, b: usize) -> usize {
    a / b
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn native_div(a: usize, b: usize) -> Result<usize, DivisionError64> {
    if b == 0 {
        Err(DivisionError64::DivisionByZero)
    } else {
        a.checked_div(b).ok_or(DivisionError64::Overflow)
    }
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_div(a: usize, b: usize) -> Result<usize, DivisionError64> {
    a.try_div(b)
}

macro_rules! wrapping_operations {
    ($native:ident, $wrapper:ident, $method:ident, $intrinsic:ident) => {
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $native(a: usize, b: usize) -> usize {
            a.$intrinsic(b)
        }
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $wrapper(a: usize, b: usize) -> usize {
            a.$method(b)
        }
    };
}
wrapping_operations!(native_wrap_add, jzap_wrap_add, wrap_add, wrapping_add);
wrapping_operations!(native_wrap_sub, jzap_wrap_sub, wrap_sub, wrapping_sub);
wrapping_operations!(native_wrap_mul, jzap_wrap_mul, wrap_mul, wrapping_mul);
#[unsafe(no_mangle)]
#[inline(never)]
pub fn native_wrap_div(a: usize, b: usize) -> Result<usize, DivisionError64> {
    if b == 0 {
        Err(DivisionError64::DivisionByZero)
    } else {
        Ok(a.wrapping_div(b))
    }
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_wrap_div(a: usize, b: usize) -> Result<usize, DivisionError64> {
    a.try_wrap_div(b)
}

macro_rules! bounded {
    ($plain:ident, $fallible:ident, $operator:tt, $try:ident, $minimum:expr) => {
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $plain(a: usize, b: usize) -> usize {
            let a = (a & 0xffff) | $minimum;
            let b = b & 0xff;
            a $operator b
        }
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $fallible(a: usize, b: usize) -> usize {
            let a = (a & 0xffff) | $minimum;
            let b = b & 0xff;
            a.$try(b).unwrap_or(0)
        }
    };
}
bounded!(bounded_plain_add, bounded_jzap_add, +, try_add, 0);
bounded!(bounded_plain_sub, bounded_jzap_sub, -, try_sub, 0x10000);
bounded!(bounded_plain_mul, bounded_jzap_mul, *, try_mul, 0);

#[unsafe(no_mangle)]
pub extern "C" fn audit_panic_control(value: usize) -> usize {
    assert!(value == 0, "deliberate arithmetic link control");
    value
}

#[cfg(feature = "link-proof")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    unsafe extern "C" {
        fn jzap_arithmetic_panic_reachable() -> !;
    }
    // Deliberately unresolved by the positive final-link fixture.
    unsafe { jzap_arithmetic_panic_reachable() }
}

#[repr(C)]
pub struct Value {
    value: usize,
    failed: usize,
}

trait FailureCode {
    fn failure_code(self) -> usize;
}
impl FailureCode for ArithmeticError {
    fn failure_code(self) -> usize {
        1
    }
}
impl FailureCode for DivisionError64 {
    fn failure_code(self) -> usize {
        match self {
            Self::Overflow => 1,
            Self::DivisionByZero => 2,
        }
    }
}

macro_rules! audit {
    ($name:ident, $method:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(a: usize, b: usize) -> Value {
            match a.$method(b) {
                Ok(value) => Value { value, failed: 0 },
                Err(error) => Value {
                    value: 0,
                    failed: error.failure_code(),
                },
            }
        }
    };
}
audit!(audit_add, try_add);
audit!(audit_sub, try_sub);
audit!(audit_mul, try_mul);

audit!(audit_div, try_div);
audit!(audit_wrap_div, try_wrap_div);
#[unsafe(no_mangle)]
pub extern "C" fn audit_wrap_add(a: usize, b: usize) -> usize {
    a.wrap_add(b)
}
#[unsafe(no_mangle)]
pub extern "C" fn audit_wrap_sub(a: usize, b: usize) -> usize {
    a.wrap_sub(b)
}
#[unsafe(no_mangle)]
pub extern "C" fn audit_wrap_mul(a: usize, b: usize) -> usize {
    a.wrap_mul(b)
}
macro_rules! signed_division {
    ($name:ident, $method:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(a: isize, b: isize) -> Value {
            match a.$method(b) {
                Ok(value) => Value {
                    value: value as usize,
                    failed: 0,
                },
                Err(error) => Value {
                    value: 0,
                    failed: error.failure_code(),
                },
            }
        }
    };
}
signed_division!(audit_signed_div, try_div);
signed_division!(audit_signed_wrap_div, try_wrap_div);

#[unsafe(no_mangle)]
#[inline(never)]
pub fn native_rem(a: usize, b: usize) -> Result<usize, DivisionError64> {
    if b == 0 {
        Err(DivisionError64::DivisionByZero)
    } else {
        a.checked_rem(b).ok_or(DivisionError64::Overflow)
    }
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_rem(a: usize, b: usize) -> Result<usize, DivisionError64> {
    a.try_rem(b)
}
macro_rules! checked_type_operation {
    ($name:ident, $operator:tt, $error:ty) => {
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $name(a: usize, b: usize) -> Result<Checked<usize>, $error> {
            Checked::new(a) $operator Checked::new(b)
        }
    };
}
checked_type_operation!(checked_type_add, +, ArithmeticError);
checked_type_operation!(checked_type_sub, -, ArithmeticError);
checked_type_operation!(checked_type_mul, *, ArithmeticError);
checked_type_operation!(checked_type_div, /, DivisionError64);
checked_type_operation!(checked_type_rem, %, DivisionError64);
audit!(audit_rem, try_rem);
audit!(audit_wrap_rem, try_wrap_rem);
macro_rules! audit_checked_type {
    ($name:ident, $operation:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(a: usize, b: usize) -> Value {
            match $operation(a, b) {
                Ok(value) => Value {
                    value: value.get(),
                    failed: 0,
                },
                Err(error) => Value {
                    value: 0,
                    failed: error.failure_code(),
                },
            }
        }
    };
}
audit_checked_type!(audit_count_add, checked_type_add);
audit_checked_type!(audit_count_sub, checked_type_sub);
audit_checked_type!(audit_count_mul, checked_type_mul);
audit_checked_type!(audit_count_div, checked_type_div);
audit_checked_type!(audit_count_rem, checked_type_rem);
