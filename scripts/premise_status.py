"""Premise status of the contracts that source obligations are bound to.

A verified contract proves its conclusion under its `requires`. A binding only
names the contract; it does not show that the call site establishes those
premises. A row counts as proved only when this table records how the premise
is established:

- ``no-premise``: the contract has no `requires`; it holds for every input.
- ``state-invariant``: the premise is the invariant of a closed state type
  whose constructors and transitions are verified on the product source.
- ``verified-caller``: the premise is discharged at every product call by a
  caller verified with Verus on the product source.
- ``direct-source``: the call is inside a product function whose own
  `verus_spec` is verified directly (the enclosing function is checked by
  `verify_wrapping_memory.py`).

Two further bases were approved by the owner on 2026-11-26 (issue #16). They
are reported under their own names and counts, never as
``solver-and-source-connected``, and do not block completion:

- ``instantiated-kani``: the premise is the owner invariant, closed at the
  zero-sized instantiation as decided by ``scripts/owner_closure.py`` (every
  constructor and writer runs under an actual-Rust Kani harness that assumes
  only the invariant; the writers are pinned by the scanner). It is not a
  solver proof for every ``T``; the instantiations are listed with the
  contract. ``check_instantiated`` refuses the basis when the owner is not
  closed, when the maintained ``requires`` is not exactly the owner invariant
  (plus ``call_requires``), or when a bound row is not in a function of the
  owner generic in its element type.
- ``trust-boundary``: every bound row's premise is exactly one item of the
  fixed list ``TRUST_BOUNDARY_ITEMS`` (docs/premise-proofs.md). No contract
  has this basis yet.

Every other contract is ``stated-not-proved``: its premise is written in a
maintained function or a harness assumption and no proof connects it to the
product state at the call site. An unknown contract fails closed.

Evidence paths are relative to the repository root. Line numbers are those of
the commit that introduced this table.
"""
import re
from pathlib import Path

MIGRATION = 'docs/direct-source-migration.md'

# Verus declarations that carry a proof obligation: exec and proof functions.
# A `spec fn` is a definition; its name is never evidence of a proof.
_DECLARATION = re.compile(
    r'\b((?:(?:pub(?:\([^)]*\))?|open|closed|uninterp|broadcast|const|unsafe|'
    r'spec(?:\([^)]*\))?|proof|exec)\s+)*)fn\s+(\w+)')


def verified_function_names(text):
    """Names of the exec and proof functions declared in Verus source text."""
    return {name for modifiers, name in _DECLARATION.findall(text)
            if not re.search(r'\bspec\b', modifiers)}


def _rows(basis, evidence, names):
    return {name: dict(status='established', basis=basis, evidence=evidence) for name in names}


