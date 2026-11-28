#!/usr/bin/env python3
"""Decide whether an owner invariant is closed at the zero-sized instantiation.

Issue #16, unit 1 step 9 (docs/premise-proofs.md). The basis
`instantiated-kani` needs an inductive closure: every constructor of the owner
establishes its invariant, every write of an owner word and every lend of one
preserves it, and the set of these sites is pinned. This script reads the
sites from the scanner inventory that `caller_bindings.py` uses (struct
literals, assignments and `state-lend` rows) and the calls of the owner's
public unsafe constructor from the unsafe-operation scanner, and maps each
site through the maintained table `SITES` to the closure harnesses that run
it or to a non-harness disposition with its reason.

It fails (AssertionError) when a site has no entry, when an entry names no
site, when a named harness is not in a `verify_source.py` group that the
certificate runs, or when a pinned fixture helper changed. It reports an
owner as not closed when any site is `uncovered` or `other-unit:<n>`, or is
mapped only to harnesses that are not admissible. Admissibility is read from
the proof source, not declared: the harness and every proof function it calls
(transitively, except the pinned fixture helpers) contain no `kani::assume`
and no `kani::any_where`, build no owner by a struct literal or by
`from_raw_parts`, and a writer harness obtains its owner from a pinned helper.
A harness whose body assumes more is admissible only when it is listed in
`EXTRA_ASSUMPTION`, and then it is evidence only for the site that its entry
names under `path_assumption` with a checked reason.

Owners are decided jointly (issue #16, unit 4): a harness that takes its
state from the helper of another owner (the VecDeque `into_vec` harness
assumes the VecDeque invariant to show the Vec invariant, its `from_vec`
harness the converse) counts only while that owner is closed; the closed set
is the greatest fixpoint. A store that cannot execute at T = () is mapped to
the harness of its function with a checked guard (`check_dead`).

Not decided here: an early `return` on a nondeterministic value restricts the
inputs like an assumption and is not detected by this text check; the
scanner's own channel limits (docs/premise-proofs.md, step 3) apply.
"""
import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# The fixed trust boundary of docs/premise-proofs.md. A `trust-boundary` entry
# names one of these items; no other reason is accepted.
TRUST_BOUNDARY = {
    'public-unsafe-caller': 'The caller of a public `unsafe fn` meets its documented contract',
    'std-adoption': 'Values adopted from `std` satisfy the `std` invariants',
}

OWNERS = {
    'Vec': dict(struct=('src/vec.rs', 'Vec'), words=('len', 'cap', 'limit'),
                invariant=('src/vec/query_proofs.rs',
                           'owner.len.get()<=owner.cap.get()&&owner.cap.get()<=owner.limit.max_len()'
                           '&&(mem::size_of::<T>()!=0&&owner.cap.get()!=0||owner.as_ptr().addr()==mem::align_of::<T>())'),
                constructor_calls=('Vec::from_raw_parts', 'crate::vec::Vec::from_raw_parts', 'super::Vec::from_raw_parts')),
    'Buffer': dict(struct=('src/boxed/buffer.rs', 'Buffer'), words=('len', 'cap', 'layout'),
                   invariant=('src/boxed/buffer_proofs.rs',
                              'owner.len.get()<=owner.cap.get()&&crate::layout::array::<T>(owner.cap.get())==Ok(owner.layout)'),
                   constructor_calls=()),
    # Issue #16, unit 4. `ptr` is not an invariant word; `limit` is never assigned.
    'VecDeque': dict(struct=('src/deque.rs', 'VecDeque'), words=('head', 'len', 'cap', 'limit'),
                     invariant=('src/deque/closure_proofs.rs',
                                'owner.len.get()<=owner.cap.get()&&owner.cap.get()<=owner.limit.max_len()'
                                '&&crate::layout::array::<T>(owner.cap.get()).is_ok()'
                                '&&ifmem::size_of::<T>()==0||owner.cap.get()==0{owner.head.get()==0}'
                                'else{owner.head.get()<owner.cap.get()}'
                                '&&(mem::size_of::<T>()!=0&&owner.cap.get()!=0||owner.probe_ptr().addr()==mem::align_of::<T>())'),
                     constructor_calls=()),
}


def owner_file(owner, file):
    if owner == 'Vec':
        return file == 'src/vec.rs' or file.startswith('src/vec/')
    if owner == 'VecDeque':
        return file == 'src/deque.rs' or file.startswith('src/deque/')
    return file == 'src/boxed/buffer.rs'


PROOF_FILES = {
    'boxed::buffer::proofs::': 'src/boxed/buffer_proofs.rs',
    'vec::metadata_proofs::': 'src/vec/metadata_proofs.rs',
    'vec::growth_proofs::': 'src/vec/growth_proofs.rs',
    'vec::edit::panic_proofs::': 'src/vec/edit_proofs.rs',
    'vec::ownership::panic_proofs::': 'src/vec/ownership_proofs.rs',
    'deque::proofs::closure::': 'src/deque/closure_proofs.rs',
    'collections::binary_heap::proofs::closure::': 'src/collections/binary_heap/closure_proofs.rs',
}
# The fixture helpers a closure harness may take its owner (or guard) from,
# each with the exact assumptions it makes: the owner invariant, or the pair
# invariant of a guard with its owner, and the admission of its words.
HELPERS = {
    ('src/vec/query_proofs.rs', 'closure_owner'): ['kani::assume(closure_invariant(&owner))'],
    ('src/vec/growth_proofs.rs', 'closure_sized_owner'): ['kani::assume(false)', 'kani::assume(closure_invariant(&owner))'],
    ('src/vec/ownership_proofs.rs', 'closure_iter'): ['kani::assume(span.is_ok())', 'kani::assume(closure_iter_invariant(&it))'],
    ('src/vec/edit_proofs.rs', 'closure_drain'): ['kani::assume(span.is_ok())', 'kani::assume(closure_drain_invariant(&drain))'],
    ('src/vec/edit_proofs.rs', 'closure_extract'): ['kani::assume(state.is_ok())', 'kani::assume(closure_extract_invariant(&extract))'],
    ('src/boxed/buffer_proofs.rs', 'closure_owner'): ['kani::assume(layout.is_ok())', 'kani::assume(closure_invariant(&owner))'],
    ('src/deque/closure_proofs.rs', 'closure_deque'): ['kani::assume(closure_invariant(&owner))'],
    # A Vec under the Vec invariant, read through the getters pinned in VEC_GETTERS.
    ('src/deque/closure_proofs.rs', 'closure_vec_owner'):
        ['kani::assume(owner.len()<=owner.capacity()&&owner.capacity()<=owner.max_len())'],
    # Issue #16, unit 7: a BinaryHeap whose one word is a Vec under the Vec invariant.
    ('src/collections/binary_heap/closure_proofs.rs', 'closure_heap'):
        ['kani::assume(owner.len()<=owner.capacity()&&owner.capacity()<=owner.max_len())'],
}
# The owner whose invariant each helper assumes. A harness that uses a helper
# is evidence for a site only while that owner is closed (the joint closure in
# `report`): the deque's into_vec harness assumes the VecDeque invariant to
# show the Vec invariant, its from_vec harness the converse.
HELPER_OWNERS = {
    ('src/vec/query_proofs.rs', 'closure_owner'): 'Vec',
    ('src/vec/growth_proofs.rs', 'closure_sized_owner'): 'Vec',
    ('src/vec/ownership_proofs.rs', 'closure_iter'): 'Vec',
    ('src/vec/edit_proofs.rs', 'closure_drain'): 'Vec',
    ('src/vec/edit_proofs.rs', 'closure_extract'): 'Vec',
    ('src/boxed/buffer_proofs.rs', 'closure_owner'): 'Buffer',
    ('src/deque/closure_proofs.rs', 'closure_deque'): 'VecDeque',
    ('src/deque/closure_proofs.rs', 'closure_vec_owner'): 'Vec',
    ('src/collections/binary_heap/closure_proofs.rs', 'closure_heap'): 'Vec',
}
# The Vec getters closure_vec_owner reads, as whole compacted definitions in src/vec.rs.
VEC_GETTERS = ['#[inline]pubfnlen(&self)->usize{self.len.get()}',
               '#[inline]pubfncapacity(&self)->usize{self.cap.get()}',
               '#[inline]pubfnmax_len(&self)->usize{self.limit.max_len()}']
# The pair invariants the guard helpers assume, pinned as text.
PAIR_INVARIANTS = {
    ('src/vec/edit_proofs.rs', 'closure_drain_invariant'):
        'matchdrain.tail_start.checked_add(drain.tail_len){Some(end)=>{end<=drain.owner.cap.get()'
        '&&(drain.owner.len.get()<=drain.tail_start||drain.tail_len==0)}None=>false,}',
    ('src/vec/edit_proofs.rs', 'closure_extract_invariant'):
        'extract.state.valid()&&extract.state.total()<=extract.owner.cap.get()',
    ('src/vec/ownership_proofs.rs', 'closure_iter_invariant'):
        'it.span.front()<=it.span.back()&&it.span.back()<=it.owner.cap.get()&&it.owner.len.get()==0',
}
# Harnesses that assume more than a helper. Evidence for no site, except the
# one an entry names under `path_assumption`.
EXTRA_ASSUMPTION = {
    'boxed::buffer::proofs::closure_collect_preserves_the_invariant_for_every_unit_owner_and_two_value_source':
        'a source of at most two values',
    'vec::metadata_proofs::closure_bounded_extend_cloned_returns_preserve_the_invariant': 'at most two new elements',
    'vec::metadata_proofs::closure_bounded_resize_returns_preserve_the_invariant': 'at most two new elements',
    'vec::metadata_proofs::closure_bounded_resize_owned_returns_preserve_the_invariant': 'at most two new elements',
    'vec::metadata_proofs::closure_bounded_extend_from_within_returns_preserve_the_invariant': 'at most two new elements',
    'vec::edit::panic_proofs::closure_splice_finish_with_a_tail_preserves_the_invariant': 'tail_len != 0',
    'vec::edit::panic_proofs::closure_splice_finish_with_a_tail_preserves_the_invariant_unlimited': 'tail_len != 0',
}

