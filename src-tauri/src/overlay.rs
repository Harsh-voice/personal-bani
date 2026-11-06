//! Recording-indicator overlay: small always-on-top pill shown while
//! recording (red pulsing dot) or processing (amber dot). Uses only
//! Tauri window APIs — platform-agnostic, no seam needed.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

/// Gap between the bottom of the overlay and the bottom of the screen,
/// in physical pixels — clears the macOS Dock at default size.
const BOTTOM_MARGIN_PX: i32 = 120;

#[derive(Clone, Copy)]
pub enum State {
    Recording,
    Processing,
    Error,
    /// Transcript came back empty — mic likely heard silence (permission
    /// denied hands back zeros with no error; see permissions.rs).
    Silent,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            State::Recording => "recording",
            State::Processing => "processing",
            State::Error => "error",
            State::Silent => "silent",
        }
    }
}

/// Show a state briefly, then hide. Rare-path feedback (mic missing, model
/// failed to load, empty transcript); a hotkey press during the flash
/// re-shows the recording state, and the delayed hide may then clip it —
/// acceptable for an error path.
pub fn flash(app: &AppHandle, state: State) {
    show(app, state);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        hide(&app);
    });
}

pub fn show(app: &AppHandle, state: State) {
    let Some(win) = app.get_webview_window("overlay") else {
        eprintln!("[overlay] overlay window missing");
        return;
    };
    let _ = app.emit_to("overlay", "overlay-state", state.as_str());
    position_bottom_center(&win);
    if let Err(e) = win.show() {
        eprintln!("[overlay] show failed: {e}");
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        let _ = win.hide();
    }
}

fn position_bottom_center(win: &tauri::WebviewWindow) {
    // Prefer the monitor the cursor is on (that's where the user is
    // dictating); fall back to the primary.
    let app = win.app_handle();
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|pos| app.monitor_from_point(pos.x, pos.y).ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    if let (Some(monitor), Ok(size)) = (monitor, win.outer_size()) {
        let mon_size = monitor.size();
        let mon_pos = monitor.position();
        let x = mon_pos.x + (mon_size.width as i32 - size.width as i32) / 2;
        let y = mon_pos.y + mon_size.height as i32 - size.height as i32 - BOTTOM_MARGIN_PX;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
}