ESTABLISHED = {
    **_rows('no-premise', 'The maintained function has no requires clause: '
            'tools/memory-proof/operation-closure.verus.rs:272 (empty_buffer), :276 (array_import); '
            'tools/memory-proof/owner-state.verus.rs:11 (empty_owner); '
            'tools/memory-proof/shared.verus.rs:161 (concurrent_admit), :251 (shared_increment_caller), '
            ':301 (shared_clone_weak_caller); tools/memory-proof/scratch.verus.rs:120; '
            'tools/memory-proof/btree.verus.rs:203; tools/direct-verus/budget-ledger.rs:57; '
            'tools/direct-verus/state-adapters.rs:9, :12, :15, :55; tools/direct-verus/init-adapters.rs:4; '
            'tools/memory-proof/wrapping-memory.verus.rs:75',
            ['empty_buffer', 'array_import', 'empty_owner', 'concurrent_admit',
             'shared_increment_caller', 'shared_clone_weak_caller', 'scratch_admitted_length_caller',
             'btree_initial_state', 'ledger_new', 'c_full', 'c_first_kept', 'c_try_prefix',
             'e_try_range', 'initialized_box_prefix', 'external_count_before_allocation']),
    **_rows('no-premise', 'replaced_owner has no requires (tools/memory-proof/operation-closure.verus.rs): '
            'each bound row is `*self = replacement`, the move of one complete safe value into a Box '
            'owner (src/boxed.rs, src/boxed/foreign.rs, src/boxed/slice.rs, src/boxed/string.rs, '
            'src/clone.rs), and the contract states only that the published words are the replacement\'s',
            ['replaced_owner']),
    **_rows('no-premise', 'The only requires of staged_collection_loop '
            '(tools/memory-proof/edit-loops.verus.rs) is call_requires of its two callbacks, which '
            'holds for every safe Rust closure; it has no numeric premise. The numeric requires of the '
            'driver it models is the premise of staged_collection_extent, which is stated-not-proved',
            ['staged_collection_loop']),
    **_rows('no-premise', 'safe_iterator_state has no requires (tools/memory-proof/operation-closure.verus.rs). '
            'Its eight rows are in src/iter.rs, which has no unsafe code and no storage state '
            '(scripts/caller_bindings.py asserts both): checked countdowns of Take and the default '
            'methods, a moved fold value and the fields of Take, FromStd and FromResults',
            ['safe_iterator_state']),
    **_rows('no-premise', 'guarded_increment has no requires (tools/direct-verus/init-adapters.rs) and '
            'calls the product wrapping_add under the same guard. Its two rows are the increments '
            'of init::driver::run (after `while state.get() < end`) and vec::staging_driver::run '
            '(after `if len >= limit { return }`); verify_wrapping_memory.guarded_stores pins each '
            'guard next to its increment, so neither can wrap whatever the caller passed. The '
            'numeric requires of the two functions is a separate premise (admitted_cursor, '
            'staged_collection_extent) and stays stated',
            ['guarded_increment']),
    **_rows('no-premise', 'Actual-Rust Kani harness over every machine word without an assumption: '
            'src/span.rs:116 (Span::full), src/init/collection_proofs.rs:5',
            ['kani:full_constructor_establishes_order',
             'kani:local_cursor_publication_preserves_every_word']),
    **_rows('state-invariant', 'Compaction/Extraction wf(): private fields (src/compaction.rs:8, :210); '
            'every constructor ensures wf (src/compaction.rs:36-72, :253-265) and every transition '
            'preserves it, verified on the product methods by the direct-foundation state group '
            '(scripts/verify_source.py:128-134); '
            'the adapters (tools/direct-verus/state-adapters.rs:30-92) require only wf',
            ['c_keep_next', 'c_remove_next', 'c_restored_len', 'c_remaining',
             'e_keep_next', 'e_remove_next', 'e_stop', 'e_written', 'e_remaining',
             'e_unvisited_len', 'e_restored_len']),
    **_rows('state-invariant', 'Kani harness src/vec/edit_proofs.rs:11 assumes only the Extraction '
            'invariant write <= read <= end <= total, closed as for e_* above',
            ['kani:extraction_drop_restores_every_numeric_cursor_exactly']),
    **_rows('state-invariant', 'Span front <= back: Span::try_new rejects front > back and Span::full '
            'starts at 0 (src/span.rs:33, :45); src/span.rs:125 checks both transitions and clear for '
            'every word (state group, scripts/verify_source.py:128-134); '
            'tools/direct-verus/state-adapters.rs:97, :103 require only front <= back',
            ['kani:both_transitions_preserve_order_and_exact_length', 'cursor_remaining',
             'iterator_cursor']),
    **_rows('state-invariant', 'CharBytes position <= len <= 4: the only constructor sets position 0 '
            'and len 0 (field check, scripts/caller_bindings.py:455-457); the utf8-native Kani harness '
            'src/boxed/text_proofs.rs:5-22 runs try_next on every valid state and an arbitrary char',
            ['utf8_byte_cursor']),
    **_rows('direct-source', 'The bound site is inside a product function with its own verus_spec, '
            'verified by the named target of verify_direct_verus.py',
            ['direct:budget-admit', 'direct:shared-numeric', 'direct:fixed-bplus',
             'direct:bplus-partial']),
}


