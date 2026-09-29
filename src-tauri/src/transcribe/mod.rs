//! Speech-to-text behind a small trait, so the dictation controller does not
//! care which engine runs (ADR 0004).

mod local;

pub use local::LocalTranscriber;

use crate::dictation::TakeId;

pub type TranscribeCallback = Box<dyn FnOnce(Result<String, TranscribeError>) + Send>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum TranscribeError {
    #[error("geen model geladen")]
    ModelMissing,
    #[error("geannuleerd")]
    Cancelled,
    #[error("transcriptie mislukt: {0}")]
    Internal(String),
}

pub trait Transcriber: Send + Sync {
    /// True when a model is loaded and ready to transcribe.
    fn is_ready(&self) -> bool;
    /// Transcribes 16 kHz mono audio on a worker thread and calls `done` with
    /// the result. Never blocks the caller.
    fn transcribe(&self, take: TakeId, audio: Vec<f32>, done: TranscribeCallback);
    /// Aborts the in-flight transcription for `take`, if any.
    fn cancel(&self, take: TakeId);
    /// Stops the worker and releases the model (and its GPU resources).
    fn shutdown(&self);
}
