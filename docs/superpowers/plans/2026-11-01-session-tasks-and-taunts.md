# Session Tasks and Taunts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users list tasks for a session, end any session by finishing every task (each with a 3-word note), and get roasted with their own weekly record when they hit a block.

**Architecture:** All rules live in `crates/core`: a new `tasks` module owns the task list and its validation, `session`/`end_policy` use it to free the session, `taunt` builds the lines, and `stats` stores tasks and counts early ends. The desktop app adds Tauri commands and fills two new `SiteRulesMsg` fields. The extension validates them and shows the taunt on its blocked page. The React UI adds a task box, a checklist with note/reason dialogs, two settings and expandable stats rows.

**Tech Stack:** Rust (serde, rusqlite, chrono), Tauri 2, React + TypeScript (Vite, Tailwind), MV3 extension (esbuild, vitest).

**Spec:** `docs/superpowers/specs/2026-11-01-session-tasks-and-taunts-design.md`

## Global Constraints

- Windows 10/11. Run commands from `C:\Users\hirsh\code\getlocked`. In PowerShell use `npm.cmd`, not `npm` (script execution is disabled on this machine).
- At most 20 tasks per session, counting dropped tasks. Task text: 1–120 characters after trimming.
- Notes and drop reasons: at least 3 words (a word is a whitespace-separated run containing a letter or digit), at most 200 characters.
- "All done": at least one non-dropped task, and every non-dropped task is done. It frees the session at every strictness, including Hard, and the outcome is `Completed`.
- Editing a ticked task's text (to a different text) unticks it and clears its note. Dropped tasks can't be edited, ticked, unticked or restored.
- The week starts Monday 00:00 local time. Taunts are on by default (`roast: true`).
- Taunt template at most 300 characters. The extension replaces only `{site}` and renders it as text, never HTML.
- No protocol version bump: the new `SiteRulesMsg` fields default when missing.
- GetLocked never changes system settings or other apps (CONTRIBUTING.md).
- **Commits:** the user commits themselves. Each task ends with "Stop for review". Offer the commit, and don't run `git commit` unless the user says to.
- Before any `cargo` command in a fresh checkout, run `npm.cmd run ext:build` once (the app bundles the built extension).

## File Structure

| File | Responsibility |
|---|---|
| `crates/core/src/tasks.rs` (new) | `Task`, `TaskState`, `TaskList`, validation, `TaskError` |
| `crates/core/src/taunt.rs` (new) | Taunt templates, name and app-label helpers |
| `crates/core/src/session.rs` | Session holds a `TaskList`. Engine task methods; ending uses all-done |
| `crates/core/src/end_policy.rs` | New `all_tasks_done` input |
| `crates/core/src/stats.rs` | `session_tasks` table, `SessionRow.tasks`, `ended_early_since`, `week_start` |
| `crates/core/src/model.rs` | `Settings.display_name`, `Settings.roast` |
| `crates/core/src/protocol.rs`, `packages/protocol/src/index.ts` | `SiteRulesMsg.open_tasks`, `SiteRulesMsg.taunt` |
| `extension/src/rules.ts`, `extension/src/blocked.ts`, `extension/public/blocked.html` | Validate new fields; taunt headline and task list |
| `apps/desktop/src-tauri/src/runtime.rs`, `commands.rs`, `lib.rs` | Task commands, weekly count cache, taunt toasts, site rules |
| `apps/desktop/src/types.ts`, `api.ts` | Mirror types; new API calls |
| `apps/desktop/src/components/TaskInput.tsx` (new) | Start page task box |
| `apps/desktop/src/components/TaskChecklist.tsx` (new) | Session checklist with tick/edit/drop dialogs |
| `apps/desktop/src/pages/StartPage.tsx`, `SessionPage.tsx`, `SettingsPage.tsx`, `StatsPage.tsx` | Wire the above in |
| `docs/getting-started.md`, `docs/how-it-works.md` | User docs |

---

### Task 1: Task list rules (`tasks.rs`)

**Files:**
- Create: `crates/core/src/tasks.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod tasks;` after `pub mod stats;`)

**Interfaces:**
- Produces:
  - `pub struct TaskList` (serde-transparent `Vec<Task>`, `Default`)
  - `TaskList::new(&[String]) -> Result<TaskList, TaskError>`
  - `.tasks() -> &[Task]`
  - `.add(&str) -> Result<u32, TaskError>`
  - `.edit(u32, &str) -> Result<(), TaskError>`
  - `.tick(u32, &str) -> Result<(), TaskError>`
  - `.untick(u32) -> Result<(), TaskError>`
  - `.drop_task(u32, &str) -> Result<(), TaskError>`
  - `.all_done() -> bool`
  - `.open_texts() -> Vec<String>`
  - `pub struct Task { pub id: u32, pub text: String, pub state: TaskState }`
  - `pub enum TaskState { Open, Done { note: String }, Dropped { reason: String } }` (serde tag `type`, camelCase)
  - `pub enum TaskError { NotFound, TooMany, InvalidText, NoteTooShort, Dropped }`
  - `pub fn word_count(&str) -> usize`

- [ ] **Step 1: Write the failing tests.** Create `crates/core/src/tasks.rs` with only the test module below plus `pub mod tasks;` in `lib.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn list(texts: &[&str]) -> TaskList {
        TaskList::new(&texts.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn word_count_needs_letters_or_digits() {
        assert_eq!(word_count("wrote the intro"), 3);
        assert_eq!(word_count("  done  "), 1);
        assert_eq!(word_count("a - b . c"), 3);
        assert_eq!(word_count("- . ! ?"), 0);
    }

    #[test]
    fn new_trims_and_validates() {
        let l = list(&["  write intro ", "fix chart"]);
        assert_eq!(l.tasks()[0].text, "write intro");
        assert_eq!(l.tasks()[1].id, 2);
        assert_eq!(TaskList::new(&["   ".into()]).unwrap_err(), TaskError::InvalidText);
        assert_eq!(TaskList::new(&["x".repeat(121)]).unwrap_err(), TaskError::InvalidText);
        assert!(TaskList::new(&["x".repeat(120)]).is_ok());
    }

    #[test]
    fn at_most_twenty_tasks_counting_dropped() {
        let mut l = list(&[]);
        for i in 0..20 {
            l.add(&format!("task {i}")).unwrap();
        }
        assert_eq!(l.add("one more").unwrap_err(), TaskError::TooMany);
        l.drop_task(1, "not needed any more").unwrap();
        assert_eq!(l.add("one more").unwrap_err(), TaskError::TooMany);
    }

    #[test]
    fn tick_needs_three_words_and_at_most_200_chars() {
        let mut l = list(&["write intro"]);
        assert_eq!(l.tick(1, "done").unwrap_err(), TaskError::NoteTooShort);
        assert_eq!(l.tick(1, "did it").unwrap_err(), TaskError::NoteTooShort);
        assert_eq!(l.tick(1, &format!("a b {}", "c".repeat(200))).unwrap_err(), TaskError::NoteTooShort);
        l.tick(1, " wrote the intro ").unwrap();
        assert_eq!(l.tasks()[0].state, TaskState::Done { note: "wrote the intro".into() });
    }

    #[test]
    fn untick_clears_the_note() {
        let mut l = list(&["write intro"]);
        l.tick(1, "wrote the intro").unwrap();
        l.untick(1).unwrap();
        assert_eq!(l.tasks()[0].state, TaskState::Open);
    }

    #[test]
    fn editing_a_ticked_task_unticks_it_unless_text_is_unchanged() {
        let mut l = list(&["write intro"]);
        l.tick(1, "wrote the intro").unwrap();
        l.edit(1, "write intro").unwrap();
        assert!(matches!(l.tasks()[0].state, TaskState::Done { .. }));
        l.edit(1, "write intro and outro").unwrap();
        assert_eq!(l.tasks()[0].state, TaskState::Open);
        assert_eq!(l.tasks()[0].text, "write intro and outro");
    }

    #[test]
    fn drop_needs_a_reason_and_is_final() {
        let mut l = list(&["write intro", "fix chart"]);
        assert_eq!(l.drop_task(2, "nope").unwrap_err(), TaskError::NoteTooShort);
        l.drop_task(2, "chart is not needed").unwrap();
        assert_eq!(l.tick(2, "fixed the chart").unwrap_err(), TaskError::Dropped);
        assert_eq!(l.edit(2, "fix chart again").unwrap_err(), TaskError::Dropped);
        assert_eq!(l.untick(2).unwrap_err(), TaskError::Dropped);
        assert_eq!(l.drop_task(2, "dropping it twice now").unwrap_err(), TaskError::Dropped);
        assert_eq!(l.tick(9, "no such task here").unwrap_err(), TaskError::NotFound);
    }

    #[test]
    fn all_done_ignores_dropped_but_needs_one_remaining() {
        let mut l = list(&["write intro", "fix chart"]);
        assert!(!l.all_done());
        l.tick(1, "wrote the intro").unwrap();
        assert!(!l.all_done());
        l.drop_task(2, "chart is not needed").unwrap();
        assert!(l.all_done());

        let mut gone = list(&["write intro"]);
        gone.drop_task(1, "changed my mind entirely").unwrap();
        assert!(!gone.all_done());
        assert!(!list(&[]).all_done());
    }

    #[test]
    fn open_texts_lists_only_open_tasks() {
        let mut l = list(&["a task", "b task", "c task"]);
        l.tick(1, "finished task a").unwrap();
        l.drop_task(3, "task c not needed").unwrap();
        assert_eq!(l.open_texts(), vec!["b task".to_string()]);
    }

    #[test]
    fn serializes_as_a_plain_array() {
        let mut l = list(&["write intro"]);
        l.tick(1, "wrote the intro").unwrap();
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(
            json,
            r#"[{"id":1,"text":"write intro","state":{"type":"done","note":"wrote the intro"}}]"#
        );
        assert_eq!(serde_json::from_str::<TaskList>(&json).unwrap(), l);
    }
}
```

