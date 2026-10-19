use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use tolk_core::config::{parse_hotkey, Config, Mode, MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_WIN};
use tolk_core::text::{direction, dress, word_count};
use tolk_core::translate::MAX_TEXT;
use tolk_core::win::{context, hook};
use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, HWND, POINT};
use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
use windows::Win32::UI::Controls::Dialogs::{GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OPENFILENAMEW};
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetForegroundWindow};

static FILL: Mutex<Option<String>> = Mutex::new(None);
static BUILDING: AtomicBool = AtomicBool::new(false);
pub static RECORDING: AtomicBool = AtomicBool::new(false);

const TRANSLATOR_W: f64 = 648.0;

pub fn open_translator(app: &AppHandle, text: Option<String>) {
    if let Some(w) = app.get_webview_window("translator") {
        place(app, &w);
        if let Some(t) = text {
            let _ = w.emit("translator:fill", t);
        }
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("translator:show", ());
        return;
    }
    if BUILDING.swap(true, SeqCst) {
        return;
    }
    *FILL.lock().unwrap() = text;
    let app = app.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&app, "translator", WebviewUrl::App("index.html".into()))
            .title("Переводчик Tolk")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .inner_size(TRANSLATOR_W, 140.0)
            .build();
        match built {
            Ok(w) => place(&app, &w),
            Err(e) => eprintln!("translator: {e}"),
        }
        BUILDING.store(false, SeqCst);
    });
}

fn place(app: &AppHandle, w: &tauri::WebviewWindow) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    let Ok(Some(mon)) = app.monitor_from_point(p.x as f64, p.y as f64) else { return };
    let (pos, size) = (mon.position(), mon.size());
    let width = (TRANSLATOR_W * mon.scale_factor()) as i32;
    let x = pos.x + (size.width as i32 - width) / 2;
    let y = pos.y + size.height as i32 / 5;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

pub fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("index.html".into()))
            .title("Настройки Tolk")
            .decorations(false)
            .shadow(true)
            .inner_size(760.0, 540.0)
            .min_inner_size(640.0, 420.0)
            .maximizable(false)
            .center()
            .build();
        match built {
            Ok(w) => w.on_window_event(|e| {
                if let tauri::WindowEvent::Destroyed = e {
                    set_recording(false);
                }
            }),
            Err(e) => eprintln!("settings: {e}"),
        }
    });
}

#[tauri::command]
pub fn open_translator_with(app: AppHandle, text: Option<String>) {
    open_translator(&app, text);
}

#[tauri::command]
pub fn translator_text() -> Option<String> {
    FILL.lock().unwrap().take()
}

#[tauri::command]
pub fn hide_translator(app: AppHandle, restore: bool) {
    if let Some(w) = app.get_webview_window("translator") {
        let _ = w.hide();
    }
    let last = context::last_foreign();
    if restore && !last.is_invalid() {
        unsafe {
            let _ = SetForegroundWindow(last);
        }
    }
}

#[derive(serde::Serialize)]
pub struct Translated {
    text: String,
    variants: Vec<String>,
    from: &'static str,
    to: &'static str,
}

#[tauri::command]
pub async fn translate_text(text: String, flip: bool) -> Result<Option<Translated>, String> {
    let core = text.trim().to_string();
    let Some(mut dir) = direction(&core) else { return Ok(None) };
    if flip {
        dir = dir.flip();
    }
    if core.chars().count() > MAX_TEXT {
        return Err("Слишком длинный текст — вставь меньше 20 000 символов".into());
    }
    let engine = crate::engine();
    let offline = engine.config().mode == Mode::Offline;
    let translator = engine.translator.clone();
    let src = core.clone();
    let got = tauri::async_runtime::spawn_blocking(move || translator.translate_long(&src, dir, offline)).await.map_err(|e| e.to_string())?;
    let t = got.map_err(|f| f.message().to_string())?;
    let variants = if word_count(&core) == 1 { t.variants.iter().map(|v| dress(&core, v)).collect() } else { Vec::new() };
    Ok(Some(Translated { text: dress(&core, &t.text), variants, from: dir.from().code(), to: dir.to().code() }))
}

#[tauri::command]
pub fn get_config() -> Config {
    crate::engine().config()
}

#[tauri::command]
pub fn set_config(app: AppHandle, cfg: Config) -> Result<(), String> {
    let hotkeys = [Some(cfg.hotkey.as_str()), Some(cfg.area_hotkey.as_str()), cfg.window_hotkey.as_deref()];
    let codes: Vec<Option<u32>> = hotkeys.into_iter().flatten().map(parse_hotkey).collect();
    if codes.iter().any(Option::is_none) {
        return Err("Не понял сочетание — нужен Ctrl, Alt или Win и клавиша".into());
    }
    if codes.iter().enumerate().any(|(i, c)| codes[..i].contains(c)) {
        return Err("Одно сочетание на два действия — выбери другое".into());
    }
    let toggle = crate::engine().config().autostart != cfg.autostart;
    if toggle {
        autostart(cfg.autostart)?;
    }
    if let Err(e) = cfg.save() {
        if toggle {
            let _ = autostart(!cfg.autostart);
        }
        return Err(format!("Не получилось сохранить настройки: {e}"));
    }
    crate::apply_config(&app, cfg);
    Ok(())
}

#[tauri::command]
pub fn set_recording(on: bool) {
    RECORDING.store(on, SeqCst);
    hook::ENABLED.store(!on && crate::engine().config().enabled, SeqCst);
}

#[tauri::command]
pub fn hotkey_free(combo: String) -> bool {
    let Some(code) = parse_hotkey(&combo) else { return false };
    let mut mods = MOD_NOREPEAT.0;
    for (ours, win) in [(MOD_ALT, 0x1), (MOD_CTRL, 0x2), (MOD_SHIFT, 0x4), (MOD_WIN, 0x8)] {
        if code & ours != 0 {
            mods |= win;
        }
    }
    unsafe {
        if RegisterHotKey(None, 0x7A01, HOT_KEY_MODIFIERS(mods), code & 0xFFFF).is_err() {
            return false;
        }
        let _ = UnregisterHotKey(None, 0x7A01);
    }
    true
}

#[tauri::command]
pub fn pick_program(app: AppHandle) -> Option<String> {
    let owner = app.get_webview_window("settings").and_then(|w| w.hwnd().ok()).map(|h| HWND(h.0));
    let mut buf = [0u16; 1024];
    let filter: Vec<u16> = "Программы (*.exe)\0*.exe\0\0".encode_utf16().collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner.unwrap_or_default(),
        lpstrFilter: windows::core::PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let path = String::from_utf16_lossy(&buf[..len]);
    std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_lowercase())
}

fn autostart(on: bool) -> Result<(), String> {
    let key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    unsafe {
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let value: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain(Some(0)).collect();
            RegSetKeyValueW(HKEY_CURRENT_USER, key, w!("Tolk"), REG_SZ.0, Some(value.as_ptr().cast()), (value.len() * 2) as u32)
                .ok()
                .map_err(|e| format!("Не получилось включить автозапуск: {e}"))
        } else {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, key, w!("Tolk"));
            if r.is_err() && r != ERROR_FILE_NOT_FOUND {
                return Err(format!("Не получилось выключить автозапуск: {}", r.to_hresult().message()));
            }
            Ok(())
        }
    }
}

pub fn emit_config(app: &AppHandle, cfg: &Config) {
    let _ = app.emit("config:changed", cfg);
}
