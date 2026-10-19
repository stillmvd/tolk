mod card;
mod hint;
mod tray;
mod ui;

use hint::Hint;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::tray::TrayIcon;
use tauri::AppHandle;
use tolk_core::config::Config;
use tolk_core::engine::{CoreEvent, Engine};
use tolk_core::win::context;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE};

static TRAY: OnceLock<TrayIcon> = OnceLock::new();
static ENGINE: OnceLock<Engine> = OnceLock::new();
static TRAY_BUSY: AtomicBool = AtomicBool::new(false);

pub fn engine() -> &'static Engine {
    ENGINE.get().expect("engine")
}

pub fn apply_config(app: &AppHandle, cfg: Config) {
    if let Some(t) = TRAY.get() {
        let _ = t.set_icon(tray::icon(cfg.enabled));
    }
    ui::emit_config(app, &cfg);
    engine().set_config(cfg);
    if ui::RECORDING.load(SeqCst) {
        tolk_core::win::hook::ENABLED.store(false, SeqCst);
    }
}

pub fn set_enabled(app: &AppHandle, on: bool) {
    let mut cfg = engine().config();
    cfg.enabled = on;
    let _ = cfg.save();
    apply_config(app, cfg);
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
            CoreEvent::Window => {
                ui::open_translator(&app, None);
                continue;
            }
            _ => continue,
        };
        let _ = hint.send(h);
    }
}

fn tray_click(app: &AppHandle) {
    if TRAY_BUSY.swap(true, SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if focus_last() {
            engine().translate_now();
        } else {
            ui::open_translator(&app, None);
        }
        TRAY_BUSY.store(false, SeqCst);
    });
}

fn focus_last() -> bool {
    let hwnd = context::last_foreign();
    if hwnd.is_invalid() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return false;
    }
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if unsafe { GetForegroundWindow() } == hwnd {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if args.iter().any(|a| a == "--settings") {
                ui::open_settings(app);
            } else if args.iter().any(|a| a == "--translator") {
                ui::open_translator(app, None);
            }
        }))
        .invoke_handler(tauri::generate_handler![
            card::card_data,
            card::copy_text,
            ui::open_translator_with,
            ui::translator_text,
            ui::hide_translator,
            ui::translate_text,
            ui::get_config,
            ui::set_config,
            ui::set_recording,
            ui::hotkey_free,
            ui::pick_program,
        ])
        .setup(|app| {
            let (tx, rx) = crossbeam_channel::unbounded();
            let _ = ENGINE.set(Engine::start(Config::load(), tx));
            let handle = app.handle().clone();
            std::thread::Builder::new().name("tolk-events".into()).spawn(move || pump(handle, rx))?;
            let tray = tray::build(app.handle(), engine().config().enabled, tray_click)?;
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
