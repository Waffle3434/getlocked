//! App state and the background loop that ticks the session, enforces the
//! rules, talks to the extension and fires schedules.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{Local, Utc};
use getlocked_core::model::{
    AppRule, AppRules, Config, ListMode, Preset, SessionConfig, SessionKind, SiteRules, Strictness,
};
use getlocked_core::persist::{load_json, remove_file, save_json};
use getlocked_core::protocol::SiteRulesMsg;
use getlocked_core::scheduler;
use getlocked_core::session::{Ended, Engine, EngineEvent, Outcome, Phase, Session, SessionView};
use getlocked_core::stats::{week_start, BlockKind, Stats};
use getlocked_core::taunt::{app_label, default_name, taunt, taunt_template};
use serde::Serialize;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_notification::NotificationExt as _;

use crate::bridge::{Bridge, BridgeEvent, ClientInfo};
use crate::enforcer::{self, CloseReason, Enforcer, ExtensionRequirement};
use crate::guard;
use crate::popup;

const TICK: Duration = Duration::from_millis(500);
const SAVE_EVERY: Duration = Duration::from_secs(5);
const SCHEDULE_EVERY: Duration = Duration::from_secs(30);
/// Don't repeat the same "X is blocked" toast more often than this.
const TOAST_COOLDOWN: Duration = Duration::from_secs(20);

pub struct Paths {
    pub dir: PathBuf,
    pub config: PathBuf,
    pub session: PathBuf,
    pub stats: PathBuf,
}

impl Paths {
    fn new(dir: PathBuf) -> Self {
        Self {
            config: dir.join("config.json"),
            session: dir.join("session.json"),
            stats: dir.join("stats.sqlite"),
            dir,
        }
    }
}

pub struct AppState {
    pub engine: Mutex<Engine>,
    pub config: Mutex<Config>,
    pub stats: Mutex<Option<Stats>>,
    pub enforcer: Mutex<Enforcer>,
    pub bridge: Bridge,
    pub paths: Paths,
    clock: Instant,
    guard: Mutex<Option<Child>>,
    toasts: Mutex<HashMap<String, Instant>>,
    pub tray_status: Mutex<Option<MenuItem<Wry>>>,
    /// Sessions ended early since Monday. `None` when stats can't be read.
    pub bails_this_week: Mutex<Option<u32>>,
    /// Rotates the taunt lines on app-block toasts.
    taunt_counter: AtomicU64,
}

/// Everything the UI needs, pushed as the `state` event twice a second.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub session: Option<SessionView>,
    pub config: Config,
    pub extensions: Vec<ClientInfo>,
    pub bridge_port: Option<u16>,
    /// Typed into the extension's popup to pair it with this app.
    pub pairing_code: String,
    pub data_dir: String,
    pub version: &'static str,
}

/// Starter presets so the app is useful on first launch.
fn default_config() -> Config {
    let distractions = SiteRules {
        mode: ListMode::Block,
        domains: ["youtube.com", "reddit.com", "tiktok.com", "instagram.com", "x.com", "twitch.tv", "netflix.com"]
            .map(String::from)
            .to_vec(),
    };
    let games = AppRules {
        mode: ListMode::Block,
        rules: ["steam.exe", "EpicGamesLauncher.exe", "Battle.net.exe", "RiotClientServices.exe", "Discord.exe"]
            .map(|n| AppRule::Exe { name: n.into() })
            .to_vec(),
    };
    Config {
        presets: vec![
            Preset {
                id: "starter-get-it-done".into(),
                name: "Get it done".into(),
                config: SessionConfig {
                    label: Some("Get it done".into()),
                    kind: SessionKind::Stopwatch { min_secs: None },
                    strictness: Strictness::Friction,
                    apps: games.clone(),
                    sites: distractions.clone(),
                    pomodoro: None,
                    require_extension: true,
                },
            },
            Preset {
                id: "starter-pomodoro".into(),
                name: "Pomodoro study".into(),
                config: SessionConfig {
                    label: Some("Pomodoro study".into()),
                    kind: SessionKind::Timed { duration_secs: 2 * 60 * 60 },
                    strictness: Strictness::Friction,
                    apps: games,
                    sites: distractions,
                    pomodoro: Some(getlocked_core::model::PomodoroCfg { focus_secs: 25 * 60, break_secs: 5 * 60 }),
                    require_extension: true,
                },
            },
        ],
        ..Default::default()
    }
}

