// Table callers composed with the product crate, then the retained
// mathematical ownership/credit lemmas. No Python-extracted bodies here.
use vstd::prelude::*;
use crate::arithmetic::bounded::{wrapping_add as add, wrapping_mul as mul, wrapping_sub as sub};
use crate::collections::hash_map::{geometry, group, raw};
verus! {
// Shape of a live, nonempty table: a power-of-two bucket count of at least
// four whose control span was admitted by `checked_sizes`.
pub open spec fn table_shape(mask: usize) -> bool {
    mask >= 3 && ((mask + 1) as usize) & mask == 0 && mask < usize::MAX / 2 - 16
}
// The value `raw::capacity_for` is verified to return.
pub open spec fn capacity_of(mask: usize) -> int {
    if mask < 8 { mask as int } else { (mask as int + 1) / 8 * 7 }
}
pub open spec fn layout_align(alignment: usize) -> bool {
    alignment > 0 && alignment & ((alignment - 1) as usize) == 0
}

// The two arrays of a live table as storages of the storage model
// (storage.verus.rs, placed in front of this file by storage_kernel.py): the
// reverse slots, which end at the control pointer, and the control bytes
// including the mirrored group. Each index function is proved for every such
// table, so the callers below pass only the index the product kernels
// computed. Stated premise: a live table of this mask is a `hash_table`.
// A slot read, write or drop is stated as a one-element range. The byte
// allocation that holds both arrays is covered by the layout facts of
// `hash_allocation_caller`, not by these functions.
struct HashTable { slots: RawStorage, controls: RawStorage }
spec fn hash_table(t: HashTable, mask: usize) -> bool {
    raw_valid(t.slots) && t.slots.cap == mask + 1
    && raw_valid(t.controls) && t.controls.cap == mask + 17 && t.controls.size == 1
}
// RawTable::slot: `ctrl.cast::<(K, V)>().sub(index + 1)` and the access.
spec fn hash_slot(t: HashTable, mask: usize, index: usize) -> bool {
    doc_ptr_sub(allocation_of(t.slots), slot_address(t.slots, (mask + 1) as usize),
        (index + 1) as usize, t.slots.size)
    && storage_range(t.slots, (mask - index) as usize, 1)
}
fn hash_slot_index(mask: usize, index: usize)
    requires table_shape(mask), index <= mask,
    ensures forall|t: HashTable| hash_table(t, mask) ==> #[trigger] hash_slot(t, mask, index),
{
    proof {
        assert forall|t: HashTable| hash_table(t, mask) implies #[trigger] hash_slot(t, mask, index) by {
            storage_sub_condition(t.slots, (mask + 1) as usize, (index + 1) as usize);
            storage_range_condition(t.slots, (mask - index) as usize, 1);
        }
    }
}
// RawTable::control and set_control: `ctrl.add(index)` and one byte.
fn hash_control_index(mask: usize, index: usize)
    requires table_shape(mask), index < mask + 17,
    ensures forall|t: HashTable| hash_table(t, mask) ==> #[trigger] storage_range(t.controls, index, 1),
{
    proof {
        assert forall|t: HashTable| hash_table(t, mask) implies #[trigger] storage_range(t.controls, index, 1) by {
            storage_range_condition(t.controls, index, 1);
        }
    }
}
// group::matching and group::vacant at `ctrl.add(pos)`: sixteen bytes.
fn hash_group_index(mask: usize, pos: usize)
    requires table_shape(mask), pos <= mask,
    ensures forall|t: HashTable| hash_table(t, mask) ==> #[trigger] storage_range(t.controls, pos, 16),
{
    proof {
        assert forall|t: HashTable| hash_table(t, mask) implies #[trigger] storage_range(t.controls, pos, 16) by {
            storage_range_condition(t.controls, pos, 16);
        }
    }
}
// `write_bytes(ctrl, EMPTY, buckets + WIDTH)` and the whole control span copy.
fn hash_controls_index(mask: usize) -> (controls: usize)
    requires table_shape(mask),
    ensures controls == mask + 17,
        forall|t: HashTable| hash_table(t, mask) ==> #[trigger] storage_range(t.controls, 0, controls),
{
    let controls = add(add(mask, 1), group::WIDTH);
    proof {
        assert forall|t: HashTable| hash_table(t, mask) implies #[trigger] storage_range(t.controls, 0, controls) by {
            storage_range_condition(t.controls, 0, controls);
        }
    }
    controls
}

