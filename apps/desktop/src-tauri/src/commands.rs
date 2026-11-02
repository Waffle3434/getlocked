//! Tauri commands called from the React UI (see `src/api.ts`).

use std::sync::Arc;

use chrono::{Local, Utc};
use getlocked_core::end_policy::EndPolicy;
use getlocked_core::model::{Config, Preset, Schedule, SessionConfig, Settings};
use getlocked_core::session::{Engine, EngineError, UnlockStatus};
use getlocked_core::sites::normalize_domain as normalize;
use getlocked_core::stats::Summary;
use getlocked_core::tasks::{self, NoteCheck};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt as _;

use crate::apps_index::{self, InstalledApp};
use crate::extension_setup::{self, BrowserInfo};
use crate::runtime::{AppState, Snapshot};

type S<'a> = State<'a, Arc<AppState>>;

#[tauri::command]
pub fn get_snapshot(state: S) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub fn start_session(app: AppHandle, state: S, config: SessionConfig, tasks: Vec<String>) -> Result<(), String> {
    state.start_session(&app, config, tasks)
}

/// Runs a task change, then saves and pushes the new state.
fn change_tasks<T>(
    app: &AppHandle,
    state: &AppState,
    f: impl FnOnce(&mut Engine) -> Result<T, EngineError>,
) -> Result<T, String> {
    // The engine lock is dropped at the end of this statement, before
    // `save_session` and `emit` take it again.
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

/// Live check for the note dialog: the same rules `tick_task` and `drop_task` apply.
#[tauri::command]
pub fn check_note(note: String) -> NoteCheck {
    tasks::check_note(&note)
}

#[tauri::command]
pub fn start_preset(app: AppHandle, state: S, id: String) -> Result<(), String> {
    state.start_preset(&app, &id)
}

#[tauri::command]
pub fn begin_unlock(state: S) -> Result<UnlockStatus, String> {
    let now = state.now_ms();
    state.engine.lock().unwrap().begin_unlock(now).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unlock_status(state: S) -> UnlockStatus {
    let now = state.now_ms();
    state.engine.lock().unwrap().unlock_status(now)
}

#[tauri::command]
pub fn cancel_unlock(state: S) {
    state.engine.lock().unwrap().cancel_unlock();
}

#[tauri::command]
pub fn end_session(app: AppHandle, state: S, code: Option<String>) -> Result<(), String> {
    let now = state.now_ms();
    let ended = state.engine.lock().unwrap().end(code.as_deref(), now, Utc::now()).map_err(|e| e.to_string())?;
    state.finish(&app, ended);
    Ok(())
}

#[tauri::command]
pub fn time_up_choice(app: AppHandle, state: S, keep_going: bool) -> Result<(), String> {
    let ended = state.engine.lock().unwrap().time_up_choice(keep_going, Utc::now()).map_err(|e| e.to_string())?;
    match ended {
        Some(ended) => state.finish(&app, ended),
        None => state.emit(&app),
    }
    Ok(())
}

#[tauri::command]
pub fn save_preset(state: S, preset: Preset) -> Result<Config, String> {
    if preset.name.trim().is_empty() {
        return Err("give the preset a name".into());
    }
    let mut config = state.config.lock().unwrap().clone();
    match config.presets.iter_mut().find(|p| p.id == preset.id) {
        Some(existing) => *existing = preset,
        None => config.presets.push(preset),
    }
    state.save_config(config)
}

#[tauri::command]
pub fn delete_preset(state: S, id: String) -> Result<Config, String> {
    let mut config = state.config.lock().unwrap().clone();
    config.presets.retain(|p| p.id != id);
    // A schedule without its preset can't start anything.
    config.schedules.retain(|s| s.preset_id != id);
    state.save_config(config)
}

#[tauri::command]
pub fn save_schedule(state: S, schedule: Schedule) -> Result<Config, String> {
    if schedule.days.is_empty() {
        return Err("pick at least one day".into());
    }
    if schedule.start_minute >= 24 * 60 || schedule.days.iter().any(|d| *d > 6) {
        return Err("invalid schedule time".into());
    }
    let mut config = state.config.lock().unwrap().clone();
    if config.preset(&schedule.preset_id).is_none() {
        return Err("pick a preset for this schedule".into());
    }
    match config.schedules.iter_mut().find(|s| s.id == schedule.id) {
        Some(existing) => *existing = schedule,
        None => config.schedules.push(schedule),
    }
    state.save_config(config)
}

#[tauri::command]
pub fn delete_schedule(state: S, id: String) -> Result<Config, String> {
    let mut config = state.config.lock().unwrap().clone();
    config.schedules.retain(|s| s.id != id);
    state.save_config(config)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: S, settings: Settings) -> Result<Config, String> {
    settings.theme.validate()?;
    let autolaunch = app.autolaunch();
    let result = if settings.autostart { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|e| format!("couldn't change Start with Windows: {e}"))?;
    let mut config = state.config.lock().unwrap().clone();
    config.settings = settings;
    let config = state.save_config(config)?;
    // The popup window follows the theme through the "state" event.
    state.emit(&app);
    Ok(config)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApp {
    pub exe_name: String,
    pub path: Option<String>,
}

#[tauri::command]
pub fn list_running_apps(state: S) -> Vec<RunningApp> {
    state
        .enforcer
        .lock()
        .unwrap()
        .running_apps()
        .into_iter()
        .map(|p| RunningApp { exe_name: p.exe_name, path: p.exe_path })
        .collect()
}

#[tauri::command]
pub async fn list_installed_apps() -> Vec<InstalledApp> {
    tauri::async_runtime::spawn_blocking(apps_index::installed_apps).await.unwrap_or_default()
}

#[tauri::command]
pub fn get_stats(state: S, days: u32) -> Result<Summary, String> {
    let stats = state.stats.lock().unwrap();
    let stats = stats.as_ref().ok_or("stats database couldn't be opened")?;
    stats.summary(Local::now().date_naive(), days.clamp(1, 365)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn normalize_domain(input: String) -> Option<String> {
    normalize(&input)
}

/// Quit from the UI or tray. Refused while the session can't be ended
/// freely; a freely endable session is ended (and recorded) first.
pub fn try_quit(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    if let Some(policy) = engine.session().map(|s| s.end_policy()) {
        if policy != EndPolicy::Free {
            return Err("End your session before quitting GetLocked.".into());
        }
        if let Ok(ended) = engine.end(None, state.now_ms(), Utc::now()) {
            drop(engine);
            state.finish(app, ended);
        }
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle, state: S) -> Result<(), String> {
    try_quit(&app, &state)
}

#[tauri::command]
pub fn list_browsers() -> Vec<BrowserInfo> {
    extension_setup::browsers()
}

/// Copies the extension to a stable folder and returns its path.
#[tauri::command]
pub fn prepare_extension(app: AppHandle, state: S, target: String) -> Result<String, String> {
    extension_setup::prepare(&app, &target, &state.bridge.pairing_code()).map(|p| p.display().to_string())
}

#[tauri::command]
pub fn reveal_extension(app: AppHandle, state: S, target: String) -> Result<(), String> {
    extension_setup::reveal(&app, &target, &state.bridge.pairing_code())
}

/// New pairing code; every extension has to pair again.
#[tauri::command]
pub fn reset_pairing_code(app: AppHandle, state: S) -> Result<String, String> {
    let code = state.bridge.reset_pairing()?;
    extension_setup::update_pairing(&app, &code);
    state.emit(&app);
    Ok(code)
}

#[tauri::command]
pub fn open_extensions_page(browser: String) -> Result<(), String> {
    extension_setup::open_extensions_page(&browser)
}
