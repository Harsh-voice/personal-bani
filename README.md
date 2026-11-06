# RootVox

> **Personal fork.** This is a private copy of
> [cyberpsyon/rootvox](https://github.com/cyberpsyon/rootvox) by Ben Mickens,
> used under the MIT License (see `LICENSE`). Upstream code is unmodified as of
> commit `e316247`.

**Fully local, push-to-talk AI voice dictation for macOS.** Hold a key, speak,
release — cleaned-up text appears at your cursor in any app. Your audio never
leaves your machine.

Think [Wispr Flow](https://wisprflow.ai), but 100% on-device: local
speech-to-text (whisper.cpp on Metal), local LLM cleanup (Ollama), zero cloud,
zero subscription, zero telemetry.

## How it works

```
Hold Right ⌥  →  record (cpal)  →  whisper.cpp (Metal)
                                        ↓
                     custom dictionary: replacements + casing
                                        ↓
              Ollama cleanup — strips "um/uh", fixes grammar,
              applies self-corrections ("at 2… actually 3" → "3")
                                        ↓
                     CGEvent keystroke injection at cursor
```

A floating pill overlay shows recording (red) / processing (teal) state.

## Features

- **Push-to-talk dictation** into any app — browsers, IDEs, terminals, chat
- **LLM cleanup**: filler removal, grammar, punctuation, live self-corrections
- **Custom dictionary**: bias the STT toward your names/jargon, enforce exact
  spelling, and map stubborn mishears (`cyber syon -> Cyberpsyon`)
- **Model picker**: any Ollama model on your system; whisper model swappable
  (base / small / large-v3-turbo)
- **Menu-bar app** with settings UI, all processing on-device

## Stack

Rust + Tauri. whisper.cpp via whisper-rs (Metal), cpal audio, Ollama over
HTTP, hand-rolled CGEventTap hotkey + CGEvent injection (see
`docs/` for why the usual crates don't survive modern macOS).

Platform-specific code is isolated behind two traits (`TextInjector`,
`HotkeyBackend`) — Linux (Wayland/uinput) and Windows (SendInput) ports are
planned without touching the pipeline.

## Build

```bash
# prereqs: Rust, Node, Ollama (brew install ollama), a whisper ggml model
npm install
ollama pull qwen2.5:7b-instruct
curl -L -o models/ggml-small.en.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin

npm run tauri dev     # development
npm run tauri build   # release .app + .dmg
```

First run: grant Accessibility + Microphone in System Settings, then restart
the app. Point "Whisper model path" in settings at your downloaded model
(absolute path).

## Status

v0.1 — daily-driven on macOS 26 (Apple Silicon). Roadmap: live partial
transcripts while speaking, VAD endpointing, voice command mode, then
Linux/Wayland and Windows ports.

## License

MIT © Ben Mickens ([cyberpsyon.com](https://cyberpsyon.com))
