# Integer arithmetic

Two sealed traits cover addition, subtraction, multiplication, division and remainder for
all 12 primitive signed and unsigned integer types. `TryArithmetic` is the
default contract for calculations that must preserve the mathematical range.
`WrappingArithmetic` explicitly opts into modular results. Both contracts are
independent of overflow-check compiler flags and use core operations without
external runtime dependencies, allocation or unsafe code.

| Operation | Default, range checked | Explicit wrapping |
| --- | --- | --- |
| Addition | `try_add`: `Result<T, ArithmeticError>` | `wrap_add`: `T` |
| Subtraction | `try_sub`: `Result<T, ArithmeticError>` | `wrap_sub`: `T` |
| Multiplication | `try_mul`: `Result<T, ArithmeticError>` | `wrap_mul`: `T` |
| Division | `try_div`: `Result<T, T::DivisionError>` | `try_wrap_div`: `Result<T, T::DivisionError>` |
| Remainder | `try_rem`: `Result<T, T::DivisionError>` | `try_wrap_rem`: `Result<T, T::DivisionError>` |

The default methods return `Overflow` for results outside the type's range,
including unsigned subtraction below zero and signed `MIN / -1`. Wrapping
addition, subtraction and multiplication return the low bits modulo `2^BITS`,
interpreted in the operand type. Division truncates toward zero;
`MIN.try_wrap_div(-1)` returns `Ok(MIN)`. Both division methods return
`DivisionError::DivisionByZero` for a zero divisor; checked signed division uses
`DivisionError::Overflow` for `MIN / -1`. Checked remainder returns the same
errors for zero and signed `MIN % -1`; wrapping remainder returns zero for
`MIN % -1`. Other remainders follow the dividend’s sign. Overflow itself is a numerical condition;
it does not require a panic or an error when modular results are requested.

Containers validate a wrapping result as an arbitrary input before using it as
an index or allocation count. Valid in-range results remain valid; internal
metadata helpers additionally require a proof that no wrapping occurs.
See [memory-boundary verification](verification/wrapping-memory.md) for the
source-connected arithmetic proofs, actual-memory checks, and their boundaries.

```rust
use jzap::{ArithmeticError, DivisionError, TryArithmetic, WrappingArithmetic};

assert_eq!(20usize.try_add(22), Ok(42));
assert_eq!(usize::MAX.try_add(1), Err(ArithmeticError::Overflow));
assert_eq!(usize::MAX.wrap_add(1), 0);
assert_eq!(0usize.wrap_sub(1), usize::MAX);
assert_eq!(usize::MAX.wrap_mul(2), usize::MAX - 1);
assert_eq!(i8::MIN.try_div(-1), Err(DivisionError::Overflow));
assert_eq!(i8::MIN.try_wrap_div(-1), Ok(i8::MIN));
assert_eq!(42u32.try_wrap_div(0), Err(DivisionError::DivisionByZero));
// 64-bit integers use DivisionError64; From converts it to DivisionError.
assert_eq!(42usize.try_div(0).map_err(DivisionError::from), Err(DivisionError::DivisionByZero));
```

Operands are copied primitive values and remain unchanged on either success or
failure. Errors do not contain operand snapshots. To preserve a stored value on
failure, compute the result first and assign only after success. No snapshot or
heap allocation is needed. Wrapping remains an explicit numerical API; it does
not replace range checks in container lengths, capacities, budgets or pointers.

`ArithmeticError` is zero-sized. The division error type is the associated
`TryArithmetic::DivisionError` (the same type as `WrappingArithmetic::DivisionError`):

| Integer types | Division error | Size |
| --- | --- | --- |
| `u8`, `u16`, `u32`, `i8`, `i16`, `i32`, `u128`, `i128` | `DivisionError` | 1 byte |
| `u64`, `i64`, and `usize`/`isize` on 64-bit targets | `DivisionError64` (`#[repr(u64)]`) | 8 bytes |
| `usize`/`isize` on 32-bit targets | `DivisionError` | 1 byte |

