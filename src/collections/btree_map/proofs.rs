//! Actual-source finite node kernels; all-word numeric projections are separate.
use super::*;

#[kani::proof]
#[kani::unwind(14)]
fn set_replacement_every_initialized_slot() {
    #[derive(Eq, PartialEq)]
    struct Key {
        id: u8,
        tag: u8,
    }
    impl PartialOrd for Key {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> Ordering {
            self.id.cmp(&other.id)
        }
    }
    let count: usize = kani::any();
    let index: usize = kani::any();
    let old_tag: u8 = kani::any();
    let new_tag: u8 = kani::any();
    kani::assume(0 < count && count <= CAP && index < count);
    if let Ok(n) = node::allocate::<Key, (), _>(0, &Unmetered) {
        for i in 0..count {
            // SAFETY: fresh owned leaf with a bounded initialized prefix.
            unsafe {
                node::insert(
                    n,
                    i,
                    Key {
                        id: i as u8,
                        tag: old_tag,
                    },
                    (),
                    None,
                )
            };
        }
        let replacement = Key {
            id: index as u8,
            tag: new_tag,
        };
        // SAFETY: the arbitrary slot is inside the initialized prefix above.
        let old = unsafe { node::replace_key(n, index, replacement) };
        assert_eq!(old.id, index as u8);
        assert_eq!(old.tag, old_tag);
        assert_eq!(unsafe { node::len(n) }, count);
        for i in 0..count {
            // SAFETY: replacement preserves all initialized slots and links.
            let stored = unsafe { &*node::key(n, i) };
            assert_eq!(stored.id, i as u8);
            assert_eq!(stored.tag, if i == index { new_tag } else { old_tag });
        }
        // SAFETY: all initialized keys are plain bytes and this leaf is owned.
        unsafe { node::free(n, 0, &Unmetered) };
    }
}

#[kani::proof]
#[kani::unwind(14)]
fn leaf_insert_remove_every_slot() {
    let count: usize = kani::any();
    let index: usize = kani::any();
    kani::assume(count < CAP && index <= count);
    if let Ok(n) = node::allocate::<u8, u8, _>(0, &Unmetered) {
        // SAFETY: proof constructs precisely the initialized prefix it tracks.
        unsafe {
            for i in 0..count {
                node::insert(n, i, i as u8, i as u8, None);
            }
            node::insert(n, index, 91, 92, None);
            assert_eq!(node::len(n), count + 1);
            assert_eq!((*node::key(n, index), *node::val(n, index)), (91, 92));
            assert_eq!(node::remove_leaf(n, index), (91, 92));
            assert_eq!(node::len(n), count);
            for i in 0..count {
                assert_eq!((*node::key(n, i), *node::val(n, i)), (i as u8, i as u8));
            }
            node::free(n, 0, &Unmetered);
        }
    }
}
#[kani::proof]
#[kani::unwind(14)]
fn split_every_incoming_edge() {
    let edge: usize = kani::any();
    kani::assume(edge <= CAP);
    if let Ok(left) = node::allocate::<u8, u8, _>(0, &Unmetered) {
        if let Ok(right) = node::allocate(0, &Unmetered) {
            // SAFETY: exactly eleven source pairs and an empty matching right node.
            unsafe {
                for i in 0..CAP {
                    node::insert(left, i, i as u8, i as u8, None);
                }
                let mid = node::splitpoint(edge);
                let pair = node::split(left, mid, right, 0);
                assert_eq!(pair, (mid as u8, mid as u8));
                if edge <= mid {
                    node::insert(left, edge, 99, 99, None);
                } else {
                    node::insert(right, sub(edge, add(mid, 1)), 99, 99, None);
                }
                assert!(node::len(left) >= MIN && node::len(left) <= CAP);
                assert!(node::len(right) >= MIN && node::len(right) <= CAP);
                assert_eq!(node::len(left) + node::len(right) + 1, 12);
                node::free(right, 0, &Unmetered);
                node::free(left, 0, &Unmetered);
            }
        } else {
            // SAFETY: empty unique leaf reservation.
            unsafe { node::free(left, 0, &Unmetered) };
        }
    }
}
#[kani::proof]
#[kani::unwind(5)]
fn external_view_arbitrary_shape_never_indexes_unchecked() {
    let n: usize = kani::any();
    let h: usize = kani::any();
    let key: u8 = kani::any();
    let keys = [3u8, 7, 19];
    let vals = [9u8, 21, 57];
    let edges = [1usize, 2, 3, 4];
    let ks = keys.get(..n).unwrap_or(&[]);
    let vs = vals.get(..n).unwrap_or(&[]);
    if let Ok(node) = view::NodeSlice::new(ks, vs, Some(&edges)) {
        if let Ok(mut lookup) = view::Lookup::new(0, h, &key) {
            let _ = lookup.provide(0, node);
        }
    }
}

