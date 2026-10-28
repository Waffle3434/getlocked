//! Decides which running processes a session should close.
//!
//! This is pure logic: the OS-specific enforcer produces `ProcessInfo`
//! snapshots and closes whatever `should_close` returns true for.

use crate::model::{AppRule, AppRules, ListMode};

/// What the enforcer knows about one running process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    /// File name, e.g. `steam.exe`.
    pub exe_name: String,
    /// Full path, when the process could be opened to read it.
    pub exe_path: Option<String>,
    /// Owns at least one visible, un-owned top-level window.
    pub has_visible_window: bool,
}

/// Windows shell and system processes that "allow only" mode must never
/// close, or the desktop itself would stop working. Compared lowercase.
pub const SHELL_ALLOW: &[&str] = &[
    "explorer.exe",
    "searchhost.exe",
    "searchapp.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "shellhost.exe",
    "textinputhost.exe",
    "applicationframehost.exe",
    "lockapp.exe",
    "taskmgr.exe",
    "systemsettings.exe",
    "dwm.exe",
    "csrss.exe",
    "winlogon.exe",
    "ctfmon.exe",
    "sihost.exe",
    "widgets.exe",
    "consent.exe",
    "credentialuibroker.exe",
    "openwith.exe",
    "pickerhost.exe",
    "rundll32.exe",
    "getlocked.exe",
    "getlocked-desktop.exe",
];

/// Browser executables, used to require the extension during a session.
/// The second value is the browser family the extension reports.
pub const BROWSERS: &[(&str, &str)] = &[
    ("chrome.exe", "chrome"),
    ("msedge.exe", "edge"),
    ("brave.exe", "brave"),
    ("opera.exe", "opera"),
    ("vivaldi.exe", "vivaldi"),
    ("firefox.exe", "firefox"),
];

pub fn browser_family(exe_name: &str) -> Option<&'static str> {
    let exe = exe_name.to_ascii_lowercase();
    BROWSERS.iter().find(|(e, _)| *e == exe).map(|(_, f)| *f)
}

/// Lowercases and normalizes slashes, with a trailing separator so that
/// `C:\Games` doesn't match `C:\GamesSaves`.
fn normalize_dir(path: &str) -> String {
    let mut p = path.trim().replace('/', "\\").to_lowercase();
    if !p.ends_with('\\') {
        p.push('\\');
    }
    p
}

fn exe_file_name(name: &str) -> String {
    let n = name.trim().replace('/', "\\");
    let file = n.rsplit('\\').next().unwrap_or(&n);
    file.to_lowercase()
}

pub fn rule_matches(rule: &AppRule, p: &ProcessInfo) -> bool {
    match rule {
        AppRule::Exe { name } => exe_file_name(name) == p.exe_name.to_lowercase(),
        AppRule::Folder { path } => match &p.exe_path {
            Some(exe) => exe.replace('/', "\\").to_lowercase().starts_with(&normalize_dir(path)),
            None => false,
        },
    }
}

pub fn is_shell_process(p: &ProcessInfo) -> bool {
    SHELL_ALLOW.contains(&p.exe_name.to_lowercase().as_str())
}

/// True if this process breaks the rules and should be closed.
///
/// Block mode closes anything matching a rule, windowed or not (so launchers
/// like Steam can't sit in the background). Allow mode only closes processes
/// that show a window, so background helpers and services are left alone.
pub fn should_close(rules: &AppRules, p: &ProcessInfo) -> bool {
    match rules.mode {
        ListMode::Off => false,
        ListMode::Block => !is_shell_process(p) && rules.rules.iter().any(|r| rule_matches(r, p)),
        ListMode::Allow => {
            p.has_visible_window && !is_shell_process(p) && !rules.rules.iter().any(|r| rule_matches(r, p))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(name: &str, path: Option<&str>, windowed: bool) -> ProcessInfo {
        ProcessInfo { pid: 1, exe_name: name.into(), exe_path: path.map(Into::into), has_visible_window: windowed }
    }

    fn exe(name: &str) -> AppRule {
        AppRule::Exe { name: name.into() }
    }

    #[test]
    fn exe_rule_is_case_insensitive_and_accepts_full_paths() {
        let p = proc("Steam.exe", None, false);
        assert!(rule_matches(&exe("steam.exe"), &p));
        assert!(rule_matches(&exe(r"C:\Program Files (x86)\Steam\STEAM.EXE"), &p));
        assert!(!rule_matches(&exe("steamwebhelper.exe"), &p));
    }

    #[test]
    fn folder_rule_matches_children_but_not_sibling_prefixes() {
        let rule = AppRule::Folder { path: r"C:\Games\Steam\steamapps\common".into() };
        let inside = proc("game.exe", Some(r"c:\games\steam\steamapps\common\Hades\Hades.exe"), true);
        let sibling = proc("x.exe", Some(r"C:\Games\Steam\steamapps\commonX\x.exe"), true);
        let unknown = proc("game.exe", None, true);
        assert!(rule_matches(&rule, &inside));
        assert!(!rule_matches(&rule, &sibling));
        assert!(!rule_matches(&rule, &unknown));
    }

    #[test]
    fn folder_rule_accepts_forward_slashes_and_trailing_separator() {
        let rule = AppRule::Folder { path: "C:/Games/".into() };
        assert!(rule_matches(&rule, &proc("a.exe", Some(r"C:\Games\a\a.exe"), true)));
    }

    #[test]
    fn off_mode_never_closes() {
        let rules = AppRules { mode: ListMode::Off, rules: vec![exe("steam.exe")] };
        assert!(!should_close(&rules, &proc("steam.exe", None, true)));
    }

    #[test]
    fn block_mode_closes_matches_even_without_a_window() {
        let rules = AppRules { mode: ListMode::Block, rules: vec![exe("steam.exe")] };
        assert!(should_close(&rules, &proc("steam.exe", None, false)));
        assert!(!should_close(&rules, &proc("code.exe", None, true)));
    }

    #[test]
    fn block_mode_never_closes_the_shell_even_if_listed() {
        let rules = AppRules { mode: ListMode::Block, rules: vec![exe("explorer.exe")] };
        assert!(!should_close(&rules, &proc("explorer.exe", None, true)));
    }

    #[test]
    fn allow_mode_closes_unlisted_windowed_apps_only() {
        let rules = AppRules { mode: ListMode::Allow, rules: vec![exe("Code.exe")] };
        assert!(!should_close(&rules, &proc("code.exe", None, true)));
        assert!(should_close(&rules, &proc("calc.exe", None, true)));
        // Background helpers without windows are left alone.
        assert!(!should_close(&rules, &proc("helper.exe", None, false)));
        // The shell always survives.
        assert!(!should_close(&rules, &proc("explorer.exe", None, true)));
        assert!(!should_close(&rules, &proc("Taskmgr.exe", None, true)));
    }

    #[test]
    fn browser_family_lookup() {
        assert_eq!(browser_family("MSEDGE.exe"), Some("edge"));
        assert_eq!(browser_family("firefox.exe"), Some("firefox"));
        assert_eq!(browser_family("notepad.exe"), None);
    }
}
