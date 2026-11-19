//! Clipboard access for the terminal context menu. macOS only (`pbpaste` / `pbcopy`), invoked with
//! fixed arguments (no shell). Only called from explicit user actions (Copy / Paste menu items);
//! the clipboard is never read in the background or logged.

use std::io::Write;
use std::process::{Command, Stdio};

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
    let out = Command::new("pbpaste")
        .env("LANG", "en_US.UTF-8")
        .output()
        .map_err(|e| format!("Could not read the clipboard: {e}"))?;
    check_size(out.stdout.len())?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[tauri::command]
pub async fn clipboard_write(text: String) -> Result<(), String> {
    check_size(text.len())?;
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
