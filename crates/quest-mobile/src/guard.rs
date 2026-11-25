//! Keeps Rust panics from reaching foreign code as crashes: exported methods run inside
//! `guard`, which turns a panic into `MobileError::Internal`.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::error::MobileError;

/// Unwind safety: shared state sits behind mutexes that recover from poisoning, so a panic
/// cannot leave a half-updated value visible to the next call.
pub(crate) fn guard<T>(call: impl FnOnce() -> Result<T, MobileError>) -> Result<T, MobileError> {
    catch_unwind(AssertUnwindSafe(call)).unwrap_or_else(|payload| Err(internal(payload.as_ref())))
}

fn internal(payload: &(dyn Any + Send)) -> MobileError {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_owned());
    MobileError::Internal { detail }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
