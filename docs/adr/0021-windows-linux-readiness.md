# ADR 0021 — Windows and Linux readiness
**Status:** Accepted. Implemented 2026-11-19 on a macOS machine only; Linux and Windows are **not validated** (see §4).

**Context:** The maintainer wants to prepare Windows and Linux builds. ADR 0001 limits support to macOS x86_64/arm64. The `core` crate is framework-independent (ADR 0002), so a port is mostly confined to a few seams. This ADR records an audit of the macOS/Unix assumptions in the repository (2026-11-19) and a staged plan. **Nothing here is a support claim:** per ADR 0001 and AGENTS.md, a platform "works" only after a recorded run on real hardware of that OS.

**Decision:** Port in the order **Linux → Windows**, as milestones M12 (Linux) and M13 (Windows), after M11a. macOS stays the only supported platform until each milestone's acceptance evidence exists. Linux first because it shares the Unix PTY/process code, `ps`, symlinks and permission bits; Windows needs the PTY lifecycle redesigned.

## 1. Audit: what is platform-specific today

| Area | Where | Problem | Linux | Windows |
|---|---|---|---|---|
| Secret store | `crates/core/src/mcp/secrets.rs` (`security-framework`, non-macOS branch returns an error) | Keychain only | Add Secret Service (libsecret / D-Bus) backend; handle "no keyring daemon running" with an actionable error | Add Credential Manager backend |
| Clipboard | `src-tauri/src/clipboard.rs` | Shells out to `pbpaste` / `pbcopy` | Replace with a clipboard path that is not a subprocess (candidate: `tauri-plugin-clipboard-manager`, or the webview Clipboard API on user gesture; check license, maintenance, capability scope per AGENTS.md); Wayland vs X11 must be tested | Same replacement |
| PTY process lifecycle | `crates/core/src/pty/session.rs` (`libc::killpg`, SIGHUP/SIGTERM/SIGKILL ladder, `poll`/`dup` on the master fd) | Unix-only, `libc` is an unconditional dependency | Works as is, but confirm `portable-pty` behaviour and Spike D (no orphans) on Linux | **Does not build.** Needs a ConPTY path (`portable-pty` supports it) and a Job Object to kill the process tree; the SIGHUP ladder has no equivalent |
| Executable lookup | `pty/spec.rs`, `agents/search.rs`, `agents/def.rs`, `agents/store.rs`, `agents/version.rs` (`PermissionsExt`, `contains('/')`, `:`-joined `PATH`, Homebrew dirs) | Unix permission bits and path syntax | Drop the Homebrew dirs, keep `~/.local/bin` etc.; Linux GUI apps usually inherit a fuller `PATH`, but verify per launcher | Use `PATHEXT`, `.exe`/`.cmd`/`.ps1` shims (npm-installed agent CLIs are `.cmd` shims), `\` and drive letters; extra dirs differ (`%APPDATA%\npm`, `%LOCALAPPDATA%`) |
| Default shell | `crates/core/src/settings.rs` (`/bin/zsh` fallback, `-l`), `src-tauri/src/terminal.rs` (`$SHELL`) | macOS default | Fallback `/bin/bash` then `/bin/sh`; `$SHELL` still valid | No `$SHELL`; default to `pwsh.exe` / `powershell.exe` / `cmd.exe`; `-l` does not apply; settings validation requires an absolute Unix-style path |
| Home directory | `src-tauri/src/{agent,mcp,skills}.rs`, `git/run.rs` (`HOME`) | `HOME` is unset on Windows | Fine | Use `USERPROFILE` (or the `dirs` crate after a dependency check) |
| Skill links | `skills/store.rs` (`std::os::unix::fs::symlink`), ADR 0016 | Reversible symlink is the safety model | Works | Creating symlinks needs Developer Mode or elevation. Options to decide in a new ADR: require Developer Mode, use directory junctions (no privilege, local volumes only), or copy with a manifest. Junctions change the "never overwrite / reversible" proof, so tests must be rewritten |
| Executable-bit tests and scanners | `skills/scan.rs`, `mcp/probe.rs` (`CommandExt`), tests using `PermissionsExt` and `#!/bin/sh` mocks | Unix-only | Fine | Tests need `#[cfg(unix)]` gating plus Windows equivalents (batch / PowerShell mocks) |
| Git | `skills/git.rs` (`GIT_ASKPASS=/usr/bin/false`), `git/run.rs` | Hard-coded Unix path | Fine | Use a platform-appropriate no-op askpass; check hardening flags behave the same with Git for Windows |
| Perf snapshot | `src-tauri/src/settings.rs` (`/bin/ps`) | macOS/BSD `ps` flags | `ps -o rss= -p` works on procps; or read `/proc/self/statm` | Needs a Windows API / sysinfo-style crate |
| Window chrome | `tauri.conf.json` (`titleBarStyle: Overlay`, `hiddenTitle`), `Sidebar.tsx` (strip under traffic lights), capability `allow-start-dragging` | macOS overlay title bar | Decide: native decorations, or custom window controls | Same |
| Bundle config | `tauri.conf.json` (`targets: app, dmg`, `.icns`, `macOS.minimumSystemVersion`), `run.sh` (Darwin check, Xcode tools, `.app` path) | macOS only | Add `deb`, `rpm`, `appimage`; list webkit2gtk 4.1 and libsecret build/runtime deps; `run.sh doctor` per OS | Add `nsis`/`msi`, `.ico` exists already; WebView2 runtime; `run.sh` is bash, so provide a PowerShell or Node script |
| Webview | WKWebView assumed (`PERFORMANCE_AND_COMPATIBILITY.md`, xterm.js WebGL decisions) | Rendering and memory differ | WebKitGTK (known slower; WebGL support varies) | WebView2 (Chromium) |
| Keyboard shortcuts and labels | `src/shortcuts.ts` (`metaKey`, `⌘` labels), `ContextMenu.tsx` (`⌘C`/`⌘V`) | Cmd-based; **`Ctrl+C`/`Ctrl+W`/`Ctrl+D` collide with terminal control keys** | Use `Ctrl+Shift+…` (terminal convention) | Same; decide copy/paste (`Ctrl+Shift+C/V`) vs shell semantics |
| User-facing text | `SettingsView.tsx`, `McpManager.tsx`, `mcp/Dialogs.tsx`, `AppearanceSettings.tsx`, `workspaces/path.ts` (`/Users`) | Mentions Keychain, "macOS Appearance", `/Users/…` home shortening | Make store name and OS wording come from the backend; make `~` shortening use the real home path | Same, plus `\` separators |
| Agent config paths | `agents/adapter.rs`, `mcp/sync.rs` (`~/.config/opencode`, `~/.claude`, …) | Verified only on macOS | Re-verify each agent's documented path on Linux (XDG rules) | Re-verify; most agents use different locations on Windows. AGENTS.md: do not assume, verify against current official docs |
| Data dir / SQLite | Tauri app-data dir | Resolved by Tauri | OK | OK |
| CI | none committed (`.github` is absent although the plan mentions CI) | No cross-OS evidence | `ubuntu-22.04`/`24.04` runners | `windows-latest` |
| Docs | ADR 0001, 0004, AGENTS.md invariant "secrets only in macOS Keychain", PERFORMANCE_AND_COMPATIBILITY §1, §5, §6 | macOS wording | Update when milestones land | Same |

Already portable: SQLite (bundled `rusqlite`), `portable-pty` for spawning, `serde`, `sha2`, the credential heuristic, theme and layout logic, and the React/xterm.js layer apart from the items above.

## 2. Milestones

| M | Scope | Acceptance (evidence required) |
|---|---|---|
| **M12a** | Seams, no behaviour change on macOS: `SecretStore` and clipboard behind traits/`cfg`; gate `libc`/`PermissionsExt`/`symlink` with `cfg(unix)`; move OS wording and shortcut modifier to a backend/`platform` value; per-OS shell default | `cargo test` and frontend tests still pass on macOS; `cargo check` for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc` compiles `core` (compile-only, not a support claim) |
| **M12b** | Linux: Secret Service backend, clipboard, shell default, window chrome decision, bundle targets, `run.sh doctor` | CI on Ubuntu runs `cargo test`, `clippy`, frontend tests; manual checklist (launch, shell, resize, paste, process cleanup, secret store with and without a keyring daemon) recorded on real hardware; benchmarks recorded as `bench/results/<date>-linux-<arch>-…` |
| **M13a** | Windows PTY: ConPTY via `portable-pty`, Job Object cleanup, executable lookup with `PATHEXT`/shims, shell default, `USERPROFILE` | Spike D equivalent report on Windows (no orphans after tab close and app exit) |
| **M13b** | Windows: Credential Manager, skill link strategy (new ADR), git askpass, perf snapshot, NSIS/MSI bundle, build script | CI on `windows-latest`; manual checklist and benchmarks recorded on real hardware |
| **M13c** | Signing (Windows code signing certificate, Linux package signing) | Deferred, like M11b; blocked on certificates |

