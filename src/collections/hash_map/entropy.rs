//! Default OS entropy for `RandomState` keys. The platform table and the
//! order of sources follow Rust 1.95.0 std's `library/std/src/sys/random`;
//! every outcome std panics on is returned as `Error::Entropy`. Only the Linux
//! source runs in this repository's native verification; the other platforms
//! are compiled and interpreted with Miri where possible and are validated
//! separately (docs/random-state.md). Platforms outside this table have no
//! default source and `RandomState::try_new` returns `Entropy { code: 0 }`.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use crate::Error;

fn keys(bytes: [u8; 16]) -> (u64, u64) {
    let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = bytes;
    (
        u64::from_ne_bytes([a, b, c, d, e, f, g, h]),
        u64::from_ne_bytes([i, j, k, l, m, n, o, p]),
    )
}

/// Linux and Android: `getrandom` with std's flag order, then `/dev/urandom`.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) use linux::os_seed;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux {
    use super::keys;
    use crate::Error;
    use core::sync::atomic::{AtomicBool, Ordering::Relaxed};

    /// Process-wide `getrandom` capability flags, as in std's Linux source. Each
    /// flag only changes from available to unavailable; any interleaving of the
    /// relaxed loads and stores selects a valid entropy source.
    struct SeedFlags {
        getrandom: AtomicBool,
        insecure: AtomicBool,
    }
    impl SeedFlags {
        const fn new() -> Self {
            Self {
                getrandom: AtomicBool::new(true),
                insecure: AtomicBool::new(true),
            }
        }
    }
    const GRND_NONBLOCK: u32 = 1;
    const GRND_INSECURE: u32 = 4;
    /// Linux `EPERM`; the value is the same on every Linux architecture.
    const EPERM: i32 = 1;

    #[cold]
    #[inline(never)]
    pub(in crate::collections::hash_map) fn os_seed() -> Result<(u64, u64), Error> {
        static FLAGS: SeedFlags = SeedFlags::new();
        let mut bytes = [0u8; 16];
        fill_seed(&mut bytes, &FLAGS, sys_getrandom, read_urandom)?;
        Ok(keys(bytes))
    }
    /// std's order: `GRND_INSECURE`, then `GRND_NONBLOCK` after `EINVAL`; `EINTR`
    /// retries; `EAGAIN` under `GRND_NONBLOCK`, `ENOSYS` or `EPERM` fall back to the
    /// device for the remaining bytes. Every other outcome std would panic on is
    /// returned as `Entropy`. No arithmetic: progress narrows the remaining slice.
    fn fill_seed(
        bytes: &mut [u8; 16],
        flags: &SeedFlags,
        mut getrandom: impl FnMut(&mut [u8], u32) -> Result<usize, std::io::Error>,
        device: impl FnOnce(&mut [u8]) -> Result<(), Error>,
    ) -> Result<(), Error> {
        use std::io::ErrorKind::{Interrupted, InvalidInput, Unsupported, WouldBlock};
        let mut rest: &mut [u8] = bytes;
        if flags.getrandom.load(Relaxed) {
            while !rest.is_empty() {
                let insecure = flags.insecure.load(Relaxed);
                let mode = if insecure {
                    GRND_INSECURE
                } else {
                    GRND_NONBLOCK
                };
                match getrandom(rest, mode) {
                    // Zero would make no progress; more than requested breaks the
                    // system-call contract. std loops or panics; both are refused.
                    Ok(0) => return Err(Error::Entropy { code: 0 }),
                    Ok(written) => {
                        rest = match core::mem::take(&mut rest).get_mut(written..) {
                            Some(tail) => tail,
                            None => return Err(Error::Entropy { code: 0 }),
                        }
                    }
                    Err(error) => match error.kind() {
                        Interrupted => {}
                        InvalidInput if insecure => flags.insecure.store(false, Relaxed),
                        WouldBlock if !insecure => break,
                        _ if error.kind() == Unsupported || error.raw_os_error() == Some(EPERM) => {
                            flags.getrandom.store(false, Relaxed);
                            break;
                        }
                        _ => {
                            return Err(Error::Entropy {
                                code: error.raw_os_error().unwrap_or(0),
                            });
                        }
                    },
                }
            }
        }
        if rest.is_empty() {
            Ok(())
        } else {
            device(rest)
        }
    }
    fn sys_getrandom(buffer: &mut [u8], flags: u32) -> Result<usize, std::io::Error> {
        unsafe extern "C" {
            fn getrandom(buffer: *mut core::ffi::c_void, length: usize, flags: u32) -> isize;
        }
        // SAFETY: the pointer and length describe the live, writable `buffer`; the
        // kernel writes at most `length` bytes. Neither flag waits for entropy.
        let written = unsafe { getrandom(buffer.as_mut_ptr().cast(), buffer.len(), flags) };
        usize::try_from(written).map_err(|_| std::io::Error::last_os_error())
    }
    /// Opens, reads and closes `/dev/urandom` with direct system calls (std keeps
    /// the file open in a `OnceLock`). `std::fs::File` is not used: its descriptor
    /// and `read_exact` paths retain std-internal panic branches in the final link.
    /// `EINTR` retries as in std; end of file or an oversize count is refused.
    fn read_urandom(mut rest: &mut [u8]) -> Result<(), Error> {
        use std::io::ErrorKind::Interrupted;
        unsafe extern "C" {
            fn open(path: *const core::ffi::c_char, flags: i32, ...) -> i32;
            fn read(fd: i32, buffer: *mut core::ffi::c_void, length: usize) -> isize;
            fn close(fd: i32) -> i32;
        }
        const O_RDONLY: i32 = 0;
        #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
        const O_CLOEXEC: i32 = 0x0040_0000;
        #[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
        const O_CLOEXEC: i32 = 0o2_000_000;
        let entropy = |error: std::io::Error| Error::Entropy {
            code: error.raw_os_error().unwrap_or(0),
        };
        let fd = loop {
            // SAFETY: the path is a static NUL-terminated string; no mode argument
            // is read without O_CREAT.
            let fd = unsafe { open(c"/dev/urandom".as_ptr(), O_RDONLY | O_CLOEXEC) };
            if fd >= 0 {
                break fd;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != Interrupted {
                return Err(entropy(error));
            }
        };
        let result = loop {
            if rest.is_empty() {
                break Ok(());
            }
            // SAFETY: the pointer and length describe the live, writable `rest`;
            // the kernel writes at most `length` bytes.
            let read_bytes = unsafe { read(fd, rest.as_mut_ptr().cast(), rest.len()) };
            match usize::try_from(read_bytes) {
                Ok(0) => break Err(Error::Entropy { code: 0 }),
                Ok(written) => match core::mem::take(&mut rest).get_mut(written..) {
                    Some(tail) => rest = tail,
                    None => break Err(Error::Entropy { code: 0 }),
                },
                Err(_) => {
                    let error = std::io::Error::last_os_error();
                    if error.kind() != Interrupted {
                        break Err(entropy(error));
                    }
                }
            }
        };
        // SAFETY: `fd` was returned by `open` above and is closed exactly once. As
        // in std's descriptor drop, a close error does not invalidate read bytes.
        unsafe { close(fd) };
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Error as IoError;
        use std::vec::Vec;

        /// One scripted system-call outcome: written bytes or an errno.
        #[derive(Clone, Copy)]
        enum Call {
            Wrote(usize),
            Errno(i32),
        }
        struct Run {
            result: Result<(), Error>,
            modes: Vec<u32>,
            device: Option<usize>,
            bytes: [u8; 16],
        }
        fn run(flags: &SeedFlags, script: &[Call], device: Result<(), Error>) -> Run {
            let mut bytes = [0u8; 16];
            let mut modes = Vec::new();
            let mut step = 0;
            let mut device_len = None;
            let result = fill_seed(
                &mut bytes,
                flags,
                |buffer, mode| {
                    modes.push(mode);
                    let call = script[step];
                    step += 1;
                    match call {
                        Call::Wrote(n) => {
                            for b in buffer.iter_mut().take(n) {
                                *b = 0xA5;
                            }
                            Ok(n)
                        }
                        Call::Errno(code) => Err(IoError::from_raw_os_error(code)),
                    }
                },
                |rest| {
                    device_len = Some(rest.len());
                    rest.fill(0x5A);
                    device
                },
            );
            assert_eq!(step, script.len(), "unused scripted calls");
            Run {
                result,
                modes,
                device: device_len,
                bytes,
            }
        }
        const EINTR: i32 = 4;
        const EAGAIN: i32 = 11;
        const EFAULT: i32 = 14;
        const EINVAL: i32 = 22;
        const ENOSYS: i32 = 38;

        #[test]
        fn insecure_partial_reads_fill_all_bytes() {
            let flags = SeedFlags::new();
            let r = run(
                &flags,
                &[Call::Wrote(5), Call::Errno(EINTR), Call::Wrote(11)],
                Ok(()),
            );
            assert_eq!(r.result, Ok(()));
            assert_eq!(r.modes, [GRND_INSECURE; 3]);
            assert_eq!((r.device, r.bytes), (None, [0xA5; 16]));
        }
        #[test]
        fn einval_under_insecure_switches_to_nonblock_for_the_process() {
            let flags = SeedFlags::new();
            let r = run(&flags, &[Call::Errno(EINVAL), Call::Wrote(16)], Ok(()));
            assert_eq!(
                (r.result, r.modes),
                (Ok(()), [GRND_INSECURE, GRND_NONBLOCK].into())
            );
            assert!(!flags.insecure.load(Relaxed) && flags.getrandom.load(Relaxed));
            let again = run(&flags, &[Call::Wrote(16)], Ok(()));
            assert_eq!(again.modes, [GRND_NONBLOCK]);
            // EINVAL is only expected for the insecure flag.
            let refused = run(&flags, &[Call::Errno(EINVAL)], Ok(()));
            assert_eq!(refused.result, Err(Error::Entropy { code: EINVAL }));
        }
        #[test]
        fn eagain_under_nonblock_falls_back_to_device_for_remaining_bytes() {
            let flags = SeedFlags::new();
            flags.insecure.store(false, Relaxed);
            let r = run(&flags, &[Call::Wrote(3), Call::Errno(EAGAIN)], Ok(()));
            assert_eq!((r.result, r.device), (Ok(()), Some(13)));
            assert_eq!(r.bytes[..3], [0xA5; 3]);
            assert_eq!(r.bytes[3..], [0x5A; 13]);
            assert!(
                flags.getrandom.load(Relaxed),
                "EAGAIN does not disable getrandom"
            );
            // EAGAIN under GRND_INSECURE is not an expected outcome.
            let insecure = SeedFlags::new();
            let refused = run(&insecure, &[Call::Errno(EAGAIN)], Ok(()));
            assert_eq!(
                (refused.result, refused.device),
                (Err(Error::Entropy { code: EAGAIN }), None)
            );
        }
        #[test]
        fn enosys_and_eperm_disable_getrandom_for_the_process() {
            for code in [ENOSYS, EPERM] {
                let flags = SeedFlags::new();
                let r = run(&flags, &[Call::Errno(code)], Ok(()));
                assert_eq!(
                    (r.result, r.device, r.bytes),
                    (Ok(()), Some(16), [0x5A; 16])
                );
                assert!(!flags.getrandom.load(Relaxed));
                let next = run(&flags, &[], Ok(()));
                assert_eq!((next.modes.len(), next.device), (0, Some(16)));
            }
        }
        #[test]
        fn unexpected_outcomes_are_returned_without_publishing() {
            let flags = SeedFlags::new();
            let other = run(&flags, &[Call::Wrote(4), Call::Errno(EFAULT)], Ok(()));
            assert_eq!(
                (other.result, other.device),
                (Err(Error::Entropy { code: EFAULT }), None)
            );
            assert!(flags.getrandom.load(Relaxed) && flags.insecure.load(Relaxed));
            let zero = run(&flags, &[Call::Wrote(0)], Ok(()));
            assert_eq!(zero.result, Err(Error::Entropy { code: 0 }));
            let over = run(&flags, &[Call::Wrote(10), Call::Wrote(7)], Ok(()));
            assert_eq!(over.result, Err(Error::Entropy { code: 0 }));
            let disabled = SeedFlags::new();
            disabled.getrandom.store(false, Relaxed);
            let device = run(&disabled, &[], Err(Error::Entropy { code: 13 }));
            assert_eq!(device.result, Err(Error::Entropy { code: 13 }));
        }
        #[test]
        fn actual_sources_produce_keys() {
            assert!(os_seed().is_ok());
            let mut buffer = [0u8; 16];
            let written = sys_getrandom(&mut buffer, GRND_NONBLOCK).unwrap();
            assert!(written > 0 && written <= 16);
        }
        /// Miri isolation has no device files; the descriptor path runs natively.
        #[cfg(not(miri))]
        #[test]
        fn actual_device_fallback_reads() {
            let mut first = [0u8; 16];
            let mut second = [0u8; 16];
            read_urandom(&mut first).unwrap();
            read_urandom(&mut second).unwrap();
            assert_ne!(first, second);
        }
    }
}

