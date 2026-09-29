//! Main window visibility and the macOS Dock icon.
//!
//! Atlas Voice is a menu bar app (`LSUIElement`). While the main window is open
//! we switch to the `Regular` activation policy so it gets a Dock icon, the app
//! menu and Cmd+Tab; when it is hidden we go back to `Accessory`.

use tauri::{AppHandle, Manager, WindowEvent};

pub const MAIN_WINDOW: &str = "main";

pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        log::warn!("main window not found");
        return;
    };
    set_dock_visible(app, true);
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn hide_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.hide();
    }
    set_dock_visible(app, false);
}

/// Closing the main window hides it; the app keeps running in the menu bar.
pub fn attach(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            hide_main(&handle);
        }
    });
}

fn set_dock_visible(app: &AppHandle, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        let policy = if visible {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        if let Err(err) = app.set_activation_policy(policy) {
            log::warn!("failed to set activation policy: {err}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, visible);
}
