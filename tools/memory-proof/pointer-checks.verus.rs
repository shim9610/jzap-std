use vstd::prelude::*;
use vstd::arithmetic::div_mod::*;
use vstd::arithmetic::mul::*;
use vstd::arithmetic::power2::*;
use vstd::bits::*;
verus! {
// Every predicate below is the pinned core text, proved true under the
// documented condition of its call (core-contracts.verus.rs). The allocation
// is a ghost argument: the core predicates read addresses and counts only.
fn pointer_add_precondition(Ghost(a): Ghost<Allocation>, base: usize, count: usize, size: usize) -> (r: bool)
    requires allocation_wf(a), doc_ptr_add(a, base, count, size),
    ensures r,
{
    POINTER_ADD_BODY
}

fn pointer_sub_precondition(Ghost(a): Ghost<Allocation>, base: usize, count: usize, size: usize) -> (r: bool)
    requires allocation_wf(a), doc_ptr_sub(a, base, count, size),
    ensures r,
{
    POINTER_SUB_BODY
}

fn slice_size_precondition(Ghost(a): Ghost<Allocation>, addr: usize, size: usize, len: usize, align: usize) -> (r: bool)
    requires doc_slice_from_raw_parts(a, addr, len, size, align),
    ensures r,
{
    proof {
        if size > 0 {
            lemma_div_is_ordered(len as int * size as int, isize::MAX as int, size as int);
            lemma_div_by_multiple(len as int, size as int);
        }
    }
    SLICE_SIZE_BODY
}

fn copy_nonoverlap_precondition(Ghost(source): Ghost<Allocation>,
        Ghost(destination): Ghost<Allocation>, src: usize,
        dst: usize, size: usize, count: usize, align: usize) -> (r: bool)
    requires allocation_wf(source), allocation_wf(destination),
        doc_copy_nonoverlapping(source, destination, src, dst, count, size, align),
    ensures r,
{
    COPY_NONOVERLAP_BODY
}

fn alignment_mask_precondition(base: usize, align: usize) -> (r: bool)
    requires is_pow2(align as int), aligned(base, align),
    ensures r,
{
    proof {
        is_pow2_equiv(align as int);
        reveal(is_pow2);
        let n = choose|n: nat| vstd::arithmetic::power::pow(2, n) == align as int;
        assert(pow2(n) == align as int);
        lemma2_to64();
        assert(pow2(usize::BITS as nat) == usize::MAX as nat + 1);
        if n >= usize::BITS {
            if n > usize::BITS { lemma_pow2_strictly_increases(usize::BITS as nat, n); }
            assert(false);
        }
        lemma_usize_low_bits_mask_is_mod(base, n);
        lemma_usize_low_bits_mask_is_mod(align, n);
        reveal(low_bits_mask);
        assert((low_bits_mask(n) as usize) == ((align - 1) as usize));
        assert(align as int % align as int == 0);
    }
    assert(align > 0);
    assert(align & ((align - 1) as usize) == 0);
    (base & (align - 1)) == 0
}

fn typed_pointer_precondition<T>(ptr: *mut T,
        Tracked(perm): Tracked<&vstd::raw_ptr::PointsTo<T>>) -> (r: bool)
    requires perm.ptr() == ptr,
    ensures r,
{
    vstd::layout::layout_for_type_is_valid::<T>();
    proof { perm.is_nonnull(); perm.is_aligned(); }
    let base = vstd::raw_ptr::cast_ptr_to_usize(ptr);
    let align = core::mem::align_of::<T>();
    let aligned = alignment_mask_precondition(base, align);
    base != 0 && aligned
}

// The concrete ptr::replace guard accepts ZST nulls too. A valid initialized
// typed slot has the stronger non-null/aligned contract, proved just above.
// Initialization and exclusive slot ownership are separate caller obligations.
fn replacement_precondition<T>(ptr: *mut T, is_zst: bool,
        Tracked(perm): Tracked<&vstd::raw_ptr::PointsTo<T>>) -> (r: bool)
    requires perm.ptr() == ptr,
    ensures r,
{
    let aligned = typed_pointer_precondition(ptr, Tracked(perm));
    proof { perm.is_nonnull(); }
    let base = vstd::raw_ptr::cast_ptr_to_usize(ptr);
    REPLACE_BODY
}

// The address form of the ptr::replace guard, under the documented condition.
fn replacement_address_precondition(Ghost(a): Ghost<Allocation>, base: usize, size: usize, align: usize,
        is_zst: bool) -> (r: bool)
    requires is_pow2(align as int), doc_replace(a, base, size, align),
    ensures r,
{
    let aligned = alignment_mask_precondition(base, align);
    REPLACE_BODY
}

fn aligned_array_layout(capacity: usize, size: usize, align: usize) -> (bytes: usize)
    requires is_pow2(align as int), 0 < align,
        size as int % align as int == 0,
        capacity as int * size as int <= isize::MAX as int,
    ensures bytes as int == capacity as int * size as int,
        vstd::layout::valid_layout(bytes, align),
        doc_layout_from_size_align(bytes, align),
{
    let bytes = capacity * size;
    proof {
        lemma_fundamental_div_mod(size as int, align as int);
        assert(size as int == align as int * (size as int / align as int));
        assert(bytes as int == align as int * (capacity as int * (size as int / align as int)))
            by(nonlinear_arith)
            requires size as int == align as int * (size as int / align as int),
                     bytes as int == capacity as int * size as int;
        lemma_mod_multiples_vanish(capacity as int * (size as int / align as int), 0, align as int);
        assert(bytes as int % align as int == 0);
        lemma_div_is_ordered(bytes as int, isize::MAX as int, align as int);
        lemma_fundamental_div_mod(bytes as int, align as int);
        lemma_fundamental_div_mod(isize::MAX as int, align as int);
        // State the two division facts the final step uses instead of leaving
        // them to automatic instantiation, which an unrelated caller added to
        // the generated module was enough to perturb.
        assert(bytes as int == (bytes as int / align as int) * align as int)
            by(nonlinear_arith)
            requires bytes as int == align as int * (bytes as int / align as int)
                         + bytes as int % align as int,
                bytes as int % align as int == 0;
        assert(isize::MAX as int == (isize::MAX as int / align as int) * align as int
                                     + isize::MAX as int % align as int)
            by(nonlinear_arith)
            requires isize::MAX as int == align as int * (isize::MAX as int / align as int)
                         + isize::MAX as int % align as int;
        assert(bytes as int <= isize::MAX as int - isize::MAX as int % align as int)
            by(nonlinear_arith)
            requires 0 < align as int,
                bytes as int == (bytes as int / align as int) * align as int,
                bytes as int / align as int <= isize::MAX as int / align as int,
                isize::MAX as int == (isize::MAX as int / align as int) * align as int
                                     + isize::MAX as int % align as int;
    }
    bytes
}

fn unchecked_index_precondition(this:usize,len:usize)->(accepted:bool)
    requires doc_get_unchecked(this,len),
    ensures accepted,
{ INDEX_BODY }

fn unchecked_index_range_precondition(start:usize,end:usize,len:usize)->(accepted:bool)
    requires doc_get_unchecked_range(start,end,len),
    ensures accepted,
{ INDEX_RANGE_BODY }
}
fn main() {}
