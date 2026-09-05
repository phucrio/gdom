use std::sync::atomic::{AtomicU64, Ordering};

pub fn next_entity_id() -> u128 {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let seq = u128::from(SEQ.fetch_add(1, Ordering::Relaxed));
    (nanos << 16) | (seq & 0xFFFF)
}