# The path assumptions an entry may accept: the exact assumption text and the
# one site whose path it selects (check_path_assumption pins that path).
PATH_ASSUMPTIONS = {'tail_len != 0': dict(
    assumes=['kani::assume(tail_len!=0)'],
    site=('Vec', 'write', 'src/vec/edit.rs', 'Splice', 'try_finish', 'self.drain.owner.len=next', 1))}

B = 'boxed::buffer::proofs::closure_'
M = 'vec::metadata_proofs::closure_'
G = 'vec::growth_proofs::closure_'
E = 'vec::edit::panic_proofs::closure_'
O = 'vec::ownership::panic_proofs::closure_'
SIZED_TYPES = ['u8', 'u64', '[u64; 200]', 'Aligned']


def h(*names, sized=(), note=None, path_assumption=None, fresh=None, dead=None, observed=None):
    # `fresh`: the written owner is created in the same function by the pinned
    # statement, so a harness of that function needs no fixture helper.
    # `dead`: the store does not execute at T = () (checked by `check_dead`);
    # the harness runs its function there, but no mutation of the store can be
    # detected at this instantiation. `observed`: the words the call produces
    # are read by the harness through the pinned function named here.
    return dict(disposition='harness', harnesses=list(names), sized=list(sized), note=note,
                path_assumption=path_assumption, fresh=fresh, dead=dead, observed=observed)


def d(disposition, reason, item=None):
    return dict(disposition=disposition, reason=reason, item=item)


VEC_GROWTH = [G + n for n in ('reserve_preserves_the_invariant', 'reserve_exact_preserves_the_invariant',
                              'grow_preserves_the_invariant', 'reserve_length_preserves_the_invariant',
                              'shrink_preserves_the_invariant_of_a_unit_owner')]
VEC_GROWTH_SIZED = ['vec::growth_proofs::sized::closure_' + n for n in (
    'reserve_preserves_the_invariant_of_every_sized_owner', 'reserve_exact_preserves_the_invariant_of_every_sized_owner',
    'grow_preserves_the_invariant_of_every_sized_owner', 'reserve_length_preserves_the_invariant_of_every_sized_owner',
    'shrink_preserves_the_invariant_of_every_sized_owner')]
PUSH_SIZED = ['vec::growth_proofs::sized::closure_push_growth_preserves_the_invariant_of_every_sized_owner']
EXCHANGE = [M + 'buffer_exchange_preserves_the_invariant_of_both_owners',
            M + 'buffer_exchange_preserves_the_invariant_of_both_unlimited_owners']
UNIT_STORAGE = M + 'adopting_constructors_establish_the_invariant_for_unit_storage'
Q = 'deque::proofs::closure::closure_'
QL = 'deque::proofs::closure::loops::closure_'
DEQUE_SIZED = ['deque::proofs::closure::sized::closure_capacity_constructors_establish_the_invariant_of_every_sized_owner']
# Stores that do not execute at a zero-sized element: (kind, guard text).
# 'enclosed': every occurrence of the store is inside the block the guard
# opens; 'return': a statement `if <guard> || .. { return ..; }` at the top
# of the function body precedes the store. 'head-zero' guards rest on the
# invariant itself at T = () (head == 0, so the ring is one segment).
ZST_GUARD = ('enclosed', 'ifmem::size_of::<T>()!=0{', 'the store is inside `if mem::size_of::<T>() != 0`')
ZST_RETURN = ('return', 'mem::size_of::<T>()==0', 'the function returns before the store when T is zero-sized')
KIND_GUARD = ('enclosed', 'ifkind!=0{', 'inside `if kind != 0`; at T = () the invariant gives head == 0, so the '
              'ring is one segment and contiguous_kind is 0')
HEAD_GUARD = ('enclosed', 'ifself.head.get()!=0{', 'inside `if self.head.get() != 0`; at T = () the invariant gives head == 0')

