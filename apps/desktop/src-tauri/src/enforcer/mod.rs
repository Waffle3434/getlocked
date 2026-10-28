//! Closes apps that break the session's rules.
//!
//! The OS-specific part is behind `Platform`. To add macOS or Linux, implement
//! it in a new module and return it from `platform()`. Everything else here
//! is shared.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use getlocked_core::matcher::{browser_family, is_shell_process, should_close, ProcessInfo};
use getlocked_core::model::AppRules;

#[cfg(windows)]
pub mod windows;

/// How long an app gets to close itself after being asked before it's ended.
const CLOSE_GRACE: Duration = Duration::from_secs(2);

pub trait Platform: Send {
    /// Processes in the current user's session that we're allowed to touch.
    fn snapshot(&mut self) -> Vec<ProcessInfo>;
    /// Politely ask the process to close (e.g. WM_CLOSE to its windows).
    /// Returns false if there was nothing to ask, so it should be ended now.
    fn request_close(&mut self, pid: u32) -> bool;
    /// End the process.
    fn kill(&mut self, pid: u32);
}

pub fn platform() -> Box<dyn Platform> {
    #[cfg(windows)]
    {
        Box::new(windows::WinPlatform::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(NoopPlatform)
    }
}

pub fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        windows::process_alive(pid)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}

#[cfg(not(windows))]
struct NoopPlatform;

#[cfg(not(windows))]
impl Platform for NoopPlatform {
    fn snapshot(&mut self) -> Vec<ProcessInfo> {
        Vec::new()
    }
    fn request_close(&mut self, _pid: u32) -> bool {
        false
    }
    fn kill(&mut self, _pid: u32) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    /// Matched the app rules.
    Rule,
    /// A browser running without the GetLocked extension connected.
    NoExtension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closed {
    pub exe_name: String,
    pub reason: CloseReason,
}

/// Which browser families the extension requirement applies to right now.
pub struct ExtensionRequirement {
    /// Families with a connected (and, if needed, incognito-enabled)
    /// extension. `chromium` means "some Chromium browser we couldn't
    /// identify" and covers any Chromium family.
    pub connected: HashSet<String>,
    pub grace: Duration,
}

pub struct Enforcer {
    platform: Box<dyn Platform>,
    own_pids: HashSet<u32>,
    /// pid → deadline after which it gets killed.
    pending: HashMap<u32, Instant>,
    /// Browser family → when we first saw it running without the extension.
    unconnected_since: HashMap<&'static str, Instant>,
}

impl Enforcer {
    pub fn new(platform: Box<dyn Platform>) -> Self {
        Self {
            platform,
            own_pids: HashSet::from([std::process::id()]),
            pending: HashMap::new(),
            unconnected_since: HashMap::new(),
        }
    }

    /// Never touch this pid (used for the watchdog).
    pub fn protect(&mut self, pid: u32) {
        self.own_pids.insert(pid);
    }

    /// Visible, non-shell apps, for the app picker.
    pub fn running_apps(&mut self) -> Vec<ProcessInfo> {
        let mut seen = HashSet::new();
        let mut apps: Vec<_> = self
            .platform
            .snapshot()
            .into_iter()
            .filter(|p| p.has_visible_window && !is_shell_process(p) && !self.own_pids.contains(&p.pid))
            .filter(|p| seen.insert(p.exe_name.to_lowercase()))
            .collect();
        apps.sort_by_key(|p| p.exe_name.to_lowercase());
        apps
    }

    /// Forget pending closes and timers, e.g. when a session ends or a break
    /// starts.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.unconnected_since.clear();
    }

    /// Run one enforcement pass. Returns the apps that were newly asked to
    /// close.
    pub fn enforce(&mut self, rules: &AppRules, ext: Option<&ExtensionRequirement>) -> Vec<Closed> {
        let now = Instant::now();
        let procs = self.platform.snapshot();
        let alive: HashSet<u32> = procs.iter().map(|p| p.pid).collect();

        // Anything that didn't close in time gets ended.
        let overdue: Vec<u32> = self
            .pending
            .iter()
            .filter(|(pid, deadline)| alive.contains(pid) && now >= **deadline)
            .map(|(pid, _)| *pid)
            .collect();
        for pid in &overdue {
            self.platform.kill(*pid);
            self.pending.remove(pid);
        }
        self.pending.retain(|pid, _| alive.contains(pid));

        let missing = ext.map(|req| self.browsers_missing_extension(&procs, req, now)).unwrap_or_default();

        let mut closed = Vec::new();
        for p in &procs {
            if self.own_pids.contains(&p.pid) || self.pending.contains_key(&p.pid) || overdue.contains(&p.pid) {
                continue;
            }
            let reason = if should_close(rules, p) {
                CloseReason::Rule
            } else if browser_family(&p.exe_name).is_some_and(|f| missing.contains(f)) {
                CloseReason::NoExtension
            } else {
                continue;
            };
            if self.platform.request_close(p.pid) {
                self.pending.insert(p.pid, now + CLOSE_GRACE);
            } else {
                self.platform.kill(p.pid);
            }
            closed.push(Closed { exe_name: p.exe_name.clone(), reason });
        }
        closed
    }