# The fixed trust boundary (docs/premise-proofs.md, "Trust boundary"). A reason
# outside this list is not accepted.
TRUST_BOUNDARY_ITEMS = {
    'core-semantics': 'The compiler, core and alloc behave as documented, including the validity of safe '
                      'references and slices',
    'allocator-storage': 'A successful allocation or reallocation returns storage valid for the requested layout',
    'public-unsafe-caller': 'The caller of a public unsafe fn meets its documented contract',
    'std-adoption': 'Values adopted from std satisfy the std invariants',
    'returning-callback': 'User callbacks return and satisfy the stated trait laws',
}
# contract -> item of TRUST_BOUNDARY_ITEMS. Considered at step 9 and left
# stated: raw_parts_owner (its rows are the literal of Vec::from_raw_parts,
# also reached from library callers in src/deque.rs), adopted_owner (also
# requires the budget invariant), capacity_bytes (Capacity::from_validated_raw
# is pub(crate), not a public unsafe contract).
TRUST_BOUNDARY = {}

# The owner invariants in the words of the maintained functions, and the
# instantiations at which scripts/owner_closure.py closes them.
OWNER_INVARIANT = {
    'Buffer': dict(requires='{len}<={cap},byte_valid({cap},size)',
                   files=('src/boxed/buffer.rs',),
                   instantiations=['T = () for every method and lend (zero-sized element, Unmetered)',
                                   'T = u8, u64, [u64; 200] and a 64-aligned 64-byte type for reserve and finish, '
                                   'with storage::reallocate and storage::deallocate stubbed']),
    # Issue #16, unit 4: Vec closes jointly with VecDeque. The maintained
    # functions state the Vec invariant in two texts: `owner_valid` of an
    # OwnerState, or its expansion over the three words (`form='expanded'`).
    'Vec': dict(requires='owner_valid({s},size)',
                expanded='{len}<={cap}<={limit},byte_valid({cap},size)',
                files=('src/vec.rs', 'src/vec/convenience.rs', 'src/vec/edit.rs', 'src/vec/ownership.rs'),
                instantiations=['T = () for every Vec method and lend (zero-sized element, Unmetered; one exchange '
                                'fixture with Budget::Limited), jointly with VecDeque (scripts/owner_closure.py)',
                                'T = [(); 1] and [(); N] for the bitwise extension and try_into_flattened',
                                'T = u8, u64, [u64; 200] and a 64-aligned 64-byte type for the capacity paths '
                                '(try_reserve, try_reserve_exact, grow, reserve_length, try_shrink_to, the capacity '
                                'constructors), with the storage functions stubbed']),
}
_BUFFER_EVIDENCE = ('scripts/owner_closure.py reports Buffer closed at the zero-sized element (constructor new, the '
                    'writes of reserve, push, append and finish, the lends of collect and collect_rest, each under '
                    'a closure harness of the buffer group); the maintained requires is the Buffer invariant '
                    'len <= cap with the byte bound, which the harness invariant (layout::array::<T>(cap) == layout) '
                    'implies')
_VEC_EVIDENCE = ('scripts/owner_closure.py reports Vec closed at the zero-sized element jointly with VecDeque (every '
                 'Vec constructor, length and capacity store, lend and library call of Vec::from_raw_parts under a '
                 'closure harness, except the trust-boundary literal of the public unsafe Vec::from_raw_parts, two '
                 'u8-only and two unobservable stores; the VecDeque into_vec harness assumes the VecDeque invariant, '
                 'closed with the retain loop contract approved on 2026-11-28); the maintained requires is the Vec '
                 'invariant len <= cap <= limit with the byte bound, which is vacuous at T = () and asserted by the '
                 'sized capacity harnesses')