Both types have the variants `Overflow` and `DivisionByZero` with the same meaning;
`DivisionError::from` (and `?`) converts `DivisionError64` without loss. On the
measured 64-bit target, `Result<usize, ArithmeticError>` and
`Result<u64, DivisionError64>` occupy 16 bytes, versus 8 bytes for a bare `usize`;
no heap storage is involved.

The error width follows the quotient so that non-inlined calls keep the register
return used by `Option<T>`. With one 1-byte error for every type, a non-inlined
`Result<u64, DivisionError>` was written through a caller-provided pointer; with one
8-byte error for every type, the 8- to 32-bit results moved to memory instead.
Measured on Rust 1.95.0 (`-O`, `panic=abort`, `codegen-units=1`, x86-64), non-inlined
`try_div` instructions versus a `checked_div` control:

| Type | One 1-byte error | One 8-byte error | Per-width error (current) |
| --- | --- | --- | --- |
| `u32` | 10 / 10, registers | 16 / 10, memory | 10 / 10, registers |
| `u64` | 24 / 20, memory | 22 / 20, registers | 22 / 20, registers |
| `i64` | 34 / 26, memory | 29 / 26, registers | 29 / 26, registers |

The same pattern holds for `try_rem`, `try_wrap_div`, `try_wrap_rem` and the
`Checked` `/` and `%` operators across all twelve types; 128-bit results return
through memory for both the product and `Option`. The remaining 64-bit difference
of two to three instructions was not attributed instruction by instruction. Inside
inlined loops that map errors to zero, 29 of 30 checked operation/type loops
compile to code identical to the `checked_*` control under all three layouts; the
exception (`i64` wrapping remainder) is shorter in the product. Keeping
`ArithmeticError` separate preserves the register return of add/sub/mul. Tests
guard the error types and conversion, and codegen checks compare the actual public
Result ABI with core controls using the same error type.

The [arithmetic policy](arithmetic-policy.md) documents internal enforcement, bounded wrapping proof obligations and the current collection link results.

## Checked operators

`Checked<T>` is a transparent wrapper around any of the 12 primitive integer
types. `+`, `-` and `*` return `Result<Checked<T>, ArithmeticError>`; `/` and `%`
return `Result<Checked<T>, DivisionError>`. It has no implicit dereference,
conversion, or assignment arithmetic. Extracting the primitive requires `get()`.
`TryClone` and `TryCloneIn` copy the scalar without allocation.

```rust
use jzap::{ArithmeticError, Checked};
let length = Checked::new(usize::MAX);
assert_eq!(length + Checked::new(1), Err(ArithmeticError::Overflow));
let next = (Checked::new(20usize) + Checked::new(22))?;
# Ok::<(), ArithmeticError>(())
```

Store the successful result only after `?` or an explicit match. Existing operands
remain unchanged on error. `Checked<usize>` has the size and alignment of `usize`;
the returned Result still carries success/error state. Compile-fail doctests reject
`+=` and assigning the operator result directly to a `Checked` variable. The
library denies `unused_must_use`; callers can still explicitly discard a Result.

## Verification

| Check | Result and scope |
| --- | --- |
| Every signed and unsigned byte pair | 1,310,720 primitive operation results checked with wider arithmetic; another 655,360 Checked-operator comparisons |
| Integer extrema | Success, overflow, underflow and zero-divisor cases across all 12 types |
| Allocation refusal | Instrumented primitive arithmetic makes zero allocator calls on success and error |
| Verus | 36 primitive and three generic Checked add/sub/mul contracts, without input bounds or additional trusted specifications |
| Arithmetic Kani | 11 harnesses: ten primitive methods and five operators, all bit patterns across all 12 types |
| Deliberate source defects | Seven direct Verus controls cover wrong operations, a new panic, dispatch, the operator specification and the numeric model; six actual-Rust controls cover wrapping, zero-divisor errors and Checked forwarding |
| Final ELF link | Release, overflow-checked release and debug; 1,088 boundary operations per configuration; negative panic control detected |
| Actual Result ABI | Optimized Checked and primitive wrappers match native checked-operation controls for all five operators |

