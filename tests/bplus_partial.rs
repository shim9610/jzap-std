//! Partial-key B+ point lookup against `std::collections::BTreeMap`.
use jzap::{
    Error,
    collections::btree_map::view::bplus::{
        PartialKey, PartialLookup, PartialNode, PartialStep, PrefixKey,
    },
};
use std::collections::BTreeMap as Model;

const P: usize = 4;

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

fn stored<'a>(full: &[u8]) -> PrefixKey<'a, P> {
    let mut prefix = [0; P];
    for (slot, byte) in prefix.iter_mut().zip(full) {
        *slot = *byte;
    }
    PrefixKey::new(prefix, full.len())
}

/// A node as the caller stores it: full keys plus values or child IDs.
struct Stored {
    keys: Vec<Vec<u8>>,
    values: Vec<u32>,
    children: Option<Vec<usize>>,
}

struct Tree {
    nodes: Vec<Stored>,
    root: usize,
    height: usize,
}

/// Build a B+ tree whose separators are the minima of their children.
fn build(entries: &[(Vec<u8>, u32)], leaf: usize, branch: usize) -> Tree {
    let mut nodes = Vec::new();
    let mut level: Vec<(usize, Vec<u8>)> = Vec::new();
    for chunk in entries.chunks(leaf) {
        level.push((nodes.len(), chunk[0].0.clone()));
        nodes.push(Stored {
            keys: chunk.iter().map(|(key, _)| key.clone()).collect(),
            values: chunk.iter().map(|(_, value)| *value).collect(),
            children: None,
        });
    }
    if level.is_empty() {
        nodes.push(Stored {
            keys: Vec::new(),
            values: Vec::new(),
            children: None,
        });
        return Tree {
            nodes,
            root: 0,
            height: 0,
        };
    }
    let mut height = 0;
    while level.len() > 1 {
        height += 1;
        let mut next = Vec::new();
        for chunk in level.chunks(branch) {
            next.push((nodes.len(), chunk[0].1.clone()));
            nodes.push(Stored {
                keys: chunk.iter().map(|(_, key)| key.clone()).collect(),
                values: Vec::new(),
                children: Some(chunk.iter().map(|(id, _)| *id).collect()),
            });
        }
        level = next;
    }
    Tree {
        nodes,
        root: level[0].0,
        height,
    }
}

#[derive(Default, Debug)]
struct Counts {
    keys: usize,
    fences: usize,
}

/// Run one lookup, attaching each requested key and passing the fences'
/// full keys on request (or in advance when `eager_fences`). `attach` picks
/// which keys are attached before the first call.
fn lookup(
    tree: &Tree,
    query: &[u8],
    attach: &mut impl FnMut() -> bool,
    eager_fences: bool,
    counts: &mut Counts,
) -> Result<Option<(Vec<u8>, u32)>, Error> {
    let mut lookup = PartialLookup::new(tree.root, tree.height, query)?;
    while let Some(id) = lookup.requested_node() {
        let node = &tree.nodes[id];
        let mut keys: Vec<PrefixKey<'_, P>> = node.keys.iter().map(|key| stored(key)).collect();
        for (key, full) in keys.iter_mut().zip(&node.keys) {
            if attach() {
                key.attach(full).unwrap();
            }
        }
        let incomplete = keys.iter().filter(|key| key.full().is_none()).count();
        let mut fenced = eager_fences;
        let mut requests = 0;
        loop {
            let view = match &node.children {
                Some(children) => PartialNode::branch(&keys, children)?,
                None => PartialNode::leaf(&keys, &node.values)?,
            };
            let (minimum, ceiling) = lookup.fence_origins();
            let full = |origin: Option<(usize, usize)>| {
                origin.map(|(node, index)| tree.nodes[node].keys[index].as_slice())
            };
            let step = if fenced {
                lookup.provide_fenced(id, &view, full(minimum), full(ceiling))?
            } else {
                lookup.provide(id, &view)?
            };
            match step {
                PartialStep::NeedNode(child) => {
                    assert_eq!(lookup.requested_node(), Some(child));
                    break;
                }
                PartialStep::NeedKey(index) => {
                    assert!(keys[index].full().is_none(), "a resolved key was requested");
                    assert_eq!(lookup.requested_node(), Some(id));
                    keys[index].attach(&node.keys[index]).unwrap();
                    counts.keys += 1;
                    requests += 1;
                    assert!(requests <= incomplete);
                }
                PartialStep::NeedFence => {
                    assert!(!fenced && (minimum.is_some() || ceiling.is_some()));
                    counts.fences += 1;
                    fenced = true;
                }
                PartialStep::Found(key, value) => {
                    return Ok(Some((key.full().unwrap().to_vec(), *value)));
                }
                PartialStep::Absent => return Ok(None),
            }
        }
    }
    unreachable!("the lookup finished without a result")
}

