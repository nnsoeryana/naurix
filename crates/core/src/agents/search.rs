use crate::platform;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Directories to search for agent CLIs: the process `PATH` first, then common install locations.
///
/// A GUI app launched from Finder inherits only a minimal `PATH` (no Homebrew, nvm, `~/.local/bin`),
/// so CLIs installed there would never be found by `PATH` alone. We deliberately do not run the
/// user's login shell to discover `PATH`: shell startup can take many seconds and executes user
/// configuration.
pub fn search_dirs(process_path: Option<&OsStr>, home: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = process_path
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    for extra in platform::extra_bin_dirs().iter().copied().chain([
        ".local/bin",
        ".claude/local",
        ".opencode/bin",
        ".bun/bin",
        ".npm-global/bin",
        ".cargo/bin",
        ".volta/bin",
    ]) {
        dirs.push(if platform::is_absolute_str(extra) {
            PathBuf::from(extra)
        } else {
            home.join(extra)
        });
    }
    // nvm: every installed Node version's global bin dir (newest listed last in the glob; order is irrelevant for lookup).
    if let Ok(rd) = std::fs::read_dir(home.join(".nvm/versions/node")) {
        let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("bin")).collect();
        v.sort();
        v.reverse(); // prefer the newest version
        dirs.extend(v);
    }
    let mut seen = std::collections::HashSet::new();
    dirs.into_iter()
        .filter(|d| d.is_absolute() && d.is_dir() && seen.insert(d.clone()))
        .collect()
}

/// First executable file called `name` in `dirs`.
pub fn find_executable(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if name.is_empty() || platform::has_separator(name) {
        return None;
    }
    let names = platform::command_candidates(name);
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| platform::is_executable_file(p))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("at-search-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_exec(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn finds_in_extra_dirs_when_process_path_is_minimal() {
        let home = tmp("home");
        let bin = home.join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = make_exec(&bin, "fake-agent");
        let dirs = search_dirs(Some(OsStr::new("/usr/bin:/bin")), &home);
        assert_eq!(find_executable("fake-agent", &dirs), Some(exe));
        assert_eq!(find_executable("nope-agent", &dirs), None);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn finds_nvm_global_bins_newest_first() {
        let home = tmp("nvm");
        for v in ["v18.1.0", "v22.2.0"] {
            let b = home.join(".nvm/versions/node").join(v).join("bin");
            std::fs::create_dir_all(&b).unwrap();
            make_exec(&b, "nvm-agent");
        }
        let dirs = search_dirs(None, &home);
        assert!(find_executable("nvm-agent", &dirs)
            .unwrap()
            .to_string_lossy()
            .contains("v22.2.0"));
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn process_path_wins_and_dirs_are_deduplicated() {
        let home = tmp("dedupe");
        let first = home.join("first");
        std::fs::create_dir_all(&first).unwrap();
        let exe = make_exec(&first, "dup-agent");
        let joined = std::env::join_paths([&first, &first]).unwrap();
        let dirs = search_dirs(Some(&joined), &home);
        assert_eq!(dirs.iter().filter(|d| **d == first).count(), 1);
        assert_eq!(find_executable("dup-agent", &dirs), Some(exe));
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn ignores_non_executables_directories_and_paths() {
        let home = tmp("noexec");
        let bin = home.join("bin");
        std::fs::create_dir_all(bin.join("adir")).unwrap();
        std::fs::write(bin.join("plain"), "x").unwrap();
        let dirs = vec![bin];
        assert_eq!(find_executable("plain", &dirs), None);
        assert_eq!(find_executable("adir", &dirs), None);
        assert_eq!(find_executable("../etc/passwd", &dirs), None);
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn finds_cmd_shims_without_the_extension() {
        let d = std::env::temp_dir().join(format!("at-search-win-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let shim = d.join("fake-agent.cmd");
        std::fs::write(&shim, "@echo off\r\n").unwrap();
        std::fs::write(d.join("plain"), "x").unwrap();
        let dirs = vec![d.clone()];
        assert_eq!(find_executable("fake-agent", &dirs), Some(shim));
        assert_eq!(find_executable("plain", &dirs), None);
        std::fs::remove_dir_all(d).unwrap();
    }
}
