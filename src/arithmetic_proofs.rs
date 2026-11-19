use super::*;

// All bit patterns for every primitive integer, including signed minima and
// full-width multiplication. Native checked operations are the reference
// contract; the actual API must preserve its value/error and remain panic-free.
macro_rules! check_types {
    ($method:ident, $native:ident) => {{
        macro_rules! one {
            ($ty:ty) => {{
                let a: $ty = kani::any();
                let b: $ty = kani::any();
                assert!(a.$method(b) == a.$native(b).ok_or(ArithmeticError::Overflow));
            }};
        }
        one!(u8);
        one!(u16);
        one!(u32);
        one!(u64);
        one!(u128);
        one!(usize);
        one!(i8);
        one!(i16);
        one!(i32);
        one!(i64);
        one!(i128);
        one!(isize);
    }};
}

#[kani::proof]
#[kani::solver(z3)]
fn addition_reports_every_native_overflow() {
    check_types!(try_add, checked_add);
}

#[kani::proof]
#[kani::solver(z3)]
fn subtraction_reports_every_native_overflow() {
    check_types!(try_sub, checked_sub);
}

#[kani::proof]
#[kani::solver(z3)]
fn multiplication_reports_every_native_overflow() {
    check_types!(try_mul, checked_mul);
}

macro_rules! check_wrapping_types {
    ($method:ident, $native:ident) => {{
        macro_rules! one {
            ($ty:ty) => {{
                let a: $ty = kani::any();
                let b: $ty = kani::any();
                assert!(a.$method(b) == a.$native(b).0);
            }};
        }
        one!(u8);
        one!(u16);
        one!(u32);
        one!(u64);
        one!(u128);
        one!(usize);
        one!(i8);
        one!(i16);
        one!(i32);
        one!(i64);
        one!(i128);
        one!(isize);
    }};
}

#[kani::proof]
#[kani::solver(z3)]
fn wrapping_addition_matches_modular_result() {
    check_wrapping_types!(wrap_add, overflowing_add);
}

#[kani::proof]
#[kani::solver(z3)]
fn wrapping_subtraction_matches_modular_result() {
    check_wrapping_types!(wrap_sub, overflowing_sub);
}

#[kani::proof]
#[kani::solver(z3)]
fn wrapping_multiplication_matches_modular_result() {
    check_wrapping_types!(wrap_mul, overflowing_mul);
}

macro_rules! division_types {
    ($wrapping:expr, $checked:ident, $wrapped:ident, $native:ident, $native_wrap:ident) => {{
        macro_rules! one {
            ($ty:ty) => {{
                let a: $ty = kani::any();
                let b: $ty = kani::any();
                let actual = if $wrapping {
                    a.$wrapped(b)
                } else {
                    a.$checked(b)
                };
                let expected = if b == 0 {
                    Err(DivisionError::DivisionByZero)
                } else if $wrapping {
                    Ok(a.$native_wrap(b))
                } else {
                    a.$native(b).ok_or(DivisionError::Overflow)
                };
                assert!(actual.map_err(DivisionError::from) == expected);
            }};
        }
        one!(u8);
        one!(u16);
        one!(u32);
        one!(u64);
        one!(u128);
        one!(usize);
        one!(i8);
        one!(i16);
        one!(i32);
        one!(i64);
        one!(i128);
        one!(isize);
    }};
}

#[kani::proof]
#[kani::solver(z3)]
fn division_reports_zero_and_every_native_overflow() {
    division_types!(false, try_div, try_wrap_div, checked_div, wrapping_div);
}

#[kani::proof]
#[kani::solver(z3)]
fn wrapping_division_only_rejects_zero() {
    division_types!(true, try_div, try_wrap_div, checked_div, wrapping_div);
}

macro_rules! remainder_types {
    ($wrapping:expr) => {{
        macro_rules! one {
            ($ty:ty) => {{
                let a: $ty = kani::any();
                let b: $ty = kani::any();
                let actual = if $wrapping {
                    a.try_wrap_rem(b)
                } else {
                    a.try_rem(b)
                };
                let checked = a.checked_rem(b);
                let expected = if b == 0 {
                    Err(DivisionError::DivisionByZero)
                } else if $wrapping {
                    Ok(checked.unwrap_or(0))
                } else {
                    checked.ok_or(DivisionError::Overflow)
                };
                assert!(actual.map_err(DivisionError::from) == expected);
            }};
        }
        one!(u8);
        one!(u16);
        one!(u32);
        one!(u64);
        one!(u128);
        one!(usize);
        one!(i8);
        one!(i16);
        one!(i32);
        one!(i64);
        one!(i128);
        one!(isize);
    }};
}

#[kani::proof]
#[kani::solver(z3)]
fn remainder_reports_zero_and_signed_overflow() {
    remainder_types!(false);
}
#[kani::proof]
#[kani::solver(z3)]
fn wrapping_remainder_only_rejects_zero() {
    remainder_types!(true);
}
#[kani::proof]
#[kani::solver(z3)]
fn checked_operators_preserve_all_primitive_results() {
    macro_rules! one {
        ($ty:ty) => {{
            let a: $ty = kani::any();
            let b: $ty = kani::any();
            let (x, y) = (Checked::new(a), Checked::new(b));
            assert!((x + y).map(Checked::get) == a.try_add(b));
            assert!((x - y).map(Checked::get) == a.try_sub(b));
            assert!((x * y).map(Checked::get) == a.try_mul(b));
            assert!((x / y).map(Checked::get) == a.try_div(b));
            assert!((x % y).map(Checked::get) == a.try_rem(b));
        }};
    }
    one!(u8);
    one!(u16);
    one!(u32);
    one!(u64);
    one!(u128);
    one!(usize);
    one!(i8);
    one!(i16);
    one!(i32);
    one!(i64);
    one!(i128);
    one!(isize);
}
