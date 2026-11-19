//! Fallible collections and checked borrowed collection views.
pub mod btree_map;
pub use btree_map::BTreeMap;
pub mod btree_set;
pub use btree_set::BTreeSet;

pub mod hash_map;
pub use hash_map::HashMap;
pub mod hash_set;
pub use hash_set::HashSet;

pub mod binary_heap;
pub use binary_heap::BinaryHeap;

pub use crate::deque as vec_deque;
pub use vec_deque::VecDeque;
