//! Helps the user install the browser extension: finds their browsers, puts
//! a copy of the extension in a stable folder, and opens the browser's
//! extensions page. Nothing is installed automatically; the user still clicks
//! "Load unpacked" themselves.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInfo {
    /// Matches the family the extension reports, e.g. `edge`.
    pub id: &'static str,
    pub name: &'static str,
    /// `chromium` or `firefox`: which extension build it needs.
    pub target: &'static str,
    /// The page where extensions are loaded.
    pub extensions_url: &'static str,
    pub installed: bool,
}

struct Known {
    id: &'static str,
    name: &'static str,
    target: &'static str,
    extensions_url: &'static str,
    /// (environment variable for the base folder, path below it)
    locations: &'static [(&'static str, &'static str)],
}

const KNOWN: &[Known] = &[
    Known {
        id: "chrome",
        name: "Google Chrome",
        target: "chromium",
        extensions_url: "chrome://extensions",
        locations: &[
            ("ProgramFiles", r"Google\Chrome\Application\chrome.exe"),
            ("ProgramFiles(x86)", r"Google\Chrome\Application\chrome.exe"),
            ("LOCALAPPDATA", r"Google\Chrome\Application\chrome.exe"),
        ],
    },
    Known {
        id: "edge",
        name: "Microsoft Edge",
        target: "chromium",
        extensions_url: "edge://extensions",
        locations: &[
            ("ProgramFiles(x86)", r"Microsoft\Edge\Application\msedge.exe"),
            ("ProgramFiles", r"Microsoft\Edge\Application\msedge.exe"),
        ],
    },
    Known {
        id: "brave",
        name: "Brave",
        target: "chromium",
        extensions_url: "brave://extensions",
        locations: &[
            ("ProgramFiles", r"BraveSoftware\Brave-Browser\Application\brave.exe"),
            ("LOCALAPPDATA", r"BraveSoftware\Brave-Browser\Application\brave.exe"),
        ],
    },
    Known {
        id: "opera",
        name: "Opera",
        target: "chromium",
        extensions_url: "opera://extensions",
        locations: &[
            ("LOCALAPPDATA", r"Programs\Opera\opera.exe"),
            ("LOCALAPPDATA", r"Programs\Opera GX\opera.exe"),
            ("ProgramFiles", r"Opera\opera.exe"),
        ],
    },
    Known {
        id: "vivaldi",
        name: "Vivaldi",
        target: "chromium",
        extensions_url: "vivaldi://extensions",
        locations: &[
            ("LOCALAPPDATA", r"Vivaldi\Application\vivaldi.exe"),
            ("ProgramFiles", r"Vivaldi\Application\vivaldi.exe"),
        ],
    },
    Known {
        id: "firefox",
        name: "Firefox",
        target: "firefox",
        extensions_url: "about:debugging#/runtime/this-firefox",
        locations: &[
            ("ProgramFiles", r"Mozilla Firefox\firefox.exe"),
            ("ProgramFiles(x86)", r"Mozilla Firefox\firefox.exe"),
        ],
    },
];

fn find_exe(known: &Known) -> Option<PathBuf> {
    known.locations.iter().find_map(|(var, rel)| {
        let path = PathBuf::from(std::env::var_os(var)?).join(rel);
        path.is_file().then_some(path)
    })
}

/// Every supported browser, installed ones first.
pub fn browsers() -> Vec<BrowserInfo> {
    let mut list: Vec<BrowserInfo> = KNOWN
        .iter()
        .map(|k| BrowserInfo {
            id: k.id,
            name: k.name,
            target: k.target,
            extensions_url: k.extensions_url,
            installed: find_exe(k).is_some(),
        })
        .collect();
    list.sort_by_key(|b| !b.installed);
    list
}

/// Open the browser's extensions page in a new tab.
pub fn open_extensions_page(id: &str) -> Result<(), String> {
    let known = KNOWN.iter().find(|k| k.id == id).ok_or("unknown browser")?;
    let exe = find_exe(known).ok_or_else(|| format!("couldn't find {} on this PC", known.name))?;
    Command::new(exe)
        .arg(known.extensions_url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't open {}: {e}", known.name))
}

/// Where the built extension ships: next to the source in dev builds, in the
/// app's resources in installed builds.
fn source_dir(app: &AppHandle, target: &str) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    let base = {
        let _ = app;
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../extension/dist")
    };
    #[cfg(not(debug_assertions))]
    let base = app.path().resource_dir().map_err(|e| e.to_string())?.join("extension");
    let dir = base.join(target);
    if dir.join("manifest.json").is_file() {
        Ok(dir)
    } else {
        Err("The extension files are missing. In a dev build, run `npm run ext:build` first.".into())
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// The folder the browser should load the extension from. It's a copy in
/// GetLocked's data folder, so the path is short, stays put across app
/// updates, and is refreshed every time setup runs.
pub fn prepare(app: &AppHandle, target: &str) -> Result<PathBuf, String> {
    if target != "chromium" && target != "firefox" {
        return Err("unknown extension target".into());
    }
    let src = source_dir(app, target)?;
    let dest = app.path().app_data_dir().map_err(|e| e.to_string())?.join("extension").join(target);
    copy_dir(&src, &dest).map_err(|e| format!("couldn't copy the extension: {e}"))?;
    Ok(dest)
}

/// Show the prepared extension folder in Explorer.
pub fn reveal(app: &AppHandle, target: &str) -> Result<(), String> {
    let dir = prepare(app, target)?;
    Command::new("explorer.exe").arg(dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}