# (owner, kind, file, context, function, expression, occurrence) -> entry.
# Expressions are compacted (no whitespace); occurrences count from 1 among
# sites with the same other key parts, in source order.
SITES = {
    # Constructors (struct literals of the owner).
    ('Buffer', 'constructor', 'src/boxed/buffer.rs', 'Buffer', 'new', 'Self{ptr,len,cap,layout,policy}', 1):
        h(B + 'new_establishes_the_invariant', note='instantiated at (), u8, u64, [u64; 200] and a 64-aligned type'),
    ('Vec', 'constructor', 'src/vec.rs', 'Vec', 'new', 'Self{ptr,len,cap,policy,limit,marker}', 1):
        h(M + 'empty_constructors_establish_the_invariant'),
    ('Vec', 'constructor', 'src/vec.rs', 'Vec', 'empty', 'Self{ptr,len,cap,policy,limit,marker}', 1):
        h(M + 'empty_constructors_establish_the_invariant', M + 'capacity_constructors_establish_the_invariant',
          M + 'slice_constructor_establishes_the_invariant',
          sized=['vec::growth_proofs::sized::closure_capacity_constructors_establish_the_invariant_of_every_sized_owner']),
    ('Vec', 'constructor', 'src/vec/ownership.rs', 'Vec', 'from_raw_parts', 'Self{ptr,len,cap,policy,limit,marker}', 1):
        d('trust-boundary', 'the words are the arguments of the public unsafe fn Vec::from_raw_parts, whose '
          'documented contract states len <= capacity <= limit and the layout; each call inside the library is '
          'a site of its own (kind call)', item='public-unsafe-caller'),
    ('Vec', 'constructor', 'src/vec/ownership.rs', 'Vec', 'try_from_std_in', 'Self{ptr,len,cap,policy,limit,marker}', 1):
        h(UNIT_STORAGE, note='at (); the std facts of a sized value are the trust boundary std-adoption'),
    ('Vec', 'constructor', 'src/vec/ownership.rs', 'Vec', 'from_boxed_slice', 'Self{ptr,len,cap,policy,limit,marker}', 1):
        h(UNIT_STORAGE),
    # Calls of the public unsafe constructor inside the library.
    ('Vec', 'call', 'src/vec/convenience.rs', 'Vec', 'try_into_flattened', 'Vec::from_raw_parts', 1):
        h(M + 'flatten_establishes_the_invariant_of_the_element_owner', note='at [(); N], N = 0, 1, 3, usize::MAX'),
    ('Vec', 'call', 'src/deque.rs', 'VecDeque', 'into_vec', 'Vec::from_raw_parts', 1):
        h(Q + 'into_vec_establishes_the_vec_invariant',
          note='the deque owner comes from closure_deque (owner VecDeque); the harness asserts the Vec invariant'),
    ('Vec', 'call', 'src/deque.rs', 'VecDeque', 'try_from_std_in', 'Vec::from_raw_parts', 1):
        h(Q + 'adopting_constructor_establishes_the_invariant_for_unit_storage', observed='from_vec',
          note='at (); for a sized value vec.len() and cap = source.capacity() rest on the trust boundary '
               'std-adoption'),
    ('Vec', 'call', 'src/string/convert.rs', 'String', 'from_raw_parts', 'Vec::from_raw_parts', 1):
        d('not-instantiable', 'builds a Vec<u8> only (String::from_raw_parts takes NonNull<u8>); it forwards the '
          'words of the public unsafe fn String::from_raw_parts, whose contract states that every '
          'Vec::from_raw_parts requirement applies'),
    # Buffer words.
    ('Buffer', 'write', 'src/boxed/buffer.rs', 'Buffer', 'reserve', 'self.cap=Checked::new(proposed)', 1):
        h(B + 'reserve_preserves_the_invariant_for_every_unit_owner', B + 'push_preserves_the_invariant_for_every_unit_owner',
          B + 'append_preserves_the_invariant_for_every_unit_owner',
          sized=['boxed::buffer::proofs::sized::closure_reserve_preserves_the_invariant_for_every_sized_owner']),
    ('Buffer', 'write', 'src/boxed/buffer.rs', 'Buffer', 'reserve', 'self.layout=layout', 1):
        h(B + 'reserve_preserves_the_invariant_for_every_unit_owner', B + 'push_preserves_the_invariant_for_every_unit_owner',
          B + 'append_preserves_the_invariant_for_every_unit_owner',
          sized=['boxed::buffer::proofs::sized::closure_reserve_preserves_the_invariant_for_every_sized_owner']),
    ('Buffer', 'write', 'src/boxed/buffer.rs', 'Buffer', 'push', 'self.len=Checked::new(needed)', 1):
        h(B + 'push_preserves_the_invariant_for_every_unit_owner'),
    ('Buffer', 'write', 'src/boxed/buffer.rs', 'Buffer', 'append', 'self.len=Checked::new(needed)', 1):
        h(B + 'append_preserves_the_invariant_for_every_unit_owner'),
    ('Buffer', 'write', 'src/boxed/buffer.rs', 'Buffer', 'finish', 'self.layout=layout', 1):
        h(B + 'finish_transfers_exactly_the_length_of_every_unit_owner',
          sized=['boxed::buffer::proofs::sized::closure_finish_transfers_exactly_the_length_of_every_sized_owner'],
          note='the owner is consumed into the Box after the store; the harnesses assert the words of the Box'),
    ('Buffer', 'lend', 'src/boxed/buffer.rs', 'Buffer', 'collect', '&mutself.len', 1):
        h(B + 'collect_lend_preserves_the_invariant_for_every_unit_owner_and_open_source',
          note='the harness makes the lend itself; caller_bindings.LEND_HARNESSES pins the statement'),
    ('Buffer', 'lend', 'src/boxed/buffer.rs', 'Buffer', 'collect_rest', '&mutself.len', 1):
        h(B + 'collect_rest_lend_preserves_the_invariant_for_every_unit_owner_and_open_source',
          note='the harness makes the lend itself; caller_bindings.LEND_HARNESSES pins the statement'),
    # Vec capacity.
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'resize_storage', 'self.cap=capacity', 1): h(*VEC_GROWTH, sized=VEC_GROWTH_SIZED),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_push', 'self.cap=cap', 1): h(G + 'push_preserves_the_invariant', sized=PUSH_SIZED),
    ('Vec', 'write', 'src/vec.rs', '', 'grow_for_push', '*cap=capacity', 1):
        h(G + 'push_preserves_the_invariant', sized=PUSH_SIZED,
          note='a write through the local reference that try_push stores as self.cap'),
    # Vec length.
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_insert', 'self.len=Checked::new(needed)', 1): h(M + 'insert_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'pop', 'self.len=next', 1): h(M + 'pop_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'remove', 'self.len=next', 1): h(M + 'remove_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'swap_remove', 'self.len=next', 1): h(M + 'swap_remove_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'truncate', 'self.len=next', 1): h(M + 'truncate_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_extend_copy', 'self.len=Checked::new(needed)', 1):
        h(M + 'extend_copy_preserves_the_invariant', M + 'clone_establishes_the_invariant_of_the_copy'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_append', 'self.len=Checked::new(needed)', 1): h(M + 'append_preserves_the_invariant_of_both_owners'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_append', 'other.len=Checked::new(0)', 1): h(M + 'append_preserves_the_invariant_of_both_owners'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_split_off', 'tail.len=Checked::new(len)', 1): h(M + 'split_off_preserves_the_invariant_of_both_owners'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_split_off', 'self.len=Checked::new(at)', 1): h(M + 'split_off_preserves_the_invariant_of_both_owners'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_resize_with_init', 'guard.owner.len=next', 1):
        h(M + 'resize_with_init_preserves_the_invariant', M + 'resize_callers_preserve_the_invariant'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_extend_cloned', 'self.len=Checked::new(needed)', 1):
        h(M + 'extend_cloned_witness_preserves_the_invariant', note='the bitwise branch at [(); 1]'),
    ('Vec', 'write', 'src/vec.rs', 'Vec', 'try_clone', 'copy.len=Checked::new(0)', 1): h(M + 'clone_establishes_the_invariant_of_the_copy'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'push_within_capacity', 'self.len=next', 1): h(M + 'push_within_capacity_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_set_len', 'self.len=Checked::new(new_len)', 1): h(M + 'set_len_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_extend_from_within', 'guard.owner.len=next', 1): h(M + 'extend_from_within_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_extend_copy_from_within', 'self.len=Checked::new(needed)', 1):
        h(M + 'extend_copy_from_within_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_resize_owned', 'guard.owner.len=next', 1): h(M + 'resize_owned_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_resize_owned', 'guard.owner.len=next', 2): h(M + 'resize_owned_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_into_array', 'self.len=Checked::new(0)', 1):
        d('unobservable', 'the store is followed only by the read of the array and the drop of the consumed owner; '
          'vec::metadata_proofs::closure_into_array_preserves_the_invariant_of_a_returned_owner runs it'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_from_array_in', 'result.len=Checked::new(N)', 1):
        h(M + 'array_constructor_establishes_the_invariant', fresh='letmutresult=Self::empty(policy,limit);'),
    ('Vec', 'write', 'src/vec/convenience.rs', 'Vec', 'try_write_vectored', 'self.len=Checked::new(needed)', 1):
        d('not-instantiable', 'defined for Vec<u8> only'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Compact', 'drop', 'self.owner.len=Checked::new(self.state.restored_len())', 1):
        h(E + 'retain_preserves_the_invariant', E + 'dedup_preserves_the_invariant', note='reached on the callback-error return'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Vec', 'try_retain_mut', 'g.owner.len=Checked::new(g.state.written())', 1): h(E + 'retain_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Vec', 'try_dedup_by', 'g.owner.len=Checked::new(g.state.written())', 1): h(E + 'dedup_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Vec', 'try_drain', 'self.len=Checked::new(start)', 1): h(E + 'drain_constructor_establishes_the_pair_invariant'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Vec', 'try_extract_if', 'self.len=Checked::new(0)', 1): h(E + 'extract_constructor_establishes_the_pair_invariant'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Drain', 'restore_tail',
     'self.owner.len=Checked::new(crate::arithmetic::bounded::wrapping_add(self.owner.len.get(),self.tail_len,))', 1):
        h(E + 'drain_drop_restores_the_invariant_from_every_pair_state'),
    ('Vec', 'write', 'src/vec/edit.rs', 'ExtractIf', 'drop', 'self.owner.len=Checked::new(self.state.restored_len())', 1):
        h(E + 'extract_drop_restores_the_invariant_from_every_pair_state', E + 'extract_constructor_establishes_the_pair_invariant'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Splice', 'try_finish', 'self.drain.owner.len=next', 1):
        h(E + 'splice_finish_with_a_tail_preserves_the_invariant', E + 'splice_finish_with_a_tail_preserves_the_invariant_unlimited',
          path_assumption='tail_len != 0'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Splice', 'stage_excess', 'self.drain.owner.len=next_len', 1):
        h(E + 'stage_excess_preserves_the_owner_invariant_and_the_filled_pair',
          E + 'stage_excess_preserves_the_owner_invariant_and_the_filled_pair_unlimited'),
    ('Vec', 'write', 'src/vec/edit.rs', 'Splice', 'stage_excess', 'extra.len=Checked::new(0)', 1):
        d('unobservable', 'the store is followed only by `Ok(())` and the drop of the local staging owner `extra`'),
    ('Vec', 'write', 'src/vec/edit.rs', '', 'stage_push', 'extra.len=next', 1): h(E + 'stage_push_preserves_the_invariant'),
    ('Vec', 'write', 'src/vec/ownership.rs', 'Vec', 'into_iter', 'self.len=Checked::new(0)', 1): h(O + 'into_iter_preserves_the_invariant'),
    # Vec lends.
    ('Vec', 'lend', 'src/vec/edit.rs', 'Splice', 'fill', '&mutself.drain.owner.len', 1): h(E + 'splice_fill_preserves_the_owner_and_pair_invariants'),
    ('Vec', 'lend', 'src/vec/edit.rs', '', 'stage_batch', '&mutextra.len', 1): h(E + 'stage_batch_preserves_the_invariant'),
    **{('Vec', 'lend', 'src/vec.rs', 'Vec', 'try_reuse_larger_buffer', expression, 1): h(*EXCHANGE)
       for expression in ('&mutself.ptr', '&mutother.ptr', '&mutself.len', '&mutother.len', '&mutself.cap', '&mutother.cap')},
    # VecDeque (issue #16, unit 4). Constructors.
    ('VecDeque', 'constructor', 'src/deque.rs', 'VecDeque', 'new', 'Self{ptr,cap,head,len,policy,limit,marker}', 1):
        h(Q + 'empty_constructors_establish_the_invariant', note='instantiated at (), u8, u64, [u64; 200] and a 64-aligned type'),
    ('VecDeque', 'constructor', 'src/deque.rs', 'VecDeque', 'empty', 'Self{ptr,cap,head,len,policy,limit,marker}', 1):
        h(Q + 'empty_constructors_establish_the_invariant', Q + 'capacity_constructors_establish_the_invariant',
          Q + 'split_off_preserves_the_invariant_of_both_owners', sized=DEQUE_SIZED),
    ('VecDeque', 'constructor', 'src/deque.rs', 'VecDeque', 'from_vec', 'Self{ptr,len,head,cap,policy,limit,marker}', 1):
        h(Q + 'from_vec_establishes_the_invariant',
          note='the Vec comes from closure_vec_owner (owner Vec): from_vec is reached with every Vec state'),
    ('VecDeque', 'constructor', 'src/deque/traits.rs', 'VecDeque', 'clone_to', 'Self{ptr,cap,head,len,policy,limit,marker}', 1):
        h(Q + 'clone_establishes_the_invariant_of_the_copy', note='the bitwise path at (); the copy is published by its own store'),
    # VecDeque capacity and head of grow.
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'grow', 'self.head=Checked::new(head)', 1):
        h(Q + 'reserve_preserves_the_invariant', Q + 'push_preserves_the_invariant', Q + 'insert_preserves_the_invariant',
          note='at () grown_head(0, len, old, new) is 0'),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'grow', 'self.cap=cap', 1):
        h(Q + 'reserve_preserves_the_invariant', Q + 'push_preserves_the_invariant', Q + 'insert_preserves_the_invariant',
          sized=DEQUE_SIZED, note='sized only from an empty owner (old == 0), through the capacity constructors'),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'try_shrink_to', 'self.cap=cap', 1):
        h(Q + 'shrink_preserves_the_invariant_of_a_unit_owner', dead=ZST_RETURN),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'try_shrink_to', 'self.head=Checked::new(0)', 1):
        h(Q + 'shrink_preserves_the_invariant_of_a_unit_owner', dead=ZST_RETURN),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'try_shrink_to', 'self.head=Checked::new(head)', 1):
        h(Q + 'shrink_preserves_the_invariant_of_a_unit_owner', dead=ZST_RETURN),
    # VecDeque length and head.
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'try_push_back', 'self.len=Checked::new(add(len,1))', 1):
        h(Q + 'push_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'try_push_front',
     'self.head=Checked::new(geometry::retreat(self.head.get(),1,self.capacity()))', 1):
        h(Q + 'push_preserves_the_invariant', dead=ZST_GUARD),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'try_push_front', 'self.len=Checked::new(add(self.len(),1))', 1):
        h(Q + 'push_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'pop_front',
     'self.len=Checked::new(geometry::after_pop(self.len(),self.capacity()))', 1): h(Q + 'pop_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'pop_front',
     'self.head=Checked::new(geometry::allocated_physical(self.head.get(),1,self.capacity(),))', 1):
        h(Q + 'pop_preserves_the_invariant', dead=ZST_GUARD),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'pop_back', 'self.len=Checked::new(last)', 1): h(Q + 'pop_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'truncate', 'self.len=Checked::new(len)', 1):
        h(Q + 'truncate_preserves_the_invariant'),
    **{('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'make_contiguous', expression, occurrence):
       h(Q + 'make_contiguous_preserves_the_invariant', dead=KIND_GUARD)
       for expression, occurrence in (('self.head=Checked::new(0)', 1), ('self.head=Checked::new(b)', 1),
                                      ('self.head=Checked::new(free)', 1), ('self.head=Checked::new(0)', 2))},
    ('VecDeque', 'write', 'src/deque.rs', 'VecDeque', 'normalize', 'self.head=Checked::new(0)', 1):
        h(Q + 'into_vec_establishes_the_vec_invariant', dead=HEAD_GUARD),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'try_insert_mut',
     'self.head=Checked::new(geometry::retreat(self.head.get(),1,self.capacity()))', 1):
        h(Q + 'insert_preserves_the_invariant', dead=ZST_GUARD),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'try_insert_mut', 'self.len=Checked::new(add(self.len(),1))', 1):
        h(Q + 'insert_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'remove',
     'self.head=Checked::new(geometry::allocated_physical(self.head.get(),1,self.capacity(),))', 1):
        h(Q + 'remove_preserves_the_invariant', dead=ZST_GUARD),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'remove', 'self.len=Checked::new(sub(self.len(),1))', 1):
        h(Q + 'remove_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'rotate_left',
     'self.head=Checked::new(geometry::allocated_physical(self.head.get(),n,self.capacity(),))', 1):
        h(Q + 'rotate_preserves_the_invariant', dead=ZST_RETURN),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'rotate_right', 'self.head=Checked::new(head)', 1):
        h(Q + 'rotate_preserves_the_invariant', dead=ZST_RETURN),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'try_append', 'self.len=Checked::new(add(len,more))', 1):
        h(Q + 'append_preserves_the_invariant_of_both_owners'),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'try_append', 'other.len=Checked::new(0)', 1):
        h(Q + 'append_preserves_the_invariant_of_both_owners'),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'try_split_off', 'result.len=Checked::new(count)', 1):
        h(Q + 'split_off_preserves_the_invariant_of_both_owners',
          fresh='letmutresult=Self::empty(self.policy.try_clone()?,self.limit);'),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'VecDeque', 'try_split_off', 'self.len=Checked::new(at)', 1):
        h(Q + 'split_off_preserves_the_invariant_of_both_owners'),
    ('VecDeque', 'write', 'src/deque/edit.rs', 'Repair', 'drop', 'self.owner.len=Checked::new(self.state.restored_len())', 1):
        h(QL + 'retain_preserves_the_invariant',
          note='after the while-let of try_retain_mut, which runs under its loop contract (the predicate restates '
               'ptr through the cfg(kani) getter VecDeque::probe_ptr, owner decision 2026-11-28)'),
    ('VecDeque', 'write', 'src/deque/iter.rs', 'VecDeque', 'drain', 'self.len=Checked::new(start)', 1):
        h(Q + 'drain_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque/iter.rs', 'Retire', 'drop',
     'self.owner.len=Checked::new(sub(self.owner.len(),self.consumed))', 1):
        d('unobservable', 'reached only at the end of IntoIter::fold, after core slice folds over the whole ring; fold '
          'consumes the IntoIter, so after the guard only the drop of the consumed owner runs (pinned by '
          'check_retire_fold)'),
    ('VecDeque', 'write', 'src/deque/iter.rs', 'Retire', 'drop',
     'self.owner.head=Checked::new(geometry::physical(self.owner.head.get(),self.consumed,self.owner.capacity(),))', 1):
        d('unobservable', 'as the length store of Retire::drop; also inside `if mem::size_of::<T>() != 0`'),
    ('VecDeque', 'write', 'src/deque/iter.rs', 'Finish', 'drop',
     'd.owner.head=Checked::new(geometry::physical(d.owner.head.get(),removed,d.owner.capacity(),))', 1):
        h(Q + 'drain_preserves_the_invariant', dead=ZST_GUARD),
    ('VecDeque', 'write', 'src/deque/iter.rs', 'Finish', 'drop', 'd.owner.len=Checked::new(add(d.start,d.tail))', 1):
        h(Q + 'drain_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque/traits.rs', 'VecDeque', 'clone_to', 'result.len=Checked::new(self.len())', 1):
        h(Q + 'clone_establishes_the_invariant_of_the_copy'),
    ('VecDeque', 'write', 'src/deque/traits.rs', 'VecDeque', 'try_clone_from', 'self.len=Checked::new(0)', 1):
        h(Q + 'clone_from_preserves_the_invariant_of_both_owners'),
    ('VecDeque', 'write', 'src/deque/traits.rs', 'VecDeque', 'try_clone_from', 'self.head=Checked::new(0)', 1):
        h(Q + 'clone_from_preserves_the_invariant_of_both_owners'),
    ('VecDeque', 'write', 'src/deque/traits.rs', 'VecDeque', 'try_clone_from', 'self.len=Checked::new(source.len())', 1):
        h(Q + 'clone_from_preserves_the_invariant_of_both_owners'),
    ('VecDeque', 'write', 'src/deque/traits.rs', 'VecDeque', 'try_extend_copy', 'self.head=Checked::new(0)', 1):
        h(Q + 'extend_copy_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque/traits.rs', 'VecDeque', 'try_extend_copy', 'self.len=Checked::new(add(len,values.len()))', 1):
        h(Q + 'extend_copy_preserves_the_invariant'),
    ('VecDeque', 'write', 'src/deque/traits.rs', 'VecDeque', 'try_resize', 'self.len=Checked::new(new_len)', 1):
        h(Q + 'resize_preserves_the_invariant'),
}


