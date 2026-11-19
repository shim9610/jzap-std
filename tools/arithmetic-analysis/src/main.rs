use jzap::{ArithmeticError, DivisionError64, TryArithmetic, WrappingArithmetic};
use jzap_arithmetic_analysis as calls;
use std::{hint::black_box, time::Instant};

#[inline]
fn plain<const OP: u8>(a: usize, b: usize) -> usize {
    match OP {
        0 => a + b,
        1 => a - b,
        2 => a * b,
        _ => a / b,
    }
}
#[inline]
fn native<const OP: u8>(a: usize, b: usize) -> usize {
    match OP {
        0 => a
            .checked_add(b)
            .ok_or(ArithmeticError::Overflow)
            .unwrap_or(0),
        1 => a
            .checked_sub(b)
            .ok_or(ArithmeticError::Overflow)
            .unwrap_or(0),
        2 => a
            .checked_mul(b)
            .ok_or(ArithmeticError::Overflow)
            .unwrap_or(0),
        _ => if b == 0 {
            Err(DivisionError64::DivisionByZero)
        } else {
            a.checked_div(b).ok_or(DivisionError64::Overflow)
        }
        .unwrap_or(0),
    }
}
#[inline]
fn fallible<const OP: u8>(a: usize, b: usize) -> usize {
    match OP {
        0 => a.try_add(b).unwrap_or(0),
        1 => a.try_sub(b).unwrap_or(0),
        2 => a.try_mul(b).unwrap_or(0),
        _ => a.try_div(b).unwrap_or(0),
    }
}

#[inline]
fn wrapping<const OP: u8>(a: usize, b: usize) -> usize {
    match OP {
        0 => a.wrap_add(b),
        1 => a.wrap_sub(b),
        2 => a.wrap_mul(b),
        _ => a.try_wrap_div(b).unwrap_or(0),
    }
}

// MODE: 0 opaque inlined operands, 1 visibly bounded inlined operands,
// 2 actual non-inlined calls including their return-value ABI.
fn measure<const OP: u8, const MODE: u8, const IMPL: u8>(label: &str, scenario: &str, n: usize) {
    let mut samples = Vec::with_capacity(15);
    let (input_a, input_b) = (8191usize, 17usize);
    let mut checksum = 0usize;
    for cycle in 0..17 {
        let start = Instant::now();
        for _ in 0..n {
            // Barrier inputs for every operation, preventing hoisting and SIMD
            // batching. The identical barriers are present in every variant.
            let (mut a, mut b) = (black_box(input_a), black_box(input_b));
            if MODE == 1 {
                a &= 0xffff;
                b &= 0xff;
                if OP == 1 {
                    a |= 0x10000;
                }
            }
            let value = if MODE == 2 {
                match (IMPL, OP) {
                    (0, 0) => calls::plain_add(a, b),
                    (0, 1) => calls::plain_sub(a, b),
                    (0, 2) => calls::plain_mul(a, b),
                    (0, _) => calls::plain_div(a, b),
                    (1, 0) => calls::native_add(a, b).unwrap_or(0),
                    (1, 1) => calls::native_sub(a, b).unwrap_or(0),
                    (1, 2) => calls::native_mul(a, b).unwrap_or(0),
                    (1, _) => calls::native_div(a, b).unwrap_or(0),
                    (2, 0) => calls::jzap_add(a, b).unwrap_or(0),
                    (2, 1) => calls::jzap_sub(a, b).unwrap_or(0),
                    (2, 2) => calls::jzap_mul(a, b).unwrap_or(0),
                    (2, _) => calls::jzap_div(a, b).unwrap_or(0),
                    (_, 0) => calls::jzap_wrap_add(a, b),
                    (_, 1) => calls::jzap_wrap_sub(a, b),
                    (_, 2) => calls::jzap_wrap_mul(a, b),
                    _ => calls::jzap_wrap_div(a, b).unwrap_or(0),
                }
            } else {
                match IMPL {
                    0 => plain::<OP>(a, b),
                    1 => native::<OP>(a, b),
                    2 => fallible::<OP>(a, b),
                    _ => wrapping::<OP>(a, b),
                }
            };
            checksum ^= value;
        }
        let elapsed = start.elapsed().as_nanos();
        black_box(checksum);
        if cycle >= 2 {
            samples.push(elapsed);
        }
    }
    println!(
        "{{\"label\":\"{label}\",\"scenario\":\"{scenario}\",\"op\":{OP},\"operations\":{n},\"samples_ns\":{samples:?}}}"
    );
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let label = args.get(1).expect("plain/native/jzap/wrapping").as_str();
    let op = args.get(2).expect("add/sub/mul/div").as_str();
    let scenario = args.get(3).expect("opaque/bounded/call").as_str();
    let n = args
        .get(4)
        .map_or(1 << 20, |s| s.parse().expect("operations"));
    macro_rules! implementation {
        ($op:expr,$mode:expr) => {
            match label {
                "plain" => measure::<$op, $mode, 0>(label, scenario, n),
                "native" => measure::<$op, $mode, 1>(label, scenario, n),
                "jzap" => measure::<$op, $mode, 2>(label, scenario, n),
                "wrapping" => measure::<$op, $mode, 3>(label, scenario, n),
                _ => panic!("implementation"),
            }
        };
    }
    macro_rules! mode {
        ($op:expr) => {
            match scenario {
                "opaque" => implementation!($op, 0),
                "bounded" => implementation!($op, 1),
                "call" => implementation!($op, 2),
                _ => panic!("scenario"),
            }
        };
    }
    match op {
        "add" => mode!(0),
        "sub" => mode!(1),
        "mul" => mode!(2),
        "div" => mode!(3),
        _ => panic!("operation"),
    }
}