_EVIDENCE = {'Buffer': None, 'Vec': _VEC_EVIDENCE}
INSTANTIATED_KANI = {
    'buffer_reserve': dict(owner='Buffer', maintained='tools/memory-proof/owner-state.verus.rs',
                           words=dict(len='len', cap='cap'), call_requires=()),
    'buffer_push': dict(owner='Buffer', maintained='tools/memory-proof/operation-closure.verus.rs',
                        words=dict(len='len', cap='cap'), call_requires=()),
    'buffer_finish': dict(owner='Buffer', maintained='tools/memory-proof/operation-closure.verus.rs',
                          words=dict(len='len', cap='cap'), call_requires=()),
    'buffer_collect': dict(owner='Buffer', maintained='tools/memory-proof/operation-closure.verus.rs',
                           words=dict(len='initial', cap='initial_cap'), call_requires=('cb', 'alloc')),
    # Vec (issue #16, unit 4 step E): requires exactly the Vec invariant, every row in Vec<T, P, L>.
    **{name: dict(owner='Vec', maintained='tools/memory-proof/' + file, words=dict(s=s), call_requires=())
       for name, file, s in (('checked_owner_length', 'owner-state.verus.rs', 's'),
                             ('split_owner', 'owner-state.verus.rs', 'original'),
                             ('shortened_owner', 'operation-closure.verus.rs', 's'),
                             ('owner_to_array', 'operation-closure.verus.rs', 's'))},
    **{name: dict(owner='Vec', maintained='tools/memory-proof/' + file, form='expanded',
                  words=dict(len='len', cap='cap', limit='limit'), call_requires=())
       for name, file in (('memory_reserve', 'wrapping-memory.verus.rs'),
                          ('insertion_before_moves', 'wrapping-memory.verus.rs'),
                          ('internal_copy_before_append', 'wrapping-memory.verus.rs'),
                          ('drain_constructor', 'operation-closure.verus.rs'))},
}


def maintained_requires(root, contract):
    """The compacted `requires` clause of a maintained function, from its source."""
    row = INSTANTIATED_KANI[contract]
    text = (Path(root) / row['maintained']).read_text()
    heads = [m for m in re.finditer(r'\bfn\s+' + contract + r'\s*[<(]', text)]
    assert len(heads) == 1, ('maintained function missing or duplicated', contract)
    rest = text[heads[0].end():]
    # The clause ends at `ensures`, or at the body when there is none.
    m = re.search(r'\brequires\b(.*?)(?:\bensures\b|$)', rest[:rest.index('{')] if '{' in rest else rest, re.S)
    assert m, ('maintained function without requires', contract)
    return re.sub(r'\s+', '', m.group(1))


def expected_requires(contract):
    row = INSTANTIATED_KANI[contract]
    text = OWNER_INVARIANT[row['owner']][row.get('form', 'requires')].format(**row['words'])
    for callback in row['call_requires']:
        text += ',forall|i:usize|call_requires(*' + callback + ',(i,))'
    return text + ','


def check_instantiated(root, closure, rows=()):
    """Refuse the basis instantiated-kani unless it is established mechanically.

    `closure` is the report of scripts/owner_closure.py; `rows` are certificate
    rows (site, contract). Returns the instantiations per contract."""
    listed = {}
    for contract, row in INSTANTIATED_KANI.items():
        owner = row['owner']
        assert closure is not None and closure.get(owner, {}).get('closed') is True, \
            ('instantiated-kani contract whose owner is not closed', contract, owner)
        actual = maintained_requires(root, contract)
        assert actual == expected_requires(contract), \
            ('maintained requires is not exactly the owner invariant', contract, actual, expected_requires(contract))
        listed[contract] = OWNER_INVARIANT[owner]['instantiations']
    for site, contract in rows:
        if contract not in INSTANTIATED_KANI:
            continue
        owner = INSTANTIATED_KANI[contract]['owner']
        context = re.sub(r'\s+', '', site.get('context', ''))
        assert site.get('file') in OWNER_INVARIANT[owner]['files'] and context.startswith(owner + '<T,'), \
            ('instantiated-kani row outside a function of the owner generic in its element', contract, site)
    return listed


