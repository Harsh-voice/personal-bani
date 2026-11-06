//! Local speech-to-text via whisper-rs (whisper.cpp, Metal on Mac).
//!
//! v0.1 is whisper-only; Parakeet joins at phase 3 for live partials
//! (see CLAUDE.md "Latency trick").

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Stt {
    ctx: WhisperContext,
}

impl Stt {
    /// Load a ggml model. ~1s for base.en; do it once, not per utterance.
    pub fn new(model_path: &str) -> Result<Self, String> {
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| format!("failed to load whisper model {model_path}: {e}"))?;
        eprintln!("[stt] loaded model {model_path}");
        Ok(Self { ctx })
    }

    /// Transcribe 16kHz mono f32 samples to text. `initial_prompt` biases
    /// decoding (used for the custom dictionary glossary).
    pub fn transcribe(
        &self,
        samples: &[f32],
        initial_prompt: Option<&str>,
    ) -> Result<String, String> {
        let started = std::time::Instant::now();
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("whisper state: {e}"))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_n_threads(
            std::thread::available_parallelism()
                .map(|n| n.get() as i32)
                .unwrap_or(4),
        );
        if let Some(prompt) = initial_prompt {
            params.set_initial_prompt(prompt);
        }
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, samples)
            .map_err(|e| format!("whisper inference: {e}"))?;

        let n = state.full_n_segments().map_err(|e| e.to_string())?;
        let mut text = String::new();
        for i in 0..n {
            text.push_str(&state.full_get_segment_text(i).map_err(|e| e.to_string())?);
        }
        // Whisper marks silence/noise with bracketed non-speech tokens
        // ("[BLANK_AUDIO]", "(coughing)", "*music*") — strip them so a silent
        // recording becomes an empty transcript instead of injected junk.
        let text = text
            .split_whitespace()
            .filter(|w| {
                !((w.starts_with('[') && w.ends_with(']'))
                    || (w.starts_with('(') && w.ends_with(')'))
                    || (w.starts_with('*') && w.ends_with('*')))
            })
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        eprintln!(
            "[stt] transcribed {:.2}s audio in {:.2}s: {text:?}",
            samples.len() as f32 / crate::audio::WHISPER_SAMPLE_RATE as f32,
            started.elapsed().as_secs_f32()
        );
        Ok(text)
    }
}
