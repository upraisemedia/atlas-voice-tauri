//! File + stdout logging.
//!
//! Privacy rule: never log transcript text, audio or device names. Log phases,
//! timings and error classes only.

use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const MAX_LOG_FILE_BYTES: u128 = 2 * 1024 * 1024;

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            // ~/Library/Logs/nl.atlasvoice.desktop/
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .level(level)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .max_file_size(MAX_LOG_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .build()
}
