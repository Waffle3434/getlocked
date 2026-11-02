# Session tasks and taunts: design

Status: approved in brainstorming on 2026-11-01. Build after the extension pairing fix is committed: both change `SiteRulesMsg`.

## Goal

GetLocked is for "sit down, open your work, lock in until it's done". This adds two things:

1. **Tasks.** You list what you're getting done. Finishing every task (with a short note on each) ends the session at any strictness.
2. **Taunts.** When you hit a block, GetLocked roasts you with your own record: *"Come on Hirsh, you said you'd lock in. You've bailed early 3 times this week, and now you're trying to open Steam? LOCK IN."*

## Behavior

### Tasks

- **Before the session.** The Start page has a **"What are you getting done?"** list. Add or remove tasks freely; no notes or reasons are needed before the session starts. Zero tasks is allowed. Presets and schedules don't store tasks: they start sessions with none.
- **During the session**, the list stays editable:

  | Action | Rule |
  |---|---|
  | Add a task | Always allowed. |
  | Edit a task's text | Allowed. If the task was ticked, it becomes unticked and its note is cleared. |
  | Tick a task | Needs a note, "What did you do?", of 3+ words. |
  | Untick a task | Allowed. The note is cleared. |
  | Delete a task | Needs a reason, "Why are you dropping it?", of 3+ words, whether or not it was ticked. The task is kept as *dropped* in the stats, not erased. A dropped task can't be edited, ticked or restored: add it again instead. |

- **"All done".** When at least one task remains (not dropped) and every remaining task is ticked, the End button becomes **"All done, end session"**. It ends the session immediately at **every** strictness, Hard included, with no unlock code and no cooldown. The session counts as **completed** in stats and streaks, even if a timer had time left. Dropping every task does not count as all done: with no remaining tasks, the normal strictness rules apply.
- A session with no tasks behaves exactly as today.
- Timers still reach "Time's up" as before; tasks don't change that.

### Validation (enforced in the core, not only the UI)

- Task text: 1–120 characters after trimming. At most 20 tasks per session, counting dropped tasks.
- Notes and reasons: at least 3 words, where a word is a run of characters containing a letter or digit; at most 200 characters. "done", "x" and "asdf" fail; "wrote the intro" passes.

### Taunts

- **Where they appear:**
  - **App blocks:** the toast shown when an app is closed. Title: `"<App> is blocked"`. Body: the taunt.
  - **Site blocks:** the extension's blocked page, which uses the taunt as its headline and lists your unticked tasks under it ("Still to do: …").
- **Content:** your name, the number of sessions ended early since Monday 00:00 local time, and the target. The target is the app name without `.exe` (`Steam.exe` → "Steam") or the site's domain.
- **Templates:** a few per bucket, chosen so the count reads naturally:
  - **0:** *"Hirsh. Zero bails this week. Don't ruin it for {target}."*
  - **1:** *"Come on Hirsh, you already bailed once this week. {target}? Really? LOCK IN."*
  - **2+:** *"Come on Hirsh, you said you'd lock in. You've bailed early {n} times this week, and now you're trying to open {target}? LOCK IN."*

  Final wording can be polished during the build, as long as each bucket keeps its meaning.
- **Name:** a Settings field, "What should GetLocked call you?". If it's empty, the Windows account name with its first letter capitalized. If that's unavailable too, the taunt is rephrased without a name.
- **Off switch:** Settings → **"Roast me when I slip"**, on by default. When it's off, toasts and the blocked page use today's neutral text.
- **Rate limit:** the existing 20-second per-app toast cooldown applies.

## Design

### Core (`crates/core`)

- **`model.rs`:** `Settings` gains `display_name: Option<String>` and `roast: bool` (default `true`). Both use serde defaults, so old `config.json` files still load.
- **`session.rs`:**
  - `Session` gains `tasks: Vec<Task>`, where `Task { id: u32, text: String, state: TaskState }` and `TaskState = Open | Done { note } | Dropped { reason }`. Ids increase per session. Tasks are saved in `session.json`, so they survive restarts and Hard-mode relaunches.
  - `Engine::start(config, tasks: Vec<String>, now)`.
  - New `Engine` methods, each validated and returning `EngineError` on failure (`TaskNotFound`, `TooManyTasks`, `InvalidTaskText`, `NoteTooShort`, `TaskDropped`): `add_task(text)`, `edit_task(id, text)`, `tick_task(id, note)`, `untick_task(id)`, `drop_task(id, reason)`.
  - `Session::all_tasks_done()`: there is at least one non-dropped task, and every non-dropped task is `Done`.
  - Ending: when `all_tasks_done()` is true, the end policy is `Free` and the outcome is `Completed`. `SessionView` gains `tasks` and `allTasksDone`.