def compact(text):
    return re.sub(r'\s+', '', text)


def strip_comments(text):
    return re.sub(r'//[^\n]*', '', text)


def block(text, start):
    opening = text.index('{', start)
    end, depth = opening + 1, 1
    while depth:
        depth += (text[end] == '{') - (text[end] == '}')
        end += 1
    return text[opening + 1:end - 1]


def functions(root, file):
    """Every `fn` of a proof file: name -> list of (body, is a harness).

    A `macro_rules!` definition is listed under `name!`, so that a harness that
    expands it is checked with the macro's text."""
    text = strip_comments((Path(root) / file).read_text())
    modules = []
    for m in re.finditer(r'\bmod\s+(\w+)\s*\{', text):
        inner = block(text, m.end() - 1)
        modules.append((m.end(), m.end() + len(inner), m.group(1)))

    def module(position):
        return tuple(name for start, end, name in modules if start <= position < end)
    found = {}
    for m in re.finditer(r'\bmacro_rules!\s*(\w+)\s*', text):
        found.setdefault(m.group(1) + '!', []).append((block(text, m.end()), False, module(m.start())))
    for m in re.finditer(r'\bfn\s+(\w+)\s*[<(]', text):
        harness = bool(re.search(r'#\[kani::proof\]\s*(#\[[^\]]*\]\s*)*$', text[:m.start()]))
        found.setdefault(m.group(1), []).append((block(text, m.end()), harness, module(m.start())))
    return found


def resolve(sources, file, scope, name):
    """The definitions a name in `scope` of `file` can refer to: the innermost
    enclosing module that defines it, else every other proof file's top level."""
    local = sources[file].get(name, [])
    for depth in range(len(scope), -1, -1):
        hits = [(file, x) for x in local if x[2] == scope[:depth]]
        if hits:
            return hits
    return [(f, x) for f, s in sources.items() if f != file for x in s.get(name, []) if not x[2]]


# The proof files whose functions a harness of each owner may call.
PROOF_SETS = {
    'Vec': ['src/vec/query_proofs.rs', 'src/vec/metadata_proofs.rs', 'src/vec/growth_proofs.rs',
            'src/vec/edit_proofs.rs', 'src/vec/ownership_proofs.rs'],
    'Buffer': ['src/boxed/buffer_proofs.rs'],
    'VecDeque': ['src/deque/closure_proofs.rs'],
    'BinaryHeap': ['src/collections/binary_heap/closure_proofs.rs'],
}


def proof_sources(root, owner):
    return {f: functions(root, f) for f in PROOF_SETS[owner]}


def check_fixtures(root):
    """The pinned helpers assume exactly their listed facts; the invariants are the stated text."""
    for (file, name), expected in HELPERS.items():
        bodies = functions(root, file).get(name, [])
        assert len(bodies) == 1, ('fixture helper missing or duplicated', file, name)
        assumes = re.findall(r'kani::assume\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\)', compact(bodies[0][0]))
        assert assumes == expected, ('fixture helper assumptions changed', file, name, assumes)
        assert 'any_where' not in bodies[0][0], ('fixture helper restricts a value', file, name)
    for owner, row in OWNERS.items():
        file, text = row['invariant']
        bodies = functions(root, file).get('closure_invariant', [])
        assert len(bodies) == 1 and compact(bodies[0][0]) == text, ('owner invariant changed', owner)
    for (file, name), text in PAIR_INVARIANTS.items():
        bodies = functions(root, file).get(name, [])
        assert len(bodies) == 1 and compact(bodies[0][0]) == text, ('pair invariant changed', file, name)
    assert set(HELPER_OWNERS) == set(HELPERS) and set(HELPER_OWNERS.values()) <= set(OWNERS), 'helper owner table changed'
    vec = compact(strip_comments((Path(root) / 'src/vec.rs').read_text()))
    for getter in VEC_GETTERS:
        assert vec.count(getter) == 1, ('Vec getter read by closure_vec_owner changed', getter)


def check_extra_assumptions(root, groups):
    """EXTRA_ASSUMPTION lists every harness registered with an extra assumption,
    and each listed harness does assume more than a fixture helper."""
    import verify_source
    registered = {name for extras in verify_source.EXTRA.values() for extra in extras
                  if extra.get('extra_assumption') for name in extra['harnesses']}
    assert registered <= set(EXTRA_ASSUMPTION), ('extra-assumption harness not listed', registered - set(EXTRA_ASSUMPTION))
    cache = {}
    for name in EXTRA_ASSUMPTION:
        assert name in groups, ('harness is not in a group that the certificate runs', name)
        assert admissible(root, name, 'write', cache) == 'assumes more than a fixture helper', \
            ('listed extra-assumption harness assumes nothing more', name)


def check_privacy(root):
    """The owner words are private: only the owner's module can write them."""
    for owner, row in OWNERS.items():
        file, name = row['struct']
        text = strip_comments((Path(root) / file).read_text())
        m = re.search(r'\bstruct\s+' + name + r'\s*<', text)
        assert m, ('owner struct missing', owner)
        fields = [f.strip() for f in block(text, m.end()).split(',') if f.strip()]
        assert fields and not [f for f in fields if f.startswith('pub')], ('owner field not private', owner, fields)
        assert {f.split(':')[0].strip() for f in fields} >= set(row['words']), ('owner words changed', owner, fields)
    # Child modules of the owner modules are proof files only, or files inside them.
    for path in [Path(root) / 'src/vec.rs', *sorted((Path(root) / 'src/vec').rglob('*.rs')), Path(root) / 'src/boxed/buffer.rs',
                 Path(root) / 'src/deque.rs', *sorted((Path(root) / 'src/deque').rglob('*.rs'))]:
        for target in re.findall(r'#\[path\s*=\s*"([^"]+)"\]', strip_comments(path.read_text())):
            assert target.endswith('proofs.rs'), ('owner module includes another file', path, target)


