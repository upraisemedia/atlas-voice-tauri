//! Global hotkeys via handy-keys (CGEventTap on macOS).
//!
//! The event tap needs the Accessibility permission. Until it is granted the
//! hotkey thread polls every second, then registers the keys. handy-keys
//! re-enables the tap itself when macOS disables it after a timeout.

use std::time::Duration;

use handy_keys::{Hotkey, HotkeyManager, HotkeyState, Key, Modifiers};

const PERMISSION_POLL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    DictateDown,
    DictateUp,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyStatus {
    WaitingForAccessibility,
    Active,
    Failed,
}

/// Starts the hotkey thread. Phase 1 uses a fixed binding: hold Fn to dictate,
/// Esc to cancel. Callbacks run on the hotkey thread and must return quickly.
pub fn spawn(
    on_action: impl Fn(HotkeyAction) + Send + 'static,
    on_status: impl Fn(HotkeyStatus) + Send + 'static,
) {
    std::thread::Builder::new()
        .name("hotkey".into())
        .spawn(move || run(on_action, on_status))
        .expect("failed to spawn hotkey thread");
}

fn run(on_action: impl Fn(HotkeyAction), on_status: impl Fn(HotkeyStatus)) {
    if !handy_keys::check_accessibility() {
        on_status(HotkeyStatus::WaitingForAccessibility);
        log::info!("waiting for Accessibility permission");
        while !handy_keys::check_accessibility() {
            std::thread::sleep(PERMISSION_POLL);
        }
    }

    let registered = HotkeyManager::new().and_then(|manager| {
        // Hotkey::new takes an optional key: `None` means modifier-only (just Fn).
        let dictate = manager.register(Hotkey::new(Modifiers::FN, None)?)?;
        let cancel = manager.register(Hotkey::new(Modifiers::empty(), Key::Escape)?)?;
        Ok((manager, dictate, cancel))
    });
    let (manager, dictate, cancel) = match registered {
        Ok(ok) => ok,
        Err(error) => {
            log::error!("could not register hotkeys: {error}");
            on_status(HotkeyStatus::Failed);
            return;
        }
    };
    log::info!("hotkeys active (Fn to dictate, Esc to cancel)");
    on_status(HotkeyStatus::Active);

    while let Ok(event) = manager.recv() {
        let action = match (event.id, event.state) {
            (id, HotkeyState::Pressed) if id == dictate => HotkeyAction::DictateDown,
            (id, HotkeyState::Released) if id == dictate => HotkeyAction::DictateUp,
            (id, HotkeyState::Pressed) if id == cancel => HotkeyAction::Cancel,
            _ => continue,
        };
        on_action(action);
    }
    log::warn!("hotkey listener stopped");
}
