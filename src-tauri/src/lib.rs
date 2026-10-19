mod card;
mod hint;
mod tray;

use hint::Hint;
use std::sync::OnceLock;
use tauri::tray::TrayIcon;
use tauri::AppHandle;
use tolk_core::config::Config;
use tolk_core::engine::{CoreEvent, Engine};

static TRAY: OnceLock<TrayIcon> = OnceLock::new();
static ENGINE: OnceLock<Engine> = OnceLock::new();

pub fn engine() -> &'static Engine {
    ENGINE.get().expect("engine")
}

pub fn set_enabled(on: bool) {
    let mut cfg = engine().config();
    cfg.enabled = on;
    let _ = cfg.save();
    engine().set_config(cfg);
    if let Some(t) = TRAY.get() {
        let _ = t.set_icon(tray::icon(on));
    }
}

fn pump(app: AppHandle, rx: crossbeam_channel::Receiver<CoreEvent>) {
    let hint = hint::start(|n| engine().pick(n));
    for ev in rx {
        let h = match ev {
            CoreEvent::Variants { list, current, rect } => Hint::Variants { list, current, rect },
            CoreEvent::Waiting { rect } => Hint::Waiting { rect },
            CoreEvent::HideHint => Hint::Hide,
            CoreEvent::Notice { text, error, rect } => Hint::Notice { text, error, rect },
            CoreEvent::Show { original, translation, rect } => {
                card::show(&app, original, translation, rect);
                continue;
            }
            _ => continue,
        };
        let _ = hint.send(h);
    }
}

#[tauri::command]
fn open_translator(text: String) {
    let _ = text;
    // TODO: фаза 9 — окно-переводчик после стенда 002
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .invoke_handler(tauri::generate_handler![card::card_data, card::copy_text, open_translator])
        .setup(|app| {
            let (tx, rx) = crossbeam_channel::unbounded();
            let _ = ENGINE.set(Engine::start(Config::load(), tx));
            let handle = app.handle().clone();
            std::thread::Builder::new().name("tolk-events".into()).spawn(move || pump(handle, rx))?;
            let tray = tray::build(app.handle(), engine().config().enabled, |_| {})?;
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
