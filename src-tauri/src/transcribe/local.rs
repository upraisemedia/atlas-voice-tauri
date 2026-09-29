//! On-device transcription with transcribe-cpp (ggml + Metal).
//!
//! One inference thread owns the loaded model. Everyone else talks to it
//! through a channel. Panics inside native inference are caught: the engine
//! is dropped and reloaded on the next request instead of taking the app down.

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use transcribe_cpp::{CancelToken, Model, RunOptions, Session};

use super::{TranscribeCallback, TranscribeError, Transcriber};
use crate::dictation::TakeId;

enum Command {
    Load {
        path: PathBuf,
        reply: Sender<Result<(), String>>,
    },
    Transcribe {
        take: TakeId,
        audio: Vec<f32>,
        done: TranscribeCallback,
    },
    Shutdown,
}

pub struct LocalTranscriber {
    tx: Sender<Command>,
    ready: Arc<AtomicBool>,
    /// Cancel token of the run in flight, shared with the worker.
    in_flight: Arc<Mutex<Option<(TakeId, CancelToken)>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl LocalTranscriber {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        let ready = Arc::new(AtomicBool::new(false));
        let in_flight = Arc::new(Mutex::new(None));
        let worker = Worker {
            engine: None,
            ready: ready.clone(),
            in_flight: in_flight.clone(),
        };
        let handle = std::thread::Builder::new()
            .name("inference".into())
            .spawn(move || worker.run(rx))
            .expect("failed to spawn inference thread");
        Self {
            tx,
            ready,
            in_flight,
            worker: Mutex::new(Some(handle)),
        }
    }

    /// Loads (or replaces) the model and runs a short warm-up. Blocks until done.
    pub fn load(&self, path: PathBuf) -> Result<(), String> {
        let (reply, rx) = mpsc::channel();
        self.tx
            .send(Command::Load { path, reply })
            .map_err(|_| "inference thread is not running".to_string())?;
        rx.recv()
            .map_err(|_| "inference thread stopped".to_string())?
    }
}

impl Transcriber for LocalTranscriber {
    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn transcribe(&self, take: TakeId, audio: Vec<f32>, done: TranscribeCallback) {
        if let Err(mpsc::SendError(Command::Transcribe { done, .. })) =
            self.tx.send(Command::Transcribe { take, audio, done })
        {
            done(Err(TranscribeError::Internal(
                "inference thread stopped".into(),
            )));
        }
    }

    fn cancel(&self, take: TakeId) {
        if let Ok(guard) = self.in_flight.lock() {
            if let Some((running, token)) = guard.as_ref() {
                if *running == take {
                    token.cancel();
                }
            }
        }
    }

    fn shutdown(&self) {
        if let Ok(guard) = self.in_flight.lock() {
            if let Some((_, token)) = guard.as_ref() {
                token.cancel();
            }
        }
        let _ = self.tx.send(Command::Shutdown);
        let handle = self.worker.lock().ok().and_then(|mut h| h.take());
        if let Some(handle) = handle {
            // Joining guarantees the model (and its Metal buffers) is dropped
            // before the process exits.
            if handle.join().is_err() {
                log::error!("inference thread panicked during shutdown");
            }
        }
    }
}

struct Engine {
    // Field order matters: the session must drop before the model it uses.
    session: Session,
    _model: Model,
}

struct Worker {
    engine: Option<Engine>,
    ready: Arc<AtomicBool>,
    in_flight: Arc<Mutex<Option<(TakeId, CancelToken)>>>,
}

impl Worker {
    fn run(mut self, rx: Receiver<Command>) {
        while let Ok(command) = rx.recv() {
            match command {
                Command::Load { path, reply } => {
                    let _ = reply.send(self.load(path));
                }
                Command::Transcribe { take, audio, done } => {
                    done(self.transcribe(take, &audio));
                }
                Command::Shutdown => break,
            }
        }
        self.unload();
        log::debug!("inference thread stopped");
    }

    fn load(&mut self, path: PathBuf) -> Result<(), String> {
        self.unload();
        let started = Instant::now();
        let result = panic::catch_unwind(AssertUnwindSafe(|| -> Result<Engine, String> {
            let model = Model::load(&path).map_err(|e| e.to_string())?;
            let mut session = model.session().map_err(|e| e.to_string())?;
            // Warm-up: the first run compiles Metal pipelines. Half a second of
            // silence keeps it cheap and makes the user's first dictation fast.
            let _ = session.run(&vec![0.0; 8_000], &RunOptions::default());
            log::info!("model loaded on {} backend", model.backend());
            Ok(Engine {
                session,
                _model: model,
            })
        }));
        match result {
            Ok(Ok(engine)) => {
                self.engine = Some(engine);
                self.ready.store(true, Ordering::Release);
                log::info!("model ready in {:.1} s", started.elapsed().as_secs_f32());
                Ok(())
            }
            Ok(Err(error)) => {
                log::error!("model load failed: {error}");
                Err(error)
            }
            Err(_) => {
                log::error!("model load panicked");
                Err("het model kon niet worden geladen".into())
            }
        }
    }

    fn transcribe(&mut self, take: TakeId, audio: &[f32]) -> Result<String, TranscribeError> {
        let Some(mut engine) = self.engine.take() else {
            return Err(TranscribeError::ModelMissing);
        };
        let token = CancelToken::new();
        engine.session.set_cancel_token(&token);
        if let Ok(mut guard) = self.in_flight.lock() {
            *guard = Some((take, token));
        }

        let started = Instant::now();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            engine.session.run(audio, &RunOptions::default())
        }));

        if let Ok(mut guard) = self.in_flight.lock() {
            *guard = None;
        }

        let audio_secs = audio.len() as f32 / 16_000.0;
        match outcome {
            Ok(Ok(transcript)) => {
                log::info!(
                    "transcribed {audio_secs:.1} s of audio in {} ms",
                    started.elapsed().as_millis()
                );
                self.engine = Some(engine);
                Ok(transcript.text)
            }
            Ok(Err(transcribe_cpp::Error::Aborted { .. })) => {
                self.engine = Some(engine);
                Err(TranscribeError::Cancelled)
            }
            Ok(Err(error)) => {
                self.engine = Some(engine);
                Err(TranscribeError::Internal(error.to_string()))
            }
            Err(_) => {
                // Engine state is unknown after a panic: drop it. The caller
                // sees an error; the model has to be reloaded.
                log::error!("inference panicked; model unloaded");
                self.ready.store(false, Ordering::Release);
                Err(TranscribeError::Internal("het model is gecrasht".into()))
            }
        }
    }

    fn unload(&mut self) {
        self.ready.store(false, Ordering::Release);
        if self.engine.take().is_some() {
            log::info!("model unloaded");
        }
    }
}
