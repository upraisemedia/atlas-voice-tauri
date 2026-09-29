//! Menu bar (tray) icon. Menu clicks are handled in `menu::handle_event`.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::AppHandle;

use super::menu::{ID_OPEN, ID_QUIT};

const TRAY_ID: &str = "main-tray";
// Template image: macOS recolours it for light/dark menu bars.
const TRAY_ICON: &[u8] = include_bytes!("../../icons/tray/menubar@2x.png");

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, ID_OPEN, "Open Atlas Voice", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ID_QUIT, "Stop Atlas Voice", true, None::<&str>)?,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_ICON)?)
        .icon_as_template(true)
        .tooltip("Atlas Voice")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .build(app)?;
    Ok(())
}
