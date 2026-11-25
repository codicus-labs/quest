//! Summary numbers of a run.

use std::time::Duration;

/// The value below which `fraction` (0.0 to 1.0) of the samples lie (nearest rank).
pub fn percentile(samples: &[Duration], fraction: f64) -> Option<Duration> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort();
    let rank = (fraction.clamp(0.0, 1.0) * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
