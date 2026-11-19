// Element spans as byte extents. Shared by the Vec memory model, the storage
// model (storage.verus.rs) and every area composition that uses it.
verus! {
spec fn byte_valid(cap: usize, size: usize) -> bool {
    cap as int * size as int <= isize::MAX as int
}
proof fn memory_span(cap: usize, size: usize, start: int, count: int)
    requires byte_valid(cap, size), 0 <= start, 0 <= count, start + count <= cap as int,
    ensures start <= usize::MAX, count <= usize::MAX,
        start as int * size as int <= isize::MAX as int,
        count as int * size as int <= isize::MAX as int,
        (start as int + count as int) * size as int <= cap as int * size as int,
        (start as int + count as int) * size as int <= isize::MAX as int,
{
    assert(start <= cap as int && count <= cap as int);
    assert(start as int * size as int <= cap as int * size as int) by(nonlinear_arith)
        requires start <= cap as int, 0 <= size as int;
    assert(count as int * size as int <= cap as int * size as int) by(nonlinear_arith)
        requires count <= cap as int, 0 <= size as int;
    assert((start as int + count as int) * size as int <= cap as int * size as int) by(nonlinear_arith)
        requires start + count <= cap as int, 0 <= size as int;
}
proof fn ordered_elements_do_not_overlap(size: usize, left: usize, count: usize, right: usize)
    requires left as int + count as int <= right as int,
    ensures (left as int + count as int) * size as int <= right as int * size as int,
{
    assert((left as int + count as int) * size as int <= right as int * size as int) by(nonlinear_arith)
        requires left as int + count as int <= right as int, 0 <= size as int;
}
}
