//! Turns the caller's `Moment` into the instant and calendar the domain works with.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;

use crate::error::MobileError;
use crate::types::Moment;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Now {
    pub at: DateTime<Utc>,
    pub tz: Tz,
}

impl Now {
    pub fn parse(moment: &Moment) -> Result<Now, MobileError> {
        let tz: Tz =
            moment.time_zone.parse().map_err(|_| MobileError::InvalidTimeZone { name: moment.time_zone.clone() })?;
        let at = DateTime::from_timestamp_millis(moment.epoch_ms)
            .ok_or(MobileError::InvalidMoment { epoch_ms: moment.epoch_ms })?;
        Ok(Now { at, tz })
    }

    pub fn today(&self) -> NaiveDate {
        self.at.with_timezone(&self.tz).date_naive()
    }

    /// The instant `day` starts in this time zone: the earlier one in a DST fold, the end of the
    /// gap where clocks skip midnight (e.g. America/Santiago).
    pub fn midnight(&self, day: NaiveDate) -> DateTime<Utc> {
        self.instant(day.and_time(NaiveTime::MIN))
    }

    /// The instant a local wall-clock time names: the earlier one in a DST fold; a skipped time
    /// reads an hour later (02:15 → 03:15 summer time; a skipped midnight is the jump).
    pub fn instant(&self, local: NaiveDateTime) -> DateTime<Utc> {
        self.resolve(local).or_else(|| self.gap_end(local)).unwrap_or_else(|| local.and_utc())
    }

    /// `at` on the local wall clock.
    pub fn local(&self, at: DateTime<Utc>) -> NaiveDateTime {
        at.with_timezone(&self.tz).naive_local()
    }

    fn resolve(&self, local: NaiveDateTime) -> Option<DateTime<Utc>> {
        self.tz.from_local_datetime(&local).earliest().map(|t| t.with_timezone(&Utc))
    }

    /// A skipped local time, read with the offset from just before the skip, is the instant
    /// the clocks jumped.
    fn gap_end(&self, skipped: NaiveDateTime) -> Option<DateTime<Utc>> {
        let hour = TimeDelta::hours(1);
        self.resolve(skipped - hour).map(|before| before + hour)
    }
}
