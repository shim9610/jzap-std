"""Audit budget/storage identity and order; never translate a product body."""
import re
from pathlib import Path
from arithmetic_projection import validate_bounded
from verify_edit_loops import block


def check(root):
    root=Path(root);validate_bounded(root)
    source=(root/'src/budget.rs').read_text()
    # Reserve/commit/retain consume the same token. There is no public raw
    # constructor for a reservation/charge; the sealed policy preserves identity.
    reservation=block(source,'pub fn commit(')
    assert 'budget: self.budget' in reservation and 'let bytes = self.bytes;' in reservation
    assert 'core::mem::forget(self);' in reservation
    charge_drop=block(source[source.index('impl Drop for Charge'):],'fn drop(')
    assert re.sub(r'\s+','',charge_drop)=='self.budget.release(self.bytes);'
    assert 'pub trait AllocationPolicy: sealed::Sealed + crate::TryClone' in source
    storage=(root/'src/storage.rs').read_text()
    alloc=block(storage,'fn allocate_impl(');resize=block(storage,'unsafe fn reallocate(')
    release=block(storage,'unsafe fn deallocate(')
    for body,tokens in [(alloc,['budget.reserve(layout.size())?','.ok_or(Error::Allocation','r.commit().retain();','Ok(ptr)']),
                        (resize,['budget.reserve(new.size())?','alloc::alloc::realloc','reservation.commit().retain();','budget.release(old.size());','Ok(result)']),
                        (release,['alloc::alloc::dealloc(ptr.as_ptr(), layout)','budget.release(layout.size());'])]:
        indexes=[body.rindex(x) if x.startswith('Ok(') else body.index(x) for x in tokens];assert indexes==sorted(indexes)
    assert 'if old.size() == new.size()' in resize and 'let result = allocate(new, budget)?;' in resize
    assert 'deallocate(ptr, old, budget)' in resize
    return dict(kind='budget/storage identity and ordering audit',
                numeric='src/budget_numeric.rs', caller_contract='actual Rust Kani plus direct numeric dual specifications')
