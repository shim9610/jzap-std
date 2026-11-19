#!/usr/bin/env python3
"""Inventory pinned stable Rc/Arc/Weak methods and explicit jzap counterparts."""
import hashlib,json,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
MAPPING={
 'new':'try_new / Weak::new','new_cyclic':'try_new_cyclic','new_uninit':'try_new_uninit','new_zeroed':'try_new_zeroed',
 'new_uninit_slice':'try_new_uninit_slice','new_zeroed_slice':'try_new_zeroed_slice','assume_init':'assume_init',
 'pin':'try_pin','try_unwrap':'try_unwrap','into_inner':'into_inner','unwrap_or_clone':'try_unwrap_or_clone',
 'get_mut':'get_mut','make_mut':'try_make_mut','downgrade':'try_downgrade','upgrade':'try_upgrade',
 'strong_count':'strong_count','weak_count':'weak_count','ptr_eq':'ptr_eq','as_ptr':'as_ptr',
 'into_raw':'into_raw','from_raw':'from_raw','increment_strong_count':'try_increment_strong_count',
 'decrement_strong_count':'decrement_strong_count','downcast':'downcast',
}
def inventory(path):
 text=path.read_text();stable=set();unstable=set()
 for match in re.finditer(r'^\s+pub (?:(?:const|unsafe) )*fn (\w+)',text,re.M):
  attrs=list(re.finditer(r'#\[(stable|unstable)\((.*?)\)\]',text[:match.start()],re.S));assert attrs,match[1]
  (stable if attrs[-1][1]=='stable' else unstable).add(match[1])
 return dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),stable=sorted(stable),unstable=sorted(unstable))
def main():
 version=subprocess.check_output(['rustc','--version'],text=True).strip();assert version.startswith('rustc 1.95.0 ')
 root=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/alloc/src'
 references={name:inventory(root/file)for name,file in [('Rc/Weak','rc.rs'),('Arc/Weak','sync.rs')]}
 for reference in references.values():assert set(reference['stable'])==set(MAPPING),(reference['stable'],MAPPING)
 product='\n'.join(p.read_text()for p in [ROOT/'src/shared.rs',*sorted((ROOT/'src/shared').glob('*.rs'))]if not p.name.endswith('proofs.rs'))
 for declaration in set(MAPPING.values())-{'try_new / Weak::new'}:
  assert re.search(r'pub (?:(?:unsafe|const) )*fn '+declaration+r'[<(]',product),declaration
 assert 'pub const fn new()'in product and 'pub fn try_new('in product
 assert not re.search(r'impl[^\n]*\b(?:Clone|Index|DerefMut|FromIterator|Extend)\s*(?:<|for)',product),'implicit failure path'
 out=ROOT/'target/verification/shared';out.mkdir(parents=True,exist_ok=True)
 (out/'api.json').write_text(json.dumps(dict(rustc=version,references=references,mapping=MAPPING,
 raw_boundary='Raw ownership pointers retain the header and metadata; as_ptr returns the payload pointer. Weak as_ptr uses Option for empty state.',
 related_traits='TryClone, TryCloneIn, TryDefault; explicit array/slice, Box (including dyn), Vec, String/Cow/CStr/OsStr/Path and iterator conversions; checked array recovery, shared str-to-bytes, Error, comparison/hash/format/borrow, Unpin/unwind bounds and compiler-checked/pinned unsizing. str/slice borrows are explicit.'),indent=2)+'\n')
 print('24 distinct stable Rc/Weak and Arc/Weak method names mapped; nightly additions are outside the pinned stable scope.')
if __name__=='__main__':main()
