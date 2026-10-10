//! Containment of panics caused by font data that did not ship with the crate.
//!
//! Such fonts are parsed and shaped under [`guarded`], which turns a panic into `None`. Shared
//! caches are locked with [`lock`], which ignores the poisoning a panic may leave behind.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`, keeping its data even if a thread panicked while holding it.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `f`, which parses or shapes font bytes that did not ship with the crate, and turns a
/// panic into `None`.
///
/// Malformed fonts could trigger panics inside the font parser or the shaper, for example
/// arithmetic overflow in debug builds. Such a font is treated as unreadable. The panic hook is
/// left alone, so the default or host hook may still print a message for the caught panic. With
/// `panic = "abort"` nothing can be caught.
pub(super) fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f)).ok()
}
