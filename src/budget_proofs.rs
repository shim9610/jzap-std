//! Actual accounting transitions over every valid machine-word state.
use super::*;

fn state() -> MemoryBudget {
    let limit: usize = kani::any();
    let occupied: usize = kani::any();
    let available: usize = kani::any();
    kani::assume(occupied <= limit && available <= limit - occupied);
    MemoryBudget {
        limit,
        occupied: Cell::new(occupied),
        available: Cell::new(available),
    }
}

fn invariant(b: &MemoryBudget) -> bool {
    b.occupied() <= b.limit && b.available() <= b.limit - b.occupied()
}

#[kani::proof]
#[kani::solver(cadical)]
fn reservation_and_commit_preserve_every_valid_state() {
    let b = state();
    let available = b.available();
    let occupied = b.occupied();
    let bytes: usize = kani::any();
    match b.reserve(bytes) {
        Err(_) => {
            assert!(bytes > available);
            assert!(b.available() == available && b.occupied() == occupied);
        }
        Ok(r) => {
            assert!(bytes <= available && invariant(&b));
            if kani::any() {
                drop(r);
            } else {
                let charge = r.commit();
                assert!(invariant(&b));
                assert!(b.occupied() == occupied + bytes);
                drop(charge);
            }
            assert!(b.available() == available && b.occupied() == occupied);
        }
    }
    assert!(invariant(&b));
}

#[kani::proof]
#[kani::solver(cadical)]
fn release_preserves_every_valid_owned_charge_partition() {
    let b = state();
    let occupied = b.occupied();
    let available = b.available();
    let bytes: usize = kani::any();
    kani::assume(bytes <= occupied);
    // Model one owned charge within an arbitrary total. This is an explicit
    // ownership precondition, not a claim that every storage caller is proved.
    drop(Charge { budget: &b, bytes });
    assert!(invariant(&b));
    assert!(b.occupied() == occupied - bytes);
    assert!(b.available() == available + bytes);
    assert!(b.reserved() == b.limit - available - occupied);
}

#[kani::proof]
#[kani::solver(cadical)]
fn every_other_owner_transition_preserves_existing_entitlements() {
    let b = state();
    let owned: usize = kani::any();
    let pending: usize = kani::any();
    let bytes: usize = kani::any();
    kani::assume(owned <= b.occupied() && pending <= b.reserved());
    match kani::any::<u8>() {
        0 => {
            if let Ok(reservation) = b.reserve(bytes) {
                // The new reservation represents another still-live owner.
                core::mem::forget(reservation);
            }
        }
        1 => {
            if bytes <= b.reserved() - pending {
                core::mem::forget(Reservation { budget: &b, bytes }.commit());
            }
        }
        2 => {
            if bytes <= b.reserved() - pending {
                drop(Reservation { budget: &b, bytes });
            }
        }
        _ => {
            if bytes <= b.occupied() - owned {
                drop(Charge { budget: &b, bytes });
            }
        }
    }
    assert!(invariant(&b));
    assert!(owned <= b.occupied() && pending <= b.reserved());
}
