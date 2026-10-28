// Release builds have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // The same executable doubles as the Hard-mode watchdog, so there's no
    // extra binary to bundle.
    if args.iter().any(|a| a == "--guard") {
        getlocked_desktop_lib::guard::run(&args);
        return;
    }
    getlocked_desktop_lib::run();
}
