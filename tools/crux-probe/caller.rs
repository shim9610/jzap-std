#[cfg(crux)]
extern crate crucible;
// Direct bounded caller probe; no product body is copied or changed.
#[cfg(crux)]
mod proofs {
    use crucible::{Symbolic, crucible_assert};
    use jzap::{Error, error::InitError, vec::Vec};

    #[crux::test]
    fn original_initializer_limited_zst() {
        let requested = usize::symbolic("requested_length");
        let fail_index = usize::symbolic("failure_index");
        let mut values = Vec::<()>::with_limit(2);
        let result = values.try_resize_with_init(requested, |i, slot| {
            if i == fail_index {
                Err(0xA5_u8)
            } else {
                slot.write(()).map_err(|_| 0x5A_u8)
            }
        });
        if requested > 2 {
            crucible_assert!(matches!(result, Err(InitError::Storage(Error::Capacity))));
            crucible_assert!(values.len() == 0);
        } else if fail_index < requested {
            crucible_assert!(matches!(result, Err(InitError::Initialize(0xA5))));
            crucible_assert!(values.len() == 0);
        } else {
            crucible_assert!(result.is_ok());
            crucible_assert!(values.len() == requested);
        }
    }
    #[crux::test]
    fn original_initializer_limited_byte() {
        let requested = usize::symbolic("requested_length");
        let fail_index = usize::symbolic("failure_index");
        let payload = u8::symbolic("element_payload");
        let mut values = Vec::<u8>::with_limit(2);
        let result = values.try_resize_with_init(requested, |i, slot| {
            if i == fail_index {
                Err(0xA5_u8)
            } else {
                slot.write(payload).map_err(|_| 0x5A_u8)
            }
        });
        if requested > 2 {
            crucible_assert!(matches!(result, Err(InitError::Storage(Error::Capacity))));
            crucible_assert!(values.len() == 0);
        } else if fail_index < requested {
            crucible_assert!(matches!(result, Err(InitError::Initialize(0xA5))));
            crucible_assert!(values.len() == 0);
        } else {
            crucible_assert!(result.is_ok());
            crucible_assert!(values.len() == requested);
            if requested > 0 {
                crucible_assert!(values.as_slice().first() == Some(&payload));
            }
            if requested > 1 {
                crucible_assert!(values.as_slice().get(1) == Some(&payload));
            }
        }
    }
}
