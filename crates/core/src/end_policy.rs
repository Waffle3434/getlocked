//! Whether a session can be ended right now, and what it takes.

use serde::Serialize;

use crate::model::{SessionKind, Strictness};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EndPolicy {
    /// End immediately.
    Free,
    /// End only by typing an unlock code.
    NeedsCode,
    /// Can't end until `focus_secs` of focus time have passed. After that
    /// it needs the unlock code if `then_needs_code`, otherwise it's free.
    LockedUntil { focus_secs: u64, then_needs_code: bool },
}

/// `goal_reached` is true once a timed session's timer has run out (including
/// when the user chose to keep going), which always frees the session.
pub fn end_policy(kind: &SessionKind, strictness: Strictness, focus_secs: u64, goal_reached: bool) -> EndPolicy {
    if goal_reached {
        return EndPolicy::Free;
    }
    match (kind, strictness) {
        (_, Strictness::Easy) => EndPolicy::Free,
        (_, Strictness::Friction) => EndPolicy::NeedsCode,
        (SessionKind::Timed { duration_secs }, Strictness::Hard) => {
            EndPolicy::LockedUntil { focus_secs: *duration_secs, then_needs_code: false }
        }
        (SessionKind::Stopwatch { min_secs }, Strictness::Hard) => match min_secs {
            Some(min) if focus_secs < *min => EndPolicy::LockedUntil { focus_secs: *min, then_needs_code: true },
            _ => EndPolicy::NeedsCode,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EndPolicy::*;
    use Strictness::*;

    const TIMED: SessionKind = SessionKind::Timed { duration_secs: 3600 };
    const WATCH: SessionKind = SessionKind::Stopwatch { min_secs: None };
    const WATCH_MIN: SessionKind = SessionKind::Stopwatch { min_secs: Some(600) };

    #[test]
    fn easy_is_always_free() {
        assert_eq!(end_policy(&TIMED, Easy, 0, false), Free);
        assert_eq!(end_policy(&WATCH, Easy, 0, false), Free);
    }

    #[test]
    fn friction_needs_code_until_goal_reached() {
        assert_eq!(end_policy(&TIMED, Friction, 10, false), NeedsCode);
        assert_eq!(end_policy(&WATCH, Friction, 10, false), NeedsCode);
        assert_eq!(end_policy(&TIMED, Friction, 3600, true), Free);
    }

    #[test]
    fn hard_timed_is_locked_until_the_timer_ends() {
        assert_eq!(end_policy(&TIMED, Hard, 100, false), LockedUntil { focus_secs: 3600, then_needs_code: false });
        assert_eq!(end_policy(&TIMED, Hard, 3600, true), Free);
    }

    #[test]
    fn hard_stopwatch_needs_minimum_then_code() {
        assert_eq!(end_policy(&WATCH_MIN, Hard, 599, false), LockedUntil { focus_secs: 600, then_needs_code: true });
        assert_eq!(end_policy(&WATCH_MIN, Hard, 600, false), NeedsCode);
        assert_eq!(end_policy(&WATCH, Hard, 0, false), NeedsCode);
    }
}
