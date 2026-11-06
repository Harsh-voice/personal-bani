mod audio;
mod cleanup;
mod config;
mod dictionary;
mod hotkey;
mod inject;
mod overlay;
mod permissions;
mod pipeline;
mod stt;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use config::{Config, SharedConfig};
use hotkey::{HotkeyBackend, PlatformBackend};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State,
};

struct AppState {
    config: SharedConfig,
    config_path: PathBuf,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn set_config(new_config: Config, state: State<AppState>) -> Result<(), String> {
    *state.config.lock().unwrap() = new_config.clone();
    config::save(&state.config_path, &new_config)
}

/// Models installed in the local Ollama instance, for the settings dropdown.
/// Async so the 3s-timeout HTTP call never blocks the UI thread.
#[tauri::command]
async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let base = state.config.lock().unwrap().ollama_url.clone();
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let resp: serde_json::Value = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

/// Gate 4 test: focus any text field within 3 seconds of clicking the
/// button, then a fixed string is typed at the cursor.
#[tauri::command]
fn test_inject() {
    std::thread::spawn(|| {
        use inject::TextInjector;
        std::thread::sleep(std::time::Duration::from_secs(3));
        if let Err(e) = inject::PlatformInjector.inject("hello from rootvox") {
            eprintln!("[inject] FAILED: {e}");
        }
    });
}

fn show_settings(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            test_inject,
            list_ollama_models
        ])
        .on_window_event(|window, event| {
            // Tray app: closing the window hides it; quit only via tray menu.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let config_path = app
                .path()
                .app_config_dir()
                .expect("no app config dir")
                .join("config.json");
            let cfg = config::load(&config_path);
            let shared: SharedConfig = Arc::new(Mutex::new(cfg.clone()));
            let enabled = Arc::new(AtomicBool::new(true));

            let toggle = CheckMenuItem::with_id(
                app,
                "enabled",
                "Dictation enabled",
                true,
                true,
                None::<&str>,
            )?;
            let settings = MenuItem::with_id(app, "settings", "Open Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit RootVox", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings, &toggle, &quit])?;

            let enabled_for_tray = Arc::clone(&enabled);
            let toggle_for_tray = toggle.clone();
            TrayIconBuilder::with_id("rootvox-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "settings" => show_settings(app),
                    "enabled" => {
                        // CheckMenuItem toggles itself; mirror it into the flag.
                        let on = toggle_for_tray.is_checked().unwrap_or(true);
                        enabled_for_tray.store(on, Ordering::Relaxed);
                        eprintln!("[tray] dictation {}", if on { "enabled" } else { "disabled" });
                    }
                    _ => {}
                })
                .build(app)?;

            // Ask for Input Monitoring + Accessibility up front — silent
            // failure modes otherwise (see permissions.rs).
            permissions::request_all();

            // Warm up the mic: fires the (one-time) permission prompt at
            // launch and spins up CoreAudio, so the first real dictation
            // isn't swallowed.
            std::thread::spawn(|| match audio::Recorder::start() {
                Ok(r) => {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    drop(r);
                    eprintln!("[audio] mic warmup done");
                }
                Err(e) => eprintln!("[audio] mic warmup failed: {e}"),
            });

            // Indicator overlay must never eat clicks meant for the app under it.
            if let Some(overlay_win) = app.get_webview_window("overlay") {
                let _ = overlay_win.set_ignore_cursor_events(true);
            }

            let (tx, rx) = mpsc::channel();
            let keycode = shared.lock().unwrap().hotkey_keycode;
            std::thread::spawn(move || {
                Box::new(PlatformBackend { keycode }).listen(tx);
            });
            pipeline::spawn(
                rx,
                Arc::clone(&shared),
                Arc::clone(&enabled),
                app.handle().clone(),
            );

            app.manage(AppState {
                config: shared,
                config_path,
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Dock icon click while window is hidden reopens settings.
            if let tauri::RunEvent::Reopen { .. } = event {
                show_settings(app);
            }
        });
}