// RawTable::allocate: admitted sizes, the filled control span, mask and growth.
fn hash_allocate_caller(size: usize, alignment: usize, buckets: usize)
    -> (out: Option<(usize, usize, usize, usize, usize)>)
    requires layout_align(alignment), buckets >= 4, buckets & ((buckets - 1) as usize) == 0,
    ensures match out {
        Some((bytes, offset, controls, mask, growth)) => table_shape(mask)
            && mask + 1 == buckets && controls == buckets + 16
            && offset + controls == bytes && bytes <= isize::MAX
            && (size as int) * (buckets as int) <= offset && growth <= mask,
        None => true,
    },
{
    match geometry::checked_sizes(size, alignment, buckets) {
        Ok((bytes, offset)) => {
            let controls = add(buckets, group::WIDTH);
            let mask = sub(buckets, 1);
            let growth = raw::capacity_for(mask);
            let _filled = hash_controls_index(mask);
            Some((bytes, offset, controls, mask, growth))
        }
        Err(_) => None,
    }
}

// RawTable::buckets for a nonempty table.
fn hash_buckets(mask: usize) -> (buckets: usize)
    requires mask == 0 || table_shape(mask),
    ensures buckets == if mask == 0 { 0int } else { mask + 1 },
{
    if mask == 0 { 0 } else { add(mask, 1) }
}

// RawTable::allocation: the reconstructed layout equals the admitted one.
fn hash_allocation_caller(size: usize, alignment: usize, mask: usize)
    requires layout_align(alignment), table_shape(mask),
{
    let buckets = hash_buckets(mask);
    // A live table was admitted with exactly this shape.
    let Ok((bytes, offset)) = geometry::checked_sizes(size, alignment, buckets) else { return; };
    let align = if alignment < group::WIDTH { group::WIDTH } else { alignment };
    let round = sub(align, 1);
    assert((buckets as int) * (size as int) == (size as int) * (buckets as int)) by(nonlinear_arith);
    assert(0 <= (size as int) * (buckets as int)) by(nonlinear_arith);
    let rebuilt = add(mul(buckets, size), round) & !round;
    let total = add(add(rebuilt, buckets), group::WIDTH);
    assert(rebuilt == offset && total == bytes);
    // ctrl - offset is the allocation start; [start, start + bytes) is the extent.
    assert(offset <= bytes);
}

// RawTable::slot: the reverse slot lies before the control pointer.
fn hash_slot_caller(size: usize, alignment: usize, mask: usize, index: usize)
    requires layout_align(alignment), table_shape(mask), index <= mask,
{
    let buckets = hash_buckets(mask);
    let Ok((bytes, offset)) = geometry::checked_sizes(size, alignment, buckets) else { return; };
    let back = add(index, 1);
    assert((back as int) * (size as int) <= (size as int) * (buckets as int)) by(nonlinear_arith)
        requires back <= buckets, size >= 0;
    assert((back as int) * (size as int) <= offset);
    hash_slot_index(mask, index);
}

// RawTable::set_control: both writes stay inside buckets + WIDTH controls.
fn hash_set_control_caller(mask: usize, index: usize)
    requires table_shape(mask), index <= mask,
{
    let mirror = geometry::mirror(index, add(mask, 1));
    assert(index < mask + 1 + 16 && mirror < mask + 1 + 16);
    assert(mirror >= 16);
    hash_control_index(mask, index);
    hash_control_index(mask, mirror);
}

