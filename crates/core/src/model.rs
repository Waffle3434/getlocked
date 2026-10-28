//! Data types shared by the engine, the stored config and the UI.
//!
//! Every type here is serialized as camelCase JSON so the React UI can use it
//! directly (see `apps/desktop/src/types.ts`, which mirrors these).

use serde::{Deserialize, Serialize};

/// How hard it is to end a session early.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Strictness {
    /// End any time.
    Easy,
    /// End early only by typing an unlock code.
    Friction,
    /// No early end. Survives app kills and reboots.
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SessionKind {
    /// Count down from `duration_secs` of focus time.
    Timed { duration_secs: u64 },
    /// Count up until the work is done. In Hard mode the session can't be
    /// ended before `min_secs` of focus time.
    Stopwatch { min_secs: Option<u64> },
}

/// Whether a rule list is ignored, blocks what's listed, or allows only
/// what's listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListMode {
    #[default]
    Off,
    Block,
    Allow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppRule {
    /// Match an executable by file name, e.g. `steam.exe`.
    Exe { name: String },
    /// Match anything running from inside a folder, e.g. a Steam library.
    Folder { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRules {
    pub mode: ListMode,
    pub rules: Vec<AppRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteRules {
    pub mode: ListMode,
    /// Bare domains such as `youtube.com`. Subdomains are included.
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroCfg {
    pub focus_secs: u64,
    pub break_secs: u64,
}

/// Everything needed to start a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    /// Shown in the UI and stats, e.g. the preset name.
    #[serde(default)]
    pub label: Option<String>,
    pub kind: SessionKind,
    pub strictness: Strictness,
    #[serde(default)]
    pub apps: AppRules,
    #[serde(default)]
    pub sites: SiteRules,
    #[serde(default)]
    pub pomodoro: Option<PomodoroCfg>,
    /// Close browsers that don't have the GetLocked extension connected.
    /// Only applies when site rules are on and strictness isn't Easy.
    #[serde(default = "default_true")]
    pub require_extension: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub config: SessionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: String,
    pub preset_id: String,
    /// 0 = Monday … 6 = Sunday.
    pub days: Vec<u8>,
    /// Minutes after local midnight.
    pub start_minute: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Start GetLocked (hidden in the tray) when you sign in to Windows.
    pub autostart: bool,
    pub notifications: bool,
    /// Seconds a browser may run without the extension connected before it's
    /// closed (when the extension is required).
    pub extension_grace_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self { autostart: false, notifications: true, extension_grace_secs: 10 }
    }
}

/// The user's saved configuration (`config.json`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub schedules: Vec<Schedule>,
    #[serde(default)]
    pub settings: Settings,
}

impl Config {
    pub fn preset(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_config_round_trips_as_camel_case_json() {
        let cfg = SessionConfig {
            label: Some("Coding".into()),
            kind: SessionKind::Stopwatch { min_secs: Some(1800) },
            strictness: Strictness::Hard,
            apps: AppRules { mode: ListMode::Block, rules: vec![AppRule::Exe { name: "steam.exe".into() }] },
            sites: SiteRules { mode: ListMode::Block, domains: vec!["youtube.com".into()] },
            pomodoro: None,
            require_extension: true,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains(r#""kind":{"type":"stopwatch","minSecs":1800}"#), "{json}");
        assert!(json.contains(r#""requireExtension":true"#), "{json}");
        let back: SessionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn missing_optional_fields_use_defaults() {
        let cfg: SessionConfig =
            serde_json::from_str(r#"{"kind":{"type":"timed","durationSecs":60},"strictness":"easy"}"#).unwrap();
        assert_eq!(cfg.apps.mode, ListMode::Off);
        assert!(cfg.require_extension);
    }
}
