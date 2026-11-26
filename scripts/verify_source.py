#!/usr/bin/env python3
"""Verify selected original Rust groups; never generate a positive proof body.

Verus includes executable product modules. Kani compiles the product crate and
its in-crate harnesses. Negative controls edit disposable source copies and must
reach a solver counterexample, rather than fail compilation.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from verify_direct_verus import verify as verus, executable

ROOT = Path(__file__).resolve().parents[1]
GROUPS = {
    'vec-staging': dict(harnesses=[
        'vec::edit::staging_proofs::staging_push_preserves_every_admitted_word_owner',
        'vec::edit::staging_proofs::staging_controller_preserves_word_extents_and_source_order',
        'vec::edit::staging_proofs::staging_push_preserves_every_three_byte_prefix',
    ], loop_contracts=False, verus=['vec-staging'], scope='Executable staging controller in both Verus word domains; actual owning Vec push over every admitted native-word unit owner and bounded-source executions with all-word limits/capacities. Unabstracted byte-prefix leaf checks and existing Vec Miri/ASan cover nonzero storage separately.'),
    'vec-splice-fill': dict(harnesses=[
        'vec::edit::panic_proofs::splice_fill_preserves_every_admitted_word_hole_and_tail',
        'init::collection::proofs::local_cursor_publication_preserves_every_word',
        'init::collection::proofs::local_fill_publishes_every_admitted_word_prefix',
    ], cfgs=['jzap_init_loop_contracts'], verus=['init-prefix'], scope='Actual Splice fill delegates to the original initialization cursor. Both Verus word domains retain exact successful history and progress; actual Rust covers every valid native-word hole/tail coordinate and arbitrary source end/error/success, exact length publication and tail restoration. Existing unabstracted typed initialization checks and Vec memory scenarios are reused.'),
    'vec-extraction': dict(harnesses=[
        'vec::edit::panic_proofs::extraction_preserves_every_valid_cursor_state',
    ], verus=['vec-extraction'], scope='Original executable selected-first ExtractIf scan and conditional compaction over every admitted machine-word cursor state and arbitrary callback replies in both Verus widths. Native Rust checks the actual iterator and cold error transition; typed contents and provenance are checked separately.'),
    'vec-extraction-values': dict(harnesses=[
        'vec::edit::extraction_proofs::extraction_errors_mutation_and_order_match_standard_for_every_three_element_input',
    ], loop_contracts=False, object_bits=16, scope='Actual unabstracted Vec ExtractIf/std content, error payload, callback count and retained order for all byte payloads, Boolean replies, ranges and error ordinals over lengths zero through three. Unwinding assertions remain enabled; separate from the full-word control proof.'),
    'vec-compaction': dict(harnesses=[
        'vec::edit::panic_proofs::retain_all_lengths_with_loop_contracts',
        'vec::edit::panic_proofs::dedup_all_lengths_with_loop_contracts',
    ], solver='z3', verus=['vec-compaction'], scope='Original executable retain/dedup prefix and compaction control, arbitrary admitted word lengths and callback replies in both Verus word widths. Actual Rust all-word ZST owner/guard checks; nonzero contents and provenance are separate.'),
    'vec-compaction-values': dict(harnesses=[
        'vec::edit::compaction_proofs::retain_error_and_mutation_match_standard_for_every_four_element_input',
        'vec::edit::compaction_proofs::dedup_error_and_mutation_match_standard_for_every_four_element_input',
    ], loop_contracts=False, scope='Actual unabstracted Vec and std content/error/callback-order comparison for every prefix length through four, every four u8 payloads and Boolean reply vector, and every error ordinal through four. Nonzero physical memory; separate from the all-word control proof.'),
    'btree-node-spans': dict(harnesses=['collections::btree_map::geometry_proofs::node_spans_cover_every_admitted_word_coordinate', 'collections::btree_map::proofs::set_replacement_every_initialized_slot', 'collections::btree_map::proofs::leaf_insert_remove_every_slot', 'collections::btree_map::proofs::split_every_incoming_edge', 'collections::btree_map::proofs::external_view_arbitrary_shape_never_indexes_unchecked', 'collections::btree_map::proofs::borrow_left_every_occupancy', 'collections::btree_map::proofs::borrow_right_every_occupancy', 'collections::btree_map::proofs::merge_every_fitting_occupancy', 'collections::btree_map::proofs::suffix_every_node_edge'], verus=['btree-node-spans'], scope='Seven original executable node span producers: complete admitted word-sized counts/indices in both Verus word domains, actual native scalar equations and the eight existing actual node/view typed harnesses without input-domain changes. Python does not translate node expressions; caller routing/ownership correspondence and generic physical memory retain read-only guards and Miri/ASan.'),
    'btree-packed-shape': dict(harnesses=[
        'collections::btree_map::bulk::shape_proofs::packed_shape_covers_every_machine_word_length',
    ], verus=['btree-packed-shape'], scope='Actual output_shape body and loop, every word input and both Verus word domains; exact leaves, success and branch/height/nonoverflow/termination properties. Native Kani calls the actual function for every usize length; the prior native/u32 copied bodies are removed.'),
    'btree-delete-decisions': dict(harnesses=[
        'collections::btree_map::geometry_proofs::deletion_decisions_cover_every_admitted_word_input',
    ], verus=['btree-delete-decisions'], scope='Actual deletion eligibility and capacity decisions over all prior word-sized numeric inputs, both Verus word domains, plus native arithmetic. The existing actual all-occupancy node kernels and Miri check typed memory/caller paths separately. A supplemental whole-rebalancer heap probe ran out of memory and is not a claimed proof; the original numeric domain is retained in full.'),
    'btree-frontiers': dict(harnesses=[
        'collections::btree_map::geometry_proofs::cleanup_frontier_preserves_every_word_count_and_remainder',
        'collections::btree_map::geometry_proofs::borrowed_leaf_edge_preserves_every_word_coordinate',
        'collections::btree_map::iter::borrowed_edge_proofs::borrowed_leaf_interval_preserves_each_slot_and_ends',
        'collections::btree_map::iter::drop_frontier_proofs::no_drop_leaf_cleanup_consumes_each_valid_owned_prefix_once',
    ], verus=['btree-frontiers'], scope='Actual all-word cleanup and borrowed leaf geometry in both Verus word domains and native Rust; actual borrowed leaf intervals and owning no-drop cleanup over every admitted physical byte-leaf prefix/endpoint (<=11), including field publication, termination and deallocation.'),
    'btree-plan-counts': dict(harnesses=[
        'collections::btree_map::bulk::count_proofs::observed_deficits_preserve_each_all_word_field_and_retirement_frame',
        'collections::btree_map::bulk::count_proofs::retirement_increments_only_the_selected_all_word_counter',
    ], verus=['btree-plan-counts'], scope='Actual all-word deficit/successor and NodeCounts field transitions in both Verus word domains. Actual Rust state methods cover arbitrary field values and all height choices, exact observation and unaffected-field framing. Closed-tree credits and output-shape models remain separate.'),
    'btree-search-cursors': dict(harnesses=[
        'collections::btree_map::geometry_proofs::search_step_preserves_every_word_interval',
        'collections::btree_map::iter::borrowed_edge_proofs::packed_cursor_methods_preserve_every_admitted_coordinate',
        'collections::btree_map::view::search_proofs::validated_slice_search_matches_std_on_every_small_query',
    ], verus=['btree-search-cursors'], scope='Actual all-word search-step and packed cursor kernels through direct Verus in both word domains; Kani checks every word interval step and owned cursor coordinate, actual validated sorted NodeSlice prefixes up to eight u8 keys for every u8 query.'),
    'shared-numeric': dict(harnesses=['shared::proofs::'+name for name in (
        'count_admission_all_machine_words', 'local_count_admission_never_changes_on_failure',
        'local_release_consumes_one_credit_without_wrap', 'retired_word_preserves_every_valid_allocation_size',
        'retirement_preserves_every_valid_weak_credit',
    )], verus=['shared-numeric','shared-numeric-reduced'], scope='Original admission and retired word encoding/decoding for all admitted word inputs, native/reduced constants and both Verus word domains. Existing actual Rust checks cover all-word admission, local cells, retired payload bytes and sequential atomic retirement/weak credit; concurrency/platform models remain separate.'),
    'vec-layout-growth': dict(harnesses=[
        'vec::growth_proofs::storage_growth_publishes_only_the_admitted_capacity_for_every_numeric_owner',
        'vec::growth_proofs::push_growth_matches_grow_one_for_every_full_numeric_owner',
        'capacity::proofs::constructors_and_reconstructed_layout_preserve_each_byte_bound',
        'wrapping_memory_proofs::every_wrapping_result_is_checked_before_reservation_or_reallocation',
    ], verus=['vec-layout-growth'], scope='Original array ceiling and Vec size-aware growth proposal for all admitted word input states in both Verus word domains. Actual grow_to publication checks all valid growing unit-owner metadata under both sealed policies; actual Capacity constructors retain all-word byte-layout domains, and the existing two-byte reservation witness retains arbitrary incoming-word refusal checks.'),
    'string-cursors': dict(harnesses=[
        'string::cursor_proofs::retain_cursors_and_error_cleanup_preserve_scalar_contents',
        'string::cursor_proofs::drain_cleanup_removes_only_the_validated_scalar_range',
        'string::proofs::edit_geometry_all_machine_words',
        'string::proofs::deletion_requires_no_growth_for_every_valid_range',
    ], verus=['string-cursors'], scope='Executable cursor advance/retained-suffix/drain arithmetic for every admitted word input in both Verus word domains. Actual Rust verifies all-word Edit geometry and deletion admission; typed content checks use four fixed scalar pairs covering UTF-8 widths 1..4, all retain/error replies, scalar-boundary drain ranges and front/back consumption. Existing all-Unicode encoding proofs remain unchanged. Pinned core models remain separately identified.'),
    'vec-cursors': dict(harnesses=[
        'vec::cursor_proofs::within_capacity_push_preserves_every_numeric_owner',
        'vec::cursor_proofs::suffix_guard_restores_every_admitted_original_length',
        'vec::edit::panic_proofs::extraction_drop_restores_every_numeric_cursor_exactly',
        'vec::edit::panic_proofs::extraction_drop_preserves_nonzero_prefix_and_unvisited_values',
    ], verus=['vec-cursors'], scope='Original append/removed-suffix arithmetic for all word inputs in both Verus word domains. Original within-capacity owner publication, suffix guard and exact extraction-drop length use arbitrary native-word numeric states; nonzero extraction moves cover every valid cursor tuple through four elements. No Vec-kernel body projection.'),
    'owner-metadata': dict(harnesses=[
        'vec::metadata_proofs::every_numeric_length_update_and_refusal_preserves_other_metadata',
        'vec::metadata_proofs::original_empty_constructors_and_standard_adoption_cover_every_unit_length_limit',
        'vec::metadata_proofs::boxed_and_vector_unit_owners_transfer_every_word_length_without_moving_storage',
        'vec::query_proofs::every_valid_zst_owner_query_and_spare_partition',
    ], verus=['owner-metadata'], scope='Original logical capacity, standard-adoption admission and reused initialized-length admission for all word-sized numeric inputs in both Verus word domains. Actual empty/standard/boxed owner adapters and query/length updates use all-word unit storage; internal representation fixtures preserve the complete numeric envelope without violating the public raw-parts ZST admission contract. Nonzero provenance remains separate.'),
    'buffer': dict(harnesses=[
        'init::collection::proofs::every_word_leaf_offset_and_source_reply',
        'init::collection::proofs::typed_spans_preserve_the_prefix_and_exact_source_consumption',
        'boxed::buffer::proofs::every_valid_unit_owner_reservation_matches_the_numeric_result',
        'proofs::collected_slice_reallocation_preserves_values',
    ], verus=['buffer'], scope='Original Buffer reservation geometry and stop/error classifier with direct all-word Verus contracts; original shared cursor for arbitrary admitted spans. Actual Rust checks cover every word-sized unit-owner reservation state and leaf offset, bounded nonzero prefix/consumption/cleanup and the real allocating public collection path. Outer growth/owner composition remains separately identified.'),
    'init-prefix': dict(harnesses=[
        'proofs::scalar_array_clone_preserves_values',
        'proofs::heap_array_slot_checks_every_partial_position',
        'proofs::dynamic_slice_initialization_and_budget_cleanup',
        'init::prefix::proofs::every_word_leaf_index_and_normalized_reply',
        'init::prefix::proofs::cleanup_owns_exactly_the_completed_prefix_and_current_slot',
    ], verus=['init-prefix'], scope='Original shared array/slice initialization loop: all-word progress, successful callback history and termination through direct Verus. Actual slot leaf for every word index and normalized callback reply; nonzero-element cleanup over lengths 0..3 through Kani; generic physical ownership retains Miri/ASan and Rust contracts.'),
    'arithmetic': dict(harnesses=['arithmetic::proofs::' + name for name in (
        'addition_reports_every_native_overflow', 'subtraction_reports_every_native_overflow',
        'multiplication_reports_every_native_overflow', 'wrapping_addition_matches_modular_result',
        'wrapping_subtraction_matches_modular_result', 'wrapping_multiplication_matches_modular_result',
        'division_reports_zero_and_every_native_overflow', 'wrapping_division_only_rejects_zero',
        'remainder_reports_zero_and_signed_overflow', 'wrapping_remainder_only_rejects_zero',
        'checked_operators_preserve_all_primitive_results',
    )], verus=['integer'], solver='z3', scope='Original 12 primitive implementations and generic Checked forwarding: complete mathematical add/sub/mul contracts through direct Verus; all-input checked, wrapping, division/remainder and Checked API equivalence through actual-Rust Kani.'),
    'budget': dict(harnesses=['proofs::budget_arithmetic_and_raii'] + ['budget::panic_proofs::' + name for name in (
        'reservation_and_commit_preserve_every_valid_state',
        'release_preserves_every_valid_owned_charge_partition',
        'every_other_owner_transition_preserves_existing_entitlements',
    )], verus=['local-budget'], scope='Original local Cell/RAII transitions over every valid machine-word accounting state, including arbitrary other-owner transitions preserving live entitlements; direct runtime numeric contracts and logical ownership ledger.'),
    'vec-geometry': dict(harnesses=['proofs::vec_required_length_checks_all_overflow_and_limit_cases', 'proofs::vec_growth_proposal_fits_limit_without_rejecting_fitting_length', 'vec::bounds_proofs::every_endpoint_and_getter_order'], verus=['vec-geometry'], scope='Original Vec length, growth and endpoint normalization for every machine-word input, including exact refusal, all Bound forms and returning getter order through direct Verus and actual-Rust Kani. No length/growth/range projection.'),
    'vec-conversion': dict(harnesses=['vec::geometry_proofs::flattened_owner_checks_all_word_lengths_and_preserves_rejection'], verus=['vec-flatten'], scope='Original flattened shape math for every word-sized width and valid layout, plus the actual owner adapter with arbitrary ZST lengths/limits at widths 0, 1, 3 and usize::MAX, including pointer/policy limit preservation and exact rejection.'),
    'vec-queries': dict(harnesses=['vec::query_proofs::every_valid_zst_owner_query_and_spare_partition'], verus=['vec-queries'], scope='Original remaining-limit, spare-capacity and split-spare methods for arbitrary valid machine-word metadata at ZST owners and both sealed policies; unchanged nonzero-element pointer/aliasing scenarios remain separate. Original policy methods and used subtraction have direct both-word-domain Verus contracts.'),
    'layout': dict(harnesses=['layout::proofs::original_layout_matches_core_for_every_count', 'capacity::proofs::constructors_and_reconstructed_layout_preserve_each_byte_bound'], verus=['layout'], scope='Original array numeric admission for every count and size through direct Verus; actual generic Layout adapter and reconstructed capacity layouts for arbitrary counts at byte/word/odd-size/aligned/ZST types. Exact acceptance and Layout refusal, not just success-conditional safety.'),
    'state': dict(harnesses=[
        'compaction::proofs::constructors_establish_every_numeric_bound',
        'compaction::proofs::both_transitions_close_all_invariant_states_and_queries',
        'compaction::proofs::constructors_and_all_transitions_preserve_range_and_cleanup',
        'span::proofs::full_constructor_establishes_order',
        'span::proofs::both_transitions_preserve_order_and_exact_length',
    ], script='verify_state_types.py', script_controls=(11, 5), scope='All 31 original Compaction/Extraction/Span numeric methods through Verus, including both 32/64-bit word domains, plus actual-Rust all-word inductive state harnesses. Pending caller projections call the original methods through maintained adapters.'),
    'owner-extent': dict(harnesses=[
        'extent::proofs::constructors_establish_the_order_for_every_word',
        'extent::proofs::every_transition_preserves_the_order_for_every_state_and_argument',
    ], verus=['vec-extent'], script='verify_owner_extent.py', script_controls=(0, 12), scope='Closed owner state of Vec: len <= cap <= limit and the byte bound of a sized capacity as Verus type invariants of src/extent.rs and src/capacity.rs, established by every constructor and preserved by every total transition for arbitrary arguments in both Verus word domains. The declaration check pins the private fields, the constructor and transition set, the bodies without a contract, construction and assignment sites and the callers of the two trusted entries. Actual-Rust harnesses cover every native-word state and argument at unit storage. Initialization and liveness of the storage are separate.'),
    'slot': dict(harnesses=[
        'proofs::slot_write_read_reject_and_cleanup',
        'proofs::slot_finish_requires_write_and_transfers_ownership',
        'init::proofs::finish_transfers_exactly_one_owned_value',
    ], scope='Actual slot methods: arbitrary u64 values, rejection preserving input, initialization queries, cleanup and exact destructor ownership transfer. Numeric array-prefix and generic provenance contracts are separate.'),
    'clone': dict(harnesses=[
        'proofs::nested_array_reuse_preserves_values_and_mismatch_destination',
        'clone::proofs::nested_array_adapter_checks_arbitrary_word_lengths',
    ], verus=['clone','array-caller'], scope='Actual count admission for every usize length and width and typed-array extent composition in both Verus word domains; actual nested-array adapter for arbitrary word lengths at widths 0, 1, 3 and usize::MAX and nonzero u8 payload equality/mismatch preservation (Kani).'),
}
CONTROLS = [
    ('staging-publication','vec-staging','src/vec/edit.rs',
     '    extra.len = next;', '    extra.len = extra.len;',
     'vec::edit::staging_proofs::staging_push_preserves_every_admitted_word_owner'),
    ('staging-source-classification','vec-staging','src/vec/staging_driver.rs',
     '            Err(error) => return (Err(error), new_len, new_cap),',
     '            Err(_) => return (Ok(false), new_len, new_cap),',
     'vec::edit::staging_proofs::staging_controller_preserves_word_extents_and_source_order'),
    ('staging-slot','vec-staging','src/vec/edit.rs',
     'extra.ptr.as_ptr().add(extra.len.get()).write(value)',
     'extra.ptr.as_ptr().write(value)',
     'vec::edit::staging_proofs::staging_push_preserves_every_three_byte_prefix'),
    ('vec-splice-fill-end', 'vec-splice-fill', 'src/vec/edit.rs',
     '                self.drain.tail_start,\n                &mut self.replacement,',
     '                self.drain.owner.cap.get(),\n                &mut self.replacement,',
     'vec::edit::panic_proofs::splice_fill_preserves_every_admitted_word_hole_and_tail'),
    ('vec-splice-fill-error', 'vec-splice-fill', 'src/vec/edit.rs',
     '        .map_err(InitError::Initialize)\n    }\n    /// Complete replacement',
     '        .map_err(|_| InitError::Storage(Error::Bounds))\n    }\n    /// Complete replacement',
     'vec::edit::panic_proofs::splice_fill_preserves_every_admitted_word_hole_and_tail'),
    ('vec-extraction-retirement', 'vec-extraction-values', 'src/vec/compaction_driver.rs',
     '            let _ = state.remove_next();\n            return Ok(Some(read));',
     '            let _ = state.keep_next();\n            return Ok(Some(read));',
     'vec::edit::extraction_proofs::extraction_errors_mutation_and_order_match_standard_for_every_three_element_input'),
    ('collection-prefix-publication', 'vec-splice-fill', 'src/init/collection.rs',
     '        *self.output = *self.current;', '        *self.output = *self.output;',
     'init::collection::proofs::local_cursor_publication_preserves_every_word'),
    ('vec-extraction-source', 'vec-extraction-values', 'src/vec/edit.rs',
     'filter(unsafe { &mut *pointer.as_ptr().add(read) })',
     'filter(unsafe { &mut *pointer.as_ptr() })',
     'vec::edit::extraction_proofs::extraction_errors_mutation_and_order_match_standard_for_every_three_element_input'),
    ('vec-extraction-error-stop', 'vec-extraction-values', 'src/vec/edit.rs',
     '        self.state.stop();\n        Some(Err(error))',
     '        Some(Err(error))',
     'vec::edit::extraction_proofs::extraction_errors_mutation_and_order_match_standard_for_every_three_element_input'),

    ('vec-retain-source', 'vec-compaction-values', 'src/vec/edit.rs',
     'keep(unsafe { &mut *pointer.as_ptr().add(read) })',
     'keep(unsafe { &mut *pointer.as_ptr().add(_write) })',
     'vec::edit::compaction_proofs::retain_error_and_mutation_match_standard_for_every_four_element_input'),
    ('vec-retain-retirement', 'vec-compaction-values', 'src/vec/compaction_driver.rs',
     'let Some(read) = state.remove_next() else {', 'let Some(read) = state.peek() else {',
     'vec::edit::compaction_proofs::retain_error_and_mutation_match_standard_for_every_four_element_input'),
    ('vec-dedup-decision', 'vec-compaction-values', 'src/vec/edit.rs',
     '.map(|duplicate| !duplicate)', '.map(|duplicate| duplicate)',
     'vec::edit::compaction_proofs::dedup_error_and_mutation_match_standard_for_every_four_element_input'),
    ('btree-node-length-routing', 'btree-node-spans', 'src/collections/btree_map/node.rs', 'set_len(node, super::geometry::insert_span(n, i).2);', 'set_len(node, super::geometry::insert_span(n, i).0);', 'collections::btree_map::proofs::leaf_insert_remove_every_slot'),

    ('btree-node-native-tail', 'btree-node-spans', 'src/collections/btree_map/geometry.rs', '(after, sub(old, index), add(old, 1), add(after, 1))', '(after, add(sub(old, index), 1), add(old, 1), add(after, 1))', 'collections::btree_map::geometry_proofs::node_spans_cover_every_admitted_word_coordinate'),
    ('btree-node-insert-routing', 'btree-node-spans', 'src/collections/btree_map/node.rs', 'pairs(node, i, node, after, tail);', 'pairs(node, i, node, i, tail);', 'collections::btree_map::proofs::leaf_insert_remove_every_slot'),
    ('btree-node-split-routing', 'btree-node-spans', 'src/collections/btree_map/node.rs', 'super::geometry::split_span(len(node), mid)', 'super::geometry::split_span(len(node), add(mid, 1))', 'collections::btree_map::proofs::split_every_incoming_edge'),

    ('btree-shape-native-height', 'btree-packed-shape', 'src/collections/btree_map/bulk.rs',
     'height = height.checked_add(1).ok_or(Error::Capacity)?;', 'height = height.checked_add(32).ok_or(Error::Capacity)?;',
     'collections::btree_map::bulk::shape_proofs::packed_shape_covers_every_machine_word_length'),
    ('btree-shape-native-ceiling', 'btree-packed-shape', 'src/collections/btree_map/bulk.rs',
     'if level % 12 != 0 { 1 } else { 0 }', '0',
     'collections::btree_map::bulk::shape_proofs::packed_shape_covers_every_machine_word_length'),
    ('btree-delete-native-separator', 'btree-delete-decisions', 'src/collections/btree_map/geometry.rs',
     'add(add(sibling, child), 1) > super::node::CAP', 'add(sibling, child) > super::node::CAP',
     'collections::btree_map::geometry_proofs::deletion_decisions_cover_every_admitted_word_input'),
    ('btree-delete-native-equality', 'btree-delete-decisions', 'src/collections/btree_map/geometry.rs',
     'add(add(sibling, child), 1) > super::node::CAP', 'add(add(sibling, child), 1) >= super::node::CAP',
     'collections::btree_map::geometry_proofs::deletion_decisions_cover_every_admitted_word_input'),
    ('btree-cleanup-publication', 'btree-frontiers', 'src/collections/btree_map/iter.rs',
     'self.state.remaining = Checked::new(sub(self.state.remaining.get(), skip));', 'self.state.remaining = Checked::new(0);',
     'collections::btree_map::iter::drop_frontier_proofs::no_drop_leaf_cleanup_consumes_each_valid_owned_prefix_once'),
    ('btree-borrowed-direction', 'btree-frontiers', 'src/collections/btree_map/iter.rs',
     'super::geometry::leaf_edge(c.index(), node::len(c.node), back)', 'super::geometry::leaf_edge(c.index(), node::len(c.node), false)',
     'collections::btree_map::iter::borrowed_edge_proofs::borrowed_leaf_interval_preserves_each_slot_and_ends'),
    ('btree-observe-input-field', 'btree-plan-counts', 'src/collections/btree_map/bulk.rs',
     'allocation_deficit(self.leaves, leaves, retired.leaves)', 'allocation_deficit(self.leaves, branches, retired.leaves)',
     'collections::btree_map::bulk::count_proofs::observed_deficits_preserve_each_all_word_field_and_retirement_frame'),
    ('btree-retire-height-choice', 'btree-plan-counts', 'src/collections/btree_map/bulk.rs',
     'let count = if height == 0 {', 'let count = if height != 0 {',
     'collections::btree_map::bulk::count_proofs::retirement_increments_only_the_selected_all_word_counter'),
    ('btree-cursor-update-argument', 'btree-search-cursors', 'src/collections/btree_map/iter.rs',
     'super::geometry::replace_index(self.position, index)', 'super::geometry::replace_index(self.position, 0)',
     'collections::btree_map::iter::borrowed_edge_proofs::packed_cursor_methods_preserve_every_admitted_coordinate'),
    ('btree-search-count-argument', 'btree-search-cursors', 'src/collections/btree_map/view.rs',
     'let mut high = keys.len();', 'let mut high = 0;',
     'collections::btree_map::view::search_proofs::validated_slice_search_matches_std_on_every_small_query'),
    ('shared-local-admission-argument', 'shared-numeric', 'src/shared/count.rs',
     'self.0.set(admitted_count(old)?);', 'self.0.set(admitted_count(0)?);',
     'shared::proofs::local_count_admission_never_changes_on_failure'),
    ('shared-retired-bytes-argument', 'shared-numeric', 'src/shared/count.rs',
     'self.0.store(retired_state(bytes), Ordering::Release);',
     'self.0.store(retired_state(0), Ordering::Release);',
     'shared::proofs::retirement_preserves_every_valid_weak_credit'),
    # Changing the push's additional count to zero is an equivalent mutant: the
    # doubling proposal or the later capacity refusal gives the same result.
    ('push-growth-current-capacity-argument', 'vec-layout-growth', 'src/vec.rs',
     'growth_for_layout(cap.get(), needed, max_len', 'growth_for_layout(0, needed, max_len',
     'vec::growth_proofs::push_growth_matches_grow_one_for_every_full_numeric_owner'),
    ('push-growth-exact-argument', 'vec-layout-growth', 'src/vec.rs',
     'mem::size_of::<T>(), false)?;', 'mem::size_of::<T>(), true)?;',
     'vec::growth_proofs::push_growth_matches_grow_one_for_every_full_numeric_owner'),
    ('layout-growth-size-argument', 'vec-layout-growth', 'src/vec.rs',
     '            mem::size_of::<T>(),\n            exact,', '            1,\n            exact,',
     'vec::growth_proofs::storage_growth_publishes_only_the_admitted_capacity_for_every_numeric_owner'),
    ('layout-growth-needed-argument', 'vec-layout-growth', 'src/vec.rs',
     '            self.cap.get(),\n            needed,\n            self.max_len(),',
     '            self.cap.get(),\n            0,\n            self.max_len(),',
     'vec::growth_proofs::storage_growth_publishes_only_the_admitted_capacity_for_every_numeric_owner'),
    ('layout-growth-exact-argument', 'vec-layout-growth', 'src/vec.rs',
     '            mem::size_of::<T>(),\n            exact,', '            mem::size_of::<T>(),\n            false,',
     'vec::growth_proofs::storage_growth_publishes_only_the_admitted_capacity_for_every_numeric_owner'),
    ('string-retain-written-argument', 'string-cursors', 'src/string/edit.rs',
     'super::numeric::advance(guard.written.get(), width)', 'super::numeric::advance(guard.read.get(), width)',
     'string::cursor_proofs::retain_cursors_and_error_cleanup_preserve_scalar_contents'),
    ('string-retain-restore-argument', 'string-cursors', 'src/string/edit.rs',
     'super::numeric::restore(self.read.get(), self.written.get(), self.original.get())',
     'super::numeric::restore(self.read.get(), self.read.get(), self.original.get())',
     'string::cursor_proofs::retain_cursors_and_error_cleanup_preserve_scalar_contents'),
    ('string-drain-start-argument', 'string-cursors', 'src/string/edit.rs',
     'super::numeric::drain(self.start, self.end, owner.len())',
     'super::numeric::drain(self.end, self.end, owner.len())',
     'string::cursor_proofs::drain_cleanup_removes_only_the_validated_scalar_range'),
    ('push-capacity-argument', 'vec-cursors', 'src/vec/convenience.rs',
     'geometry::append_cursor(self.len, self.cap.get())', 'geometry::append_cursor(self.len, self.max_len())',
     'vec::cursor_proofs::within_capacity_push_preserves_every_numeric_owner'),
    ('suffix-original-length', 'vec-cursors', 'src/vec.rs',
     'self.owner.truncate(self.original);', 'self.owner.truncate(0);',
     'vec::cursor_proofs::suffix_guard_restores_every_admitted_original_length'),
    ('extraction-copy-destination', 'vec-cursors', 'src/vec/edit.rs',
     'self.owner.ptr.as_ptr().add(self.state.written()),\n                    self.state.unvisited_len(),',
     'self.owner.ptr.as_ptr().add(self.state.read()),\n                    self.state.unvisited_len(),',
     'vec::edit::panic_proofs::extraction_drop_preserves_nonzero_prefix_and_unvisited_values'),
    ('extraction-restored-length', 'vec-cursors', 'src/compaction.rs',
     'wrapping_sub(self.total.get(), self.deleted.get())',
     'self.total.get()',
     'vec::edit::panic_proofs::extraction_drop_restores_every_numeric_cursor_exactly'),
    ('owner-empty-capacity-argument', 'owner-metadata', 'src/capacity.rs',
     'logical_capacity(mem::size_of::<T>(), 0, limit)', 'logical_capacity(mem::size_of::<T>(), 0, 0)',
     'vec::metadata_proofs::original_empty_constructors_and_standard_adoption_cover_every_unit_length_limit'),
    ('owner-slice-capacity-argument', 'owner-metadata', 'src/capacity.rs',
     'logical_capacity(mem::size_of::<T>(), values.len(), limit)', 'logical_capacity(mem::size_of::<T>(), values.len(), 0)',
     'vec::metadata_proofs::boxed_and_vector_unit_owners_transfer_every_word_length_without_moving_storage'),
    ('owner-adoption-length-argument', 'owner-metadata', 'src/vec/ownership.rs',
     '            source.len(),\n            source.capacity(),', '            0,\n            source.capacity(),',
     'vec::metadata_proofs::original_empty_constructors_and_standard_adoption_cover_every_unit_length_limit'),
    ('owner-length-capacity-argument', 'owner-metadata', 'src/vec/convenience.rs',
     'required_len(0, new_len, self.cap.get())?', 'required_len(0, new_len, self.max_len())?',
     'vec::metadata_proofs::every_numeric_length_update_and_refusal_preserves_other_metadata'),
    ('collection-missing-write', 'buffer', 'src/init/collection.rs',
     'unsafe { ptr.as_ptr().add(index).write(value) };', 'drop(value);',
     'init::collection::proofs::typed_spans_preserve_the_prefix_and_exact_source_consumption'),
    ('collection-wrong-offset', 'buffer', 'src/init/collection.rs',
     'ptr.as_ptr().add(index).write(value)', 'ptr.as_ptr().add(index.wrapping_add(1)).write(value)',
     'init::collection::proofs::typed_spans_preserve_the_prefix_and_exact_source_consumption'),
    ('collection-none-status', 'buffer', 'src/init/collection.rs',
     'Err(Stop::End) => Ok(false),', 'Err(Stop::End) => Ok(true),',
     'init::collection::proofs::every_word_leaf_offset_and_source_reply'),
    ('collection-error-status', 'buffer', 'src/init/collection.rs',
     'Err(Stop::Source(error)) => Err(error),', 'Err(Stop::Source(_error)) => Ok(false),',
     'init::collection::proofs::every_word_leaf_offset_and_source_reply'),
    ('buffer-reservation-argument', 'buffer', 'src/boxed/buffer.rs',
     'super::buffer_numeric::reservation(self.len.get(), self.cap.get(), additional, hint)',
     'super::buffer_numeric::reservation(self.len.get(), self.cap.get(), 0, hint)',
     'boxed::buffer::proofs::every_valid_unit_owner_reservation_matches_the_numeric_result'),
    ('collection-initial-bound', 'buffer', 'src/boxed/buffer.rs',
     'fill(self.ptr, &mut self.len, reserved_end, &mut iter)',
     'fill(self.ptr, &mut self.len, reserved_end.wrapping_add(1), &mut iter)',
     'proofs::collected_slice_reallocation_preserves_values'),
    ('collection-growth-bound', 'buffer', 'src/boxed/buffer.rs',
     'fill(self.ptr, &mut self.len, self.cap.get(), &mut iter)',
     'fill(self.ptr, &mut self.len, self.cap.get().wrapping_add(1), &mut iter)',
     'proofs::collected_slice_reallocation_preserves_values'),
    ('init-array-caller-extent', 'init-prefix', 'src/init.rs',
     'prefix::initialize(self.ptr.cast::<T>(), N, &mut initialize)',
     'prefix::initialize(self.ptr.cast::<T>(), 0, &mut initialize)',
     'proofs::heap_array_slot_checks_every_partial_position'),
    ('init-slice-caller-extent', 'init-prefix', 'src/boxed/slice.rs',
     'crate::init::prefix::initialize(ptr, len, &mut initialize)',
     'crate::init::prefix::initialize(ptr, 0, &mut initialize)',
     'proofs::dynamic_slice_initialization_and_budget_cleanup'),
    ('init-clone-caller-extent', 'init-prefix', 'src/clone.rs',
     'crate::init::prefix::initialize(ptr, N, &mut initialize)',
     'crate::init::prefix::initialize(ptr, 0, &mut initialize)',
     'proofs::scalar_array_clone_preserves_values'),
    ('init-leaf-callback-index', 'init-prefix', 'src/init/prefix.rs',
     'initialize(index, &mut slot)', 'initialize(index.wrapping_add(1), &mut slot)',
     'init::prefix::proofs::every_word_leaf_index_and_normalized_reply'),
    ('init-leaf-finish', 'init-prefix', 'src/init/prefix.rs',
     'slot.finish().map_err(InitError::Storage)', 'core::mem::forget(slot); Ok(())',
     'init::prefix::proofs::every_word_leaf_index_and_normalized_reply'),
    ('init-prefix-disarm', 'init-prefix', 'src/init/prefix.rs',
     'prefix.state = Checked::new(0);', 'prefix.state = Checked::new(length);',
     'init::prefix::proofs::cleanup_owns_exactly_the_completed_prefix_and_current_slot'),
    ('init-prefix-cleanup', 'init-prefix', 'src/init/prefix.rs',
     '                self.state.get(),', '                0,',
     'init::prefix::proofs::cleanup_owns_exactly_the_completed_prefix_and_current_slot'),
    ('query-remaining-input', 'vec-queries', 'src/vec.rs',
     'wrapping_sub(self.max_len(), self.len.get())',
     'wrapping_sub(self.len.get(), self.len.get())',
     'vec::query_proofs::every_valid_zst_owner_query_and_spare_partition'),
    ('query-spare-count', 'vec-queries', 'src/vec/convenience.rs',
     '\n                crate::arithmetic::bounded::wrapping_sub(self.cap.get(), self.len.get()),\n',
     '\n                crate::arithmetic::bounded::wrapping_sub(self.cap.get(), 0),\n',
     'vec::query_proofs::every_valid_zst_owner_query_and_spare_partition'),
    ('capacity-runtime-callee', 'layout', 'src/capacity.rs',
     'wrapping_mul(self.count.get(), mem::size_of::<T>())',
     'crate::arithmetic::bounded::wrapping_add(self.count.get(), mem::size_of::<T>())',
     'capacity::proofs::constructors_and_reconstructed_layout_preserve_each_byte_bound'),
    ('layout-hidden-panic', 'layout', 'src/layout.rs',
     '    let bytes = numeric::array_bytes(count, size)?;',
     '    if count == 17 { panic!("layout source control"); }\n    let bytes = numeric::array_bytes(count, size)?;',
     'layout::proofs::original_layout_matches_core_for_every_count'),
    ('layout-spurious-refusal', 'layout', 'src/layout.rs',
     '    let bytes = numeric::array_bytes(count, size)?;',
     '    if count == 17 { return Err(Error::Layout); }\n    let bytes = numeric::array_bytes(count, size)?;',
     'layout::proofs::original_layout_matches_core_for_every_count'),
    ('flatten-adapter-width', 'vec-conversion', 'src/vec/convenience.rs',
     '            mem::size_of::<T>(),\n            N,',
     '            mem::size_of::<T>(),\n            0,',
     'vec::geometry_proofs::flattened_owner_checks_all_word_lengths_and_preserves_rejection'),
    ('vec-range-getter-order', 'vec-geometry', 'src/vec/edit.rs',
     'let start = super::bounds::lower(range.start_bound())?;\n    let end = super::bounds::upper(range.end_bound(), len)?;',
     'let end_bound = range.end_bound();\n    let start = super::bounds::lower(range.start_bound())?;\n    let end = super::bounds::upper(end_bound, len)?;',
     'vec::bounds_proofs::every_endpoint_and_getter_order'),
    ('vec-length-hidden-panic', 'vec-geometry', 'src/vec.rs',
     'pub(crate) fn required_len(len: usize, additional: usize, limit: usize) -> Result<usize, Error> {',
     'pub(crate) fn required_len(len: usize, additional: usize, limit: usize) -> Result<usize, Error> {\n    if len == 17 && additional == 19 { panic!("length source control"); }',
     'proofs::vec_required_length_checks_all_overflow_and_limit_cases'),
    ('budget-reserve-hidden-panic', 'budget', 'src/budget.rs',
     '        Ok(Reservation {\n',
     '        if bytes == 1 { panic!("unmodeled admission side effect"); }\n        Ok(Reservation {\n',
     'budget::panic_proofs::reservation_and_commit_preserve_every_valid_state'),
    ('arithmetic-wrapped-add', 'arithmetic', 'src/arithmetic.rs',
     'self.wrapping_add(rhs)', 'self.wrapping_sub(rhs)',
     'arithmetic::proofs::wrapping_addition_matches_modular_result'),
    ('arithmetic-wrapped-sub', 'arithmetic', 'src/arithmetic.rs',
     'self.wrapping_sub(rhs)', 'self.wrapping_add(rhs)',
     'arithmetic::proofs::wrapping_subtraction_matches_modular_result'),
    ('arithmetic-wrapped-mul', 'arithmetic', 'src/arithmetic.rs',
     'self.wrapping_mul(rhs)', 'self.wrapping_add(rhs)',
     'arithmetic::proofs::wrapping_multiplication_matches_modular_result'),
    ('arithmetic-div-zero', 'arithmetic', 'src/arithmetic.rs',
     'fn try_div(self, rhs: Self) -> Result<Self, $err> {\n                if rhs == 0 {',
     'fn try_div(self, rhs: Self) -> Result<Self, $err> {\n                if rhs == 1 {',
     'arithmetic::proofs::division_reports_zero_and_every_native_overflow'),
    ('arithmetic-rem-zero', 'arithmetic', 'src/arithmetic.rs',
     'fn try_rem(self, rhs: Self) -> Result<Self, $err> {\n                if rhs == 0 {',
     'fn try_rem(self, rhs: Self) -> Result<Self, $err> {\n                if rhs == 1 {',
     'arithmetic::proofs::remainder_reports_zero_and_signed_overflow'),
    ('arithmetic-checked-forward', 'arithmetic', 'src/arithmetic/checked.rs',
     'operator!(Add, add, try_add, ArithmeticError, all(), +);',
     'operator!(Add, add, try_sub, ArithmeticError, all(), +);',
     'arithmetic::proofs::checked_operators_preserve_all_primitive_results'),
    ('slot-write-flag', 'slot', 'src/init.rs',
     'unsafe { self.ptr.as_ptr().write(value) };\n        self.initialized = true;',
     'unsafe { self.ptr.as_ptr().write(value) };\n        self.initialized = false;',
     'proofs::slot_write_read_reject_and_cleanup'),
    ('slot-finish-ownership', 'slot', 'src/init.rs',
     '        self.initialized = false;\n        Ok(())',
     '        self.initialized = true;\n        Ok(())',
     'init::proofs::finish_transfers_exactly_one_owned_value'),
    ('clone-width-argument', 'clone', 'src/clone.rs',
     'geometry::flatten_count(source.len(), destination.len(), N)',
     'geometry::flatten_count(source.len(), destination.len(), 0)',
     'proofs::nested_array_reuse_preserves_values_and_mismatch_destination'),
]
# Only negative controls use these small concrete counterexamples. Positive
# arithmetic proofs still cover every bit pattern of all twelve integer types.
WITNESSES = {
    'vec-extraction-source': '{ let Ok(mut v)=crate::vec::Vec::<u8>::try_with_capacity(3) else{return;};\nassert!(v.try_extend_copy(&[1,2,3]).is_ok());\nlet mut it=v.try_extract_if(..,|value|{*value=value.wrapping_add(17);Ok::<_,()>(*value&1==0)}).unwrap();\nassert_eq!(it.next(),Some(Ok(18)));assert_eq!(it.next(),Some(Ok(20)));assert!(it.next().is_none());drop(it);\nv.as_slice()==&[19] }',
    'vec-extraction-retirement': '{ let Ok(mut v)=crate::vec::Vec::<u8>::try_with_capacity(3) else{return;};\nassert!(v.try_extend_copy(&[1,2,3]).is_ok());\nlet mut it=v.try_extract_if(..,|value|{*value=value.wrapping_add(17);Ok::<_,()>(*value&1==0)}).unwrap();\nassert_eq!(it.next(),Some(Ok(18)));assert_eq!(it.next(),Some(Ok(20)));assert!(it.next().is_none());drop(it);\nv.as_slice()==&[19] }',
    'vec-extraction-error-stop': '{ let Ok(mut v)=crate::vec::Vec::<u8>::try_with_capacity(3) else{return;};\nassert!(v.try_extend_copy(&[1,2,3]).is_ok());\nlet mut it=v.try_extract_if(..,|value| if *value==1 {Err(())}else{Ok(true)}).unwrap();\nassert_eq!(it.next(),Some(Err(())));it.next().is_none() }',
    'vec-retain-source': '{ let Ok(mut v) = crate::vec::Vec::<u8>::try_with_capacity(3) else { return; };\nassert!(v.try_extend_copy(&[1,2,3]).is_ok());\nassert!(v.try_retain_mut(|value| { *value = value.wrapping_add(17); Ok::<_,()>(*value & 1 == 0) }).is_ok());\nv.as_slice() == &[18,20] }',
    'vec-retain-retirement': '{ let Ok(mut v) = crate::vec::Vec::<u8>::try_with_capacity(3) else { return; };\nassert!(v.try_extend_copy(&[1,2,3]).is_ok());\nassert!(v.try_retain_mut(|value| { *value = value.wrapping_add(17); Ok::<_,()>(*value & 1 == 0) }).is_ok());\nv.as_slice() == &[18,20] }',
    'vec-dedup-decision': '{ let Ok(mut v) = crate::vec::Vec::<u8>::try_with_capacity(3) else { return; };\nassert!(v.try_extend_copy(&[1,1,2]).is_ok());\nassert!(v.try_dedup_by(|a,b| Ok::<_,()>(*a == *b)).is_ok());\nv.as_slice() == &[1,2] }',
    'arithmetic-wrapped-add': '7u8.wrap_add(9) == 16',
    'arithmetic-wrapped-sub': '9u8.wrap_sub(7) == 2',
    'arithmetic-wrapped-mul': '6u8.wrap_mul(7) == 42',
    'arithmetic-div-zero': '0u8.try_div(0) == Err(crate::DivisionError::DivisionByZero)',
    'arithmetic-rem-zero': '0u8.try_rem(0) == Err(crate::DivisionError::DivisionByZero)',
    'arithmetic-checked-forward': '(crate::Checked::new(2u8) + crate::Checked::new(3u8)).map(crate::Checked::get) == Ok(5u8)',
}


def kani(root, out, harnesses, *, failure=False, target=None, solver='cadical', loop_contracts=True, object_bits=10, cfgs=()):
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.pop('RUSTFLAGS', None)
    flags = ['--cfg jzap_loop_contracts'] if loop_contracts else []
    if any(not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', name) for name in cfgs):
        raise ValueError('Invalid verification cfg')
    flags += ['--cfg '+name for name in cfgs]
    if flags:
        env['RUSTFLAGS'] = ' '.join(flags)
    if solver == 'z3':
        # Scalar division/remainder use the same bundled SMT backend as the
        # original arithmetic campaign. Kani locates z3 through PATH.
        env['PATH'] = str(Path(shutil.which(executable())).resolve().parent) + os.pathsep + env['PATH']
    env.pop('CARGO_ENCODED_RUSTFLAGS', None)
    if target:
        env['CARGO_TARGET_DIR'] = str(target)
    command = ['cargo', 'kani', '--lib', '--exact', '--solver', solver]
    if loop_contracts:
        command += ['-Z', 'loop-contracts']
    for name in harnesses:
        command += ['--harness', name]
    # Match the established whole-memory campaign's CBMC backend settings.
    # Object bits encode object identity, not the usize input or loop range.
    command += ['-Z', 'unstable-options', '--cbmc-args', '--object-bits', str(object_bits)]
    with (out / 'kani.log').open('w') as log:
        result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=log)
    log = (out / 'kani.log').read_text()
    total = len(harnesses)
    expected = f'Complete - {0 if failure else total} successfully verified harnesses, {total if failure else 0} failures, {total} total.'
    if expected not in log or (result.returncode != 0) != failure:
        raise RuntimeError('Unexpected actual-Rust result; see ' + str(out / 'kani.log'))
    if failure and not re.search(r'- Status: FAILURE', log):
        raise RuntimeError('Negative control did not reach a solver failure')
    return dict(command=command, exit_code=result.returncode, harnesses=harnesses,
                solver_counterexample=failure, log=str(out / 'kani.log'))


def verify(groups, out, mutations=False):
    if not __debug__:
        raise RuntimeError('Verification requires Python assertions; remove -O/PYTHONOPTIMIZE')
    if not groups or len(groups) != len(set(groups)) or any(name not in GROUPS for name in groups):
        raise ValueError('Choose distinct registered direct-source groups')
    out = out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    lock = (out / '.lock').open('a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    (out / 'summary.json').unlink(missing_ok=True)
    paths = sorted((ROOT / 'src').rglob('*.rs'))
    paths += sorted((ROOT / 'tools/direct-verus').glob('*.rs'))
    # The B-tree adapters are composed behind the storage kernel.
    paths += [ROOT / 'tools/memory-proof' / name for name in ('core-contracts.verus.rs', 'pointer-checks.verus.rs', 'extent.verus.rs', 'storage.verus.rs')]
    paths += [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock']
    paths += sorted((ROOT / 'scripts').glob('*.py'))
    paths += [ROOT / 'tools/panic-audit' / p for p in
              ('Cargo.toml', 'Cargo.lock', 'src/bin/state_sites.rs')]
    fingerprint = lambda: hashlib.sha256(b''.join(
        str(p.relative_to(ROOT)).encode() + b'\0' + p.read_bytes() for p in paths)).hexdigest()
    before = fingerprint()
    results, controls = [], []
    for name in groups:
        group = GROUPS[name]
        row = dict(group=name, scope=group['scope'], kind='direct-product-source')
        if group.get('script'):
            output = out / name / 'verus'
            command = ['python3', 'scripts/' + group['script'], '--output', str(output)]
            subprocess.run(command, cwd=ROOT, check=True)
            row['direct_state'] = json.loads((output / 'summary.json').read_text())
            if not (row['direct_state']['status'] == 'pass' and row['direct_state']['source_unchanged']):
                raise RuntimeError('Missing original state proof result')
            solver_controls, scope_controls = group['script_controls']
            if not (row['direct_state']['controls_run'] and len(row['direct_state']['controls']) == solver_controls
                    and all(c['detected'] for c in row['direct_state']['controls'])):
                raise RuntimeError('Missing actual state or word-domain negative controls')
            if len(row['direct_state']['domain_controls']) != scope_controls or not all(
                    c['detected'] and c['compiled'] for c in row['direct_state']['domain_controls']):
                raise RuntimeError('Missing compiled state scope controls')
        if group.get('verus'):
            row['verus'] = verus(out=out / name / 'verus', targets=group['verus'], mutations=mutations)
        row['kani'] = kani(ROOT, out / name, group['harnesses'], solver=group.get('solver', 'cadical'), loop_contracts=group.get('loop_contracts', True), object_bits=group.get('object_bits', 10), cfgs=group.get('cfgs', ()))
        results.append(row)
        print(name + ': PASS (original Rust)', flush=True)
    if mutations:
        for name, group, file, old, new, harness in CONTROLS:
            if group not in groups:
                continue
            with tempfile.TemporaryDirectory(prefix='jzap-original-source-') as temporary:
                work = Path(temporary)
                for folder in ('src', 'examples'):
                    shutil.copytree(ROOT / folder, work / folder)
                for file_name in ('Cargo.toml', 'Cargo.lock'):
                    shutil.copy(ROOT / file_name, work / file_name)
                path = work / file
                text = path.read_text()
                if text.count(old) != 1:
                    raise RuntimeError('Mutation anchor requires review: ' + name)
                path.write_text(text.replace(old, new))
                if name in WITNESSES:
                    driver = work / 'src/lib.rs'
                    with driver.open('a') as source:
                        source.write('\n#[cfg(kani)] #[kani::proof] #[kani::unwind(14)]\nfn source_mutation_control() {\n'
                                     'use crate::{TryArithmetic, WrappingArithmetic};\n'
                                     'assert!(' + WITNESSES[name] + ');\n}\n')
                    harness = 'source_mutation_control'
                result = kani(work, out / name, [harness], failure=True,
                              target=ROOT / 'target/verification/direct-source-mutants',
                              solver=GROUPS[group].get('solver', 'cadical'),
                              loop_contracts=GROUPS[group].get('loop_contracts', True), object_bits=GROUPS[group].get('object_bits', 10), cfgs=GROUPS[group].get('cfgs', ()))
                controls.append(dict(name=name, group=group, file=file, detected=True,
                                     method='actual Rust solver counterexample',
                                     witness=WITNESSES.get(name), result=result))
                print(name + ': detected', flush=True)
    if fingerprint() != before:
        raise RuntimeError('Direct verification inputs changed during execution')
    report = dict(status='pass', kind='direct-product-source', source_sha256=before,
                  source_unchanged=True, groups=results, controls=controls, mutations_run=mutations)
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--group', choices=GROUPS, action='append')
    parser.add_argument('--mutations', action='store_true')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/direct-source')
    args = parser.parse_args()
    verify(list(dict.fromkeys(args.group or GROUPS)), args.output, args.mutations)
    print(args.output / 'summary.json')


if __name__ == '__main__':
    main()
