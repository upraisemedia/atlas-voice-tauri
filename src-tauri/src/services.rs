//! Wires the subsystems together at startup and tears them down on quit.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::Recorder;
use crate::dictation::{Dictation, Input};
use crate::hotkey::{self, HotkeyAction, HotkeyStatus};
use crate::models::{self, ModelSpec};
use crate::overlay;
use crate::transcribe::{LocalTranscriber, Transcriber};

pub const EVENT_MODEL: &str = "model-state";
pub const EVENT_HOTKEY: &str = "hotkey-status";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ModelStatus {
    NotInstalled,
    #[serde(rename_all = "camelCase")]
    Downloading {
        downloaded: u64,
        total: u64,
    },
    Verifying,
    /// Loading into memory and warming up (first time can take ~15 s).
    Loading,
    Ready,
    Error {
        message: String,
    },
}

pub struct Services {
    pub dictation: Dictation,
    pub recorder: Recorder,
    pub transcriber: Arc<LocalTranscriber>,
    pub model_status: Mutex<ModelStatus>,
    pub hotkey_status: Mutex<HotkeyStatus>,
    pub cancel_download: AtomicBool,
}

impl Services {
    pub fn active_model(&self) -> &'static ModelSpec {
        &models::PARAKEET_V3
    }
}

pub fn start(app: &AppHandle) -> tauri::Result<()> {
    let (recorder, _audio_thread) = Recorder::spawn();
    let transcriber = Arc::new(LocalTranscriber::spawn());
    let dictation = Dictation::spawn(app.clone(), recorder.clone(), transcriber.clone());

    app.manage(Services {
        dictation,
        recorder,
        transcriber,
        model_status: Mutex::new(ModelStatus::NotInstalled),
        hotkey_status: Mutex::new(HotkeyStatus::WaitingForAccessibility),
        cancel_download: AtomicBool::new(false),
    });

    overlay::create(app)?;
    start_hotkeys(app);
    load_installed_model(app);
    Ok(())
}

/// Releases native resources in reverse start order (ADR 0003).
pub fn shutdown(app: &AppHandle) {
    let Some(services) = app.try_state::<Services>() else {
        return;
    };
    services.dictation.shutdown();
    services.recorder.shutdown();
    // Joins the inference thread, which drops the model and its Metal buffers.
    services.transcriber.shutdown();
    // The hotkey event tap is removed by the OS when the process exits.
}

pub fn set_model_status(app: &AppHandle, status: ModelStatus) {
    if let Some(services) = app.try_state::<Services>() {
        if let Ok(mut current) = services.model_status.lock() {
            *current = status.clone();
        }
    }
    let _ = app.emit(EVENT_MODEL, status);
}

fn start_hotkeys(app: &AppHandle) {
    let action_app = app.clone();
    let status_app = app.clone();
    hotkey::spawn(
        move |action| {
            let Some(services) = action_app.try_state::<Services>() else {
                return;
            };
            services.dictation.send(match action {
                HotkeyAction::DictateDown => Input::HotkeyDown,
                HotkeyAction::DictateUp => Input::HotkeyUp,
                HotkeyAction::Cancel => Input::Cancel,
            });
        },
        move |status| {
            if let Some(services) = status_app.try_state::<Services>() {
                if let Ok(mut current) = services.hotkey_status.lock() {
                    *current = status;
                }
            }
            let _ = status_app.emit(EVENT_HOTKEY, hotkey_status_name(status));
        },
    );
}

pub fn hotkey_status_name(status: HotkeyStatus) -> &'static str {
    match status {
        HotkeyStatus::WaitingForAccessibility => "waitingForAccessibility",
        HotkeyStatus::Active => "active",
        HotkeyStatus::Failed => "failed",
    }
}

/// Loads the model in the background at startup if it is already downloaded.
fn load_installed_model(app: &AppHandle) {
    let spec = &models::PARAKEET_V3;
    if !models::is_installed(app, spec) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || load_model(&app, spec));
}

/// Loads `spec` into the transcriber and reports progress. Blocking.
pub fn load_model(app: &AppHandle, spec: &ModelSpec) {
    let Some(services) = app.try_state::<Services>() else {
        return;
    };
    set_model_status(app, ModelStatus::Loading);
    let result = models::model_path(app, spec).and_then(|path| services.transcriber.load(path));
    match result {
        Ok(()) => set_model_status(app, ModelStatus::Ready),
        Err(message) => set_model_status(app, ModelStatus::Error { message }),
    }
}
