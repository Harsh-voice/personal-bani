//! Global hold-to-talk hotkey capture.
//!
//! `HotkeyBackend` is a platform seam (see CLAUDE.md "Key seams"). The macOS
//! impl is a raw CGEventTap rather than rdev: rdev 0.5 converts every key
//! event to a string via TIS/TSM APIs, which macOS 26 asserts must run on the
//! main queue — any keypress SIGTRAPs the whole app when listening from a
//! background thread. We only need FlagsChanged + one keycode, so we tap
//! exactly that and never touch the text-input APIs.

use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// Push-to-talk key went down — start recording.
    Pressed,
    /// Push-to-talk key came up — stop recording, run pipeline.
    Released,
}

pub trait HotkeyBackend: Send {
    /// Block forever, sending `Pressed`/`Released` for the configured key.
    /// Run this on a dedicated thread.
    fn listen(self: Box<Self>, tx: Sender<HotkeyEvent>);
}

#[cfg(target_os = "macos")]
pub use mac::MacTapBackend as PlatformBackend;

#[cfg(target_os = "macos")]
mod mac {
    use super::{HotkeyBackend, HotkeyEvent};
    use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
    use core_graphics::event::{
        CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
        CGEventType, EventField,
    };
    use std::cell::Cell;
    use std::sync::mpsc::Sender;

    /// v0.1 supports modifier keys only (they arrive as FlagsChanged with a
    /// per-family flag bit; regular keys would need KeyDown/KeyUp taps and
    /// collide with typing). 61 = Right Option, the default.
    fn flag_for_keycode(keycode: i64) -> CGEventFlags {
        match keycode {
            58 | 61 => CGEventFlags::CGEventFlagAlternate, // left/right Option
            54 | 55 => CGEventFlags::CGEventFlagCommand,   // right/left Command
            56 | 60 => CGEventFlags::CGEventFlagShift,     // left/right Shift
            59 | 62 => CGEventFlags::CGEventFlagControl,   // left/right Control
            63 => CGEventFlags::CGEventFlagSecondaryFn,    // Fn
            _ => CGEventFlags::CGEventFlagAlternate,
        }
    }

    pub struct MacTapBackend {
        pub keycode: i64,
    }

    impl HotkeyBackend for MacTapBackend {
        fn listen(self: Box<Self>, tx: Sender<HotkeyEvent>) {
            // Modifier keys arrive as FlagsChanged, not KeyDown/KeyUp.
            // Pressed vs released is derived from the modifier flag state.
            let ptt_keycode = self.keycode;
            let ptt_flag = flag_for_keycode(ptt_keycode);
            let held = Cell::new(false);
            let tap = CGEventTap::new(
                CGEventTapLocation::Session,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::ListenOnly,
                vec![CGEventType::FlagsChanged],
                move |_proxy, _type, event| {
                    let keycode =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                    if keycode == ptt_keycode {
                        let down = event.get_flags().contains(ptt_flag);
                        if down && !held.get() {
                            held.set(true);
                            eprintln!("[hotkey] PTT pressed (keycode {ptt_keycode})");
                            let _ = tx.send(HotkeyEvent::Pressed);
                        } else if !down && held.get() {
                            held.set(false);
                            eprintln!("[hotkey] PTT released (keycode {ptt_keycode})");
                            let _ = tx.send(HotkeyEvent::Released);
                        }
                    }
                    None
                },
            );

            match tap {
                Ok(tap) => {
                    let source = tap
                        .mach_port
                        .create_runloop_source(0)
                        .expect("failed to create event tap runloop source");
                    let run_loop = CFRunLoop::get_current();
                    unsafe {
                        run_loop.add_source(&source, kCFRunLoopCommonModes);
                    }
                    tap.enable();
                    eprintln!("[hotkey] event tap installed, listening for keycode {ptt_keycode}");
                    CFRunLoop::run_current();
                    eprintln!("[hotkey] run loop exited unexpectedly");
                }
                Err(()) => {
                    eprintln!(
                        "[hotkey] FAILED to create event tap — grant Accessibility permission \
                         to the app (or the terminal running `tauri dev`) and restart it"
                    );
                }
            }
        }
    }
}
