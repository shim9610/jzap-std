#!/usr/bin/env python3
"""Generate isolated B+ lookup controls from the current implementation."""

from pathlib import Path
import sys

repo = Path(sys.argv[1]).resolve()
dest = Path(sys.argv[2]).resolve()
if not dest.is_relative_to(repo / "target"):
    raise SystemExit("Generate the experiment under the ignored target directory")
(dest / "src").mkdir(parents=True, exist_ok=True)
bench_dir = repo / 'tools/throughput-analysis'
bplus_path = repo / 'src/collections/btree_map/view/bplus.rs'
source = bplus_path.read_text().split('#[cfg(kani)]')[0]
node_types = source[source.index("enum Contents"):source.index("/// Validates the visited path")]
route = source[source.index("#[inline]"):source.index("enum Contents")]
lookup = source[source.index("/// Validates the visited path"):]
search = bplus_path.parent.with_suffix('.rs').read_text().split('fn search_index', 1)[1].split('#[cfg(kani)]', 1)[0]
search = 'fn search_index' + search.replace('super::geometry::', '')

def replace_once(code, old, new):
    assert code.count(old) == 1, old
    return code.replace(old, new)

lazy = replace_once(lookup, 'path: [I; usize::BITS as usize],', 'path: [MaybeUninit<I>; usize::BITS as usize],')
lazy = replace_once(lazy, 'path: [root; usize::BITS as usize],', 'path: [MaybeUninit::uninit(); usize::BITS as usize],')
lazy = replace_once(lazy, 'history.contains(&child)', '''{
            // Every descent writes its current slot before increasing depth.
            let initialized = unsafe {
                core::slice::from_raw_parts(history.as_ptr().cast::<I>(), history.len())
            };
            initialized.contains(&child)
        }''')
lazy = replace_once(lazy, '*slot = id;', 'slot.write(id);')
no_cycle = replace_once(lazy, '''        let (slot, history) = self
            .path
            .get_mut(..=self.depth)
            .and_then(|path| path.split_last_mut())
            .ok_or(Error::InvalidStructure)?;
        if child == id || {
            // Every descent writes its current slot before increasing depth.
            let initialized = unsafe {
                core::slice::from_raw_parts(history.as_ptr().cast::<I>(), history.len())
            };
            initialized.contains(&child)
        } {
            return Err(Error::InvalidStructure);
        }
''', '')
for line in ['    path: [MaybeUninit<I>; usize::BITS as usize],\n', '            path: [MaybeUninit::uninit(); usize::BITS as usize],\n', '        slot.write(id);\n']:
    no_cycle = replace_once(no_cycle, line, '')
no_cycle = replace_once(no_cycle, '''return self.descend(id, index''', '''return self.descend(index''')
no_cycle = replace_once(no_cycle, '''        id: I,
        index: usize,''', '''        index: usize,''')
fences = lookup[lookup.index('            || self\n                .minimum'):lookup.index('\n        {\n            return Err(Error::InvalidStructure);', lookup.index('            || self\n                .minimum'))]
def omit_fences(code):
    code = replace_once(code, fences, '')
    for line in [
        '    minimum: Option<K>,\n', '    ceiling: Option<K>,\n',
        '            minimum: None,\n', '            ceiling: None,\n',
        '        let minimum = *minima.get(index).ok_or(Error::InvalidStructure)?;\n',
        '        let next = index.checked_add(1).ok_or(Error::InvalidStructure)?;\n',
        '        self.minimum = Some(minimum);\n',
        '        self.ceiling = minima.get(next).copied().or(self.ceiling);\n',
        '        minima: &[K],\n',
    ]:
        code = replace_once(code, line, '')
    code = replace_once(code, 'index, node.keys, children', 'index, children')
    code = replace_once(code, "    query: &'q Q,", "    query: &'q Q,\n    key: core::marker::PhantomData<K>,")
    return replace_once(code, '            query,', '            query,\n            key: core::marker::PhantomData,')
branchless = replace_once(lookup, 'child_index(node.keys, self.query)', 'branchless_child_index(node.keys, self.query)')
branchless_route = replace_once(route, 'fn child_index', 'fn branchless_child_index')
branchless_route = replace_once(branchless_route, '''        if key.borrow() <= query {
            base = middle;
        }''', '''        let advance = core::hint::select_unpredictable(key.borrow() <= query, half, 0);
        base = base.wrapping_add(advance);''')
