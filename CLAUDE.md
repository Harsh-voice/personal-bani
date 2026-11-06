# rootvox

Local, fully-on-device clone of **Wispr Flow** (AI voice dictation). Push-to-talk → local STT → local LLM cleanup (via Ollama) → inject cleaned text at cursor system-wide. No cloud.

> **Session handoff:** current state, install locations, and next steps live in
> `docs/handoff-2026-07-06.md`. Read it before resuming work.

## Goal & scope

Full-agentic parity with Wispr Flow, entirely local:
- Push-to-talk voice dictation, real-time STT
- LLM cleanup: strip fillers (um/uh), fix grammar, auto-punctuate/format, real-time self-corrections ("meet at 2... actually 3" → "3")
- System-wide text injection into any app (browsers, IDEs, terminals, messaging)
- Agentic text edits ("make it formal"), command mode, custom dictionary, voice commands
- Advanced (do last): app-aware context, cross-app memory

## Decisions locked

- **Language/stack**: Rust + Tauri. NOT Swift — cross-platform core, avoid rewrite.
- **Approach**: build from scratch.
- **Platform order**: **Mac-first** (easiest injection, best HW, most prior art) → Fedora/Wayland → Windows. Mac-first ≠ Mac-only.
- **STT**: Parakeet TDT 0.6B v2 (CPU-efficient, 6.05% WER) as default; whisper.cpp (Metal on Mac) for accurate final pass. v0.1 MVP is whisper-only; Parakeet joins at phase 3 (dual-model partials).
- **LLM cleanup**: Ollama sidecar. Qwen 2.5 7B Instruct (sweet spot, ~5GB VRAM) or gemma3:4b (lighter). Fire only for text >50 chars.
- **VAD**: Silero.
- **Injection lib**: direct CGEvent on macOS (v0.1, working). Enigo planned for Windows/X11 — but note its macOS backend hard-fails for terminal-spawned dev builds (CGPreflightPostEventAccess false-negative), which is why Mac doesn't use it.

## Architecture

```
hotkey listener → audio capture → Silero VAD
  → STT: Parakeet (CPU) / whisper.cpp
      ↓ partial (tiny model, 1s chunks, live) → overlay
      ↓ final (big model, on key release)
  → Ollama HTTP → cleanup / agentic edit (>50 char gate)
  → TextInjector trait:
       macOS  = Accessibility API / CGEvent
       Linux  = ydotool / uinput (NOT clipboard — race)
       Windows= SendInput
```

**Key seams**: isolate both platform-specific pieces behind traits —
`TextInjector` (inject text at cursor) and `HotkeyBackend` (global hold-to-talk
capture). Mac impls first; Linux/Windows swap in later without touching
STT/VAD/Ollama/pipeline code. `pipeline.rs` talks only to traits, never
platform APIs. `#[cfg(target_os = "...")]` selects impls at compile time. This
is what keeps the Fedora/Windows port from being a rewrite. See Porting guide.

## Latency trick

Run tiny model on 1s chunks for live partials *while recording*; run big accurate model once on key-release for final. Hides slow pass behind fast preview.

## Hardest challenges (ranked)

1. **Linux/Wayland injection + global hotkey** — the boss fight. Deferred to phase 4.
   - xdotool silently fails on native Wayland (returns exit 0, does nothing). rdev global-listen = X11 only. Enigo Wayland backend = experimental/buggy.
   - Working path: kernel `/dev/uinput` via `ydotool` (needs `ydotoold` + perms) or `wtype`/`dotool`.
   - Clipboard-paste has a restore-timing race (text flashes/vanishes) — use direct keystroke synth, not clipboard.
   - Wayland global hotkey = no standard API; needs compositor portal or evdev.
2. LLM cleanup latency without killing real-time feel (measure; may make async/optional).
3. Clipboard/keystroke race conditions.
4. Agentic parity (app context, memory) — undocumented in Wispr, pure reverse-eng.

## Build phases

1. ✅ (2026-07-06) MVP core loop, Mac: hotkey → record → whisper.cpp → inject at cursor.
2. ✅ (2026-07-06) Ollama cleanup pass — folded into v0.1 with settings UI + tray toggle + config.
2.5. ✅ (2026-07-06) Quick-wins detour: recording-indicator overlay (transparent
   always-on-top pill), custom dictionary (whisper glossary bias + casing
   enforcement + replacement pairs), CyberPsyon brand UI + waveform icon,
   release .app packaging, Ollama model dropdown. Display name is **RootVox**
   (repo/binary/identifier stay `rootvox`).
3. Latency: dual-model partials + Silero VAD endpointing. **← next, but only if
   daily-driving surfaces latency pain** — user is daily-driving as of
   2026-07-06 to collect an annoyance list; let that drive priority.
4. Linux/Wayland injection (ydotool/uinput, direct keystroke, compositor hotkey).
5. Windows (SendInput via Enigo).
6. Agentic: command-mode intent detection, selected-text edits, custom dictionary, voice commands.
7. Advanced parity: app-aware context, cross-app memory. Speculative — last.

## Porting guide (Linux/Windows — later, not now)

Rust + Tauri means the codebase is inherently cross-platform: porting = swap 2
small modules, not a rewrite. Build Mac now; do NOT write Linux/Windows code
prematurely (YAGNI) — just preserve the trait seams so the ports are cheap.

