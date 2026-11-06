import { listen } from "@tauri-apps/api/event";

const pill = document.getElementById("pill")!;
const label = document.getElementById("label")!;

const LABELS: Record<string, string> = {
  recording: "Listening…",
  processing: "Processing…",
  error: "RootVox error — check settings",
  silent: "Heard nothing — check mic permission",
};

// Backend emits "recording" | "processing" | "error" | "silent" right before showing.
listen<string>("overlay-state", (event) => {
  pill.className = event.payload;
  label.textContent = LABELS[event.payload] ?? event.payload;
});
