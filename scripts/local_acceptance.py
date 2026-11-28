"""Rule A of issue #16 (approved by the owner on 2026-11-28): instantiated-kani+local.

docs/premise-proofs.md, "Extended acceptance and storage validity", has the
rule. A contract is accepted only when `check(root, closure, rows, verified)`
establishes, mechanically:

1. its owner (and wrapper) is closed in the scripts/owner_closure.py report;
2. its compacted maintained `requires` splits exactly into (i) the Vec
   invariant in its expanded accepted text, (ii) the storage text
   `paired_storage(s,cap,size)` of rule B, each at most once, and (iii) the
   listed local facts, each exactly once; for a contract discharged by its
   callers every conjunct is local;
3. every local fact is discharged at every bound row: by verified maintained
   callers (every product call of the contract's product function, from the
   pinned call table checked against the hash-pinned heap files, is in a
   product function whose maintained caller is verified, has its whole body
   pinned, reaches the contract in the pinned maintained call lists and is
   itself accepted), or by an actual-Rust Kani harness that runs the real
   product function with the callee at the row replaced by a pinned
   precondition stub (`-Z stubbing`), is admissible by owner_closure's rules
   and is in a group the certificate runs;
4. every bound row is in a pinned function of the heap files (and, outside
   the contract's own product function, its block calls that function).

The text correspondence of a maintained caller to its pinned product body and
of a stub to its pinned assertion is a review trigger, not a solver proof;
the allocator item of rule B and the compiler facts are trusted.
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASIS = 'instantiated-kani+local'
MODEL = 'tools/memory-proof/binary-heap.verus.rs'
INVARIANT = '{len}<={cap}<={limit},byte_valid({cap},size)'
STORAGE = 'paired_storage({s},{cap},size)'
HEAP = 'src/collections/binary_heap.rs'
CORE = 'src/collections/binary_heap/core.rs'
FILES = (HEAP, CORE, 'src/collections/binary_heap/iter.rs', 'src/collections/binary_heap/geometry.rs')
H = 'collections::binary_heap::proofs::closure::local::closure_'
PROOF = 'src/collections/binary_heap/closure_proofs.rs'
WORDS = dict(len='len', cap='cap', limit='limit', s='s')


def kani(harness, target, stub, body, runs):
    return dict(kind='kani', harness=H + harness, target=target, stub=stub, body=body, runs=runs)


CALLERS = dict(kind='callers')
# Product functions, as (file, impl type or '', function chain): the call
# pattern of each, and the maintained caller of each function that calls one.
FUNCTIONS = {
    'Hole::new': (CORE, r'\bHole::new\('), 'Hole::get': (CORE, r'\bhole\.get\('),
    'Hole::move_to': (CORE, r'\bhole\.move_to\('), 'sift_up': (None, r'\bsift_up\('),
    'sift_down': (None, r'\bsift_down\('), 'sift_bottom': (None, r'\bsift_bottom\('),
    'rebuild': (None, r'\brebuild\('), 'rebuild_tail': (None, r'\brebuild_tail\('),
    'pop_nonempty': (None, r'\bpop_nonempty\('),
}
# The pinned call table: callee -> {(file, impl, chain): count}. Valid only for
# the reviewed sha256 of the heap files (verify_binary_heap.REVIEWED).
CALLS = {
    'Hole::new': {(CORE, '', ('sift_up',)): 1, (CORE, '', ('sift_down',)): 1, (CORE, '', ('sift_bottom',)): 1},
    'Hole::get': {(CORE, '', ('sift_up',)): 1, (CORE, '', ('sift_down',)): 4, (CORE, '', ('sift_bottom',)): 3},
    'Hole::move_to': {(CORE, '', ('sift_up',)): 1, (CORE, '', ('sift_down',)): 2, (CORE, '', ('sift_bottom',)): 3},
    'sift_up': {(HEAP, 'BinaryHeap', ('try_push',)): 1, (CORE, '', ('rebuild_tail',)): 1},
    'sift_down': {(HEAP, 'PeekMut', ('drop',)): 1, (HEAP, 'BinaryHeap', ('into_sorted_vec',)): 1, (CORE, '', ('rebuild',)): 1},
    'sift_bottom': {(CORE, '', ('pop_nonempty',)): 1},
    'rebuild': {(HEAP, 'BinaryHeap', ('from_vec',)): 1, (CORE, '', ('rebuild_tail',)): 1},
    'rebuild_tail': {(HEAP, 'BinaryHeap', ('try_append',)): 1, (HEAP, 'Repair', ('try_retain', 'drop')): 1,
                     (HEAP, 'Repair', ('try_extend', 'drop')): 1},
    'pop_nonempty': {(HEAP, 'BinaryHeap', ('pop',)): 1, (HEAP, 'PeekMut', ('pop',)): 1},
}
# Product function -> (maintained caller, heap_callers.BODIES key of its body).
MAINTAINED = {
    (CORE, '', ('sift_up',)): ('heap_sift_up_caller', ('collections/binary_heap/core.rs', '', 'unsafe fn sift_up<')),
    (CORE, '', ('sift_down',)): ('heap_descend_caller', ('collections/binary_heap/core.rs', '', 'unsafe fn sift_down<')),
    (CORE, '', ('sift_bottom',)): ('heap_sift_bottom_caller', ('collections/binary_heap/core.rs', '', 'unsafe fn sift_bottom<')),
    (CORE, '', ('rebuild',)): ('heap_rebuild_caller', ('collections/binary_heap/core.rs', '', 'fn rebuild<')),
    (CORE, '', ('rebuild_tail',)): ('heap_rebuild_tail_caller', ('collections/binary_heap/core.rs', '', 'fn rebuild_tail<')),
    (CORE, '', ('pop_nonempty',)): ('heap_pop_caller', ('collections/binary_heap/core.rs', '', 'unsafe fn pop_nonempty<')),
    (HEAP, 'BinaryHeap', ('try_push',)): ('heap_push_caller', ('collections/binary_heap.rs', '', 'pub fn try_push(')),
    (HEAP, 'PeekMut', ('drop',)): ('heap_peek_drop_caller', ('collections/binary_heap.rs', "Drop for PeekMut<'_, T, P, L> {", 'fn drop(')),
    (HEAP, 'BinaryHeap', ('into_sorted_vec',)): ('heap_sorted_caller', ('collections/binary_heap.rs', '', 'pub fn into_sorted_vec(')),
    (HEAP, 'BinaryHeap', ('from_vec',)): ('heap_from_vec_caller', ('collections/binary_heap.rs', '', 'pub fn from_vec(')),
    (HEAP, 'BinaryHeap', ('try_append',)): ('heap_append_caller', ('collections/binary_heap.rs', '', 'pub fn try_append(')),
    (HEAP, 'Repair', ('try_retain', 'drop')): ('heap_retain_repair_caller', ('collections/binary_heap.rs', 'pub fn try_retain<E>(', 'fn drop(')),
    (HEAP, 'Repair', ('try_extend', 'drop')): ('heap_extend_repair_caller', ('collections/binary_heap.rs', 'pub fn try_extend<I: TryIntoIterator<Item = T>>(', 'fn drop(')),
}
POP_RUNS = ['heap.pop()', 'PeekMut::pop(guard)']
# contract -> entry. `owner_words`: the (i)/(ii) mapping, or None for a contract
# discharged by its callers. `rows`: (file, impl, function, call text or None).
CONTRACTS = {
    # Entered with a Vec at the modeled point: (i), (ii) and local facts.
    'heap_push_caller': dict(product=None, owner_words=WORDS, local={'len>0': kani(
        'push_establishes_its_local_fact', 'crate::collections::binary_heap::core::sift_up', 'sift_up_precondition',
        'assert!(data.len()>0);', ['heap.try_push(()'])},
        rows=[(HEAP, 'BinaryHeap', 'try_push', None)]),
    'heap_pop_caller': dict(product='pop_nonempty', owner_words=WORDS, local={'len>0': kani(
        'pop_establishes_its_local_fact', 'crate::collections::binary_heap::core::pop_nonempty', 'pop_nonempty_precondition',
        'assert!(data.len()>0);unsafe{data.as_ptr().read()}', POP_RUNS)},
        rows=[(CORE, '', 'pop_nonempty', None), (HEAP, 'BinaryHeap', 'pop', 'core::pop_nonempty('),
              (HEAP, 'PeekMut', 'pop', 'core::pop_nonempty(')]),
    'heap_peek_drop_caller': dict(product=None, owner_words=dict(WORDS, len='vlen'), local={
        f: kani('peek_drop_establishes_its_local_facts', 'crate::collections::binary_heap::core::sift_down',
                'sift_down_precondition', 'assert!(0<end&&end<=data.len());', ['drop(guard)'])
        for f in ('0<len', 'len<=vlen')},
        rows=[(HEAP, 'PeekMut', 'drop', 'core::sift_down(')]),
    'heap_sorted_caller': dict(product=None, owner_words=WORDS, local={},
                               rows=[(HEAP, 'BinaryHeap', 'into_sorted_vec', None)]),
    # Discharged by their verified callers: every conjunct is local.
    'heap_sift_up_caller': dict(product='sift_up', owner_words=None, local=CALLERS, rows=[(CORE, '', 'sift_up', None)]),
    'heap_descend_caller': dict(product='sift_down', owner_words=None, local=CALLERS, rows=[(CORE, '', 'sift_down', None)]),
    'heap_sift_bottom_caller': dict(product='sift_bottom', owner_words=None, local=CALLERS,
                                    rows=[(CORE, '', 'sift_bottom', None)]),
    'heap_rebuild_caller': dict(product='rebuild', owner_words=None, local=CALLERS, rows=[(CORE, '', 'rebuild', None)]),
    'heap_rebuild_tail_caller': dict(product='rebuild_tail', owner_words=None, local=CALLERS,
                                     rows=[(CORE, '', 'rebuild_tail', None)]),
    'heap_element_caller': dict(product=('Hole::new', 'Hole::get'), owner_words=None, local=CALLERS,
                                rows=[(CORE, 'Hole', 'new', None), (CORE, 'Hole', 'get', None)]),
    'heap_move_caller': dict(product='Hole::move_to', owner_words=None, local=CALLERS,
                             rows=[(CORE, 'Hole', 'move_to', None)]),
}
# Maintained callers without rows, accepted only as callers: (i) and (ii).
ENTRIES = {name: dict(owner_words=WORDS, local={}) for name in
           ('heap_from_vec_caller', 'heap_append_caller', 'heap_retain_repair_caller', 'heap_extend_repair_caller')}
# Left stated, with the reason (checked to be the complement of CONTRACTS).
LEFT = {
    'heap_restore_caller': 'Hole::drop runs as drop glue: there is no product call to a verified caller and no callee '
                           'to stub; its value storage is the stack slot of the hole',
    'heap_peek_caller': 'the rows are in Deref::deref and DerefMut::deref_mut, entered from user code: the fact '
                        '0 < len is the PeekMut guard invariant, not discharged by a caller or a stub of the rule',
}


def compact(text):
    return re.sub(r'\s+', '', re.sub(r'//[^\n]*', '', text))


def requires(root, name):
    text = (Path(root) / MODEL).read_text()
    heads = list(re.finditer(r'\nfn\s+' + name + r'\s*\(', text))
    assert len(heads) == 1, ('maintained caller missing or duplicated', name)
    rest = text[heads[0].end():]
    head = rest[:rest.index('\n{')]
    m = re.search(r'\brequires\b(.*?)(?:\bensures\b|$)', head, re.S)
    assert m, ('maintained caller without requires', name)
    return conjuncts_of(compact(m.group(1)))


def conjuncts_of(text):
    """The top-level comma-separated conjuncts of a compacted requires clause."""
    parts, depth, current = [], 0, ''
    for c in text:
        if c == ',' and depth == 0:
            parts.append(current)
            current = ''
            continue
        depth += (c in '([{') - (c in ')]}')
        current += c
    parts.append(current)
    return [p for p in parts if p]


def split(root, name, row):
    """The exact split of rule A (2); returns the local conjuncts."""
    conjuncts = requires(root, name)
    if row['owner_words'] is None:
        return conjuncts
    owner = conjuncts_of(compact(INVARIANT.format(**row['owner_words'])))
    storage = compact(STORAGE.format(**row['owner_words']))
    assert conjuncts[:len(owner)] == owner, ('requires does not start with the owner invariant', name, conjuncts)
    rest = conjuncts[len(owner):]
    assert rest[:1] == [storage], ('requires has no paired storage after the owner invariant', name, conjuncts)
    local = rest[1:]
    assert sorted(local) == sorted(row['local']) and len(set(local)) == len(local), \
        ('requires has an undischarged or missing local fact', name, local, sorted(row['local']))
    return local


def skip_generics(text):
    """`text` without a leading balanced `<...>`."""
    stripped = text.lstrip()
    if not stripped.startswith('<'):
        return stripped
    depth = 0
    for i, c in enumerate(stripped):
        depth += (c == '<') - (c == '>')
        if depth == 0:
            return stripped[i + 1:]
    return stripped


def functions(text):
    """(name, body start, body end, impl type) of every fn of a product file."""
    impls = []
    for m in re.finditer(r'\bimpl\b[^{;]*\{', text):
        header = skip_generics(m.group(0)[4:])
        target = header.split(' for ')[-1] if ' for ' in header else header
        name = re.match(r'\s*([A-Za-z_]\w*)', target)
        from owner_closure import block
        inner = block(text, m.start())
        impls.append((m.end(), m.end() + len(inner), name.group(1) if name else ''))
    found = []
    for m in re.finditer(r'\bfn\s+(\w+)', text):
        try:
            start = text.index('{', m.end())
            if ';' in text[m.end():start]:
                continue
            from owner_closure import block
            inner = block(text, m.end())
        except ValueError:
            continue
        found.append((m.group(1), start, start + len(inner) + 2, m.start()))
    return found, impls


def call_table(root):
    """The product calls of each heap function, read from the files."""
    from owner_closure import strip_comments
    table = {}
    for f in FILES:
        text = strip_comments((Path(root) / f).read_text())
        fns, impls = functions(text)
        for callee, (where, pattern) in FUNCTIONS.items():
            if where and where != f:
                continue
            for m in re.finditer(pattern, text):
                chain = tuple(n for n, s, e, h in sorted(fns, key=lambda x: x[1]) if s <= m.start() < e)
                inner = [i for i in impls if i[0] <= m.start() < i[1]]
                impl = max(inner, key=lambda i: i[0])[2] if inner else ''
                if f == CORE and impl == 'Hole':
                    impl = ''
                key = (f, impl if f == HEAP else '', chain)
                table.setdefault(callee, {})
                table[callee][key] = table[callee].get(key, 0) + 1
    return table


def reaches(callers, start, target, seen=()):
    """The maintained call graph (heap_callers.CALLERS) reaches `target` from `start`."""
    if start == target:
        return True
    for call in callers.get(start, ()):
        name = call.split('(')[0]
        if name in callers and name not in seen and reaches(callers, name, target, seen + (start,)):
            return True
    return False


def check_kani(root, discharge, closed, groups):
    import owner_closure
    from owner_closure import admissible, strip_comments
    name = discharge['harness']
    assert name in groups, ('discharge harness not in a group the certificate runs', name)
    uses = set()
    reason = admissible(Path(root), name, 'lend', {}, uses=uses)
    assert reason is None, ('discharge harness not admissible', name, reason)
    assert uses <= closed, ('discharge harness assumes an owner that is not closed', name, uses)
    text = strip_comments((Path(root) / PROOF).read_text())
    short = name.rsplit('::', 1)[1]
    head = re.search(r'#\[kani::proof\]\s*#\[kani::stub\(([^,]+),\s*(\w+)\s*\)\]\s*fn\s+' + short + r'\(', text)
    assert head and compact(head.group(1)) == discharge['target'] and head.group(2) == discharge['stub'], \
        ('discharge harness does not stub the row callee', name)
    stub = re.search(r'unsafe\s+fn\s+' + discharge['stub'] + r'\b', text)
    assert stub and compact(owner_closure.block(text, stub.end())) == discharge['body'], \
        ('precondition stub changed', discharge['stub'])
    bodies = owner_closure.functions(root, PROOF)
    harness = compact(bodies[short][0][0])
    reached = harness + ''.join(compact(b[0]) for n, rows in bodies.items() if n + '(' in harness for b in rows)
    for call in discharge['runs']:
        assert call.replace(' ', '') in reached, ('discharge harness does not run the product function', name, call)


def check(root=ROOT, closure=None, rows=(), verified=None, groups=None):
    """Accept the contracts of CONTRACTS or fail; returns contract -> instantiations."""
    import heap_callers
    import verify_binary_heap
    from owner_closure import harness_groups
    import hashlib
    root = Path(root)
    assert closure is not None and closure.get('Vec', {}).get('closed') is True \
        and closure.get('BinaryHeap', {}).get('closed') is True, ('instantiated-kani+local owner is not closed',)
    closed = {o for o, r in closure.items() if r.get('closed')}
    phases = {f: hashlib.sha256((root / f).read_bytes()).hexdigest() for f in verify_binary_heap.FILES}
    assert phases == verify_binary_heap.REVIEWED, 'heap files changed: the call table needs review'
    verify_binary_heap.correspondence(root)
    assert call_table(root) == CALLS, ('heap call table changed', call_table(root))
    assert set(CONTRACTS) | set(LEFT) == {n for n in verify_binary_heap_contracts()}, 'heap contract list changed'
    groups = harness_groups() if groups is None else groups
    accepted = {}

    def accept(name, stack=()):
        if name in accepted:
            return accepted[name]
        assert name not in stack, ('cyclic discharge', stack + (name,))
        row = CONTRACTS.get(name) or ENTRIES.get(name)
        assert row is not None, ('maintained caller is not accepted under rule A', name)
        if verified is not None:
            assert name in verified, ('maintained caller is not verified', name)
        local = split(root, name, row)
        if row['local'] is CALLERS:
            products = row['product'] if isinstance(row['product'], tuple) else (row['product'],)
            for product in products:
                callers = CALLS[product]
                assert callers, ('no product caller', name)
                for site in callers:
                    caller, key = MAINTAINED[site]
                    assert key in heap_callers.BODIES, ('caller body is not pinned', caller, key)
                    assert reaches(heap_callers.CALLERS, caller, name) and caller != name, \
                        ('maintained caller does not reach the contract', caller, name)
                    accept(caller, stack + (name,))
        else:
            for fact in local:
                discharge = row['local'][fact]
                assert discharge['kind'] == 'kani', ('unknown discharge', name, fact)
                check_kani(root, discharge, closed, groups)
        accepted[name] = ['T = () (zero-sized element) for every heap method; the Vec closure instantiations']
        return accepted[name]
    for name in CONTRACTS:
        accept(name)
    for site, contract in rows:
        if contract not in CONTRACTS:
            continue
        context = re.sub(r'\s+', '', site.get('context', '')).split('<')[0]
        expression = compact(site.get('expression', ''))
        ok = any(site.get('file') == f and context == impl and site.get('function') == fn
                 and (call is None or call in expression) for f, impl, fn, call in CONTRACTS[contract]['rows'])
        assert ok, ('instantiated-kani+local row outside its pinned functions', contract, site)
    return {name: accepted[name] for name in CONTRACTS}


def verify_binary_heap_contracts():
    """The 13 heap contracts of premise_status (its stated heap entry)."""
    return ('heap_descend_caller', 'heap_element_caller', 'heap_move_caller', 'heap_peek_caller',
            'heap_peek_drop_caller', 'heap_pop_caller', 'heap_push_caller', 'heap_rebuild_caller',
            'heap_rebuild_tail_caller', 'heap_restore_caller', 'heap_sift_bottom_caller',
            'heap_sift_up_caller', 'heap_sorted_caller')
