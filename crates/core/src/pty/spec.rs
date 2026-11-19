use super::PtyError;
use crate::platform;
use std::path::PathBuf;

pub const MAX_DIMENSION: u16 = 1000;
pub const MAX_INPUT_BYTES: usize = 1 << 20; // 1 MiB per write call

/// A request to start a process on a PTY. Executable and arguments are separate; nothing is
/// interpreted by a shell unless the caller explicitly chooses a shell as the executable.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Additions to the inherited environment. Values are never logged.
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

impl SpawnSpec {
    pub(crate) fn validate(&self) -> Result<PathBuf, PtyError> {
        validate_size(self.rows, self.cols)?;
        if self.program.is_empty() || self.program.contains('\0') {
            return Err(PtyError::Invalid("program is empty or contains NUL".into()));
        }
        if self.args.iter().any(|a| a.contains('\0')) {
            return Err(PtyError::Invalid("argument contains NUL".into()));
        }
        for (k, v) in &self.env {
            if k.is_empty() || k.contains('=') || k.contains('\0') || v.contains('\0') {
                return Err(PtyError::Invalid(format!(
                    "invalid environment entry '{}'",
                    k.replace('\0', "")
                )));
            }
        }
        if !self.cwd.is_dir() {
            return Err(PtyError::BadWorkingDirectory(
                self.cwd.display().to_string(),
            ));
        }
        resolve_executable(&self.program)
    }
}

pub(crate) fn validate_size(rows: u16, cols: u16) -> Result<(), PtyError> {
    if rows == 0 || cols == 0 || rows > MAX_DIMENSION || cols > MAX_DIMENSION {
        return Err(PtyError::Invalid(format!(
            "terminal size {cols}x{rows} out of range 1..={MAX_DIMENSION}"
        )));
    }
    Ok(())
}

/// Resolve `program` to an executable path. Names containing a path separator are used as-is; bare
/// names are searched on the current process's `PATH` (with `PATHEXT` extensions on Windows).
pub fn resolve_executable(program: &str) -> Result<PathBuf, PtyError> {
    if platform::has_separator(program) {
        let p = PathBuf::from(program);
        return if platform::is_executable_file(&p) {
            Ok(p)
        } else {
            Err(PtyError::ExecutableNotFound(program.into()))
        };
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let names = platform::command_candidates(program);
    std::env::split_paths(&path)
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|c| platform::is_executable_file(c))
        .ok_or_else(|| PtyError::ExecutableNotFound(program.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shell() -> &'static str {
        if cfg!(windows) {
            "cmd.exe"
        } else {
            "/bin/sh"
        }
    }

    fn spec() -> SpawnSpec {
        SpawnSpec {
            program: test_shell().into(),
            args: vec![],
            cwd: std::env::temp_dir(),
            env: vec![],
            rows: 24,
            cols: 80,
        }
    }

    #[test]
    fn accepts_valid_spec() {
        assert!(spec().validate().is_ok());
    }

    #[test]
    fn rejects_bad_inputs() {
        let mut s = spec();
        s.rows = 0;
        assert!(matches!(s.validate(), Err(PtyError::Invalid(_))));
        let mut s = spec();
        s.cols = MAX_DIMENSION + 1;
        assert!(matches!(s.validate(), Err(PtyError::Invalid(_))));
        let mut s = spec();
        s.program = "".into();
        assert!(matches!(s.validate(), Err(PtyError::Invalid(_))));
        let mut s = spec();
        s.args = vec!["a\0b".into()];
        assert!(matches!(s.validate(), Err(PtyError::Invalid(_))));
        let mut s = spec();
        s.env = vec![("A=B".into(), "x".into())];
        assert!(matches!(s.validate(), Err(PtyError::Invalid(_))));
        let mut s = spec();
        s.cwd = "/definitely/not/here".into();
        assert!(matches!(
            s.validate(),
            Err(PtyError::BadWorkingDirectory(_))
        ));
        let mut s = spec();
        s.program = "/definitely/not/here".into();
        assert!(matches!(s.validate(), Err(PtyError::ExecutableNotFound(_))));
    }

    #[test]
    fn resolves_bare_names_on_path() {
        assert!(resolve_executable(if cfg!(windows) { "cmd" } else { "sh" }).is_ok());
        assert!(resolve_executable("definitely-not-a-real-binary-xyz").is_err());
        // a directory is not an executable file
        assert!(resolve_executable(if cfg!(windows) {
            "C:\\Windows"
        } else {
            "/usr/bin"
        })
        .is_err());
    }
}