// RawTable::groups.
fn hash_groups(mask: usize) -> (groups: usize)
    requires table_shape(mask),
    ensures groups >= 1, (groups - 1) * 16 <= mask,
{
    if mask < group::WIDTH { 1 } else { add(mask / group::WIDTH, 1) }
}

// RawTable::find and RawTable::vacant: bounded triangular probe.
fn hash_probe_caller(mask: usize, hash: u64, matches: u16, first_group: u16)
    requires table_shape(mask),
{
    // RawTable::vacant falls back to the aligned first group for small tables.
    let fallback = group::first(first_group);
    if fallback < hash_buckets(mask) {
        assert(fallback <= mask);
    }
    let mut pos = (hash as usize) & mask;
    assert(pos <= mask) by(bit_vector) requires pos == (hash as usize) & mask;
    let mut stride = 0usize;
    let mut groups = hash_groups(mask);
    let ghost total = groups;
    while groups > 0
        invariant table_shape(mask), pos <= mask, total >= 1, (total - 1) * 16 <= mask,
            groups <= total, groups > 0 ==> stride == 16 * (total - groups),
        decreases groups,
    {
        // One group load at pos: sixteen controls inside buckets + WIDTH.
        assert(pos + 16 <= mask + 1 + 16);
        hash_group_index(mask, pos);
        let mut bits = matches;
        while bits != 0
            invariant pos <= mask, table_shape(mask),
            decreases bits,
        {
            let sum = add(pos, group::first(bits));
            let index = sum & mask;
            assert(index <= mask) by(bit_vector) requires index == sum & mask;
            hash_slot_index(mask, index);
            bits = group::clear_first(bits);
        }
        groups = sub(groups, 1);
        if groups != 0 {
            let next = geometry::probe_next(pos, stride, mask);
            pos = next.0;
            stride = next.1;
        }
    }
}

// RawTable::find_insert: the stride check bounds the same probe.
fn hash_find_insert_caller(mask: usize, hash: u64, matches: u16, vacant: u16, first_group: u16)
    requires table_shape(mask),
{
    let mut pos = (hash as usize) & mask;
    assert(pos <= mask) by(bit_vector) requires pos == (hash as usize) & mask;
    let mut stride = 0usize;
    loop
        invariant table_shape(mask), pos <= mask, stride <= mask,
        decreases mask - stride,
    {
        assert(pos + 16 <= mask + 1 + 16);
        hash_group_index(mask, pos);
        let mut bits = matches;
        while bits != 0
            invariant pos <= mask, table_shape(mask),
            decreases bits,
        {
            let sum = add(pos, group::first(bits));
            let index = sum & mask;
            assert(index <= mask) by(bit_vector) requires index == sum & mask;
            hash_slot_index(mask, index);
            bits = group::clear_first(bits);
        }
        if vacant != 0 {
            let sum = add(pos, group::first(vacant));
            let index = sum & mask;
            assert(index <= mask) by(bit_vector) requires index == sum & mask;
            // Otherwise the aligned first group is read at offset zero.
            let index = group::first(first_group);
            if index < hash_buckets(mask) {
                assert(index <= mask);
            }
        }
        let next = geometry::probe_next(pos, stride, mask);
        pos = next.0;
        stride = next.1;
        if stride > mask {
            return;
        }
    }
}

// RawTable::insert_at: growth and length under the credit invariant.
fn hash_insert_at_caller(mask: usize, len: usize, growth: usize, deleted: Ghost<int>, empty: bool, hash: u64)
    -> (out: (usize, usize))
    requires table_shape(mask), deleted@ >= 0,
        len + growth + deleted@ == capacity_of(mask),
        empty ==> growth > 0, !empty ==> deleted@ > 0,
    ensures out.0 == len + 1, out.0 <= mask,
        empty ==> out.0 + out.1 + deleted@ == capacity_of(mask),
        !empty ==> out.0 + out.1 + (deleted@ - 1) == capacity_of(mask),
{
    let capacity = raw::capacity_for(mask);
    let growth = if empty { sub(growth, 1) } else { growth };
    // A FULL tag never has the vacant bit.
    let tag = raw::tag(hash);
    assert(tag & 0x80 == 0) by(bit_vector) requires tag < 0x80;
    (add(len, 1), growth)
}

