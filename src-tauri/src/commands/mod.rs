//! Thin IPC layer. Commands translate frontend calls into core operations and
//! contain no business logic themselves.

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::dictation::{Input, Phase};
use crate::error::{AppError, AppResult};
use crate::models::{self, download::DownloadEvent, ModelSpec};
use crate::services::{self, ModelStatus, Services};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppResult<AppInfo> {
    let info = app.package_info();
    Ok(AppInfo {
        name: info.name.clone(),
        version: info.version.to_string(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: Phase,
    pub accessibility: bool,
    pub hotkey: &'static str,
    pub model: ModelSpec,
    pub model_status: ModelStatus,
}

#[tauri::command]
pub fn get_status(services: State<'_, Services>) -> AppResult<Status> {
    let hotkey = services
        .hotkey_status
        .lock()
        .map(|s| services::hotkey_status_name(*s))
        .unwrap_or("failed");
    let model_status = services
        .model_status
        .lock()
        .map(|s| s.clone())
        .map_err(|_| AppError::Internal("model status unavailable".into()))?;
    Ok(Status {
        phase: services.dictation.phase(),
        accessibility: handy_keys::check_accessibility(),
        hotkey,
        model: *services.active_model(),
        model_status,
    })
}

#[tauri::command]
pub fn open_accessibility_settings() -> AppResult<()> {
    handy_keys::open_accessibility_settings().map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
pub fn cancel_dictation(services: State<'_, Services>) {
    services.dictation.send(Input::Cancel);
}

/// Downloads the active model, verifies it and loads it. Progress arrives as
/// `model-state` events.
#[tauri::command]
pub async fn download_model(app: AppHandle, services: State<'_, Services>) -> AppResult<()> {
    let spec = services.active_model();
    let busy = services
        .model_status
        .lock()
        .map(|s| {
            matches!(
                *s,
                ModelStatus::Downloading { .. } | ModelStatus::Verifying | ModelStatus::Loading
            )
        })
        .unwrap_or(true);
    if busy {
        return Err(AppError::Internal("er loopt al een download".into()));
    }

    services.cancel_download.store(false, Ordering::SeqCst);
    let dest = models::model_path(&app, spec).map_err(AppError::Internal)?;
    services::set_model_status(
        &app,
        ModelStatus::Downloading {
            downloaded: 0,
            total: spec.size_bytes,
        },
    );

    let progress_app = app.clone();
    let result = models::download::download(spec, &dest, &services.cancel_download, |event| {
        let status = match event {
            DownloadEvent::Progress { downloaded, total } => {
                ModelStatus::Downloading { downloaded, total }
            }
            DownloadEvent::Verifying => ModelStatus::Verifying,
        };
        services::set_model_status(&progress_app, status);
    })
    .await;

    if let Err(message) = result {
        let status = if services.cancel_download.load(Ordering::SeqCst) {
            ModelStatus::NotInstalled
        } else {
            ModelStatus::Error {
                message: message.clone(),
            }
        };
        services::set_model_status(&app, status);
        return Err(AppError::Internal(message));
    }

    // Loading blocks (disk + Metal warm-up); keep it off the async runtime.
    let load_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || services::load_model(&load_app, spec))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn cancel_download(services: State<'_, Services>) {
    services.cancel_download.store(true, Ordering::SeqCst);
}