def _stated(reason, names):
    return {name: dict(status='stated-not-proved', basis='stated-not-proved', evidence=reason) for name in names}


STATED = {
    **_stated(f'Vec/Buffer owner premises (len <= cap <= limit, owner_valid, byte_valid, the Drain/hole '
              f'invariants, the range validator result, the reservation result) are requires of the '
              f'maintained function; {MIGRATION}:1882-1886, 1949-1951',
              ['adopted_owner', 'append_owner', 'boxed_to_owner',
               'exchanged_owners',
               'buffer_span', 'checked_index_before_access',
               'clone_within_operation', 'commit_reserved_owner',
               'compaction_before_move', 'compaction_before_element', 'complete_splice',
               'cursor_before_read', 'dedup_loop', 'extend_operation',
               'extract_next_loop', 'extraction_before_move', 'hole_move', 'hole_restore',
               'initialized_operation', 
               'owned_resize_operation', 'prefix_storage_connection',
               'removal_before_moves', 'reservation_before_bulk_write', 'resized_owner',
               'retained_loop', 'spare_view_before_slice', 'splice_fill_loop',
               'splice_operation', 'stage_operation',
               'swap_remove_before_moves', 'vectored_owner', 'owner_remaining_limit',
               'owner_spare_capacity', 'drain_before_tail_restore',
               'span_clear_caller', 'span_next_caller', 'span_next_back_caller', 'span_remaining_caller']),
    # A directly verified function does not discharge its own requires. The two
    # stores below follow a guard inside such a function, so the modular call
    # cannot wrap; the function's numeric requires still has to be met by its
    # product callers, and those are not verified.
    **_stated('The store `*state = Checked::new(wrapping_add(index, 1))` follows the loop guard '
              '`state.get() < end` inside init::driver::run (src/init/driver.rs), which has its own '
              'verus_spec, verified by the direct target init-prefix. That function requires '
              '`old(state)@ <= end`; its product callers init::prefix::initialize (a cursor started '
              'at 0) and init::collection::fill (for Buffer::collect, Buffer::collect_rest, '
              'Splice::fill and stage_batch, which pass an owner length and an end) are not verified '
              'to meet it. admitted_cursor (tools/direct-verus/init-adapters.rs) restates the same '
              'requires',
              ['admitted_cursor']),
    **_stated('`wrapping_add(len, 1)` follows `if len >= limit { return }` inside '
              'vec::staging_driver::run (src/vec/staging_driver.rs), which has its own verus_spec, '
              'verified by the direct target vec-staging. That function requires '
              '`initial_cap <= limit` and `byte_valid(initial_cap, size)`; its one product caller, '
              'Splice::stage_excess (src/vec/edit.rs), is not verified to meet them. '
              'staged_collection_extent (tools/direct-verus/vec-adapters.rs) restates the same '
              'requires',
              ['staged_collection_extent']),
    **_stated('len <= cap <= limit and the byte bound are the public unsafe contract of '
              'Vec::from_raw_parts (src/vec/ownership.rs). For an external caller it is a promise no '
              'library proof establishes (trust boundary). The library also calls it itself '
              '(src/deque.rs, src/vec/convenience.rs, src/string/convert.rs) and those calls are not '
              'verified to meet it',
              ['raw_parts_owner']),
    **_stated('Requires wf and read < original. compact_one (src/vec/compaction_driver.rs) is verified '
              'and establishes it, but VecDeque::try_retain_mut (src/deque/edit.rs) also calls '
              'keep_live/remove_live and is unverified Rust; caller_bindings.py does not see that '
              'caller because deque files are filtered out',
              ['c_keep_live', 'c_remove_live']),
    **_stated('Capacity cap * size <= isize::MAX: Capacity::from_validated_raw is an unsafe constructor '
              'whose bound is a caller obligation (src/capacity.rs), not a verified invariant',
              ['capacity_bytes']),
    **_stated('write > 0 is required (tools/direct-verus/state-adapters.rs:27); the try_dedup_by '
              'closure that calls previous_kept is unverified Rust',
              ['c_previous_kept']),
    **_stated('The Kani harness assumes the owner premise len <= cap <= limit '
              '(src/vec/staging_proofs.rs:10, src/vec/cursor_proofs.rs:35, :46)',
              ['kani:staging_push_preserves_every_admitted_word_owner',
               'kani:suffix_guard_restores_every_admitted_original_length',
               'kani:within_capacity_push_preserves_every_numeric_owner']),
    **_stated('Ghost ledger premises (wf, key membership, owns(l, c)) and the local budget invariant '
              'occupied + available <= limit with charge ownership for release are stated; the '
              'correspondence of MemoryBudget to the ghost ledger is not proved '
              '(tools/direct-verus/budget-ledger.rs, budget-adapters.rs, '
              'tools/memory-proof/owner-credit.verus.rs, shared.verus.rs:172)',
              ['ledger_reserve', 'ledger_commit', 'ledger_cancel', 'ledger_release',
               'allocation_credit', 'reallocation_credit', 'released_credit', 'transferred_credit',
               'concurrent_commit_preserves_tokens', 'reserved', 'reserve', 'commit', 'cancel',
               'release']),
    **_stated('The Kani harness assumes the budget invariant (src/budget_proofs.rs:4-13) whose '
              'preservation by release rests on the stated charge-ownership premise',
              ['kani:budget::panic_proofs::reservation_and_commit_preserve_every_valid_state']),
    **_stated('Bounded Kani scenarios on concrete small states; they do not establish the premise for '
              'every state reaching the bound site',
              ['kani:budget::concurrent::proofs::actual_concurrent_reservation_commit_and_cancel',
               'kani:collections::binary_heap::proofs::actual_owner_pop_peek_and_drain_protocols',
               'kani:collections::hash_map::proofs::hash_set_replace_preserves_slot_class_length_and_credit',
               'kani:deque::proofs::actual_clone_limits_and_consuming_fold',
               'kani:deque::proofs::actual_insert_remove_and_drain',
               'kani:deque::proofs::actual_ring_moves_and_contiguous_growth',
               'kani:scratch::proofs::actual_borrowed_admission_rollback_and_bytes',
               'kani:scratch::proofs::actual_owned_storage_never_grows_after_construction']),
    **_stated(f'B-tree tree premises (counts <= 11, cursor and parent indices, heights, node shape) '
              f'are stated; {MIGRATION}:2424-2432, 2819-2823',
              ['btree_actual_drop_skip', 'btree_actual_insert', 'btree_actual_leaf_edge',
               'btree_actual_merge', 'btree_actual_move_suffix', 'btree_actual_remove_leaf',
               'btree_actual_split', 'btree_actual_steal_left', 'btree_actual_steal_right',
               'btree_borrow_interval_caller', 'btree_borrow_remaining_caller', 'btree_borrow_take_caller',
               'btree_bound_caller', 'btree_bulk_push_caller', 'btree_bytes_caller', 'btree_cleanup_prefix',
               'btree_clone_caller', 'btree_closed_node_count', 'btree_commit_fit_caller',
               'btree_commit_split_caller', 'btree_count_caller', 'btree_count_index', 'btree_cursor',
               'btree_cursor_state', 'btree_cursor_step_caller', 'btree_descend_caller',
               'btree_destroy_caller', 'btree_drain_caller', 'btree_edge_address_index', 'btree_edge_index',
               'btree_edges_index', 'btree_fix_border_caller', 'btree_fix_caller', 'btree_found_slot_caller',
               'btree_layout_slot', 'btree_ordered_caller', 'btree_packed_border_caller',
               'btree_pair_address_index', 'btree_pair_index', 'btree_pairs_index', 'btree_pillar_caller',
               'btree_plan_caller', 'btree_planned_allocation', 'btree_rebalance_caller',
               'btree_replace_index', 'btree_replace_key_caller', 'btree_retire_chain_caller',
               'btree_search_node_caller', 'btree_shrink_caller', 'btree_split_off_caller',
               'btree_state_take_caller', 'btree_view_child_caller', 'btree_view_index']),
    **_stated(f'Hash-table shape, credit and tag/ownership invariants are stated; '
              f'{MIGRATION}:2114-2132, 2312-2322',
              ['hash_allocate_caller', 'hash_allocation_caller', 'hash_clear_run_caller', 'hash_clone_caller',
               'hash_erase_caller', 'hash_find_insert_caller', 'hash_insert_at_caller', 'hash_layout_ranges',
               'hash_neon_mask_octet', 'hash_neon_nibble_pair', 'hash_physical_duplicates',
               'hash_probe_caller', 'hash_rehash_pending', 'hash_reset_caller', 'hash_resize_caller',
               'hash_retire_caller', 'hash_scan_caller', 'hash_set_control_caller', 'hash_slot_caller']),
    **_stated(f'Heap premises (raw_valid storage, slice length equals the heap length, at most '
              f'isize::MAX elements) are stated; {MIGRATION}:2570-2574, 2771-2775. Issue #16 unit 7: '
              f'scripts/owner_closure.py reports BinaryHeap closed as a wrapper of its one Vec word, but no '
              f'maintained requires is exactly the Vec invariant (each adds raw_valid storage and local index '
              f'facts) and every row is in a heap function, so instantiated-kani does not apply',
              ['heap_descend_caller', 'heap_element_caller', 'heap_move_caller', 'heap_peek_caller',
               'heap_peek_drop_caller', 'heap_pop_caller', 'heap_push_caller', 'heap_rebuild_caller',
               'heap_rebuild_tail_caller', 'heap_restore_caller', 'heap_sift_bottom_caller',
               'heap_sift_up_caller', 'heap_sorted_caller']),
    **_stated(f'Ring premises (ring(head, len, cap), reservation results, target capacities) are stated; '
              f'{MIGRATION}:2644-2652',
              ['deque_append_caller', 'deque_clone_slice_caller', 'deque_clone_slices_caller',
               'deque_contiguous_caller', 'deque_copy_ring_caller', 'deque_deallocate_caller',
               'deque_drain_drop_caller', 'deque_extend_copy_caller', 'deque_front_caller', 'deque_get_caller',
               'deque_grow_caller', 'deque_insert_caller', 'deque_normalize_caller', 'deque_pop_back_caller',
               'deque_pop_front_caller', 'deque_push_back_caller', 'deque_push_front_caller',
               'deque_raw_parts_caller', 'deque_remove_caller', 'deque_resize_copy_caller',
               'deque_retain_caller', 'deque_rotate_caller', 'deque_search_caller', 'deque_shrink_caller',
               'deque_slices_caller', 'deque_split_off_caller', 'deque_write_slice_caller']),
    **_stated(f'The scratch owner length is at most the fixed capacity: stated; {MIGRATION}:2532',
              ['scratch_cloned_caller', 'scratch_copy_caller', 'scratch_pop_caller', 'scratch_prefix_caller',
               'scratch_push_caller', 'scratch_resize_init_caller', 'scratch_truncate_caller']),
    **_stated(f'Shared counter words of a live owner (shared_valid) and the allocation/payload extents '
              f'are stated; {MIGRATION}:2970-2975, 2991-2996',
              ['shared_buffered_clone_caller', 'shared_bytes_copy_caller', 'shared_count_credit',
               'shared_cyclic_publish', 'shared_data_caller', 'shared_header_caller', 'shared_last_strong',
               'shared_last_weak', 'shared_layout_extent', 'shared_payload_extent', 'shared_pointer_frame',
               'shared_prefix_driver', 'shared_release_caller', 'shared_release_credit',
               'shared_slice_copy_caller', 'shared_source_retirement', 'shared_typed_transfer',
               'shared_value_caller', 'shared_buffered_reservation_extent']),
    **_stated('String byte extents: string_push_caller and string_replace_caller require the owner '
              'premises len <= cap <= limit and raw_valid storage (tools/memory-proof/string.verus.rs:113, '
              ':136); retain_next, retain_restore and string_drain_restore require the retain/drain cursor '
              'state (read <= original, written <= read, a valid range; :3, :16, :71). The product '
              'String methods are not verified against these requires',
              ['string_push_caller', 'string_replace_caller', 'retain_next', 'retain_restore',
               'string_drain_restore']),
    **_stated('UTF-8 validity of the owner bytes is the String type invariant: every mutator writes '
              'scalars encoded by write_scalar (proved valid UTF-8 for every char by this harness, '
              'src/string/proofs.rs:56) or copies validated UTF-8, std String values are valid by the std '
              'invariant, and from_raw_parts is a public unsafe caller contract; no proof closes the '
              'invariant over every mutator',
              ['kani:string::proofs::scalar_encoding_fits_four_bytes_and_is_utf8']),
    **_stated('Projection lemma over Seq<bool> (tools/consumer-audit/bplus_core_guards.rs:69-91); the '
              'product descend loop is bound to it only by a whole-file SHA-256 '
              '(scripts/bplus_bindings.py) and is not verified against initialized_ancestors',
              ['lookup_path_read']),
}

