//! Fetching a skill repository. The only thing this does is `git clone` into a staging folder.
//! Nothing from the repository is ever executed: hooks are disabled, symlinks are not materialised,
//! submodules and non-HTTPS transports are refused, and credential helpers are off.

use super::SkillError;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default)]
pub struct CloneOptions {
    /// Tests only: allow a local repository path instead of an https URL.
    pub allow_local: bool,
}

pub fn validate_url(url: &str, opts: CloneOptions) -> Result<(), SkillError> {
    if opts.allow_local && url.starts_with('/') && Path::new(url).is_dir() {
        return Ok(());
    }
    let bad = |m: &str| Err(SkillError::Invalid(m.into()));
    let Some(rest) = url.strip_prefix("https://") else {
        return bad("Only https:// repository URLs are supported.");
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if url.len() > 500
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
        || authority.is_empty()
        || !authority.contains('.')
    {
        return bad("That does not look like a valid repository URL.");
    }
    if authority.contains('@') {
        return bad("Do not put a username or password in the URL.");
    }
    Ok(())
}

/// Shallow-clone `url` into `dest` (must not exist). Returns the commit hash that was checked out.
pub fn clone_shallow(
    url: &str,
    dest: &Path,
    opts: CloneOptions,
    timeout: Duration,
) -> Result<String, SkillError> {
    validate_url(url, opts)?;
    if dest.exists() {
        return Err(SkillError::Invalid(
            "The staging folder already exists.".into(),
        ));
    }
    let allow = if opts.allow_local {
        "https:file"
    } else {
        "https"
    };
    let mut cmd = Command::new("git");
    cmd.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.symlinks=false",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "credential.helper=",
        "-c",
        "protocol.version=2",
    ]);
    cmd.args([
        "clone",
        "--depth",
        "1",
        "--no-tags",
        "--single-branch",
        "--no-recurse-submodules",
        "--quiet",
        "--",
    ])
    .arg(url)
    .arg(dest);
    cmd.env("GIT_ALLOW_PROTOCOL", allow)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    // Never prompt: stdin is null, GIT_TERMINAL_PROMPT=0 and the credential helper is cleared. Unix
    // also points askpass at `false`; Windows has no equivalent binary at a fixed path.
    #[cfg(unix)]
    cmd.env("GIT_ASKPASS", "/usr/bin/false");
    if opts.allow_local {
        cmd.env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
            .env("GIT_CONFIG_VALUE_0", "always");
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| SkillError::Git(format!("Could not run git: {e}")))?;
    let mut stderr = child.stderr.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.by_ref().take(16 * 1024).read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child
            .try_wait()
            .map_err(|e| SkillError::Git(e.to_string()))?
        {
            Some(st) => break st,
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(dest);
                return Err(SkillError::Git(
                    "The download took too long and was stopped.".into(),
                ));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let err = reader.join().unwrap_or_default();
    if !status.success() {
        let _ = std::fs::remove_dir_all(dest);
        let first = err
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("git failed");
        return Err(SkillError::Git(first.chars().take(300).collect()));
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dest)
        .args(["rev-parse", "HEAD"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .map_err(|e| SkillError::Git(e.to_string()))?;
    let rev = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if rev.len() < 7 {
        return Err(SkillError::Git(
            "Could not read the downloaded revision.".into(),
        ));
    }
    Ok(rev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_https_urls_are_accepted() {
        let o = CloneOptions::default();
        assert!(validate_url("https://github.com/anthropics/skills", o).is_ok());
        for bad in [
            "http://github.com/x/y",
            "git@github.com:x/y.git",
            "ssh://git@github.com/x/y",
            "file:///tmp/x",
            "ext::sh -c id",
            "/tmp/local",
            "https://user:pw@github.com/x/y",
            "https://",
            "https://nodots/x",
            "https://github.com/x y",
            "--upload-pack=evil",
            "https://github.com/x\ny",
        ] {
            assert!(validate_url(bad, o).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn local_paths_need_the_test_only_switch() {
        let d = std::env::temp_dir();
        assert!(validate_url(d.to_str().unwrap(), CloneOptions::default()).is_err());
        assert!(validate_url(d.to_str().unwrap(), CloneOptions { allow_local: true }).is_ok());
    }
}
