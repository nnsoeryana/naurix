//! Tauri commands for the approval gate (ADR 0027). The webview can only answer a request by id and
//! scope; it never supplies a fingerprint, a command or a target.

use naurix_core::approval::{Approval, ApprovalGate, Decision, Scope};
use naurix_core::persistence::Db;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

pub type SharedGate = Arc<ApprovalGate>;

#[derive(Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum DecisionInput {
    Deny,
    Allow { scope: Scope },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalDto {
    id: String,
    class: String,
    scope: String,
    summary: String,
    workspace_id: Option<String>,
    workspace_name: Option<String>,
    state: String,
    created_at: String,
    last_used_at: Option<String>,
    ended_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalsDto {
    active: Vec<ApprovalDto>,
    history: Vec<ApprovalDto>,
}

fn dto(db: &Db, a: Approval) -> ApprovalDto {
    let workspace_name = a
        .workspace_id
        .as_deref()
        .and_then(|id| db.workspace(id).ok())
        .map(|w| w.name);
    ApprovalDto {
        id: a.id,
        class: a.class,
        scope: a.scope,
        summary: a.summary,
        workspace_id: a.workspace_id,
        workspace_name,
        state: a.state,
        created_at: a.created_at,
        last_used_at: a.last_used_at,
        ended_at: a.ended_at,
    }
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

/// Answer an approval request. Allowing records a grant for what the backend built the request from.
#[tauri::command]
pub async fn approval_decide(
    gate: State<'_, SharedGate>,
    request_id: String,
    decision: DecisionInput,
) -> Result<(), String> {
    let d = match decision {
        DecisionInput::Deny => Decision::Deny,
        DecisionInput::Allow { scope } => Decision::Allow(scope),
    };
    gate.decide(&request_id, d).map(|_| ()).map_err(err)
}

#[tauri::command]
pub async fn approval_list(
    gate: State<'_, SharedGate>,
    db: State<'_, Arc<Db>>,
) -> Result<ApprovalsDto, String> {
    let active = gate.list_active().map_err(err)?;
    let history = gate.history(50).map_err(err)?;
    Ok(ApprovalsDto {
        active: active.into_iter().map(|a| dto(&db, a)).collect(),
        history: history.into_iter().map(|a| dto(&db, a)).collect(),
    })
}

#[tauri::command]
pub async fn approval_revoke(gate: State<'_, SharedGate>, id: String) -> Result<bool, String> {
    gate.revoke(&id).map_err(err)
}

#[tauri::command]
pub async fn approval_revoke_all(gate: State<'_, SharedGate>) -> Result<usize, String> {
    gate.revoke_all().map_err(err)
}
