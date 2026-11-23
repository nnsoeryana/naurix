//! Framework-independent core (ADR 0002). Must not depend on Tauri.

pub mod agents;
pub mod approval;
pub mod credential;
pub mod events;
pub mod git;
pub mod mcp;
pub mod persistence;
pub mod platform;
pub mod pty;
pub mod session;
pub mod settings;
pub mod skills;

/// Static information about the running build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub version: &'static str,
    pub arch: &'static str,
    pub os: &'static str,
}

pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        arch: std::env::consts::ARCH,
        os: std::env::consts::OS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_host_arch_and_os() {
        let info = app_info();
        assert_eq!(info.arch, std::env::consts::ARCH);
        assert_eq!(info.os, std::env::consts::OS);
    }
}