- [ ] **Step 2: Run to confirm it fails.** Run `cargo test -p getlocked-core tasks`. Expected: compile errors (`TaskList`, `word_count` not found).

- [ ] **Step 3: Implement.** Put this above the test module in `crates/core/src/tasks.rs`:

```rust
//! The tasks a user means to finish in a session, and the rules for changing
//! them. Notes and drop reasons need a few real words: friction, not proof.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_TASKS: usize = 20;
pub const MAX_TASK_LEN: usize = 120;
pub const MAX_NOTE_LEN: usize = 200;
pub const MIN_NOTE_WORDS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TaskState {
    Open,
    Done { note: String },
    /// Deleted mid-session. Kept for the stats; can't come back.
    Dropped { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: u32,
    pub text: String,
    pub state: TaskState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TaskError {
    #[error("that task no longer exists")]
    NotFound,
    #[error("a session can have at most 20 tasks")]
    TooMany,
    #[error("a task needs 1 to 120 characters")]
    InvalidText,
    #[error("write at least 3 words (up to 200 characters)")]
    NoteTooShort,
    #[error("that task was dropped; add it again instead")]
    Dropped,
}

/// Words that contain a letter or digit, so "- . !" doesn't count.
pub fn word_count(s: &str) -> usize {
    s.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
}

fn clean_text(text: &str) -> Result<String, TaskError> {
    let t = text.trim();
    if t.is_empty() || t.chars().count() > MAX_TASK_LEN {
        return Err(TaskError::InvalidText);
    }
    Ok(t.to_string())
}

fn clean_note(note: &str) -> Result<String, TaskError> {
    let n = note.trim();
    if word_count(n) < MIN_NOTE_WORDS || n.chars().count() > MAX_NOTE_LEN {
        return Err(TaskError::NoteTooShort);
    }
    Ok(n.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskList(Vec<Task>);

impl TaskList {
    pub fn new(texts: &[String]) -> Result<Self, TaskError> {
        let mut list = Self::default();
        for t in texts {
            list.add(t)?;
        }
        Ok(list)
    }

    pub fn tasks(&self) -> &[Task] {
        &self.0
    }

    pub fn add(&mut self, text: &str) -> Result<u32, TaskError> {
        if self.0.len() >= MAX_TASKS {
            return Err(TaskError::TooMany);
        }
        let text = clean_text(text)?;
        let id = self.0.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        self.0.push(Task { id, text, state: TaskState::Open });
        Ok(id)
    }

    /// A live (not dropped) task.
    fn live_mut(&mut self, id: u32) -> Result<&mut Task, TaskError> {
        let task = self.0.iter_mut().find(|t| t.id == id).ok_or(TaskError::NotFound)?;
        if matches!(task.state, TaskState::Dropped { .. }) {
            return Err(TaskError::Dropped);
        }
        Ok(task)
    }

    pub fn edit(&mut self, id: u32, text: &str) -> Result<(), TaskError> {
        let text = clean_text(text)?;
        let task = self.live_mut(id)?;
        if task.text != text {
            task.text = text;
            task.state = TaskState::Open;
        }
        Ok(())
    }

    pub fn tick(&mut self, id: u32, note: &str) -> Result<(), TaskError> {
        let note = clean_note(note)?;
        self.live_mut(id)?.state = TaskState::Done { note };
        Ok(())
    }

    pub fn untick(&mut self, id: u32) -> Result<(), TaskError> {
        self.live_mut(id)?.state = TaskState::Open;
        Ok(())
    }

    pub fn drop_task(&mut self, id: u32, reason: &str) -> Result<(), TaskError> {
        let reason = clean_note(reason)?;
        self.live_mut(id)?.state = TaskState::Dropped { reason };
        Ok(())
    }

    /// At least one task is left, and every task that's left is done.
    pub fn all_done(&self) -> bool {
        let mut live = self.0.iter().filter(|t| !matches!(t.state, TaskState::Dropped { .. })).peekable();
        live.peek().is_some() && live.all(|t| matches!(t.state, TaskState::Done { .. }))
    }

    pub fn open_texts(&self) -> Vec<String> {
        self.0.iter().filter(|t| t.state == TaskState::Open).map(|t| t.text.clone()).collect()
    }
}
```

- [ ] **Step 4: Run the tests.** Run `cargo test -p getlocked-core tasks`. Expected: 10 passed.
- [ ] **Step 5: Stop for review.** Offer the commit: "Add task list rules to the core". Don't commit unless the user says to.

---

### Task 2: Sessions use tasks to end (`session.rs`, `end_policy.rs`)

**Files:**
- Modify: `crates/core/src/end_policy.rs` (the `end_policy` function and its tests)
- Modify: `crates/core/src/session.rs` (`Session`, `SessionView`, `EngineError`, `Engine::start`, new task methods, `outcome`, `view`, tests)
- Modify: `crates/core/src/stats.rs` (test helper `ended()` builds a `Session` literal: add `tasks: Default::default(),`)

