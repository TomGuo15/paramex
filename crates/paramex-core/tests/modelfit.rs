use std::sync::{Mutex, OnceLock};

/// Test-binary-wide immutable fixture cache. `FittedDevice` is `Send` but not
/// `Sync`, so the mutex supplies safe static storage while callers receive an
/// independent clone for mutation.
type FixtureCache<T> = OnceLock<Mutex<T>>;

fn clone_fixture<T: Clone + 'static>(
    cache: &'static FixtureCache<T>,
    initialize: impl FnOnce() -> T,
) -> T {
    cache
        .get_or_init(|| Mutex::new(initialize()))
        .lock()
        .expect("fixture cache lock remains available")
        .clone()
}

#[path = "modelfit/defaults.rs"]
mod defaults;
#[path = "modelfit/fitted_device_real.rs"]
mod fitted_device_real;
#[path = "modelfit/public_ingest.rs"]
mod public_ingest;
