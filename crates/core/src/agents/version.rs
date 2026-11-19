use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Ask an executable for its version (`<program> --version`, argument array, no shell, no stdin).
///
/// This **executes** the program, so callers must only use it for executables the user has already
/// reviewed and approved. Returns the first non-empty output line (max 200 chars), or `None` on
/// failure or timeout (the process is killed on timeout).
pub fn detect_version(program: &Path, timeout: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.by_ref().take(64 * 1024).read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait().ok()? {
            Some(st) => break st,
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let text = reader.join().ok()?;
    if !status.success() {
        return None;
    }
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(200).collect())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(tag: &str, body: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("at-ver-{tag}-{}", std::process::id()));
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn reads_first_nonempty_line() {
        let p = script(
            "ok",
            "#!/bin/sh\necho\necho 'tool 1.2.3 (build x)'\necho more\n",
        );
        assert_eq!(
            detect_version(&p, Duration::from_secs(5)).as_deref(),
            Some("tool 1.2.3 (build x)")
        );
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn failure_and_missing_binary_give_none() {
        let p = script("fail", "#!/bin/sh\necho oops\nexit 3\n");
        assert_eq!(detect_version(&p, Duration::from_secs(5)), None);
        assert_eq!(
            detect_version(Path::new("/no/such/binary"), Duration::from_secs(1)),
            None
        );
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn a_hanging_program_is_killed_at_the_timeout() {
        let p = script("hang", "#!/bin/sh\nsleep 30\n");
        let t = Instant::now();
        assert_eq!(detect_version(&p, Duration::from_millis(300)), None);
        assert!(
            t.elapsed() < Duration::from_secs(3),
            "returned after {:?}",
            t.elapsed()
        );
        std::fs::remove_file(p).unwrap();
    }
}