**Interfaces:**
- Consumes: `TaskList`, `Task`, `TaskError` from Task 1.
- Produces:
  - `end_policy(kind: &SessionKind, strictness: Strictness, focus_secs: u64, goal_reached: bool, all_tasks_done: bool) -> EndPolicy`
  - `Session.tasks: TaskList` (`#[serde(default)]`)
  - `Engine::start(&mut self, config: SessionConfig, tasks: &[String], now: DateTime<Utc>)`
  - `Engine::add_task(&str) -> Result<u32, EngineError>`
  - `edit_task(u32, &str)`, `tick_task(u32, &str)`, `untick_task(u32)` and `drop_task(u32, &str)`, each returning `Result<(), EngineError>`
  - `EngineError::Task(TaskError)`
  - `SessionView.tasks: Vec<Task>` and `SessionView.all_tasks_done: bool` (camelCase `allTasksDone`)

- [ ] **Step 1: Write the failing tests.**
  - **`end_policy.rs`:** add a fifth argument, `false`, to every existing `end_policy(...)` call in its tests. For example, `end_policy(&TIMED, Easy, 0, false)` becomes `end_policy(&TIMED, Easy, 0, false, false)`. Then add:

```rust
    #[test]
    fn finishing_every_task_frees_any_session() {
        for s in [Easy, Friction, Hard] {
            assert_eq!(end_policy(&TIMED, s, 0, false, true), Free);
            assert_eq!(end_policy(&WATCH_MIN, s, 0, false, true), Free);
        }
    }
```

  - **`session.rs`:** in its tests, give every `e.start(<config>, now())` call an empty task list: `e.start(<config>, &[], now())`. That includes the calls that pass a variable such as `c`. Then add:

```rust
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
```

  Add `use crate::tasks::TaskError;` to the session test module's imports.

- [ ] **Step 2: Run to confirm it fails.** Run `cargo test -p getlocked-core`. Expected: compile errors (wrong argument counts, missing methods).

- [ ] **Step 3: Implement.**
  - **`end_policy.rs`:** replace the function's signature and first statement:

```rust
/// `goal_reached` is true once a timed session's timer has run out (including
/// when the user chose to keep going). `all_tasks_done` is true when every
/// remaining task is ticked. Either one frees the session.
pub fn end_policy(
    kind: &SessionKind,
    strictness: Strictness,
    focus_secs: u64,
    goal_reached: bool,
    all_tasks_done: bool,
) -> EndPolicy {
    if goal_reached || all_tasks_done {
        return EndPolicy::Free;
    }
```

  - **`session.rs`, imports:** add `use crate::tasks::{Task, TaskError, TaskList};`.
  - **`session.rs`, `Session` struct:** add the field after `blocked_count`:

```rust
    /// What the user means to get done. Empty for sessions without tasks.
    #[serde(default)]
    pub tasks: TaskList,
```

  - **`session.rs`, `Session::end_policy`:**

```rust
    pub fn end_policy(&self) -> EndPolicy {
        end_policy(
            &self.config.kind,
            self.config.strictness,
            self.focus_ms / 1000,
            self.goal_reached(),
            self.tasks.all_done(),
        )
    }
```

  - **`session.rs`, `Session::outcome`:**

```rust
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
```

  - **`session.rs`, `EngineError`:** add a variant:

```rust
    #[error(transparent)]
    Task(#[from] TaskError),
```

  - **`session.rs`, `SessionView`:** add after `blocked_count`:

```rust
    pub tasks: Vec<Task>,
    pub all_tasks_done: bool,
```

  - **`session.rs`, `Engine::start`:** replace with:

```rust
    pub fn start(&mut self, config: SessionConfig, tasks: &[String], now: DateTime<Utc>) -> Result<&Session, EngineError> {
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
```

  - **`session.rs`, new `Engine` methods:** add after `record_block`:

```rust
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
```

  - **`session.rs`, `Engine::view`:** add to the struct literal:

```rust
            tasks: s.tasks.tasks().to_vec(),
            all_tasks_done: s.tasks.all_done(),
```

  - **`stats.rs`, test helper `ended()`:** add `tasks: Default::default(),` to the `Session { ... }` literal.

- [ ] **Step 4: Run the tests.** Run `cargo test -p getlocked-core`. Expected: all pass, including the 7 new tests.
- [ ] **Step 5: Stop for review.** Offer the commit: "Let finished tasks end any session". Note that `apps/desktop` won't compile until Task 6 updates its `start` call. Don't commit unless the user says to.

---

### Task 3: Taunt lines (`taunt.rs`)

**Files:**
- Create: `crates/core/src/taunt.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod taunt;` after `pub mod tasks;`)

**Interfaces:**
- Produces:
  - `pub const SITE: &str = "{site}"`
  - `pub fn taunt_template(name: Option<&str>, bails: Option<u32>, variant: u64) -> String`: contains `{site}`. `bails: None` means the count is unknown.
  - `pub fn taunt(name: Option<&str>, bails: Option<u32>, target: &str, variant: u64) -> String`
  - `pub fn default_name(account: &str) -> Option<String>`
  - `pub fn app_label(exe: &str) -> String`

