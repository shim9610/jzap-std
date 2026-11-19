use jzap::{
    ArithmeticError, Checked, DivisionError, DivisionError64, TryArithmetic, WrappingArithmetic,
};
#[expect(
    dead_code,
    reason = "Other suites use the shared allocator's separate drop watcher."
)]
mod common;
#[global_allocator]
static ALLOC: common::FaultAllocator = common::FaultAllocator;

#[test]
fn arithmetic_never_allocates_even_when_returning_errors() {
    assert_eq!(std::mem::size_of::<ArithmeticError>(), 0);
    assert_eq!(
        std::mem::size_of::<Checked<usize>>(),
        std::mem::size_of::<usize>()
    );
    assert_eq!(
        std::mem::align_of::<Checked<usize>>(),
        std::mem::align_of::<usize>()
    );
    assert_eq!(
        std::mem::size_of::<Result<usize, ArithmeticError>>(),
        2 * std::mem::size_of::<usize>()
    );
    let (results, stats) = common::track(1, || {
        let maximum = std::hint::black_box(usize::MAX);
        let one = std::hint::black_box(1usize);
        (
            [
                maximum.try_add(one),
                std::hint::black_box(0usize).try_sub(one),
                maximum.try_mul(std::hint::black_box(2)),
                std::hint::black_box(20usize).try_add(std::hint::black_box(22)),
                Ok(maximum.wrap_add(one)),
                Ok(std::hint::black_box(0usize).wrap_sub(one)),
                Ok(maximum.wrap_mul(std::hint::black_box(2))),
            ],
            [
                maximum.try_div(one).map_err(DivisionError::from),
                maximum
                    .try_div(std::hint::black_box(0))
                    .map_err(DivisionError::from),
                maximum
                    .try_wrap_div(std::hint::black_box(0))
                    .map_err(DivisionError::from),
            ],
        )
    });
    assert_eq!(
        results,
        (
            [
                Err(ArithmeticError::Overflow),
                Err(ArithmeticError::Overflow),
                Err(ArithmeticError::Overflow),
                Ok(42),
                Ok(0),
                Ok(usize::MAX),
                Ok(usize::MAX - 1),
            ],
            [
                Ok(usize::MAX),
                Err(DivisionError::DivisionByZero),
                Err(DivisionError::DivisionByZero),
            ]
        )
    );
    assert_eq!((stats.calls, stats.live, stats.peak), (0, 0, 0));
}

#[test]
fn every_byte_pair_matches_independent_wider_arithmetic() {
    for a in u8::MIN..=u8::MAX {
        for b in u8::MIN..=u8::MAX {
            let (x, y) = (i32::from(a), i32::from(b));
            for (actual, mathematical) in [
                (a.try_add(b), x + y),
                (a.try_sub(b), x - y),
                (a.try_mul(b), x * y),
            ] {
                assert_eq!(
                    actual,
                    u8::try_from(mathematical).map_err(|_| ArithmeticError::Overflow)
                );
            }
            assert_eq!(a.wrap_add(b), (x + y) as u8);
            assert_eq!(a.wrap_sub(b), (x - y) as u8);
            assert_eq!(a.wrap_mul(b), (x * y) as u8);
            let (left, right) = (Checked::new(a), Checked::new(b));
            assert_eq!((left + right).map(Checked::get), a.try_add(b));
            assert_eq!((left - right).map(Checked::get), a.try_sub(b));
            assert_eq!((left * right).map(Checked::get), a.try_mul(b));
            assert_eq!((left / right).map(Checked::get), a.try_div(b));
            assert_eq!((left % right).map(Checked::get), a.try_rem(b));
            if y == 0 {
                assert_eq!(a.try_div(b), Err(DivisionError::DivisionByZero));
                assert_eq!(a.try_wrap_div(b), Err(DivisionError::DivisionByZero));
                assert_eq!(a.try_rem(b), Err(DivisionError::DivisionByZero));
                assert_eq!(a.try_wrap_rem(b), Err(DivisionError::DivisionByZero));
            } else {
                let quotient = x / y;
                assert_eq!(
                    a.try_div(b),
                    u8::try_from(quotient).map_err(|_| DivisionError::Overflow)
                );
                assert_eq!(a.try_wrap_div(b), Ok(quotient as u8));
                let remainder = Ok((x % y) as u8);
                assert_eq!(a.try_rem(b), remainder);
                assert_eq!(a.try_wrap_rem(b), Ok((x % y) as u8));
            }
        }
    }
    for a in i8::MIN..=i8::MAX {
        for b in i8::MIN..=i8::MAX {
            let (x, y) = (i32::from(a), i32::from(b));
            for (actual, mathematical) in [
                (a.try_add(b), x + y),
                (a.try_sub(b), x - y),
                (a.try_mul(b), x * y),
            ] {
                assert_eq!(
                    actual,
                    i8::try_from(mathematical).map_err(|_| ArithmeticError::Overflow)
                );
            }
            assert_eq!(a.wrap_add(b), (x + y) as i8);
            assert_eq!(a.wrap_sub(b), (x - y) as i8);
            assert_eq!(a.wrap_mul(b), (x * y) as i8);
            let (left, right) = (Checked::new(a), Checked::new(b));
            assert_eq!((left + right).map(Checked::get), a.try_add(b));
            assert_eq!((left - right).map(Checked::get), a.try_sub(b));
            assert_eq!((left * right).map(Checked::get), a.try_mul(b));
            assert_eq!((left / right).map(Checked::get), a.try_div(b));
            assert_eq!((left % right).map(Checked::get), a.try_rem(b));
            if y == 0 {
                assert_eq!(a.try_div(b), Err(DivisionError::DivisionByZero));
                assert_eq!(a.try_wrap_div(b), Err(DivisionError::DivisionByZero));
                assert_eq!(a.try_rem(b), Err(DivisionError::DivisionByZero));
                assert_eq!(a.try_wrap_rem(b), Err(DivisionError::DivisionByZero));
            } else {
                let quotient = x / y;
                assert_eq!(
                    a.try_div(b),
                    i8::try_from(quotient).map_err(|_| DivisionError::Overflow)
                );
                assert_eq!(a.try_wrap_div(b), Ok(quotient as i8));
                let remainder = if x == -128 && y == -1 {
                    Err(DivisionError::Overflow)
                } else {
                    Ok((x % y) as i8)
                };
                assert_eq!(a.try_rem(b), remainder);
                assert_eq!(a.try_wrap_rem(b), Ok((x % y) as i8));
            }
        }
    }
}

