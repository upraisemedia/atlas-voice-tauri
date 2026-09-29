//! Microphone capture.
//!
//! A dedicated audio thread owns the cpal stream (cpal streams are not `Send`
//! on macOS, so they cannot move between threads). The realtime callback only
//! downmixes to mono and pushes into a lock-free ring buffer; the audio thread
//! drains it every few milliseconds, and on stop resamples the clip to 16 kHz.

mod resample;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};

use crate::dictation::TakeId;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;
/// Hard cap on one recording (10 minutes); samples beyond it are dropped.
const MAX_RECORDING_SECS: usize = 600;
const DRAIN_INTERVAL: Duration = Duration::from_millis(10);
const RING_SECONDS: usize = 2;

/// Events the recorder reports back to whoever started the capture.
pub enum CaptureEvent {
    Started { take: TakeId },
    Failed { take: TakeId, error: String },
}

/// A finished recording: 16 kHz mono f32 samples in [-1, 1].
pub struct Clip {
    pub samples: Vec<f32>,
}

impl Clip {
    pub fn duration_ms(&self) -> u64 {
        (self.samples.len() as u64 * 1000) / TARGET_SAMPLE_RATE as u64
    }
}

enum Command {
    Start {
        take: TakeId,
        events: Box<dyn Fn(CaptureEvent) + Send>,
    },
    Stop {
        take: TakeId,
        reply: Sender<Result<Clip, String>>,
    },
    StreamError(String),
    Shutdown,
}

/// Handle to the audio thread. Cheap to clone.
#[derive(Clone)]
pub struct Recorder {
    tx: Sender<Command>,
}

impl Recorder {
    pub fn spawn() -> (Self, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel();
        let error_tx = tx.clone();
        let handle = std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || AudioThread::new(error_tx).run(rx))
            .expect("failed to spawn audio thread");
        (Self { tx }, handle)
    }

    /// Opens the default microphone. Reports `Started` once real samples arrive.
    pub fn start(&self, take: TakeId, events: impl Fn(CaptureEvent) + Send + 'static) {
        let _ = self.tx.send(Command::Start {
            take,
            events: Box::new(events),
        });
    }

    /// Stops capturing and returns the resampled clip. Blocks briefly.
    pub fn stop(&self, take: TakeId) -> Result<Clip, String> {
        let (reply, rx) = mpsc::channel();
        self.tx
            .send(Command::Stop { take, reply })
            .map_err(|_| "audio thread is not running".to_string())?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "audio thread did not respond".to_string())?
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

struct Active {
    take: TakeId,
    stream: Stream,
    consumer: rtrb::Consumer<f32>,
    received_audio: Arc<AtomicBool>,
    started_reported: bool,
    events: Box<dyn Fn(CaptureEvent) + Send>,
    sample_rate: u32,
    samples: Vec<f32>,
    max_samples: usize,
}

struct AudioThread {
    active: Option<Active>,
    error_tx: Sender<Command>,
}

impl AudioThread {
    fn new(error_tx: Sender<Command>) -> Self {
        Self {
            active: None,
            error_tx,
        }
    }