**Ports with zero change (~70% of code):** cpal (audio), whisper-rs (STT),
Ollama HTTP cleanup, pipeline, config, Tauri UI. Platform-agnostic.

**Only 2 pieces are per-platform, both behind traits:**

| Piece | macOS (v0.1, done) | Windows | Linux X11 | Linux Wayland |
|---|---|---|---|---|
| `TextInjector` | direct CGEvent | enigo/SendInput (easy) | enigo/XTEST (easy) | ydotool/uinput (custom) |
| `HotkeyBackend` | direct CGEventTap | rdev (works) | rdev (works) | rdev BROKEN → uinput/evdev/portal |

**macOS learnings (v0.1, macOS 26):** rdev 0.5 is broken on modern macOS too,
not just Wayland — its key-event string conversion calls TIS APIs off the main
thread, which SIGTRAPs the whole app on any keypress. Both mac impls are
hand-rolled on core-graphics: CGEventTap on FlagsChanged for the hotkey
(modifier keys only in v0.1), CGEvent-with-unicode-payload for injection.
TCC gotchas: Accessibility grant needs a terminal restart to apply, and mic
permission blocks `Recorder::start()` on first use (first utterance is lost).

**Release packaging learnings (2026-07-06):**
- whisper.cpp uses `std::filesystem` → release build FAILS with Tauri's default
  `MACOSX_DEPLOYMENT_TARGET` 10.13. Fixed: `bundle.macOS.minimumSystemVersion:
  "11.0"` in tauri.conf.json. If you change it, wipe the stale CMake cache:
  `rm -rf src-tauri/target/release/build/whisper-rs-sys-*` before rebuilding.
- Transparent overlay window needs BOTH the `macos-private-api` cargo feature
  AND `"macOSPrivateApi": true` in tauri.conf.json — otherwise it renders as a
  white box.
- Config `whisper_model_path` must be ABSOLUTE for the bundled .app (cwd is
  unpredictable); relative `../models/...` only works in `tauri dev`.
- Tray apps need an explicit reopen path: "Open Settings" menu item + a
  `RunEvent::Reopen` handler, or a closed (hidden) window is unreachable.
- Running the naked binary (`RootVox.app/Contents/MacOS/rootvox`) shows a
  generic "exec" dock icon — launch via `open` or Finder.
- **TCC:** hotkey tap (ListenOnly) = Input Monitoring; injection =
  Accessibility; mic needs `NSMicrophoneUsageDescription`
  (src-tauri/Info.plist, Tauri auto-merges) **AND**
  `com.apple.security.device.audio-input` in `Entitlements.plist`
  (`bundle.macOS.entitlements`) — Tauri signs with the hardened runtime, and
  without that entitlement macOS auto-denies mic with NO prompt: CoreAudio
  returns silence, transcripts come back empty, and the app never appears in
  the Microphone privacy pane. cpal's HAL capture also never raises the TCC
  prompt by itself — `permissions.rs` asks explicitly via
  `AVCaptureDevice requestAccessForMediaType:`. Debug recipe for "bubble
  shows, nothing types": relaunch with
  `open /Applications/RootVox.app --stderr /tmp/rv.log` (keeps launchd TCC
  identity, captures eprintln); terminal-spawned launches inherit the
  terminal's mic grant and mask the bug. Grants are tied to the code
  signature — the app signs with the stable self-signed "RootVox Dev" cert
  (`bundle.macOS.signingIdentity`) so grants survive rebuilds. If that cert
  is missing on a new machine, recreate it in Keychain Access (Code Signing
  type, Always Trust) or builds fail. `permissions.rs` requests everything
  properly at launch.

**Effort by target:**
- **Windows ≈ a weekend.** enigo + rdev already support it. Add
  `WindowsInjector`, recompile, test. Mostly "does it work," not new code.
- **Linux X11 ≈ nearly free.** Same tools work.
- **Linux Wayland = the real project** (the daily-driver Fedora target). Both
  seams break: needs `/dev/uinput` injection (ydotool, direct keystroke — NOT
  clipboard, race) + a non-rdev hotkey path (uinput/evdev or compositor
  portal). This is phase 4, the boss fight.

**Keep the door open (cheap, do during Mac build):** all platform code behind
`TextInjector` + `HotkeyBackend` traits; no platform API leaks into
`pipeline.rs`; `#[cfg(target_os)]` selects impls; add a mac/win/linux CI build
matrix once a second platform exists so ports don't rot.

## How Wispr Flow works (reference)

4-stage cloud pipeline: streaming STT → fine-tuned Meta Llama LLM cleanup → inject at cursor via OS accessibility APIs. No offline mode (confirmed via their infra vendor Baseten). rootvox replaces cloud STT+LLM with local Parakeet/whisper + Ollama.

## Open questions (unresolved by research)

- Real end-to-end latency of full local STT+Ollama loop on consumer HW — no verified number exists. Measure ourselves.
- Reliable Wayland global hotkey capture — no standard API; investigate compositor portals / evdev.
- Wispr's app-aware context / cross-app memory internals — undocumented.

## Full research report

`docs/wispr-clone-research.md` (deep-research, 24 sources, 25 claims verified 3-vote adversarial).
