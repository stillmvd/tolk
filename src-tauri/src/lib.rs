mod tray;

use std::sync::OnceLock;
use tauri::tray::TrayIcon;

static TRAY: OnceLock<TrayIcon> = OnceLock::new();

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .setup(|app| {
            let tray = tray::build(app.handle(), |_| {})?;
            let _ = TRAY.set(tray);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("tolk")
        .run(|_, event| {
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
