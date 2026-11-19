//! Fallible formatting directly into UTF-8 storage, without a temporary std String.
pub use crate::string::FormatError;
use crate::{budget::AllocationPolicy, string::String, vec::LengthPolicy};

pub fn try_format(args: core::fmt::Arguments<'_>) -> Result<String, FormatError> {
    String::try_format(args)
}
pub fn try_format_in<P: AllocationPolicy, L: LengthPolicy>(
    args: core::fmt::Arguments<'_>,
    policy: P,
    limit: L,
) -> Result<String<P, L>, FormatError> {
    let mut result = String::try_from_str_in("", policy, limit).map_err(FormatError::Storage)?;
    result.try_write_fmt(args)?;
    Ok(result)
}
pub trait TryToString {
    fn try_to_string(&self) -> Result<String, FormatError>;
}
impl<T: core::fmt::Display + ?Sized> TryToString for T {
    fn try_to_string(&self) -> Result<String, FormatError> {
        try_format(format_args!("{self}"))
    }
}