/// Keys from a two-letter alphabet with long shared prefixes.
fn key(seed: &mut u64) -> Vec<u8> {
    let shared: &[&[u8]] = &[b"", b"ab", b"abab", b"ababab", b"abababab"];
    let mut key = shared[rng(seed) as usize % shared.len()].to_vec();
    for _ in 0..rng(seed) % 6 {
        key.push(b'a' + (rng(seed) % 2) as u8);
    }
    key
}

#[test]
fn partial_lookup_matches_std_with_shared_prefixes() {
    let rounds = if cfg!(miri) { 3 } else { 300 };
    let mut seed = 0x9e37_79b9_7f4a_7c15;
    let mut totals = Counts::default();
    for _ in 0..rounds {
        let mut model = Model::new();
        for value in 0..rng(&mut seed) % 60 {
            model.insert(key(&mut seed), value as u32);
        }
        let entries: Vec<_> = model.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let leaf = 2 + rng(&mut seed) as usize % 5;
        let branch = 2 + rng(&mut seed) as usize % 4;
        let tree = build(&entries, leaf, branch);
        let rate = rng(&mut seed) % 3;
        let eager = rng(&mut seed) % 2 == 0;
        for _ in 0..if cfg!(miri) { 4 } else { 40 } {
            let query = key(&mut seed);
            let mut attach_seed = rng(&mut seed) | 1;
            let mut attach = || rng(&mut attach_seed) % 3 < rate;
            let actual = lookup(&tree, &query, &mut attach, eager, &mut totals).unwrap();
            let expected = model.get_key_value(&query).map(|(k, v)| (k.clone(), *v));
            assert_eq!(actual, expected, "query {query:?}");
        }
    }
    eprintln!("requests: {totals:?}");
    if !cfg!(miri) {
        assert!(totals.keys > 0 && totals.fences > 0, "{totals:?}");
    }
}

#[test]
fn attached_keys_need_no_requests() {
    let mut seed = 7;
    let mut model = Model::new();
    for value in 0..50 {
        model.insert(key(&mut seed), value);
    }
    let entries: Vec<_> = model.iter().map(|(k, v)| (k.clone(), *v)).collect();
    let tree = build(&entries, 4, 3);
    let mut counts = Counts::default();
    for (query, value) in &entries {
        let found = lookup(&tree, query, &mut || true, true, &mut counts).unwrap();
        assert_eq!(found, Some((query.clone(), *value)));
    }
    assert_eq!((counts.keys, counts.fences), (0, 0));
}

fn leaf_result(keys: &[&[u8]], query: &[u8]) -> Result<PartialStep<'static, (), (), usize>, Error> {
    let values: Vec<u32> = (0..keys.len() as u32).collect();
    let mut stored: Vec<PrefixKey<'_, P>> = keys.iter().map(|key| stored(key)).collect();
    let mut lookup = PartialLookup::new(0usize, 0, query)?;
    loop {
        let node = PartialNode::leaf(&stored, &values)?;
        match lookup.provide(0, &node)? {
            PartialStep::NeedKey(index) => stored[index].attach(keys[index])?,
            PartialStep::Found(..) => return Ok(PartialStep::Found(&(), &())),
            PartialStep::Absent => return Ok(PartialStep::Absent),
            _ => unreachable!(),
        }
    }
}