ASSUME = r'kani::assume\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\)'


def admissible(root, harness, kind, cache, fresh=False, allowed=(), uses=None):
    """None when the harness is admissible for a site of this kind, else the reason.

    `allowed` lists assumptions (compacted text) that an entry's path
    assumption accepts; any other assumption makes the harness inadmissible.
    `uses`, when given, receives the owners of the fixture helpers the harness
    calls (HELPER_OWNERS)."""
    prefix = max((p for p in PROOF_FILES if harness.startswith(p)), key=len, default=None)
    if prefix is None:
        return 'no proof file is registered for its module'
    file = PROOF_FILES[prefix]
    name = harness.rsplit('::', 1)[1]
    owner = next(o for o, files in PROOF_SETS.items() if file in files)
    sources = cache.setdefault(owner, proof_sources(root, owner))
    own = [x for x in sources[file].get(name, []) if x[1]]
    if len(own) != 1:
        return 'not a single #[kani::proof] function in ' + file
    helpers = set(HELPERS)
    seen, pending, used, texts = set(), [(file, own[0])], set(), []
    while pending:
        f, (body, _, scope) = pending.pop()
        if (f, body) in seen:
            continue
        seen.add((f, body))
        texts.append(body)
        # Every identifier that names a proof function or macro: a call, a
        # function passed by name (to a macro) or a macro invocation.
        for called in set(re.findall(r'(?<![.\w])(\w+!?)', body)):
            for target in resolve(sources, f, scope, called):
                if (target[0], called) in helpers:
                    used.add(called)
                    if uses is not None:
                        uses.add(HELPER_OWNERS[(target[0], called)])
                else:
                    pending.append(target)
    text = compact(''.join(texts))
    extra = re.findall(ASSUME, text)
    if 'any_where' in text or text.count('kani::assume') != len(extra) or sorted(extra) != sorted(allowed):
        return 'assumes more than a fixture helper'
    if (re.search(r'(?<![\w:])(Vec|Buffer|VecDeque|BinaryHeap)(::<[^{}]*>)?\{', text)
            or re.search(r'(?<![\w:])(Vec|Buffer)(::<.*?>)?::from_raw_parts\(', text) or re.search(r'(?<![\w.])units\(', text)):
        return 'builds an owner outside the fixture helpers'
    # A harness that stores to or lends an owner word itself narrows the state
    # like an assumption; the one exception is the pinned lend statement of a
    # harness in caller_bindings.LEND_HARNESSES, which repeats the product lend.
    from caller_bindings import LEND_HARNESSES
    lend = r'&mut(?:raw)?\w+(?:\.\w+)*\.(?:len|cap|limit|layout|ptr|head)\b'
    pinned = sorted(x for row in LEND_HARNESSES.values() if row['harness'] == harness
                    for _, h in row['statements'] for x in re.findall(lend, h))
    if re.search(r'\.(?:len|cap|limit|layout|ptr|head)[-+*/]?=(?!=)', text):
        return 'stores to an owner word itself'
    if sorted(re.findall(lend, text)) != pinned:
        return 'lends an owner word itself'
    if kind in ('write', 'lend') and not fresh and not used:
        return 'takes no owner from a fixture helper'
    return None


def harness_groups():
    import verify_source
    from source_certificate import FOUNDATION_GROUPS
    run = {}
    for group in FOUNDATION_GROUPS:
        for name in verify_source.GROUPS[group]['harnesses']:
            run.setdefault(name, []).append(group)
        for extra in verify_source.EXTRA.get(group, ()):
            for name in extra['harnesses']:
                run.setdefault(name, []).append(group)
    return run


def raw_parts_calls(root, out):
    """Calls of an owner's public unsafe constructor, from the unsafe-operation scanner."""
    from scanner_tools import scanner
    from caller_bindings import parsed_operations
    root = Path(root)
    files = sorted(str(p.relative_to(root)) for p in (root / 'src').rglob('*.rs') if not p.name.endswith('proofs.rs'))
    rows = json.loads(subprocess.check_output([str(scanner('unsafe_sites')), *files], cwd=root, text=True))
    calls = []
    others = {'Box::from_raw_parts', 'crate::boxed::Box::from_raw_parts', 'NonNull::slice_from_raw_parts',
              'core::ptr::NonNull::slice_from_raw_parts', 'core::slice::from_raw_parts', 'slice::from_raw_parts',
              'alloc::vec::Vec::from_raw_parts'}
    for site in parsed_operations(rows, Path(out) / 'operations'):
        context = compact(site.get('context', '')).split('<')[0]
        for op in site.get('operations', []):
            name = re.sub(r'::<.*?>(?=::)', '', compact(op['operation']))
            if op['kind'] != 'call' or 'from_raw_parts' not in name or name.endswith('from_raw_parts_mut'):
                continue
            if name == 'Self::from_raw_parts':
                name = context + '::from_raw_parts'
            if name in others:
                continue
            owner = next((o for o, row in OWNERS.items() if name in row['constructor_calls'] or name == o + '::from_raw_parts'), None)
            assert owner is not None, ('unclassified from_raw_parts call', site['file'], site['function'], name)
            calls.append(dict(owner=owner, kind='call', file=site['file'], context=context,
                              full_context=compact(site.get('context', '')),
                              function=site['function'], expression='Vec::from_raw_parts', line=site['line']))
    return calls


def sites(root, out):
    from caller_bindings import inventory, lend_bindings
    rows = inventory(root, out)
    found = []
    constructors = {}
    for r in rows:
        if r['kind'] != 'field':
            continue
        context = compact(r['context']).split('<')[0]
        ctor = compact(r['detail']['constructor'])
        for owner in OWNERS:
            if (ctor == 'Self' and context == owner) or ctor == owner:
                key = (owner, r['file'], context, r['function'], ctor)
                constructors.setdefault(key, []).append(r)
    for (owner, file, context, function, ctor), items in constructors.items():
        assert owner_file(owner, file), ('owner constructed outside its module', owner, file)
        fields = ','.join(compact(x['detail']['field']) for x in items)
        found.append(dict(owner=owner, kind='constructor', file=file, context=context, function=function,
                          expression=ctor + '{' + fields + '}', line=items[0]['line']))
    for r in rows:
        if r['kind'] != 'write':
            continue
        target = compact(r['detail']['target'])
        parts = target.lstrip('*').split('.')
        for owner, row in OWNERS.items():
            if owner_file(owner, r['file']) and parts[-1] in row['words'] and (len(parts) > 1 or target.startswith('*')):
                found.append(dict(owner=owner, kind='write', file=r['file'], context=compact(r['context']).split('<')[0],
                                  full_context=compact(r['context']),
                                  function=r['function'], expression=compact(r['expression']), line=r['line']))
    # caller_bindings binds lends of ptr/len/cap/limit/layout and refuses them
    # outside the foundation files; `head` is a VecDeque word only, so a borrow
    # or pattern binding of any VecDeque word is refused here.
    for r in rows:
        if r['kind'] in ('field_borrow', 'field_pattern') and owner_file('VecDeque', r['file']):
            members = r['detail'].get('members', [r['detail'].get('member')])
            assert not set(members) & set(OWNERS['VecDeque']['words']), ('borrow of a VecDeque word', r['file'], r['line'])
    lends, _ = lend_bindings(rows, lambda name: name)
    for b in lends:
        r = b['site']
        owner = next(o for o in OWNERS if owner_file(o, r['file']))
        found.append(dict(owner=owner, kind='lend', file=r['file'], context=compact(r['context']).split('<')[0],
                          function=r['function'], expression=compact(r['expression']), line=r['line']))
    found += raw_parts_calls(root, out)
    count = {}
    for s in sorted(found, key=lambda s: (s['file'], s['line'])):
        base = (s['owner'], s['kind'], s['file'], s['context'], s['function'], s['expression'])
        count[base] = count.get(base, 0) + 1
        s['occurrence'] = count[base]
        s['key'] = base + (count[base],)
    return found


def check_path_assumption(root):
    """`tail_len != 0` selects the only path of Splice::try_finish that reaches its store."""
    body = compact(strip_comments(block((Path(root) / 'src/vec/edit.rs').read_text(),
                                        (Path(root) / 'src/vec/edit.rs').read_text().index('pub fn try_finish('))))
    branch = 'ifself.drain.tail_len==0{'
    assert body.count(branch) == 1 and body.count('self.drain.owner.len=next;') == 1, 'try_finish shape changed'
    start = body.index(branch) + len(branch) - 1
    inner = block(body, start)
    assert inner.endswith('returnOk(());') and 'self.drain.owner.len=' not in inner, 'empty-tail branch changed'
    end = start + len(inner) + 2
    assert body.index('self.drain.owner.len=next;') > end, 'the store moved into the empty-tail branch'
    before = body[:body.index('self.drain.owner.len=next;')]
    assert not re.search(r'tail_len[-+*/]?=(?!=)|&mut[\w.]*tail_len', before), 'tail_len written before the store'
    case = functions(root, 'src/vec/edit_proofs.rs').get('closure_splice_finish_case', [])
    assert len(case) == 1, 'whole try_finish case missing'
    case = compact(case[0][0])
    assert case.count('kani::assume') == 1 and 'kani::assume(tail_len!=0)' in case and 'lettail_len=drain.tail_len;' in case, \
        'the path assumption of the whole try_finish harness changed'
    return ('the store `self.drain.owner.len = next` follows the `if self.drain.tail_len == 0 { .. return Ok(()); }` '
            'branch, which writes no owner word, and tail_len is not written before it; the harness assumes only '
            'tail_len != 0 beyond its helpers')


def enclosing_body(root, site):
    """The comment-stripped, compacted body of the function that holds the site."""
    text = strip_comments((Path(root) / site['file']).read_text())
    offset = sum(len(line) + 1 for line in text.split('\n')[:site['line'] - 1])
    heads = [m for m in re.finditer(r'\bfn\s+' + site['function'] + r'\b', text) if m.start() < offset]
    assert heads, ('enclosing function not found', site['key'])
    return compact(block(text, heads[-1].end()))


