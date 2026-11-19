//! Concrete std-enabled root for default hash-key acquisition. The checker walks
//! the linked call graph from `seed_link_root`; controls add known exit edges.
use jzap::collections::{HashMap, HashSet, hash_map::RandomState};
use std::hint::black_box;

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn seed_link_root() -> u32 {
    let mut status = 0;
    match RandomState::try_new() {
        Ok(state) => drop(black_box(state)),
        Err(error) => status |= 1 | black_box(error_code(error)),
    }
    match HashMap::<u64, u64>::try_new() {
        Ok(map) => drop(black_box(map)),
        Err(error) => status |= 2 | black_box(error_code(error)),
    }
    match HashSet::<u64>::try_new() {
        Ok(set) => drop(black_box(set)),
        Err(error) => status |= 4 | black_box(error_code(error)),
    }
    status | controls()
}
fn error_code(error: jzap::Error) -> u32 {
    match error {
        jzap::Error::Entropy { code } => (code as u32) << 8,
        _ => 0x8000_0000,
    }
}
#[inline(always)]
fn controls() -> u32 {
    #[allow(unused_mut)]
    let mut status = 0;
    #[cfg(feature = "panic-control")]
    {
        status |= black_box(None::<u32>).unwrap();
    }
    #[cfg(feature = "alloc-control")]
    {
        status |= u32::from(*black_box(Box::new(black_box(0u8))));
    }
    #[cfg(feature = "std-file-control")]
    {
        use std::io::Read;
        let mut bytes = [0u8; 16];
        if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
            status |= u32::from(file.read_exact(&mut bytes).is_err());
        }
        status |= u32::from(black_box(bytes)[0] & 0);
    }
    status
}
fn main() -> std::process::ExitCode {
    // Repeated calls cover the initialized path; a refusal is a valid result.
    let mut status = 0;
    for _ in 0..4 {
        status |= seed_link_root();
    }
    println!("{{\"status\":{status}}}");
    std::process::ExitCode::SUCCESS
}
