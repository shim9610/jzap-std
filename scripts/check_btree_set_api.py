#!/usr/bin/env python3
"""Fail closed on pinned stable BTreeSet API drift; declarations are not proofs."""
import hashlib,json,re,subprocess
from pathlib import Path
from check_shared_api import inventory
ROOT=Path(__file__).resolve().parents[1]
MAPPING={name:name for name in ['new','clear','contains','get','is_disjoint','is_subset','is_superset','first','last','pop_first','pop_last','remove','take','iter','len','is_empty','difference','intersection','union','symmetric_difference']}
MAPPING.update({name:'try_'+name for name in ['range','insert','replace','retain','append','split_off','extract_if']})
def main():
    version=subprocess.check_output(['rustc','--version'],text=True).strip();assert version.startswith('rustc 1.95.0 ')
    path=Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())/'lib/rustlib/src/rust/library/alloc/src/collections/btree/set.rs'
    reference=inventory(path);assert set(reference['stable'])==set(MAPPING),(reference['stable'],MAPPING)
    files=[ROOT/'src/collections/btree_set.rs',*sorted((ROOT/'src/collections/btree_set').glob('*.rs'))]
    product='\n'.join(p.read_text() for p in files)
    for name in MAPPING.values():assert re.search(r'pub (?:(?:const|unsafe) )*fn '+name+r'[<(]',product),name
    assert not re.search(r'impl[^\n]*\b(?:Clone|Index|FromIterator|Extend|BitOr|BitAnd|BitXor|Sub)\s*(?:<|for)[^\n]*BTreeSet',product),'implicit allocation/failure path'
    out=ROOT/'target/verification/btree-set';out.mkdir(parents=True,exist_ok=True)
    (out/'api.json').write_text(json.dumps(dict(status='surface-present',rustc=version,reference=reference,mapping=MAPPING,
        traits='TryClone/TryCloneIn/TryDefault, explicit construction/extension and owned algebra, comparison/hash/format, borrowed/owning fallible iterators',
        scope='Declarations only; independent contracts, safety checks and equal-work benchmarks determine completion.'),indent=2)+'\n')
    print(len(MAPPING),'stable BTreeSet names mapped; nightly Entry/cursor APIs excluded.')
if __name__=='__main__':main()
