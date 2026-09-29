//! Streaming model download: `.part` file, SHA-256 while streaming, atomic
//! rename on success. Resume and stall detection follow in phase 3.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::ModelSpec;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        downloaded: u64,
        total: u64,
    },
    Verifying,
}

/// Downloads `spec` to `dest`. `cancel` is polled between chunks.
pub async fn download(
    spec: &ModelSpec,
    dest: &Path,
    cancel: &AtomicBool,
    mut on_event: impl FnMut(DownloadEvent),
) -> Result<(), String> {
    let part = dest.with_extension("part");
    let result = stream_to_file(spec, &part, cancel, &mut on_event).await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(error);
    }
    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| format!("kon het model niet opslaan: {e}"))?;
    log::info!("model {} installed", spec.id);
    Ok(())
}

async fn stream_to_file(
    spec: &ModelSpec,
    part: &Path,
    cancel: &AtomicBool,
    on_event: &mut impl FnMut(DownloadEvent),
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .get(spec.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("download mislukt: {e}"))?;
    let total = response.content_length().unwrap_or(spec.size_bytes);

    let mut file = tokio::fs::File::create(part)
        .await
        .map_err(|e| format!("kan niet schrijven naar {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut last_report = Instant::now() - PROGRESS_INTERVAL;
    let mut body = response.bytes_stream();

    while let Some(chunk) = body.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Err("download geannuleerd".into());
        }
        let chunk = chunk.map_err(|e| format!("verbinding verbroken: {e}"))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("schrijven mislukt: {e}"))?;
        downloaded += chunk.len() as u64;
        if last_report.elapsed() >= PROGRESS_INTERVAL {
            last_report = Instant::now();
            on_event(DownloadEvent::Progress { downloaded, total });
        }
    }
    file.flush()
        .await
        .map_err(|e| format!("schrijven mislukt: {e}"))?;
    file.sync_all()
        .await
        .map_err(|e| format!("schrijven mislukt: {e}"))?;
    on_event(DownloadEvent::Progress { downloaded, total });

    on_event(DownloadEvent::Verifying);
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex != spec.sha256 {
        log::error!("checksum mismatch for {}", spec.id);
        return Err("het gedownloade bestand is beschadigd; probeer het opnieuw".into());
    }
    Ok(())
}
