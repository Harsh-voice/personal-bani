//! Mic capture while the push-to-talk key is held.
//!
//! Platform-agnostic (cpal). Whisper needs 16kHz mono f32; Mac hardware
//! usually delivers 48kHz multi-channel, so capture at native rate and
//! resample on stop.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// A live capture. Holds the cpal stream (not Send — keep it on the thread
/// that created it, which the pipeline consumer thread does naturally).
pub struct Recorder {
    _stream: cpal::Stream,
    buf: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

impl Recorder {
    /// Open the default input device and start capturing immediately.
    pub fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no default input device — is a mic connected?")?;
        let config = device
            .default_input_config()
            .map_err(|e| format!("no default input config: {e}"))?;

        if config.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!(
                "unsupported input sample format {:?} (v0.1 handles f32 only)",
                config.sample_format()
            ));
        }

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        eprintln!(
            "[audio] recording from {:?} @ {sample_rate}Hz, {channels}ch",
            device.name().unwrap_or_else(|_| "<unnamed>".into())
        );

        let buf = Arc::new(Mutex::new(Vec::<f32>::new()));
        let buf_writer = Arc::clone(&buf);
        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    buf_writer.lock().unwrap().extend_from_slice(data);
                },
                |e| eprintln!("[audio] stream error: {e}"),
                None,
            )
            .map_err(|e| format!("failed to build input stream: {e}"))?;
        stream
            .play()
            .map_err(|e| format!("failed to start stream: {e}"))?;

        Ok(Self {
            _stream: stream,
            buf,
            sample_rate,
            channels,
        })
    }

    /// Stop capturing and return 16kHz mono samples ready for whisper.
    pub fn stop(self) -> Vec<f32> {
        let raw = std::mem::take(&mut *self.buf.lock().unwrap());
        drop(self._stream);

        let mono = downmix_mono(&raw, self.channels);
        let out = resample(&mono, self.sample_rate, WHISPER_SAMPLE_RATE);
        eprintln!(
            "[audio] captured {:.2}s ({} samples @ 16kHz)",
            out.len() as f32 / WHISPER_SAMPLE_RATE as f32,
            out.len()
        );
        out
    }
}

fn downmix_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    let ch = channels as usize;
    interleaved
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    // Never panic here: this runs on the pipeline thread, and a panic kills
    // dictation silently until the app restarts. Fall back to raw samples —
    // a wrong-rate transcript beats a dead pipeline.
    let mut resampler = match SincFixedIn::<f32>::new(to as f64 / from as f64, 2.0, params, input.len(), 1)
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[audio] resampler init failed ({e}) — passing raw samples");
            return input.to_vec();
        }
    };
    match resampler.process(&[input.to_vec()], None) {
        Ok(mut out) => out.remove(0),
        Err(e) => {
            eprintln!("[audio] resample failed ({e}) — passing raw samples");
            input.to_vec()
        }
    }
}

/// Write 16kHz mono f32 samples as a 16-bit PCM wav (playable anywhere).
pub fn write_wav(path: &std::path::Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: WHISPER_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        writer
            .write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}
