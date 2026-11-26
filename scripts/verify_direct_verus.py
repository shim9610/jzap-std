#!/usr/bin/env python3
"""Run Verus on annotated product Rust modules, without source projection.

The Python code chooses commands and records outcomes. Maintained Rust entry
files include original product modules; no function body is extracted or rewritten.
Unannotated Rust elsewhere in the crate is outside this direct proof scope.
"""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
import verus_std_specs
ROOT=Path(__file__).resolve().parents[1]
TARGETS={
    'array-caller':dict(entry='src/lib.rs',composition='tools/direct-verus/array-caller.rs',kernel=True,minimum=328,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original flatten_count in both Verus word domains, composed with typed-array layout and canonical storage/slice contracts. Flattened extents are derived from the returned count, including zero width/size and overflow refusal; no product body is translated.'),
    'vec-staging':dict(entry='src/lib.rs',minimum=245,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original executable excess collection controller: every admitted word-sized limit, initial capacity, element size and arbitrary callback reply in both Verus widths. Progress, termination and bounded admitted allocation metadata; actual owning Vec leaf and content/provenance checks remain separately identified.'),
    'vec-extraction':dict(entry='src/lib.rs',minimum=243,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original executable selected-first ExtractIf decision, scan and conditional compaction, complete admitted word-sized coordinates in both Verus widths; exact callback result, cursor retirement, frame, error exit and termination. Original cold stop and physical transfer callers are checked separately.'),
    'vec-compaction':dict(entry='src/lib.rs',minimum=239,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual generic prefix/compact-one/sweep/run bodies used by Vec retain/dedup; all admitted word lengths and arbitrary callback replies in both Verus word widths, bounded state, error exit and termination. Physical leaves and content semantics retain separate actual Rust/Miri checks.'),
    'btree-node-spans':dict(entry='src/lib.rs',composition='tools/direct-verus/btree-adapters.rs',kernel=True,minimum=251,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Seven executable span producers consumed by actual node insertion/split/suffix/removal/merge/borrow kernels. Exact results and pair/edge bounds retain every previously admitted count/index in both word domains. Actual all-occupancy typed node harnesses retain their compiler/Kani/Miri memory roles; no node-body extraction.'),
    'btree-packed-shape':dict(entry='src/lib.rs',composition='tools/direct-verus/btree-adapters.rs',kernel=True,minimum=237,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original packed-output shape loop for every word length in both Verus word domains: success, exact leaf count, zero case, branch/height bounds, nonoverflow and termination. Ghost quarter-shrink/bit-bound reasoning is erased from normal builds. Actual native Rust checks call the same function; no body extraction or word-type replacement.'),
    'btree-delete-decisions':dict(entry='src/lib.rs',composition='tools/direct-verus/btree-adapters.rs',kernel=True,minimum=232,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual CAP constant and executable left/right eligibility and borrow-versus-merge predicates in both word domains, including exact results and preserved prior all-word admission/merge contracts. Native arithmetic, retained actual all-occupancy node kernels and Miri have separate scopes; no arbitrary-heap theorem is claimed.'),
    'btree-frontiers':dict(entry='src/lib.rs',composition='tools/direct-verus/btree-adapters.rs',kernel=True,minimum=226,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual no-drop cleanup frontier and borrowed leaf predecessor/successor for every admitted word-sized count/index/remainder in both word domains. Maintained adapters retain the prior <=11/cast contracts. Actual typed iterator/drop callers have native checks; whole tree/ancestor credit models remain separate.'),
    'btree-plan-counts':dict(entry='src/lib.rs',composition='tools/direct-verus/btree-adapters.rs',kernel=True,minimum=222,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual allocation deficit and retirement successor, actual two-field NodeCounts observation and height-selected retirement in both word domains, with exact state and frame contracts. Packed output shape and whole merge/tree-credit models remain separate.'),
    'btree-search-cursors':dict(entry='src/lib.rs',composition='tools/direct-verus/btree-adapters.rs',kernel=True,minimum=215,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual BTree binary search midpoint/advance with bounded strictly decreasing intervals and packed owned-cursor operations over every admitted word input in both word domains. Actual physical NodeSlice and Cursor callers have Rust checks; remaining BTree models are separate.'),
    'shared-numeric':dict(entry='src/lib.rs',minimum=203,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual reference-count admission and retired OR/mask encoding/decoding over every admitted word input in both word domains. The shared bit/number lemma is proved on source; atomic schedules and platform bounds remain separate.'),
    'shared-numeric-reduced':dict(entry='src/lib.rs',minimum=203,cfgs=['jzap_small_counts'],sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='The same actual numeric functions and constants compiled with the real reduced-count model configuration; all word inputs and both word domains are retained.'),
    'vec-layout-growth':dict(entry='src/lib.rs',composition='tools/direct-verus/layout-adapters.rs',minimum=191,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual shared array ceiling and Vec layout-aware growth proposal for all word-sized sizes and admitted current/needed/limit states in both word domains; exact Layout refusal and exact-growth result. Actual owner publication and nonzero reservations have Rust checks.'),
    'string-cursors':dict(entry='src/lib.rs',minimum=187,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original scalar cursor advance, retained-suffix restoration and drain deletion geometry over every admitted word input in both Verus word domains. UTF-8 producers, physical contents and caller cleanup use actual-Rust checks; pinned core models remain separate.'),
    'vec-cursors':dict(entry='src/lib.rs',composition='tools/direct-verus/vec-adapters.rs',minimum=186,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original within-capacity append and removed-suffix arithmetic for every word input, exact admission/refusal, both Verus word domains. Actual owner publication, rollback and extraction cleanup use compiled Rust checks.'),
    'owner-metadata':dict(entry='src/lib.rs',composition='tools/direct-verus/layout-adapters.rs',minimum=184,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original logical capacity and standard-adoption shape for every word-sized element size, source length/capacity and limit; exact admission/error. Original required_len supplies initialized-length admission. Actual owner calls and transitions are checked on Rust separately.'),
    'buffer':dict(entry='src/lib.rs',composition='tools/direct-verus/init-adapters.rs',minimum=183,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual Buffer required length/growth proposal, exact overflow refusal and source-stop/error normalization; actual shared cursor over every admitted word-sized span in either word domain. Physical writes and owner publication have actual-Rust checks.'),
    'init-prefix':dict(entry='src/lib.rs',composition='tools/direct-verus/init-adapters.rs',minimum=179,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual generic initialization cursor for every admitted word-sized start/end in either word domain, with exact successful call history, error stopping and termination. Physical writes and typed cleanup are checked separately on actual Rust.'),
    'fixed-bplus':dict(entry='src/lib.rs',minimum=279,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual FixedTree assemble/summarize/summarize_branch validation and find/floor_slot/leaf_slot/leaf_rank lookup for every pool content, child ID and key order in both Verus word domains. The type invariant established by assemble (children precede their parent one level lower, root last) bounds every unchecked pool, child, leaf, value and summary-slot access. Key comparisons arrive as closures whose results are not assumed; core::hint::select_unpredictable is the one specification imported from tools/direct-verus/std-specs.rs. The unverified try_new/get wrappers only allocate and build those closures.'),
    'bplus-partial':dict(entry='src/lib.rs',minimum=287,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual partial-key pass core (pass_over) for every key count, progress recorded by an earlier pass and closure reply in both Verus word domains: every recorded check holds for the completions and is kept by the pass, a requested key is unattached, and an answer is the specification over the completions (refusals only for invalid nodes). The comparison closures built from order_keys/order_fence/order_query and the PartialKey laws for PrefixKey are checked by Kani; the fence admission prologue and child lookup in the unverified pass wrapper are covered by tests and Kani.'),
    'integer':dict(entry='src/lib.rs',minimum=162,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original TryArithmetic implementations: mathematical add/sub/mul value and exact overflow contracts for all 12 integer types, with no input restriction. Original checked division/remainder bodies and companion annotated kernels are also verified; Generic Checked add/sub/mul and the Vec length/growth kernels have direct contracts; wrapping/division/remainder forwarding semantics have separate actual-Rust Kani checks.'),
    'local-budget':dict(entry='src/lib.rs',composition='tools/direct-verus/local-budget.rs',minimum=150,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Actual local-budget pending, admission, credit and retirement functions with all-word contracts; logical ownership ledger calls their compiler-generated dual specifications. Cell/RAII transitions and ownership framing have actual-Rust Kani checks.'),
    'vec-geometry':dict(entry='src/lib.rs',composition='tools/direct-verus/vec-adapters.rs',minimum=167,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original Vec required_len and growth_capacity, including checked addition, exact Capacity errors, inclusive/exclusive endpoint normalization and the shared range validator for every machine-word input. Native callers preserve getter order. Caller memory and edit loops remain separate.'),
    'vec-flatten':dict(entry='src/lib.rs',composition='tools/direct-verus/vec-adapters.rs',minimum=169,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original flattened shape admission for every word-sized length, capacity, element limit, array width and element size under the original physical layout precondition, including exact refusal and unchanged allocation bytes.'),
    'vec-extent':dict(entry='src/lib.rs',minimum=310,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original closed owner state of Vec (src/extent.rs) and its capacity (src/capacity.rs), generic over the element and the sealed length policy, in both Verus word domains. Type invariants: len <= cap <= limit, and the capacity of a sized element spans at most isize::MAX bytes. Every constructor establishes and every transition preserves them for arbitrary arguments, with exact resulting words. Trusted entries with a stated precondition: Extent::from_raw_parts and Capacity::from_validated_raw. Initialization and liveness of the storage are outside this numeric state.'),
    'vec-queries':dict(entry='src/lib.rs',composition='tools/direct-verus/vec-adapters.rs',minimum=182,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original Unlimited/LengthLimit methods and bounded subtraction in either word domain. Original owner queries run through Kani at arbitrary valid ZST length/capacity/limit states and both sealed length policies; physical nonzero-element storage is separate.'),
    'layout':dict(entry='src/lib.rs',composition='tools/direct-verus/layout-adapters.rs',minimum=173,sources=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'src').rglob('*.rs')),scope='Original runtime array-byte admission and capacity multiplication, for every word-sized count/element size in either word domain; generic Layout construction and alignment checks run on actual Rust separately.'),
    'clone':dict(entry='tools/direct-verus/clone.rs',minimum=1,sources=['src/clone/geometry.rs','src/error.rs'],scope='Actual nested-array adapter admission for every usize source length, destination length and array width, including mismatch and multiplication overflow; typed storage and trait callers are checked separately.'),
    'scratch-slice':dict(entry='tools/direct-verus/scratch-slice.rs',minimum=9,sources=['src/slice/geometry.rs','src/arithmetic/bounded.rs','src/error.rs'],scope='Original fixed suffix admission, normalized range and single-element retirement for every usize input; typed storage and callback callers are checked separately.'),
    'deque':dict(entry='tools/direct-verus/deque.rs',minimum=13,sources=['src/deque/geometry.rs','src/arithmetic/bounded.rs'],scope='Original ring wrap, retreat, split and grown-head bodies for every admitted usize state; allocation/provenance and caller state are checked separately.'),
    'binary-heap':dict(entry='tools/direct-verus/binary-heap.rs',minimum=4,sources=['src/collections/binary_heap/geometry.rs','src/arithmetic/bounded.rs'],scope='Actual parent/child indices for every native-word frontier, including usize::MAX zero-sized heaps; allocation and hole provenance remain separate.'),
    'hash-map':dict(entry='tools/direct-verus/hash-map.rs',minimum=3,sources=['src/collections/hash_map/geometry.rs','src/arithmetic/bounded.rs','src/error.rs'],scope='Actual checked table sizes, mirror and masked probe offsets for every admitted native-word table shape; private state/caller establishment remains separate.'),
    'bounded':dict(entry='tools/direct-verus/bounded.rs',minimum=3,
        sources=['src/arithmetic/bounded.rs'],scope='Three actual usize wrapping bodies under their explicit no-overflow/no-underflow preconditions; caller establishment is separate.'),
    'budget-admit':dict(entry='tools/direct-verus/budget-admit.rs',minimum=1,
        sources=['src/budget/admission.rs','src/error.rs'],scope='Actual shared-byte admission for every usize limit/used/request: accepted sum or exact capacity/budget refusal. Atomic interleavings and ownership are separate.'),
}
def executable():
    name=os.environ.get('JZAP_VERUS') or shutil.which('verus')
    local=Path('/workspace/.devtools/verus/verus-x86-linux/verus')
    if not name and local.is_file():name=str(local)
    if not name:raise RuntimeError('Verus not found; install the documented version and set JZAP_VERUS=/path/to/verus')
    if not shutil.which(name):raise RuntimeError('Verus executable unavailable: '+name)
    return name
def arithmetic_scope(root):
    # Declaration/configuration audit only: no positive body is translated.
    text = ''.join((root/'src/arithmetic/checked.rs').read_text().split())
    for trait, method, token in [('Add','add','+'),('Sub','sub','-'),('Mul','mul','*')]:
        if text.count(f'operator!({trait},{method},try_{method},ArithmeticError,all(),{token});') != 1:
            raise RuntimeError('Checked operator verification scope changed: '+trait)
        for declaration in (f'openspecfnobeys_{method}_spec()->bool{{true}}',
                            f'openspecfn{method}_req(self,_rhs:Self)->bool{{true}}'):
            if text.count(declaration) != 1:
                raise RuntimeError('Checked standard-operator contract disabled: '+method)

def run_target(root,out,name,binary,expect_failure=False):
    target=TARGETS[name];out.mkdir(parents=True,exist_ok=True)
    command=[binary,target['entry'],'--crate-type','lib','--crate-name','direct_product','--edition=2024',
        '--cfg','feature="alloc"',
        '--no-cheating','--output-json']
    for cfg in target.get('cfgs',[]):command += ['--cfg',cfg]
    # The product crate imports the checked core specifications vstd lacks.
    command += verus_std_specs.build(binary)
    env = dict(os.environ)
    if target.get('composition'):
        command += ['--cfg', 'jzap_proof_composition']
        composition = (root / target['composition']).resolve()
        if target.get('kernel'):
            # The B-tree adapters prove their node primitives from the storage
            # kernel, which is placed in front of them unchanged.
            import storage_kernel
            composed = out / (name + '.composition.rs')
            composed.write_text(storage_kernel.kernel() + composition.read_text())
            composition = composed.resolve()
        env['JZAP_PROOF_COMPOSITION'] = str(composition)
    result=subprocess.run(command,cwd=root,env=env,capture_output=True,text=True)
    (out/(name+'.stdout.json')).write_text(result.stdout)
    (out/(name+'.stderr.log')).write_text(result.stderr)
    try:data=json.loads(result.stdout);verdict=data['verification-results']
    except (ValueError,KeyError) as error:raise RuntimeError('Verus produced no verification result; see '+str(out)) from error
    if expect_failure:
        passed=result.returncode!=0 and not verdict['encountered-vir-error'] and verdict['errors']>0
    else:
        passed=result.returncode==0 and verdict['success'] and not verdict['encountered-error'] and verdict['errors']==0 and verdict['verified']>=target['minimum']
    if not passed:raise RuntimeError('Unexpected direct verification result for '+name+': '+str(verdict)+'; see '+str(out))
    return dict(target=name,command=command,cwd=str(root),scope=target['scope'],exit_code=result.returncode,
        verification=verdict,verus=data['verus'],sources={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in target['sources']},
        stdout=str(out/(name+'.stdout.json')),stderr=str(out/(name+'.stderr.log')))
def verify(root=ROOT,out=None,targets=None,mutations=False):
    root=Path(root).resolve();out=Path(out or ROOT/'target/verification/direct-verus').resolve();out.mkdir(parents=True,exist_ok=True)
    (out/'summary.json').unlink(missing_ok=True)
    if not (root/'tools/direct-verus').is_dir():
        shutil.copytree(ROOT/'tools/direct-verus',root/'tools/direct-verus')
    targets=list(targets or TARGETS);binary=executable()
    import storage_kernel
    paths=[*sorted((root/'src').rglob('*.rs')),*sorted((root/'tools/direct-verus').glob('*.rs')),Path(__file__),*storage_kernel.INPUTS]
    fingerprint=lambda:hashlib.sha256(b''.join(str(p.relative_to(root) if p.is_relative_to(root) else p.name).encode()+b'\0'+p.read_bytes() for p in paths)).hexdigest()
    before=fingerprint()
    if any(name in targets for name in ('integer', 'vec-geometry', 'vec-flatten', 'layout', 'vec-queries')):arithmetic_scope(root)
    rows=[run_target(root,out,name,binary) for name in targets];controls=[]
    if mutations:
        variants=[
            ('staging-limit','vec-staging','src/vec/staging_driver.rs',
             '        if len >= limit {','        if len > limit {'),
            ('staging-progress','vec-staging','src/vec/staging_driver.rs',
             '                len = new_len;','                len = len;'),
            ('staging-capacity','vec-staging','src/vec/staging_driver.rs',
             '            || new_cap > limit','            || false'),
            ('extract-retained-gap','vec-extraction','src/vec/compaction_driver.rs',
             '        if !state.untouched() {\n            act(read, state.written());',
             '        if state.untouched() {\n            act(read, state.written());'),
            ('extract-retirement','vec-extraction','src/vec/compaction_driver.rs',
             '        let _ = state.keep_next();\n    }\n    Ok(None)',
             '        let _ = state.remaining();\n    }\n    Ok(None)'),
            ('extract-selected-retirement','vec-extraction','src/vec/compaction_driver.rs',
             '        if extraction_decision(&mut step, read)? {\n            let _ = state.remove_next();\n            return Ok(Some(read));',
             '        if extraction_decision(&mut step, read)? {\n            let _ = state.remaining();\n            return Ok(Some(read));'),
            ('vec-prefix-progress', 'vec-compaction', 'src/vec/compaction_driver.rs', '        let _ = state.keep_next();\n    }\n    Ok(true)', '        let _ = state.peek();\n    }\n    Ok(true)'),
            ('vec-compaction-retirement', 'vec-compaction', 'src/vec/compaction_driver.rs', 'state.remove_live();', 'state.keep_live();'),
            ('vec-compaction-first-removal', 'vec-compaction', 'src/vec/compaction_driver.rs', 'let Some(read) = state.remove_next() else {', 'let Some(read) = state.peek() else {'),
            ('vec-paired-boundary', 'vec-compaction', 'src/vec/compaction_driver.rs', 'end.saturating_sub(1)', 'end'),
            ('btree-node-insert-tail', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', '(after, sub(old, index), add(old, 1), add(after, 1))', '(after, add(sub(old, index), 1), add(old, 1), add(after, 1))'),
            ('btree-node-split-underflow', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', 'let count = sub(old, start);', 'let count = sub(start, old);'),
            ('btree-node-suffix-start', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', '(sub(old, index), add(index, 1))', '(sub(old, index), index)'),
            ('btree-node-remove-count', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', '(after, sub(old, after), sub(old, 1))', '(after, sub(old, after), old)'),
            ('btree-node-merge-edge-count', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', '(start, add(start, right), add(right, 1))', '(start, add(start, right), right)'),
            ('btree-node-left-borrow-count', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', '(sub(left, 1), add(right, 1))', '(left, add(right, 1))'),
            ('btree-node-right-borrow-count', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', '(add(left, 1), sub(right, 1))', '(left, sub(right, 1))'),
            ('btree-shape-leaves','btree-packed-shape','src/collections/btree_map/bulk.rs','let leaves = leaves.checked_add(1).ok_or(Error::Capacity)?;','let leaves = leaves.checked_add(2).ok_or(Error::Capacity)?;'),
            ('btree-shape-ceiling','btree-packed-shape','src/collections/btree_map/bulk.rs','if level % 12 != 0 { 1 } else { 0 }','0'),
            ('btree-shape-height','btree-packed-shape','src/collections/btree_map/bulk.rs','height = height.checked_add(1).ok_or(Error::Capacity)?;','height = height.checked_add(32).ok_or(Error::Capacity)?;'),
            ('btree-shape-branches','btree-packed-shape','src/collections/btree_map/bulk.rs','branches = branches.checked_add(level).ok_or(Error::Capacity)?;','branches = branches.checked_add(0).ok_or(Error::Capacity)?;'),

            ('btree-delete-left-eligibility','btree-delete-decisions','src/collections/btree_map/geometry.rs','pub(crate) fn consider_left(index: usize) -> bool {\n    index > 0\n}','pub(crate) fn consider_left(index: usize) -> bool {\n    index == 0\n}'),
            ('btree-delete-right-boundary','btree-delete-decisions','src/collections/btree_map/geometry.rs','index == 0 && index < parent','index == 0 && index <= parent'),
            ('btree-delete-separator','btree-delete-decisions','src/collections/btree_map/geometry.rs','add(add(sibling, child), 1) > super::node::CAP','add(sibling, child) > super::node::CAP'),
            ('btree-delete-merge-equality','btree-delete-decisions','src/collections/btree_map/geometry.rs','add(add(sibling, child), 1) > super::node::CAP','add(add(sibling, child), 1) >= super::node::CAP'),

            ('btree-cleanup-cap','btree-frontiers','src/collections/btree_map/geometry.rs','suffix.min(sub(remaining, 1))','suffix'),
            ('btree-cleanup-reserved-last','btree-frontiers','src/collections/btree_map/geometry.rs','suffix.min(sub(remaining, 1))','suffix.min(remaining)'),
            ('btree-leaf-predecessor','btree-frontiers','src/collections/btree_map/geometry.rs','let i = if back { sub(index, 1) } else { index };','let i = index;'),
            ('btree-leaf-successor','btree-frontiers','src/collections/btree_map/geometry.rs','(i, if back { i } else { add(i, 1) })','(i, i)'),

            ('btree-deficit-maximum','btree-plan-counts','src/collections/btree_map/geometry.rs','maximum.max(allocated.saturating_sub(retired))','maximum.min(allocated.saturating_sub(retired))'),
            ('btree-deficit-subtraction','btree-plan-counts','src/collections/btree_map/geometry.rs','maximum.max(allocated.saturating_sub(retired))','maximum.max(retired.saturating_sub(allocated))'),
            ('btree-retired-successor','btree-plan-counts','src/collections/btree_map/geometry.rs','pub(crate) fn retired_successor(count: usize) -> usize {\n    add(count, 1)\n}', 'pub(crate) fn retired_successor(count: usize) -> usize {\n    add(count, 2)\n}'),
            ('btree-observe-retired-field','btree-plan-counts','src/collections/btree_map/bulk.rs','allocation_deficit(self.leaves, leaves, retired.leaves)','allocation_deficit(self.leaves, leaves, retired.branches)'),

            ('btree-search-middle','btree-search-cursors','src/collections/btree_map/geometry.rs','add(low, sub(high, low) / 2)','add(high, sub(high, low) / 2)'),
            ('btree-search-progress','btree-search-cursors','src/collections/btree_map/geometry.rs','(add(middle, 1), high)','(middle, high)'),
            ('btree-cursor-pack','btree-search-cursors','src/collections/btree_map/geometry.rs','height.wrapping_shl(4) | index','height.wrapping_shl(3) | index'),
            ('btree-cursor-index','btree-search-cursors','src/collections/btree_map/geometry.rs','(position & 15) as usize','(position & 7) as usize'),

            ('shared-admission-boundary','shared-numeric','src/shared/count.rs','if old >= MAX_COUNT {','if old > MAX_COUNT {'),
            ('shared-admission-advance','shared-numeric','src/shared/count.rs','old.checked_add(1).ok_or(Error::RefCount)\n}','old.checked_add(2).ok_or(Error::RefCount)\n}'),
            ('shared-retired-tag','shared-numeric','src/shared/count.rs','let out = bytes | RETIRED;','let out = bytes & RETIRED;'),
            ('shared-retired-mask','shared-numeric','src/shared/count.rs','let out = state & !RETIRED;','let out = state & RETIRED;'),
            ('array-ceiling-zero-size','vec-layout-growth','src/layout/numeric.rs','.unwrap_or(usize::MAX)','.unwrap_or(0)'),
            ('layout-growth-boundary','vec-layout-growth','src/vec/geometry.rs','if needed > layout_ceiling {','if needed >= layout_ceiling {'),
            ('layout-growth-clamp','vec-layout-growth','src/vec/geometry.rs','limit.min(layout_ceiling)','limit.max(layout_ceiling)'),
            ('layout-growth-error','vec-layout-growth','src/vec/geometry.rs','return Err(Error::Layout);','return Err(Error::Bounds);'),
            ('string-cursor-advance','string-cursors','src/string/numeric.rs','cursor.saturating_add(width)','cursor.saturating_add(width).saturating_add(1)'),
            ('string-retain-restore','string-cursors','src/string/numeric.rs','let len = written.saturating_add(remaining);','let len = read.saturating_add(remaining);'),
            ('string-edit-destination','string-cursors','src/string/edit.rs','Checked::new(start) + Checked::new(inserted)','Checked::new(end) + Checked::new(inserted)'),
            ('string-edit-additional','string-cursors','src/string/edit.rs','additional: len.saturating_sub(old),','additional: old.saturating_sub(len),'),
            ('string-edit-tail','string-cursors','src/string/edit.rs','Checked::new(old) - Checked::new(end)','Checked::new(old) - Checked::new(start)'),
            ('string-drain-length','string-cursors','src/string/numeric.rs','let length = old.saturating_sub(removed);','let length = old.saturating_add(removed);'),
            ('insertion-count','vec-cursors','src/vec/geometry.rs','    match len - Checked::new(index) {\n        Ok(shifted) => Ok(shifted),','    match len - Checked::new(0) {\n        Ok(shifted) => Ok(shifted),'),
            ('shortened-guard','vec-cursors','src/vec/geometry.rs','    if index >= len.get() {\n        return Err(Error::Bounds);\n    }\n    match len - Checked::new(1) {','    if index > len.get() {\n        return Err(Error::Bounds);\n    }\n    match len - Checked::new(1) {'),
            ('removal-count','vec-cursors','src/vec/geometry.rs','    match next - Checked::new(index) {','    match len - Checked::new(index) {'),
            ('append-cursor-guard','vec-cursors','src/vec/geometry.rs','if len.get() >= cap {','if len.get() > cap {'),
            ('append-cursor-advance','vec-cursors','src/vec/geometry.rs','match len + Checked::new(1) {','match len + Checked::new(2) {'),
            ('suffix-count-direction','vec-cursors','src/vec/geometry.rs','let Ok(removed) = current - requested else {','let Ok(removed) = requested - current else {'),
            ('owner-logical-capacity','owner-metadata','src/capacity.rs','    if size == 0 { limit } else { stored }','    if size != 0 { limit } else { stored }'),
            ('owner-adoption-guard','owner-metadata','src/capacity.rs','if length > limit || capacity > limit {','if length > limit && capacity > limit {'),
            ('owner-adoption-error','owner-metadata','src/capacity.rs','return Err(Error::Capacity);','return Err(Error::Bounds);'),
            ('buffer-growth-factor','buffer','src/boxed/buffer_numeric.rs','Checked::new(cap) * Checked::new(2)','Checked::new(cap) * Checked::new(3)'),
            ('buffer-initial-hint','buffer','src/boxed/buffer_numeric.rs','hint.max(needed)','needed'),
            ('buffer-overflow-kind','buffer','src/boxed/buffer_numeric.rs','|_error| Error::Capacity,','|_error| Error::Layout,'),
            ('collection-end','buffer','src/init/collection.rs','Err(Stop::End) => Ok(false),','Err(Stop::End) => Ok(true),'),
            ('collection-source-error','buffer','src/init/collection.rs','Err(Stop::Source(error)) => Err(error),','Err(Stop::Source(_error)) => Ok(false),'),
            ('init-advance','init-prefix','src/init/driver.rs','wrapping_add(index, 1)','wrapping_add(index, 2)'),
            ('init-missing-advance','init-prefix','src/init/driver.rs','*state = Checked::new(wrapping_add(index, 1));','*state = Checked::new(index);'),
            ('init-early-stop','init-prefix','src/init/driver.rs','while state.get() < end {','while state.get() < end && state.get() != 17 {'),
            ('init-wrong-callback','init-prefix','src/init/driver.rs','let result = step(index);','let result = step(end);'),
            ('init-swallowed-error','init-prefix','src/init/driver.rs','        result?;','        let _ = result;'),
            ('extent-skipped-check','vec-extent','src/extent.rs',
             '        if needed > self.cap.get() {\n            return Err(Error::Capacity);\n        }\n        self.len = Checked::new(needed);',
             '        self.len = Checked::new(needed);'),
            ('extent-constructor-order','vec-extent','src/extent.rs','if len > cap || cap > limit.max_len() {','if len > cap && cap > limit.max_len() {'),
            ('extent-constructor-limit','vec-extent','src/extent.rs','if len > cap || cap > limit.max_len() {','if len > cap {'),
            ('extent-clamp-above-capacity','vec-extent','src/extent.rs','self.len = Checked::new(if next <= cap { next } else { cap });','self.len = Checked::new(if next <= cap { next } else { next });'),
            ('extent-unlimited-clamp','vec-extent','src/extent.rs','len: Checked::new(if len <= cap { len } else { cap }),','len: Checked::new(len),'),
            ('extent-shorten-lengthens','vec-extent','src/extent.rs','        if next < self.len.get() {','        if next > self.len.get() {'),
            ('extent-growth-below-length','vec-extent','src/extent.rs','if needed < self.len.get() || needed > limit {','if needed > limit {'),
            ('extent-growth-above-limit','vec-extent','src/extent.rs','if needed < self.len.get() || needed > limit {','if needed < self.len.get() {'),
            ('extent-growth-current-length','vec-extent','src/extent.rs','        Ok(Self {\n            len: self.len,\n            cap,\n','        Ok(Self {\n            len: Checked::new(count),\n            cap,\n'),
            ('extent-resize-below-length','vec-extent','src/extent.rs','if cap < self.len.get() || cap > self.limit.max_len() {','if cap > self.limit.max_len() {'),
            ('extent-resize-above-limit','vec-extent','src/extent.rs','if cap < self.len.get() || cap > self.limit.max_len() {','if cap < self.len.get() {'),
            ('extent-empty-capacity','vec-extent','src/extent.rs','cap: Capacity::empty(limit.max_len()),','cap: Capacity::empty(usize::MAX),'),
            ('extent-trusted-entry-precondition','vec-extent','src/extent.rs','        requires len <= capacity@ <= limit.bound(),\n','        requires capacity@ <= limit.bound(),\n'),
            ('extent-length-getter','vec-extent','src/extent.rs','    pub(crate) fn len(&self) -> usize {\n        self.len.get()','    pub(crate) fn len(&self) -> usize {\n        self.cap.get()'),
            ('capacity-unchecked-count','vec-extent','src/capacity.rs','        let _bytes = crate::layout::numeric::array_bytes(count, mem::size_of::<T>())?;\n','        let _bytes = crate::layout::numeric::array_bytes(0, mem::size_of::<T>())?;\n'),
            ('capacity-empty-sized','vec-extent','src/capacity.rs','count: Checked::new(logical_capacity(mem::size_of::<T>(), 0, limit)),','count: Checked::new(logical_capacity(mem::size_of::<T>(), limit, limit)),'),
            ('capacity-trusted-entry-precondition','vec-extent','src/capacity.rs','        requires vstd::layout::size_of::<T>() != 0\n            ==> byte_valid(count, vstd::layout::size_of::<T>() as usize),\n        ensures out@ == count,','        ensures out@ == count,'),
            ('policy-bound-unlimited','vec-extent','src/vec.rs','open spec fn bound(self) -> usize { usize::MAX }','open spec fn bound(self) -> usize { 0 }'),
            ('policy-bound-configured','vec-extent','src/vec.rs','open spec fn bound(self) -> usize { self@ }','open spec fn bound(self) -> usize { 0 }'),
            ('query-unlimited-value','vec-queries','src/vec.rs','fn max_len(self) -> usize {\n        usize::MAX','fn max_len(self) -> usize {\n        0'),
            ('query-configured-value','vec-queries','src/vec.rs','fn max_len(self) -> usize {\n        self.0','fn max_len(self) -> usize {\n        0'),
            ('query-configured-view','vec-queries','src/vec.rs','pub closed spec fn view(self) -> usize { self.0 }','pub closed spec fn view(self) -> usize { 0 }'),
            ('partial-pair-order','bplus-partial','src/collections/btree_map/view/bplus/partial.rs','            Decision::Order(Ordering::Less) => {}\n            Decision::Order(_) | Decision::Fence => {','            Decision::Order(_) => {}\n            Decision::Fence => {'),
            ('partial-minimum-equal','bplus-partial','src/collections/btree_map/view/bplus/partial.rs','            Decision::Order(Ordering::Equal) => Check::Passed,','            Decision::Order(Ordering::Less | Ordering::Equal) => Check::Passed,'),
            ('partial-ceiling-below','bplus-partial','src/collections/btree_map/view/bplus/partial.rs','            Decision::Order(Ordering::Less) => Check::Passed,','            Decision::Order(Ordering::Less | Ordering::Equal) => Check::Passed,'),
            ('partial-search-advance','bplus-partial','src/collections/btree_map/view/bplus/partial.rs','                low = wrapping_add(middle, 1);','                low = middle;'),
            ('partial-branch-floor','bplus-partial','src/collections/btree_map/view/bplus/partial.rs','        None => Answer::Descend(wrapping_sub(low, 1)),','        None => Answer::Descend(low),'),
            ('partial-deferred-fence','bplus-partial','src/collections/btree_map/view/bplus/partial.rs','    if progress.minimum.deferred() || progress.ceiling.deferred() {','    if progress.minimum.deferred() {'),
            ('fixed-child-past-prefix','fixed-bplus','src/collections/btree_map/view/bplus/fixed.rs','if id >= prefix.len() {','if id > prefix.len() {'),
            ('fixed-mixed-child-heights','fixed-bplus','src/collections/btree_map/view/bplus/fixed.rs','Some(expected) => expected != child.height,','Some(expected) => false,'),
            ('fixed-branch-height','fixed-bplus','src/collections/btree_map/view/bplus/fixed.rs','Some(value) => match value.checked_add(1) {','Some(value) => match value.checked_add(2) {'),
            ('fixed-leaf-height','fixed-bplus','src/collections/btree_map/view/bplus/fixed.rs','            height: 0,\n            has_parent: false,\n        });','            height: 1,\n            has_parent: false,\n        });'),
            ('fixed-root-past-end','fixed-bplus','src/collections/btree_map/view/bplus/fixed.rs','root: Some(wrapping_sub(count, 1)),','root: Some(count),'),
            ('fixed-route-past-middle','fixed-bplus','src/collections/btree_map/view/bplus/fixed.rs','base = select_unpredictable(admits(key), middle, base);','base = select_unpredictable(admits(key), wrapping_add(middle, 1), base);'),
            ('fixed-value-past-match','fixed-bplus','src/collections/btree_map/view/bplus/fixed.rs','                Some(base)\n','                Some(wrapping_add(base, 1))\n'),
            ('layout-reject','layout','src/layout/numeric.rs','if count > ceiling {','if count < ceiling {'),
            ('layout-product','layout','src/layout/numeric.rs','size.checked_mul(count)','size.checked_add(count)'),
            ('layout-error-kind','layout','src/layout/numeric.rs','return Err(Error::Layout);','return Err(Error::Bounds);'),
            ('flatten-length','vec-flatten','src/vec/geometry.rs','Checked::new(len) * Checked::new(width)','Checked::new(len) + Checked::new(width)'),
            ('flatten-capacity-limit','vec-flatten','src/vec/geometry.rs','if length > limit || capacity > limit {','if length > limit || capacity > usize::MAX {'),
            ('flatten-zero-size','vec-flatten','src/vec/geometry.rs','let capacity = if size == 0 {\n        limit','let capacity = if size == 0 {\n        cap'),
            ('integer-numeric-model','integer','src/arithmetic.rs','open spec fn numeric_value(self) -> int { self as int }','open spec fn numeric_value(self) -> int { self as int + 1 }'),
            ('checked-add-dispatch','integer','src/arithmetic/checked.rs','operator!(Add, add, try_add, ArithmeticError, all(), +);','operator!(Add, add, try_sub, ArithmeticError, all(), +);'),
            ('checked-spec-result','integer','src/arithmetic/checked.rs','let value = self@.numeric_value() + rhs@.numeric_value();','let value = self@.numeric_value() - rhs@.numeric_value();'),
            ('vec-lower-endpoint','vec-geometry','src/vec/bounds.rs','Bound::Excluded(&n) => (Checked::new(n) + Checked::new(1))','Bound::Excluded(&n) => (Checked::new(n) + Checked::new(2))'),
            ('vec-upper-endpoint','vec-geometry','src/vec/bounds.rs','Bound::Included(&n) => (Checked::new(n) + Checked::new(1))','Bound::Included(&n) => (Checked::new(n) + Checked::new(2))'),
            ('vec-range-guard','vec-geometry','src/slice/geometry.rs','if start > end || end > length {','if start > end && end > length {'),
            ('vec-length-limit','vec-geometry','src/vec.rs','if needed > limit {','if needed > usize::MAX {'),
            ('vec-growth-ceiling','vec-geometry','src/vec.rs','        .min(ceiling)','        .max(ceiling)'),
            ('vec-error-kind','vec-geometry','src/vec.rs','|_error| Error::Capacity,','|_error| Error::Bounds,'),
            ('budget-pending','local-budget','src/budget_numeric.rs','wrapping_sub(wrapping_sub(limit, available), occupied)','wrapping_sub(wrapping_sub(limit, available), available)'),
            ('budget-admission','local-budget','src/budget_numeric.rs','if bytes > available {','if bytes < available {'),
            ('budget-credit','local-budget','src/budget_numeric.rs','wrapping_add(current, bytes)','wrapping_sub(current, bytes)'),
            ('budget-retirement','local-budget','src/budget_numeric.rs','wrapping_sub(current, bytes)','wrapping_add(current, bytes)'),
            ('integer-add','integer','src/arithmetic.rs','self.checked_add(rhs).ok_or(ArithmeticError::Overflow)','self.checked_sub(rhs).ok_or(ArithmeticError::Overflow)'),
            ('integer-sub','integer','src/arithmetic.rs','self.checked_sub(rhs).ok_or(ArithmeticError::Overflow)','self.checked_add(rhs).ok_or(ArithmeticError::Overflow)'),
            ('integer-mul','integer','src/arithmetic.rs','self.checked_mul(rhs).ok_or(ArithmeticError::Overflow)','self.checked_add(rhs).ok_or(ArithmeticError::Overflow)'),
            ('integer-hidden-panic','integer','src/arithmetic.rs','self.checked_add(rhs).ok_or(ArithmeticError::Overflow)','if self == 0 && rhs == 0 { panic!("direct arithmetic control"); }\n                self.checked_add(rhs).ok_or(ArithmeticError::Overflow)'),
            ('clone-length','clone','src/clone/geometry.rs','if source != destination {','if source == destination {'),
            ('array-caller-offset','array-caller','tools/direct-verus/array-caller.rs','slice_caller(source,0,count)','slice_caller(source,1,count)'),
            ('clone-overflow','clone','src/clone/geometry.rs','let Some(count) = source.checked_mul(width) else {\n        return Err(crate::Error::Capacity);\n    };','let count = source.wrapping_mul(width);'),
            ('scratch-initialized-progress','scratch-slice','src/slice/geometry.rs','    add(index, 1)', '    add(index, 2)'),
            ('scratch-admission','scratch-slice','src/slice/geometry.rs','if count > sub(capacity, start) {','if count < sub(capacity, start) {'),
            ('slice-range','scratch-slice','src/slice/geometry.rs','if start > end || end > length {','if start > end && end > length {'),
            ('scratch-retirement','scratch-slice','src/slice/geometry.rs','    sub(old, 1)','    old'),
            ('deque-backward-progress','deque','src/deque/geometry.rs','(n, sub(src, n), sub(dst, n), sub(count, n))','(n, sub(src, n), sub(dst, n), count)'),
            ('deque-allocated-wrap','deque','src/deque/geometry.rs','if sum >= cap {','if sum > cap {'),
            ('deque-pop-spare', 'deque', 'src/deque/geometry.rs', '    sub(len, 1)\n}', '    len\n}'),
            ('deque-wrap','deque','src/deque/geometry.rs','sub(offset, end)','add(offset, end)'),
            ('deque-retreat','deque','src/deque/geometry.rs','sub(cap, sub(offset, head))','sub(cap, offset)'),
            ('deque-split','deque','src/deque/geometry.rs','(first, sub(len, first))','(first, sub(len, head))'),
            ('heap-fast-child','binary-heap','src/collections/binary_heap/geometry.rs','    add(mul(pos, 2), 1)','    add(mul(pos, 2), 2)'),
            ('heap-select','binary-heap','src/collections/binary_heap/geometry.rs','add(left, choose_right as usize)','add(left, (!choose_right) as usize)'),
            ('heap-parent','binary-heap','src/collections/binary_heap/geometry.rs','sub(pos, 1) / 2','sub(pos, 1) / 3'),
            ('heap-child','binary-heap','src/collections/binary_heap/geometry.rs','let left = add(mul(pos, 2), 1);','let left = add(mul(pos, 2), 2);'),
            ('table-size-overflow','hash-map','src/collections/hash_map/geometry.rs','let bytes = prefix.checked_add(16).ok_or(Error::Layout)?;', 'let bytes = prefix.wrapping_add(16);'),
            ('mirror-tail','hash-map','src/collections/hash_map/geometry.rs','    add(offset, 16)', '    add(offset, 15)'),
            ('probe-mask','hash-map','src/collections/hash_map/geometry.rs','let next = sum & mask;', 'let next = sum | mask;'),
            ('bounded-add','bounded','src/arithmetic/bounded.rs','left.wrapping_add(right)','left.wrapping_sub(right)'),
            ('bounded-sub','bounded','src/arithmetic/bounded.rs','left.wrapping_sub(right)','left.wrapping_add(right)'),
            ('bounded-mul','bounded','src/arithmetic/bounded.rs','left.wrapping_mul(right)','left.wrapping_add(right)'),
            ('admission-guard','budget-admit','src/budget/admission.rs','if bytes > available {','if bytes < available {'),
            ('unmodeled-panic','budget-admit','src/budget/admission.rs','    let available = limit.checked_sub(used).ok_or(Error::Capacity)?;',
                '    if bytes == 1 { panic!("direct source control"); }\n    let available = limit.checked_sub(used).ok_or(Error::Capacity)?;'),
        ]
        for name,target,file,old,new in variants:
            if target not in targets:continue
            with tempfile.TemporaryDirectory(prefix='jzap-direct-verus-') as tmp:
                work=Path(tmp);shutil.copytree(root/'src',work/'src');shutil.copytree(root/'tools/direct-verus',work/'tools/direct-verus');path=work/file;text=path.read_text()
                assert text.count(old)==1,('direct mutation anchor requires review',name)
                path.write_text(text.replace(old,new));row=run_target(work,out/name,target,binary,True)
                controls.append(dict(name=name,detected=True,method='actual product-source solver counterexample',result=row))
    assert fingerprint()==before,'direct verification source changed during execution'
    report=dict(status='pass',kind='direct-product-source',source_sha256=before,source_unchanged=True,targets=rows,controls=controls,
        mutations_run=mutations,scope='Only the named annotated product functions are directly verified. No claim about unannotated Rust, full ownership/provenance or arbitrary callbacks follows from this result. Verus, vstd and the source-checked core specifications in tools/direct-verus/std-specs.rs remain trusted; no assumed product function bodies or disabled proof checks are used.')
    (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');return report
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--target',action='append',choices=TARGETS);p.add_argument('--mutations',action='store_true');p.add_argument('--output',type=Path,default=ROOT/'target/verification/direct-verus');a=p.parse_args()
    r=verify(out=a.output,targets=a.target,mutations=a.mutations)
    for target in r['targets']:print(target['target']+': PASS (direct product source; '+str(target['verification']['verified'])+' verifier obligations)')
    print(str(len(r['controls']))+' actual-source negative controls detected; '+str(a.output/'summary.json'))
if __name__=='__main__':main()
