use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::window;

const ICON: &[u8] = include_bytes!("../icons/tray.png");

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open OneNote", true, None::<&str>)?;
    let reload = MenuItem::with_id(app, "reload", "Reload", true, None::<&str>)?;
    let clear = MenuItem::with_id(
        app,
        "clear-cache",
        "Clear Cache and Sign Out\u{2026}",
        true,
        None::<&str>,
    )?;
    let sep = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings\u{2026}", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("Ctrl+Q"))?;

    let menu = Menu::with_items(app, &[&open, &reload, &clear, &sep, &settings, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(Image::from_bytes(ICON)?)
        .tooltip("OneNote")
        .menu(&menu)
        // Left click should show the window, not the menu.
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => window::show_main(app),
            "reload" => {
                if let Some(win) = app.get_webview_window(window::MAIN_LABEL) {
                    let _ = win.reload();
                }
            }
            "clear-cache" => window::clear_cache(app),
            "settings" => crate::settings::show_settings_window(app),
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::toggle_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
