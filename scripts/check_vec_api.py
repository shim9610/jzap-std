#!/usr/bin/env python3
"""Check the pinned stable Vec inherent inventory against maintained adapters.

Requires rust-src for Rust 1.95. Raw inventory and hashes are local artifacts.
This structural check complements, and does not replace, behavioral tests.
"""
import hashlib,json,pathlib,re,subprocess
root=pathlib.Path(__file__).resolve().parents[1]
version=subprocess.check_output(['rustc','--version'],text=True).strip()
assert version.startswith('rustc 1.95.0 '),version
sysroot=pathlib.Path(subprocess.check_output(['rustc','--print','sysroot'],text=True).strip())
path=sysroot/'lib/rustlib/src/rust/library/alloc/src/vec/mod.rs'
source=path.read_text();stable={};nightly=[]
for m in re.finditer(r'^    pub (?:unsafe |const )?fn (\w+)',source,re.M):
 attrs=list(re.finditer(r'#\[(stable|unstable)\((.*?)\)\]',source[max(0,m.start()-600):m.start()],re.S))
 assert attrs,m[1]
 if attrs[-1][1]=='stable':stable[m[1]]=attrs[-1][2]
 else:nightly.append(m[1])
alternatives={
 'new':'new','with_capacity':'try_with_capacity','from_raw_parts':'from_raw_parts','into_raw_parts':'into_raw_parts',
 'push':'try_push','push_mut':'try_push_mut','capacity':'capacity','reserve':'try_reserve','reserve_exact':'try_reserve_exact',
 'try_reserve':'try_reserve','try_reserve_exact':'try_reserve_exact','shrink_to_fit':'try_shrink_to_fit','shrink_to':'try_shrink_to',
 'into_boxed_slice':'try_into_boxed_slice','truncate':'truncate','as_slice':'as_slice','as_mut_slice':'as_mut_slice',
 'as_ptr':'as_ptr','as_mut_ptr':'as_mut_ptr','set_len':'try_set_len','swap_remove':'swap_remove','insert':'try_insert','insert_mut':'try_insert_mut',
 'remove':'remove','retain':'try_retain','retain_mut':'try_retain_mut','dedup_by_key':'try_dedup_by_key','dedup_by':'try_dedup_by',
 'pop':'pop','pop_if':'try_pop_if','append':'try_append','drain':'try_drain','clear':'clear','len':'len','is_empty':'is_empty',
 'split_off':'try_split_off','resize_with':'try_resize_with','leak':'leak','spare_capacity_mut':'spare_capacity_mut',
 'resize':'try_resize_owned','extend_from_slice':'try_extend_cloned','extend_from_within':'try_extend_from_within',
 'into_flattened':'try_into_flattened','dedup':'dedup','splice':'try_splice','extract_if':'try_extract_if',
}
assert set(stable)==set(alternatives),(set(stable)-set(alternatives),set(alternatives)-set(stable))
product='\n'.join(p.read_text() for p in [root/'src/vec.rs',*sorted((root/'src/vec').glob('*.rs'))])
for name in alternatives.values():assert re.search(r'pub (?:unsafe |const )?fn '+name+r'[<(]',product),name
out=root/'target/verification/vec-completion';out.mkdir(parents=True,exist_ok=True)
(out/'api.json').write_text(json.dumps(dict(rustc=version,reference_sha256=hashlib.sha256(source.encode()).hexdigest(),stable_methods=alternatives,unstable_reference_only=nightly),indent=2))
print(f'{len(stable)} stable Vec inherent methods mapped; {len(nightly)} unstable declarations recorded separately.')
