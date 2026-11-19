// Compose the actual array-count kernel with the documented typed-array ABI.
// No product function body is copied or translated here.
verus! {
proof fn flattened_array_storage(a:RawStorage,outer:usize,width:usize,count:usize,size:usize)
    requires raw_valid(a),outer<=a.cap,
        a.size as int==width as int*size as int,
        count as int==outer as int*width as int,
        size as int%a.align as int==0,
    ensures raw_valid(RawStorage{cap:count,size,..a}),
{
    assert(count as int*size as int==outer as int*a.size as int) by(nonlinear_arith)
        requires a.size as int==width as int*size as int,
            count as int==outer as int*width as int;
    assert(outer as int*a.size as int<=a.cap as int*a.size as int) by(nonlinear_arith)
        requires outer<=a.cap,0<=a.size;
}

// A valid borrow of [T; WIDTH] supplies the array layout and live extent.
// The original count admission supplies the only newly computed slice length.
fn array_slice_caller(a:RawStorage,b:RawStorage,source:usize,destination:usize,width:usize,size:usize)
    requires raw_valid(a),raw_valid(b),source<=a.cap,destination<=b.cap,
        a.size==b.size,a.align==b.align,
        a.size as int==width as int*size as int,size as int%a.align as int==0,
{
    let Ok(count)=crate::clone::geometry::flatten_count(source,destination,width) else{return;};
    proof {
        flattened_array_storage(a,source,width,count,size);
        flattened_array_storage(b,destination,width,count,size);
    }
    let source=RawStorage{cap:count,size,..a};
    let destination=RawStorage{cap:count,size,..b};
    let _=slice_caller(source,0,count);
    let _=slice_caller(destination,0,count);
}
}
