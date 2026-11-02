//! The session state machine.
//!
//! Time is fed in by the caller as deltas from a monotonic clock (`tick`), so
//! changing the system clock can't skip time, and the engine can be tested
//! without sleeping. Only focus time counts toward a timed session.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::end_policy::{end_policy, EndPolicy};
use crate::model::{SessionConfig, SessionKind};
use crate::tasks::{Task, TaskError, TaskList};
use crate::unlock::{code_matches, UnlockChallenge};

/// Ticks longer than this are clamped. A huge gap means the PC was asleep,
/// and sleeping shouldn't count as focus time.
pub const MAX_TICK_MS: u64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Blocks are on and focus time is counting.
    Focus,
    /// Pomodoro break: blocks are lifted.
    Break,
    /// A timed session ran out. Blocks stay on until the user picks
    /// "Done" or "Keep going".
    TimeUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Completed,
    EndedEarly,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::EndedEarly => "endedEarly",
        }
    }
}

/// A running session. This is what gets saved to `session.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub config: SessionConfig,
    pub started_at: DateTime<Utc>,
    pub focus_ms: u64,
    pub phase: Phase,
    pub phase_ms: u64,
    /// A timed session the user chose to keep going on after time ran out.
    pub overtime: bool,
    pub blocked_count: u32,
    /// What the user means to get done. Empty for sessions without tasks.
    #[serde(default)]
    pub tasks: TaskList,
}

impl Session {
    fn duration_ms(&self) -> Option<u64> {
        match self.config.kind {
            SessionKind::Timed { duration_secs } => Some(duration_secs * 1000),
            SessionKind::Stopwatch { .. } => None,
        }
    }

    /// True once a timed session's timer has run out.
    pub fn goal_reached(&self) -> bool {
        self.phase == Phase::TimeUp || self.overtime
    }

    /// Blocks are enforced in Focus and TimeUp, lifted during breaks.
    pub fn blocking(&self) -> bool {
        self.phase != Phase::Break
    }

    pub fn end_policy(&self) -> EndPolicy {
        end_policy(
            &self.config.kind,
            self.config.strictness,
            self.focus_ms / 1000,
            self.goal_reached(),
            self.tasks.all_done(),
        )
    }