- [ ] **Step 1: Write the failing tests.** Create `taunt.rs` with only this, plus the `lib.rs` line:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_names_the_target_and_mentions_the_count() {
        for v in 0..4 {
            let t = taunt(Some("Hirsh"), Some(3), "Steam", v);
            assert!(t.contains("Steam") && t.contains('3') && t.contains("Hirsh"), "{t}");
            let one = taunt(Some("Hirsh"), Some(1), "youtube.com", v);
            assert!(one.contains("once") || one.contains("one"), "{one}");
            let zero = taunt(Some("Hirsh"), Some(0), "x.com", v);
            assert!(zero.to_lowercase().contains("zero") || zero.contains("Clean week"), "{zero}");
            assert!(!taunt(Some("Hirsh"), None, "Steam", v).chars().any(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn templates_keep_the_site_gap_and_fit_the_limit() {
        let long = "N".repeat(40);
        for bails in [None, Some(0), Some(1), Some(42)] {
            for name in [None, Some("Hirsh"), Some(long.as_str())] {
                for v in 0..4 {
                    let t = taunt_template(name, bails, v);
                    assert!(t.contains(SITE), "{t}");
                    assert!(t.chars().count() <= 300, "{t}");
                }
            }
        }
    }

    #[test]
    fn works_without_a_name() {
        let t = taunt(None, Some(3), "Steam", 0);
        assert!(t.starts_with(char::is_uppercase) && t.contains("Steam") && t.contains('3'), "{t}");
        assert_eq!(taunt(Some("  "), Some(3), "Steam", 0), t);
    }

    #[test]
    fn default_name_capitalizes_the_account() {
        assert_eq!(default_name("hirsh").as_deref(), Some("Hirsh"));
        assert_eq!(default_name("  ").as_deref(), None);
    }

    #[test]
    fn app_label_drops_the_extension() {
        assert_eq!(app_label("Steam.exe"), "Steam");
        assert_eq!(app_label("Discord.EXE"), "Discord");
        assert_eq!(app_label("tool"), "tool");
    }
}
```

- [ ] **Step 2: Run to confirm it fails.** Run `cargo test -p getlocked-core taunt`. Expected: compile errors.

- [ ] **Step 3: Implement** above the tests:

```rust
//! The roast shown when a user hits a block. `{site}` is left in templates
//! for the extension to fill with the blocked domain.

pub const SITE: &str = "{site}";
const MAX_NAME: usize = 40;

fn clean_name(name: Option<&str>) -> Option<String> {
    let n = name?.trim();
    (!n.is_empty()).then(|| n.chars().take(MAX_NAME).collect())
}

/// A taunt with `{site}` where the target goes. `bails` is the number of
/// sessions ended early this week, or `None` if it couldn't be read.
pub fn taunt_template(name: Option<&str>, bails: Option<u32>, variant: u64) -> String {
    let Some(n) = clean_name(name) else {
        return match bails {
            None => format!("Come on. {SITE}? LOCK IN."),
            Some(0) => format!("Zero bails this week. Don't ruin it for {SITE}."),
            Some(1) => format!("You already bailed once this week. {SITE}? Really? LOCK IN."),
            Some(b) => format!("You've bailed early {b} times this week, and now you're trying to open {SITE}? LOCK IN."),
        };
    };
    let lines: [String; 2] = match bails {
        None => [format!("Come on {n}, {SITE}? LOCK IN."), format!("{n}. {SITE}. Seriously? LOCK IN.")],
        Some(0) => [
            format!("{n}. Zero bails this week. Don't ruin it for {SITE}."),
            format!("Clean week so far, {n}. {SITE} isn't worth breaking it. LOCK IN."),
        ],
        Some(1) => [
            format!("Come on {n}, you already bailed once this week. {SITE}? Really? LOCK IN."),
            format!("{n}, one early exit this week is plenty. Close {SITE} and LOCK IN."),
        ],
        Some(b) => [
            format!(
                "Come on {n}, you said you'd lock in. You've bailed early {b} times this week, and now you're trying to open {SITE}? LOCK IN."
            ),
            format!("{b} early exits this week, {n}, and now {SITE}? LOCK IN."),
        ],
    };
    lines[(variant % 2) as usize].clone()
}

pub fn taunt(name: Option<&str>, bails: Option<u32>, target: &str, variant: u64) -> String {
    taunt_template(name, bails, variant).replace(SITE, target)
}

/// "hirsh" → "Hirsh", for when the user hasn't set a name.
pub fn default_name(account: &str) -> Option<String> {
    let a = account.trim();
    let mut chars = a.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

/// "Steam.exe" → "Steam".
pub fn app_label(exe: &str) -> String {
    let lower = exe.to_ascii_lowercase();
    match lower.strip_suffix(".exe") {
        Some(_) => exe[..exe.len() - 4].to_string(),
        None => exe.to_string(),
    }
}
```

- [ ] **Step 4: Run the tests.** Run `cargo test -p getlocked-core taunt`. Expected: 5 passed.
- [ ] **Step 5: Stop for review.** Offer the commit: "Add taunt lines". Don't commit unless the user says to.

---

### Task 4: Settings and stats (`model.rs`, `stats.rs`)

**Files:**
- Modify: `crates/core/src/model.rs` (`Settings`, its `Default`, tests)
- Modify: `crates/core/src/stats.rs` (schema, `record_session`, `SessionRow`, `summary`, new `ended_early_since` and `week_start`, tests)

**Interfaces:**
- Consumes: `TaskState` (Task 1), `Session.tasks` (Task 2).
- Produces:
  - `Settings.display_name: Option<String>` and `Settings.roast: bool`
  - `Stats::ended_early_since(&self, since: NaiveDate) -> Result<u32, StatsError>`
  - `pub fn week_start(d: NaiveDate) -> NaiveDate`
  - `SessionRow.tasks: Vec<TaskRow>`, where `pub struct TaskRow { pub text: String, pub state: String, pub note: Option<String>, pub reason: Option<String> }` (camelCase, `state` ∈ `"open" | "done" | "dropped"`)

- [ ] **Step 1: Write the failing tests.**
  - **`model.rs` tests:** add:

```rust
    #[test]
    fn old_settings_get_name_and_roast_defaults() {
        let s: Settings =
            serde_json::from_str(r#"{"autostart":false,"notifications":true,"extensionGraceSecs":10}"#).unwrap();
        assert_eq!(s.display_name, None);
        assert!(s.roast);
    }
```

  - **`stats.rs` tests:** add (`ended()` is the existing helper; `d()` builds an October 2026 date):

```rust
    #[test]
    fn week_starts_on_monday() {
        // 2026-10-26 is a Monday.
        assert_eq!(week_start(d(26)), d(26));
        assert_eq!(week_start(d(28)), d(26));
        assert_eq!(week_start(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()), d(26));
        assert_eq!(week_start(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()), NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    }

    #[test]
    fn counts_early_ends_since_a_date() {
        let stats = Stats::in_memory().unwrap();
        stats.record_session(&ended("a", 60, Outcome::EndedEarly), d(25)).unwrap();
        stats.record_session(&ended("b", 60, Outcome::EndedEarly), d(26)).unwrap();
        stats.record_session(&ended("c", 60, Outcome::EndedEarly), d(28)).unwrap();
        stats.record_session(&ended("d", 60, Outcome::Completed), d(28)).unwrap();
        assert_eq!(stats.ended_early_since(d(26)).unwrap(), 2);
    }

    #[test]
    fn records_tasks_with_notes_and_reasons() {
        let stats = Stats::in_memory().unwrap();
        let mut e = ended("t", 600, Outcome::Completed);
        e.session.tasks = crate::tasks::TaskList::new(&["intro".into(), "chart".into(), "refs".into()]).unwrap();
        e.session.tasks.tick(1, "wrote the intro").unwrap();
        e.session.tasks.drop_task(2, "chart not needed now").unwrap();
        stats.record_session(&e, d(28)).unwrap();
        // Recording the same session again replaces its tasks, not duplicates them.
        stats.record_session(&e, d(28)).unwrap();
        let row = &stats.summary(d(28), 1).unwrap().recent[0];
        assert_eq!(row.tasks.len(), 3);
        assert_eq!(row.tasks[0], TaskRow { text: "intro".into(), state: "done".into(), note: Some("wrote the intro".into()), reason: None });
        assert_eq!(row.tasks[1].state, "dropped");
        assert_eq!(row.tasks[1].reason.as_deref(), Some("chart not needed now"));
        assert_eq!(row.tasks[2].state, "open");
    }
```

- [ ] **Step 2: Run to confirm it fails.** Run `cargo test -p getlocked-core`. Expected: compile errors.

- [ ] **Step 3: Implement.**
  - **`model.rs`, `Settings`:** add two fields:

```rust
    /// What taunts call the user. Empty or missing: the Windows account name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Roast the user when they hit a block.
    #[serde(default = "default_true")]
    pub roast: bool,
```

    Then update `Default`:

```rust
        Self { autostart: false, notifications: true, extension_grace_secs: 10, display_name: None, roast: true }
```

    Also give the existing three fields serde defaults so partial objects load: `#[serde(default)]` on `autostart`, `#[serde(default = "default_true")]` on `notifications`, and `#[serde(default = "default_grace")]` on `extension_grace_secs` with `fn default_grace() -> u64 { 10 }`.
  - **`stats.rs`, imports:** add `Datelike` to the chrono import, and `use crate::tasks::TaskState;`.
  - **`stats.rs`, schema:** in `init`, add this table to the batch, before the `CREATE INDEX` lines:

```sql
            CREATE TABLE IF NOT EXISTS session_tasks (
                session_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                text TEXT NOT NULL,
                state TEXT NOT NULL,
                note TEXT,
                reason TEXT
            );
            CREATE INDEX IF NOT EXISTS session_tasks_session ON session_tasks(session_id);
```

  - **`stats.rs`, `record_session`:** after the existing `INSERT OR REPLACE`, before `Ok(())`:

```rust
        self.conn.execute("DELETE FROM session_tasks WHERE session_id = ?1", params![s.id])?;
        for (i, t) in s.tasks.tasks().iter().enumerate() {
            let (state, note, reason) = match &t.state {
                TaskState::Open => ("open", None, None),
                TaskState::Done { note } => ("done", Some(note.as_str()), None),
                TaskState::Dropped { reason } => ("dropped", None, Some(reason.as_str())),
            };
            self.conn.execute(
                "INSERT INTO session_tasks (session_id, position, text, state, note, reason) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![s.id, i as i64, t.text, state, note, reason],
            )?;
        }
```

  - **`stats.rs`, new types:** add `TaskRow`, and a `tasks` field on `SessionRow`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub text: String,
    /// "open", "done" or "dropped".
    pub state: String,
    pub note: Option<String>,
    pub reason: Option<String>,
}
```

    Add `pub tasks: Vec<TaskRow>,` to `SessionRow`. In `summary`'s `recent` query, set `tasks: Vec::new()` inside the row closure. After collecting `recent` (make it `let mut recent: Vec<SessionRow>`), fill it:

```rust
        let mut stmt = self.conn.prepare(
            "SELECT text, state, note, reason FROM session_tasks WHERE session_id = ?1 ORDER BY position",
        )?;
        for row in &mut recent {
            row.tasks = stmt
                .query_map(params![row.id], |r| {
                    Ok(TaskRow { text: r.get(0)?, state: r.get(1)?, note: r.get(2)?, reason: r.get(3)? })
                })?
                .collect::<Result<_, _>>()?;
        }
```

  - **`stats.rs`, new methods:** add to `impl Stats`, and the free function below it:

```rust
    /// Sessions ended early on or after `since` (a local date).
    pub fn ended_early_since(&self, since: NaiveDate) -> Result<u32, StatsError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE outcome = 'endedEarly' AND local_date >= ?1",
            params![since.to_string()],
            |r| r.get(0),
        )?;
        Ok(n as u32)
    }
```

```rust
/// The Monday on or before `d`.
pub fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(i64::from(d.weekday().num_days_from_monday()))
}
```

- [ ] **Step 4: Run the tests.** Run `cargo test -p getlocked-core`. Expected: all pass.
- [ ] **Step 5: Stop for review.** Offer the commit: "Store tasks in stats and count weekly early ends". Don't commit unless the user says to.

---

### Task 5: Protocol fields and the extension's blocked page

**Files:**
- Modify: `crates/core/src/protocol.rs` (`SiteRulesMsg`, wire test)
- Modify: `packages/protocol/src/index.ts` (`SiteRulesMsg`)
- Modify: `extension/src/rules.ts` (`parseRules`, new `fillTaunt`)
- Modify: `extension/src/rules.test.ts` (helper, new tests)
- Modify: `extension/public/blocked.html`, `extension/src/blocked.ts`

**Interfaces:**
- Produces:
  - Rust `SiteRulesMsg { ..., open_tasks: Vec<String>, taunt: Option<String> }` (both `#[serde(default)]`)
  - TS `SiteRulesMsg.openTasks: string[]`, `SiteRulesMsg.taunt: string | null`
  - `export function fillTaunt(template: string, site: string): string`

- [ ] **Step 1: Write the failing tests.**
  - **`protocol.rs` wire test:** change the expected JSON to:

```rust
            r#"{"type":"rules","active":true,"mode":"block","domains":["youtube.com"],"endsAt":5,"label":null,"openTasks":[],"taunt":null}"#
```

    Then add, inside the same test:

```rust
        let old: SiteRulesMsg = serde_json::from_str(r#"{"active":true,"mode":"block","domains":[],"endsAt":null,"label":null}"#).unwrap();
        assert!(old.open_tasks.is_empty() && old.taunt.is_none());
```

  - **`rules.test.ts`:** change the `rules()` helper to return `{ active, mode, domains, endsAt: null, label: null, openTasks: [], taunt: null }`, and import `fillTaunt`. Then add:

```ts
describe("parseRules new fields", () => {
  it("defaults them when an older app leaves them out", () => {
    const r = parseRules({ active: true, mode: "block", domains: ["a.com"], endsAt: null, label: null });
    expect(r?.openTasks).toEqual([]);
    expect(r?.taunt).toBeNull();
  });

  it("accepts tasks and a taunt within limits", () => {
    const ok = { ...rules("block", ["a.com"]), openTasks: ["write intro"], taunt: "Come on Hirsh, {site}? LOCK IN." };
    expect(parseRules(ok)).toEqual(ok);
  });

  it("rejects oversized or mistyped tasks and taunts", () => {
    const base = rules("block", ["a.com"]);
    expect(parseRules({ ...base, openTasks: Array(21).fill("t") })).toBeNull();
    expect(parseRules({ ...base, openTasks: ["x".repeat(121)] })).toBeNull();
    expect(parseRules({ ...base, openTasks: [5] })).toBeNull();
    expect(parseRules({ ...base, taunt: "x".repeat(301) })).toBeNull();
    expect(parseRules({ ...base, taunt: 7 })).toBeNull();
  });
});

describe("fillTaunt", () => {
  it("fills every {site} and nothing else", () => {
    expect(fillTaunt("{site}? {site}! {name}", "x.com")).toBe("x.com? x.com! {name}");
  });
});
```

- [ ] **Step 2: Run to confirm it fails.** Run `cargo test -p getlocked-core protocol` and `npm.cmd test -w extension`. Expected: failures (missing fields and `fillTaunt`).

- [ ] **Step 3: Implement.**
  - **`protocol.rs`, `SiteRulesMsg`:** add after `label`:

```rust
    /// Tasks not yet done, for the blocked page.
    #[serde(default)]
    pub open_tasks: Vec<String>,
    /// A taunt with `{site}` where the blocked domain goes. `None` when the
    /// user turned roasting off.
    #[serde(default)]
    pub taunt: Option<String>,
```

  - **`packages/protocol/src/index.ts`, `SiteRulesMsg`:** add:

```ts
  /** Tasks not yet done, shown on the blocked page. */
  openTasks: string[];
  /** A taunt with `{site}` where the blocked domain goes; null when roasting is off. */
  taunt: string | null;
```

  - **`rules.ts`, `parseRules`:** after the existing `ok` check, and before `return`, read the new fields:

```ts
  const openTasks = m.openTasks === undefined ? [] : m.openTasks;
  const taunt = m.taunt === undefined ? null : m.taunt;
  const extrasOk =
    Array.isArray(openTasks) &&
    openTasks.length <= 20 &&
    openTasks.every((t) => typeof t === "string" && t.length <= 120) &&
    (taunt === null || (typeof taunt === "string" && taunt.length <= 300));
  if (!extrasOk) return null;
```

    Then add `openTasks: openTasks as string[], taunt: taunt as string | null,` to the returned object. Also add:

```ts
/** The taunt with the blocked domain filled in. Rendered as text only. */
export function fillTaunt(template: string, site: string): string {
  return template.split("{site}").join(site);
}
```

  - **`blocked.html`:**
    - Give the `<h1>` an id: `<h1 id="headline"><span class="domain" id="domain">This site</span> is blocked</h1>`.
    - After `<p class="quote" id="quote"></p>`, add:

```html
      <div id="todo" hidden>
        <p class="muted">Still to do:</p>
        <ul id="tasks" style="text-align: left; display: inline-block; margin: 0 0 24px"></ul>
      </div>
```

  - **`blocked.ts`:** import `fillTaunt` from `./rules`. In `render()`, after the label line, add:

```ts
  if (rules?.taunt && domain) $("headline").textContent = fillTaunt(rules.taunt, domain);
  const open = rules?.openTasks ?? [];
  $("todo").hidden = open.length === 0;
  $("tasks").replaceChildren(
    ...open.map((t) => {
      const li = document.createElement("li");
      li.textContent = t;
      return li;
    }),
  );
```

- [ ] **Step 4: Run the tests.** Run `cargo test -p getlocked-core protocol`, `npm.cmd test -w extension`, `npm.cmd run typecheck` and `npm.cmd run ext:build`. Expected: all pass. Typecheck fails only in `apps/desktop` if Task 6 isn't done yet, and that's expected.
- [ ] **Step 5: Stop for review.** Offer the commit: "Send open tasks and a taunt to the blocked page". Don't commit unless the user says to.

---

### Task 6: Desktop app wiring (runtime, commands)

**Files:**
- Modify: `apps/desktop/src-tauri/src/runtime.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (`invoke_handler` list)

**Interfaces:**
- Consumes: Task 2 engine methods; `taunt`, `taunt_template`, `default_name`, `app_label` (Task 3); `ended_early_since`, `week_start` (Task 4); `SiteRulesMsg.open_tasks` and `taunt` (Task 5).
- Produces these Tauri commands (JS names in camelCase):
  - `start_session(config, tasks: Vec<String>)`
  - `add_task(text) -> u32`
  - `edit_task(id, text)`
  - `tick_task(id, note)`
  - `untick_task(id)`
  - `drop_task(id, reason)`

- [ ] **Step 1: Update the runtime.**
  - **Imports:** add `use std::sync::atomic::{AtomicU64, Ordering};`, `use getlocked_core::stats::week_start;` and `use getlocked_core::taunt::{app_label, default_name, taunt, taunt_template};`.
  - **`AppState` fields:** add `pub bails_this_week: Mutex<Option<u32>>,` and `taunt_counter: AtomicU64,`. Initialize them in `AppState::load` with `Mutex::new(None)` and `AtomicU64::new(0)`.
  - **`save_session`:** make it `pub fn save_session(&self)`.
  - **New methods on `AppState`:**

```rust
    /// Re-reads how many sessions were ended early since Monday.
    pub fn refresh_bails(&self) {
        let since = week_start(Local::now().date_naive());
        let n = self.stats.lock().unwrap().as_ref().and_then(|s| s.ended_early_since(since).ok());
        *self.bails_this_week.lock().unwrap() = n;
    }

    /// The name taunts use: the Settings name, else the Windows account.
    fn display_name(&self) -> Option<String> {
        let set = self.config.lock().unwrap().settings.display_name.clone();
        set.filter(|n| !n.trim().is_empty()).or_else(|| std::env::var("USERNAME").ok().and_then(|a| default_name(&a)))
    }

    fn roast(&self) -> bool {
        self.config.lock().unwrap().settings.roast
    }
```

  - **`start_session`:** change the signature to `pub fn start_session(&self, app: &AppHandle, config: SessionConfig, tasks: Vec<String>)`. The engine call becomes `.start(config, &tasks, Utc::now())`. In `start_preset`, call `self.start_session(app, config, Vec::new())`.
  - **`finish`:** add `self.refresh_bails();` after recording the session.
  - **`site_rules`:** read the name, roast and bails values *before* locking the engine. Locking the config while the engine lock is held is fine, but keep reads together:

```rust
    fn site_rules(&self) -> SiteRulesMsg {
        let (name, roast, bails) = (self.display_name(), self.roast(), *self.bails_this_week.lock().unwrap());
        let engine = self.engine.lock().unwrap();
        let Some(s) = engine.session().filter(|s| s.blocking() && s.config.sites.mode != ListMode::Off) else {
            return SiteRulesMsg::default();
        };
        // Round to 10s so the message only changes when something real does.
        let ends_at = s.phase_ms_left().map(|ms| {
            let t = Utc::now().timestamp_millis() + ms as i64;
            t - t.rem_euclid(10_000) + 10_000
        });
        // Fixed per session, so the message doesn't change every tick.
        let variant = s.id.bytes().map(u64::from).sum();
        SiteRulesMsg {
            active: true,
            mode: s.config.sites.mode,
            domains: s.config.sites.domains.clone(),
            ends_at,
            label: s.config.label.clone(),
            open_tasks: s.tasks.open_texts(),
            taunt: roast.then(|| taunt_template(name.as_deref(), bails, variant)),
        }
    }
```

  - **`enforce`:** in the `CloseReason::Rule` arm, replace the toast call:

```rust
                CloseReason::Rule => {
                    let label = app_label(&c.exe_name);
                    let body = if self.roast() {
                        let bails = *self.bails_this_week.lock().unwrap();
                        let v = self.taunt_counter.fetch_add(1, Ordering::Relaxed);
                        taunt(self.display_name().as_deref(), bails, &label, v)
                    } else {
                        status.clone()
                    };
                    self.toast(app, &c.exe_name, &format!("{label} is blocked"), &body)
                }
```

  - **`spawn_ticker`:** call `state.refresh_bails();` before the loop. Inside the loop's schedule block (it runs every 30 s), also call `state.refresh_bails();` so Monday's reset is picked up.

- [ ] **Step 2: Add the commands.** In `commands.rs`, change `start_session`:

```rust
#[tauri::command]
pub fn start_session(app: AppHandle, state: S, config: SessionConfig, tasks: Vec<String>) -> Result<(), String> {
    state.start_session(&app, config, tasks)
}
```

    Then add (and import `getlocked_core::session::{Engine, EngineError}`):

```rust
/// Runs a task change, then saves and pushes the new state.
fn change_tasks<T>(
    app: &AppHandle,
    state: &AppState,
    f: impl FnOnce(&mut Engine) -> Result<T, EngineError>,
) -> Result<T, String> {
    let out = f(&mut state.engine.lock().unwrap()).map_err(|e| e.to_string())?;
    state.save_session();
    state.emit(app);
    Ok(out)
}

#[tauri::command]
pub fn add_task(app: AppHandle, state: S, text: String) -> Result<u32, String> {
    change_tasks(&app, &state, |e| e.add_task(&text))
}

#[tauri::command]
pub fn edit_task(app: AppHandle, state: S, id: u32, text: String) -> Result<(), String> {
    change_tasks(&app, &state, |e| e.edit_task(id, &text))
}

#[tauri::command]
pub fn tick_task(app: AppHandle, state: S, id: u32, note: String) -> Result<(), String> {
    change_tasks(&app, &state, |e| e.tick_task(id, &note))
}

#[tauri::command]
pub fn untick_task(app: AppHandle, state: S, id: u32) -> Result<(), String> {
    change_tasks(&app, &state, |e| e.untick_task(id))
}

#[tauri::command]
pub fn drop_task(app: AppHandle, state: S, id: u32, reason: String) -> Result<(), String> {
    change_tasks(&app, &state, |e| e.drop_task(id, &reason))
}
```

  `save_settings` needs no change, because it stores the whole `Settings` including the new fields.
- [ ] **Step 3: Register the commands.** In `lib.rs`, add `commands::add_task, commands::edit_task, commands::tick_task, commands::untick_task, commands::drop_task,` to `generate_handler!`.
- [ ] **Step 4: Build, lint and test.** Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. Expected: no warnings, and all tests pass.
- [ ] **Step 5: Stop for review.** Offer the commit: "Wire tasks and taunts into the app". Don't commit unless the user says to.

---

### Task 7: UI: task box, checklist, settings, stats

**Files:**
- Modify: `apps/desktop/src/types.ts`, `apps/desktop/src/api.ts`
- Create: `apps/desktop/src/components/TaskInput.tsx`, `apps/desktop/src/components/TaskChecklist.tsx`
- Modify: `apps/desktop/src/pages/StartPage.tsx`, `SessionPage.tsx`, `SettingsPage.tsx`, `StatsPage.tsx`
- Modify: `apps/desktop/src/format.ts` and `apps/desktop/src/format.test.ts` (`wordCount` helper)

**Interfaces:**
- Consumes: the Task 6 commands and the Task 2/4 serde shapes.
- Produces:
  - `api.startSession(config, tasks)`
  - `api.addTask(text)`, `api.editTask(id, text)`, `api.tickTask(id, note)`, `api.untickTask(id)`, `api.dropTask(id, reason)`
  - `wordCount(s: string): number`, which must match the Rust `word_count` rule

- [ ] **Step 1: Write the failing test.** In `format.test.ts`, import `wordCount` and add:

```ts
group("wordCount", () => {
  it("counts words with a letter or digit, like the app does", () => {
    expect(wordCount("wrote the intro")).toBe(3);
    expect(wordCount("  done ")).toBe(1);
    expect(wordCount("- . ! ?")).toBe(0);
    expect(wordCount("a - b . c")).toBe(3);
  });
});
```

    Run `npm.cmd test -w apps/desktop`. Expected: FAIL (`wordCount` is not exported).

- [ ] **Step 2: Add the types, API and helper.**
  - **`format.ts`:**

```ts
/** Same rule as the app: whitespace-separated words containing a letter or digit. */
export function wordCount(s: string): number {
  return s.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w)).length;
}
```

  - **`types.ts`:**

```ts
export type TaskState = { type: "open" } | { type: "done"; note: string } | { type: "dropped"; reason: string };

