"""Storage kernel for area compositions.

Returns one Rust text: the pinned core predicates under the documented
conditions (pointer_predicates.py), the element-extent lemmas and the storage
model. An area model appended to it calls `offset_storage`, `slice_caller` and
the copy callers with the indices its product kernels compute, so each modelled
pointer operation ends in the documented condition of its core call. The text
has no `main`; the area model supplies it.
"""
from pathlib import Path
import re
from pointer_predicates import substituted

ROOT = Path(__file__).resolve().parents[1]
FILES = [ROOT / "tools/memory-proof" / name for name in
         ("core-contracts.verus.rs", "pointer-checks.verus.rs", "extent.verus.rs", "storage.verus.rs")]
INPUTS = FILES + [ROOT / "scripts/pointer_predicates.py", ROOT / "scripts/storage_kernel.py"]


def kernel():
    pointer = re.sub(r"fn main\(\)\s*\{\}", "", substituted())
    return pointer + FILES[2].read_text() + FILES[3].read_text()
