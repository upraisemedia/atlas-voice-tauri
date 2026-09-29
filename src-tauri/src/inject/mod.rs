//! Putting the transcript into the focused text field of another app.
//!
//! macOS: snapshot the clipboard, put the transcript on it (marked transient
//! so clipboard managers skip it), post Cmd+V, then restore the previous
//! clipboard, but only if nothing else changed it in the meantime.

#[cfg(target_os = "macos")]
mod macos;

use std::sync::mpsc;
use std::time::Duration;

use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Pasted,
    /// Could not paste automatically (no Accessibility permission); the text is
    /// on the clipboard for the user to paste with Cmd+V.
    LeftOnClipboard,
}

/// Gives the target app time to see the new clipboard before Cmd+V.
const BEFORE_PASTE: Duration = Duration::from_millis(30);
/// Time for the target app to read the clipboard before we restore it.
/// Slow (Electron) apps need more than the Swift app's 300 ms.
const BEFORE_RESTORE: Duration = Duration::from_millis(500);
const MAIN_THREAD_TIMEOUT: Duration = Duration::from_secs(2);

/// Pastes `text` into the frontmost app. Blocks for ~0.5 s; call it from a
/// worker thread, never from the main thread (AppKit work is dispatched to it).
#[cfg(target_os = "macos")]
pub fn paste(app: &AppHandle, text: String) -> Result<PasteOutcome, String> {
    if !handy_keys::check_accessibility() {
        on_main(app, move || macos::pasteboard::write_text(&text))?;
        return Ok(PasteOutcome::LeftOnClipboard);
    }

    let (snapshot, our_change) = on_main(app, move || {
        let snapshot = macos::pasteboard::snapshot();
        let change = macos::pasteboard::write_text(&text);
        (snapshot, change)
    })?;
    std::thread::sleep(BEFORE_PASTE);
    on_main(app, macos::keyboard::post_command_v)??;
    std::thread::sleep(BEFORE_RESTORE);
    on_main(app, move || {
        if macos::pasteboard::change_count() == our_change {
            macos::pasteboard::restore(snapshot);
        } else {
            log::debug!("clipboard changed after paste; not restoring");
        }
    })?;
    Ok(PasteOutcome::Pasted)
}

/// Runs `f` on the main thread and waits for its result.
fn on_main<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|e| format!("main thread unavailable: {e}"))?;
    rx.recv_timeout(MAIN_THREAD_TIMEOUT)
        .map_err(|_| "main thread did not respond".to_string())
}