    fn run(mut self, rx: Receiver<Command>) {
        loop {
            match rx.recv_timeout(DRAIN_INTERVAL) {
                Ok(Command::Start { take, events }) => self.start(take, events),
                Ok(Command::Stop { take, reply }) => {
                    let _ = reply.send(self.stop(take));
                }
                Ok(Command::StreamError(error)) => {
                    if let Some(active) = self.active.take() {
                        log::warn!("audio stream error: {error}");
                        (active.events)(CaptureEvent::Failed {
                            take: active.take,
                            error: "De microfoon viel weg tijdens de opname.".into(),
                        });
                    }
                }
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    // Dropping the stream closes the microphone.
                    self.active = None;
                    log::debug!("audio thread stopped");
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.drain();
        }
    }

    fn start(&mut self, take: TakeId, events: Box<dyn Fn(CaptureEvent) + Send>) {
        // A leftover capture (should not happen) is discarded first.
        self.active = None;
        match open_stream(self.error_tx.clone()) {
            Ok((stream, consumer, received_audio, sample_rate)) => {
                log::info!("capture started (take {take}, {sample_rate} Hz)");
                self.active = Some(Active {
                    take,
                    stream,
                    consumer,
                    received_audio,
                    started_reported: false,
                    events,
                    sample_rate,
                    samples: Vec::with_capacity(sample_rate as usize * 30),
                    max_samples: sample_rate as usize * MAX_RECORDING_SECS,
                });
            }
            Err(error) => {
                log::warn!("could not open microphone: {error}");
                events(CaptureEvent::Failed {
                    take,
                    error: "De microfoon kon niet worden geopend.".into(),
                });
            }
        }
    }

    fn drain(&mut self) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let available = active.consumer.slots();
        if available > 0 {
            if let Ok(chunk) = active.consumer.read_chunk(available) {
                // The ring buffer may wrap around, so it hands out two slices.
                let (first, second) = chunk.as_slices();
                for part in [first, second] {
                    let room = active.max_samples - active.samples.len();
                    active
                        .samples
                        .extend_from_slice(&part[..part.len().min(room)]);
                }
                chunk.commit_all();
            }
        }
        if !active.started_reported && active.received_audio.load(Ordering::Acquire) {
            active.started_reported = true;
            (active.events)(CaptureEvent::Started { take: active.take });
        }
    }

    fn stop(&mut self, take: TakeId) -> Result<Clip, String> {
        self.drain();
        let Some(active) = self.active.take() else {
            return Err("er liep geen opname".into());
        };
        if active.take != take {
            return Err(format!("stop for take {take}, active is {}", active.take));
        }
        let _ = active.stream.pause();
        drop(active.stream);
        let seconds = active.samples.len() as f32 / active.sample_rate as f32;
        log::info!("capture stopped (take {take}, {seconds:.1} s)");
        let samples = resample::to_16k(&active.samples, active.sample_rate)?;
        Ok(Clip { samples })
    }
}

type OpenedStream = (Stream, rtrb::Consumer<f32>, Arc<AtomicBool>, u32);

fn open_stream(error_tx: Sender<Command>) -> Result<OpenedStream, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no input device".to_string())?;
    let supported = device
        .default_input_config()
        .map_err(|e| format!("no input config: {e}"))?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let format = supported.sample_format();

    let (producer, consumer) = rtrb::RingBuffer::new(sample_rate as usize * RING_SECONDS);
    let received_audio = Arc::new(AtomicBool::new(false));

    let config: cpal::StreamConfig = supported.into();
    let flag = received_audio.clone();
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, channels, producer, flag, error_tx),
        SampleFormat::I16 => build::<i16>(&device, config, channels, producer, flag, error_tx),
        SampleFormat::I32 => build::<i32>(&device, config, channels, producer, flag, error_tx),
        other => return Err(format!("unsupported sample format {other}")),
    }?;
    stream
        .play()
        .map_err(|e| format!("could not start stream: {e}"))?;
    Ok((stream, consumer, received_audio, sample_rate))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
    received_audio: Arc<AtomicBool>,
    error_tx: Sender<Command>,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    // Realtime callback: no locks, no allocation, no logging.
    let data_callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
        if data.is_empty() {
            return;
        }
        for frame in data.chunks_exact(channels) {
            let sum: f32 = frame.iter().map(|&s| f32::from_sample(s)).sum();
            // A full ring buffer means the drain thread stalled: drop samples
            // rather than block the audio callback.
            let _ = producer.push(sum / channels as f32);
        }
        received_audio.store(true, Ordering::Release);
    };
    let error_callback = move |err: cpal::Error| {
        let _ = error_tx.send(Command::StreamError(err.to_string()));
    };
    device
        .build_input_stream(config, data_callback, error_callback, None)
        .map_err(|e| format!("could not build input stream: {e}"))
}