export interface Task {
  id: number;
  text: string;
  state: TaskState;
}

export interface TaskRow {
  text: string;
  state: "open" | "done" | "dropped";
  note: string | null;
  reason: string | null;
}
```

    Then add more fields to existing types:
    - `SessionView`: `tasks: Task[]; allTasksDone: boolean;`
    - `Settings`: `displayName: string | null; roast: boolean;`
    - `SessionRow`: `tasks: TaskRow[];`
  - **`api.ts`:** replace `startSession` and add the task calls:

```ts
  startSession: (config: SessionConfig, tasks: string[]) => invoke<void>("start_session", { config, tasks }),
  addTask: (text: string) => invoke<number>("add_task", { text }),
  editTask: (id: number, text: string) => invoke<void>("edit_task", { id, text }),
  tickTask: (id: number, note: string) => invoke<void>("tick_task", { id, note }),
  untickTask: (id: number) => invoke<void>("untick_task", { id }),
  dropTask: (id: number, reason: string) => invoke<void>("drop_task", { id, reason }),
```

- [ ] **Step 3: Create `components/TaskInput.tsx`.**

```tsx
import { useState } from "react";
import { Card, Chip, SectionTitle } from "./ui";

const MAX_TASKS = 20;
const MAX_LEN = 120;

/** The Start page's "What are you getting done?" list. */
export function TaskInput({ tasks, onChange }: { tasks: string[]; onChange: (t: string[]) => void }) {
  const [text, setText] = useState("");
  const add = () => {
    const t = text.trim();
    if (!t || t.length > MAX_LEN || tasks.length >= MAX_TASKS) return;
    onChange([...tasks, t]);
    setText("");
  };
  return (
    <Card>
      <SectionTitle hint="Optional. Tick every task (with a short note) and you can end the session at any strictness.">
        What are you getting done?
      </SectionTitle>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input
          value={text}
          maxLength={MAX_LEN}
          onChange={(e) => setText(e.target.value)}
          placeholder="e.g. Write the essay intro"
          className="flex-1 rounded-lg border border-ink-700 bg-ink-850 px-3 py-1.5 text-sm outline-none focus:border-accent-500"
        />
      </form>
      {tasks.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {tasks.map((t, i) => (
            <Chip key={`${i}:${t}`} onRemove={() => onChange(tasks.filter((_, j) => j !== i))}>
              {t}
            </Chip>
          ))}
        </div>
      )}
    </Card>
  );
}
```

- [ ] **Step 4: Create `components/TaskChecklist.tsx`.**

```tsx
import { useState } from "react";
import { Check, Pencil, Trash2 } from "lucide-react";
import { api, errorText } from "../api";
import { wordCount } from "../format";
import type { Task } from "../types";
import { Button, Card, ErrorText, Modal } from "./ui";

