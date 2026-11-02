//! GetLocked desktop app: Tauri wiring. The interesting logic lives in
//! `getlocked-core` (session, rules, stats) and in `enforcer` (closing apps).

mod apps_index;
mod bridge;
mod commands;
mod enforcer;
mod extension_setup;
pub mod guard;
mod popup;
mod runtime;

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};

use runtime::{show_main, AppState};

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just focuses the running app.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|a| a == "--hidden") {
                show_main(app);
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let (tx, rx) = std::sync::mpsc::channel();
            let bridge = bridge::Bridge::start(tx, dir.join("pairing.json"));
            let state = Arc::new(AppState::load(dir, bridge));
            app.manage(state.clone());

            let status = MenuItem::with_id(app, "status", "No session running", false, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "Open GetLocked", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit GetLocked", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&status, &PredefinedMenuItem::separator(app)?, &open, &quit])?;
            *state.tray_status.lock().unwrap() = Some(status);
            let mut tray = TrayIconBuilder::with_id("main")
                .tooltip("GetLocked")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => {
                        let state = app.state::<Arc<AppState>>();
                        if let Err(msg) = commands::try_quit(app, &state) {
                            show_main(app);
                            let _ = app.emit("quit-blocked", msg);
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left, button_state: MouseButtonState::Up, ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            let args: Vec<String> = std::env::args().collect();
            let resumed = state.engine.lock().unwrap().is_running();
            if !args.iter().any(|a| a == "--hidden") || resumed {
                show_main(app.handle());
            }

            // The block-message popup is optional: the notification still works without it.
            if let Err(e) = popup::create(app.handle()) {
                eprintln!("couldn't create the popup window: {e}");
            }

            runtime::spawn_ticker(app.handle().clone(), state, rx);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing a window just hides it; GetLocked keeps running in
            // the tray so sessions and schedules keep working.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == popup::LABEL {
                    popup::hide(window.app_handle());
                } else {
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::start_session,
            commands::start_preset,
            commands::add_task,
            commands::edit_task,
            commands::tick_task,
            commands::untick_task,
            commands::drop_task,
            commands::check_note,
            commands::begin_unlock,
            commands::unlock_status,
            commands::cancel_unlock,
            commands::end_session,
            commands::time_up_choice,
            commands::save_preset,
            commands::delete_preset,
            commands::save_schedule,
            commands::delete_schedule,
            commands::save_settings,
            commands::list_running_apps,
            commands::list_installed_apps,
            commands::get_stats,
            commands::normalize_domain,
            commands::quit_app,
            commands::list_browsers,
            commands::prepare_extension,
            commands::reveal_extension,
            commands::reset_pairing_code,
            commands::open_extensions_page,
            popup::popup_ready,
            popup::popup_dismiss,
        ])
        .run(tauri::generate_context!())
        .expect("error while running GetLocked");
}
