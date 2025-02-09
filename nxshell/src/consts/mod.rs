use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;

mod id;
pub use id::*;

pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");
pub const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");
pub static GLOBAL_COUNTER: Counter = Counter::new();

pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    pub const fn new() -> Counter {
        Self {
            value: AtomicU64::new(0),
        }
    }

    pub fn next(&self) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

// operation execute once
pub static SHOW_DOCK_PANEL_ONCE: Once = Once::new();