    /// Milliseconds until the current phase ends on its own, if it will.
    pub fn phase_ms_left(&self) -> Option<u64> {
        match self.phase {
            Phase::TimeUp => None,
            Phase::Break => self.config.pomodoro.map(|p| (p.break_secs * 1000).saturating_sub(self.phase_ms)),
            Phase::Focus => {
                let timer = self.duration_ms().filter(|_| !self.overtime).map(|d| d.saturating_sub(self.focus_ms));
                let pomo = self.config.pomodoro.map(|p| (p.focus_secs * 1000).saturating_sub(self.phase_ms));
                match (timer, pomo) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                }
            }
        }
    }

    fn outcome(&self) -> Outcome {
        if self.tasks.all_done() {
            return Outcome::Completed;
        }
        match self.config.kind {
            // A stopwatch has no target to fall short of.
            SessionKind::Stopwatch { .. } => Outcome::Completed,
            SessionKind::Timed { .. } if self.goal_reached() => Outcome::Completed,
            SessionKind::Timed { .. } => Outcome::EndedEarly,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineEvent {
    BreakStarted,
    FocusStarted,
    TimeUp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum UnlockStatus {
    None,
    CoolingDown { secs_left: u64 },
    Ready { code: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("a session is already running")]
    AlreadyRunning,
    #[error("no session is running")]
    NotRunning,
    #[error("this session can't be ended yet")]
    Locked,
    #[error("an unlock code is required")]
    CodeRequired,
    #[error("wait for the cooldown to finish")]
    CoolingDown,
    #[error("that code doesn't match; a new one was generated")]
    WrongCode,
    #[error("the timer hasn't run out")]
    NotTimeUp,
    #[error("invalid session: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Task(#[from] TaskError),
}

/// A finished session, ready to be written to stats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
    pub session: Session,
    pub outcome: Outcome,
    pub ended_with_code: bool,
    pub ended_at: DateTime<Utc>,
}

impl Ended {
    pub fn focus_secs(&self) -> u64 {
        self.session.focus_ms / 1000
    }

    pub fn overtime_secs(&self) -> u64 {
        match self.session.duration_ms() {
            Some(d) if self.session.overtime => self.session.focus_ms.saturating_sub(d) / 1000,
            _ => 0,
        }
    }
}

/// What the UI shows for the running session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub label: Option<String>,
    pub config: SessionConfig,
    pub started_at: DateTime<Utc>,
    pub phase: Phase,
    pub focus_secs: u64,
    /// Timed sessions only, until time runs out.
    pub remaining_secs: Option<u64>,
    pub phase_remaining_secs: Option<u64>,
    pub overtime: bool,
    pub overtime_secs: u64,
    pub blocking: bool,
    pub end_policy: EndPolicy,
    pub blocked_count: u32,
    pub tasks: Vec<Task>,
    pub all_tasks_done: bool,
}

#[derive(Debug, Default)]
pub struct Engine {
    session: Option<Session>,
    challenge: Option<UnlockChallenge>,
}

fn validate(config: &SessionConfig) -> Result<(), EngineError> {
    if let SessionKind::Timed { duration_secs } = config.kind {
        if duration_secs == 0 {
            return Err(EngineError::Invalid("duration must be more than 0"));
        }
    }
    if let Some(p) = config.pomodoro {
        if p.focus_secs == 0 || p.break_secs == 0 {
            return Err(EngineError::Invalid("pomodoro focus and break must be more than 0"));
        }
    }
    Ok(())
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resume a session loaded from disk (after a crash, kill or reboot).
    pub fn resume(session: Session) -> Self {
        Self { session: Some(session), challenge: None }
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn is_running(&self) -> bool {
        self.session.is_some()
    }

    pub fn blocking(&self) -> bool {
        self.session.as_ref().is_some_and(Session::blocking)
    }

    pub fn start(
        &mut self,
        config: SessionConfig,
        tasks: &[String],
        now: DateTime<Utc>,
    ) -> Result<&Session, EngineError> {
        if self.session.is_some() {
            return Err(EngineError::AlreadyRunning);
        }
        validate(&config)?;
        let tasks = TaskList::new(tasks)?;
        self.challenge = None;
        Ok(self.session.insert(Session {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            started_at: now,
            focus_ms: 0,
            phase: Phase::Focus,
            phase_ms: 0,
            overtime: false,
            blocked_count: 0,
            tasks,
        }))
    }

    pub fn record_block(&mut self) {
        if let Some(s) = &mut self.session {
            s.blocked_count += 1;
        }
    }

    fn tasks_mut(&mut self) -> Result<&mut TaskList, EngineError> {
        Ok(&mut self.session.as_mut().ok_or(EngineError::NotRunning)?.tasks)
    }

    pub fn add_task(&mut self, text: &str) -> Result<u32, EngineError> {
        Ok(self.tasks_mut()?.add(text)?)
    }

    pub fn edit_task(&mut self, id: u32, text: &str) -> Result<(), EngineError> {
        Ok(self.tasks_mut()?.edit(id, text)?)
    }

    pub fn tick_task(&mut self, id: u32, note: &str) -> Result<(), EngineError> {
        Ok(self.tasks_mut()?.tick(id, note)?)
    }

    pub fn untick_task(&mut self, id: u32) -> Result<(), EngineError> {
        Ok(self.tasks_mut()?.untick(id)?)
    }

    pub fn drop_task(&mut self, id: u32, reason: &str) -> Result<(), EngineError> {
        Ok(self.tasks_mut()?.drop_task(id, reason)?)
    }

    /// Advance by `delta_ms` of real time.
    pub fn tick(&mut self, delta_ms: u64) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        let Some(s) = &mut self.session else { return events };
        let d = delta_ms.min(MAX_TICK_MS);
        match s.phase {
            Phase::Focus => {
                s.focus_ms += d;
                s.phase_ms += d;
                let timer_done = s.duration_ms().is_some_and(|dur| !s.overtime && s.focus_ms >= dur);
                if timer_done {
                    s.focus_ms = s.duration_ms().unwrap_or(s.focus_ms);
                    s.phase = Phase::TimeUp;
                    s.phase_ms = 0;
                    events.push(EngineEvent::TimeUp);
                } else if s.config.pomodoro.is_some_and(|p| s.phase_ms >= p.focus_secs * 1000) {
                    s.phase = Phase::Break;
                    s.phase_ms = 0;
                    events.push(EngineEvent::BreakStarted);
                }
            }
            Phase::Break => {
                s.phase_ms += d;
                if s.config.pomodoro.is_none_or(|p| s.phase_ms >= p.break_secs * 1000) {
                    s.phase = Phase::Focus;
                    s.phase_ms = 0;
                    events.push(EngineEvent::FocusStarted);
                }
            }
            Phase::TimeUp => s.phase_ms += d,
        }
        events
    }

    /// The user answered the "time's up" prompt.
    pub fn time_up_choice(&mut self, keep_going: bool, now: DateTime<Utc>) -> Result<Option<Ended>, EngineError> {
        let s = self.session.as_mut().ok_or(EngineError::NotRunning)?;
        if s.phase != Phase::TimeUp {
            return Err(EngineError::NotTimeUp);
        }
        if keep_going {
            s.overtime = true;
            s.phase = Phase::Focus;
            s.phase_ms = 0;
            Ok(None)
        } else {
            Ok(Some(self.finish(false, now)))
        }
    }

    pub fn begin_unlock(&mut self, now_ms: u64) -> Result<UnlockStatus, EngineError> {
        let s = self.session.as_ref().ok_or(EngineError::NotRunning)?;
        match s.end_policy() {
            EndPolicy::NeedsCode => {
                if self.challenge.is_none() {
                    self.challenge = Some(UnlockChallenge::new(now_ms));
                }
                Ok(self.unlock_status(now_ms))
            }
            EndPolicy::Free => Ok(UnlockStatus::None),
            EndPolicy::LockedUntil { .. } => Err(EngineError::Locked),
        }
    }

    pub fn cancel_unlock(&mut self) {
        self.challenge = None;
    }

    pub fn unlock_status(&self, now_ms: u64) -> UnlockStatus {
        match &self.challenge {
            None => UnlockStatus::None,
            Some(c) if c.secs_left(now_ms) > 0 => UnlockStatus::CoolingDown { secs_left: c.secs_left(now_ms) },
            Some(c) => UnlockStatus::Ready { code: c.code.clone() },
        }
    }

    /// Try to end the session. `code` is required when the policy says so.
    pub fn end(&mut self, code: Option<&str>, now_ms: u64, now: DateTime<Utc>) -> Result<Ended, EngineError> {
        let s = self.session.as_ref().ok_or(EngineError::NotRunning)?;
        match s.end_policy() {
            EndPolicy::Free => Ok(self.finish(false, now)),
            EndPolicy::LockedUntil { .. } => Err(EngineError::Locked),
            EndPolicy::NeedsCode => {
                let challenge = self.challenge.as_mut().ok_or(EngineError::CodeRequired)?;
                if challenge.secs_left(now_ms) > 0 {
                    return Err(EngineError::CoolingDown);
                }
                let typed = code.ok_or(EngineError::CodeRequired)?;
                if !code_matches(&challenge.code, typed) {
                    challenge.code = crate::unlock::generate_code();
                    return Err(EngineError::WrongCode);
                }
                Ok(self.finish(true, now))
            }
        }
    }

    fn finish(&mut self, ended_with_code: bool, now: DateTime<Utc>) -> Ended {
        self.challenge = None;
        let session = self.session.take().expect("finish called without a session");
        let outcome = session.outcome();
        Ended { session, outcome, ended_with_code, ended_at: now }
    }

    pub fn view(&self) -> Option<SessionView> {
        let s = self.session.as_ref()?;
        let remaining_secs =
            s.duration_ms().filter(|_| !s.goal_reached()).map(|d| d.saturating_sub(s.focus_ms).div_ceil(1000));
        let overtime_secs = match s.duration_ms() {
            Some(d) if s.overtime => s.focus_ms.saturating_sub(d) / 1000,
            _ => 0,
        };
        Some(SessionView {
            id: s.id.clone(),
            label: s.config.label.clone(),
            config: s.config.clone(),
            started_at: s.started_at,
            phase: s.phase,
            focus_secs: s.focus_ms / 1000,
            remaining_secs,
            phase_remaining_secs: s.phase_ms_left().map(|ms| ms.div_ceil(1000)),
            overtime: s.overtime,
            overtime_secs,
            blocking: s.blocking(),
            end_policy: s.end_policy(),
            blocked_count: s.blocked_count,
            tasks: s.tasks.tasks().to_vec(),
            all_tasks_done: s.tasks.all_done(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PomodoroCfg, Strictness};
    use crate::tasks::TaskError;

    fn cfg(kind: SessionKind, strictness: Strictness) -> SessionConfig {
        SessionConfig {
            label: None,
            kind,
            strictness,
            apps: Default::default(),
            sites: Default::default(),
            pomodoro: None,
            require_extension: true,
        }
    }

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    /// Advance in 1s steps, like the real ticker.
    fn run(e: &mut Engine, secs: u64) -> Vec<EngineEvent> {
        (0..secs).flat_map(|_| e.tick(1000)).collect()
    }

    #[test]
    fn cannot_start_twice_or_with_zero_duration() {
        let mut e = Engine::new();
        assert_eq!(
            e.start(cfg(SessionKind::Timed { duration_secs: 0 }, Strictness::Easy), &[], now()).unwrap_err(),
            EngineError::Invalid("duration must be more than 0")
        );
        e.start(cfg(SessionKind::Timed { duration_secs: 5 }, Strictness::Easy), &[], now()).unwrap();
        assert_eq!(
            e.start(cfg(SessionKind::Timed { duration_secs: 5 }, Strictness::Easy), &[], now()).unwrap_err(),
            EngineError::AlreadyRunning
        );
    }

    #[test]
    fn timed_session_reaches_time_up_and_keeps_blocking() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 3 }, Strictness::Hard), &[], now()).unwrap();
        assert_eq!(e.view().unwrap().remaining_secs, Some(3));
        assert_eq!(run(&mut e, 3), vec![EngineEvent::TimeUp]);
        let v = e.view().unwrap();
        assert_eq!(v.phase, Phase::TimeUp);
        assert!(v.blocking);
        assert_eq!(v.end_policy, EndPolicy::Free);
        // Time up doesn't keep adding focus time while waiting for an answer.
        run(&mut e, 10);
        assert_eq!(e.view().unwrap().focus_secs, 3);
    }

    #[test]
    fn time_up_done_completes() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 2 }, Strictness::Hard), &[], now()).unwrap();
        run(&mut e, 2);
        let ended = e.time_up_choice(false, now()).unwrap().unwrap();
        assert_eq!(ended.outcome, Outcome::Completed);
        assert!(!e.is_running());
    }

    #[test]
    fn time_up_keep_going_becomes_free_stopwatch_with_overtime() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 2 }, Strictness::Hard), &[], now()).unwrap();
        run(&mut e, 2);
        assert!(e.time_up_choice(true, now()).unwrap().is_none());
        run(&mut e, 5);
        let v = e.view().unwrap();
        assert_eq!(v.phase, Phase::Focus);
        assert_eq!(v.remaining_secs, None);
        assert_eq!(v.overtime_secs, 5);
        assert_eq!(v.end_policy, EndPolicy::Free);
        let ended = e.end(None, 0, now()).unwrap();
        assert_eq!(ended.outcome, Outcome::Completed);
        assert_eq!(ended.focus_secs(), 7);
        assert_eq!(ended.overtime_secs(), 5);
    }

    #[test]
    fn time_up_choice_requires_time_up() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 60 }, Strictness::Easy), &[], now()).unwrap();
        assert_eq!(e.time_up_choice(true, now()).unwrap_err(), EngineError::NotTimeUp);
    }

    #[test]
    fn easy_timed_can_end_early_and_counts_as_ended_early() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 60 }, Strictness::Easy), &[], now()).unwrap();
        run(&mut e, 10);
        let ended = e.end(None, 0, now()).unwrap();
        assert_eq!(ended.outcome, Outcome::EndedEarly);
        assert!(!ended.ended_with_code);
    }

    #[test]
    fn hard_timed_cannot_end_or_start_unlock() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 60 }, Strictness::Hard), &[], now()).unwrap();
        assert_eq!(e.end(None, 0, now()).unwrap_err(), EngineError::Locked);
        assert_eq!(e.begin_unlock(0).unwrap_err(), EngineError::Locked);
    }

    #[test]
    fn friction_unlock_flow_cooldown_then_code() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 600 }, Strictness::Friction), &[], now()).unwrap();
        assert_eq!(e.end(None, 0, now()).unwrap_err(), EngineError::CodeRequired);

        let t0 = 10_000;
        assert_eq!(e.begin_unlock(t0).unwrap(), UnlockStatus::CoolingDown { secs_left: 30 });
        assert_eq!(e.end(Some("x"), t0 + 1000, now()).unwrap_err(), EngineError::CoolingDown);

        let ready = t0 + 30_000;
        let UnlockStatus::Ready { code } = e.unlock_status(ready) else { panic!("not ready") };
        // A wrong attempt regenerates the code.
        assert_eq!(e.end(Some("nope"), ready, now()).unwrap_err(), EngineError::WrongCode);
        let UnlockStatus::Ready { code: new_code } = e.unlock_status(ready) else { panic!() };
        assert_ne!(code, new_code);
        assert_eq!(e.end(Some(&code), ready, now()).unwrap_err(), EngineError::WrongCode);

        let UnlockStatus::Ready { code } = e.unlock_status(ready) else { panic!() };
        let ended = e.end(Some(&code), ready, now()).unwrap();
        assert!(ended.ended_with_code);
        assert_eq!(ended.outcome, Outcome::EndedEarly);
    }

    #[test]
    fn begin_unlock_twice_keeps_the_same_cooldown() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Stopwatch { min_secs: None }, Strictness::Friction), &[], now()).unwrap();
        e.begin_unlock(0).unwrap();
        assert_eq!(e.begin_unlock(20_000).unwrap(), UnlockStatus::CoolingDown { secs_left: 10 });
    }

    #[test]
    fn hard_stopwatch_locked_until_minimum_then_code_and_completes() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Stopwatch { min_secs: Some(5) }, Strictness::Hard), &[], now()).unwrap();
        run(&mut e, 4);
        assert_eq!(e.begin_unlock(0).unwrap_err(), EngineError::Locked);
        run(&mut e, 1);
        e.begin_unlock(0).unwrap();
        let UnlockStatus::Ready { code } = e.unlock_status(30_000) else { panic!() };
        let ended = e.end(Some(&code), 30_000, now()).unwrap();
        assert_eq!(ended.outcome, Outcome::Completed);
        assert_eq!(ended.focus_secs(), 5);
    }

    #[test]
    fn stopwatch_counts_up_with_no_remaining() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Stopwatch { min_secs: None }, Strictness::Easy), &[], now()).unwrap();
        run(&mut e, 42);
        let v = e.view().unwrap();
        assert_eq!(v.focus_secs, 42);
        assert_eq!(v.remaining_secs, None);
        assert_eq!(v.phase_remaining_secs, None);
    }

    #[test]
    fn pomodoro_breaks_lift_blocks_and_dont_count_as_focus() {
        let mut c = cfg(SessionKind::Timed { duration_secs: 10 }, Strictness::Hard);
        c.pomodoro = Some(PomodoroCfg { focus_secs: 4, break_secs: 2 });
        let mut e = Engine::new();
        e.start(c, &[], now()).unwrap();
        assert_eq!(run(&mut e, 4), vec![EngineEvent::BreakStarted]);
        assert!(!e.blocking());
        assert_eq!(e.view().unwrap().phase_remaining_secs, Some(2));
        assert_eq!(run(&mut e, 2), vec![EngineEvent::FocusStarted]);
        assert!(e.blocking());
        assert_eq!(e.view().unwrap().focus_secs, 4);
        assert_eq!(run(&mut e, 4), vec![EngineEvent::BreakStarted]);
        run(&mut e, 2);
        // 8s of focus so far; the timer (2s left) ends before the next break.
        assert_eq!(e.view().unwrap().phase_remaining_secs, Some(2));
        assert_eq!(run(&mut e, 2), vec![EngineEvent::TimeUp]);
    }

    #[test]
    fn stopwatch_pomodoro_repeats_forever() {
        let mut c = cfg(SessionKind::Stopwatch { min_secs: None }, Strictness::Easy);
        c.pomodoro = Some(PomodoroCfg { focus_secs: 2, break_secs: 1 });
        let mut e = Engine::new();
        e.start(c, &[], now()).unwrap();
        let events = run(&mut e, 9);
        assert_eq!(events.iter().filter(|e| **e == EngineEvent::BreakStarted).count(), 3);
        assert_eq!(e.view().unwrap().focus_secs, 6);
    }

    #[test]
    fn long_gaps_are_clamped_so_sleep_does_not_count() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 3600 }, Strictness::Hard), &[], now()).unwrap();
        e.tick(8 * 3600 * 1000);
        assert_eq!(e.view().unwrap().focus_secs, MAX_TICK_MS / 1000);
    }

    #[test]
    fn resumed_session_continues_where_it_left_off() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 100 }, Strictness::Hard), &[], now()).unwrap();
        run(&mut e, 40);
        let json = serde_json::to_string(e.session().unwrap()).unwrap();
        let mut resumed = Engine::resume(serde_json::from_str(&json).unwrap());
        assert_eq!(resumed.view().unwrap().remaining_secs, Some(60));
        run(&mut resumed, 60);
        assert_eq!(resumed.view().unwrap().phase, Phase::TimeUp);
    }

    #[test]
    fn record_block_counts() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Stopwatch { min_secs: None }, Strictness::Easy), &[], now()).unwrap();
        e.record_block();
        e.record_block();
        assert_eq!(e.view().unwrap().blocked_count, 2);
    }

    fn tasks(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn finishing_all_tasks_ends_a_hard_timer_as_completed() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 3600 }, Strictness::Hard), &tasks(&["intro", "chart"]), now())
            .unwrap();
        assert_eq!(e.end(None, 0, now()).unwrap_err(), EngineError::Locked);
        e.tick_task(1, "wrote the intro").unwrap();
        assert!(!e.view().unwrap().all_tasks_done);
        e.tick_task(2, "fixed the chart").unwrap();
        let v = e.view().unwrap();
        assert!(v.all_tasks_done);
        assert_eq!(v.end_policy, EndPolicy::Free);
        let ended = e.end(None, 0, now()).unwrap();
        assert_eq!(ended.outcome, Outcome::Completed);
        assert!(!ended.ended_with_code);
    }

    #[test]
    fn dropping_every_task_falls_back_to_strictness() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Stopwatch { min_secs: None }, Strictness::Friction), &tasks(&["intro"]), now())
            .unwrap();
        e.drop_task(1, "not doing this today").unwrap();
        assert_eq!(e.view().unwrap().end_policy, EndPolicy::NeedsCode);
    }

    #[test]
    fn task_errors_surface_and_need_a_session() {
        let mut e = Engine::new();
        assert_eq!(e.add_task("x").unwrap_err(), EngineError::NotRunning);
        e.start(cfg(SessionKind::Stopwatch { min_secs: None }, Strictness::Easy), &[], now()).unwrap();
        let id = e.add_task("write intro").unwrap();
        assert_eq!(e.tick_task(id, "done").unwrap_err(), EngineError::Task(TaskError::NoteTooShort));
        e.edit_task(id, "write the intro").unwrap();
        e.untick_task(id).unwrap();
        assert_eq!(e.view().unwrap().tasks.len(), 1);
    }

    #[test]
    fn invalid_starting_tasks_refuse_to_start() {
        let mut e = Engine::new();
        let err = e.start(cfg(SessionKind::Stopwatch { min_secs: None }, Strictness::Easy), &tasks(&["  "]), now());
        assert_eq!(err.unwrap_err(), EngineError::Task(TaskError::InvalidText));
        assert!(!e.is_running());
    }

    #[test]
    fn tasks_survive_a_save_and_resume() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 60 }, Strictness::Hard), &tasks(&["intro"]), now()).unwrap();
        e.tick_task(1, "wrote the intro").unwrap();
        let json = serde_json::to_string(e.session().unwrap()).unwrap();
        let resumed = Engine::resume(serde_json::from_str(&json).unwrap());
        assert!(resumed.view().unwrap().all_tasks_done);
    }

    #[test]
    fn old_session_files_without_tasks_still_load() {
        let mut e = Engine::new();
        e.start(cfg(SessionKind::Timed { duration_secs: 60 }, Strictness::Easy), &[], now()).unwrap();
        let mut json: serde_json::Value = serde_json::to_value(e.session().unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("tasks");
        let s: Session = serde_json::from_value(json).unwrap();
        assert!(s.tasks.tasks().is_empty());
    }
}
