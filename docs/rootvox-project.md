---
name: rootvox-project
description: "rootvox = local fully-on-device Wispr Flow clone; goals, locked stack decisions, build phases"
metadata: 
  node_type: memory
  type: project
  originSessionId: 0573de5b-76c4-4135-835c-78990d644c62
---

**rootvox** = a from-scratch, fully-local clone of Wispr Flow (AI voice dictation). Push-to-talk → local STT → local Ollama LLM cleanup → system-wide cursor text injection. No cloud. Full-agentic scope (fillers/grammar/formatting, agentic edits like "make it formal", command mode, custom dictionary; app-context + cross-app memory last).

**Locked decisions (2026-07-05):**
- Stack: **Rust + Tauri**. Not Swift (avoid Fedora-port rewrite).
- Build from scratch.
- **Mac-first, not Mac-only.** Order: macOS → Fedora/Wayland → Windows. Isolate injection behind one `TextInjector` trait so Fedora port isn't a rewrite.
- STT: Parakeet TDT 0.6B v2 (CPU default) + whisper.cpp (Metal, accurate final). Dual-model latency trick: tiny on 1s chunks for live partials, big on key-release.
- LLM cleanup: Ollama sidecar, Qwen 2.5 7B Instruct or gemma3:4b, fire only >50 chars.
- VAD: Silero. Injection base lib: Enigo.

Full plan + architecture + build phases live in project `CLAUDE.md` (loaded each session). See [[wispr-clone-research]] for verified research facts. User works mainly on Fedora 42 PC + MacBook Pro; Windows 11 laptop secondary (school/certs).
