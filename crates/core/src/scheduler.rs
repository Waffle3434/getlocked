//! Which schedules should fire between two checks.

use chrono::{Datelike, Duration, NaiveDateTime, Timelike};

use crate::model::Schedule;

/// How late a missed scheduled start may still fire.
pub const CATCH_UP: Duration = Duration::minutes(15);

/// Schedules whose start time falls in `(last, now]`, in local time.
///
/// The caller checks every ~30s and passes the previous check time as `last`,
/// so each start is seen exactly once. A start missed by up to `CATCH_UP`
/// (the PC was asleep or the app was starting) still fires; older ones
/// are skipped rather than starting a surprise session hours late.
pub fn due(schedules: &[Schedule], last: NaiveDateTime, now: NaiveDateTime) -> Vec<&Schedule> {
    if now <= last {
        return Vec::new();
    }
    let last = last.max(now - CATCH_UP);
    schedules
        .iter()
        .filter(|s| s.enabled)
        .filter(|s| {
            let mut day = last.date();
            while day <= now.date() {
                let weekday = day.weekday().num_days_from_monday() as u8;
                if s.days.contains(&weekday) {
                    let minute = u32::from(s.start_minute);
                    if let Some(t) = day.and_hms_opt(minute / 60, minute % 60, 0) {
                        if t > last && t <= now {
                            return true;
                        }
                    }
                }
                day = day.succ_opt().expect("date overflow");
            }
            false
        })
        .collect()
}

/// Minutes after midnight for a local time, as stored in `Schedule`.
pub fn minute_of_day(t: NaiveDateTime) -> u16 {
    (t.hour() * 60 + t.minute()) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn sched(days: &[u8], start_minute: u16, enabled: bool) -> Schedule {
        Schedule { id: "s".into(), preset_id: "p".into(), days: days.to_vec(), start_minute, enabled }
    }

    /// 2026-10-26 is a Monday.
    fn at(day: u32, h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(h, m, s).unwrap()
    }

    #[test]
    fn fires_once_when_start_is_crossed() {
        let s = [sched(&[0], 9 * 60, true)];
        assert_eq!(due(&s, at(26, 8, 59, 40), at(26, 9, 0, 10)).len(), 1);
        assert_eq!(due(&s, at(26, 9, 0, 10), at(26, 9, 0, 40)).len(), 0);
        assert_eq!(due(&s, at(26, 8, 59, 0), at(26, 8, 59, 30)).len(), 0);
    }

    #[test]
    fn respects_weekday_and_enabled() {
        let tuesday_only = [sched(&[1], 9 * 60, true)];
        assert!(due(&tuesday_only, at(26, 8, 59, 0), at(26, 9, 1, 0)).is_empty());
        assert_eq!(due(&tuesday_only, at(27, 8, 59, 0), at(27, 9, 1, 0)).len(), 1);
        let disabled = [sched(&[0], 9 * 60, false)];
        assert!(due(&disabled, at(26, 8, 59, 0), at(26, 9, 1, 0)).is_empty());
    }

    #[test]
    fn handles_midnight() {
        let s = [sched(&[1], 0, true)];
        assert_eq!(due(&s, at(26, 23, 59, 50), at(27, 0, 0, 20)).len(), 1);
    }

    #[test]
    fn does_not_replay_old_starts_after_long_gap() {
        let s = [sched(&[0, 1, 2, 3, 4, 5, 6], 9 * 60, true)];
        // Off for a week, back on at 8am: yesterday's 9am start is long gone.
        assert!(due(&s, at(19, 8, 0, 0), at(26, 8, 0, 0)).is_empty());
        // Off since 8am, back at 9:30: too late, skip it.
        assert!(due(&s, at(26, 8, 0, 0), at(26, 9, 30, 0)).is_empty());
        // Back at 9:10: still within the catch-up window.
        assert_eq!(due(&s, at(26, 8, 0, 0), at(26, 9, 10, 0)).len(), 1);
    }

    #[test]
    fn minute_of_day_works() {
        assert_eq!(minute_of_day(at(26, 13, 37, 59)), 13 * 60 + 37);
    }
}
