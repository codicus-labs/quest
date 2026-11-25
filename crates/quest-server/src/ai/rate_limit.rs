//! In-memory sliding-window limit of AI requests per account. Resets when the server restarts,
//! which is fine for a cost guard on a personal server.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use quest_protocol::UserId;

const MINUTE: Duration = Duration::from_secs(60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateLimits {
    pub per_minute: usize,
    pub per_day: usize,
}

impl Default for RateLimits {
    fn default() -> Self {
        RateLimits { per_minute: 10, per_day: 200 }
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    limits: RateLimits,
    /// Start times of the accepted requests of the last day, oldest first.
    accepted: HashMap<UserId, VecDeque<Instant>>,
}

impl RateLimiter {
    pub fn new(limits: RateLimits) -> Self {
        RateLimiter { limits, accepted: HashMap::new() }
    }

    /// Counts a request at `now` if the account is below both limits; false otherwise.
    pub fn try_acquire(&mut self, user: UserId, now: Instant) -> bool {
        self.forget_older_than_a_day(now);
        let recent = self.accepted.entry(user).or_default();
        let last_minute = recent.iter().rev().take_while(|at| now.duration_since(**at) < MINUTE).count();
        if last_minute >= self.limits.per_minute || recent.len() >= self.limits.per_day {
            return false;
        }
        recent.push_back(now);
        true
    }

    fn forget_older_than_a_day(&mut self, now: Instant) {
        self.accepted.retain(|_, times| {
            while times.front().is_some_and(|at| now.duration_since(*at) >= DAY) {
                times.pop_front();
            }
            !times.is_empty()
        });
    }
}
