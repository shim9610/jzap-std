//! Compiler-checked explicit shared-owner coercions; no fabricated metadata.
#[macro_export]
macro_rules! rc_unsize {
    ($value:expr => $target:ty) => {{
        let __raw = $crate::rc::Rc::into_raw($value);
        let __coerced: *mut $crate::shared::Control<$crate::shared::count::LocalCount, _, $target> =
            __raw.as_ptr();
        // SAFETY: compiler-checked tail unsizing preserves header/layout/receipt.
        unsafe { $crate::rc::Rc::from_raw(core::ptr::NonNull::new_unchecked(__coerced)) }
    }};
}
#[macro_export]
macro_rules! arc_unsize {
    ($value:expr => $target:ty) => {{
        let __raw = $crate::sync::Arc::into_raw($value);
        let __coerced: *mut $crate::shared::Control<
            $crate::shared::count::AtomicCount,
            _,
            $target,
        > = __raw.as_ptr();
        // SAFETY: compiler-checked tail unsizing preserves header/layout/receipt.
        unsafe { $crate::sync::Arc::from_raw(core::ptr::NonNull::new_unchecked(__coerced)) }
    }};
}
#[macro_export]
macro_rules! rc_weak_unsize {
    ($value:expr => $target:ty) => {{
        let __raw = $crate::rc::Weak::into_raw($value);
        let __coerced = __raw.map(|__ptr| {
            let __p: *mut $crate::shared::Control<$crate::shared::count::LocalCount, _, $target> =
                __ptr.as_ptr();
            // SAFETY: only compiler-checked metadata coercion, even for dead T.
            unsafe { core::ptr::NonNull::new_unchecked(__p) }
        });
        // SAFETY: transfers the original weak credit or empty state once.
        unsafe { $crate::rc::Weak::from_raw(__coerced) }
    }};
}
#[macro_export]
macro_rules! arc_weak_unsize {
    ($value:expr => $target:ty) => {{
        let __raw = $crate::sync::Weak::into_raw($value);
        let __coerced = __raw.map(|__ptr| {
            let __p: *mut $crate::shared::Control<$crate::shared::count::AtomicCount, _, $target> =
                __ptr.as_ptr();
            // SAFETY: only compiler-checked metadata coercion, even for dead T.
            unsafe { core::ptr::NonNull::new_unchecked(__p) }
        });
        // SAFETY: transfers the original weak credit or empty state once.
        unsafe { $crate::sync::Weak::from_raw(__coerced) }
    }};
}

#[macro_export]
macro_rules! pin_rc_unsize {
    ($value:expr => $target:ty) => {{
        let __pinned=$value;
        // SAFETY: only the handle is transferred and immediately repinned;
        // compiler-checked unsizing preserves the payload address and lifetime.
        let __owner=unsafe{$crate::shared::Pinned::into_inner_unchecked(__pinned)};
        unsafe{$crate::rc::Rc::into_pin_unchecked($crate::rc_unsize!(__owner=>$target))}
    }};
}
#[macro_export]
macro_rules! pin_arc_unsize {
    ($value:expr => $target:ty) => {{
        let __pinned=$value;
        // SAFETY: only the handle is transferred and immediately repinned;
        // compiler-checked unsizing preserves the payload address and lifetime.
        let __owner=unsafe{$crate::shared::Pinned::into_inner_unchecked(__pinned)};
        unsafe{$crate::sync::Arc::into_pin_unchecked($crate::arc_unsize!(__owner=>$target))}
    }};
}
