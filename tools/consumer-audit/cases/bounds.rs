// No panic!/unwrap: ordinary indexing can violate the iterator contract too.
use jzap::{Error, TryIterator};
struct Bad(usize);
impl TryIterator for Bad {
    type Item = usize;
    type Error = Error;
    fn try_next(&mut self) -> Result<Option<usize>, Error> {
        let values = core::hint::black_box([7usize, 11]);
        Ok(Some(values[self.0]))
    }
}
pub fn run(seed: usize) -> Result<(), Error> {
    core::hint::black_box(Bad(seed).try_next()?);
    Ok(())
}
