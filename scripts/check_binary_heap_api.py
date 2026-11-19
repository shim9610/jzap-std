#!/usr/bin/env python3
"""Pinned stable BinaryHeap API inventory; semantic evidence is separate."""
import json,re,subprocess
from pathlib import Path
from check_shared_api import inventory
ROOT=Path(__file__).resolve().parents[1]
def main():
 assert subprocess.check_output(['rustc','--version'],text=True).startswith('rustc 1.95.0 ')
 ref=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/alloc/src/collections/binary_heap/mod.rs'
 refs=inventory(ref)
 expected=set('append as_slice capacity clear drain into_sorted_vec into_vec is_empty iter len new peek peek_mut pop push reserve reserve_exact retain shrink_to shrink_to_fit try_reserve try_reserve_exact with_capacity'.split())
 assert set(refs['stable'])==expected,refs['stable']
 fallible=set('append push reserve reserve_exact retain shrink_to shrink_to_fit with_capacity'.split())
 mapping={k:'try_'+k if k in fallible else k for k in expected}
 text='\n'.join(p.read_text() for p in [ROOT/'src/collections/binary_heap.rs',*sorted((ROOT/'src/collections/binary_heap').glob('*.rs'))] if not p.name.endswith('proofs.rs'))
 for name in mapping.values():assert re.search(r'pub (?:(?:unsafe|const) )*fn '+name+r'[<(]',text),name
 assert not re.search(r'impl[^\n]*\b(?:Clone|Index|FromIterator|Extend)\s*(?:<|for)\s+BinaryHeap',text)
 out=ROOT/'target/verification/binary-heap';out.mkdir(parents=True,exist_ok=True)
 (out/'api.json').write_text(json.dumps(dict(reference=refs,mapping=mapping,traits='Default/TryDefault, TryClone/TryCloneIn, Debug, Vec ownership conversions, fallible array/iterator construction, standard and fallible borrowed/consuming/draining iteration. Unstable sorted iterators, allocator_api, pop_if and refresh are excluded.'),indent=2)+'\n')
 print(f'{len(mapping)} pinned stable BinaryHeap names mapped; semantic checks separate.')
if __name__=='__main__':main()