def check_dead(root, site, dead):
    """The store does not execute at T = (): the guard text is checked, the argument recorded."""
    kind, guard, argument = dead
    body = enclosing_body(root, site)
    expression = site['expression']
    total = body.count(expression)
    assert total >= 1, ('dead store not found in its function', site['key'])
    if kind == 'enclosed':
        guarded, start = 0, 0
        while (i := body.find(guard, start)) >= 0:
            guarded += block(body, i + len(guard) - 1).count(expression)
            start = i + len(guard)
        assert guarded == total, ('store not inside its guard', site['key'], guard)
    else:
        first = body.index(expression)
        found = False
        for m in re.finditer(r'if([^{};]*)\{return[^{};]*;\}', body[:first]):
            depth = body[:m.start()].count('{') - body[:m.start()].count('}')
            if depth == 0 and guard in m.group(1).split('||'):
                found = True
        assert found, ('no early return before the store', site['key'], guard)
    return 'not executed at T = (): ' + argument


def check_retire_fold(root):
    """IntoIter::fold consumes its iterator and ends with the second slice fold."""
    text = strip_comments((Path(root) / 'src/deque/iter.rs').read_text())
    heads = [m for m in re.finditer(r'\bfn\s+fold<B,\s*F:\s*FnMut\(B,\s*T\)\s*->\s*B>\(mut self,', text)]
    assert len(heads) == 1, 'IntoIter::fold signature changed'
    body = compact(block(text, heads[0].end()))
    guard = 'letmutguard=Retire{owner:&mutself.owner,consumed:0,};'
    assert body.count(guard) == 1 and body.endswith('letvalue=a.iter().fold(initial,&mutconsume);b.iter().fold(value,consume)'), \
        'IntoIter::fold shape changed'
    assert 'self' not in body[body.index(guard) + len(guard):], 'IntoIter::fold uses self after the guard'
    return ('IntoIter::fold takes `mut self`; after `let mut guard = Retire { owner: &mut self.owner, .. }` it uses '
            'only the slices, and its last expression is `b.iter().fold(value, consume)`, so the guard drop is '
            'followed only by the drop of the consumed owner')


OBSERVED = {'from_vec': [
    ('src/deque.rs', 'pub fn from_vec(',
     'let(ptr,len,cap,policy,limit)=source.into_raw_parts();Self{ptr,len:Checked::new(len),head:Checked::new(0),'
     'cap:unsafe{Capacity::from_validated_raw(cap)},policy,limit,marker:PhantomData,}'),
    ('src/vec/ownership.rs', 'pub fn into_raw_parts(',
     'letowner=core::mem::ManuallyDrop::new(self);(owner.ptr,owner.len.get(),owner.cap.get(),'
     'unsafe{core::ptr::read(&owner.policy)},owner.limit,)')]}


def check_observed(root, site, through):
    """The Vec built at the call goes straight to the pinned functions that copy its words."""
    body = enclosing_body(root, site)
    assert body.count('letowner=unsafe{Vec::from_raw_parts(') == 1 and body.endswith('Ok(Self::from_vec(owner))'), \
        ('the adopted Vec is not handed to from_vec', site['key'])
    for file, head, expected in OBSERVED[through]:
        text = strip_comments((Path(root) / file).read_text())
        assert text.count(head) == 1 and compact(block(text, text.index(head))) == expected, ('pinned body changed', file, head)
    return ('the Vec is passed only to VecDeque::from_vec, which copies len, cap and limit through Vec::into_raw_parts '
            'verbatim; the harness reads them from the deque')


# Issue #16, unit 7. A wrapper is a struct whose one private word is a value of
# a closed owner; its invariant is that owner's invariant of the field. The
# field's words are private to the inner owner's module (check_privacy), so a
# wrapper changes them only through the inner owner's methods, which are its
# sites. `check_wrapper` pins this and maps the wrapper's own sites: every
# struct literal, every lend of the field, every whole-value swap; a write of
# the field or a pattern binding of it fails.
HP = 'collections::binary_heap::proofs::closure::closure_'
HEAP_FILES = ('src/collections/binary_heap.rs', 'src/collections/binary_heap/core.rs',
              'src/collections/binary_heap/geometry.rs', 'src/collections/binary_heap/iter.rs')


def vc(via, reason):
    # `via`: compacted text that occurs exactly once in the enclosing function
    # and hands the field to a method of the inner owner (or exchanges whole values).
    return dict(disposition='inner-closure', via=via, reason=reason)


WRAPPERS = {
    'BinaryHeap': dict(
        struct=('src/collections/binary_heap.rs', 'BinaryHeap'), field='data:Vec<T,P,L>', inner='Vec', files=HEAP_FILES,
        # The two heap helpers that receive the field: only Vec methods touch it.
        helpers={('src/collections/binary_heap/core.rs', 'unsafe fn set_len<'): 'let_=unsafe{data.try_set_len(len)};',
                 ('src/collections/binary_heap/core.rs', 'unsafe fn pop_nonempty<'):
                     'letlast=sub(data.len(),1);letvalue=unsafe{data.as_ptr().add(last).read()};unsafe{set_len(data,last)};'
                     'iflast==0{returnvalue;}letresult=unsafe{data.as_mut_ptr().replace(value)};'
                     'unsafe{sift_bottom(data.as_mut_slice(),0,last)};result'},
        # The kernels that the heap runs on the field's elements take a slice, never the Vec.
        kernels=('unsafefnsift_up<T:Ord>(data:&mut[T],', 'unsafefnsift_down<T:Ord>(data:&mut[T],',
                 'unsafefnsift_bottom<T:Ord>(data:&mut[T],', 'fnrebuild<T:Ord>(data:&mut[T])',
                 'fnrebuild_tail<T:Ord>(data:&mut[T],'),
        # (kind, file, context, function, expression, occurrence) -> entry.
        sites={
            ('constructor', 'src/collections/binary_heap.rs', 'BinaryHeap', 'new', 'Self{data}', 1):
                h(HP + 'constructors_establish_the_invariant'),
            ('constructor', 'src/collections/binary_heap.rs', 'BinaryHeap', 'try_with_capacity', 'Self{data}', 1):
                h(HP + 'constructors_establish_the_invariant'),
            ('constructor', 'src/collections/binary_heap.rs', 'BinaryHeap', 'new_in', 'Self{data}', 1):
                h(HP + 'constructors_establish_the_invariant'),
            ('constructor', 'src/collections/binary_heap.rs', 'BinaryHeap', 'try_with_capacity_in', 'Self{data}', 1):
                h(HP + 'constructors_establish_the_invariant'),
            ('constructor', 'src/collections/binary_heap.rs', 'BinaryHeap', 'from_vec', 'Self{data}', 1):
                h(HP + 'constructors_establish_the_invariant',
                  note='run at N = 3 through try_from_array; the field is the moved-in Vec value, and '
                       'rebuild(data.as_mut_slice()) before the literal takes &mut [T]'),
            ('constructor', 'src/collections/binary_heap.rs', 'BinaryHeap', 'try_clone', 'Self{data}', 1):
                h(HP + 'constructors_establish_the_invariant'),
            ('lend', 'src/collections/binary_heap.rs', 'BinaryHeap', 'pop', '&mutself.data', 1):
                h(HP + 'push_pop_preserve_the_invariant'),
            ('lend', 'src/collections/binary_heap.rs', 'BinaryHeap', 'try_append', '&mutother.data', 1):
                vc('self.data.try_reuse_larger_buffer(&mutother.data)', 'a Vec method (Vec site, the exchange harnesses)'),
            ('lend', 'src/collections/binary_heap.rs', 'BinaryHeap', 'try_append', '&mutother.data', 2):
                vc('self.data.try_append(&mutother.data)', 'a Vec method (Vec site, the append harnesses)'),
            ('swap', 'src/collections/binary_heap.rs', 'BinaryHeap', 'try_append', '::core::mem::swap(self,other)', 1):
                vc('pubfntry_append(&mutself,other:&mutSelf)', 'exchanges two whole BinaryHeap values'),
            ('swap', 'src/collections/binary_heap.rs', 'BinaryHeap', 'try_append', '::core::mem::swap(self,other)', 2):
                vc('pubfntry_append(&mutself,other:&mutSelf)', 'exchanges two whole BinaryHeap values'),
            ('lend', 'src/collections/binary_heap.rs', 'PeekMut', 'deref_mut', '&mutself.heap.data', 1):
                h(HP + 'peek_mut_restores_the_length'),
            ('lend', 'src/collections/binary_heap.rs', 'PeekMut', 'drop', '&mutself.heap.data', 1):
                h(HP + 'peek_mut_restores_the_length'),
            ('lend', 'src/collections/binary_heap.rs', 'PeekMut', 'pop', '&mutthis.heap.data', 1):
                h(HP + 'peek_mut_restores_the_length'),
            ('lend', 'src/collections/binary_heap.rs', 'PeekMut', 'pop', '&mutthis.heap.data', 2):
                h(HP + 'peek_mut_restores_the_length'),
            ('lend', 'src/collections/binary_heap/iter.rs', 'Drain', 'new', '&mutheap.data', 1):
                h(HP + 'drain_retires_the_length'),
        }),
}


