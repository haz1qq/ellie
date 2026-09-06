use std::sync::atomic::Ordering;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

use crate::commands::AppState;

fn open(app: &tauri::AppHandle, settings: bool) -> tauri::Result<()> {
    app.state::<AppState>()
        .settings_view
        .store(settings, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        window.emit("navigate", if settings { "settings" } else { "dashboard" })?;
    }
    Ok(())
}

pub fn create(app: &tauri::AppHandle) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Open Ellie", true, None::<&str>)?;
    let refresh = MenuItem::with_id(
        app,
        "refresh",
        "Refresh — no providers yet",
        false,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Ellie", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &refresh, &settings, &separator, &quit])?;
    TrayIconBuilder::with_id("ellie")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray.png"
        ))?)
        .tooltip("Ellie — no usage data yet")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let result = match event.id.as_ref() {
                "open" => open(app, false),
                "settings" => open(app, true),
                "quit" => {
                    tracing::info!(event = "quit_requested");
                    app.exit(0);
                    Ok(())
                }
                _ => Ok(()),
            };
            if result.is_err() {
                tracing::warn!(event = "tray_action_failed");
            }
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) && open(tray.app_handle(), false).is_err()
            {
                tracing::warn!(event = "tray_open_failed");
            }
        })
        .build(app)?;
    Ok(())
}
