//! Custom dictionary: user terms (names, jargon) STT must spell exactly,
//! plus explicit replacement pairs for sound-alike misses whisper keeps
//! making ("cyber syon" → "Cyberpsyon").
//!
//! Three hooks: whisper initial_prompt bias (stt.rs consumes
//! `whisper_prompt`), post-STT text rewrite (`apply`), and the cleanup
//! system prompt (cleanup.rs gets the term list).

/// Whisper initial_prompt biasing decode toward dictionary terms.
/// Whisper treats it as preceding context, so phrase it as prose.
pub fn whisper_prompt(terms: &[String]) -> Option<String> {
    if terms.is_empty() {
        return None;
    }
    Some(format!("Glossary: {}.", terms.join(", ")))
}

/// Rewrite the transcript against the user dictionary: explicit
/// replacement pairs first (fix sound-alike misses), then enforce the
/// exact dictionary spelling of every whole-word, case-insensitive term
/// match ("Rootvox" → "RootVox"). All matching is whole-word.
pub fn apply(text: &str, terms: &[String], replacements: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (from, to) in replacements {
        if !from.is_empty() && !to.is_empty() {
            out = replace_word_ci(&out, from, to);
        }
    }
    for term in terms {
        if !term.is_empty() {
            out = replace_word_ci(&out, term, term);
        }
    }
    out
}

fn replace_word_ci(text: &str, pattern: &str, replacement: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let pat_lc: Vec<char> = pattern.to_lowercase().chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let end = i + pat_lc.len();
        let hit = end <= chars.len()
            && chars[i..end]
                .iter()
                .flat_map(|c| c.to_lowercase())
                .eq(pat_lc.iter().copied())
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && (end == chars.len() || !chars[end].is_alphanumeric());
        if hit {
            out.push_str(replacement);
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(f, t)| (f.to_string(), t.to_string())).collect()
    }

    #[test]
    fn fixes_casing_whole_word() {
        assert_eq!(
            apply("Rootvox uses OLLAMA.", &terms(&["RootVox", "Ollama"]), &[]),
            "RootVox uses Ollama."
        );
    }

    #[test]
    fn ignores_substrings() {
        assert_eq!(apply("groots", &terms(&["root"]), &[]), "groots");
    }

    #[test]
    fn multi_word_terms() {
        assert_eq!(
            apply("wispr flow is cloud", &terms(&["Wispr Flow"]), &[]),
            "Wispr Flow is cloud"
        );
    }

    #[test]
    fn empty_dictionary_passthrough() {
        assert_eq!(apply("hello", &[], &[]), "hello");
    }

    #[test]
    fn replacement_pair_case_insensitive() {
        assert_eq!(
            apply(
                "My name is Cyber Syon.",
                &[],
                &pairs(&[("cyber syon", "Cyberpsyon")])
            ),
            "My name is Cyberpsyon."
        );
    }

    #[test]
    fn replacement_respects_word_boundaries() {
        assert_eq!(
            apply("cyber syonics", &[], &pairs(&[("cyber syon", "Cyberpsyon")])),
            "cyber syonics"
        );
    }

    #[test]
    fn replacement_then_casing() {
        assert_eq!(
            apply(
                "cybercyon built rootvox",
                &terms(&["RootVox", "Cyberpsyon"]),
                &pairs(&[("cybercyon", "Cyberpsyon")])
            ),
            "Cyberpsyon built RootVox"
        );
    }
}
