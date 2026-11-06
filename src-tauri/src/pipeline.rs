//! Pipeline orchestration: hotkey events → record → STT → cleanup → inject.
//!
//! Talks only to the `HotkeyBackend`/`TextInjector` traits and the
//! platform-agnostic audio/stt/cleanup modules — no platform APIs here
//! (see CLAUDE.md "Key seams").

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use tauri::AppHandle;

use crate::config::SharedConfig;
use crate::hotkey::HotkeyEvent;
use crate::inject::TextInjector;
use crate::{audio, cleanup, dictionary, inject, overlay, stt};

pub fn spawn(
    rx: Receiver<HotkeyEvent>,
    config: SharedConfig,
    enabled: Arc<AtomicBool>,
    app: AppHandle,
) {
    std::thread::spawn(move || {
        let model_path = config.lock().unwrap().whisper_model_path.clone();
        let stt = match stt::Stt::new(&model_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[pipeline] STT init failed: {e}");
                overlay::flash(&app, overlay::State::Error);
                return;
            }
        };
        let mut injector = inject::PlatformInjector;
        let mut recorder: Option<audio::Recorder> = None;

        for event in rx {
            if !enabled.load(Ordering::Relaxed) {
                // Drop a recording in progress if dictation was toggled off mid-hold.
                if recorder.take().is_some() {
                    overlay::hide(&app);
                }
                continue;
            }
            match event {
                HotkeyEvent::Pressed => match audio::Recorder::start() {
                    Ok(r) => {
                        recorder = Some(r);
                        overlay::show(&app, overlay::State::Recording);
                    }
                    Err(e) => {
                        eprintln!("[pipeline] recorder failed: {e}");
                        overlay::flash(&app, overlay::State::Error);
                    }
                },
                HotkeyEvent::Released => {
                    if let Some(r) = recorder.take() {
                        overlay::show(&app, overlay::State::Processing);
                        let samples = r.stop();
                        let cfg = config.lock().unwrap().clone();
                        // Replacement targets are dictionary terms too: bias
                        // whisper toward them and protect them from cleanup.
                        let mut terms = cfg.dictionary.clone();
                        for (_, to) in &cfg.replacements {
                            if !terms.contains(to) {
                                terms.push(to.clone());
                            }
                        }
                        let glossary = dictionary::whisper_prompt(&terms);
                        match stt.transcribe(&samples, glossary.as_deref()) {
                            Ok(text) if text.is_empty() => {
                                eprintln!("[pipeline] empty transcript, nothing to inject");
                                // Empty transcript usually means the mic fed us
                                // silence (e.g. permission silently denied) —
                                // tell the user instead of failing invisibly.
                                overlay::flash(&app, overlay::State::Silent);
                                continue;
                            }
                            Ok(text) => {
                                let text = dictionary::apply(&text, &terms, &cfg.replacements);
                                let final_text = if cfg.cleanup_enabled {
                                    cleanup::Cleanup::new(
                                        &cfg.ollama_url,
                                        &cfg.ollama_model,
                                        cfg.cleanup_min_chars,
                                        &terms,
                                    )
                                    .clean(&text)
                                } else {
                                    text
                                };
                                // Cleanup LLM can re-mangle terms; enforce again.
                                let final_text =
                                    dictionary::apply(&final_text, &terms, &cfg.replacements);
                                // Trailing space so back-to-back dictations
                                // don't run together at the cursor.
                                let final_text = format!("{} ", final_text.trim_end());
                                if let Err(e) = injector.inject(&final_text) {
                                    eprintln!("[pipeline] injection failed: {e}");
                                }
                            }
                            Err(e) => {
                                eprintln!("[pipeline] STT failed: {e}");
                                overlay::flash(&app, overlay::State::Error);
                                continue;
                            }
                        }
                        overlay::hide(&app);
                    }
                }
            }
        }
    });
}
