use std::hint::black_box;

#[inline(never)]
fn allocate(leak: bool) {
    let mut buffer = jzap::vec::Vec::<u8>::try_with_capacity(257).unwrap();
    buffer.try_extend_copy(&[0x7du8; 257]).unwrap();
    black_box(&mut buffer);
    if leak {
        std::mem::forget(buffer);
    }
}

#[inline(never)]
fn clear_stack() {
    let mut scratch = [0usize; 1024];
    black_box(&mut scratch);
}

fn main() {
    allocate(std::env::args().nth(1).as_deref() == Some("leak"));
    clear_stack();
}
