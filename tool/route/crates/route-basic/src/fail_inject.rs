//! Failure injection for crash-safety testing.
//!
//! `check` is always compiled (and is a no-op in release builds), so
//! call sites inside `rollback_to_with` stay clean and unconditional.
//! Only the test-facing API is gated to `cfg(any(test, feature =
//! "test-utils"))` — it is never linked into release binaries.
//!
//! ## Why per-thread hooks
//!
//! Rust's test harness runs tests in parallel on a thread pool, and
//! `rollback_to_with` is on the hot path of *every* rollback test. A
//! process-global hook would leak from a crash test into unrelated
//! rollback tests running concurrently. We therefore key the hook by
//! `ThreadId`: only the thread that installed a hook sees it fire.
//! Combined with the `InjectionGuard` RAII (which clears the hook on
//! drop, even under panic), this gives complete isolation between
//! tests.
//!
//! ## Why error-injection rather than true process killing
//!
//! A true crash test would need a subprocess, which is painful on
//! Windows and fragile in CI. Error-injection is sufficient to prove
//! the journal works because the *observable* effect of a crash
//! mid-apply is identical: control flow leaves `rollback_to_with`
//! early, the journal `state` file stays at APPLYING, and recovery
//! must converge. We verify convergence, which is the actual
//! guarantee.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;

/// A point in the rollback flow where a test may inject a failure.
#[derive(Debug, Clone)]
pub enum InjectionPoint {
    /// Just before the apply phase writes any file.
    BeforeApply,
    /// After `written` of `total` target files have been atomically
    /// replaced.
    AfterFileWrite { written: usize, total: usize },
    /// After all files applied, before the SQLite metadata transaction.
    AfterApplyBeforeMetadata,
    /// After the SQLite transaction committed, before journal state
    /// moves to COMMITTED (the true commit point).
    AfterMetadataBeforeCommitted,
    /// After state=COMMITTED, before the journal directory is removed.
    BeforeTxRemove,
}

type Hook = Box<dyn Fn(&InjectionPoint) -> Option<String> + Send + Sync>;

fn hook_table() -> &'static Mutex<HashMap<ThreadId, Hook>> {
    static TBL: OnceLock<Mutex<HashMap<ThreadId, Hook>>> = OnceLock::new();
    TBL.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Called from `rollback_to_with` at each injection point. Returns
/// `Err(msg)` if a hook is installed **for the current thread** and
/// fired; `Ok(())` otherwise. In release builds no thread has a hook,
/// so this is a no-op the optimizer removes.
pub(crate) fn check(point: &InjectionPoint) -> Result<(), String> {
    let tid = std::thread::current().id();
    // `unwrap_or_else` survives a poisoned mutex (a prior test that
    // panicked inside a hook). Never panics itself.
    let guard = hook_table().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = guard.get(&tid) {
        if let Some(msg) = h(point) {
            return Err(msg);
        }
    }
    Ok(())
}

// ---- Test-only public API. Not linked into release builds. ----
#[cfg(any(test, feature = "test-utils"))]
#[doc(hidden)]
pub mod test_api {
    use super::{hook_table, InjectionPoint};
    use std::thread::ThreadId;

    /// Install a failure-injection hook for the **current thread
    /// only**. The hook is called at every injection point reached on
    /// this thread; if it returns `Some(msg)`, the operation aborts
    /// with that error. If `None`, the operation continues.
    ///
    /// Other threads are unaffected, so concurrent non-crash tests run
    /// normally. Prefer [`InjectionGuard`] which auto-clears on drop.
    pub fn set_fail_hook<F>(hook: F)
    where
        F: Fn(&InjectionPoint) -> Option<String> + Send + Sync + 'static,
    {
        let tid = std::thread::current().id();
        let mut guard = hook_table().lock().unwrap();
        guard.insert(tid, Box::new(hook));
    }

    /// Clear the hook installed on the current thread, if any.
    pub fn clear_fail_hook() {
        let tid: ThreadId = std::thread::current().id();
        let mut guard = hook_table().lock().unwrap();
        guard.remove(&tid);
    }

    /// RAII guard that clears the current thread's hook on drop.
    /// Use this so a hook never leaks past a test, even if the test
    /// body panics.
    pub struct InjectionGuard;
    impl InjectionGuard {
        pub fn new() -> Self {
            Self
        }
    }
    impl Default for InjectionGuard {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Drop for InjectionGuard {
        fn drop(&mut self) {
            clear_fail_hook();
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub use test_api::{clear_fail_hook, set_fail_hook, InjectionGuard};
