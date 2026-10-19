use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

pub fn build(app: &AppHandle, on_left: impl Fn(&AppHandle) + Send + Sync + 'static) -> tauri::Result<TrayIcon> {
    let menu = Menu::with_items(app, &[
        &MenuItem::with_id(app, "window", "Окно-переводчик", true, None::<&str>)?,
        &MenuItem::with_id(app, "area", "Перевести область экрана", true, Some("Alt+Shift+Q"))?,
        &PredefinedMenuItem::separator(app)?,
        &CheckMenuItem::with_id(app, "enabled", "Включено", true, true, None::<&str>)?,
        &MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?,
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(app, "quit", "Выйти", true, None::<&str>)?,
    ])?;
    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Tolk — Alt+Q переводит слово или выделение")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| {
            if e.id.as_ref() == "quit" {
                app.exit(0);
            }
        })
        .on_tray_icon_event(move |tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                on_left(tray.app_handle());
            }
        })
        .build(app)
}
