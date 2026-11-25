//! Settings: the weekly target for Quest Hours. Pure.

use quest_domain::{Command, Snapshot};

use crate::error::MobileError;
use crate::types::SettingsData;
use quest_view::format::hours;

/// One hour up to a full week.
const WEEKLY_TARGET_MINUTES: std::ops::RangeInclusive<u32> = 60..=168 * 60;

pub(crate) fn settings(snapshot: &Snapshot) -> SettingsData {
    let minutes = snapshot.settings.weekly_target_minutes;
    SettingsData { weekly_target_minutes: minutes, weekly_target_label: hours(i64::from(minutes) * 60_000) }
}

pub(crate) fn set_weekly_target(minutes: u32) -> Result<Command, MobileError> {
    if !WEEKLY_TARGET_MINUTES.contains(&minutes) {
        return Err(MobileError::InvalidWeeklyTarget { minutes });
    }
    Ok(Command::SetWeeklyTarget { minutes })
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
