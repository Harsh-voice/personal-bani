//! LLM transcript cleanup via local Ollama.
//!
//! Strips fillers, fixes grammar/punctuation, applies self-corrections.
//! Fires only above a length gate — short utterances aren't worth the LLM
//! round-trip and are more likely commands than prose.

use serde::Deserialize;

const SYSTEM_PROMPT: &str = "You are a transcription cleanup engine. Rewrite the dictated text: \
remove filler words (um, uh, like, you know), fix grammar and punctuation, and apply the \
speaker's self-corrections (if they correct themselves, keep only the corrected version). \
Do not add content, answer questions, change meaning, or add commentary. Output ONLY the \
cleaned text, nothing else.";

pub struct Cleanup {
    client: reqwest::blocking::Client,
    url: String,
    model: String,
    min_chars: usize,
    system_prompt: String,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

impl Cleanup {
    pub fn new(ollama_url: &str, model: &str, min_chars: usize, dictionary: &[String]) -> Self {
        let mut system_prompt = SYSTEM_PROMPT.to_string();
        if !dictionary.is_empty() {
            system_prompt.push_str(&format!(
                " The following terms are correctly-spelled names/jargon; keep them exactly \
                 as written, do not \"fix\" them: {}.",
                dictionary.join(", ")
            ));
        }
        Self {
            system_prompt,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("failed to build http client"),
            url: format!("{}/api/generate", ollama_url.trim_end_matches('/')),
            model: model.to_string(),
            min_chars,
        }
    }

    /// Clean `text`, or pass it through untouched when below the length gate
    /// or when Ollama is unreachable (dictation must never hard-fail on the
    /// cleanup stage).
    pub fn clean(&self, text: &str) -> String {
        if text.chars().count() <= self.min_chars {
            eprintln!(
                "[cleanup] skipped (≤{} chars), passing raw transcript through",
                self.min_chars
            );
            return text.to_string();
        }
        let started = std::time::Instant::now();
        match self.request(text) {
            Ok(cleaned) if cleaned.is_empty() => {
                eprintln!("[cleanup] model returned empty output — keeping raw transcript");
                text.to_string()
            }
            Ok(cleaned) => {
                eprintln!(
                    "[cleanup] cleaned in {:.2}s: {cleaned:?}",
                    started.elapsed().as_secs_f32()
                );
                cleaned
            }
            Err(e) => {
                eprintln!("[cleanup] FAILED ({e}) — keeping raw transcript");
                text.to_string()
            }
        }
    }

    fn request(&self, text: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "system": self.system_prompt,
            "prompt": text,
            "stream": false,
            // Keep the model resident between dictations; otherwise Ollama
            // unloads after 5min idle and the next utterance pays ~10s reload.
            "keep_alive": "30m",
        });
        let resp: GenerateResponse = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
        Ok(resp.response.trim().to_string())
    }
}
