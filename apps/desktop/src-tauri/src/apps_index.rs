//! Lists installed apps for the app picker by reading Start Menu shortcuts.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    /// The shortcut's name, e.g. "Steam".
    pub name: String,
    /// e.g. `steam.exe`.
    pub exe_name: String,
    pub path: String,
}

fn start_menu_dirs() -> Vec<PathBuf> {
    ["ProgramData", "APPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"))
        .collect()
}

fn find_shortcuts(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth < 4 {
            find_shortcuts(&path, out, depth + 1);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            out.push(path);
        }
    }
}

#[cfg(windows)]
fn resolve(link: &Path) -> Option<String> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = shell_link.cast().ok()?;
        file.Load(&HSTRING::from(link.as_os_str()), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        shell_link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let target = String::from_utf16_lossy(&buf[..end]);
        (!target.is_empty()).then_some(target)
    }
}

#[cfg(not(windows))]
fn resolve(_link: &Path) -> Option<String> {
    None
}

/// Runs on its own thread because COM needs per-thread setup.
pub fn installed_apps() -> Vec<InstalledApp> {
    std::thread::spawn(|| {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let mut links = Vec::new();
        for dir in start_menu_dirs() {
            find_shortcuts(&dir, &mut links, 0);
        }
        let mut seen = HashSet::new();
        let mut apps: Vec<InstalledApp> = links
            .iter()
            .filter_map(|link| {
                let name = link.file_stem()?.to_string_lossy().to_string();
                let lower = name.to_lowercase();
                if lower.contains("uninstall") || lower.contains("readme") || lower.contains("help") {
                    return None;
                }
                let path = resolve(link)?;
                if !path.to_lowercase().ends_with(".exe") {
                    return None;
                }
                let exe_name = Path::new(&path).file_name()?.to_string_lossy().to_string();
                seen.insert(path.to_lowercase()).then_some(InstalledApp { name, exe_name, path })
            })
            .collect();
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps
    })
    .join()
    .unwrap_or_default()
}
