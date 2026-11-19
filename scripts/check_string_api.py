#!/usr/bin/env python3
"""Fail closed on the pinned stable String inherent API inventory."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MAPPING = {
    'new': 'new', 'with_capacity': 'try_with_capacity',
    'from_utf8': 'try_from_utf8', 'from_utf8_lossy': 'try_from_utf8_lossy',
    'from_utf16': 'try_from_utf16', 'from_utf16_lossy': 'try_from_utf16_lossy',
    'into_raw_parts': 'into_raw_parts', 'from_raw_parts': 'from_raw_parts',
    'from_utf8_unchecked': 'from_utf8_unchecked', 'into_bytes': 'into_bytes',
    'as_str': 'as_str', 'as_mut_str': 'as_mut_str', 'push_str': 'try_push_str',
    'extend_from_within': 'try_extend_from_within', 'capacity': 'capacity',
    'reserve': 'try_reserve', 'reserve_exact': 'try_reserve_exact',
    'try_reserve': 'try_reserve', 'try_reserve_exact': 'try_reserve_exact',
    'shrink_to_fit': 'try_shrink_to_fit', 'shrink_to': 'try_shrink_to',
    'push': 'try_push', 'as_bytes': 'as_bytes', 'truncate': 'truncate',
    'pop': 'pop', 'remove': 'remove', 'retain': 'try_retain',
    'insert': 'try_insert', 'insert_str': 'try_insert_str', 'as_mut_vec': 'as_mut_vec',
    'len': 'len', 'is_empty': 'is_empty', 'split_off': 'try_split_off',
    'clear': 'clear', 'drain': 'try_drain', 'replace_range': 'try_replace_range',
    'into_boxed_str': 'try_into_boxed_str', 'leak': 'leak',
}

def main():
    version = subprocess.check_output(['rustc', '--version'], text=True).strip()
    assert version.startswith('rustc 1.95.0 '), version
    sysroot = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
    source = (sysroot / 'lib/rustlib/src/rust/library/alloc/src/string.rs').read_text()
    # Later impls contain FromUtf8Error/Drain methods with overlapping names.
    owner = source[:source.index('impl FromUtf8Error')]
    stable, unstable = {}, []
    for match in re.finditer(r'^    pub (?:(?:unsafe|const) )*fn (\w+)', owner, re.M):
        attrs = list(re.finditer(r'#\[(stable|unstable)\((.*?)\)\]',
                                owner[max(0, match.start()-900):match.start()], re.S))
        assert attrs, match[1]
        if attrs[-1][1] == 'stable': stable[match[1]] = attrs[-1][2]
        else: unstable.append(match[1])
    assert set(stable) == set(MAPPING), (set(stable)-set(MAPPING), set(MAPPING)-set(stable))
    product = '\n'.join(p.read_text() for p in [ROOT/'src/string.rs', *sorted((ROOT/'src/string').glob('*.rs'))] if not p.name.endswith('proofs.rs'))
    for method in MAPPING.values():
        assert re.search(r'pub (?:(?:unsafe|const) )*fn '+method+r'[<(]', product), method
    forbidden = [r'impl[^\n]*\b(?:Deref|Index|Clone|Add|AddAssign|Extend|FromIterator)\s*(?:<|for)']
    for pattern in forbidden: assert not re.search(pattern, product), 'implicit failure path'
    out = ROOT/'target/verification/string'; out.mkdir(parents=True, exist_ok=True)
    (out/'api.json').write_text(json.dumps(dict(rustc=version, stable_methods=MAPPING,
        reference_sha256=hashlib.sha256(source.encode()).hexdigest(), nightly_only=unstable), indent=2)+'\n')
    print(f'{len(stable)} stable String methods mapped, including const unsafe methods; {len(unstable)} nightly declarations separate.')

if __name__ == '__main__': main()
