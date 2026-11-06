# Mac kickoff prompt — MVP v0.1

Paste this into a Claude Code session (model: Fable) opened in this repo on macOS.
CLAUDE.md + docs/ auto-load. This is the session's kickoff scope.

Before pasting: grant **Microphone** + **Accessibility** perms when the agent
pauses (System Settings → Privacy & Security). Those are GUI clicks the agent
can't do.

---

```
You are building rootvox on macOS. First, read CLAUDE.md and docs/ in this
repo — they hold the full plan, locked decisions, and verified research.
That's the project bible; this message is THIS session's scope.

## Session goal: MVP v0.1

Build a working Tauri v2 desktop app (macOS only for now) that does the core
Wispr Flow loop:

  hold hotkey → record mic → transcribe locally (whisper) → clean up text
  with a local LLM via Ollama → inject cleaned text at the cursor in any app

No Linux, no Windows, no advanced features (command mode, dictionary,
app-context) this session. Prove the core loop end-to-end, mac-native.

## Process

Decisions are already locked (see CLAUDE.md) — no need to re-brainstorm the
stack. Use TDD where it fits, debug systematically on any failure, and use
the /verify skill before you claim any gate passes (run it, show real
output — never assert success without evidence). Grant-permission and
"speak into the mic" steps are mine to do; pause and tell me clearly when
you need them.

## Stack (all locked)

- Rust + Tauri v2 (app from the start; tray icon + settings later in session)
- Hotkey (hold-to-talk): rdev in a background thread (gives key-down AND
  key-up, which Tauri's global-shortcut plugin does not). Default key: Right
  Option. Needs Accessibility permission. Put it behind a `HotkeyBackend`
  trait (rdev impl) mirroring TextInjector — keeps the later Wayland port
  cheap (rdev is broken on Wayland). Caveat: rdev on macOS may report Right
  Option as `Key::AltGr` or `Key::Unknown(61)` depending on version — log
  raw keycodes in hotkey.rs and match on what actually arrives; key is
  configurable anyway.
- Mic capture: cpal → buffer → hound (wav / f32 samples). Mac default input
  is usually 48kHz; whisper requires 16kHz mono f32 — add a resample step in
  audio.rs (rubato crate, or simple decimation) or transcription will be
  garbage.
- STT: whisper-rs (bundles whisper.cpp with Metal). Download a ggml model
  (start with ggml-base.en.bin; make the path configurable).
- LLM cleanup: reqwest POST to Ollama at http://localhost:11434/api/generate,
  model qwen2.5:7b-instruct, streaming off for v0.1. Only run cleanup when
  transcript length > 50 chars (configurable).
- Inject at cursor: enigo (CGEvent). Needs Accessibility permission. Put it
  behind a `TextInjector` trait with a `MacInjector` impl — keep that seam
  even though only mac exists now (Linux/Windows swap in later).

## Prereqs — install and VERIFY before writing app code

1. xcode-select --install (Command Line Tools)
2. rustup + stable toolchain
3. Ollama: install, start `ollama serve`, `ollama pull qwen2.5:7b-instruct`,
   verify with a curl to /api/generate
4. Tauri v2 CLI + prereqs
5. Download ggml-base.en.bin into a models/ dir (gitignored)
Report what's already present vs what you installed.

## Module layout (src-tauri/src)

  main.rs      Tauri setup, system tray, spawn hotkey thread
  config.rs    Config struct + load/save (hotkey, model paths, ollama url +
               model, cleanup_min_chars, cleanup_enabled)
  hotkey.rs    rdev listener, hold/release detection, emit start/stop
  audio.rs     cpal capture while held → samples/wav
  stt.rs       whisper-rs transcribe
  cleanup.rs   Ollama HTTP cleanup
  inject.rs    TextInjector trait + MacInjector (enigo)
  pipeline.rs  orchestrate: start → record → stt → cleanup → inject

## Ollama cleanup system prompt (use exactly this intent)

  "You are a transcription cleanup engine. Rewrite the dictated text: remove
  filler words (um, uh, like, you know), fix grammar and punctuation, and
  apply the speaker's self-corrections (if they correct themselves, keep only
  the corrected version). Do not add content, answer questions, change
  meaning, or add commentary. Output ONLY the cleaned text, nothing else."

## Build order — VERIFY each gate before the next, show me output

0. Scaffold Tauri v2 app; it launches; tray icon appears.
1. rdev thread detects Right-Option hold and release; log both. (I grant
   Accessibility here — pause and tell me.) IMPORTANT: macOS ties the
   Accessibility grant to the binary's code signature. An unsigned dev
   binary changes on every rebuild, so the permission silently stops
   working (no error — rdev/enigo just go dead). Fix: grant Accessibility
   to the terminal app running `cargo tauri dev` (children inherit it), or
   ad-hoc codesign with a stable identifier. If gates 1 or 4 flake after a
   rebuild, suspect this first.
2. Record mic while key held → write test.wav → I confirm it plays.
3. whisper-rs transcribes test.wav → log the text.
4. enigo types a fixed string ("hello from rootvox") at my cursor → I
   confirm it appears in a text field.
5. Wire the full pipeline with the RAW transcript (no LLM yet) → I hold,
   speak, and the raw text is typed. Confirm end-to-end.
6. Insert the Ollama cleanup stage (>50-char gate) → I speak a filler-heavy
   sentence and the cleaned version is typed. Confirm the difference.
7. Minimal settings UI + a tray toggle to enable/disable. Wire config.rs.

## Definition of done

I hold Right Option, say "um so like I need to, uh, send the report by
Friday", release, and "I need to send the report by Friday." is typed at my
cursor in whatever app is focused — fully locally, no network except
localhost Ollama.

Start by reading CLAUDE.md + docs/, then report the prereq status and your
step-0 plan before writing code.
```
