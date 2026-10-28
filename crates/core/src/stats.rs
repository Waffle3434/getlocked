//! Local session history in SQLite. Nothing here ever leaves the PC.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::model::{SessionKind, Strictness};
use crate::session::Ended;

pub use rusqlite::Error as StatsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockKind {
    App,
    Site,
}

impl BlockKind {
    fn as_str(self) -> &'static str {
        match self {
            BlockKind::App => "app",
            BlockKind::Site => "site",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    pub date: NaiveDate,
    pub focus_secs: u64,
    pub sessions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedTarget {
    pub kind: String,
    pub target: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: String,
    pub label: Option<String>,
    pub kind: String,
    pub strictness: String,
    pub started_at: String,
    pub focus_secs: u64,
    pub overtime_secs: u64,
    pub outcome: String,
    pub blocked_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub days: Vec<DayStat>,
    pub total_focus_secs: u64,
    pub completed: u32,
    pub ended_early: u32,
    pub current_streak: u32,
    pub best_streak: u32,
    pub top_blocked: Vec<BlockedTarget>,
    pub recent: Vec<SessionRow>,
}

pub struct Stats {
    conn: Connection,
}

fn kind_str(kind: &SessionKind) -> &'static str {
    match kind {
        SessionKind::Timed { .. } => "timed",
        SessionKind::Stopwatch { .. } => "stopwatch",
    }
}

fn strictness_str(s: Strictness) -> &'static str {
    match s {
        Strictness::Easy => "easy",
        Strictness::Friction => "friction",
        Strictness::Hard => "hard",
    }
}

impl Stats {
    pub fn open(path: &Path) -> Result<Self, StatsError> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, StatsError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StatsError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                label TEXT,
                kind TEXT NOT NULL,
                strictness TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT NOT NULL,
                local_date TEXT NOT NULL,
                focus_secs INTEGER NOT NULL,
                overtime_secs INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                ended_with_code INTEGER NOT NULL,
                blocked_count INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                at TEXT NOT NULL,
                local_date TEXT NOT NULL,
                kind TEXT NOT NULL,
                target TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS sessions_date ON sessions(local_date);
            CREATE INDEX IF NOT EXISTS blocks_date ON blocks(local_date);",
        )?;
        Ok(Self { conn })
    }

    /// `local_date` is the user's local date when the session started.
    pub fn record_session(&self, ended: &Ended, local_date: NaiveDate) -> Result<(), StatsError> {
        let s = &ended.session;
        self.conn.execute(
            "INSERT OR REPLACE INTO sessions VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                s.id,
                s.config.label,
                kind_str(&s.config.kind),
                strictness_str(s.config.strictness),
                s.started_at.to_rfc3339(),
                ended.ended_at.to_rfc3339(),
                local_date.to_string(),
                ended.focus_secs() as i64,
                ended.overtime_secs() as i64,
                ended.outcome.as_str(),
                ended.ended_with_code,
                s.blocked_count,
            ],
        )?;
        Ok(())
    }

    pub fn record_block(
        &self,
        session_id: Option<&str>,
        kind: BlockKind,
        target: &str,
        at: DateTime<Utc>,
        local_date: NaiveDate,
    ) -> Result<(), StatsError> {
        self.conn.execute(
            "INSERT INTO blocks (session_id, at, local_date, kind, target) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, at.to_rfc3339(), local_date.to_string(), kind.as_str(), target],
        )?;
        Ok(())
    }

    /// Stats for the `days` days ending on `today`, plus all-time totals.
    pub fn summary(&self, today: NaiveDate, days: u32) -> Result<Summary, StatsError> {
        let days = days.max(1);
        let first = today - Duration::days(i64::from(days) - 1);

        let mut per_day: BTreeMap<NaiveDate, (u64, u32)> = BTreeMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT local_date, SUM(focus_secs), COUNT(*) FROM sessions
             WHERE local_date >= ?1 AND local_date <= ?2 GROUP BY local_date",
        )?;
        let rows = stmt.query_map(params![first.to_string(), today.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (date, secs, count) = row?;
            if let Ok(d) = date.parse::<NaiveDate>() {
                per_day.insert(d, (secs as u64, count as u32));
            }
        }
        let day_stats = (0..days)
            .map(|i| {
                let date = first + Duration::days(i64::from(i));
                let (focus_secs, sessions) = per_day.get(&date).copied().unwrap_or((0, 0));
                DayStat { date, focus_secs, sessions }
            })
            .collect();

        let (total_focus_secs, completed, ended_early): (i64, i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(focus_secs), 0),
                    COALESCE(SUM(outcome = 'completed'), 0),
                    COALESCE(SUM(outcome = 'endedEarly'), 0)
             FROM sessions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        let mut stmt = self.conn.prepare("SELECT DISTINCT local_date FROM sessions WHERE outcome = 'completed'")?;
        let completed_days: BTreeSet<NaiveDate> =
            stmt.query_map([], |r| r.get::<_, String>(0))?.filter_map(|d| d.ok()?.parse().ok()).collect();
        let (current_streak, best_streak) = streaks(&completed_days, today);

        let mut stmt = self.conn.prepare(
            "SELECT kind, target, COUNT(*) AS n FROM blocks WHERE local_date >= ?1
             GROUP BY kind, target ORDER BY n DESC, target LIMIT 8",
        )?;
        let top_blocked = stmt
            .query_map(params![first.to_string()], |r| {
                Ok(BlockedTarget { kind: r.get(0)?, target: r.get(1)?, count: r.get::<_, i64>(2)? as u32 })
            })?
            .collect::<Result<_, _>>()?;

        let mut stmt = self.conn.prepare(
            "SELECT id, label, kind, strictness, started_at, focus_secs, overtime_secs, outcome, blocked_count
             FROM sessions ORDER BY started_at DESC LIMIT 10",
        )?;
        let recent = stmt
            .query_map([], |r| {
                Ok(SessionRow {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    kind: r.get(2)?,
                    strictness: r.get(3)?,
                    started_at: r.get(4)?,
                    focus_secs: r.get::<_, i64>(5)? as u64,
                    overtime_secs: r.get::<_, i64>(6)? as u64,
                    outcome: r.get(7)?,
                    blocked_count: r.get::<_, i64>(8)? as u32,
                })
            })?
            .collect::<Result<_, _>>()?;

        Ok(Summary {
            days: day_stats,
            total_focus_secs: total_focus_secs as u64,
            completed: completed as u32,
            ended_early: ended_early as u32,
            current_streak,
            best_streak,
            top_blocked,
            recent,
        })
    }
}

