//! Explicit failure-capable defaults; no blanket delegation to user Default.
use crate::{Error, error::InitError, init::InitSlot};
pub trait TryDefault: Sized {
    fn try_default() -> Result<Self, Error>;
    fn try_default_into(slot: &mut InitSlot<'_, Self>) -> Result<(), Error> {
        if slot.get().is_some() {
            return Err(Error::AlreadyInitialized);
        }
        slot.write(Self::try_default()?).map_err(|e| e.error)
    }
}
macro_rules! scalar { ($($ty:ty),*) => { $(impl TryDefault for $ty {
    fn try_default() -> Result<Self, Error> { Ok(Self::default()) }
})* }; }
scalar!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64
);
impl<T> TryDefault for Option<T> {
    fn try_default() -> Result<Self, Error> {
        Ok(None)
    }
}
impl<T> TryDefault for &[T] {
    fn try_default() -> Result<Self, Error> {
        Ok(&[])
    }
}
impl TryDefault for &str {
    fn try_default() -> Result<Self, Error> {
        Ok("")
    }
}
impl<T: TryDefault, const N: usize> TryDefault for [T; N] {
    fn try_default() -> Result<Self, Error> {
        crate::clone::array_from_fn(|_| T::try_default())
    }
    fn try_default_into(slot: &mut InitSlot<'_, Self>) -> Result<(), Error> {
        slot.try_init_array(|_, element| T::try_default_into(element))
            .map_err(|e| match e {
                InitError::Storage(e) | InitError::Initialize(e) => e,
            })
    }
}
macro_rules! tuple { ($($t:ident),+) => {impl<$($t:TryDefault),+> TryDefault for ($($t,)+) {
    fn try_default() -> Result<Self, Error> { Ok(($($t::try_default()?,)+)) }
}}; }
tuple!(A);
tuple!(A, B);
tuple!(A, B, C);
tuple!(A, B, C, D);
tuple!(A, B, C, D, E);
tuple!(A, B, C, D, E, F);
tuple!(A, B, C, D, E, F, G);
tuple!(A, B, C, D, E, F, G, H);
tuple!(A, B, C, D, E, F, G, H, I);
tuple!(A, B, C, D, E, F, G, H, I, J);
tuple!(A, B, C, D, E, F, G, H, I, J, K);
tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
