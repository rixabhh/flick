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
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn get() -> Option<ActiveTarget> {
    let window = active_win_pos_rs::get_active_window().ok()?;
    Some(ActiveTarget {
        app_name: window.app_name.to_ascii_lowercase(),
        title: window.title.to_ascii_lowercase(),
        process_path: window.process_path.to_string_lossy().to_ascii_lowercase(),
    })
}

#[cfg(target_os = "macos")]
pub fn get() -> Option<ActiveTarget> {
    use std::process::Command;

    // NSWorkspace gives us the frontmost bundle without requesting screen
    // capture or inspecting any UI content. A tab separator keeps the
    // localized name and bundle path unambiguous for normal macOS paths.
    let script = r#"
        ObjC.import('AppKit');
        const app = $.NSWorkspace.sharedWorkspace.frontmostApplication;
        [app.localizedName.js, app.bundleURL.path.js].join('\t');
    "#;
    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let (app_name, process_path) = value.trim().split_once('\t')?;
    (!app_name.trim().is_empty()).then(|| ActiveTarget {
        app_name: app_name.trim().to_ascii_lowercase(),
        title: String::new(),
        process_path: process_path.trim().to_ascii_lowercase(),
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn get() -> Option<ActiveTarget> {
    None
}

#[cfg(test)]
mod tests {
    use super::ActiveTarget;

    #[test]
    fn target_identity_is_content_free() {
        let target = ActiveTarget::default();
        assert!(target.app_name.is_empty());
        assert!(target.title.is_empty());
        assert!(target.process_path.is_empty());
    }
}