## 3. Decisions taken in the implementation
1. Window chrome: macOS keeps the overlay title bar; Linux and Windows use the native title bar (the Tauri `titleBarStyle`/`hiddenTitle` keys are macOS-only, and the sidebar spacer is macOS-only). The custom menu exists only on macOS.
2. Windows skill links: still real directory symlinks (`platform::symlink_dir`); without Developer Mode the link fails with a message that says so and nothing is changed. Junctions or copies remain an option if this proves too restrictive. `current` is replaced in two steps on Windows because a directory symlink cannot be renamed over.
3. Clipboard: `arboard` (MIT/Apache, Wayland data-control enabled) on Linux and Windows; macOS keeps `pbcopy`/`pbpaste`. Same commands, same size limit, only on explicit user action.
4. Secrets: `keyring` 3 (Secret Service via `dbus-secret-service` with vendored libdbus; Windows Credential Manager); macOS keeps `security-framework`.
5. Windows process cleanup: `taskkill /T /F` on the process tree instead of a Job Object (no new dependency). It cannot catch processes that detach from the tree; a Job Object is the follow-up if Spike D on Windows shows leaks. There is no signal ladder, and closing never asks about a foreground job.
6. Windows PTY reads go through a helper thread because ConPTY reads cannot be polled.
7. Shortcuts: Ctrl+Shift (and Ctrl+Alt for pane/tab-number variants) on Linux and Windows; Ctrl+Shift+C/V copy and paste. Ctrl+Alt combinations can collide with AltGr on some keyboard layouts.
8. Shell default: `$SHELL` on Linux (fallback `/bin/bash`, then `/bin/sh`), `%COMSPEC%` on Windows (fallback `cmd.exe`, no `-l`).
9. Windows executable lookup honours `PATHEXT` for `.exe/.com/.bat/.cmd` (npm shims). `.ps1` is not directly spawnable and is not offered.