// RawTable::erase: the two group loads and the published credit.
fn hash_erase_caller(mask: usize, index: usize, len: usize, growth: usize, before_empty: u16, after_empty: u16)
    -> (out: (usize, usize))
    requires table_shape(mask), index <= mask, len >= 1, len + growth <= mask,
    ensures out.0 == len - 1, out.0 + out.1 <= mask,
{
    let before = sub(geometry::mirror(index, add(mask, 1)), group::WIDTH);
    assert(before + 16 <= mask + 1 + 16 && index + 16 <= mask + 1 + 16);
    hash_group_index(mask, before);
    hash_group_index(mask, index);
    hash_slot_index(mask, index);
    let a = group::first(before_empty);
    let b = group::first(after_empty);
    let growth = if add(a, b) >= group::WIDTH { growth } else { add(growth, 1) };
    (sub(len, 1), growth)
}

// RawTable::clear without destructors and the end of Clear::run.
fn hash_reset_caller(size: usize, alignment: usize, mask: usize) -> (growth: usize)
    requires layout_align(alignment), table_shape(mask),
    ensures growth <= mask,
{
    let buckets = hash_buckets(mask);
    let Ok((bytes, offset)) = geometry::checked_sizes(size, alignment, buckets) else { return 0; };
    let controls = add(buckets, group::WIDTH);
    assert(offset + controls == bytes);
    let _filled = hash_controls_index(mask);
    raw::capacity_for(mask)
}

// Clear::run: whole-group retirement for tables of at least one group, then
// the per-bucket tail. Tables smaller than a group rely on EMPTY padding.
fn hash_clear_run_caller(mask: usize, full: u16)
    requires table_shape(mask), mask >= 15,
{
    let buckets = hash_buckets(mask);
    assert(buckets % 16 == 0) by(bit_vector)
        requires buckets >= 16, buckets & ((buckets - 1) as usize) == 0;
    let mut index = 0usize;
    while index < buckets
        invariant index % 16 == 0, index <= buckets, buckets % 16 == 0, buckets == mask + 1,
            table_shape(mask),
        decreases buckets - index,
    {
        let base = index;
        assert(base + 16 <= buckets);
        hash_group_index(mask, base);
        let mut bits = full;
        while bits != 0
            invariant base + 16 <= buckets, buckets == mask + 1, table_shape(mask),
            decreases bits,
        {
            let slot = add(base, group::first(bits));
            bits = group::clear_first(bits);
            assert(slot <= mask);
            hash_slot_index(mask, slot);
        }
        index = add(base, group::WIDTH);
    }
    while index < buckets
        invariant index <= buckets, buckets == mask + 1, table_shape(mask),
        decreases buckets - index,
    {
        let slot = index;
        index = add(slot, 1);
        assert(slot <= mask);
        hash_control_index(mask, slot);
        hash_slot_index(mask, slot);
    }
}

