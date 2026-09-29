//! The dictation controller: one thread that owns the state machine and
//! executes its effects (start/stop audio, transcribe, paste).
//!
//! Everything that happens (hotkeys, audio callbacks, finished transcripts,
//! UI commands) arrives as a message in one inbox, so state is only touched
//! by this thread and needs no locks. This is the "actor" pattern.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::state::{Effect, Input, Machine, Phase, TakeId};
use crate::audio::{CaptureEvent, Clip, Recorder};
use crate::inject::{self, PasteOutcome};
use crate::overlay;
use crate::transcribe::{TranscribeError, Transcriber};

pub const EVENT_STATE: &str = "dictation-state";
pub const EVENT_NOTICE: &str = "dictation-notice";
pub const EVENT_TRANSCRIPT: &str = "dictation-transcript";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub kind: NoticeKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoticeKind {
    Error,
    Info,
}

enum Msg {
    Input(Input),
    Shutdown(Sender<()>),
}

/// Handle to the controller thread; managed as Tauri state.
pub struct Dictation {
    tx: Sender<Msg>,
    phase: Arc<Mutex<Phase>>,
}

impl Dictation {
    pub fn spawn(app: AppHandle, recorder: Recorder, transcriber: Arc<dyn Transcriber>) -> Self {
        let (tx, rx) = mpsc::channel();
        let phase = Arc::new(Mutex::new(Phase::Idle));
        let controller = Controller {
            machine: Machine::default(),
            app,
            recorder,
            transcriber,
            tx: tx.clone(),
            clip: None,
            phase: phase.clone(),
        };
        std::thread::Builder::new()
            .name("dictation".into())
            .spawn(move || controller.run(rx))
            .expect("failed to spawn dictation thread");
        Self { tx, phase }
    }

    pub fn send(&self, input: Input) {
        let _ = self.tx.send(Msg::Input(input));
    }

    pub fn phase(&self) -> Phase {
        self.phase.lock().map(|p| *p).unwrap_or(Phase::Idle)
    }

    /// Cancels any running take and stops the controller thread.
    pub fn shutdown(&self) {
        let (ack, rx) = mpsc::channel();
        if self.tx.send(Msg::Shutdown(ack)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(2));
        }
    }
}

struct Controller {
    machine: Machine,
    app: AppHandle,
    recorder: Recorder,
    transcriber: Arc<dyn Transcriber>,
    /// Sender into our own inbox, cloned into callbacks from other threads.
    tx: Sender<Msg>,
    /// The finished recording waiting to be transcribed.
    clip: Option<(TakeId, Clip)>,
    phase: Arc<Mutex<Phase>>,
}

impl Controller {
    fn run(mut self, rx: Receiver<Msg>) {
        while let Ok(msg) = rx.recv() {
            match msg {
                Msg::Input(input) => self.process(input),
                Msg::Shutdown(ack) => {
                    self.process(Input::Cancel);
                    let _ = ack.send(());
                    break;
                }
            }
        }
        log::debug!("dictation thread stopped");
    }

    fn process(&mut self, input: Input) {
        if input == Input::HotkeyDown
            && self.machine.phase() == Phase::Idle
            && !self.transcriber.is_ready()
        {
            self.notify(
                NoticeKind::Error,
                "Nog geen model geladen. Open Atlas Voice om het model te downloaden.",
            );
            return;
        }

        // Effects can produce follow-up inputs (e.g. stopping capture yields a
        // clip); handle them in order before looking at the inbox again.
        let mut queue = VecDeque::from([input]);
        while let Some(input) = queue.pop_front() {
            let before = self.machine.phase();
            let effects = self.machine.handle(input);
            if self.machine.phase() != before {
                self.publish_phase();
            }
            for effect in effects {
                if let Some(next) = self.execute(effect) {
                    queue.push_back(next);
                }
            }
        }
    }

    fn execute(&mut self, effect: Effect) -> Option<Input> {
        match effect {
            Effect::StartCapture { take } => {
                let tx = self.tx.clone();
                self.recorder.start(take, move |event| {
                    let input = match event {
                        CaptureEvent::Started { take } => Input::CaptureStarted { take },
                        CaptureEvent::Failed { take, error } => {
                            Input::CaptureFailed { take, error }
                        }
                    };
                    let _ = tx.send(Msg::Input(input));
                });
                None
            }
            Effect::StopCapture { take } => match self.recorder.stop(take) {
                Ok(clip) => {
                    let duration_ms = clip.duration_ms();
                    self.clip = Some((take, clip));
                    Some(Input::ClipReady { take, duration_ms })
                }
                Err(error) => Some(Input::CaptureFailed { take, error }),
            },
            Effect::DiscardCapture { take } => {
                let _ = self.recorder.stop(take);
                self.clip = None;
                None
            }
            Effect::Transcribe { take } => {
                let Some((_, clip)) = self.clip.take().filter(|(t, _)| *t == take) else {
                    return Some(Input::TranscriptFailed {
                        take,
                        error: "opname ontbreekt".into(),
                    });
                };
                let tx = self.tx.clone();
                self.transcriber.transcribe(
                    take,
                    clip.samples,
                    Box::new(move |result| {
                        let input = match result {
                            Ok(text) => Input::TranscriptReady { take, text },
                            Err(TranscribeError::Cancelled) => return,
                            Err(error) => Input::TranscriptFailed {
                                take,
                                error: error.to_string(),
                            },
                        };
                        let _ = tx.send(Msg::Input(input));
                    }),
                );
                None
            }
            Effect::CancelTranscription { take } => {
                self.transcriber.cancel(take);
                self.clip = None;
                None
            }
            Effect::Inject { take, text } => {
                let _ = self.app.emit(EVENT_TRANSCRIPT, &text);
                let app = self.app.clone();
                let tx = self.tx.clone();
                // Pasting waits for the target app; keep that off this thread.
                std::thread::spawn(move || {
                    let input = match inject::paste(&app, text) {
                        Ok(PasteOutcome::Pasted) => Input::InjectDone { take },
                        Ok(PasteOutcome::LeftOnClipboard) => {
                            emit_notice(
                                &app,
                                NoticeKind::Info,
                                "Tekst staat op het klembord. Plak met ⌘V (geef Atlas Voice toegankelijkheidsrechten om automatisch te plakken).",
                            );
                            Input::InjectDone { take }
                        }
                        Err(error) => Input::InjectFailed { take, error },
                    };
                    let _ = tx.send(Msg::Input(input));
                });
                None
            }
            Effect::ReportError { message } => {
                self.notify(NoticeKind::Error, &message);
                None
            }
        }
    }

    fn publish_phase(&self) {
        let phase = self.machine.phase();
        if let Ok(mut shared) = self.phase.lock() {
            *shared = phase;
        }
        let _ = self.app.emit(EVENT_STATE, phase);
        overlay::on_phase(&self.app, phase);
    }

    fn notify(&self, kind: NoticeKind, message: &str) {
        emit_notice(&self.app, kind, message);
    }
}

fn emit_notice(app: &AppHandle, kind: NoticeKind, message: &str) {
    log::info!("notice ({kind:?})");
    let _ = app.emit(
        EVENT_NOTICE,
        Notice {
            kind,
            message: message.to_string(),
        },
    );
    overlay::flash(app);
}