## 4. Not done / unverified
- Nothing was compiled or run for Linux or Windows here (no cross toolchain). Only `platform.rs` was type-checked for `x86_64-pc-windows-gnu`; the rest relies on the new CI matrix (`.github/workflows/ci.yml`, macOS 14, Ubuntu 24.04, Windows latest), which has not run yet.
- Unix-only integration tests (`pty.rs`, `agents.rs`, `skills.rs`, `adapters.rs`, `mcp.rs`, hostile-repo git tests, `skills/scan.rs`, `agents/version.rs`) are gated with `cfg(unix)`. Windows has only `tests/pty_windows.rs` (ConPTY run, exit code, terminate) plus unit tests; MCP probe, skill links and git hardening have no Windows tests yet.
- Agent config paths (`~/.config/opencode`, `~/.claude`, …) are verified on macOS only. Windows locations in particular must be checked against each agent's current docs before MCP/skill sync is trusted there.
- Git hardening uses `core.hooksPath=/dev/null`; behaviour with Git for Windows is unverified. On Windows `GIT_ASKPASS` is not set (prompts are still disabled by `GIT_TERMINAL_PROMPT=0`, a null stdin and a cleared credential helper).
- Windows: no memory reading in Settings, no signing, WebView2 is fetched by the installer bootstrapper. Linux: WebKitGTK performance and WebGL availability are unmeasured; no signing.
- No benchmarks, no manual checklist, no hardware runs on either OS.

## 5. Remaining open decisions
1. Linux distribution formats to ship and the minimum distro/WebKitGTK version.
2. Hardware for recorded runs (Linux x86_64/arm64, Windows x86_64/arm64).
3. Whether to move Windows to a Job Object and/or junction-based skill links after the first real runs.
1. Window chrome on Windows/Linux: native decorations or a custom title bar.
2. Windows skill links: Developer Mode requirement, junctions, or copy-with-manifest (ADR 0016 consequences).
3. Clipboard: new Tauri plugin versus webview API (dependency review required).
4. Linux distribution formats to ship (AppImage, deb, rpm) and minimum distro/WebKitGTK version.
5. Hardware available for recorded runs (Linux x86_64/arm64, Windows x86_64/arm64). Without it no platform claim can be made.
6. Whether ADR 0004 / the AGENTS.md invariant is reworded to "the OS credential store" (macOS Keychain, Secret Service, Credential Manager). Note ADR 0019 already says Naurix stores no credentials, which limits the secret-store work to what the MCP manager still uses.

**Consequences:** macOS behaviour is unchanged (all existing tests pass). ADR 0001 is superseded in part, README and `IMPLEMENTATION_PLAN.md` carry M12/M13, and the compatibility matrix gains Linux and Windows rows that stay empty until recorded runs exist. Cross-compilation or a green CI job alone never counts as "works".