// Scan::fill_group and Scan::take_cached: every group load lies inside the
// control span; for tables of at least one group every yielded index is a
// bucket. Smaller tables rely on EMPTY padding, as in Clear::run.
fn hash_scan_caller(mask: usize, full: u16)
    requires table_shape(mask),
{
    let buckets = add(mask, 1);
    assert(buckets >= 16 ==> buckets % 16 == 0) by(bit_vector)
        requires buckets & ((buckets - 1) as usize) == 0;
    let mut group = 0usize;
    let mut bits = 0u16;
    loop
        invariant table_shape(mask), group % 16 == 0, group <= mask + 16,
            mask >= 15 ==> group <= mask + 1 && (mask + 1) % 16 == 0,
            bits != 0 ==> group >= 16,
        decreases mask + 16 - group, bits,
    {
        let ghost entered = (group, bits);
        while bits == 0
            invariant table_shape(mask), group % 16 == 0, group <= mask + 16,
                mask >= 15 ==> group <= mask + 1 && (mask + 1) % 16 == 0,
                bits != 0 ==> group >= 16,
                group >= entered.0, group == entered.0 ==> bits == entered.1,
            decreases mask + 16 - group,
        {
            if group > mask {
                return;
            }
            assert(group + 16 <= mask + 1 + 16);
            hash_group_index(mask, group);
            bits = full;
            group = add(group, group::WIDTH);
        }
        let index = add(sub(group, group::WIDTH), group::first(bits));
        bits = group::clear_first(bits);
        if mask >= 15 {
            assert(index <= mask);
            hash_slot_index(mask, index);
        }
    }
}

// IntoIter/Drain next and fold, Clear::run: one yielded slot is retired.
// That the slot is counted in `len` is the frontier premise.
fn hash_retire_caller(mask: usize, index: usize, len: usize) -> (out: usize)
    requires table_shape(mask), index <= mask, len >= 1,
    ensures out == len - 1,
{
    hash_slot_index(mask, index);
    hash_control_index(mask, index);
    let mirror = geometry::mirror(index, add(mask, 1));
    hash_control_index(mask, mirror);
    assert(index < mask + 1 + 16 && mirror < mask + 1 + 16);
    sub(len, 1)
}

// HashMap::resize: one copied slot and the published vacancy credit.
fn hash_resize_caller(source: usize, destination: usize, index: usize, target: usize, len: usize, hash: u64)
    -> (out: Option<usize>)
    requires table_shape(source), table_shape(destination), index <= source, target <= destination,
    ensures match out {
        Some(growth) => len + growth == capacity_of(destination) && len + growth <= destination,
        None => true,
    },
{
    let mirror = geometry::mirror(target, add(destination, 1));
    assert(target < destination + 1 + 16 && mirror < destination + 1 + 16);
    hash_slot_index(source, index);
    hash_slot_index(destination, target);
    hash_control_index(destination, target);
    hash_control_index(destination, mirror);
    let tag = raw::tag(hash);
    assert(tag & 0x80 == 0) by(bit_vector) requires tag < 0x80;
    raw::capacity_for(destination).checked_sub(len)
}

// HashMap::try_clone without Copy capabilities: per-bucket publication, the
// vacancy credit and the whole control span copy between equal shapes.
fn hash_clone_caller(size: usize, alignment: usize, mask: usize, full: bool)
    requires layout_align(alignment), table_shape(mask),
{
    let buckets = hash_buckets(mask);
    let Ok((bytes, offset)) = geometry::checked_sizes(size, alignment, buckets) else { return; };
    let mut len = 0usize;
    let mut index = 0usize;
    while index < buckets
        invariant index <= buckets, len <= index, buckets == mask + 1, table_shape(mask),
        decreases buckets - index,
    {
        if full {
            let mirror = geometry::mirror(index, add(mask, 1));
            assert(index <= mask && mirror < mask + 1 + 16);
            hash_slot_index(mask, index);
            hash_control_index(mask, index);
            hash_control_index(mask, mirror);
            len = add(len, 1);
        }
        index = add(index, 1);
    }
    let capacity = raw::capacity_for(mask);
    // The cloned FULL slots are the source's, whose count is within capacity.
    if len > capacity {
        return;
    }
    let growth = sub(capacity, len);
    let controls = add(buckets, group::WIDTH);
    assert(offset + controls == bytes);
    let _copied = hash_controls_index(mask);
}

// End of RawTable::rehash and of a pending Clear::run: remaining credit.
fn hash_growth_caller(mask: usize, len: usize) -> (growth: usize)
    requires table_shape(mask), len <= capacity_of(mask),
    ensures len + growth == capacity_of(mask),
{
    sub(raw::capacity_for(mask), len)
}

