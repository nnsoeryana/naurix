//! The few places where Naurix differs between macOS, Linux and Windows (ADR 0021).
//!
//! Everything else in `core` is portable. Keep OS checks here so the rest of the code stays free of
//! `cfg` noise.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The user's home directory: `HOME` on Unix, `USERPROFILE` on Windows (with `HOME` as a fallback,
/// which Git Bash and similar environments set).
pub fn home_dir() -> Option<PathBuf> {
    let var = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty());
    #[cfg(windows)]
    let found = var("USERPROFILE").or_else(|| var("HOME"));
    #[cfg(not(windows))]
    let found = var("HOME");
    found.map(PathBuf::from)
}

/// Like [`home_dir`] but never fails: an unset home yields an empty path, which matches nothing.
pub fn home_dir_or_empty() -> PathBuf {
    home_dir().unwrap_or_default()
}

/// True if `s` is an absolute path on this OS (`/x` on Unix, `C:\x` or `\\server\x` on Windows).
pub fn is_absolute_str(s: &str) -> bool {
    !s.contains('\0') && Path::new(s).is_absolute()
}

/// Extensions Windows treats as runnable, from `PATHEXT` (lower-case, with the leading dot).
#[cfg(windows)]
fn path_exts() -> Vec<String> {
    let raw = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    raw.split(';')
        .filter(|e| e.starts_with('.') && e.len() > 1)
        // `.ps1`, `.vbs`, `.js` etc. are not directly spawnable; only keep what CreateProcess runs.
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| matches!(e.as_str(), ".com" | ".exe" | ".bat" | ".cmd"))
        .collect()
}

/// A regular file the OS would run: any executable bit on Unix; a runnable extension on Windows.
pub fn is_executable_file(p: &Path) -> bool {
    let Ok(m) = p.metadata() else { return false };
    if !m.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_ascii_lowercase()))
            .is_some_and(|e| path_exts().contains(&e))
    }
}

/// Candidate file names for a bare command name: `name` itself on Unix; on Windows also
/// `name.exe`, `name.cmd`, … unless the name already has an extension.
pub fn command_candidates(name: &str) -> Vec<OsString> {
    #[cfg(windows)]
    {
        if Path::new(name).extension().is_some() {
            return vec![name.into()];
        }
        let mut v: Vec<OsString> = path_exts()
            .into_iter()
            .map(|e| format!("{name}{e}").into())
            .collect();
        v.push(name.into());
        v
    }
    #[cfg(not(windows))]
    {
        vec![name.into()]
    }
}

/// True if a command name contains a path separator (so it must not be searched on `PATH`).
pub fn has_separator(name: &str) -> bool {
    name.contains('/') || (cfg!(windows) && name.contains('\\'))
}

/// Install locations of common agent CLIs that a GUI launch does not have on `PATH`.
/// Entries starting with a separator are absolute; the rest are relative to the home directory.
pub fn extra_bin_dirs() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &["/opt/homebrew/bin", "/usr/local/bin"]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "/usr/local/bin",
            "/usr/bin",
            "/snap/bin",
            "/home/linuxbrew/.linuxbrew/bin",
        ]
    }
    #[cfg(windows)]
    {
        // npm's global prefix on Windows is %APPDATA%\npm.
        &["AppData/Roaming/npm", "AppData/Local/Programs/claude"]
    }
}

/// Default shell when neither the setting nor `$SHELL` names a usable one, plus its login flag.
pub fn fallback_shell() -> (&'static str, &'static [&'static str]) {
    #[cfg(target_os = "macos")]
    {
        ("/bin/zsh", &["-l"])
    }
    #[cfg(target_os = "linux")]
    {
        // bash is the usual default; `sh` always exists.
        if Path::new("/bin/bash").is_file() {
            ("/bin/bash", &["-l"])
        } else {
            ("/bin/sh", &["-l"])
        }
    }
    #[cfg(windows)]
    {
        ("cmd.exe", &[])
    }
}

/// Human-readable name of the OS credential store Naurix uses for secrets.
pub fn secret_store_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macOS Keychain"
    }
    #[cfg(target_os = "linux")]
    {
        "Secret Service (GNOME Keyring / KWallet)"
    }
    #[cfg(windows)]
    {
        "Windows Credential Manager"
    }
}

/// Resident memory of a process in KiB. macOS: `ps`; Linux: `/proc/<pid>/statm`; Windows: `None`
/// (no cheap reading without extra dependencies).
pub fn resident_kb(pid: u32) -> Option<f64> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/bin/ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
        let pages: f64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        // Page size is 4 KiB on x86_64 but 16/64 KiB on some arm64 kernels.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        Some(pages * if page > 0 { page as f64 } else { 4096.0 } / 1024.0)
    }
    #[cfg(windows)]
    {
        let _ = pid;
        None
    }
}

/// Whether the shell flag for a login shell applies on this OS.
pub fn supports_login_flag() -> bool {
    cfg!(unix)
}

/// Create a directory symlink `link` -> `target`. On Windows this needs Developer Mode (or an
/// elevated process); the error then says so instead of showing a raw OS code.
pub fn symlink_dir(target: &Path, link: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).map_err(|e| e.to_string())
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).map_err(|e| {
            if e.raw_os_error() == Some(1314) {
                "Windows needs Developer Mode (Settings > System > For developers) to create the link. Nothing was changed.".to_string()
            } else {
                e.to_string()
            }
        })
    }
}

/// Remove a symlink created by [`symlink_dir`] without touching its target.
pub fn remove_symlink(link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::remove_file(link)
    }
    #[cfg(windows)]
    {
        // A directory symlink is removed like a directory on Windows (this never follows it).
        std::fs::remove_dir(link).or_else(|_| std::fs::remove_file(link))
    }
}

/// Unix permission bits of a file; 0 where the OS has none (Windows).
pub fn file_mode(md: &std::fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        md.permissions().mode()
    }
    #[cfg(not(unix))]
    {
        let _ = md;
        0
    }
}

/// Forcefully end a process and everything it started. Used for helper processes that Naurix
/// spawns without a PTY (MCP test probes).
pub fn kill_process_tree(pid: u32) {
    #[cfg(unix)]
    {
        // The probe starts its child as its own process-group leader.
        unsafe { libc::killpg(pid as i32, libc::SIGKILL) };
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_found_in_a_normal_environment() {
        assert!(home_dir().is_some_and(|h| h.is_absolute()));
    }

    #[test]
    fn absolute_path_detection_follows_the_os() {
        assert!(!is_absolute_str("relative/x"));
        assert!(!is_absolute_str("a\0b"));
        #[cfg(unix)]
        assert!(is_absolute_str("/usr/bin"));
        #[cfg(windows)]
        assert!(is_absolute_str("C:\\Windows"));
    }

    #[test]
    fn separators() {
        assert!(has_separator("a/b"));
        assert!(!has_separator("ab"));
        assert_eq!(has_separator("a\\b"), cfg!(windows));
    }

    #[test]
    fn candidates_always_include_the_name() {
        assert!(command_candidates("tool")
            .iter()
            .any(|c| c.to_str() == Some("tool") || cfg!(windows)));
    }
}
