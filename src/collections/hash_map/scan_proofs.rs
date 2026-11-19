//! An arbitrary valid control frontier, then one actual scanner transition.
use super::*;
fn remaining(group: usize, bits: u16, masks: [u16; 2]) -> usize {
    bits.count_ones() as usize
        + if group == 0 {
            masks[0].count_ones() as usize + masks[1].count_ones() as usize
        } else if group == 16 {
            masks[1].count_ones() as usize
        } else {
            0
        }
}
#[kani::proof]
#[kani::unwind(18)]
fn every_control_frontier_preserves_bounds_and_retirement() {
    let mut bytes: [u8; 48] = kani::any();
    for i in 0..16 {
        bytes[32 + i] = bytes[i];
    }
    let masks = unsafe {
        [
            !group::vacant(bytes.as_ptr()),
            !group::vacant(bytes.as_ptr().add(16)),
        ]
    };
    let group = (kani::any::<u8>() as usize % 3) * 16;
    let allowed = if group == 0 { 0 } else { masks[group / 16 - 1] };
    let bits = kani::any::<u16>() & allowed;
    let count = remaining(group, bits, masks);
    let mut scan = Scan {
        ctrl: NonNull::from(&mut bytes).cast(),
        mask: 31,
        group,
        bits,
        remaining: Checked::new(count),
    };
    // Exercise the fold fast step directly, without next's count guard.
    let result = if bits != 0 {
        Some(scan.take_cached())
    } else {
        scan.next()
    };
    match result {
        None => assert_eq!(count, 0),
        Some(index) => {
            assert!(index < 32);
            assert!(bytes[index] & 128 == 0);
            if group == 16 && index < 16 {
                assert!(bits & (1 << index) != 0);
            }
            if group == 32 {
                assert!(index >= 16);
                assert!(bits & (1 << (index - 16)) != 0);
            }
            assert_eq!(scan.remaining.get() + 1, count);
            assert!(scan.group <= 32 && scan.group % 16 == 0);
            let allowed = if scan.group == 0 {
                0
            } else {
                masks[scan.group / 16 - 1]
            };
            assert_eq!(scan.bits & !allowed, 0);
            assert_eq!(
                scan.remaining.get(),
                remaining(scan.group, scan.bits, masks)
            );
        }
    }
}
