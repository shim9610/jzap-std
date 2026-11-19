//! Fallible atomic shared ownership and weak references.
pub type Arc<T, P = crate::budget::Unmetered> =
    crate::shared::Strong<T, crate::shared::count::AtomicCount, P>;
pub type Weak<T, P = crate::budget::Unmetered> =
    crate::shared::Weak<T, crate::shared::count::AtomicCount, P>;