#[test]
fn all_integer_widths_preserve_extremes_and_report_each_overflow_direction() {
    macro_rules! unsigned {
        ($($ty:ty),*) => {$(
            assert_eq!(<$ty>::MAX.try_add(0), Ok(<$ty>::MAX));
            assert_eq!(<$ty>::MAX.try_add(1), Err(ArithmeticError::Overflow));
            assert_eq!((0 as $ty).try_sub(1), Err(ArithmeticError::Overflow));
            assert_eq!(<$ty>::MAX.try_sub(<$ty>::MAX), Ok(0));
            assert_eq!(<$ty>::MAX.try_mul(1), Ok(<$ty>::MAX));
            assert_eq!(<$ty>::MAX.try_mul(2), Err(ArithmeticError::Overflow));
            assert_eq!(<$ty>::MAX.try_mul(0), Ok(0));
            assert_eq!(<$ty>::MAX.wrap_add(1), 0);
            assert_eq!((0 as $ty).wrap_sub(1), <$ty>::MAX);
            assert_eq!(<$ty>::MAX.wrap_mul(2), <$ty>::MAX - 1);
            assert_eq!(<$ty>::MAX.try_div(1), Ok(<$ty>::MAX));
            assert_eq!(<$ty>::MAX.try_wrap_div(1), Ok(<$ty>::MAX));
            assert_eq!((0 as $ty).try_div(0).map_err(DivisionError::from), Err(DivisionError::DivisionByZero));
            assert_eq!((0 as $ty).try_wrap_div(0).map_err(DivisionError::from), Err(DivisionError::DivisionByZero));
        )*};
    }
    unsigned!(u8, u16, u32, u64, u128, usize);
    macro_rules! signed {
        ($($ty:ty),*) => {$(
            assert_eq!(<$ty>::MAX.try_add(1), Err(ArithmeticError::Overflow));
            assert_eq!(<$ty>::MIN.try_add(-1), Err(ArithmeticError::Overflow));
            assert_eq!(<$ty>::MIN.try_sub(1), Err(ArithmeticError::Overflow));
            assert_eq!(<$ty>::MAX.try_sub(-1), Err(ArithmeticError::Overflow));
            assert_eq!(<$ty>::MIN.try_mul(-1), Err(ArithmeticError::Overflow));
            assert_eq!(<$ty>::MIN.try_mul(1), Ok(<$ty>::MIN));
            assert_eq!(<$ty>::MAX.try_mul(-1), Ok(-<$ty>::MAX));
            assert_eq!(<$ty>::MIN.try_add(<$ty>::MAX), Ok(-1));
            assert_eq!(<$ty>::MAX.wrap_add(1), <$ty>::MIN);
            assert_eq!(<$ty>::MIN.wrap_sub(1), <$ty>::MAX);
            assert_eq!(<$ty>::MIN.wrap_mul(-1), <$ty>::MIN);
            assert_eq!(<$ty>::MIN.try_div(-1).map_err(DivisionError::from), Err(DivisionError::Overflow));
            assert_eq!(<$ty>::MIN.try_wrap_div(-1), Ok(<$ty>::MIN));
            assert_eq!((7 as $ty).try_div(-2), Ok(-3));
            assert_eq!((-7 as $ty).try_wrap_div(2), Ok(-3));
            assert_eq!(<$ty>::MIN.try_div(0).map_err(DivisionError::from), Err(DivisionError::DivisionByZero));
            assert_eq!(<$ty>::MIN.try_wrap_div(0).map_err(DivisionError::from), Err(DivisionError::DivisionByZero));
        )*};
    }
    signed!(i8, i16, i32, i64, i128, isize);
}

#[test]
fn sixty_four_bit_division_uses_its_own_error_and_converts_losslessly() {
    let quotient: Result<u64, DivisionError64> = 7u64.try_div(0);
    assert_eq!(quotient, Err(DivisionError64::DivisionByZero));
    let quotient: Result<i64, DivisionError64> = i64::MIN.try_div(-1);
    assert_eq!(quotient, Err(DivisionError64::Overflow));
    let remainder: Result<u32, DivisionError> = 7u32.try_rem(0);
    assert_eq!(remainder, Err(DivisionError::DivisionByZero));
    assert_eq!(
        DivisionError::from(DivisionError64::Overflow),
        DivisionError::Overflow
    );
    assert_eq!(
        DivisionError::from(DivisionError64::DivisionByZero),
        DivisionError::DivisionByZero
    );
    assert_eq!(
        DivisionError64::Overflow.to_string(),
        DivisionError::Overflow.to_string()
    );
    #[cfg(target_pointer_width = "64")]
    {
        let size: Result<usize, DivisionError64> = 1usize.try_wrap_div(1);
        assert_eq!(size, Ok(1));
    }
}
