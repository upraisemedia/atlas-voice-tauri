//! The floating pill shown while dictating.
//!
//! macOS: a non-activating NSPanel (via tauri-nspanel), so showing it never
//! steals focus from the app the user is typing in. It floats above
//! full-screen apps and follows the user across Spaces. The panel is hidden
//! (not just transparent) when idle: a visible transparent window keeps
//! WindowServer compositing every frame (tauri#15471).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, LogicalPosition, Manager, WebviewUrl};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask,
};

use crate::dictation::Phase;

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 240.0;
const HEIGHT: f64 = 64.0;
/// Distance between the pill and the bottom of the usable screen area (Dock).
const BOTTOM_MARGIN: f64 = 12.0;
/// Lets the "done" state register before the pill disappears.
const HIDE_DELAY: Duration = Duration::from_millis(450);
pub const NOTICE_DURATION: Duration = Duration::from_secs(3);

/// Bumped on every show; a pending hide only runs if nothing was shown since.
static GENERATION: AtomicU64 = AtomicU64::new(0);

tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    PanelBuilder::<_, OverlayPanel>::new(app, LABEL)
        .url(WebviewUrl::App("overlay.html".into()))
        .size(tauri::Size::Logical(tauri::LogicalSize {
            width: WIDTH,
            height: HEIGHT,
        }))
        .level(PanelLevel::Status)
        .has_shadow(false)
        .transparent(true)
        .no_activate(true)
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary(),
        )
        .with_window(|w| {
            w.decorations(false)
                .transparent(true)
                .focusable(false)
                .resizable(false)
                .skip_taskbar(true)
                .visible(false)
        })
        .build()?;
    Ok(())
}

/// Shows or schedules hiding of the pill for a dictation phase.
pub fn on_phase(app: &AppHandle, phase: Phase) {
    match phase {
        Phase::Idle => hide_after(app, HIDE_DELAY),
        _ => show(app),
    }
}

/// Shows the pill for a notice (error, "text on clipboard") while idle.
pub fn flash(app: &AppHandle) {
    show(app);
    hide_after(app, NOTICE_DURATION);
}

fn show(app: &AppHandle) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        position_on_active_screen(&handle);
        if let Ok(panel) = handle.get_webview_panel(LABEL) {
            if !panel.is_visible() {
                panel.show();
            }
        }
    });
}

fn hide_after(app: &AppHandle, delay: Duration) {
    let generation = GENERATION.load(Ordering::SeqCst);
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        let inner = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if GENERATION.load(Ordering::SeqCst) == generation {
                if let Ok(panel) = inner.get_webview_panel(LABEL) {
                    panel.hide();
                }
            }
        });
    });
}

/// Centres the pill at the bottom of the screen the mouse is on.
fn position_on_active_screen(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let x = area.position.x as f64 / scale + (area.size.width as f64 / scale - WIDTH) / 2.0;
    let y = (area.position.y as f64 + area.size.height as f64) / scale - HEIGHT - BOTTOM_MARGIN;
    let _ = window.set_position(LogicalPosition::new(x.round(), y.round()));
}
