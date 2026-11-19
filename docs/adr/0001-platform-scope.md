# ADR 0001 — Platform scope: macOS x86_64 + arm64
**Status:** Accepted; the "no Windows/Linux work is planned" clause is superseded by [ADR 0021](0021-windows-linux-readiness.md)
**Context:** Initial docs said "macOS first; cross-platform architecture". The product brief names Intel and Apple Silicon as equal first-class targets. The primary dev machine is Intel (i5-8257U, macOS 14.6); Apple Silicon hardware is held by the maintainer.
**Decision:** Supported targets are `x86_64-apple-darwin` and `aarch64-apple-darwin`; both ship (universal or two builds — decided in M11). Core logic avoids macOS-specific assumptions where free, but no Windows/Linux work is planned. "Works on Intel/arm64" is claimed only after a recorded test run on real hardware of that architecture; cross-compilation alone is not evidence.
**Consequences:** CI needs both `macos-13` (x86_64) and `macos-14` (arm64) runners. Benchmarks are recorded per architecture.