#[test]
fn damage_off_the_search_path_is_refused() {
    // Ordered by the first byte only: 1 < 5 > 3 < 7. A search for 3 sees
    // 5 and 1 and would answer absent without full validation.
    let keys: [&[u8]; 4] = [b"1", b"5", b"3", b"7"];
    assert_eq!(leaf_result(&keys, b"3"), Err(Error::InvalidStructure));
    // The same damage hidden behind equal prefixes needs resolution first.
    let keys: [&[u8]; 4] = [b"abcd1", b"abcd5", b"abcd3", b"abcd7"];
    assert_eq!(leaf_result(&keys, b"abcd3"), Err(Error::InvalidStructure));
    let keys: [&[u8]; 3] = [b"abcd1", b"abcd1", b"abcd7"];
    assert_eq!(leaf_result(&keys, b"abcd7"), Err(Error::InvalidStructure));
    let keys: [&[u8]; 4] = [b"abcd1", b"abcd3", b"abcd5", b"abcd7"];
    assert_eq!(
        leaf_result(&keys, b"abcd5"),
        Ok(PartialStep::Found(&(), &()))
    );
    assert_eq!(leaf_result(&keys, b"abcd4"), Ok(PartialStep::Absent));
}

#[test]
fn attachments_and_fences_must_match_the_stored_key() {
    let mut key = stored(b"abcdef");
    assert_eq!(key.attach(b"abcdeg!"), Err(Error::InvalidStructure));
    assert_eq!(key.attach(b"abceef"), Err(Error::InvalidStructure));
    assert!(key.full().is_none());
    key.attach(b"abcdeg").unwrap();
    assert_eq!(key.full(), Some(&b"abcdeg"[..]));

    // Root branch over one leaf whose first key differs from the separator
    // only after the inline prefix. The separator is resolved in the root,
    // but its fence keeps only the prefix and length.
    let separators = [PrefixKey::<P>::from_full(b"abcdef")];
    let children = [1usize];
    let query: &[u8] = b"abcdzz";
    let mut lookup = PartialLookup::new(0usize, 1, query).unwrap();
    let root = PartialNode::<_, u32, _>::branch(&separators, &children).unwrap();
    assert!(matches!(
        lookup.provide(0, &root),
        Ok(PartialStep::NeedNode(1))
    ));
    assert_eq!(lookup.fence_origins(), (Some((0, 0)), None));

    let mut leaf_keys = [PrefixKey::<P>::from_full(b"abcdeg")];
    let values = [7u32];
    let leaf = PartialNode::leaf(&leaf_keys, &values).unwrap();
    // Out of turn: the root is no longer requested.
    assert!(matches!(
        lookup.provide(0, &leaf),
        Err(Error::InvalidStructure)
    ));
    assert!(matches!(
        lookup.provide(1, &leaf),
        Ok(PartialStep::NeedFence)
    ));
    // A full key the fence does not admit, or one for a missing fence.
    let fence: &[u8] = b"abcdef";
    assert!(matches!(
        lookup.provide_fenced(1, &leaf, Some(&b"abcdefg"[..]), None),
        Err(Error::InvalidStructure)
    ));
    assert!(matches!(
        lookup.provide_fenced(1, &leaf, Some(fence), Some(query)),
        Err(Error::InvalidStructure)
    ));
    // The resolved separator differs from the child's minimum.
    assert!(matches!(
        lookup.provide_fenced(1, &leaf, Some(fence), None),
        Err(Error::InvalidStructure)
    ));
    assert_eq!(lookup.requested_node(), Some(1));
    // A matching child is accepted after the refusals.
    leaf_keys = [PrefixKey::from_full(b"abcdef")];
    let leaf = PartialNode::leaf(&leaf_keys, &values).unwrap();
    assert!(matches!(
        lookup.provide_fenced(1, &leaf, Some(fence), None),
        Ok(PartialStep::Absent)
    ));
    assert_eq!(lookup.requested_node(), None);
}

