//! Hard-mode watchdog.
//!
//! During a Hard session the app starts a second copy of itself with
//! `--guard`. The guard watches the app and relaunches it if it's killed; the
//! app restarts the guard if *it's* killed. The guard exits by itself once no
//! Hard session is saved on disk.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use getlocked_core::model::Strictness;
use getlocked_core::persist::load_json;
use getlocked_core::session::Session;

use crate::enforcer::process_alive;

fn arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn hard_session_saved(session_file: &Path) -> bool {
    matches!(load_json::<Session>(session_file), Ok(Some(s)) if s.config.strictness == Strictness::Hard)
}

#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// Entry point for `GetLocked.exe --guard --parent <pid> --session <path>`.
pub fn run(args: &[String]) {
    let Some(parent) = arg_value(args, "--parent").and_then(|p| p.parse::<u32>().ok()) else { return };
    let Some(session_file) = arg_value(args, "--session").map(PathBuf::from) else { return };
    let Ok(exe) = std::env::current_exe() else { return };

    loop {
        std::thread::sleep(Duration::from_secs(1));
        if !hard_session_saved(&session_file) {
            return;
        }
        if !process_alive(parent) {
            let mut cmd = Command::new(&exe);
            cmd.arg("--resume");
            hide_console(&mut cmd);
            // The relaunched app starts its own guard, so this one is done.
            // If launching failed, try again next second.
            if cmd.spawn().is_ok() {
                return;
            }
        }
    }
}

/// Start a guard for this process.
pub fn spawn(session_file: &Path) -> std::io::Result<Child> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("--guard").arg("--parent").arg(std::process::id().to_string()).arg("--session").arg(session_file);
    hide_console(&mut cmd);
    cmd.spawn()
}