def wrapper_sites(root, out, name, row):
    """The sites of a wrapper from the scanner inventory, with the structural pins."""
    from caller_bindings import inventory
    root = Path(root)
    file, struct = row['struct']
    text = strip_comments((root / file).read_text())
    m = re.search(r'\bstruct\s+' + struct + r'\s*<', text)
    assert m, ('wrapper struct missing', name)
    fields = compact(block(text, m.end())).rstrip(',')
    assert fields == row['field'], ('wrapper fields changed', name, fields)
    actual = {str(p.relative_to(root)) for p in [root / file, *(root / file[:-3]).rglob('*.rs')]
              if not p.name.endswith('proofs.rs')}
    assert actual == set(row['files']), ('wrapper files changed', name, sorted(actual))
    for f in row['files']:
        for target in re.findall(r'#\[path\s*=\s*"([^"]+)"\]', strip_comments((root / f).read_text())):
            assert target.endswith('proofs.rs'), ('wrapper module includes another file', f, target)
    for (f, head), body in row['helpers'].items():
        source = strip_comments((root / f).read_text())
        assert source.count(head) == 1 and compact(block(source, source.index(head))) == body, \
            ('wrapper helper changed', name, head)
    kernels = compact(strip_comments((root / 'src/collections/binary_heap/core.rs').read_text()))
    for head in row['kernels']:
        assert kernels.count(head) == 1, ('kernel no longer takes a slice', name, head)
    word = row['field'].split(':')[0]
    rows = inventory(root, Path(out) / ('wrapper-' + name))
    found = []
    for r in rows:
        if r['file'] not in row['files']:
            continue
        context = compact(r.get('context', '')).split('<')[0]
        if r['kind'] == 'field':
            ctor = compact(r['detail']['constructor'])
            if (ctor == 'Self' and context == struct) or ctor == struct:
                assert compact(r['detail']['field']) == word, ('wrapper literal with another field', r)
                found.append(dict(kind='constructor', file=r['file'], context=context, function=r['function'],
                                  expression=ctor + '{' + word + '}', line=r['line']))
        elif r['kind'] == 'write':
            target = compact(r['detail']['target'])
            assert target.lstrip('*').split('.')[-1] != word, ('write of a wrapper word', r['file'], r['line'])
        elif r['kind'] == 'field_pattern':
            assert r['detail'].get('member') != word, ('pattern binding of a wrapper word', r['file'], r['line'])
        elif r['kind'] == 'field_borrow':
            members = r['detail'].get('members', [r['detail'].get('member')])
            if members[-1] == word:
                found.append(dict(kind='lend', file=r['file'], context=context, function=r['function'],
                                  expression=compact(r['expression']), line=r['line']))
        elif r['kind'] == 'protocol_call' and re.search(r'mem::(swap|replace|take)$', compact(r['detail']['callee'])):
            found.append(dict(kind='swap', file=r['file'], context=context, function=r['function'],
                              expression=compact(r['expression']), line=r['line']))
    count = {}
    for s in sorted(found, key=lambda s: (s['file'], s['line'])):
        base = (s['kind'], s['file'], s['context'], s['function'], s['expression'])
        count[base] = count.get(base, 0) + 1
        s['occurrence'] = count[base]
        s['key'] = base + (count[base],)
    return found


def wrapper_report(root, out, closed, groups, cache, wrappers=None):
    """A wrapper is closed while its inner owner is closed and every site is mapped."""
    result = {}
    for name, row in (WRAPPERS if wrappers is None else wrappers).items():
        found = wrapper_sites(root, out, name, row)
        keys = [s['key'] for s in found]
        assert len(keys) == len(set(keys)), ('duplicate wrapper site', name)
        missing = [s['key'] + (s['line'],) for s in found if s['key'] not in row['sites']]
        assert not missing, ('wrapper site without an entry', missing)
        stale = sorted(set(row['sites']) - set(keys))
        assert not stale, ('wrapper entry without a site', stale)
        rows = []
        for s in found:
            entry = row['sites'][s['key']]
            r = dict(kind=s['kind'], file=s['file'], line=s['line'], context=s['context'], function=s['function'],
                     expression=s['expression'], occurrence=s['occurrence'], disposition=entry['disposition'])
            if entry['disposition'] == 'harness':
                evidence, refused = [], []
                for harness in entry['harnesses']:
                    assert harness in groups, ('harness is not in a group that the certificate runs', harness)
                    uses = set()
                    reason = admissible(Path(root), harness, s['kind'], cache, uses=uses)
                    if reason is None and not uses <= closed:
                        reason = 'assumes the invariant of an owner that is not closed: ' + ', '.join(sorted(uses - closed))
                    (refused if reason else evidence).append(dict(harness=harness, groups=groups[harness], reason=reason,
                                                                  uses=sorted(uses)))
                r.update(harnesses=evidence, refused=refused, note=entry.get('note'))
                if not evidence:
                    r['disposition'] = 'uncovered'
            elif entry['disposition'] == 'inner-closure':
                assert s['kind'] != 'constructor', ('a wrapper literal needs a harness', s['key'])
                assert enclosing_body(root, s).count(entry['via']) == 1 or \
                    compact(strip_comments((Path(root) / s['file']).read_text())).count(entry['via']) == 1, \
                    ('pinned hand-over moved', s['key'], entry['via'])
                r.update(via=entry['via'], reason=entry['reason'])
            else:
                raise AssertionError(('unknown wrapper disposition', s['key'], entry['disposition']))
            rows.append(r)
        blocking = [r for r in rows if r['disposition'] == 'uncovered']
        inner = row['inner'] in closed
        result[name] = dict(closed=inner and not blocking, instantiation='zero-sized element', wrapper_of=row['inner'],
                            sites=rows, depends_on=[row['inner']],
                            blocking=[dict(kind=r['kind'], file=r['file'], line=r['line'], function=r['function'],
                                           expression=r['expression'], disposition=r['disposition'],
                                           reason=[x['reason'] for x in r.get('refused', [])]) for r in blocking]
                            + ([] if inner else [dict(kind='inner', file=row['struct'][0], line=0, function='',
                                                      expression=row['field'], disposition='inner-not-closed',
                                                      reason=row['inner'] + ' is not closed')]),
                            sized_capacity_harnesses=[], sized_types=[],
                            counts={k: sum(r['disposition'] == k for r in rows) for k in sorted({r['disposition'] for r in rows})})
    return result


