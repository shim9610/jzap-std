// Boundary experiment, not a proof of the original Vec caller or its loop.
use crucible::{Symbolic, crucible_assert, crucible_assume};

#[path = "../../src/error.rs"]
mod error;
use error::{Error, InitError};

fn arbitrary_error() -> Error {
    let tag = u8::symbolic("error_tag");
    crucible_assume!(tag < 11);
    let a = usize::symbolic("first_payload");
    let b = usize::symbolic("second_payload");
    let code = i32::symbolic("entropy_code");
    match tag {
        0 => Error::Allocation { size: a, align: b },
        1 => Error::Layout,
        2 => Error::Budget {
            requested: a,
            available: b,
        },
        3 => Error::Capacity,
        4 => Error::RefCount,
        5 => Error::Entropy { code },
        6 => Error::Bounds,
        7 => Error::CharBoundary,
        8 => Error::InvalidStructure,
        9 => Error::Uninitialized,
        _ => Error::AlreadyInitialized,
    }
}

#[crux::test]
fn constructor_as_function() {
    let error = arbitrary_error();
    let value = u64::symbolic("success_payload");
    let input: Result<u64, Error> = if bool::symbolic("succeeds") {
        Ok(value)
    } else {
        Err(error)
    };
    // Exercises the constructor-as-function syntax rejected by pinned Verus.
    // This is a core/enum boundary harness, not a copied Vec body.
    let output = input.map_err(InitError::<Error>::Storage);
    match (input, output) {
        (Ok(expected), Ok(actual)) => crucible_assert!(expected == actual),
        (Err(expected), Err(InitError::Storage(actual))) => {
            crucible_assert!(expected == actual);
        }
        _ => crucible_assert!(false),
    }
}

#[crux::test]
fn original_from_implementation() {
    let expected = arbitrary_error();
    // Calls the unmodified implementation included from src/error.rs.
    let output = InitError::<u8>::from(expected);
    match output {
        InitError::Storage(actual) => crucible_assert!(expected == actual),
        InitError::Initialize(_) => crucible_assert!(false),
    }
}
