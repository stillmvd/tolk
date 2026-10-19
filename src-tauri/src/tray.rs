use std::sync::{Mutex, OnceLock};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};
use tolk_core::win::theme;

const TOOLTIP: &str = "Tolk — Alt+Q переводит слово или выделение";

static MENU: OnceLock<Menu<Wry>> = OnceLock::new();
type Shown = (String, MenuItem<Wry>, PredefinedMenuItem<Wry>);

static UPDATE: Mutex<Option<Shown>> = Mutex::new(None);

#[derive(serde::Deserialize)]
struct Status {
    phase: String,
    available: Option<Available>,
}

#[derive(serde::Deserialize)]
struct Available {
    version: String,
}

pub fn icon(enabled: bool) -> Option<Image<'static>> {
    let bytes: &'static [u8] = match (theme::taskbar_light(), enabled) {
        (false, true) => include_bytes!("../icons/tray/dark-on.png"),
        (false, false) => include_bytes!("../icons/tray/dark-off.png"),
        (true, true) => include_bytes!("../icons/tray/light-on.png"),
        (true, false) => include_bytes!("../icons/tray/light-off.png"),
    };
    Image::from_bytes(bytes).ok()
}

pub fn build(app: &AppHandle, enabled: bool, on_left: impl Fn(&AppHandle) + Send + Sync + 'static) -> tauri::Result<TrayIcon> {
    let menu = Menu::with_items(app, &[
        &MenuItem::with_id(app, "window", "Окно-переводчик", true, None::<&str>)?,
        &MenuItem::with_id(app, "area", "Перевести область экрана", true, Some("Alt+Shift+Q"))?,
        &PredefinedMenuItem::separator(app)?,
        &CheckMenuItem::with_id(app, "enabled", "Включено", true, enabled, None::<&str>)?,
        &MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?,
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(app, "quit", "Выйти", true, None::<&str>)?,
    ])?;
    let _ = MENU.set(menu.clone());
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "quit" => app.exit(0),
            "enabled" => crate::set_enabled(app, !crate::engine().config().enabled),
            "window" => crate::ui::open_translator(app, None, None),
            "area" => crate::area::start(app.clone()),
            "settings" => crate::ui::open_settings(app),
            "update" => crate::ui::request_install(app),
            _ => {}
        })
        .on_tray_icon_event(move |tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                on_left(tray.app_handle());
            }
        });
    if let Some(img) = icon(enabled) {
        builder = builder.icon(img);
    }
    builder.build(app)
}

pub fn update_status(app: &AppHandle, payload: &str) {
    let Ok(s) = serde_json::from_str::<Status>(payload) else { return };
    let ready = s.available.filter(|_| s.phase == "ready").map(|a| a.version);
    let (Some(menu), Some(tray)) = (MENU.get(), app.tray_by_id("main")) else { return };
    let mut shown = UPDATE.lock().unwrap();
    if shown.as_ref().map(|u| &u.0) == ready.as_ref() {
        return;
    }
    if let Some((_, item, sep)) = shown.take() {
        let _ = menu.remove(&item);
        let _ = menu.remove(&sep);
    }
    let _ = tray.set_tooltip(Some(TOOLTIP));
    let Some(version) = ready else { return };
    let (Ok(item), Ok(sep)) = (MenuItem::with_id(app, "update", format!("Обновить до {version} и перезапустить"), true, None::<&str>), PredefinedMenuItem::separator(app)) else { return };
    let _ = menu.insert(&item, 0);
    let _ = menu.insert(&sep, 1);
    let _ = tray.set_tooltip(Some(format!("Tolk — готово обновление {version}")));
    *shown = Some((version, item, sep));
}