- **`end_policy.rs`:** `end_policy` gains an `all_tasks_done: bool` input that returns `Free` before any other rule, as `goal_reached` does now.
- **`taunt.rs` (new):** `taunt(name: Option<&str>, bails_this_week: u32, target: &str, variant: u64) -> String`, plus a `{site}` template form for the extension, `taunt_template(name, bails, variant)`. Pure functions with no I/O.
- **`stats.rs`:**
  - A new table, `session_tasks(session_id, position, text, state, note, reason)`, written by `record_session`.
  - `SessionRow` gains `tasks`.
  - A new `ended_early_since(date) -> u32`.

### App (`apps/desktop/src-tauri`)

- **Commands:**
  - `start_session(config, tasks)`
  - `add_task`, `edit_task`, `tick_task`, `untick_task`, `drop_task`, each followed by a save and a state emit
  - `save_settings` carries the two new fields
- **Runtime:**
  - **Caching the count:** `AppState` caches `bails_this_week`. It's refreshed at startup, after each session ends, and every 60 s in the ticker (for the Monday rollover).
  - **App-block toasts:** these call `taunt(…)` when `roast` is on. The variant comes from a counter, so lines rotate.
- **Site rules sent to the extension:** `site_rules()` fills two new fields on `SiteRulesMsg`:
  - `open_tasks: Vec<String>`: unticked, non-dropped task texts.
  - `taunt: Option<String>`: from `taunt_template`, with the variant fixed per session so the message doesn't change every tick. `None` when `roast` is off.

### Protocol

- `SiteRulesMsg` gains `openTasks: string[]` and `taunt: string | null` (Rust: `#[serde(default)]`).
- Mirror the change in `packages/protocol`.
- No protocol version bump. Both fields are optional on read: Rust uses `#[serde(default)]`, and the extension's `parseRules` treats a missing `openTasks` as `[]` and a missing `taunt` as `null`. The handshake doesn't compare versions, so an older extension still works and just shows the neutral page.

### Extension

- **`parseRules`:** accepts and validates the new fields: at most 20 tasks of at most 120 characters each, and a taunt of at most 300 characters.
- **Blocked page:**
  - The headline is the taunt with `{site}` replaced by the domain, set via `textContent`, never HTML.
  - Below it, "Still to do:" lists `openTasks`.
  - It falls back to today's text when `taunt` is null.

### UI (`apps/desktop/src`)

- **Start:** a "What are you getting done?" task list (Enter adds a task, × removes one), passed to `startSession`.
- **Session:** a checklist under the timer showing "2 of 3 done".
  - Clicking a checkbox opens a "What did you do?" dialog with live 3-word validation.
  - A pencil icon edits a task inline.
  - A trash icon opens a "Why are you dropping it?" dialog.
  - An "Add task" field sits under the list.
  - When all tasks are done, the End button turns primary and reads "All done, end session".
- **Settings:** a name field and a "Roast me when I slip" toggle.
- **Stats:** each recent session can be expanded to show its tasks, with notes for done tasks and reasons for dropped ones.

## Error handling

- Task commands return the core's error text, and the UI shows it inline in the dialog.
- If the stats query for the count fails, taunts use a count-free line (*"Come on Hirsh, {target}? LOCK IN."*). Using the 0 bucket would falsely claim zero bails, and the block itself must never fail.
- An unknown `taunt` placeholder never reaches the page: the extension only replaces `{site}` and renders text, never HTML.

## Testing

- **Core:**
  - Every row of the task-action table.
  - Validation edges: 2 versus 3 words, 120/121 characters, the 20-task cap.
  - `all_tasks_done`: all ticked; one dropped plus the rest ticked; all dropped (must be false).
  - End policy and outcome for Easy, Friction, Hard timer and Hard stopwatch with all tasks done.
  - Tasks survive a `session.json` round trip.
  - `taunt` for counts 0, 1, 3, with and without a name.
  - `ended_early_since` around a Monday boundary.
  - `session_tasks` rows written.
- **Extension:** `parseRules` with the new fields (valid, too long, wrong types); `{site}` substitution.
- **Live:**
  - Start a Hard timer with 2 tasks, tick both with notes, and end via "All done". Check that it shows as completed in Stats.
  - Drop a task and check the reason is in Stats.
  - Hit a blocked app and a blocked site, and check both taunts.
  - Turn roast off and check the neutral text returns.

## Out of scope

- Tasks in presets or schedules, and tasks carried between sessions.
- Judging whether a note is honest. The 3-word rule is friction, not proof.
- Taunt languages other than English.