type Asking = { kind: "tick" | "drop"; task: Task } | null;

function NoteDialog({ asking, onClose }: { asking: NonNullable<Asking>; onClose: () => void }) {
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  const words = wordCount(text);
  const ok = words >= 3 && text.trim().length <= 200;
  const submit = async () => {
    try {
      if (asking.kind === "tick") await api.tickTask(asking.task.id, text);
      else await api.dropTask(asking.task.id, text);
      onClose();
    } catch (e) {
      setError(errorText(e));
    }
  };
  return (
    <Modal title={asking.kind === "tick" ? "What did you do?" : "Why are you dropping it?"} onClose={onClose}>
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (ok) void submit();
        }}
      >
        <p className="text-sm text-ink-300">“{asking.task.text}”</p>
        <input
          autoFocus
          value={text}
          maxLength={200}
          onChange={(e) => setText(e.target.value)}
          placeholder={asking.kind === "tick" ? "e.g. wrote the intro section" : "e.g. moved it to tomorrow"}
          className="rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 text-sm outline-none focus:border-accent-500"
        />
        <p className={`text-xs ${ok ? "text-emerald-300" : "text-ink-400"}`}>{words} of 3 words</p>
        <ErrorText>{error}</ErrorText>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant={asking.kind === "tick" ? "primary" : "danger"} disabled={!ok}>
            {asking.kind === "tick" ? "Tick it off" : "Drop task"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

/** The session's tasks: tick (with a note), untick, edit, drop (with a reason), add. */
export function TaskChecklist({ tasks }: { tasks: Task[] }) {
  const [asking, setAsking] = useState<Asking>(null);
  const [editing, setEditing] = useState<{ id: number; text: string } | null>(null);
  const [adding, setAdding] = useState("");
  const [error, setError] = useState("");
  const run = (p: Promise<unknown>) => {
    setError("");
    p.catch((e) => setError(errorText(e)));
  };

  const live = tasks.filter((t) => t.state.type !== "dropped");
  const done = live.filter((t) => t.state.type === "done").length;

  return (
    <Card className="w-full">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-semibold">Tasks</h3>
        {live.length > 0 && (
          <span className="text-xs text-ink-400">
            {done} of {live.length} done
          </span>
        )}
      </div>
      <ul className="flex flex-col gap-1">
        {live.map((t) => (
          <li key={t.id} className="group flex items-start gap-3 rounded-lg px-2 py-1.5 hover:bg-ink-850">
            <button
              aria-label={t.state.type === "done" ? "Untick" : "Tick"}
              onClick={() => (t.state.type === "done" ? run(api.untickTask(t.id)) : setAsking({ kind: "tick", task: t }))}
              className={`mt-0.5 grid h-5 w-5 shrink-0 place-items-center rounded border ${
                t.state.type === "done" ? "border-accent-400 bg-accent-600 text-white" : "border-ink-600"
              }`}
            >
              {t.state.type === "done" && <Check size={14} />}
            </button>
            <div className="min-w-0 flex-1">
              {editing?.id === t.id ? (
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    run(api.editTask(t.id, editing.text));
                    setEditing(null);
                  }}
                >
                  <input
                    autoFocus
                    value={editing.text}
                    maxLength={120}
                    onChange={(e) => setEditing({ id: t.id, text: e.target.value })}
                    onBlur={() => setEditing(null)}
                    className="w-full rounded border border-ink-700 bg-ink-850 px-2 py-0.5 text-sm outline-none focus:border-accent-500"
                  />
                </form>
              ) : (
                <div className={`text-sm ${t.state.type === "done" ? "text-ink-400 line-through" : ""}`}>{t.text}</div>
              )}
              {t.state.type === "done" && <div className="text-xs text-ink-400">{t.state.note}</div>}
            </div>
            <button
              aria-label="Edit"
              onClick={() => setEditing({ id: t.id, text: t.text })}
              className="text-ink-400 opacity-0 group-hover:opacity-100 hover:text-ink-100"
            >
              <Pencil size={14} />
            </button>
            <button
              aria-label="Drop"
              onClick={() => setAsking({ kind: "drop", task: t })}
              className="text-ink-400 opacity-0 group-hover:opacity-100 hover:text-red-300"
            >
              <Trash2 size={14} />
            </button>
          </li>
        ))}
      </ul>
      <form
        className="mt-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (adding.trim()) {
            run(api.addTask(adding));
            setAdding("");
          }
        }}
      >
        <input
          value={adding}
          maxLength={120}
          onChange={(e) => setAdding(e.target.value)}
          placeholder="+ Add a task"
          className="w-full rounded-lg border border-ink-800 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-accent-500"
        />
      </form>
      <ErrorText>{error}</ErrorText>
      {asking && <NoteDialog asking={asking} onClose={() => setAsking(null)} />}
    </Card>
  );
}
```

- [ ] **Step 5: Wire up the pages.**
  - **`StartPage.tsx`:**
    - Import `TaskInput` and add `const [tasks, setTasks] = useState<string[]>([]);`.
    - Render `<TaskInput tasks={tasks} onChange={setTasks} />` right before `<ConfigEditor … />`.
    - In `start`, call `api.startSession(config, tasks)`.
  - **`SessionPage.tsx`:**
    - Import `TaskChecklist`.
    - Under the End button block, render `<TaskChecklist tasks={session.tasks} />`, always, even with no tasks, so tasks can be added mid-session.
    - Replace the End button with:

```tsx
        <Button
          variant={session.allTasksDone ? "primary" : end.disabled ? "secondary" : "danger"}
          onClick={onEnd}
          disabled={end.disabled}
        >
          {end.disabled && <Lock size={14} />}
          {session.allTasksDone ? "All done, end session" : end.label}
        </Button>