pub fn format_duration(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn friendly_minutes(secs: u64) -> String {
    let mins = secs.div_ceil(60);
    if mins >= 60 {
        format!("{}h {}m", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    }
}

/// "41:20 left", "Locked in for 25:03", "Break: 4:10 left", "Time's up".
pub fn status_line(v: &SessionView) -> String {
    match v.phase {
        Phase::TimeUp => "Time's up! Done, or keep going?".into(),
        Phase::Break => format!("Break: {} left", format_duration(v.phase_remaining_secs.unwrap_or(0))),
        Phase::Focus => match v.remaining_secs {
            Some(r) => format!("{} left", format_duration(r)),
            None => format!("Locked in for {}", format_duration(v.focus_secs)),
        },
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

impl AppState {
    pub fn load(dir: PathBuf, bridge: Bridge) -> Self {
        let paths = Paths::new(dir);
        let config = match load_json::<Config>(&paths.config) {
            Ok(Some(c)) => c,
            Ok(None) => {
                let c = default_config();
                let _ = save_json(&paths.config, &c);
                c
            }
            Err(e) => {
                // Keep the broken file for the user to inspect; don't overwrite it.
                eprintln!("config.json is unreadable ({e}); using defaults for this run");
                default_config()
            }
        };
        let engine = match load_json::<Session>(&paths.session) {
            Ok(Some(s)) => Engine::resume(s),
            _ => Engine::new(),
        };
        let stats = Stats::open(&paths.stats).map_err(|e| eprintln!("stats unavailable: {e}")).ok();
        Self {
            engine: Mutex::new(engine),
            config: Mutex::new(config),
            stats: Mutex::new(stats),
            enforcer: Mutex::new(Enforcer::new(enforcer::platform())),
            bridge,
            paths,
            clock: Instant::now(),
            guard: Mutex::new(None),
            toasts: Mutex::new(HashMap::new()),
            tray_status: Mutex::new(None),
            bails_this_week: Mutex::new(None),
            taunt_counter: AtomicU64::new(0),
        }
    }

    /// Re-reads how many sessions were ended early since Monday.
    pub fn refresh_bails(&self) {
        let since = week_start(Local::now().date_naive());
        let n = self.stats.lock().unwrap().as_ref().and_then(|s| s.ended_early_since(since).ok());
        *self.bails_this_week.lock().unwrap() = n;
    }

    /// The name taunts use: the Settings name, else the Windows account.
    /// Locks `config`, so don't call it while holding that lock.
    fn display_name(&self) -> Option<String> {
        let set = self.config.lock().unwrap().settings.display_name.clone();
        set.filter(|n| !n.trim().is_empty()).or_else(|| std::env::var("USERNAME").ok().and_then(|a| default_name(&a)))
    }

    /// Locks `config`, so don't call it while holding that lock.
    fn roast(&self) -> bool {
        self.config.lock().unwrap().settings.roast
    }

    pub fn now_ms(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            session: self.engine.lock().unwrap().view(),
            config: self.config.lock().unwrap().clone(),
            extensions: self.bridge.clients(),
            bridge_port: self.bridge.port(),
            pairing_code: self.bridge.pairing_code(),
            data_dir: self.paths.dir.display().to_string(),
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub fn emit(&self, app: &AppHandle) {
        let _ = app.emit("state", self.snapshot());
    }

    pub fn save_config(&self, config: Config) -> Result<Config, String> {
        save_json(&self.paths.config, &config).map_err(|e| format!("couldn't save settings: {e}"))?;
        *self.config.lock().unwrap() = config.clone();
        Ok(config)
    }

    /// Locks `engine`, so don't call it while holding that lock.
    pub fn save_session(&self) {
        if let Some(s) = self.engine.lock().unwrap().session() {
            let _ = save_json(&self.paths.session, s);
        }
    }

    /// Whether a message for `key` may go out now: notifications are on and
    /// the same key wasn't used in the last `TOAST_COOLDOWN`. Claims the slot.
    fn may_notify(&self, key: &str) -> bool {
        if !self.config.lock().unwrap().settings.notifications {
            return false;
        }
        let mut toasts = self.toasts.lock().unwrap();
        if toasts.get(key).is_some_and(|t| t.elapsed() < TOAST_COOLDOWN) {
            return false;
        }
        toasts.insert(key.to_string(), Instant::now());
        true
    }

    fn notify(app: &AppHandle, title: &str, body: &str) {
        if let Err(e) = app.notification().builder().title(title).body(body).show() {
            eprintln!("couldn't show notification \"{title}\": {e}");
        }
    }

    fn toast(&self, app: &AppHandle, key: &str, title: &str, body: &str) {
        if self.may_notify(key) {
            Self::notify(app, title, body);
        }
    }

    /// A block message: the notification plus GetLocked's own popup, which
    /// shows even when Windows hides the notification (Do not disturb).
    fn alert(&self, app: &AppHandle, key: &str, title: &str, body: &str) {
        if self.may_notify(key) {
            Self::notify(app, title, body);
            popup::show(app, title, body);
        }
    }

    pub fn start_session(&self, app: &AppHandle, config: SessionConfig, tasks: Vec<String>) -> Result<(), String> {
        let hard = config.strictness == Strictness::Hard;
        self.engine.lock().unwrap().start(config, &tasks, Utc::now()).map_err(|e| e.to_string())?;
        self.save_session();
        self.enforcer.lock().unwrap().reset();
        if hard {
            // Hard sessions must survive a reboot, which needs autostart.
            let _ = app.autolaunch().enable();
            self.ensure_guard();
        }
        self.emit(app);
        Ok(())
    }

    pub fn start_preset(&self, app: &AppHandle, id: &str) -> Result<(), String> {
        let preset = self.config.lock().unwrap().preset(id).cloned().ok_or("that preset no longer exists")?;
        let mut config = preset.config;
        config.label.get_or_insert(preset.name);
        self.start_session(app, config, Vec::new())
    }

    /// Record a finished session and lift every block.
    pub fn finish(&self, app: &AppHandle, ended: Ended) {
        let local_date = ended.session.started_at.with_timezone(&Local).date_naive();
        if let Some(stats) = self.stats.lock().unwrap().as_ref() {
            if let Err(e) = stats.record_session(&ended, local_date) {
                eprintln!("couldn't record session: {e}");
            }
        }
        self.refresh_bails();
        let _ = remove_file(&self.paths.session);
        self.stop_guard();
        self.enforcer.lock().unwrap().reset();
        self.bridge.set_rules(SiteRulesMsg::default());
        let title = match ended.outcome {
            Outcome::Completed => "Session complete. Nice work!",
            Outcome::EndedEarly => "Session ended early",
        };
        let body = format!("You focused for {}.", friendly_minutes(ended.focus_secs()));
        self.toast(app, "session-end", title, &body);
        self.emit(app);
    }

    fn ensure_guard(&self) {
        let mut guard = self.guard.lock().unwrap();
        let running = guard.as_mut().is_some_and(|c| matches!(c.try_wait(), Ok(None)));
        if !running {
            match guard::spawn(&self.paths.session) {
                Ok(child) => {
                    self.enforcer.lock().unwrap().protect(child.id());
                    *guard = Some(child);
                }
                Err(e) => eprintln!("couldn't start the watchdog: {e}"),
            }
        }
    }

    fn stop_guard(&self) {
        if let Some(mut child) = self.guard.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Site rules for the extension, given the current session.
    fn site_rules(&self) -> SiteRulesMsg {
        // Read these before locking the engine; each takes its own lock.
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

    fn enforce(&self, app: &AppHandle) {
        let (rules, ext_req, status) = {
            let engine = self.engine.lock().unwrap();
            let Some(s) = engine.session().filter(|s| s.blocking()) else { return };
            let c = &s.config;
            let ext_req = (c.require_extension && c.sites.mode != ListMode::Off && c.strictness != Strictness::Easy)
                .then(|| ExtensionRequirement {
                    connected: self.bridge.connected_families(c.strictness == Strictness::Hard),
                    grace: Duration::from_secs(self.config.lock().unwrap().settings.extension_grace_secs),
                });
            (c.apps.clone(), ext_req, engine.view().map(|v| status_line(&v)).unwrap_or_default())
        };
        let closed = self.enforcer.lock().unwrap().enforce(&rules, ext_req.as_ref());
        for c in closed {
            self.record_block(BlockKind::App, &c.exe_name);
            match c.reason {
                CloseReason::Rule => {
                    // The engine lock is released by now; these take the
                    // config and bails locks one at a time.
                    let label = app_label(&c.exe_name);
                    let body = if self.roast() {
                        let bails = *self.bails_this_week.lock().unwrap();
                        let v = self.taunt_counter.fetch_add(1, Ordering::Relaxed);
                        taunt(self.display_name().as_deref(), bails, &label, v)
                    } else {
                        status.clone()
                    };
                    self.alert(app, &c.exe_name, &format!("{label} is blocked"), &body)
                }
                CloseReason::NoExtension => self.alert(
                    app,
                    &c.exe_name,
                    &format!("{} was closed", c.exe_name),
                    "Install or turn on the GetLocked extension in this browser to use it during a session. \
                     Set it up in Settings \u{2192} Browser extension.",
                ),
            }
        }
    }

    fn record_block(&self, kind: BlockKind, target: &str) {
        let session_id = {
            let mut engine = self.engine.lock().unwrap();
            engine.record_block();
            engine.session().map(|s| s.id.clone())
        };
        if let Some(stats) = self.stats.lock().unwrap().as_ref() {
            let _ = stats.record_block(session_id.as_deref(), kind, target, Utc::now(), Local::now().date_naive());
        }
    }

    fn update_tray(&self, app: &AppHandle, view: Option<&SessionView>) {
        let text = view.map(status_line).unwrap_or_else(|| "No session running".into());
        if let Some(item) = self.tray_status.lock().unwrap().as_ref() {
            let _ = item.set_text(&text);
        }
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_tooltip(Some(format!("GetLocked: {text}")));
        }
    }
}

pub fn spawn_ticker(app: AppHandle, state: Arc<AppState>, bridge_events: Receiver<BridgeEvent>) {
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let mut last_save = Instant::now();
        let mut last_schedule_check = Instant::now() - SCHEDULE_EVERY;
        // Look back a little so a start that happened while the app was
        // launching still fires.
        let mut schedule_cursor = Local::now().naive_local() - chrono::Duration::minutes(2);
        state.refresh_bails();

        if state.engine.lock().unwrap().session().is_some_and(|s| s.config.strictness == Strictness::Hard) {
            state.ensure_guard();
        }

        loop {
            std::thread::sleep(TICK);
            let now = Instant::now();
            let delta = now.duration_since(last).as_millis() as u64;
            last = now;

            let events = state.engine.lock().unwrap().tick(delta);
            for event in events {
                match event {
                    EngineEvent::BreakStarted => {
                        state.enforcer.lock().unwrap().reset();
                        state.toast(&app, "phase", "Break time", "Blocks are lifted until the break ends.");
                    }
                    EngineEvent::FocusStarted => {
                        state.toast(&app, "phase", "Back to work", "Break's over. Blocks are back on.")
                    }
                    EngineEvent::TimeUp => {
                        show_main(&app);
                        state.toast(
                            &app,
                            "phase",
                            "Time's up!",
                            "Done, or keep going? Blocks stay on until you choose.",
                        );
                    }
                }
                state.save_session();
            }

            if last_save.elapsed() >= SAVE_EVERY {
                state.save_session();
                last_save = Instant::now();
            }

            state.bridge.set_rules(state.site_rules());
            state.enforce(&app);

            while let Ok(ev) = bridge_events.try_recv() {
                match ev {
                    BridgeEvent::Blocked { domain } => {
                        if state.engine.lock().unwrap().blocking() {
                            state.record_block(BlockKind::Site, &domain);
                        }
                    }
                    BridgeEvent::ClientsChanged => {}
                }
            }

            let hard = state.engine.lock().unwrap().session().is_some_and(|s| s.config.strictness == Strictness::Hard);
            if hard {
                state.ensure_guard();
            }

            if last_schedule_check.elapsed() >= SCHEDULE_EVERY {
                last_schedule_check = Instant::now();
                // Picks up Monday's reset.
                state.refresh_bails();
                let now_local = Local::now().naive_local();
                let due: Vec<String> = {
                    let config = state.config.lock().unwrap();
                    scheduler::due(&config.schedules, schedule_cursor, now_local)
                        .into_iter()
                        .map(|s| s.preset_id.clone())
                        .collect()
                };
                schedule_cursor = now_local;
                if let Some(preset_id) = due.first() {
                    if !state.engine.lock().unwrap().is_running() && state.start_preset(&app, preset_id).is_ok() {
                        show_main(&app);
                        state.toast(&app, "schedule", "Scheduled session started", "Time to lock in.");
                    }
                }
            }

            let view = state.engine.lock().unwrap().view();
            state.update_tray(&app, view.as_ref());
            state.emit(&app);
        }
    });
}