#[test]
fn prefix_keys_order_by_bytes() {
    let words: [&[u8]; 9] = [
        b"", b"a", b"abcd", b"abcda", b"abcdb", b"abce", b"abcd\0", b"b", b"abcdaa",
    ];
    for a in words {
        for b in words {
            let actual = a.cmp(b);
            let (x, y) = (stored(a), stored(b));
            for decided in [
                x.cmp_stored(&y),
                x.cmp_full(b),
                x.cmp_fence(&y.fence()),
                PrefixKey::<P>::fence_cmp_full(&x.fence(), b),
            ] {
                assert!(decided.is_none_or(|order| order == actual), "{a:?} {b:?}");
            }
            if a.len() <= P && b.len() <= P {
                assert_eq!(x.cmp_stored(&y), Some(actual));
            }
            if a.len() > P && b.len() > P && a[..P] == b[..P] {
                assert_eq!(x.cmp_stored(&y), None);
            }
        }
    }
}

/// Every byte string of at most two bytes over {0, 1, 2, 3}. Four values
/// realize every order of three keys and a query at each byte position.
fn small_strings() -> Vec<Vec<u8>> {
    let mut all = vec![Vec::new()];
    for a in 0..4u8 {
        all.push(vec![a]);
        for b in 0..4u8 {
            all.push(vec![a, b]);
        }
    }
    all
}

/// Mirrors the Kani leaf harness exhaustively over a small alphabet: one
/// inline byte, keys of up to two bytes, every attachment choice and query.
#[test]
fn exhaustive_small_leaves_match_complete_lookup() {
    use jzap::collections::btree_map::view::{
        LookupStep,
        bplus::{Lookup, Node},
    };
    let strings = small_strings();
    let counts = if cfg!(miri) { 1..=1 } else { 0..=3 };
    let mut cases = 0usize;
    for n in counts {
        let mut index = vec![0usize; n];
        loop {
            let completed: Vec<&[u8]> = index.iter().map(|&i| strings[i].as_slice()).collect();
            let values: Vec<u8> = (0..n as u8).collect();
            for mask in 0..1u32 << n {
                for query in &strings {
                    let mut keys: Vec<PrefixKey<'_, 1>> = completed
                        .iter()
                        .map(|full| {
                            PrefixKey::new([full.first().copied().unwrap_or(0)], full.len())
                        })
                        .collect();
                    for (i, key) in keys.iter_mut().enumerate() {
                        if mask >> i & 1 == 1 {
                            key.attach(completed[i]).unwrap();
                        }
                    }
                    let mut lookup = PartialLookup::new(0usize, 0, query.as_slice()).unwrap();
                    let mut result = None;
                    for _ in 0..=n {
                        let node = PartialNode::leaf(&keys, &values).unwrap();
                        let requested = match lookup.provide(0, &node) {
                            Ok(PartialStep::Found(key, value)) => {
                                result = Some(Ok(Some((key.full().unwrap().to_vec(), *value))));
                                break;
                            }
                            Ok(PartialStep::Absent) => {
                                result = Some(Ok(None));
                                break;
                            }
                            Ok(PartialStep::NeedKey(i)) => i,
                            Ok(step) => panic!("leaf returned {step:?}"),
                            Err(error) => {
                                result = Some(Err(error));
                                break;
                            }
                        };
                        assert!(keys[requested].full().is_none());
                        keys[requested].attach(completed[requested]).unwrap();
                    }
                    let mut complete = Lookup::new(0usize, 0, query.as_slice()).unwrap();
                    let expected = Node::leaf(&completed, &values).and_then(|node| {
                        complete.provide(0, &node).map(|step| match step {
                            LookupStep::Found(key, value) => Some((key.to_vec(), *value)),
                            LookupStep::Absent => None,
                            step => panic!("leaf returned {step:?}"),
                        })
                    });
                    assert_eq!(
                        result,
                        Some(expected),
                        "{completed:?} mask {mask} query {query:?}"
                    );
                    cases += 1;
                }
            }
            // Next index tuple.
            let mut position = 0;
            while position < n {
                index[position] += 1;
                if index[position] < strings.len() {
                    break;
                }
                index[position] = 0;
                position += 1;
            }
            if position == n {
                break;
            }
        }
    }
    eprintln!("exhaustive cases: {cases}");
    if !cfg!(miri) {
        // 21 strings: sum over n = 0..=3 of 21^n keys * 2^n masks * 21 queries.
        assert_eq!(cases, 21 * (1 + 42 + 42 * 42 + 42 * 42 * 42));
    }
}
