//! Direct emission after reservation. Bit layout follows pinned Rust core's
//! UTF-8 encoder (MIT OR Apache-2.0), with an actual-source all-scalar proof.
/// # Safety
/// destination uniquely permits writes to at least ch.len_utf8() bytes. The
/// returned prefix is initialized canonical UTF-8; no previous value is read.
#[inline]
pub(super) unsafe fn write_scalar(ch: char, destination: *mut u8) -> usize {
    let code = ch as u32;
    // SAFETY: each branch writes exactly its returned scalar width. Every
    // offset is smaller than that width. Kani checks all Unicode scalar values.
    unsafe {
        if code < 0x80 {
            destination.write(code as u8);
            1
        } else if code < 0x800 {
            destination.write(((code >> 6) | 0xc0) as u8);
            destination.add(1).write(((code & 0x3f) | 0x80) as u8);
            2
        } else if code < 0x10000 {
            destination.write(((code >> 12) | 0xe0) as u8);
            destination
                .add(1)
                .write((((code >> 6) & 0x3f) | 0x80) as u8);
            destination.add(2).write(((code & 0x3f) | 0x80) as u8);
            3
        } else {
            destination.write(((code >> 18) | 0xf0) as u8);
            destination
                .add(1)
                .write((((code >> 12) & 0x3f) | 0x80) as u8);
            destination
                .add(2)
                .write((((code >> 6) & 0x3f) | 0x80) as u8);
            destination.add(3).write(((code & 0x3f) | 0x80) as u8);
            4
        }
    }
}