/// Windows 10 and later: `ProcessPrng`, documented to always succeed; std only
/// debug-asserts its result. A `FALSE` result is returned as `Entropy`.
#[cfg(all(target_os = "windows", not(target_vendor = "win7")))]
pub(super) fn os_seed() -> Result<(u64, u64), Error> {
    #[cfg_attr(
        target_arch = "x86",
        link(
            name = "bcryptprimitives",
            kind = "raw-dylib",
            import_name_type = "undecorated"
        )
    )]
    #[cfg_attr(
        not(target_arch = "x86"),
        link(name = "bcryptprimitives", kind = "raw-dylib")
    )]
    unsafe extern "system" {
        fn ProcessPrng(data: *mut u8, length: usize) -> i32;
    }
    let mut bytes = [0u8; 16];
    // SAFETY: the pointer and length describe the live, writable `bytes`.
    let succeeded = unsafe { ProcessPrng(bytes.as_mut_ptr(), bytes.len()) };
    if succeeded == 0 {
        return Err(Error::Entropy { code: 0 });
    }
    Ok(keys(bytes))
}

/// Apple platforms (macOS, iOS, tvOS, watchOS, visionOS):
/// `CCRandomGenerateBytes`; std asserts `kCCSuccess` (0). Any other status is
/// returned as `Entropy { code: status }`.
#[cfg(target_vendor = "apple")]
pub(super) fn os_seed() -> Result<(u64, u64), Error> {
    unsafe extern "C" {
        fn CCRandomGenerateBytes(bytes: *mut core::ffi::c_void, count: usize) -> i32;
    }
    let mut bytes = [0u8; 16];
    // SAFETY: the pointer and length describe the live, writable `bytes`.
    let status = unsafe { CCRandomGenerateBytes(bytes.as_mut_ptr().cast(), bytes.len()) };
    if status != 0 {
        return Err(Error::Entropy { code: status });
    }
    Ok(keys(bytes))
}

/// BSD family, Haiku, illumos and Solaris: `arc4random_buf`, which has no
/// failure result (std calls it the same way).
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "haiku",
    target_os = "illumos",
    target_os = "solaris"
))]
pub(super) fn os_seed() -> Result<(u64, u64), Error> {
    #[cfg_attr(target_os = "haiku", link(name = "bsd"))]
    unsafe extern "C" {
        fn arc4random_buf(buffer: *mut core::ffi::c_void, length: usize);
    }
    let mut bytes = [0u8; 16];
    // SAFETY: the pointer and length describe the live, writable `bytes`.
    unsafe { arc4random_buf(bytes.as_mut_ptr().cast(), bytes.len()) };
    Ok(keys(bytes))
}

#[cfg(all(
    test,
    any(
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
mod platform_tests {
    #[test]
    fn platform_source_produces_distinct_keys() {
        let first = super::os_seed().unwrap();
        let second = super::os_seed().unwrap();
        assert_ne!(first, second);
    }
}
