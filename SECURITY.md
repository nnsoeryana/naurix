# Security Policy

Report vulnerabilities privately through GitHub Security Advisories on this repository (enabled when the repo is published); do not open public issues for security problems. Expect acknowledgement within 7 days.

Security model summary: Rust owns process and filesystem access; least-privilege Tauri capabilities; executable + argument arrays only; secrets only in macOS Keychain; MCP servers and skills are untrusted until the user approves them, and skills are never executed by the app. Details: `docs/TECHNICAL_DESIGN.md` §13.
