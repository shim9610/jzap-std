//! Transparent integer ownership: ordinary operators must handle failure.
use super::{ArithmeticError, TryArithmetic};
use core::ops::{Add, Div, Mul, Rem, Sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// A primitive integer whose arithmetic operators explicitly return errors.
///
/// ```
/// use jzap::{Checked, ArithmeticError};
/// let a = Checked::new(usize::MAX);
/// assert_eq!(a + Checked::new(1), Err(ArithmeticError::Overflow));
/// assert_eq!((Checked::new(20u32) + Checked::new(22)).map(Checked::get), Ok(42));
/// ```
///
/// No implicit conversion, dereference, or assignment arithmetic can bypass the
/// Result. `get` explicitly extracts the primitive at an API/storage boundary.
/// ```compile_fail
/// use jzap::Checked;
/// let mut count = Checked::new(1usize);
/// count += Checked::new(1);
/// ```
/// ```compile_fail
/// use jzap::Checked;
/// let count = Checked::new(1usize);
/// let _: Checked<usize> = count + Checked::new(1);
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct Checked<T: TryArithmetic>(T);
/*@
pred<T> <arithmetic::checked::Checked<T>>.own(t, c) = <T>.own(t, c.0);

lem arithmetic::checked::Checked_drop<T>()
    req arithmetic::checked::Checked_own::<T>(?t, ?c);
    ens <T>.own(t, c.0);
{
    open arithmetic::checked::Checked_own::<T>(t, c);
}

lem arithmetic::checked::Checked_own_mono<T0, T1>()
    req type_interp::<T0>() &*& type_interp::<T1>() &*& arithmetic::checked::Checked_own::<T0>(?t, ?v) &*& is_subtype_of::<T0, T1>() == true;
    ens type_interp::<T0>() &*& type_interp::<T1>() &*& arithmetic::checked::Checked_own::<T1>(t, arithmetic::checked::Checked::<T1> { 0: upcast(v.0) });
{
    assume(false); // variance obligation; VeriFast's own RawVec proof does the same (verifast issue #610)
}

lem arithmetic::checked::Checked_send<T>(t1: thread_id_t)
    req type_interp::<T>() &*& is_Send(typeid(T)) == true &*& arithmetic::checked::Checked_own::<T>(?t0, ?v);
    ens type_interp::<T>() &*& arithmetic::checked::Checked_own::<T>(t1, v);
{
    open arithmetic::checked::Checked_own::<T>(t0, v);
    Send::send::<T>(t0, t1, v.0);
    close arithmetic::checked::Checked_own::<T>(t1, v);
}
@*/

#[cfg(verus_keep_ghost)]
verus! {
impl<T: TryArithmetic> Checked<T> {
    pub closed spec fn view(self) -> T { self.0 }
    pub closed spec fn math_new(value: T) -> Self { Checked(value) }
}
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl<T: TryArithmetic> Checked<T> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out@ == value,))]
    #[inline]
    pub const fn new(value: T) -> Self 
    //@ req true;
    //@ ens result == arithmetic::checked::Checked::<T> { 0: value };
    //@ on_unwind_ens false;
    //@ safety_proof { let outcome = call(); match outcome { returning(r) => { close arithmetic::checked::Checked_own::<T>(_t, r); } unwinding => {} } }
    {
        Self(value)
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@,))]
    #[inline]
    pub const fn get(self) -> T 
    //@ req true;
    //@ ens result == self.0;
    //@ on_unwind_ens false;
    //@ safety_proof { open arithmetic::checked::Checked_own::<T>(_t, self); let outcome = call(); }
    {
        self.0
    }
}
macro_rules! operator {
    ($trait:ident, $operator:ident, $method:ident, $error:ty, $verify:meta $(, $math:tt)?) => {
        #[cfg_attr(all(verus_keep_ghost, $verify), verus_verify)]
        impl<T: TryArithmetic> $trait for Checked<T> {
            type Output = Result<Self, $error>;
            $(#[cfg_attr(verus_keep_ghost, verus_spec(out =>
                ensures match out {
                    Ok(value) => value@.numeric_value() == self@.numeric_value() $math rhs@.numeric_value(),
                    Err(_) => (self@.numeric_value() $math rhs@.numeric_value()) < T::numeric_min()
                        || (self@.numeric_value() $math rhs@.numeric_value()) > T::numeric_max(),
                },
            ))])?
            #[inline]
            fn $operator(self, rhs: Self) -> Self::Output {
                match self.0.$method(rhs.0) {
                    Ok(value) => {
                        #[cfg(verus_keep_ghost)]
                        proof! { value.numeric_identity(); }
                        Ok(Self(value))
                    },
                    Err(error) => Err(error),
                }
            }
        }
    };
}
operator!(Add, add, try_add, ArithmeticError, all(), +);
operator!(Sub, sub, try_sub, ArithmeticError, all(), -);
operator!(Mul, mul, try_mul, ArithmeticError, all(), *);
operator!(
    Div,
    div,
    try_div,
    <T as TryArithmetic>::DivisionError,
    any()
);
operator!(
    Rem,
    rem,
    try_rem,
    <T as TryArithmetic>::DivisionError,
    any()
);

// The standard operator specification is checked against the same product bodies.
#[cfg(verus_keep_ghost)]
verus! {
impl<T: TryArithmetic> vstd::std_specs::ops::AddSpecImpl for Checked<T> {
    open spec fn obeys_add_spec() -> bool { true }
    open spec fn add_req(self, _rhs: Self) -> bool { true }
    open spec fn add_spec(self, rhs: Self) -> Self::Output {
        let value = self@.numeric_value() + rhs@.numeric_value();
        if value < T::numeric_min() || value > T::numeric_max() {
            Err(ArithmeticError::Overflow)
        } else {
            Ok(Checked::math_new(T::numeric_from_int(value)))
        }
    }
}
impl<T: TryArithmetic> vstd::std_specs::ops::SubSpecImpl for Checked<T> {
    open spec fn obeys_sub_spec() -> bool { true }
    open spec fn sub_req(self, _rhs: Self) -> bool { true }
    open spec fn sub_spec(self, rhs: Self) -> Self::Output {
        let value = self@.numeric_value() - rhs@.numeric_value();
        if value < T::numeric_min() || value > T::numeric_max() {
            Err(ArithmeticError::Overflow)
        } else {
            Ok(Checked::math_new(T::numeric_from_int(value)))
        }
    }
}
impl<T: TryArithmetic> vstd::std_specs::ops::MulSpecImpl for Checked<T> {
    open spec fn obeys_mul_spec() -> bool { true }
    open spec fn mul_req(self, _rhs: Self) -> bool { true }
    open spec fn mul_spec(self, rhs: Self) -> Self::Output {
        let value = self@.numeric_value() * rhs@.numeric_value();
        if value < T::numeric_min() || value > T::numeric_max() {
            Err(ArithmeticError::Overflow)
        } else {
            Ok(Checked::math_new(T::numeric_from_int(value)))
        }
    }
}
}
