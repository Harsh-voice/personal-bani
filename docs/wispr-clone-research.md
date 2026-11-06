---
name: wispr-clone-research
description: Verified deep-research facts on Wispr Flow internals + local STT/LLM/injection stack for rootvox
metadata: 
  node_type: memory
  type: reference
  originSessionId: 0573de5b-76c4-4135-835c-78990d644c62
---

Deep-research (2026-07-05, 24 sources, 25 claims verified 3-vote adversarial) for [[rootvox-project]].

**Wispr Flow internals**: 4-stage cloud pipeline — streaming STT → fine-tuned Meta Llama LLM cleanup (strips um/uh, grammar, real-time self-corrections) → inject at cursor via OS accessibility APIs. No offline mode (confirmed via infra vendor Baseten).

**STT options**: Parakeet TDT 0.6B v2 = 6.05% WER, CPU-efficient (runs CPU-side). Moonshine v2 = 50/148/258ms latency M3, 6.65-12% WER (first-party arXiv, streaming-vs-batch — optimistic). whisper.cpp/faster-whisper same WER, differ in runtime.

**LLM cleanup**: Qwen 2.5 7B Instruct = sweet spot (~5GB VRAM). gemma3:4b lighter. Gate: only fire >50 chars.

**Hardest problem = Linux/Wayland injection + global hotkey**:
- xdotool silently fails on native Wayland (exit 0, does nothing). rdev global-listen = X11 only. Enigo Wayland backend experimental/buggy.
- Working path: `/dev/uinput` via ydotool (needs ydotoold + perms), or wtype/dotool.
- Clipboard-paste race: fixed restore delay fires before app processes paste → text flashes/vanishes. Use direct keystroke synth, not clipboard.
- macOS injection = easy (Accessibility API/CGEvent, one-time permission). Windows = SendInput.

**Unresolved**: no verified end-to-end latency for local STT+Ollama loop on consumer HW; reliable Wayland global hotkey capture (no standard API — compositor portals/evdev); Wispr app-context/memory internals undocumented.
