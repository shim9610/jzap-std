#!/usr/bin/env python3
"""Fail closed on drift in the pinned stable ordered-map/Entry surface."""
import hashlib,json,re,subprocess
from pathlib import Path
from check_shared_api import inventory
ROOT=Path(__file__).resolve().parents[1]
MAPPING={name:name for name in ['new','clear','contains_key','entry','first_entry','first_key_value','get','get_key_value','get_mut','into_keys','into_values','is_empty','iter','iter_mut','keys','last_entry','last_key_value','len','pop_first','pop_last','remove','remove_entry','values','values_mut']}
MAPPING.update({name:'try_'+name for name in ['append','extract_if','insert','range','range_mut','retain','split_off']})
ENTRY={name:('try_'+name if name in ['and_modify','insert_entry','or_default','or_insert','or_insert_with','or_insert_with_key'] else name) for name in ['and_modify','get','get_mut','insert','insert_entry','into_key','into_mut','key','or_default','or_insert','or_insert_with','or_insert_with_key','remove','remove_entry']}
def main():
    version=subprocess.check_output(['rustc','--version'],text=True).strip();assert version.startswith('rustc 1.95.0 ')
    base=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/alloc/src/collections/btree'
    refs={name:inventory(base/file) for name,file in [('map','map.rs'),('entry','map/entry.rs')]}
    texts={name:'\n'.join(p.read_text() for p in ([ROOT/'src/collections/btree_map.rs',*sorted((ROOT/'src/collections/btree_map').glob('*.rs'))]) if not p.name.endswith('proofs.rs')) for name in ['map','entry']}
    for name,mapping in [('map',MAPPING),('entry',ENTRY)]:
        assert set(refs[name]['stable'])==set(mapping),(name,refs[name]['stable'])
        for counterpart in mapping.values():assert re.search(r'pub (?:(?:const|unsafe) )*fn '+counterpart+r'[<(]',texts[name]),counterpart
    assert not re.search(r'impl[^\n]*\b(?:Clone|Index|FromIterator|Extend)\s*(?:<|for)[^\n]*BTreeMap',texts['map']),'implicit failure path'
    out=ROOT/'target/verification/btree';out.mkdir(parents=True,exist_ok=True)
    report=dict(rustc=version,references=refs,mapping=MAPPING,entry_mapping=ENTRY,status='surface-present',
        scope='Public declarations only; independent implementation/contract/proof/performance gates determine support.',
        traits='TryClone/TryCloneIn/TryDefault; explicit fallible array/iterator construction and extension; comparison/hash/format; borrowed/owning fallible iteration adapters; no indexing or infallible allocation conversion')
    (out/'api.json').write_text(json.dumps(report,indent=2)+'\n')
    print('31 stable BTreeMap names and 14 Entry names mapped; no support claim from declaration presence.')
if __name__=='__main__':main()