// RawTable::rehash: probe-distance classes of the source and target slots.
fn hash_rehash_distance_caller(mask: usize, index: usize, target: usize, value: u64)
    requires table_shape(mask), index <= mask, target <= mask,
{
    let buckets = hash_buckets(mask);
    let start = (value as usize) & mask;
    assert(start <= mask) by(bit_vector) requires start == (value as usize) & mask;
    let a = sub(add(index, buckets), start) & mask;
    let b = sub(add(target, buckets), start) & mask;
    let same = a / group::WIDTH == b / group::WIDTH;
}
}
verus! {
proof fn hash_layout_ranges(data:nat,offset:nat,buckets:nat,width:nat,index:nat,size:nat)
 requires data == buckets * size, data <= offset, width == 16, index < buckets,
          offset + buckets + width <= isize::MAX as nat,
 ensures (index + 1) * size <= offset,
         index + width < buckets + width,
         offset + buckets + width <= usize::MAX as nat,
{ assert((index + 1) * size <= buckets * size) by(nonlinear_arith) requires index < buckets, size >= 0; }
proof fn hash_insert_credit(len:nat,growth:nat,deleted:nat,capacity:nat,empty:bool)
 requires len+growth+deleted==capacity,
          empty ==> growth>0, !empty ==> deleted>0,
 ensures len+1<=capacity,
         empty ==> len+1+(growth-1)+deleted==capacity,
         !empty ==> len+1+growth+(deleted-1)==capacity,
{}
proof fn hash_retire_progress(len:nat,full:bool,index:nat,buckets:nat)
 requires index<buckets, full ==> len>0,
 ensures index+1<=buckets, full ==> len-1<len,
{}
proof fn hash_probe_progress(groups:nat,pos:nat,stride:nat,buckets:nat)
 requires buckets>=4, buckets <= (isize::MAX as nat)-16,
          pos<buckets, stride<buckets, groups>1,
 ensures groups-1<groups, stride+16<=buckets+15,
         pos+stride+16<=usize::MAX as nat,
{}
proof fn hash_rehash_pending(pending:nat,len:nat,full:nat,move_to_empty:bool)
 requires pending>0, pending+full==len,
 ensures pending-1+full+1==len, pending-1<pending,
{}
proof fn hash_physical_duplicates(indices:Seq<Option<nat>>, i:int)
 requires 0<=i<=indices.len(),
          forall|a:int,b:int| 0<=a<b<i && indices[a].is_some() ==> indices[a]!=indices[b],
          i<indices.len() ==> forall|a:int| 0<=a<i && indices[a].is_some() ==> indices[a]!=indices[i],
 ensures i<indices.len() ==> forall|a:int,b:int| 0<=a<b<i+1 && indices[a].is_some() ==> indices[a]!=indices[b],
{}
proof fn hash_clone_publication(initialized:Set<int>,index:int)
 requires !initialized.contains(index),
 ensures initialized.insert(index).contains(index),
         forall|i:int| i!=index ==> initialized.insert(index).contains(i)==initialized.contains(i),
{}
proof fn hash_mirror_padding(buckets:nat,index:nat,mirror:nat)
 requires buckets>=4,index<buckets,16<=mirror<buckets+16,
 ensures index<buckets+16,mirror<buckets+16,
{}
proof fn hash_source_error_prefix(len:nat,accepted:nat)
 ensures len<=len+accepted,
{}
proof fn hash_core_u32_shift(rhs:u32)
 ensures rhs & 31 < 32,
{ assert(rhs & 31 < 32) by(bit_vector); }
proof fn hash_core_u64_shift(rhs:u32)
 ensures rhs & 63 < 64,
{ assert(rhs & 63 < 64) by(bit_vector); }
proof fn hash_core_split_chunk(len:nat,n:nat)
 requires !(len<n),
 ensures n<=len,
{}
}
