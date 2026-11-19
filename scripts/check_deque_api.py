#!/usr/bin/env python3
"""Pinned VecDeque declarations; implementations are checked separately."""
import json,re,subprocess
from pathlib import Path
from check_shared_api import inventory
ROOT=Path(__file__).resolve().parents[1]
def main():
 assert subprocess.check_output(['rustc','--version'],text=True).startswith('rustc 1.95.0 ')
 ref=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/alloc/src/collections/vec_deque/mod.rs'
 refs=inventory(ref)
 expected=set('append as_mut_slices as_slices back back_mut binary_search binary_search_by binary_search_by_key capacity clear contains drain front front_mut get get_mut insert insert_mut is_empty iter iter_mut len make_contiguous new partition_point pop_back pop_back_if pop_front pop_front_if push_back push_back_mut push_front push_front_mut range range_mut remove reserve reserve_exact resize resize_with retain retain_mut rotate_left rotate_right shrink_to shrink_to_fit split_off swap swap_remove_back swap_remove_front truncate try_reserve try_reserve_exact with_capacity'.split())
 assert set(refs['stable'])==expected,refs['stable']
 fallible=set('append insert insert_mut pop_back_if pop_front_if push_back push_back_mut push_front push_front_mut reserve reserve_exact resize resize_with retain retain_mut shrink_to shrink_to_fit split_off with_capacity'.split())
 mapping={k:'try_'+k if k in fallible else k for k in expected}
 files=[ROOT/'src/deque.rs',*sorted((ROOT/'src/deque').glob('*.rs'))]
 text='\n'.join(p.read_text() for p in files if not p.name.endswith('proofs.rs'))
 for name in mapping.values():assert re.search(r'pub (?:(?:unsafe|const) )*fn '+name+r'[<(]',text),name
 assert not re.search(r'impl[^\n]*\b(?:Clone|Index|FromIterator|Extend)\s*(?:<|for)\s+VecDeque',text)
 out=ROOT/'target/verification/vec-deque';out.mkdir(parents=True,exist_ok=True)
 (out/'api.json').write_text(json.dumps(dict(reference=refs,mapping=mapping,traits='Default/TryDefault, TryClone/TryCloneIn, Debug, equality/order/hash, explicit array/iterator/Vec/std ownership conversions, standard and fallible bidirectional exact iteration.'),indent=2)+'\n')
 print(f'{len(mapping)} stable VecDeque names mapped; semantic evidence separate.')
if __name__=='__main__':main()