Verus reads `src/lib.rs` and the original `integers!` implementations. Contracts
are attached to those methods; the Python runner does not extract or rewrite
their bodies. Core checked-integer specifications form part of its trusted library. Kani runs
the actual Rust methods, including division and remainder, against core checked
and wrapping contracts. Byte-pair and extrema tests provide independent
wider-arithmetic comparisons, including the signed MIN/-1 exceptions. Z3 handles
full-width multiplication; no input domain is narrowed to obtain a pass.
Kani runs on x86-64, so its `usize` and `isize` are 64-bit. Wasm and thumb are
compilation checks, not executions of those proofs on 32-bit machines.

Positive proofs retain all bit patterns. The six Kani mutation controls use
concrete `u8` counterexamples to the modified public APIs; they do not repeat
full-width negative searches. Negative-control inputs do not restrict positive
proof domains. The [migration guide](direct-source-migration.md) records which
other library projections remain.

## Performance interpretation

The codegen runner compares the actual public Result return types with native
`checked_*` controls using the same errors. It also requires wrapping add/sub/mul
to match plain default-release arithmetic, and visibly bounded checked
add/sub/mul to match their plain controls. Checks run in both optimized profiles.
This establishes wrapper instruction equality for those concrete functions;
inlining and the surrounding consumer can still change loop layout.

The timing runner covers four operations (remainder is not timed), three
conditions (opaque inputs, visibly bounded inputs and non-inlined calls), and
default/overflow-checked release. Each condition uses 30 independent processes,
two warmups and 15 retained batches of 1,048,576 operations. It alternates order
and includes an identical plain control. Timings include identical input barriers
and XOR bookkeeping; they are not individual instruction latencies. Timed inputs
succeed, and the harness consumes an error as zero.

Checking is not equivalent to unchecked release arithmetic: unsigned subtraction
may need a conditional result selection; multiplication may need the full product
and an overflow branch instead of low-half multiplication. Such costs accumulate
per scalar operation. A container's invariant can let LLVM remove those checks.
The [metadata report](checked-metadata.md) records the actual container comparison
and the separate remaining whole-library proof obligations.

## Reproduction

Requires Kani 0.68.0, Verus 0.2026.11.02.8bf68ee with Z3, Python 3, a C compiler
and Linux ELF tools. Set `JZAP_VERUS` when Verus is installed elsewhere.

```sh
cargo test --locked --no-default-features --test arithmetic
CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true cargo test --locked --release --test arithmetic
python3 scripts/verify_source.py --group arithmetic --mutations
python3 scripts/verify_direct_verus.py --target integer --mutations
python3 scripts/verify_arithmetic.py --output target/verification/arithmetic-wrapping
python3 scripts/compare_arithmetic.py --output target/verification/arithmetic-wrapping --trials 30
```

The verification runner builds both timing binaries before measurement, checks
native-wrapper, wrapping-wrapper and bounded-expression assembly equality, and
runs positive and negative link fixtures. Its metadata records the arithmetic source hash, tool
versions and baseline commit. Solver logs, raw timings and disassembly stay
under the chosen ignored `target/verification/` output directory.

## Generic checked operator contracts

The sealed integer interface has Verus-only value, range and inverse-conversion
metadata. All 12 primitive implementations prove the range and round trip.
The original generic `Checked` add/sub/mul implementations then prove their
mathematical value and exact overflow conditions. Their standard-operator
preconditions are true for every input, including overflow cases, which return
errors. The standard-operator equality obligations remain enabled.

These items are erased from stable Cargo builds. The runtime field remains
private and transparent. Wrapping/division/remainder forwarding retains its
actual-Rust Kani checks. The source controls also reject changed dispatch,
operator specifications and numeric models. See the
[migration record](direct-source-migration.md#checked-operators-and-vec-lengthgrowth-follow-up)
for code-generation comparisons and partial reproduction.
