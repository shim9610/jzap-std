// Documented safety conditions of the pinned Rust 1.95 core calls, restricted
// to what can be stated about integer addresses and counts. Each predicate
// quotes its source in library/core. They are the interface between the two
// halves of the guard argument: the storage model establishes them at every
// caller (raw-owner.verus.rs), and the pinned core predicates are proved true
// under them (pointer-checks.verus.rs). Provenance, liveness, initialization
// and aliasing are not address facts; the Kani and Miri checks cover those.
// The file has no imports: verify_pointer_checks.py places it after the
// imports of pointer-checks.verus.rs.
verus! {

// ptr/mod.rs, "Allocation": "`base` is not equal to null()", "`base + size <=
// usize::MAX`", "`size <= isize::MAX`".
struct Allocation { base: usize, bytes: usize }

spec fn allocation_wf(a: Allocation) -> bool {
    a.base != 0
    && a.base as int + a.bytes as int <= usize::MAX as int
    && a.bytes as int <= isize::MAX as int
}

// "the memory range of the given size starting at the pointer is entirely
// contained within the bounds of that allocation". One-past-the-end is a
// permitted start for a range of zero bytes.
spec fn in_bounds(a: Allocation, addr: usize, bytes: int) -> bool {
    0 <= bytes
    && a.base as int <= addr as int
    && addr as int + bytes <= a.base as int + a.bytes as int
}

// ptr/mod.rs, "Safety": "A null pointer is *never* valid" and "a pointer is
// dereferenceable if the memory range ... is entirely contained within the
// bounds of that allocation". STRICTER THAN THE TEXT: the documented exemption
// "For memory accesses of size zero, *every* pointer is valid, including the
// null pointer" is not used; a zero-byte access is also non-null and in bounds.
spec fn valid_access(a: Allocation, addr: usize, bytes: int) -> bool {
    addr != 0 && in_bounds(a, addr, bytes)
}

spec fn aligned(addr: usize, align: usize) -> bool {
    align != 0 && addr as int % align as int == 0
}

// ptr/docs/add.md: "The offset in bytes, `count * size_of::<T>()`, computed on
// mathematical integers, must fit in an `isize`." / "If the computed offset is
// non-zero, then ... the entire memory range between `self` and the result
// must be in bounds of that allocation." STRICTER THAN THE TEXT: the range is
// in bounds for a zero offset as well.
spec fn doc_ptr_add(a: Allocation, addr: usize, count: usize, size: usize) -> bool {
    let offset = count as int * size as int;
    offset <= isize::MAX as int
    && in_bounds(a, addr, offset)
}

// ptr/mut_ptr.rs and ptr/const_ptr.rs sub: "The offset in bytes, `count *
// size_of::<T>()`, computed on mathematical integers, must fit in an `isize`."
// / "If the computed offset is non-zero, then ... the entire memory range
// between `self` and the result must be in bounds of that allocation. In
// particular, this range must not "wrap around" the edge of the address
// space." STRICTER THAN THE TEXT: the range is in bounds for a zero offset as
// well, as for `doc_ptr_add`.
spec fn doc_ptr_sub(a: Allocation, addr: usize, count: usize, size: usize) -> bool {
    let offset = count as int * size as int;
    offset <= isize::MAX as int
    && offset <= addr as int
    && in_bounds(a, (addr as int - offset) as usize, offset)
}

// slice/raw.rs from_raw_parts(_mut): "`data` must be non-null, valid for reads
// for `len * size_of::<T>()` many bytes, and it must be properly aligned" (also
// "for zero-length slices or slices of ZSTs"); "The total size ... must be no
// larger than `isize::MAX`, and adding that size to `data` must not wrap".
spec fn doc_slice_from_raw_parts(a: Allocation, addr: usize, len: usize,
        size: usize, align: usize) -> bool {
    let bytes = len as int * size as int;
    addr != 0
    && aligned(addr, align)
    && bytes <= isize::MAX as int
    && addr as int + bytes <= usize::MAX as int
    && valid_access(a, addr, bytes)
}

// ptr/mod.rs copy: "`src` must be valid for reads of `count * size_of::<T>()`
// bytes", "`dst` must be valid for writes of [the same]", "Both `src` and `dst`
// must be properly aligned" ("even if the effectively copied size is 0").
spec fn doc_copy(src_alloc: Allocation, dst_alloc: Allocation, src: usize,
        dst: usize, count: usize, size: usize, align: usize) -> bool {
    let bytes = count as int * size as int;
    valid_access(src_alloc, src, bytes)
    && valid_access(dst_alloc, dst, bytes)
    && aligned(src, align)
    && aligned(dst, align)
}

// ptr/mod.rs copy_nonoverlapping: the conditions of `copy`, and "The region of
// memory beginning at `src` with a size of `count * size_of::<T>()` bytes must
// *not* overlap with the region of memory beginning at `dst` with the same
// size." For zero bytes the ordering below holds for every pair of addresses.
spec fn doc_copy_nonoverlapping(src_alloc: Allocation, dst_alloc: Allocation,
        src: usize, dst: usize, count: usize, size: usize, align: usize) -> bool {
    let bytes = count as int * size as int;
    doc_copy(src_alloc, dst_alloc, src, dst, count, size, align)
    && (src as int + bytes <= dst as int || dst as int + bytes <= src as int)
}

// ptr/mod.rs replace: "`dst` must be valid for both reads and writes", "`dst`
// must be properly aligned" ("even if `T` has size `0`"). "must point to a
// properly initialized value" is not an address fact.
spec fn doc_replace(a: Allocation, dst: usize, size: usize, align: usize) -> bool {
    valid_access(a, dst, size as int) && aligned(dst, align)
}

// ptr/mod.rs write_bytes: "`dst` must be valid for writes of `count *
// size_of::<T>()` bytes", "`dst` must be properly aligned".
spec fn doc_write_bytes(a: Allocation, dst: usize, count: usize, size: usize,
        align: usize) -> bool {
    valid_access(a, dst, count as int * size as int) && aligned(dst, align)
}

// slice/mod.rs get_unchecked(_mut): "Calling this method with an out-of-bounds
// index is undefined behavior"; "It's UB to call `.get_unchecked(len)`" and
// "`.get_unchecked(..len + 1)`".
spec fn doc_get_unchecked(index: usize, len: usize) -> bool {
    index < len
}

spec fn doc_get_unchecked_range(start: usize, end: usize, len: usize) -> bool {
    start <= end && end <= len
}

// ptr/non_null.rs new_unchecked: "`ptr` must be non-null."
spec fn doc_non_null_new_unchecked(addr: usize) -> bool {
    addr != 0
}

// ptr/alignment.rs new_unchecked: "`align` must be a power of two. ... It must
// *not* be zero."
spec fn doc_alignment_new_unchecked(align: usize) -> bool {
    align != 0 && is_pow2(align as int)
}

// alloc/layout.rs from_size_align(_unchecked): "`align` must not be zero",
// "`align` must be a power of two", "`size`, when rounded up to the nearest
// multiple of `align`, must not overflow `isize`".
spec fn doc_layout_from_size_align(size: usize, align: usize) -> bool {
    align != 0
    && is_pow2(align as int)
    && rounded_up(size, align) <= isize::MAX as int
}

// The least multiple of `align` that is not below `size`.
spec fn rounded_up(size: usize, align: usize) -> int {
    let remainder = size as int % align as int;
    if remainder == 0 { size as int } else { size as int + align as int - remainder }
}

// num/uint_macros.rs unchecked_add: "undefined behavior when `self + rhs >
// MAX`". Stated for usize; the other widths use their own MAX.
spec fn doc_unchecked_add_usize(lhs: usize, rhs: usize) -> bool {
    lhs as int + rhs as int <= usize::MAX as int
}

// num/uint_macros.rs unchecked_shl: "undefined behavior if `rhs` is larger
// than or equal to the number of bits in `self`".
spec fn doc_unchecked_shl(rhs: u32, bits: u32) -> bool {
    rhs < bits
}

}