```

    `onEnd` needs no change: when all tasks are done, the policy is `free`, so it calls `api.endSession(null)`.
  - **`SettingsPage.tsx`:** in the first `Card`, after the Notifications toggle, add:

```tsx
        <Toggle
          checked={settings.roast}
          onChange={(roast) => save({ roast })}
          label="Roast me when I slip"
          hint="When you hit a block, GetLocked reminds you how many sessions you've bailed on this week."
        />
        <label className="flex items-center justify-between gap-4">
          <span>
            <span className="block text-sm">What should GetLocked call you?</span>
            <span className="block text-xs text-ink-400">Used in roasts. Leave empty to use your Windows name.</span>
          </span>
          <input
            defaultValue={settings.displayName ?? ""}
            maxLength={40}
            onBlur={(e) => save({ displayName: e.target.value.trim() || null })}
            className="w-48 rounded-lg border border-ink-700 bg-ink-850 px-3 py-1.5 text-sm outline-none focus:border-accent-500"
          />
        </label>
```

  - **`StatsPage.tsx`:** in the Recent sessions table, add `const [open, setOpen] = useState<string | null>(null);` at the top of `StatsPage`. Make each session row clickable when it has tasks (`onClick={() => s.tasks.length && setOpen(open === s.id ? null : s.id)}` with `className` adding `cursor-pointer` when `s.tasks.length`). Directly after each row, render:

```tsx
                  {open === s.id && (
                    <tr>
                      <td colSpan={3} className="pb-2">
                        <ul className="flex flex-col gap-1 rounded-lg bg-ink-850 p-3 text-xs">
                          {s.tasks.map((t, i) => (
                            <li key={i}>
                              <span className={t.state === "dropped" ? "text-ink-400 line-through" : ""}>{t.text}</span>
                              {t.state === "done" && <span className="text-emerald-300"> ✓ {t.note}</span>}
                              {t.state === "dropped" && <span className="text-ink-400"> (dropped: {t.reason})</span>}
                              {t.state === "open" && <span className="text-ink-400"> (not done)</span>}
                            </li>
                          ))}
                        </ul>
                      </td>
                    </tr>
                  )}
