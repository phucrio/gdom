use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const MAX_RETRY_ATTEMPTS: u32 = 6;
pub const MAX_BACKOFF: Duration = Duration::from_secs(64);

pub trait Sleeper: Send + Sync {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

pub trait JitterSource: Send + Sync {
    fn jitter_secs(&self) -> u64;
}

pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(tokio::time::sleep(duration))
    }
}

pub struct SystemJitter;

impl JitterSource for SystemJitter {
    fn jitter_secs(&self) -> u64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => u64::from(duration.subsec_nanos() % 2),
            Err(_) => 0,
        }
    }
}

pub struct ZeroJitter;

impl JitterSource for ZeroJitter {
    fn jitter_secs(&self) -> u64 {
        0
    }
}

/// Bounded exponential backoff: `min(2^attempt + jitter, 64 seconds)`.
pub fn backoff_delay(attempt: u32, jitter_secs: u64) -> Duration {
    let base = 1_u64.checked_shl(attempt).unwrap_or(u64::MAX);
    Duration::from_secs(base.saturating_add(jitter_secs).min(MAX_BACKOFF.as_secs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_follows_plan_formula_and_caps_at_64s() {
        assert_eq!(backoff_delay(0, 0), Duration::from_secs(1));
        assert_eq!(backoff_delay(1, 0), Duration::from_secs(2));
        assert_eq!(backoff_delay(2, 1), Duration::from_secs(5));
        assert_eq!(backoff_delay(6, 0), Duration::from_secs(64));
        assert_eq!(backoff_delay(7, 9), Duration::from_secs(64));
    }
}
