//! Tauri commands for the Security Suite (ADR 0032). The webview names only a workspace id: the file
//! dialog is opened from here, so which report is read comes from the user's choice, never from the page.
//! A report is read, returned to the view and forgotten. Nothing is stored, logged or published.

use naurix_core::persistence::Db;
use naurix_core::security::{compare, read_report, Comparison, Report};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

fn err(e: impl ToString) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// The file's name only (for the heading), not its folder.
    file_name: String,
    report: Report,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpenResult {
    /// The user closed the dialog. Nothing was read.
    Cancelled,
    Opened(Opened),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compared {
    baseline: Opened,
    current: Opened,
    comparison: Comparison,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompareResult {
    Cancelled,
    Compared(Box<Compared>),
}

fn root_of(db: &Db, workspace_id: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(
        db.workspace(workspace_id).map_err(err)?.root_path,
    ))
}

/// Ask for one report, starting in the workspace folder. `None` when the user cancels.
fn pick(app: &AppHandle, root: &Path, title: &str) -> Option<PathBuf> {
    app.dialog()
        .file()
        .set_title(title)
        .set_directory(root)
        .add_filter("SARIF report", &["sarif", "json"])
        .blocking_pick_file()?
        .into_path()
        .ok()
}

fn open_one(path: &Path, root: &Path) -> Result<Opened, String> {
    let report = read_report(path, root).map_err(err)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Opened { file_name, report })
}

/// Pick a SARIF report and read it. The result lives only in the view that asked.
#[tauri::command]
pub async fn security_open_report(
    app: AppHandle,
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<OpenResult, String> {
    let root = root_of(&db, &workspace_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = pick(&app, &root, "Open a security report (SARIF)") else {
            return Ok(OpenResult::Cancelled);
        };
        open_one(&path, &root).map(OpenResult::Opened)
    })
    .await
    .map_err(err)?
}

/// Pick a baseline report, then a current one, and compare them (Purple regression).
#[tauri::command]
pub async fn security_compare(
    app: AppHandle,
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<CompareResult, String> {
    let root = root_of(&db, &workspace_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(base) = pick(&app, &root, "Choose the baseline report (the earlier one)") else {
            return Ok(CompareResult::Cancelled);
        };
        let baseline = open_one(&base, &root)?;
        let Some(cur) = pick(&app, &root, "Choose the current report (the later one)") else {
            return Ok(CompareResult::Cancelled);
        };
        let current = open_one(&cur, &root)?;
        let comparison = compare(&baseline.report, &current.report);
        Ok(CompareResult::Compared(Box::new(Compared {
            baseline,
            current,
            comparison,
        })))
    })
    .await
    .map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use naurix_core::security::parse_report;

    fn opened(name: &str, sarif: &str) -> Opened {
        Opened {
            file_name: name.into(),
            report: parse_report(sarif.as_bytes(), Path::new("/w")).unwrap(),
        }
    }

    const ONE: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"t"}},"results":[{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.rs"},"region":{"startLine":3}}}]}]}]}"#;

    /// The shapes `src/features/security/api.ts` reads.
    #[test]
    fn results_serialise_with_a_kind_tag_and_camel_case_fields() {
        let v = serde_json::to_value(OpenResult::Opened(opened("r.sarif", ONE))).unwrap();
        assert_eq!(v["kind"], "opened");
        assert_eq!(v["fileName"], "r.sarif");
        let f = &v["report"]["findings"][0];
        assert_eq!(f["ruleId"], "R");
        assert_eq!(f["severity"], "medium");
        assert_eq!(f["location"]["path"], "a.rs");
        assert_eq!(f["location"]["line"], 3);
        assert_eq!(v["report"]["notShown"], 0);

        let (b, c) = (opened("b.sarif", ONE), opened("c.sarif", ONE));
        let comparison = compare(&b.report, &c.report);
        let v = serde_json::to_value(CompareResult::Compared(Box::new(Compared {
            baseline: b,
            current: c,
            comparison,
        })))
        .unwrap();
        assert_eq!(v["kind"], "compared");
        assert_eq!(v["comparison"]["stillPresent"].as_array().unwrap().len(), 1);
        assert_eq!(v["comparison"]["noLongerReported"], serde_json::json!([]));
        assert_eq!(v["baseline"]["fileName"], "b.sarif");
        assert_eq!(
            serde_json::to_value(OpenResult::Cancelled).unwrap()["kind"],
            "cancelled"
        );
    }
}