```

    Wrap the row and this block in `<Fragment key={s.id}>` (import `Fragment` from `react`), and move `key` off the `<tr>`.

- [ ] **Step 6: Run the checks.** Run `npm.cmd run typecheck`, `npm.cmd test` and `npm.cmd run build -w apps/desktop`. Expected: no type errors, and every test passes.
- [ ] **Step 7: Stop for review.** Offer the commit: "Add tasks and roast settings to the UI". Don't commit unless the user says to.

---

### Task 8: Docs and the live check

**Files:**
- Modify: `docs/getting-started.md` (new "Tasks" subsection under "3. Start a session", and a line under Stats)
- Modify: `docs/how-it-works.md` (a short "Tasks and taunts" section after "Strictness and the unlock code")

- [ ] **Step 1: Write the docs.**
  - **`getting-started.md`:** after the strictness table, add:

```markdown
**Tasks (optional).** Under "What are you getting done?", list what you mean to finish. During the session you can add, edit and tick tasks. Ticking needs a short note (3+ words) on what you did, and deleting a task needs a reason. Tick every remaining task and **All done, end session** appears, which ends the session at any strictness, Hard included, and counts it as completed.
```

  - **`how-it-works.md`:**

```markdown
## Tasks and taunts

Task rules live in the Rust core (`crates/core/src/tasks.rs`), not the UI: notes and drop reasons need 3+ words, editing a ticked task unticks it, and dropped tasks stay in your stats. When every remaining task is ticked, the end check frees the session at any strictness.

When you hit a block, GetLocked can roast you with your own record: how many sessions you've ended early since Monday, from your local stats. Turn it off with **Settings → Roast me when I slip**. The browser extension gets a ready-made line with a `{site}` gap, plus your unticked tasks, so the blocked page can show them. Only the paired extension receives them.
```

- [ ] **Step 2: Run the full automated check.**
  - Run `npm.cmd run ext:build`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm.cmd run typecheck` and `npm.cmd test`.
  - Expected: all clean, and all tests pass.
- [ ] **Step 3: Live check.** Run `npm.cmd run dev`, then check that each item behaves as expected:
  1. **Settings:** set the name to "Hirsh" and check that roast is on.
  2. **Start:** add tasks "write intro" and "fix chart", choose Hard plus Timer 60 min, block a harmless test app (a renamed copy of any small `.exe` under `target/`, as in earlier smoke tests), and click Lock in.
  3. **Session page:** the checklist shows "0 of 2 done", and the End button is locked.
  4. **Tick:** ticking "write intro" with "done" leaves the button disabled (1 of 3 words). With "wrote the intro", the task is ticked and the note shows.
  5. **Edit:** edit "write intro" to "write intro v2". The task becomes unticked.
  6. **Drop:** drop "fix chart" with "not needed". Refused. With "chart is not needed", the task disappears from the list.
  7. **Taunt toast:** tick "write intro v2" with a note, then launch the test app. It's closed, and the toast reads "<name> is blocked" with a taunt.
  8. **Blocked page** (extension loaded and paired, a blocked site in the session): the page headline is the taunt with the domain filled in.
  9. **All done:** the button reads "All done, end session". Clicking it ends the session, and Stats shows it as completed, with tasks visible when you expand the row.
  10. **Roast off:** turn roast off, start a short Easy session, and hit the test app. The toast shows the neutral status text.
- [ ] **Step 4: Stop for review.** Offer the commit: "Document tasks and taunts". Don't commit unless the user says to.
