//! Startup TCC permission requests (macOS).
//!
//! Without these, a missing grant fails silently: an unauthorized event tap
//! still receives the app's own events (hotkey "works" only while RootVox is
//! focused), and CoreAudio hands back silence instead of erroring. Asking
//! explicitly at launch makes macOS show the prompt / add the app to the
//! right Privacy & Security pane so the user never hunts for a "+" button.

#[cfg(target_os = "macos")]
pub fn request_all() {
    mac::request_all();
}

#[cfg(not(target_os = "macos"))]
pub fn request_all() {}

#[cfg(target_os = "macos")]
mod mac {
    use block2::StackBlock;
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, Bool};
    use objc2_foundation::{ns_string, NSString};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGPreflightListenEventAccess() -> bool;
        fn CGRequestListenEventAccess() -> bool;
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
        static kAXTrustedCheckOptionPrompt: CFStringRef;
    }

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    pub fn request_all() {
        unsafe {
            // Input Monitoring — governs the ListenOnly hotkey event tap.
            if CGPreflightListenEventAccess() {
                eprintln!("[perms] Input Monitoring: granted");
            } else {
                eprintln!("[perms] Input Monitoring: MISSING — requesting (grant, then relaunch)");
                CGRequestListenEventAccess();
            }

            // Accessibility — governs CGEvent posting (text injection).
            if AXIsProcessTrusted() {
                eprintln!("[perms] Accessibility: granted");
            } else {
                eprintln!("[perms] Accessibility: MISSING — prompting (grant, then relaunch)");
                let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
                let opts = CFDictionary::from_CFType_pairs(&[(
                    key.as_CFType(),
                    CFBoolean::true_value().as_CFType(),
                )]);
                AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef());
            }
            // Microphone — cpal's CoreAudio HAL capture does NOT raise the TCC
            // prompt (macOS 26 hands back silence with no error and the app
            // never appears in the Privacy pane), so ask explicitly through
            // AVCaptureDevice before the warmup runs.
            request_microphone();
        }
    }

    fn request_microphone() {
        let cls = match AnyClass::get(c"AVCaptureDevice") {
            Some(c) => c,
            None => {
                eprintln!("[perms] Microphone: AVCaptureDevice class missing — cannot request");
                return;
            }
        };
        // AVMediaTypeAudio == "soun"
        let media: &NSString = ns_string!("soun");
        // AVAuthorizationStatus: 0 notDetermined, 1 restricted, 2 denied, 3 authorized
        let status: isize = unsafe { msg_send![cls, authorizationStatusForMediaType: media] };
        match status {
            3 => eprintln!("[perms] Microphone: granted"),
            0 => {
                eprintln!("[perms] Microphone: not determined — prompting");
                let handler = StackBlock::new(|granted: Bool| {
                    if granted.as_bool() {
                        eprintln!("[perms] Microphone: user granted");
                    } else {
                        eprintln!("[perms] Microphone: user DENIED — dictation will hear silence");
                    }
                })
                .copy();
                let _: () = unsafe {
                    msg_send![cls, requestAccessForMediaType: media, completionHandler: &*handler]
                };
            }
            _ => eprintln!(
                "[perms] Microphone: DENIED/restricted — enable RootVox in System Settings → \
                 Privacy & Security → Microphone, then relaunch"
            ),
        }
    }
}
