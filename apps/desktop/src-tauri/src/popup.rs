//! GetLocked's own popup for block messages: a small always-on-top card at
//! the bottom-right of the primary monitor, shown next to the Windows
//! notification (which users often miss with Do not disturb on).
//!
//! The popup must never take keyboard focus from what the user is doing. The
//! window is built unfocused and unfocusable (`WS_EX_NOACTIVATE` on Windows),
//! and on Windows it is shown with `SetWindowPos(SWP_NOACTIVATE |
//! SWP_SHOWWINDOW)` instead of Tauri's `show()`, which calls
//! `ShowWindow(SW_SHOW)` and so activates the window.
//!
//! Flow: `show` emits the message to the popup page, which renders it and
//! calls `popup_ready`; only then is the window placed and shown, so the
//! previous message never flashes. A timer hides it after `SHOW_FOR` unless a
//! newer message has replaced it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "popup";
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 120.0;
/// Gap from the edges of the work area (so it sits above the taskbar).
const MARGIN: f64 = 16.0;
const SHOW_FOR: Duration = Duration::from_secs(6);

/// Bumped for every message, so an older timer can't hide a newer message
/// and a late `popup_ready` can't re-show a message that already expired.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
struct Message<'a> {
    id: u64,
    title: &'a str,
    body: &'a str,
}

/// Builds the (hidden) popup window. Called once at startup.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?popup".into()))
        .title("GetLocked")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .shadow(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .focusable(false)
        .build()?;
    Ok(())
}

/// Shows `title` and `body` in the popup, replacing any current message and
/// restarting the hide timer.
pub fn show(app: &AppHandle, title: &str, body: &str) {
    if app.get_webview_window(LABEL).is_none() {
        return;
    }
    let id = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if let Err(e) = app.emit_to(LABEL, "popup", Message { id, title, body }) {
        eprintln!("couldn't send the popup its message: {e}");
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SHOW_FOR);
        if GENERATION.load(Ordering::SeqCst) == id {
            hide(&app);
        }
    });
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        platform::hide(&w);
    }
}

/// The popup page rendered message `id`; show the window if it's still current.
#[tauri::command]
pub fn popup_ready(app: AppHandle, id: u64) {
    if GENERATION.load(Ordering::SeqCst) != id {
        return;
    }
    let Some(w) = app.get_webview_window(LABEL) else { return };
    let monitor = match w.primary_monitor() {
        Ok(Some(m)) => m,
        _ => match w.current_monitor() {
            Ok(Some(m)) => m,
            _ => return,
        },
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let px = |v: f64| (v * scale).round() as i32;
    let (width, height, margin) = (px(WIDTH), px(HEIGHT), px(MARGIN));
    let x = area.position.x + area.size.width as i32 - width - margin;
    let y = area.position.y + area.size.height as i32 - height - margin;
    platform::show_at(&w, x, y, width, height);
}

/// The user clicked the popup.
#[tauri::command]
pub fn popup_dismiss(app: AppHandle) {
    hide(&app);
}

#[cfg(windows)]
mod platform {
    use tauri::WebviewWindow;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
    };

    /// Runs `f` with the window's handle on the UI thread that owns it.
    fn on_window(w: &WebviewWindow, f: impl FnOnce(HWND) + Send + 'static) {
        let Ok(hwnd) = w.hwnd() else { return };
        // HWND isn't Send; pass the raw value across.
        let raw = hwnd.0 as isize;
        let _ = w.run_on_main_thread(move || f(HWND(raw as *mut _)));
    }

    pub fn show_at(w: &WebviewWindow, x: i32, y: i32, width: i32, height: i32) {
        on_window(w, move |hwnd| {
            // SWP_NOACTIVATE: show it on top without taking focus.
            let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
            if let Err(e) = unsafe { SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, width, height, flags) } {
                eprintln!("couldn't show the popup: {e}");
            }
        });
    }

    pub fn hide(w: &WebviewWindow) {
        // Hidden with ShowWindow, not Tauri's hide(): Tauri didn't show it,
        // so it still thinks the window is hidden and its hide() is a no-op.
        on_window(w, |hwnd| {
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        });
    }
}

#[cfg(not(windows))]
mod platform {
    use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

    pub fn show_at(w: &WebviewWindow, x: i32, y: i32, width: i32, height: i32) {
        let _ = w.set_size(PhysicalSize::new(width.max(1) as u32, height.max(1) as u32));
        let _ = w.set_position(PhysicalPosition::new(x, y));
        let _ = w.show();
    }

    pub fn hide(w: &WebviewWindow) {
        let _ = w.hide();
    }
}
