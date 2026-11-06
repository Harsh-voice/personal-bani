//! System-wide text injection at the cursor.
//!
//! `TextInjector` is the second platform seam (see CLAUDE.md "Key seams").
//! macOS posts CGEvents directly (enigo 0.6 was tried and hard-fails here:
//! its CGPreflightPostEventAccess check returns false for terminal child
//! processes even when Accessibility is granted, while actually posting
//! events works fine). Windows gets enigo/SendInput at phase 5; Linux
//! Wayland gets ydotool/uinput at phase 4. Direct keystroke synthesis,
//! never clipboard-paste (restore race).

pub trait TextInjector: Send {
    /// Type `text` into whatever app currently has keyboard focus.
    fn inject(&mut self, text: &str) -> Result<(), String>;
}

#[cfg(target_os = "macos")]
pub use mac::MacInjector as PlatformInjector;

#[cfg(target_os = "macos")]
mod mac {
    use super::TextInjector;
    use core_graphics::event::{CGEvent, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    /// Injects text as CGEvent keyboard events carrying unicode payloads
    /// (keycode stays 0 — receivers read the attached string, so this types
    /// arbitrary unicode without caring about keyboard layout).
    pub struct MacInjector;

    /// Apps drop characters beyond ~20 UTF-16 units per keyboard event, so
    /// chunk long transcripts.
    const CHUNK_UTF16: usize = 20;

    /// Split into ≤CHUNK_UTF16-unit chunks without ever splitting a
    /// character across events (a surrogate pair split across two keyboard
    /// events renders as garbage in the receiving app).
    fn chunk_utf16(text: &str) -> Vec<Vec<u16>> {
        let mut chunks = Vec::new();
        let mut cur: Vec<u16> = Vec::new();
        let mut buf = [0u16; 2];
        for c in text.chars() {
            let units = c.encode_utf16(&mut buf);
            if cur.len() + units.len() > CHUNK_UTF16 && !cur.is_empty() {
                chunks.push(std::mem::take(&mut cur));
            }
            cur.extend_from_slice(units);
        }
        if !cur.is_empty() {
            chunks.push(cur);
        }
        chunks
    }

    impl TextInjector for MacInjector {
        fn inject(&mut self, text: &str) -> Result<(), String> {
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .map_err(|_| "failed to create CGEventSource".to_string())?;

            for chunk in chunk_utf16(text) {
                let down = CGEvent::new_keyboard_event(source.clone(), 0, true)
                    .map_err(|_| "failed to create key-down event".to_string())?;
                down.set_string_from_utf16_unchecked(&chunk);
                down.post(CGEventTapLocation::HID);

                let up = CGEvent::new_keyboard_event(source.clone(), 0, false)
                    .map_err(|_| "failed to create key-up event".to_string())?;
                up.post(CGEventTapLocation::HID);

                // Give slow apps time to drain the event queue between chunks.
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            eprintln!("[inject] typed {} chars", text.chars().count());
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn chunks_never_split_surrogate_pairs() {
            // 21 chars: 19 ASCII then an emoji (2 UTF-16 units) — the emoji
            // must move whole into the second chunk, not straddle 20.
            let text = format!("{}🎤x", "a".repeat(19));
            let chunks = chunk_utf16(&text);
            assert_eq!(chunks[0].len(), 19);
            assert_eq!(chunks[1], "🎤x".encode_utf16().collect::<Vec<u16>>());
            let total: usize = chunks.iter().map(|c| c.len()).sum();
            assert_eq!(total, text.encode_utf16().count());
        }

        #[test]
        fn ascii_chunks_at_limit() {
            let chunks = chunk_utf16(&"a".repeat(45));
            assert_eq!(
                chunks.iter().map(|c| c.len()).collect::<Vec<_>>(),
                vec![20, 20, 5]
            );
        }
    }
}
