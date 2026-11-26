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

Every other contract is ``stated-not-proved``: its premise is written in a
maintained function or a harness assumption and no proof connects it to the
product state at the call site. An unknown contract fails closed.

Evidence paths are relative to the repository root. Line numbers are those of
the commit that introduced this table.
"""
import re

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
    **_rows('state-invariant', 'Extent wf(): len <= cap <= limit.bound(), and Capacity wf(): a sized '
            'element spans at most isize::MAX bytes (src/extent.rs:44-57, src/capacity.rs:62-73). The '
            'fields are private; every constructor establishes and every transition preserves both for '
            'arbitrary arguments, verified on the product methods for every element type and length '
            'policy (verify_direct_verus.py target vec-extent) and closed by verify_owner_extent.py '
            '(verify_source.py group owner-extent). Every row bound to these contracts is in a Vec '
            'method, its Drop or a free function that receives the Vec or its Extent, and reads the '
            'words through the getters (src/extent.rs:152-190). The requires of each contract is '
            'exactly that invariant of each owner it takes; resized_owner also requires the new '
            'capacity between the length and the limit, which is the invariant of the Extent that '
            'holding/resized return. staged_collection_extent takes the capacity of a fresh Vec built '
            'with the staging limit (src/vec/edit.rs, stage_excess), so its premise is that Vec\'s '
            'invariant. A call_requires clause states that a callback accepts every argument, which '
            'holds for every safe Rust closure and trait method',
            ['append_owner', 'checked_owner_length', 'commit_reserved_owner', 'split_owner',
             'vectored_owner', 'complete_splice', 'drain_constructor', 'extend_operation',
             'initialized_operation', 'owner_to_array', 'shortened_owner', 'resized_owner',
             'checked_index_before_access', 'insertion_before_moves', 'internal_copy_before_append',
             'memory_reserve', 'removal_before_moves', 'swap_remove_before_moves',
             'spare_view_before_slice', 'clone_within_operation', 'owned_resize_operation',
             'retained_loop', 'dedup_loop', 'owner_remaining_limit', 'owner_spare_capacity',
             'staged_collection_extent']),
    **_rows('state-invariant', 'Kani harnesses that assume only the Extent invariant len <= cap <= limit, '
            'closed as above (src/vec/staging_proofs.rs:10, src/vec/cursor_proofs.rs:35)',
            ['kani:staging_push_preserves_every_admitted_word_owner',
             'kani:within_capacity_push_preserves_every_numeric_owner']),
    **_rows('verified-caller', 'Requires wf and read < original. The only product caller is '
            'compact_one (src/vec/compaction_driver.rs:54-83), verified with its verus_spec; '
            'caller_bindings.py admits no other call of keep_live/remove_live',
            ['c_keep_live', 'c_remove_live']),
    **_rows('direct-source', 'The bound site is inside a product function with its own verus_spec, '
            'verified by the named target of verify_direct_verus.py',
            ['direct:budget-admit', 'direct:shared-numeric', 'direct:fixed-bplus',
             'direct:bplus-partial']),
}


def _stated(reason, names):
    return {name: dict(status='stated-not-proved', basis='stated-not-proved', evidence=reason) for name in names}


STATED = {
    **_stated(f'Buffer owner premises, the Drain/hole invariants, the range validator result, the '
              f'reservation result, the cursor and guard states tied to the owner and the adoption of '
              f'std storage are requires of the maintained function; {MIGRATION}:1882-1886, 1949-1951',
              ['adopted_owner', 'boxed_to_owner', 'buffer_collect', 'buffer_finish', 'buffer_push',
               'buffer_reserve', 'buffer_span', 'compaction_before_move', 'compaction_before_element',
               'cursor_before_read', 'extract_next_loop', 'extraction_before_move', 'hole_move',
               'hole_restore', 'prefix_storage_connection', 'replaced_owner',
               'reservation_before_bulk_write', 'splice_fill_loop', 'splice_operation',
               'stage_operation', 'staged_collection_loop', 'admitted_cursor',
               'drain_before_tail_restore', 'span_clear_caller', 'span_next_caller',
               'span_next_back_caller', 'span_remaining_caller']),
    **_stated('len <= cap <= limit and the byte bound are the public unsafe contract of '
              'Vec::from_raw_parts (src/vec/ownership.rs), passed unchecked to the trusted entries '
              'Extent::from_raw_parts and Capacity::from_validated_raw: a promise of the caller, which no '
              'library proof establishes (trust boundary)',
              ['raw_parts_owner']),
    **_stated('Capacity cap * size <= isize::MAX: Capacity::from_validated_raw is an unsafe constructor '
              'whose bound is a caller obligation (src/capacity.rs), not a verified invariant',
              ['capacity_bytes']),
    **_stated('write > 0 is required (tools/direct-verus/state-adapters.rs:27); the try_dedup_by '
              'closure that calls previous_kept is unverified Rust',
              ['c_previous_kept']),
    **_stated('The Kani harness assumes original <= initialized besides the owner invariant '
              '(src/vec/cursor_proofs.rs:46); the guard state is not closed',
              ['kani:suffix_guard_restores_every_admitted_original_length']),
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
              f'isize::MAX elements) are stated; {MIGRATION}:2570-2574, 2771-2775',
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

assert not set(ESTABLISHED) & set(STATED), set(ESTABLISHED) & set(STATED)
PREMISES = {**ESTABLISHED, **STATED}

PROVED = 'solver-and-source-connected'
STATED_NOT_PROVED = 'premise-stated-not-proved'
UNPROVED = 'unproved'


def premise(contract):
    """The recorded premise status of one contract; unknown contracts fail closed."""
    assert contract in PREMISES, ('contract has no recorded premise status', contract)
    return PREMISES[contract]


def disposition(contract, verified):
    """One rule for every certificate row.

    A contract that is not in the verified set is unproved. A verified contract
    is solver-and-source-connected only when its premise is established;
    otherwise its premise is stated and not proved.
    """
    if contract not in verified:
        return UNPROVED
    return PROVED if premise(contract)['status'] == 'established' else STATED_NOT_PROVED


def row_premise(contracts):
    """Premise status of a modular call bound to several contracts."""
    statuses = [premise(name)['status'] for name in contracts]
    return 'established' if statuses and all(s == 'established' for s in statuses) else 'stated-not-proved'
