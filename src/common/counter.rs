use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

static GLOBAL_COUNTER: OnceLock<AtomicU32> = OnceLock::new();

fn get_counter() -> &'static AtomicU32 {
    GLOBAL_COUNTER.get_or_init(|| AtomicU32::new(0))
}

pub fn get_inc() -> u32 {
    get_counter().fetch_add(1, Ordering::Relaxed)
}