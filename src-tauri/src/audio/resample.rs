//! Offline resampling of a finished clip to 16 kHz mono.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use super::TARGET_SAMPLE_RATE;

const CHUNK_FRAMES: usize = 1024;

pub fn to_16k(samples: &[f32], sample_rate: u32) -> Result<Vec<f32>, String> {
    if sample_rate == TARGET_SAMPLE_RATE || samples.is_empty() {
        return Ok(samples.to_vec());
    }
    let mut resampler = Fft::<f32>::new(
        sample_rate as usize,
        TARGET_SAMPLE_RATE as usize,
        CHUNK_FRAMES,
        1,
        FixedSync::Input,
    )
    .map_err(|e| format!("resampler setup failed: {e}"))?;
    let input = InterleavedSlice::new(samples, 1, samples.len())
        .map_err(|e| format!("resampler input: {e}"))?;
    let output = resampler
        .process_all(&input, samples.len(), None)
        .map_err(|e| format!("resampling failed: {e}"))?;
    Ok(output.take_data())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_duration_when_downsampling_48k() {
        let one_second: Vec<f32> = (0..48_000)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5)
            .collect();
        let out = to_16k(&one_second, 48_000).unwrap();
        // process_all trims the resampler delay, so the length is exact within a frame.
        assert!((out.len() as i64 - 16_000).abs() <= 1, "len {}", out.len());
        let peak = out.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
        assert!((0.4..0.6).contains(&peak), "peak {peak}");
    }

    #[test]
    fn passes_16k_through() {
        let clip = vec![0.1_f32; 320];
        assert_eq!(to_16k(&clip, 16_000).unwrap(), clip);
    }
}
