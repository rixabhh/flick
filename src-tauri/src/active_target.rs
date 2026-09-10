//! Minimal cross-platform foreground-application metadata.
//!
//! This module deliberately exposes app identity only. It never reads field
//! values, window contents, or accessibility trees; callers use it to refuse
//! unsafe paste transactions when focus changes.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveTarget {
    pub app_name: String,
    pub title: String,
    pub process_path: String,
    pub process_id: u64,
    pub window_id: String,
}

/// App names alone cannot distinguish two windows of the same browser. When
/// the OS supplies a process/window identity, require it to survive unchanged.
/// A failed follow-up identity lookup must never relax an earlier guard.
pub fn matches_target(expected: &ActiveTarget, current: &ActiveTarget) -> bool {
    !expected.app_name.is_empty()
        && expected.app_name == current.app_name
        && (expected.process_path.is_empty() || expected.process_path == current.process_path)
        && (expected.process_id == 0 || expected.process_id == current.process_id)
        && (expected.window_id.is_empty() || expected.window_id == current.window_id)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn get() -> Option<ActiveTarget> {
    let window = active_win_pos_rs::get_active_window().ok()?;
    Some(ActiveTarget {
        app_name: window.app_name.to_ascii_lowercase(),
        title: window.title.to_ascii_lowercase(),
        process_path: window.process_path.to_string_lossy().to_ascii_lowercase(),
        process_id: window.process_id,
        window_id: window.window_id,
    })
}

#[cfg(target_os = "macos")]
pub fn get() -> Option<ActiveTarget> {
    // Query the native API directly. Starting an AppleScript process for each
    // protection/target check added avoidable latency before every recording.
    // No screen capture, field contents or Apple Events permission is needed.
    objc2::rc::autoreleasepool(|_| {
        let app = objc2_app_kit::NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let app_name = app.localizedName()?.to_string().to_ascii_lowercase();
        let process_path = app.bundleURL()?.path()?.to_string().to_ascii_lowercase();
        let process_id = u64::try_from(app.processIdentifier()).ok()?;
        (!app_name.is_empty()).then(|| ActiveTarget {
            app_name,
            title: String::new(),
            process_path,
            process_id,
            window_id: String::new(),
        })
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn get() -> Option<ActiveTarget> {
    None
}

#[cfg(test)]
mod tests {
    use super::{matches_target, ActiveTarget};

    #[test]
    fn target_identity_is_content_free() {
        let target = ActiveTarget::default();
        assert!(target.app_name.is_empty());
        assert!(target.title.is_empty());
        assert!(target.process_path.is_empty());
    }

    #[test]
    fn target_guard_distinguishes_windows_and_fails_on_missing_identity() {
        let original = ActiveTarget {
            app_name: "browser".into(),
            process_path: "/apps/browser".into(),
            process_id: 42,
            window_id: "window-a".into(),
            ..ActiveTarget::default()
        };
        assert!(matches_target(&original, &original));
        let mut changed = original.clone();
        changed.window_id = "window-b".into();
        assert!(!matches_target(&original, &changed));
        changed = original.clone();
        changed.process_path.clear();
        assert!(!matches_target(&original, &changed));
        changed = original.clone();
        changed.process_id = 0;
        assert!(!matches_target(&original, &changed));
    }
}