    /// Browser families that have been running without the extension for
    /// longer than the grace period.
    fn browsers_missing_extension(
        &mut self,
        procs: &[ProcessInfo],
        req: &ExtensionRequirement,
        now: Instant,
    ) -> HashSet<&'static str> {
        let running: HashSet<&'static str> = procs.iter().filter_map(|p| browser_family(&p.exe_name)).collect();
        let covered = |family: &str| {
            req.connected.contains(family) || (family != "firefox" && req.connected.contains("chromium"))
        };
        self.unconnected_since.retain(|f, _| running.contains(f) && !covered(f));
        let mut missing = HashSet::new();
        for family in running {
            if covered(family) {
                continue;
            }
            let since = *self.unconnected_since.entry(family).or_insert(now);
            if now.duration_since(since) >= req.grace {
                missing.insert(family);
            }
        }
        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use getlocked_core::model::{AppRule, ListMode};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Log {
        procs: Vec<ProcessInfo>,
        asked: Vec<u32>,
        killed: Vec<u32>,
    }

    struct Fake(Arc<Mutex<Log>>);

    impl Platform for Fake {
        fn snapshot(&mut self) -> Vec<ProcessInfo> {
            self.0.lock().unwrap().procs.clone()
        }
        fn request_close(&mut self, pid: u32) -> bool {
            let mut log = self.0.lock().unwrap();
            log.asked.push(pid);
            log.procs.iter().any(|p| p.pid == pid && p.has_visible_window)
        }
        fn kill(&mut self, pid: u32) {
            let mut log = self.0.lock().unwrap();
            log.killed.push(pid);
            log.procs.retain(|p| p.pid != pid);
        }
    }

    fn proc(pid: u32, name: &str, windowed: bool) -> ProcessInfo {
        ProcessInfo { pid, exe_name: name.into(), exe_path: None, has_visible_window: windowed }
    }

    fn block(names: &[&str]) -> AppRules {
        AppRules { mode: ListMode::Block, rules: names.iter().map(|n| AppRule::Exe { name: (*n).into() }).collect() }
    }

    fn setup(procs: Vec<ProcessInfo>) -> (Enforcer, Arc<Mutex<Log>>) {
        let log = Arc::new(Mutex::new(Log { procs, ..Default::default() }));
        (Enforcer::new(Box::new(Fake(log.clone()))), log)
    }

    #[test]
    fn asks_windowed_apps_to_close_and_kills_windowless_ones() {
        let (mut e, log) = setup(vec![proc(10, "steam.exe", true), proc(11, "discord.exe", false)]);
        let closed = e.enforce(&block(&["steam.exe", "discord.exe"]), None);
        assert_eq!(closed.len(), 2);
        let log = log.lock().unwrap();
        assert_eq!(log.asked, vec![10, 11]);
        assert_eq!(log.killed, vec![11]);
    }

    #[test]
    fn kills_after_grace_if_still_running_and_does_not_re_report() {
        let (mut e, log) = setup(vec![proc(10, "steam.exe", true)]);
        assert_eq!(e.enforce(&block(&["steam.exe"]), None).len(), 1);
        // Still running, inside grace: not reported again, not killed yet.
        assert!(e.enforce(&block(&["steam.exe"]), None).is_empty());
        assert!(log.lock().unwrap().killed.is_empty());
        e.pending.insert(10, Instant::now() - Duration::from_millis(1));
        e.enforce(&block(&["steam.exe"]), None);
        assert_eq!(log.lock().unwrap().killed, vec![10]);
    }

    #[test]
    fn never_touches_own_process() {
        let me = std::process::id();
        let (mut e, log) = setup(vec![proc(me, "steam.exe", true)]);
        assert!(e.enforce(&block(&["steam.exe"]), None).is_empty());
        assert!(log.lock().unwrap().asked.is_empty());
    }

    #[test]
    fn closes_browsers_without_extension_after_grace() {
        let (mut e, _log) = setup(vec![proc(20, "chrome.exe", true), proc(21, "msedge.exe", true)]);
        let mut req = ExtensionRequirement { connected: HashSet::from(["edge".to_string()]), grace: Duration::ZERO };
        let closed = e.enforce(&AppRules::default(), Some(&req));
        assert_eq!(closed, vec![Closed { exe_name: "chrome.exe".into(), reason: CloseReason::NoExtension }]);

        // An unidentified Chromium extension covers Chrome but not Firefox.
        let (mut e, _log) = setup(vec![proc(30, "chrome.exe", true), proc(31, "firefox.exe", true)]);
        req.connected = HashSet::from(["chromium".to_string()]);
        let closed = e.enforce(&AppRules::default(), Some(&req));
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].exe_name, "firefox.exe");
    }

    #[test]
    fn browser_grace_period_is_respected() {
        let (mut e, _log) = setup(vec![proc(20, "chrome.exe", true)]);
        let req = ExtensionRequirement { connected: HashSet::new(), grace: Duration::from_secs(60) };
        assert!(e.enforce(&AppRules::default(), Some(&req)).is_empty());
    }
}
