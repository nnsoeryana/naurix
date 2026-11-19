//! Test Connection: start an **approved** stdio server, speak MCP over its stdin/stdout
//! (`initialize`, `notifications/initialized`, `tools/list`), then stop it and everything it started.
//! Verified against a real server (`claude mcp serve`, protocol 2025-06-18, newline-delimited JSON-RPC).

use super::model::{McpServer, Transport};
use super::store::McpStore;
use super::McpError;
use crate::agents::find_executable;
use serde::Serialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
const MAX_LINE: usize = 1 << 20;
const MAX_TOOLS: usize = 200;
const MAX_PAGES: usize = 5;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProbeResult {
    pub ok: bool,
    /// `connected`, `failed` or `not_testable`.
    pub status: String,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub protocol: Option<String>,
    pub tools: Vec<ToolInfo>,
    pub error: Option<String>,
}

fn fail(msg: impl Into<String>) -> ProbeResult {
    ProbeResult {
        ok: false,
        status: "failed".into(),
        error: Some(msg.into().chars().take(500).collect()),
        ..Default::default()
    }
}

/// Kill the whole process group (the server and anything it spawned).
struct Guard(std::process::Child);
impl Drop for Guard {
    fn drop(&mut self) {
        crate::platform::kill_process_tree(self.0.id());
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn probe(
    store: &McpStore<'_>,
    server: &McpServer,
    dirs: &[PathBuf],
    timeout: Duration,
) -> Result<ProbeResult, McpError> {
    if server.transport != Transport::Stdio {
        return Ok(ProbeResult {
            status: "not_testable".into(),
            error: Some("Remote (HTTP) servers are tested by the agent that connects to them; Naurix cannot test them yet.".into()),
            ..Default::default()
        });
    }
    if !server.is_approved() {
        return Err(McpError::NotApproved);
    }
    let cmd = server.command.as_deref().unwrap_or("");
    let program = if crate::platform::is_absolute_str(cmd) {
        PathBuf::from(cmd)
    } else {
        find_executable(cmd, dirs).unwrap_or_default()
    };
    if program.as_os_str().is_empty() || !program.is_file() {
        return Ok(fail(format!(
            "The command `{cmd}` was not found. Install it, or use its absolute path."
        )));
    }
    let args: Vec<String> = server
        .args
        .iter()
        .map(|a| store.resolve(server, a))
        .collect::<Result<_, _>>()?;
    let mut env = vec![(
        "PATH".to_string(),
        std::env::join_paths(dirs)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )];
    for e in &server.env {
        env.push((e.name.clone(), store.resolve(server, &e.value)?));
    }

    let mut command = Command::new(&program);
    command
        .args(&args)
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    #[cfg(windows)]
    {
        // No console window for the probe; taskkill /T ends the tree.
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| McpError::Invalid(format!("Could not start the server: {e}")))?;
    let mut stdin = child.stdin.take().expect("piped");
    let stdout = child.stdout.take().expect("piped");
    let _guard = Guard(child);

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut r = BufReader::new(stdout);
        let mut total = 0usize;
        loop {
            let mut line = String::new();
            match Read::take(&mut r, MAX_LINE as u64).read_line(&mut line) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    total += n;
                    if total > 4 * MAX_LINE || tx.send(line).is_err() {
                        return;
                    }
                }
            }
        }
    });

    let deadline = Instant::now() + timeout;
    let mut send = |v: serde_json::Value| -> bool {
        writeln!(stdin, "{v}").and_then(|_| stdin.flush()).is_ok()
    };
    // Wait for the response with this id; notifications and server requests are ignored.
    let wait = |id: i64| -> Result<serde_json::Value, String> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(line) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                        if v.get("id").and_then(|x| x.as_i64()) == Some(id)
                            && (v.get("result").is_some() || v.get("error").is_some())
                        {
                            return Ok(v);
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err("No answer from the server in time.".into())
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("The server exited before finishing the handshake.".into())
                }
            }
        }
    };

    if !send(
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"naurix","version":env!("CARGO_PKG_VERSION")}}}),
    ) {
        return Ok(fail("The server closed its input immediately."));
    }
    let init = match wait(1) {
        Ok(v) => v,
        Err(e) => return Ok(fail(e)),
    };
    if let Some(err) = init.get("error") {
        return Ok(fail(format!(
            "The server rejected the handshake: {}",
            err.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error")
        )));
    }
    let result = &init["result"];
    let mut out = ProbeResult {
        ok: true,
        status: "connected".into(),
        server_name: result["serverInfo"]["name"].as_str().map(str::to_string),
        server_version: result["serverInfo"]["version"].as_str().map(str::to_string),
        protocol: result["protocolVersion"].as_str().map(str::to_string),
        ..Default::default()
    };
    send(serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}));

    // Only ask for tools if the server declares them.
    if result["capabilities"].get("tools").is_some() {
        let mut cursor: Option<String> = None;
        for page in 0..MAX_PAGES {
            let id = 2 + page as i64;
            let params = cursor
                .as_ref()
                .map(|c| serde_json::json!({"cursor": c}))
                .unwrap_or_else(|| serde_json::json!({}));
            send(
                serde_json::json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params}),
            );
            let resp = match wait(id) {
                Ok(v) => v,
                Err(e) => {
                    out.ok = false;
                    out.status = "failed".into();
                    out.error = Some(format!("Connected, but listing tools failed: {e}"));
                    return Ok(out);
                }
            };
            for t in resp["result"]["tools"].as_array().into_iter().flatten() {
                if out.tools.len() >= MAX_TOOLS {
                    break;
                }
                if let Some(name) = t["name"].as_str() {
                    out.tools.push(ToolInfo {
                        name: name.chars().take(100).collect(),
                        description: t["description"]
                            .as_str()
                            .map(|d| d.chars().take(200).collect()),
                    });
                }
            }
            cursor = resp["result"]["nextCursor"].as_str().map(str::to_string);
            if cursor.is_none() || out.tools.len() >= MAX_TOOLS {
                break;
            }
        }
    }
    Ok(out)
}
