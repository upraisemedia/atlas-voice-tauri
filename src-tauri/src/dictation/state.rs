//! The dictation state machine.
//!
//! Pure logic, no I/O: every input produces a new phase plus a list of
//! effects that the controller executes. Each recording gets a `TakeId`;
//! inputs carrying a stale take id are ignored, so a late transcript from a
//! cancelled recording can never be pasted.

use serde::Serialize;

pub type TakeId = u64;

/// Recordings shorter than this are dropped silently (accidental taps).
pub const MIN_CLIP_MS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum Phase {
    Idle,
    /// Microphone is starting; no audio has arrived yet.
    Arming {
        take: TakeId,
    },
    Recording {
        take: TakeId,
    },
    Transcribing {
        take: TakeId,
    },
    Injecting {
        take: TakeId,
    },
}

impl Phase {
    pub fn take(self) -> Option<TakeId> {
        match self {
            Phase::Idle => None,
            Phase::Arming { take }
            | Phase::Recording { take }
            | Phase::Transcribing { take }
            | Phase::Injecting { take } => Some(take),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    HotkeyDown,
    HotkeyUp,
    Cancel,
    /// The first real audio samples arrived.
    CaptureStarted {
        take: TakeId,
    },
    CaptureFailed {
        take: TakeId,
        error: String,
    },
    /// Capture stopped; the clip is held by the controller.
    ClipReady {
        take: TakeId,
        duration_ms: u64,
    },
    TranscriptReady {
        take: TakeId,
        text: String,
    },
    TranscriptFailed {
        take: TakeId,
        error: String,
    },
    InjectDone {
        take: TakeId,
    },
    InjectFailed {
        take: TakeId,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    StartCapture { take: TakeId },
    StopCapture { take: TakeId },
    DiscardCapture { take: TakeId },
    Transcribe { take: TakeId },
    CancelTranscription { take: TakeId },
    Inject { take: TakeId, text: String },
    ReportError { message: String },
}

#[derive(Debug)]
pub struct Machine {
    phase: Phase,
    next_take: TakeId,
    /// A key-down was observed for the current press. Releases without a
    /// matching press (e.g. the key was already held at startup) are ignored.
    key_down: bool,
    /// The key was released while the microphone was still arming.
    release_pending: bool,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            next_take: 1,
            key_down: false,
            release_pending: false,
        }
    }
}

impl Machine {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn handle(&mut self, input: Input) -> Vec<Effect> {
        let current = self.phase.take();
        let is_for_current = |take: TakeId| current == Some(take);

        match (self.phase, input) {
            // --- hotkey -------------------------------------------------
            (Phase::Idle, Input::HotkeyDown) => {
                let take = self.next_take;
                self.next_take += 1;
                self.key_down = true;
                self.release_pending = false;
                self.phase = Phase::Arming { take };
                vec![Effect::StartCapture { take }]
            }
            (_, Input::HotkeyDown) => {
                // Busy (or still held): ignore extra presses.
                vec![]
            }
            (_, Input::HotkeyUp) if !self.key_down => vec![],
            (Phase::Arming { .. }, Input::HotkeyUp) => {
                self.key_down = false;
                self.release_pending = true;
                vec![]
            }
            (Phase::Recording { take }, Input::HotkeyUp) => {
                self.key_down = false;
                self.phase = Phase::Transcribing { take };
                vec![Effect::StopCapture { take }]
            }
            (_, Input::HotkeyUp) => {
                self.key_down = false;
                vec![]
            }

            // --- cancel -------------------------------------------------
            (Phase::Arming { take } | Phase::Recording { take }, Input::Cancel) => {
                self.reset();
                vec![Effect::DiscardCapture { take }]
            }
            (Phase::Transcribing { take }, Input::Cancel) => {
                self.reset();
                vec![Effect::CancelTranscription { take }]
            }
            // Injecting takes milliseconds and cannot be undone halfway.
            (_, Input::Cancel) => vec![],

            // --- capture ------------------------------------------------
            (Phase::Arming { take }, Input::CaptureStarted { take: t }) if t == take => {
                if self.release_pending {
                    self.release_pending = false;
                    self.phase = Phase::Transcribing { take };
                    vec![Effect::StopCapture { take }]
                } else {
                    self.phase = Phase::Recording { take };
                    vec![]
                }
            }
            // Transcribing is included: stopping the capture itself can fail.
            (
                Phase::Arming { take } | Phase::Recording { take } | Phase::Transcribing { take },
                Input::CaptureFailed { take: t, error },
            ) if t == take => {
                self.reset();
                vec![
                    Effect::DiscardCapture { take },
                    Effect::ReportError { message: error },
                ]
            }
            (
                Phase::Transcribing { take },
                Input::ClipReady {
                    take: t,
                    duration_ms,
                },
            ) if t == take => {
                if duration_ms < MIN_CLIP_MS {
                    self.reset();
                    vec![]
                } else {
                    vec![Effect::Transcribe { take }]
                }
            }

            // --- transcription ------------------------------------------
            (Phase::Transcribing { take }, Input::TranscriptReady { take: t, text })
                if t == take =>
            {
                let text = text.trim().to_string();
                if text.is_empty() {
                    self.reset();
                    vec![]
                } else {
                    self.phase = Phase::Injecting { take };
                    vec![Effect::Inject { take, text }]
                }
            }
            (Phase::Transcribing { take }, Input::TranscriptFailed { take: t, error })
                if t == take =>
            {
                self.reset();
                vec![Effect::ReportError { message: error }]
            }

            // --- injection ----------------------------------------------
            (Phase::Injecting { take }, Input::InjectDone { take: t }) if t == take => {
                self.reset();
                vec![]
            }
            (Phase::Injecting { take }, Input::InjectFailed { take: t, error }) if t == take => {
                self.reset();
                vec![Effect::ReportError { message: error }]
            }

            // Anything else is stale (old take) or out of order: ignore.
            (_, other) => {
                if let Some(take) = input_take(&other) {
                    if !is_for_current(take) {
                        log::debug!("ignoring stale input for take {take}");
                    }
                }
                vec![]
            }
        }
    }

    fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.release_pending = false;
    }
}

fn input_take(input: &Input) -> Option<TakeId> {
    match input {
        Input::CaptureStarted { take }
        | Input::CaptureFailed { take, .. }
        | Input::ClipReady { take, .. }
        | Input::TranscriptReady { take, .. }
        | Input::TranscriptFailed { take, .. }
        | Input::InjectDone { take }
        | Input::InjectFailed { take, .. } => Some(*take),
        Input::HotkeyDown | Input::HotkeyUp | Input::Cancel => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(m: &mut Machine) -> TakeId {
        let fx = m.handle(Input::HotkeyDown);
        let take = match fx.as_slice() {
            [Effect::StartCapture { take }] => *take,
            other => panic!("unexpected effects {other:?}"),
        };
        assert!(m.handle(Input::CaptureStarted { take }).is_empty());
        assert_eq!(m.phase(), Phase::Recording { take });
        take
    }

    #[test]
    fn happy_path_press_speak_release_paste() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        assert_eq!(
            m.handle(Input::HotkeyUp),
            vec![Effect::StopCapture { take }]
        );
        assert_eq!(
            m.handle(Input::ClipReady {
                take,
                duration_ms: 2_000
            }),
            vec![Effect::Transcribe { take }]
        );
        assert_eq!(
            m.handle(Input::TranscriptReady {
                take,
                text: "  Hallo wereld. ".into()
            }),
            vec![Effect::Inject {
                take,
                text: "Hallo wereld.".into()
            }]
        );
        assert!(m.handle(Input::InjectDone { take }).is_empty());
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn release_while_arming_finishes_once_audio_starts() {
        let mut m = Machine::default();
        let take = match m.handle(Input::HotkeyDown).as_slice() {
            [Effect::StartCapture { take }] => *take,
            _ => unreachable!(),
        };
        assert!(m.handle(Input::HotkeyUp).is_empty());
        assert_eq!(m.phase(), Phase::Arming { take });
        assert_eq!(
            m.handle(Input::CaptureStarted { take }),
            vec![Effect::StopCapture { take }]
        );
    }

    #[test]
    fn short_clip_is_dropped_silently() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        m.handle(Input::HotkeyUp);
        assert!(m
            .handle(Input::ClipReady {
                take,
                duration_ms: MIN_CLIP_MS - 1
            })
            .is_empty());
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn empty_transcript_does_not_paste() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        m.handle(Input::HotkeyUp);
        m.handle(Input::ClipReady {
            take,
            duration_ms: 1_000,
        });
        assert!(m
            .handle(Input::TranscriptReady {
                take,
                text: "   ".into()
            })
            .is_empty());
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn cancel_during_recording_discards_audio() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        assert_eq!(
            m.handle(Input::Cancel),
            vec![Effect::DiscardCapture { take }]
        );
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn cancel_during_transcription_drops_the_late_result() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        m.handle(Input::HotkeyUp);
        m.handle(Input::ClipReady {
            take,
            duration_ms: 1_000,
        });
        assert_eq!(
            m.handle(Input::Cancel),
            vec![Effect::CancelTranscription { take }]
        );
        // The transcript arrives anyway: it must not be pasted.
        assert!(m
            .handle(Input::TranscriptReady {
                take,
                text: "te laat".into()
            })
            .is_empty());
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn stale_transcript_from_previous_take_is_ignored() {
        let mut m = Machine::default();
        let first = recording(&mut m);
        m.handle(Input::Cancel);
        let second = recording(&mut m);
        assert_ne!(first, second);
        assert!(m
            .handle(Input::TranscriptReady {
                take: first,
                text: "oud".into()
            })
            .is_empty());
        assert_eq!(m.phase(), Phase::Recording { take: second });
    }

    #[test]
    fn presses_while_busy_are_ignored() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        m.handle(Input::HotkeyUp);
        assert!(m.handle(Input::HotkeyDown).is_empty());
        assert_eq!(m.phase(), Phase::Transcribing { take });
        // ...and the matching release does not disturb the running take.
        assert!(m.handle(Input::HotkeyUp).is_empty());
        assert_eq!(m.phase(), Phase::Transcribing { take });
    }

    #[test]
    fn release_without_press_is_ignored() {
        let mut m = Machine::default();
        assert!(m.handle(Input::HotkeyUp).is_empty());
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn capture_failure_reports_and_returns_to_idle() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        let fx = m.handle(Input::CaptureFailed {
            take,
            error: "mic weg".into(),
        });
        assert_eq!(
            fx,
            vec![
                Effect::DiscardCapture { take },
                Effect::ReportError {
                    message: "mic weg".into()
                }
            ]
        );
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn transcription_failure_reports_error() {
        let mut m = Machine::default();
        let take = recording(&mut m);
        m.handle(Input::HotkeyUp);
        m.handle(Input::ClipReady {
            take,
            duration_ms: 1_000,
        });
        assert_eq!(
            m.handle(Input::TranscriptFailed {
                take,
                error: "model".into()
            }),
            vec![Effect::ReportError {
                message: "model".into()
            }]
        );
        assert_eq!(m.phase(), Phase::Idle);
    }
}
