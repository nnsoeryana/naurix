//! Clipboard access for the terminal context menu. macOS only (`pbpaste` / `pbcopy`), invoked with
//! fixed arguments (no shell). Only called from explicit user actions (Copy / Paste menu items);
//! the clipboard is never read in the background or logged.

/// Same bound as terminal input (`MAX_INPUT_BYTES`), so a paste can never be silently truncated.
pub const MAX_CLIPBOARD_BYTES: usize = 1 << 20;

pub(crate) fn check_size(len: usize) -> Result<(), String> {
    if len > MAX_CLIPBOARD_BYTES {
        Err(format!(
            "The clipboard is larger than {} MiB, which is too large to paste into a terminal.",
            MAX_CLIPBOARD_BYTES >> 20
        ))
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn clipboard_read() -> Result<String, String> {
    let text = os::read()?;
    check_size(text.len())?;
    Ok(text)
}

#[tauri::command]
pub async fn clipboard_write(text: String) -> Result<(), String> {
    check_size(text.len())?;
    os::write(&text)
}

#[cfg(target_os = "macos")]
mod os {
    use std::io::Write;
    use std::process::{Command, Stdio};

    pub fn read() -> Result<String, String> {
        let out = Command::new("pbpaste")
            .env("LANG", "en_US.UTF-8")
            .output()
            .map_err(|e| format!("Could not read the clipboard: {e}"))?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn write(text: &str) -> Result<(), String> {
        let mut child = Command::new("pbcopy")
            .env("LANG", "en_US.UTF-8")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not write the clipboard: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
        child.wait().map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Linux and Windows: `arboard`. On X11 the clipboard owner must stay alive to serve pastes, so one
/// handle is kept for the life of the app (it is created lazily, on the first user action).
#[cfg(any(target_os = "linux", windows))]
mod os {
    use std::sync::{Mutex, OnceLock};

    static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

    fn with<T>(
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
    ) -> Result<T, String> {
        let cell = CLIPBOARD.get_or_init(|| Mutex::new(None));
        let mut guard = cell.lock().map_err(|_| "clipboard is unavailable")?;
        if guard.is_none() {
            *guard = Some(
                arboard::Clipboard::new()
                    .map_err(|e| format!("Could not open the clipboard: {e}"))?,
            );
        }
        f(guard.as_mut().expect("just created")).map_err(|e| format!("Clipboard error: {e}"))
    }

    pub fn read() -> Result<String, String> {
        match with(|c| c.get_text()) {
            // An empty or non-text clipboard pastes nothing.
            Err(e) if e.contains("not available in the requested format") => Ok(String::new()),
            other => other,
        }
    }

    pub fn write(text: &str) -> Result<(), String> {
        with(|c| c.set_text(text.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_limit_is_enforced_at_the_boundary() {
        assert!(check_size(0).is_ok());
        assert!(check_size(MAX_CLIPBOARD_BYTES).is_ok());
        assert!(check_size(MAX_CLIPBOARD_BYTES + 1).is_err());
    }
}
