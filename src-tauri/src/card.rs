use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use tolk_core::engine::Rect;

#[derive(Clone, serde::Serialize)]
pub struct CardData {
    original: String,
    translation: String,
    above: i32,
}

static DATA: Mutex<Option<CardData>> = Mutex::new(None);
static NEXT: AtomicU32 = AtomicU32::new(0);

const PAD_X: i32 = 32;
const PAD_TOP: i32 = 8;

pub fn show(app: &AppHandle, original: String, translation: String, rect: Option<Rect>) {
    let r = rect.unwrap_or_else(cursor);
    *DATA.lock().unwrap() = Some(CardData { original, translation, above: r.top - 8 });
    for (label, w) in app.webview_windows() {
        if label.starts_with("card-") {
            let _ = w.close();
        }
    }
    let (x, y) = (r.left - PAD_X, r.bottom + 8 - PAD_TOP);
    let label = format!("card-{}", NEXT.fetch_add(1, SeqCst));
    let app = app.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
            .title("Tolk")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .inner_size(600.0, 200.0)
            .position(0.0, 0.0)
            .build()
            .and_then(|w| w.set_position(PhysicalPosition::new(x, y)));
        if let Err(e) = built {
            eprintln!("card: {e}");
        }
    });
}

fn cursor() -> Rect {
    let mut p = windows::Win32::Foundation::POINT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p);
    }
    Rect { left: p.x, top: p.y, right: p.x, bottom: p.y + 8 }
}

#[tauri::command]
pub fn card_data() -> Option<CardData> {
    DATA.lock().unwrap().clone()
}

#[tauri::command]
pub fn copy_text(text: String) -> bool {
    tolk_core::win::clipboard::copy(&text)
}
