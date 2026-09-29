//! Application menu (visible while the main window is open) and the single
//! handler for all menu events, including the tray menu.

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::AppHandle;

use super::{lifecycle, windows};

pub const ID_QUIT: &str = "quit";
pub const ID_OPEN: &str = "open";
pub const ID_CLOSE_WINDOW: &str = "close-window";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    // A custom Quit item instead of the predefined one, so Cmd+Q runs our
    // cleanup path (the predefined item bypasses `ExitRequested`, tauri#9198).
    let quit = MenuItem::with_id(app, ID_QUIT, "Stop Atlas Voice", true, Some("CmdOrCtrl+Q"))?;
    let close = MenuItem::with_id(
        app,
        ID_CLOSE_WINDOW,
        "Sluit venster",
        true,
        Some("CmdOrCtrl+W"),
    )?;

    let app_menu = Submenu::with_items(
        app,
        "Atlas Voice",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("Over Atlas Voice"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("Verberg Atlas Voice"))?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    // Without an Edit menu, Cmd+C/V/X/A do not work in the webview's text fields.
    let edit_menu = Submenu::with_items(
        app,
        "Wijzig",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("Herstel"))?,
            &PredefinedMenuItem::redo(app, Some("Opnieuw"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("Knip"))?,
            &PredefinedMenuItem::copy(app, Some("Kopieer"))?,
            &PredefinedMenuItem::paste(app, Some("Plak"))?,
            &PredefinedMenuItem::select_all(app, Some("Selecteer alles"))?,
        ],
    )?;

    let window_menu = Submenu::with_items(
        app,
        "Venster",
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some("Minimaliseer"))?,
            &close,
        ],
    )?;

    let menu = Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu)?;
    Ok(())
}

pub fn handle_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        ID_QUIT => lifecycle::quit(app),
        ID_OPEN => windows::show_main(app),
        ID_CLOSE_WINDOW => windows::hide_main(app),
        _ => {}
    }
}
