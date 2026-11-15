//! A cooperative step counter. Every call of `Value::resolve_val` (one per AST node evaluation,
//! macros included) counts one step against a thread-local limit.
use crate::ExecutionError;
use std::cell::Cell;

thread_local! {
    static USED: Cell<u64> = const { Cell::new(0) };
    static LIMIT: Cell<u64> = const { Cell::new(u64::MAX) };
}

/// Resets the counter and sets the limit for the current thread.
pub fn set_limit(limit: u64) {
    USED.with(|u| u.set(0));
    LIMIT.with(|l| l.set(limit));
}

/// Steps used on the current thread since the last `set_limit`.
pub fn used() -> u64 {
    USED.with(|u| u.get())
}

#[inline(always)]
pub(crate) fn tick() -> Result<(), ExecutionError> {
    let n = USED.with(|u| {
        let n = u.get() + 1;
        u.set(n);
        n
    });
    if n > LIMIT.with(|l| l.get()) {
        Err(ExecutionError::function_error(
            "cost",
            "step limit exceeded",
        ))
    } else {
        Ok(())
    }
}
