#!/usr/bin/env python3
"""Pinned HashMap and Entry declaration mapping; no semantic proof claim."""
import json,re,subprocess
from pathlib import Path
from check_shared_api import inventory
ROOT=Path(__file__).resolve().parents[1]
FALLIBLE={'new','with_capacity','with_capacity_and_hasher','reserve','try_reserve','shrink_to','shrink_to_fit','insert','retain','extract_if','get_disjoint_mut','and_modify','insert_entry','or_default','or_insert','or_insert_with','or_insert_with_key'}
def main():
 version=subprocess.check_output(['rustc','--version'],text=True).strip();assert version.startswith('rustc 1.95.0 ')
 reference=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/std/src/collections/hash/map.rs'
 refs=inventory(reference)
 expected=set('and_modify capacity clear contains_key drain entry extract_if get get_disjoint_mut get_disjoint_unchecked_mut get_key_value get_mut hasher insert insert_entry into_key into_keys into_mut into_values is_empty iter iter_mut key keys len new or_default or_insert or_insert_with or_insert_with_key remove remove_entry reserve retain shrink_to shrink_to_fit try_reserve values values_mut with_capacity with_capacity_and_hasher with_hasher'.split())
 assert set(refs['stable'])==expected,refs['stable']
 mapping={name:('try_reserve' if name=='try_reserve' else 'try_'+name if name in FALLIBLE else name) for name in expected}
 texts='\n'.join(p.read_text() for p in [ROOT/'src/collections/hash_map.rs',*sorted((ROOT/'src/collections/hash_map').glob('*.rs'))] if not p.name.endswith('proofs.rs'))
 for name in mapping.values():assert re.search(r'pub (?:(?:unsafe|const) )*fn '+name+r'[<(]',texts),name
 assert not re.search(r'impl[^\n]*\b(?:Index|FromIterator|Extend)\s*(?:<|for)',texts)
 out=ROOT/'target/verification/hash-map';out.mkdir(parents=True,exist_ok=True)
 (out/'api.json').write_text(json.dumps(dict(reference=refs,mapping=mapping,scope='Combined map/Entry method names, declaration presence only; iterator and trait contracts are tested separately.'),indent=2)+'\n')
 print(f'{len(mapping)} unique pinned stable HashMap/Entry names mapped; not a semantic support certificate.')
if __name__=='__main__':main()
