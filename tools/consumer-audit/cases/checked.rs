use jzap::{Checked, Error, TryIterator, error::InitError, vec::Vec};
struct Numbers {
    next: Checked<usize>,
    remaining: Checked<usize>,
}
impl TryIterator for Numbers {
    type Item = usize;
    type Error = Error;
    fn try_next(&mut self) -> Result<Option<usize>, Error> {
        if self.remaining.get() == 0 {
            return Ok(None);
        }
        let remaining = (self.remaining - Checked::new(1)).map_err(|_| Error::Capacity)?;
        let next = (self.next + Checked::new(1)).map_err(|_| Error::Capacity)?;
        let item = self.next.get();
        self.remaining = remaining;
        self.next = next;
        Ok(Some(item))
    }
}
pub fn run(seed: usize) -> Result<(), Error> {
    let mut values = Vec::with_limit(16);
    values
        .try_extend_fallible(Numbers {
            next: Checked::new(seed),
            remaining: Checked::new(8),
        })
        .map_err(|error| match error {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })?;
    core::hint::black_box(values.as_slice());
    Ok(())
}
