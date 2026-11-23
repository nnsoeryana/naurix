//! Bridges Core domain events (ADR 0024) to the webview as one Tauri event, `session://lifecycle`.
//! Metadata only (ids, kind, exit status); terminal bytes never travel this way. Nothing in the
//! frontend consumes it yet.

use naurix_core::events::Event;
use naurix_core::persistence::SessionKind;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const SESSION_LIFECYCLE: &str = "session://lifecycle";
/// A workflow run or one of its steps changed (ADR 0030). Ids only; the view re-reads the run.
pub const RUN_CHANGED: &str = "run://changed";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RunPayload {
    run_id: String,
    workspace_id: String,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
enum LifecyclePayload {
    #[serde(rename_all = "camelCase")]
    Started {
        session: u64,
        workspace_id: String,
        kind: &'static str,
        adapter_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Exited {
        session: u64,
        workspace_id: String,
        code: Option<u32>,
        signal: Option<String>,
    },
    /// An agent's reported status (ADR 0029): a fixed word and a time, never content.
    #[serde(rename_all = "camelCase")]
    Status {
        session: u64,
        workspace_id: String,
        kind: &'static str,
        at: String,
    },
}

fn payload(e: Event) -> Option<LifecyclePayload> {
    Some(match e {
        Event::SessionStarted {
            session,
            workspace_id,
            kind,
            adapter_id,
        } => LifecyclePayload::Started {
            session: session.0,
            workspace_id,
            kind: match kind {
                SessionKind::Terminal => "terminal",
                SessionKind::Agent => "agent",
            },
            adapter_id,
        },
        Event::SessionExited {
            session,
            workspace_id,
            code,
            signal,
        } => LifecyclePayload::Exited {
            session: session.0,
            workspace_id,
            code,
            signal,
        },
        Event::SessionStatus {
            session,
            workspace_id,
            kind,
            at,
        } => LifecyclePayload::Status {
            session: session.0,
            workspace_id,
            kind: kind.word(),
            at,
        },
        // Runs have their own event (`run://changed`), handled by the bridge.
        Event::RunChanged { .. } => return None,
    })
}

/// Forward session events to the webview until the bus is dropped.
pub fn bridge_session_events(app: AppHandle, terminals: &crate::terminal::SharedTerminals) {
    let rx = terminals.events().subscribe();
    let _ = std::thread::Builder::new()
        .name("session-events".into())
        .spawn(move || {
            for e in rx {
                if let Event::RunChanged {
                    run_id,
                    workspace_id,
                } = &e
                {
                    let _ = app.emit(
                        RUN_CHANGED,
                        RunPayload {
                            run_id: run_id.clone(),
                            workspace_id: workspace_id.clone(),
                        },
                    );
                } else if let Some(p) = payload(e) {
                    let _ = app.emit(SESSION_LIFECYCLE, p);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use naurix_core::pty::SessionId;

    #[test]
    fn run_changes_use_their_own_event_not_the_lifecycle_one() {
        assert!(payload(Event::RunChanged {
            run_id: "r".into(),
            workspace_id: "w".into()
        })
        .is_none());
    }

    #[test]
    fn payload_is_camel_case_metadata_only() {
        let j = serde_json::to_value(payload(Event::SessionExited {
            session: SessionId(4),
            workspace_id: "w1".into(),
            code: Some(0),
            signal: None,
        }))
        .unwrap();
        assert_eq!(
            j,
            serde_json::json!({"type":"exited","session":4,"workspaceId":"w1","code":0,"signal":null})
        );
        let j = serde_json::to_value(payload(Event::SessionStarted {
            session: SessionId(5),
            workspace_id: "w1".into(),
            kind: SessionKind::Agent,
            adapter_id: Some("codex".into()),
        }))
        .unwrap();
        assert_eq!(
            j,
            serde_json::json!({"type":"started","session":5,"workspaceId":"w1","kind":"agent","adapterId":"codex"})
        );
        let j = serde_json::to_value(payload(Event::SessionStatus {
            session: SessionId(6),
            workspace_id: "w1".into(),
            kind: naurix_core::signals::SignalKind::AwaitingApproval,
            at: "2026-11-23T10:00:00Z".into(),
        }))
        .unwrap();
        assert_eq!(
            j,
            serde_json::json!({"type":"status","session":6,"workspaceId":"w1","kind":"awaiting_approval","at":"2026-11-23T10:00:00Z"})
        );
    }
}