fn leaf(count: usize, start: u8) -> Result<Link<u8, u8>, Error> {
    let n = node::allocate::<u8, u8, _>(0, &Unmetered)?;
    for i in 0..count {
        // SAFETY: caller bounds count<=11 and this helper owns a fresh leaf.
        unsafe {
            node::key(n, i).write(start + i as u8);
            node::val(n, i).write(start + i as u8);
        }
    }
    // SAFETY: exactly count pairs were initialized above. Kernel preparation
    // does not recursively re-prove insert, whose independent harness covers it.
    unsafe { node::set_len(n, count) };
    Ok(n)
}
fn siblings(l: usize, r: usize) -> Result<(Link<u8, u8>, Link<u8, u8>, Link<u8, u8>), Error> {
    let a = leaf(l, 0)?;
    let b = match leaf(r, 200) {
        Ok(b) => b,
        Err(e) => {
            // SAFETY: uniquely owned u8 leaf; u8 destruction has no effects.
            unsafe { node::free(a, 0, &Unmetered) };
            return Err(e);
        }
    };
    let parent = match node::allocate(1, &Unmetered) {
        Ok(p) => p,
        Err(e) => {
            // SAFETY: unique u8 leaves, no payload resources or descendants.
            unsafe {
                node::free(a, 0, &Unmetered);
                node::free(b, 0, &Unmetered)
            };
            return Err(e);
        }
    };
    // SAFETY: exclusively owned leaves and a matching empty internal parent.
    unsafe {
        node::set_child(parent, 0, a);
        node::set_child(parent, 1, b);
        node::key(parent, 0).write(100);
        node::val(parent, 0).write(100);
        node::set_len(parent, 1);
    }
    Ok((parent, a, b))
}
#[kani::proof]
#[kani::unwind(14)]
fn borrow_left_every_occupancy() {
    let l: usize = kani::any();
    let r: usize = kani::any();
    kani::assume(0 < l && l <= CAP && r < CAP);
    if let Ok((p, a, b)) = siblings(l, r) {
        // SAFETY: initialized adjacent leaf siblings satisfy the steal bounds.
        unsafe {
            node::steal_left(p, 0, 0);
            assert_eq!(node::len(a), l - 1);
            assert_eq!(node::len(b), r + 1);
            assert_eq!(*node::key(p, 0), (l - 1) as u8);
            assert_eq!(*node::key(b, 0), 100);
            for i in 0..l - 1 {
                assert_eq!(*node::key(a, i), i as u8);
            }
            for i in 0..r {
                assert_eq!(*node::key(b, i + 1), 200 + i as u8);
            }
            node::free(a, 0, &Unmetered);
            node::free(b, 0, &Unmetered);
            node::free(p, 1, &Unmetered);
        }
    }
}
#[kani::proof]
#[kani::unwind(14)]
fn borrow_right_every_occupancy() {
    let l: usize = kani::any();
    let r: usize = kani::any();
    kani::assume(l < CAP && 0 < r && r <= CAP);
    if let Ok((p, a, b)) = siblings(l, r) {
        // SAFETY: initialized adjacent leaf siblings satisfy the steal bounds.
        unsafe {
            node::steal_right(p, 0, 0);
            assert_eq!(node::len(a), l + 1);
            assert_eq!(node::len(b), r - 1);
            assert_eq!(*node::key(p, 0), 200);
            assert_eq!(*node::key(a, l), 100);
            for i in 0..l {
                assert_eq!(*node::key(a, i), i as u8);
            }
            for i in 0..r - 1 {
                assert_eq!(*node::key(b, i), 201 + i as u8);
            }
            node::free(a, 0, &Unmetered);
            node::free(b, 0, &Unmetered);
            node::free(p, 1, &Unmetered);
        }
    }
}
#[kani::proof]
#[kani::unwind(14)]
fn merge_every_fitting_occupancy() {
    let l: usize = kani::any();
    let r: usize = kani::any();
    kani::assume(l <= CAP && r <= CAP && l + r + 1 <= CAP);
    if let Ok((p, a, _)) = siblings(l, r) {
        // SAFETY: combined initialized siblings fit the allocated destination.
        unsafe {
            assert_eq!(node::merge(p, 0, 0, &Unmetered), a);
            assert_eq!(node::len(a), l + r + 1);
            assert_eq!(node::len(p), 0);
            assert_eq!(*node::key(a, l), 100);
            for i in 0..l {
                assert_eq!(*node::key(a, i), i as u8);
            }
            for i in 0..r {
                assert_eq!(*node::key(a, l + 1 + i), 200 + i as u8);
            }
            node::free(a, 0, &Unmetered);
            node::free(p, 1, &Unmetered);
        }
    }
}
#[kani::proof]
#[kani::unwind(14)]
fn suffix_every_node_edge() {
    let count: usize = kani::any();
    let i: usize = kani::any();
    kani::assume(count <= CAP && i <= count);
    if let Ok(left) = leaf(count, 0) {
        if let Ok(right) = leaf(0, 0) {
            let (a, b) = (left, right);
            // SAFETY: two uniquely owned matching leaves, valid split edge.
            unsafe {
                node::move_suffix(a, i, b, 0);
                assert_eq!(node::len(a), i);
                assert_eq!(node::len(b), count - i);
                for x in 0..i {
                    assert_eq!(*node::key(a, x), x as u8);
                }
                for x in 0..count - i {
                    assert_eq!(*node::key(b, x), (i + x) as u8);
                }
                node::free(a, 0, &Unmetered);
                node::free(b, 0, &Unmetered);
            }
        } else {
            // SAFETY: unique u8 leaf; no payload resource destructor is needed.
            unsafe { node::free(left, 0, &Unmetered) };
        }
    }
}
