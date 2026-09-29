//! Model catalog and on-disk storage.
//!
//! Models are GGUF files from pinned Hugging Face revisions, verified by
//! SHA-256 after download (ADR 0004). They live in
//! `~/Library/Application Support/nl.atlasvoice.desktop/models/`.

pub mod download;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub file_name: &'static str,
    #[serde(skip)]
    pub url: &'static str,
    #[serde(skip)]
    pub sha256: &'static str,
    pub size_bytes: u64,
    pub license: &'static str,
}

pub const PARAKEET_V3: ModelSpec = ModelSpec {
    id: "parakeet-tdt-0.6b-v3",
    name: "Parakeet v3",
    description: "Snel en nauwkeurig. 25 Europese talen, herkent zelf de taal.",
    file_name: "parakeet-tdt-0.6b-v3-Q8_0.gguf",
    url: "https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/85ac09ea12fc4b1112fa76810059364bc6adc9de/parakeet-tdt-0.6b-v3-Q8_0.gguf",
    sha256: "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7",
    size_bytes: 739_508_576,
    license: "CC-BY-4.0 (NVIDIA)",
};

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("no app data dir: {e}"))?
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn model_path(app: &AppHandle, spec: &ModelSpec) -> Result<PathBuf, String> {
    Ok(models_dir(app)?.join(spec.file_name))
}

/// A model counts as installed when the file exists with the expected size.
/// The SHA-256 check happens once, right after download.
pub fn is_installed(app: &AppHandle, spec: &ModelSpec) -> bool {
    model_path(app, spec)
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .is_some_and(|m| m.len() == spec.size_bytes)
}
