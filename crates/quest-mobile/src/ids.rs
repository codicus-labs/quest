//! Reads the ids and days the UI passes back.

use chrono::NaiveDate;
use quest_domain::EntityId;

use crate::error::MobileError;

pub(crate) fn parse_id(value: &str) -> Result<EntityId, MobileError> {
    value.parse().map_err(|_| MobileError::InvalidId { value: value.to_owned() })
}

/// "2026-11-24".
pub(crate) fn parse_iso_day(value: &str) -> Result<NaiveDate, MobileError> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| MobileError::InvalidDate { value: value.to_owned() })
}

pub(crate) fn iso_day(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}
