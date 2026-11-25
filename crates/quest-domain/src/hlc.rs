use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Identifies one installation (client database or server) that issues timestamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

/// Hybrid logical clock timestamp. Total order: wall time, then counter, then node, so two
/// nodes never produce equal timestamps and every replica picks the same winner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hlc {
    pub wall_ms: u64,
    pub counter: u32,
    pub node: NodeId,
}

impl Hlc {
    pub const ZERO: Hlc = Hlc { wall_ms: 0, counter: 0, node: NodeId(0) };

    /// False if the timestamp is further than `max_drift_ms` ahead of `physical_ms`.
    pub fn is_plausible(&self, physical_ms: u64, max_drift_ms: u64) -> bool {
        self.wall_ms <= physical_ms.saturating_add(max_drift_ms)
    }
}

/// Fixed-width hex, so the string order equals the `Ord` order (useful for SQL comparisons).
impl fmt::Display for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}-{:08x}-{:016x}", self.wall_ms, self.counter, self.node.0)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid HLC timestamp {0:?}: expected <16 hex>-<8 hex>-<16 hex>")]
pub struct HlcParseError(String);

impl FromStr for Hlc {
    type Err = HlcParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || HlcParseError(s.to_owned());
        let mut parts = s.split('-');
        let (Some(wall), Some(counter), Some(node), None) = (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };
        if wall.len() != 16 || counter.len() != 8 || node.len() != 16 {
            return Err(invalid());
        }
        Ok(Hlc {
            wall_ms: u64::from_str_radix(wall, 16).map_err(|_| invalid())?,
            counter: u32::from_str_radix(counter, 16).map_err(|_| invalid())?,
            node: NodeId(u64::from_str_radix(node, 16).map_err(|_| invalid())?),
        })
    }
}

impl Serialize for Hlc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Hlc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

/// Issues monotonically increasing timestamps for one node. The physical time is passed in so
/// the clock stays pure and testable.
#[derive(Clone, Debug)]
pub struct HlcClock {
    last: Hlc,
}

impl HlcClock {
    pub fn new(node: NodeId) -> Self {
        HlcClock { last: Hlc { wall_ms: 0, counter: 0, node } }
    }

    /// Resumes from the highest timestamp this node has seen, e.g. after a restart.
    pub fn resume(node: NodeId, highest_seen: Hlc) -> Self {
        let mut clock = HlcClock::new(node);
        clock.advance_to(highest_seen);
        clock
    }

    pub fn node(&self) -> NodeId {
        self.last.node
    }

    pub fn now(&mut self, physical_ms: u64) -> Hlc {
        let node = self.last.node;
        self.last = if physical_ms > self.last.wall_ms {
            Hlc { wall_ms: physical_ms, counter: 0, node }
        } else {
            match self.last.counter.checked_add(1) {
                Some(counter) => Hlc { wall_ms: self.last.wall_ms, counter, node },
                None => Hlc { wall_ms: self.last.wall_ms.saturating_add(1), counter: 0, node },
            }
        };
        self.last
    }

    /// Records a remote timestamp so the next local one sorts after it. Timestamps more than
    /// `max_drift_ms` ahead of `physical_ms` are ignored (returns false): a device with a wrong
    /// clock must not drag every other clock into the future.
    pub fn observe(&mut self, remote: Hlc, physical_ms: u64, max_drift_ms: u64) -> bool {
        if !remote.is_plausible(physical_ms, max_drift_ms) {
            return false;
        }
        self.advance_to(remote);
        true
    }

    fn advance_to(&mut self, remote: Hlc) {
        let node = self.last.node;
        let remote_here = Hlc { wall_ms: remote.wall_ms, counter: remote.counter, node };
        self.last = self.last.max(remote_here);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_order_matches_ord() {
        let a = Hlc { wall_ms: 9, counter: 300, node: NodeId(2) };
        let b = Hlc { wall_ms: 10, counter: 0, node: NodeId(1) };
        assert!(a < b);
        assert!(a.to_string() < b.to_string());
        assert_eq!(a.to_string().parse::<Hlc>().unwrap(), a);
    }

    #[test]
    fn clock_is_monotonic_when_physical_time_stalls_or_goes_back() {
        let mut clock = HlcClock::new(NodeId(1));
        let first = clock.now(100);
        let second = clock.now(100);
        let third = clock.now(50);
        assert!(first < second && second < third);
    }

    #[test]
    fn observed_remote_time_is_exceeded_by_next_local_time() {
        let mut clock = HlcClock::new(NodeId(1));
        let remote = Hlc { wall_ms: 1_000, counter: 7, node: NodeId(9) };
        assert!(clock.observe(remote, 1_000, 0));
        assert!(clock.now(10) > remote);
    }

    #[test]
    fn far_future_remote_time_is_ignored() {
        let mut clock = HlcClock::new(NodeId(1));
        let poisoned = Hlc { wall_ms: u64::MAX, counter: 0, node: NodeId(9) };
        assert!(!clock.observe(poisoned, 1_000, 300_000));
        assert!(clock.now(1_000).wall_ms == 1_000);
    }

    #[test]
    fn counter_overflow_moves_to_the_next_millisecond() {
        let mut clock = HlcClock::resume(NodeId(1), Hlc { wall_ms: 5, counter: u32::MAX, node: NodeId(1) });
        let next = clock.now(5);
        assert_eq!((next.wall_ms, next.counter), (6, 0));
    }

    #[test]
    fn rejects_malformed_strings() {
        assert!("nope".parse::<Hlc>().is_err());
        assert!("0-0-0".parse::<Hlc>().is_err());
    }
}
