use jzap::Error;
unsafe extern "C" {
    fn abort() -> !;
}
pub fn run(_: usize) -> Result<(), Error> {
    unsafe { abort() }
}
