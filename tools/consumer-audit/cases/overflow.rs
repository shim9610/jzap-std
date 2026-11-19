// No panic!/unwrap: a user implementation bypasses checked arithmetic.
use jzap::{Error, TryIterator};
struct Bad(usize);
impl TryIterator for Bad {
    type Item = usize;
    type Error = Error;
    fn try_next(&mut self) -> Result<Option<usize>, Error> {
        Ok(Some(self.0 + 1))
    }
}
pub fn run(seed: usize) -> Result<(), Error> {
    core::hint::black_box(Bad(seed).try_next()?);
    Ok(())
}