lower_search = '''#[inline]
fn lower_search_leaf<K: Borrow<Q>, Q: ?Sized + Ord>(keys: &[K], query: &Q) -> Result<usize, usize> {
    let index = keys.partition_point(|key| key.borrow() < query);
    if keys.get(index).is_some_and(|key| key.borrow() == query) { Ok(index) } else { Err(index) }
}
'''
variants = {
    'lazy': lazy,
    'eager_matched': replace_once(lazy, '[MaybeUninit::uninit(); usize::BITS as usize]', '[MaybeUninit::new(root); usize::BITS as usize]'),
    'no_cycle': no_cycle,
    'no_fences': omit_fences(lazy),
    'trusted': omit_fences(no_cycle),
    'branchless': branchless,
    'both_searches': replace_once(branchless, 'search_index(node.keys, self.query)?', 'lower_search_leaf(node.keys, self.query)'),
    'lazy_both_searches': replace_once(
        replace_once(lazy, 'child_index(node.keys, self.query)', 'branchless_child_index(node.keys, self.query)'),
        'search_index(node.keys, self.query)?', 'lower_search_leaf(node.keys, self.query)',
    ),
}
support = '''use brie_tree::{BTree, nonmax::NonMaxU64};
use jzap::{Error, collections::btree_map::view::{LookupStep, bplus::{Lookup, Node}}};
use core::{borrow::Borrow, mem::MaybeUninit};
use std::{collections::BTreeMap, hint::black_box, time::Instant};

#[inline] fn middle(low: usize, high: usize) -> usize { low.wrapping_add(high.wrapping_sub(low) / 2) }
#[inline] fn advance(low: usize, high: usize, middle: usize, less: bool) -> (usize, usize) {
    if less { (middle.wrapping_add(1), high) } else { (low, middle) }
}

mod experiments {
    use super::{advance, middle, Borrow, Error, LookupStep, MaybeUninit};
'''
exp = support + search + route + branchless_route + lower_search + node_types
bench = (bench_dir / 'src/bin/bplus.rs').read_text()
experiments_lookup = bench[bench.index('fn jzap_lookup'):bench.index('fn reference_lookup')].rstrip()
for name, code in variants.items():
    exp += '\n    pub(super) mod ' + name + ' {\n        use super::*;\n' + code
    fn = experiments_lookup.replace('fn jzap_lookup', '#[inline(always)] pub(crate) fn run').replace("WordNode<'_, I>", "Node<'_, u64, u64, I>")
    fn = fn.replace('            constructed = pages.get(index).ok_or(Error::InvalidStructure)?.view()?;', '''            let page = pages.get(index).ok_or(Error::InvalidStructure)?;
            constructed = if page.children.is_empty() {
                Node::leaf(&page.keys, &page.values)?
            } else {
                Node::branch(&page.keys, &page.children)?
            };''')
    exp += '\nuse crate::{Page, Answer};\n' + fn + '\n}\n'
exp += '\n  pub(super) fn view(page: &super::Page<usize>) -> Node<\'_,u64,u64,usize> { if page.children.is_empty() { Node::leaf(&page.keys, &page.values).unwrap() } else { Node::branch(&page.keys, &page.children).unwrap() } }\n}\n'
live = bench[bench.index('type WordNode'):bench.index('fn reference_lookup')] + bench[bench.index('fn measure'):bench.index('fn main')]
live = replace_once(live, '''    let pages_u32 = make_pages::<u32>(fanout, height);
    let views_u32: Vec<_> = pages_u32.iter().map(|page| page.view().unwrap()).collect();''','''    let experiment_views: Vec<_> = pages.iter().map(experiments::view).collect();
    let packed_keys: Vec<_> = pages.iter().flat_map(|page| page.keys.iter().copied()).collect();
    let packed_children: Vec<_> = pages.iter().flat_map(|page| page.children.iter().copied()).collect();
    let packed_values: Vec<_> = pages.iter().flat_map(|page| page.values.iter().copied()).collect();
    let mut children = packed_children.chunks_exact(fanout);
    let mut values = packed_values.chunks_exact(fanout);
    let packed_views: Vec<_> = pages.iter().zip(packed_keys.chunks_exact(fanout))
        .map(|(page, keys)| if page.children.is_empty() {
            Node::leaf(keys, values.next().unwrap()).unwrap()
        } else {
            Node::branch(keys, children.next().unwrap()).unwrap()
        }).collect();''')
live = replace_once(live, 'fn jzap_lookup', '#[inline(always)] fn jzap_lookup')
start=live.index('    let methods:')
end=live.index('    for query',start)
methods = ['    let product = |q| jzap_lookup::<false, _>(&pages, &views, height, q);',
           f'    let methods: [(&str, &dyn Fn(u64) -> Answer); {len(variants)+4}] = [',
           '        ("jzap", &product),',
           '        ("brie", &|q| Ok(brie.get(NonMaxU64::new(q).unwrap()).copied())),']
for name in variants:
    methods.append(f'        ("{name}", &|q| experiments::{name}::run::<false, _>(&pages, &experiment_views, height, q)),')
methods += ['        ("packed", &|q| jzap_lookup::<false, _>(&pages, &packed_views, height, q)),',
            '        ("jzap_repeat", &product),', '    ];']
live = live[:start] + '\n'.join(methods) + '\n' + live[end:]
start=live.index('    #[cfg(feature = "bplus-memory")]')
end=live.index('    {\n        let batches', start)
live = live[:start] + live[end:]
live = live.replace('assert!(elapsed >= 1.0, "short interval for {name}");','assert!(elapsed >= 1.0 || std::env::var_os("BPLUS_PILOT").is_some(), "short interval for {name}");')
main = bench[bench.index('fn main'):]
start=main.index('    eprintln!')
end=main.index('    #[cfg(not(feature = "bplus-memory"))]',start)
main=main[:start]+main[end:]
dest.joinpath('src/main.rs').write_text(exp + live + main)
manifest= (bench_dir / 'Cargo.toml').read_text()
manifest=manifest.replace('path = "../.."',f'path = "{repo}"').replace('[[bin]]\nname = "bplus"\nrequired-features = ["bplus-bench"]\n\n','')
dest.joinpath('Cargo.toml').write_text(manifest)
dest.joinpath('Cargo.lock').write_bytes((bench_dir / 'Cargo.lock').read_bytes())
