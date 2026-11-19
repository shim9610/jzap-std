#!/usr/bin/env python3
"""Pinned HashSet declarations; presence is separate from semantic evidence."""
import json,re,subprocess
from pathlib import Path
from check_shared_api import inventory
ROOT=Path(__file__).resolve().parents[1]
FALLIBLE=set('new with_capacity with_capacity_and_hasher insert replace reserve retain extract_if shrink_to shrink_to_fit'.split())
def main():
 assert subprocess.check_output(['rustc','--version'],text=True).startswith('rustc 1.95.0 ')
 ref=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/std/src/collections/hash/set.rs'
 refs=inventory(ref)
 expected=set('capacity clear contains difference drain extract_if get hasher insert intersection is_disjoint is_empty is_subset is_superset iter len new remove replace reserve retain shrink_to shrink_to_fit symmetric_difference take try_reserve union with_capacity with_capacity_and_hasher with_hasher'.split())
 assert set(refs['stable'])==expected,refs['stable']
 mapping={k:('try_reserve' if k=='try_reserve' else 'try_'+k if k in FALLIBLE else k) for k in expected}
 files=[ROOT/'src/collections/hash_set.rs',*sorted((ROOT/'src/collections/hash_set').glob('*.rs'))]
 text='\n'.join(p.read_text() for p in files if not p.name.endswith('proofs.rs'))
 for name in mapping.values():assert re.search(r'pub (?:(?:unsafe|const) )*fn '+name+r'[<(]',text),name
 assert not re.search(r'impl[^\n]*\b(?:Default|Clone|Index|FromIterator|Extend|BitOr|BitAnd|BitXor|Sub)\s*(?:<|for)\s+HashSet',text)
 out=ROOT/'target/verification/hash-set';out.mkdir(parents=True,exist_ok=True)
 (out/'api.json').write_text(json.dumps(dict(reference=refs,mapping=mapping,traits='TryDefault/TryClone/TryCloneIn, explicit array/fallible/copy-reference extension, owned algebra, equality/debug, standard and fallible iteration. Nightly Entry/get_or_insert APIs are outside the stable scope.'),indent=2)+'\n')
 print(f'{len(mapping)} pinned stable HashSet names mapped; semantic checks are separate.')
if __name__=='__main__':main()