INSTANTIATED = 'instantiated-kani'
TRUSTED = 'trust-boundary'
_INSTANTIATED_ROWS = {name: dict(status=INSTANTIATED, basis=INSTANTIATED, owner=row['owner'],
                                 instantiations=OWNER_INVARIANT[row['owner']]['instantiations'],
                                 evidence=_EVIDENCE[row['owner']] or _BUFFER_EVIDENCE) for name, row in INSTANTIATED_KANI.items()}
_TRUSTED_ROWS = {name: dict(status=TRUSTED, basis=TRUSTED, item=item, evidence=TRUST_BOUNDARY_ITEMS[item])
                 for name, item in TRUST_BOUNDARY.items()}
_tables = (ESTABLISHED, STATED, _INSTANTIATED_ROWS, _TRUSTED_ROWS)
assert sum(map(len, _tables)) == len(set().union(*_tables)), 'a contract has two premise statuses'
PREMISES = {**ESTABLISHED, **STATED, **_INSTANTIATED_ROWS, **_TRUSTED_ROWS}

PROVED = 'solver-and-source-connected'
STATED_NOT_PROVED = 'premise-stated-not-proved'
UNPROVED = 'unproved'
# Every disposition, in report order. The two approved bases are separate.
DISPOSITIONS = (PROVED, INSTANTIATED, TRUSTED, STATED_NOT_PROVED, UNPROVED)


