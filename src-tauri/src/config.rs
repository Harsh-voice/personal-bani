//! App configuration: load/save JSON in the platform config dir.
//!
//! `hotkey_keycode` and `whisper_model_path` are read once at startup
//! (restart to apply); the cleanup settings are read per-utterance and
//! apply live.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

pub type SharedConfig = Arc<Mutex<Config>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Platform virtual keycode for push-to-talk. 61 = Right Option (macOS).
    pub hotkey_keycode: i64,
    /// ggml model path, relative to the app's working directory or absolute.
    pub whisper_model_path: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub cleanup_enabled: bool,
    /// Transcripts at or below this length skip the LLM cleanup pass.
    pub cleanup_min_chars: usize,
    /// Custom terms (names, jargon) STT must spell exactly. Applies live.
    pub dictionary: Vec<String>,
    /// Explicit (wrong → right) fixes for sound-alike misses whisper keeps
    /// making despite the glossary bias. Whole-word, case-insensitive.
    pub replacements: Vec<(String, String)>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey_keycode: 61,
            whisper_model_path: "../models/ggml-base.en.bin".into(),
            ollama_url: "http://localhost:11434".into(),
            ollama_model: "qwen2.5:7b-instruct".into(),
            cleanup_enabled: true,
            cleanup_min_chars: 50,
            dictionary: Vec::new(),
            replacements: Vec::new(),
        }
    }
}

pub fn load(path: &Path) -> Config {
    match std::fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(cfg) => {
                eprintln!("[config] loaded {}", path.display());
                cfg
            }
            Err(e) => {
                eprintln!("[config] {} unparseable ({e}) — using defaults", path.display());
                Config::default()
            }
        },
        Err(_) => {
            eprintln!("[config] no config file yet — using defaults");
            Config::default()
        }
    }
}

pub fn save(path: &PathBuf, config: &Config) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, serde_json::to_string_pretty(config).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    eprintln!("[config] saved {}", path.display());
    Ok(())
}
