//! Sign-in backoff.
//!
//! Keyed by the login that was *attempted*, whether or not such a login exists.
//! Keying only real accounts would turn the throttle itself into a way to learn
//! which names are real: a name that slows down after a few tries exists, and
//! one that never slows down does not.
//!
//! In memory only. A host restart forgets the backoff, which is an acceptable
//! trade — the alternative is a restart being unable to clear a lockout, and a
//! household host gets restarted.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Failures allowed before any delay is imposed. People mistype.
const FREE_ATTEMPTS: u32 = 3;

/// The first delay, doubling with each further failure.
const BASE_DELAY: Duration = Duration::from_secs(2);

/// The longest a login is held off.
const MAX_DELAY: Duration = Duration::from_secs(5 * 60);

/// Forget a quiet login after this long, so the map cannot grow without bound.
const FORGET_AFTER: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone)]
struct Record {
    failures: u32,
    blocked_until: Option<Instant>,
    last_touched: Instant,
}

#[derive(Debug, Default)]
pub struct Throttle {
    records: Mutex<HashMap<String, Record>>,
}

fn key_for(login: &str) -> String {
    login.trim().to_lowercase()
}

fn delay_after(failures: u32) -> Option<Duration> {
    if failures <= FREE_ATTEMPTS {
        return None;
    }
    let steps = failures - FREE_ATTEMPTS - 1;
    let multiplier = 1u64.checked_shl(steps.min(20)).unwrap_or(u64::MAX);
    let delay = BASE_DELAY
        .checked_mul(multiplier.min(u32::MAX as u64) as u32)
        .unwrap_or(MAX_DELAY);
    Some(delay.min(MAX_DELAY))
}

impl Throttle {
    pub fn new() -> Self {
        Throttle {
            records: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Record>> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// How long this login must wait, if at all.
    pub fn retry_after(&self, login: &str, now: Instant) -> Option<Duration> {
        let records = self.lock();
        let record = records.get(&key_for(login))?;
        let until = record.blocked_until?;
        if until > now {
            Some(until - now)
        } else {
            None
        }
    }

    pub fn is_blocked(&self, login: &str, now: Instant) -> bool {
        self.retry_after(login, now).is_some()
    }

    pub fn record_failure(&self, login: &str, now: Instant) {
        let mut records = self.lock();
        let record = records.entry(key_for(login)).or_insert(Record {
            failures: 0,
            blocked_until: None,
            last_touched: now,
        });

        record.failures += 1;
        record.last_touched = now;
        record.blocked_until = delay_after(record.failures).map(|d| now + d);
    }

    /// A successful sign-in clears the history for that login.
    pub fn record_success(&self, login: &str) {
        self.lock().remove(&key_for(login));
    }

    /// Drop records nobody has touched in a while.
    pub fn prune(&self, now: Instant) {
        self.lock()
            .retain(|_, record| now.duration_since(record.last_touched) < FORGET_AFTER);
    }

    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn a_few_mistypes_cost_nothing() {
        let throttle = Throttle::new();
        let now = t0();
        for _ in 0..FREE_ATTEMPTS {
            throttle.record_failure("sam", now);
        }
        assert!(
            !throttle.is_blocked("sam", now),
            "people mistype; the first few tries should be free"
        );
    }

    #[test]
    fn the_delay_starts_after_the_free_attempts_and_grows() {
        let throttle = Throttle::new();
        let now = t0();
        for _ in 0..=FREE_ATTEMPTS {
            throttle.record_failure("sam", now);
        }
        let first = throttle.retry_after("sam", now).expect("should be blocked");
        assert_eq!(first, BASE_DELAY);

        throttle.record_failure("sam", now);
        let second = throttle.retry_after("sam", now).unwrap();
        assert!(second > first, "the delay should grow: {first:?} -> {second:?}");
    }

    #[test]
    fn the_delay_is_capped() {
        let throttle = Throttle::new();
        let now = t0();
        for _ in 0..80 {
            throttle.record_failure("sam", now);
        }
        let delay = throttle.retry_after("sam", now).unwrap();
        assert!(delay <= MAX_DELAY, "delay {delay:?} exceeded the cap");
    }

    #[test]
    fn a_block_lifts_once_the_delay_has_passed() {
        let throttle = Throttle::new();
        let now = t0();
        for _ in 0..=FREE_ATTEMPTS {
            throttle.record_failure("sam", now);
        }
        assert!(throttle.is_blocked("sam", now));

        let later = now + BASE_DELAY + Duration::from_millis(1);
        assert!(!throttle.is_blocked("sam", later));
    }

    #[test]
    fn signing_in_successfully_clears_the_history() {
        let throttle = Throttle::new();
        let now = t0();
        for _ in 0..=FREE_ATTEMPTS {
            throttle.record_failure("sam", now);
        }
        assert!(throttle.is_blocked("sam", now));

        throttle.record_success("sam");
        assert!(!throttle.is_blocked("sam", now));
    }

    /// The property that keeps the throttle from becoming an oracle: a login
    /// nobody has ever created must slow down exactly like a real one.
    #[test]
    fn an_unknown_login_is_throttled_exactly_like_a_real_one() {
        let throttle = Throttle::new();
        let now = t0();

        for _ in 0..=FREE_ATTEMPTS {
            throttle.record_failure("sam", now);
            throttle.record_failure("no-such-person", now);
        }

        assert_eq!(
            throttle.retry_after("sam", now),
            throttle.retry_after("no-such-person", now),
            "a real and an invented login must be indistinguishable by timing"
        );
    }

    #[test]
    fn throttling_one_login_does_not_lock_out_another() {
        let throttle = Throttle::new();
        let now = t0();
        for _ in 0..=FREE_ATTEMPTS {
            throttle.record_failure("sam", now);
        }
        assert!(throttle.is_blocked("sam", now));
        assert!(
            !throttle.is_blocked("alex", now),
            "one person's failures must not lock out another"
        );
    }

    #[test]
    fn the_key_folds_case_and_spacing_like_the_login_itself_does() {
        let throttle = Throttle::new();
        let now = t0();
        for _ in 0..=FREE_ATTEMPTS {
            throttle.record_failure("Sam", now);
        }
        assert!(
            throttle.is_blocked("  sam ", now),
            "logins are case-insensitive, so the throttle must be too"
        );
    }

    #[test]
    fn quiet_logins_are_forgotten_so_the_map_cannot_grow_forever() {
        let throttle = Throttle::new();
        let now = t0();
        for i in 0..50 {
            throttle.record_failure(&format!("person-{i}"), now);
        }
        assert_eq!(throttle.tracked(), 50);

        throttle.prune(now + FORGET_AFTER + Duration::from_secs(1));
        assert_eq!(throttle.tracked(), 0);
    }

    #[test]
    fn pruning_keeps_a_login_that_is_still_being_hammered() {
        let throttle = Throttle::new();
        let now = t0();
        throttle.record_failure("sam", now);

        let later = now + FORGET_AFTER - Duration::from_secs(1);
        throttle.prune(later);
        assert_eq!(throttle.tracked(), 1);
    }
}