def premise(contract):
    """The recorded premise status of one contract; unknown contracts fail closed."""
    assert contract in PREMISES, ('contract has no recorded premise status', contract)
    return PREMISES[contract]


def disposition(contract, verified, closure=None):
    """One rule for every certificate row.

    A contract that is not in the verified set is unproved. A verified contract
    is solver-and-source-connected only when its premise is established;
    instantiated-kani only while `closure` (the owner_closure.py report) has its
    owner closed, and the call fails otherwise; trust-boundary when it is listed
    with an item of the fixed list; otherwise its premise is stated and not
    proved.
    """
    if contract not in verified:
        return UNPROVED
    status = premise(contract)['status']
    if status == 'established':
        return PROVED
    if status == INSTANTIATED:
        owner = INSTANTIATED_KANI[contract]['owner']
        assert closure is not None and closure.get(owner, {}).get('closed') is True, \
            ('instantiated-kani contract whose owner is not closed', contract, owner)
        return INSTANTIATED
    if status == TRUSTED:
        return TRUSTED
    return STATED_NOT_PROVED


def row_premise(contracts):
    """Premise status of a modular call bound to several contracts."""
    statuses = [premise(name)['status'] for name in contracts]
    return 'established' if statuses and all(s == 'established' for s in statuses) else 'stated-not-proved'