/// (current, best) runs of consecutive days. The current streak is still
/// alive if the last completed day was today or yesterday.
pub fn streaks(days: &BTreeSet<NaiveDate>, today: NaiveDate) -> (u32, u32) {
    let mut best = 0;
    let mut run = 0;
    let mut prev: Option<NaiveDate> = None;
    for &d in days {
        run = match prev {
            Some(p) if d - p == Duration::days(1) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(d);
    }
    let current = match prev {
        Some(last) if last == today || last == today - Duration::days(1) => run,
        _ => 0,
    };
    (current, best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SessionConfig;
    use crate::session::{Outcome, Phase, Session};

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn ended(id: &str, focus_secs: u64, outcome: Outcome) -> Ended {
        Ended {
            session: Session {
                id: id.into(),
                config: SessionConfig {
                    label: Some("Study".into()),
                    kind: SessionKind::Stopwatch { min_secs: None },
                    strictness: Strictness::Friction,
                    apps: Default::default(),
                    sites: Default::default(),
                    pomodoro: None,
                    require_extension: true,
                },
                started_at: Utc::now(),
                focus_ms: focus_secs * 1000,
                phase: Phase::Focus,
                phase_ms: 0,
                overtime: false,
                blocked_count: 2,
            },
            outcome,
            ended_with_code: false,
            ended_at: Utc::now(),
        }
    }

    #[test]
    fn streak_math() {
        let set = |v: &[u32]| v.iter().map(|&x| d(x)).collect::<BTreeSet<_>>();
        assert_eq!(streaks(&set(&[]), d(28)), (0, 0));
        assert_eq!(streaks(&set(&[26, 27, 28]), d(28)), (3, 3));
        // Yesterday still counts as alive.
        assert_eq!(streaks(&set(&[26, 27]), d(28)), (2, 2));
        // Broken streak.
        assert_eq!(streaks(&set(&[20, 21, 22, 23, 27]), d(28)), (1, 4));
        assert_eq!(streaks(&set(&[20, 21]), d(28)), (0, 2));
    }

    #[test]
    fn summary_aggregates_sessions_and_blocks() {
        let stats = Stats::in_memory().unwrap();
        stats.record_session(&ended("a", 1500, Outcome::Completed), d(27)).unwrap();
        stats.record_session(&ended("b", 600, Outcome::EndedEarly), d(28)).unwrap();
        stats.record_session(&ended("c", 900, Outcome::Completed), d(28)).unwrap();
        for _ in 0..3 {
            stats.record_block(Some("c"), BlockKind::Site, "youtube.com", Utc::now(), d(28)).unwrap();
        }
        stats.record_block(Some("c"), BlockKind::App, "steam.exe", Utc::now(), d(28)).unwrap();

        let s = stats.summary(d(28), 7).unwrap();
        assert_eq!(s.days.len(), 7);
        assert_eq!(s.days[6], DayStat { date: d(28), focus_secs: 1500, sessions: 2 });
        assert_eq!(s.days[5].focus_secs, 1500);
        assert_eq!(s.days[0].focus_secs, 0);
        assert_eq!(s.total_focus_secs, 3000);
        assert_eq!((s.completed, s.ended_early), (2, 1));
        assert_eq!((s.current_streak, s.best_streak), (2, 2));
        assert_eq!(s.top_blocked[0].target, "youtube.com");
        assert_eq!(s.top_blocked[0].count, 3);
        assert_eq!(s.recent.len(), 3);
    }

    #[test]
    fn empty_db_summary_is_zeroes() {
        let s = Stats::in_memory().unwrap().summary(d(28), 3).unwrap();
        assert_eq!(s.total_focus_secs, 0);
        assert_eq!(s.current_streak, 0);
        assert!(s.recent.is_empty());
    }
}
