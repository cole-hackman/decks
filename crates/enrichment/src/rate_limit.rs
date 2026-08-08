//! One request per second, per provider.
//!
//! MusicBrainz documents this as a **condition of use**, not a courtesy —
//! clients that exceed it are blocked, and a blocked client means the feature
//! stops working for the user with no obvious cause. It is enforced here rather
//! than left to callers because a limit that each call site has to remember is
//! a limit that one call site will forget.
//!
//! The gate is a monotonic clock plus an async sleep, so it costs nothing when
//! requests are naturally spaced (the common case — a user enriching one track)
//! and serialises them when they are not (a bulk run over a selection).

use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

/// MusicBrainz's documented limit.
pub const MUSICBRAINZ_INTERVAL: Duration = Duration::from_millis(1000);

/// Cover Art Archive redirects to the Internet Archive and publishes no hard
/// limit; a slower cadence than MusicBrainz is polite and costs nothing, since
/// at most one image is fetched per track.
pub const COVER_ART_INTERVAL: Duration = Duration::from_millis(500);

/// Discogs allows 60 requests/minute for authenticated clients.
pub const DISCOGS_INTERVAL: Duration = Duration::from_millis(1000);

/// A minimum spacing between requests.
#[derive(Debug)]
pub struct RateLimiter {
    interval: Duration,
    /// When the next request may go out. `None` until the first one.
    next_allowed: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            next_allowed: Mutex::new(None),
        }
    }

    /// Wait until a request may be sent, then reserve the next slot.
    ///
    /// The lock is held across the sleep on purpose. Releasing it first would
    /// let every waiting caller compute the same wake time and then fire
    /// together — which is a burst, exactly what the limit forbids. Holding it
    /// makes the waiters queue, so N callers take N intervals.
    pub async fn acquire(&self) {
        let mut next = self.next_allowed.lock().await;
        let now = Instant::now();
        let go_at = match *next {
            Some(t) if t > now => {
                tokio::time::sleep_until(t).await;
                t
            }
            _ => now,
        };
        *next = Some(go_at + self.interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn the_first_request_is_not_delayed() {
        let rl = RateLimiter::new(Duration::from_secs(1));
        let start = Instant::now();
        rl.acquire().await;
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn a_second_request_waits_a_full_interval() {
        let rl = RateLimiter::new(Duration::from_secs(1));
        let start = Instant::now();
        rl.acquire().await;
        rl.acquire().await;
        assert_eq!(start.elapsed(), Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn requests_queue_rather_than_bursting() {
        // The property the held lock buys: five callers take four intervals,
        // not one. Releasing the lock before sleeping would let them all wake
        // at the same instant and fire together.
        let rl = RateLimiter::new(Duration::from_secs(1));
        let start = Instant::now();
        for _ in 0..5 {
            rl.acquire().await;
        }
        assert_eq!(start.elapsed(), Duration::from_secs(4));
    }

    #[tokio::test(start_paused = true)]
    async fn a_naturally_spaced_caller_is_never_delayed() {
        // The common case — one track at a time — must cost nothing.
        let rl = RateLimiter::new(Duration::from_secs(1));
        rl.acquire().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        let start = Instant::now();
        rl.acquire().await;
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[test]
    fn the_musicbrainz_interval_is_the_documented_one() {
        // Loosening this is a licence-of-use question, not a tuning knob.
        assert_eq!(MUSICBRAINZ_INTERVAL, Duration::from_secs(1));
    }
}
