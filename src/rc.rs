//! Fallible single-thread shared ownership and weak references.
pub type Rc<T, P = crate::budget::Unmetered> =
    crate::shared::Strong<T, crate::shared::count::LocalCount, P>;
pub type Weak<T, P = crate::budget::Unmetered> =
    crate::shared::Weak<T, crate::shared::count::LocalCount, P>;
