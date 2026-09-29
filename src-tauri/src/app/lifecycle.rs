//! Orderly shutdown.
//!
//! Tauri does not drop managed state on exit (tauri#14420) and does not emit
//! `ExitRequested` for a system Cmd+Q (tauri#9198). Every subsystem that owns
//! native resources (audio streams, loaded models, event taps) must therefore be
//! released explicitly here, before the process exits.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;

static SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);

/// The only sanctioned way to quit: clean up first, then exit.
pub fn quit(app: &AppHandle) {
    shutdown(app);
    app.exit(0);
}

/// Releases native resources. Idempotent, because both the explicit quit path
/// and `RunEvent::Exit` call it.
pub fn shutdown(app: &AppHandle) {
    if SHUTDOWN_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    log::info!("shutting down");
    crate::services::shutdown(app);
    log::info!("shutdown complete");
}
