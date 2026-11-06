import { invoke } from "@tauri-apps/api/core";

interface Config {
  hotkey_keycode: number;
  whisper_model_path: string;
  ollama_url: string;
  ollama_model: string;
  cleanup_enabled: boolean;
  cleanup_min_chars: number;
  dictionary: string[];
  replacements: [string, string][];
}

const READY = "RootVox running. Hold Right Option to dictate.";

function el<T extends HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

function setStatus(text: string) {
  el<HTMLParagraphElement>("status").textContent = text;
}

/// Fill the model dropdown from the local Ollama instance; the saved model
/// stays selectable (and first) even when Ollama is unreachable.
async function loadOllamaModels(current: string) {
  const sel = el<HTMLSelectElement>("ollama_model");
  let models: string[] = [];
  try {
    models = await invoke<string[]>("list_ollama_models");
  } catch {
    // Ollama down — fall through to just the saved model.
  }
  if (current && !models.includes(current)) models.unshift(current);
  if (models.length === 0) models = ["qwen2.5:7b-instruct"];
  sel.innerHTML = "";
  for (const m of models) {
    const opt = document.createElement("option");
    opt.value = m;
    opt.textContent = m;
    sel.appendChild(opt);
  }
  sel.value = current || models[0];
}

async function loadConfig() {
  const cfg = await invoke<Config>("get_config");
  el<HTMLInputElement>("ollama_url").value = cfg.ollama_url;
  await loadOllamaModels(cfg.ollama_model);
  el<HTMLInputElement>("cleanup_enabled").checked = cfg.cleanup_enabled;
  el<HTMLInputElement>("cleanup_min_chars").value = String(cfg.cleanup_min_chars);
  el<HTMLInputElement>("whisper_model_path").value = cfg.whisper_model_path;
  el<HTMLTextAreaElement>("dictionary").value = cfg.dictionary.join("\n");
  el<HTMLTextAreaElement>("replacements").value = cfg.replacements
    .map(([from, to]) => `${from} -> ${to}`)
    .join("\n");
  return cfg;
}

window.addEventListener("DOMContentLoaded", async () => {
  let current: Config;
  try {
    current = await loadConfig();
    setStatus(READY);
  } catch (e) {
    setStatus(`Failed to load config: ${e}`);
    return;
  }

  el<HTMLFormElement>("settings").addEventListener("submit", async (e) => {
    e.preventDefault();
    const newConfig: Config = {
      ...current,
      ollama_url: el<HTMLInputElement>("ollama_url").value.trim(),
      ollama_model: el<HTMLSelectElement>("ollama_model").value,
      cleanup_enabled: el<HTMLInputElement>("cleanup_enabled").checked,
      cleanup_min_chars: Number(el<HTMLInputElement>("cleanup_min_chars").value) || 0,
      whisper_model_path: el<HTMLInputElement>("whisper_model_path").value.trim(),
      dictionary: el<HTMLTextAreaElement>("dictionary")
        .value.split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
      replacements: el<HTMLTextAreaElement>("replacements")
        .value.split("\n")
        .map((line): [string, string] => {
          const sep = line.indexOf("->");
          if (sep === -1) return ["", ""];
          return [line.slice(0, sep).trim(), line.slice(sep + 2).trim()];
        })
        .filter(([from, to]) => from && to),
    };
    try {
      await invoke("set_config", { newConfig });
      current = newConfig;
      setStatus("Settings saved.");
    } catch (e) {
      setStatus(`Save failed: ${e}`);
    }
    setTimeout(() => setStatus(READY), 3000);
  });

  el<HTMLButtonElement>("test-inject").addEventListener("click", async () => {
    setStatus("Focus a text field — typing in 3 seconds…");
    await invoke("test_inject");
    setTimeout(() => setStatus(READY), 4000);
  });
});