def report(root=ROOT, out=None, table=None, groups=None, wrappers=None):
    root = Path(root)
    out = Path(out or ROOT / 'target/verification/owner-closure')
    out.mkdir(parents=True, exist_ok=True)
    table = SITES if table is None else table
    groups = harness_groups() if groups is None else groups
    check_privacy(root)
    check_fixtures(root)
    check_extra_assumptions(root, groups)
    path_reason = check_path_assumption(root)
    retire_reason = check_retire_fold(root)
    found = sites(root, out)
    keys = [s['key'] for s in found]
    assert len(keys) == len(set(keys)), 'duplicate site key'
    missing = [s for s in found if s['key'] not in table]
    assert not missing, ('site without an entry', [s['key'] + (s['line'],) for s in missing])
    stale = sorted(set(table) - set(keys))
    assert not stale, ('entry without a site', stale)
    cache = {}
    rows_by_owner = {}
    for owner in OWNERS:
        rows = []
        for s in [s for s in found if s['owner'] == owner]:
            entry = table[s['key']]
            row = dict(kind=s['kind'], file=s['file'], line=s['line'], context=s['context'], function=s['function'],
                       expression=s['expression'], occurrence=s['occurrence'], disposition=entry['disposition'])
            if entry['disposition'] == 'harness':
                assert entry['harnesses'], ('harness entry without a harness', s['key'])
                evidence, refused = [], []
                fresh = entry.get('fresh')
                if fresh:
                    source = (root / s['file']).read_text()
                    head = source.index('fn ' + s['function'])
                    assert compact(strip_comments(block(source, head))).count(fresh) == 1, ('fresh owner statement moved', s['key'])
                    row['fresh_owner'] = fresh
                if entry.get('dead'):
                    row['dead'] = check_dead(root, s, entry['dead'])
                if entry.get('observed'):
                    row['observed'] = check_observed(root, s, entry['observed'])
                for name in entry['harnesses']:
                    assert name in groups, ('harness is not in a group that the certificate runs', name)
                    uses = set()
                    if name in EXTRA_ASSUMPTION:
                        # Admissible for this site only when its path assumption
                        # is the harness's one extra assumption, checked above.
                        accepted = PATH_ASSUMPTIONS.get(entry.get('path_assumption'))
                        if accepted and accepted['site'] == s['key'] and entry['path_assumption'] == EXTRA_ASSUMPTION[name]:
                            reason = admissible(root, name, s['kind'], cache, fresh=bool(fresh), allowed=accepted['assumes'],
                                                uses=uses)
                            if reason is None:
                                row['path_assumption'] = dict(assumption=entry['path_assumption'], reason=path_reason)
                        else:
                            reason = 'extra assumption: ' + EXTRA_ASSUMPTION[name]
                    else:
                        reason = admissible(root, name, s['kind'], cache, fresh=bool(fresh), uses=uses)
                    (refused if reason else evidence).append(dict(harness=name, groups=groups[name], reason=reason,
                                                                  uses=sorted(uses)))
                sized = []
                for name in entry.get('sized', []):
                    assert name in groups, ('harness is not in a group that the certificate runs', name)
                    uses = set()
                    reason = admissible(root, name, s['kind'], cache, uses=uses)
                    (refused if reason else sized).append(dict(harness=name, groups=groups[name], reason=reason,
                                                               uses=sorted(uses)))
                row.update(harnesses=evidence, refused=refused, sized=sized, note=entry.get('note'))
            else:
                row.update(reason=entry['reason'])
                if entry['disposition'] == 'trust-boundary':
                    assert entry.get('item') in TRUST_BOUNDARY, ('trust-boundary outside the fixed list', s['key'])
                    assert s['kind'] == 'constructor', ('trust-boundary only for a constructor', s['key'])
                    text = compact(strip_comments((root / s['file']).read_text()))
                    assert 'pubunsafefn' + s['function'] + '(' in text, ('not a public unsafe fn', s['key'])
                    row['item'] = TRUST_BOUNDARY[entry['item']]
                elif entry['disposition'] == 'not-instantiable':
                    # The enclosing impl fixes the element type to u8: Vec<u8, ..>,
                    # or String, whose bytes are a Vec<u8>.
                    assert s.get('full_context', '').startswith(('Vec<u8,', 'String<')), \
                        ('instantiable at a zero-sized element', s['key'], s.get('full_context'))
                else:
                    assert entry['disposition'] in ('unobservable', 'uncovered') or entry['disposition'].startswith('other-unit:'), \
                        ('unknown disposition', s['key'], entry['disposition'])
                    assert s['kind'] != 'constructor' or entry['disposition'] != 'unobservable', s['key']
                    if entry['disposition'] == 'unobservable' and s['file'] == 'src/deque/iter.rs' and s['context'] == 'Retire':
                        row['pinned'] = retire_reason
            rows.append(row)
        rows_by_owner[owner] = rows
    # Joint closure, a greatest fixpoint: start from every owner closed and
    # remove an owner while one of its sites has no harness whose helpers all
    # belong to owners still closed. The invariants then hold together: each
    # constructor and writer of each closed owner was run assuming only the
    # invariants of closed owners.
    # A harness of an owner may assume that owner's own invariant (the
    # inductive step); the invariants of other owners count only while they
    # are closed.
    closed = set(OWNERS)

    def counts(x, owner):
        return set(x['uses']) - {owner} <= closed
    while True:
        still = set()
        for owner, rows in rows_by_owner.items():
            blocked = [r for r in rows if r['disposition'].startswith('other-unit:') or r['disposition'] == 'uncovered'
                       or (r['disposition'] == 'harness' and not any(counts(x, owner) for x in r['harnesses']))
                       or (r['kind'] == 'constructor' and r['disposition'] not in ('harness', 'trust-boundary'))]
            if not blocked:
                still.add(owner)
        if still == closed:
            break
        closed = still
    result = {}
    for owner, rows in rows_by_owner.items():
        for r in rows:
            if r['disposition'] != 'harness':
                continue
            waiting = [x for x in r['harnesses'] if not counts(x, owner)]
            for x in waiting:
                x['reason'] = 'assumes the invariant of an owner that is not closed: ' + ', '.join(
                    o for o in x['uses'] if o not in closed and o != owner)
            r['refused'] = r['refused'] + waiting
            r['harnesses'] = [x for x in r['harnesses'] if counts(x, owner)]
            r['sized'] = [x for x in r['sized'] if counts(x, owner)]
            if not r['harnesses']:
                r['disposition'] = 'uncovered'
        blocking = [r for r in rows if r['disposition'] in ('uncovered',) or r['disposition'].startswith('other-unit:')
                    or (r['kind'] == 'constructor' and r['disposition'] not in ('harness', 'trust-boundary'))]
        assert (not blocking) == (owner in closed), ('joint closure inconsistent', owner)
        sized = sorted({x['harness'] for r in rows for x in r.get('sized', [])})
        result[owner] = dict(closed=not blocking, instantiation='zero-sized element', sites=rows,
                             depends_on=sorted({o for r in rows for x in r.get('harnesses', []) for o in x['uses']} - {owner}),
                             blocking=[dict(kind=r['kind'], file=r['file'], line=r['line'], function=r['function'],
                                            expression=r['expression'], disposition=r['disposition'],
                                            reason=r.get('reason') or [x['reason'] for x in r.get('refused', [])])
                                       for r in blocking],
                             sized_capacity_harnesses=sized, sized_types=SIZED_TYPES if sized else [],
                             counts={k: sum(r['disposition'] == k for r in rows) for k in sorted({r['disposition'] for r in rows})})
    result.update(wrapper_report(root, out, closed, groups, cache, wrappers))
    (out / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


RETAIN_SITE = ('VecDeque', 'write', 'src/deque/edit.rs', 'Repair', 'drop',
               'self.owner.len=Checked::new(self.state.restored_len())', 1)


def controls(root=ROOT, out=None):
    """Defects, each must make the result not closed or fail the script."""
    import shutil
    import tempfile
    out = Path(out or ROOT / 'target/verification/owner-closure') / 'controls'
    results = []

    def copy():
        work = Path(tempfile.mkdtemp(prefix='owner-closure-', dir=out))
        shutil.copytree(Path(root) / 'src', work / 'src')
        return work

    def mutate(work, file, old, new):
        path = work / file
        text = path.read_text()
        assert text.count(old) == 1, ('control anchor', file, old)
        path.write_text(text.replace(old, new))

    out.mkdir(parents=True, exist_ok=True)
    work = copy()
    mutate(work, 'src/boxed/buffer.rs', '        self.len = Checked::new(needed);\n        Ok(())\n    }\n    #[inline]\n    pub(super) fn append',
           '        self.len = Checked::new(needed);\n        self.len = Checked::new(needed);\n        Ok(())\n    }\n    #[inline]\n    pub(super) fn append')
    try:
        report(work, out / 'unmapped')
        results.append(dict(name='unmapped-write', detected=False))
    except AssertionError as error:
        results.append(dict(name='unmapped-write', detected='site without an entry' in str(error), error=str(error)[:300]))
    work = copy()
    mutate(work, 'src/boxed/buffer_proofs.rs', '    let result = owner.push((), kani::any());',
           '    kani::assume(owner.len.get() == 0);\n    let result = owner.push((), kani::any());')
    r = report(work, out / 'assume')
    results.append(dict(name='harness-assumption', detected=not r['Buffer']['closed'], blocking=r['Buffer']['blocking']))
    groups = harness_groups()
    groups.pop(B + 'push_preserves_the_invariant_for_every_unit_owner')
    try:
        report(root, out / 'group', groups=groups)
        results.append(dict(name='harness-out-of-group', detected=False))
    except AssertionError as error:
        results.append(dict(name='harness-out-of-group', detected='not in a group' in str(error), error=str(error)[:300]))
    table = dict(SITES)
    key = ('Buffer', 'lend', 'src/boxed/buffer.rs', 'Buffer', 'collect_rest', '&mutself.len', 1)
    table[key] = d('other-unit:9', 'control')
    r = report(root, out / 'other-unit', table=table)
    results.append(dict(name='other-unit-site', detected=not r['Buffer']['closed'], blocking=r['Buffer']['blocking']))
    # Issue #16, unit 4: the joint closure of Vec and VecDeque.
    work = copy()
    mutate(work, 'src/deque/edit.rs', '        self.len = Checked::new(at);\n        Ok(result)',
           '        self.len = Checked::new(at);\n        self.len = Checked::new(at);\n        Ok(result)')
    try:
        report(work, out / 'deque-unmapped')
        results.append(dict(name='deque-unmapped-write', detected=False))
    except AssertionError as error:
        results.append(dict(name='deque-unmapped-write', detected='site without an entry' in str(error), error=str(error)[:300]))
    # The retain harness weakened by an assumption, or the retain store left
    # without a harness: VecDeque is not closed, and through the joint closure
    # neither is Vec. Then each Vec<->VecDeque conversion harness weakened.
    for name, anchor in [('retain-harness-assumption', '        let result = owner.try_retain_mut(|_| closure_choice());'),
                         ('into-vec-harness-assumption', '    let values = owner.into_vec();'),
                         ('from-vec-harness-assumption', '    let owner = VecDeque::from_vec(source);')]:
        work = copy()
        mutate(work, 'src/deque/closure_proofs.rs', anchor, '    kani::assume(kani::any());\n' + anchor)
        r = report(work, out / name)
        results.append(dict(name=name, detected=not r['Vec']['closed'] and not r['VecDeque']['closed'],
                            blocking=r['Vec']['blocking'] + r['VecDeque']['blocking']))
    table = dict(SITES)
    table[RETAIN_SITE] = d('uncovered', 'control: the retain harness is not registered')
    r = report(root, out / 'retain-uncovered', table=table)
    results.append(dict(name='retain-uncovered', detected=not r['VecDeque']['closed'] and not r['Vec']['closed'],
                        blocking=r['VecDeque']['blocking'] + r['Vec']['blocking']))
    # Issue #16, unit 7: the BinaryHeap wrapper.
    r = report(root, out / 'heap-vec-open', table=table)
    results.append(dict(name='heap-inner-not-closed', detected=not r['BinaryHeap']['closed'] and r['Vec']['closed'] is False,
                        blocking=r['BinaryHeap']['blocking']))
    work = copy()
    mutate(work, 'src/collections/binary_heap/closure_proofs.rs', '    let touch: bool = kani::any();',
           '    kani::assume(kani::any());\n    let touch: bool = kani::any();')
    r = report(work, out / 'heap-assume')
    results.append(dict(name='heap-harness-assumption', detected=not r['BinaryHeap']['closed'] and r['Vec']['closed'],
                        blocking=r['BinaryHeap']['blocking']))
    for name, file, old, new, expected in [
            ('heap-unmapped-lend', 'src/collections/binary_heap.rs', '        self.data.clear()',
             '        (&mut self.data).clear()', 'wrapper site without an entry'),
            ('heap-second-field', 'src/collections/binary_heap.rs', '    data: Vec<T, P, L>,\n}',
             '    data: Vec<T, P, L>,\n    spare: usize,\n}', 'wrapper fields changed'),
            ('heap-field-write', 'src/collections/binary_heap.rs', '        self.data.clear()',
             '        self.data = Vec::new_in(unsafe { core::ptr::read(self.data.allocation_policy()) })', 'write of a wrapper word'),
            ('heap-helper-changed', 'src/collections/binary_heap/core.rs', '    let _ = unsafe { data.try_set_len(len) };',
             '    let _ = unsafe { data.try_set_len(len) };\n    data.clear();', 'wrapper helper changed')]:
        work = copy()
        mutate(work, file, old, new)
        try:
            report(work, out / name)
            results.append(dict(name=name, detected=False))
        except AssertionError as error:
            results.append(dict(name=name, detected=expected in str(error), error=str(error)[:300]))
    shutil.rmtree(out, ignore_errors=True)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verification/owner-closure')
    parser.add_argument('--controls', action='store_true', help="Also run the defect controls")
    args = parser.parse_args()
    result = report(ROOT, args.output)
    for owner, row in result.items():
        print(f"{owner}: closed={str(row['closed']).lower()} at the {row['instantiation']}; sites by disposition {row['counts']}"
              + (f"; harnesses assume the invariant of {', '.join(row['depends_on'])}" if row['depends_on'] else ''))
        for site in row['sites']:
            evidence = ', '.join(x['harness'].rsplit('::', 1)[1] for x in site.get('harnesses', [])) or site.get('reason', '')
            marks = ' [not executed at T = ()]' if site.get('dead') else ''
            print(f"  {site['disposition']:<16} {site['kind']:<11} {site['file']}:{site['line']} {site['function']} "
                  f"`{site['expression']}`#{site['occurrence']}: {evidence}{marks}")
        if row['sized_capacity_harnesses']:
            print(f"  sized capacity paths ({', '.join(row['sized_types'])}): "
                  + ', '.join(x.rsplit('::', 1)[1] for x in row['sized_capacity_harnesses']))
        for site in row['blocking']:
            print(f"  BLOCKING {site['file']}:{site['line']} {site['function']}: {site['disposition']} ({site['reason']})")
    if args.controls:
        results = controls(ROOT, args.output)
        for row in results:
            print(f"control {row['name']}: {'detected' if row['detected'] else 'NOT DETECTED'}")
        assert all(row['detected'] for row in results), 'a defect control was not detected'
    print(args.output / 'summary.json')


if __name__ == '__main__':
    main()
