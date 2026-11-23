//! Thin Tauri shell: validate IPC input, delegate to `naurix-core`.

mod agent;
mod approval;
mod clipboard;
mod events;
mod git;
mod lifecycle;
mod mcp;
mod settings;
mod skills;
mod terminal;
mod workspace;
mod worktree;

use naurix_core::persistence::Db;
use naurix_core::session::SessionManager;
use serde::Serialize;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Manager;
use terminal::{SharedTerminals, TERMINATE_GRACE};

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    arch: &'static str,
    os: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    let i = naurix_core::app_info();
    AppInfo {
        version: i.version,
        arch: i.arch,
        os: i.os,
    }
}

#[cfg(target_os = "macos")]
fn build_menu<R: tauri::Runtime>(handle: &tauri::AppHandle<R>) -> tauri::Result<Menu<R>> {
    // Custom menu: the default one binds Cmd+W to "Close Window", which must instead close a pane.
    let app = Submenu::with_items(
        handle,
        "Naurix",
        true,
        &[
            &PredefinedMenuItem::about(handle, None, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::hide(handle, None)?,
            &PredefinedMenuItem::hide_others(handle, None)?,
            &PredefinedMenuItem::show_all(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &MenuItem::with_id(
                handle,
                lifecycle::MENU_QUIT,
                "Quit Naurix",
                true,
                Some("CmdOrCtrl+Q"),
            )?,
        ],
    )?;
    let edit = Submenu::with_items(
        handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(handle, None)?,
            &PredefinedMenuItem::redo(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::cut(handle, None)?,
            &PredefinedMenuItem::copy(handle, None)?,
            &PredefinedMenuItem::paste(handle, None)?,
            &PredefinedMenuItem::select_all(handle, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        handle,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
            &PredefinedMenuItem::fullscreen(handle, None)?,
        ],
    )?;
    Menu::with_items(handle, &[&app, &edit, &window])
}

pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    // The custom menu only exists to rebind Cmd+W on macOS. Linux and Windows get no menu bar:
    // terminal shortcuts are handled in the web view (see src/shortcuts.ts).
    #[cfg(target_os = "macos")]
    let builder = builder.menu(build_menu).on_menu_event(|app, event| {
        if event.id() == lifecycle::MENU_QUIT {
            lifecycle::request_quit(app);
        }
    });
    let app = builder
        .setup(|app| {
            // NAURIX_DATA_DIR (absolute path) points the app at a separate data folder, so tests and
            // experiments never touch your real workspaces. Without it, the standard app data folder is used.
            let dir = match std::env::var_os("NAURIX_DATA_DIR").map(std::path::PathBuf::from) {
                Some(d) if d.is_absolute() => d,
                Some(d) => {
                    return Err(format!(
                        "NAURIX_DATA_DIR must be an absolute path, got {}",
                        d.display()
                    )
                    .into())
                }
                None => {
                    let dir = app.path().app_data_dir()?;
                    // Pre-rename installs stored data under dev.agentterminal.app; keep it.
                    if let Some(parent) = dir.parent() {
                        let _ = naurix_core::persistence::migrate_legacy_data_dir(
                            &parent.join("dev.agentterminal.app"),
                            &dir,
                        );
                    }
                    dir
                }
            };
            std::fs::create_dir_all(&dir)?;
            let db = Arc::new(Db::open(&dir.join("naurix.db")).map_err(|e| e.to_string())?);
            // Recovery (DATA-004): processes do not survive a restart.
            db.mark_active_sessions_interrupted()
                .map_err(|e| e.to_string())?;
            db.prune_sessions(50).map_err(|e| e.to_string())?;
            app.manage(db.clone());
            // Approval gate (ADR 0027). Earlier preset approvals become grants once, so nobody is asked
            // again for what they already approved. A failure leaves the old setting for the next start.
            let gate: approval::SharedGate =
                Arc::new(naurix_core::approval::ApprovalGate::new(db.clone()));
            let _ = naurix_core::agents::migrate_legacy_approvals(&db, &gate);
            app.manage(gate);
            app.manage(mcp::DataDir(dir.clone()));
            // Half-finished downloads from a previous run are never reused.
            let skills = naurix_core::skills::SkillStore::new(&db, &dir);
            skills.cleanup_staging();
            skills.seed_builtin();
            app.manage::<mcp::SharedSecrets>(Arc::new(naurix_core::mcp::KeychainStore));
            let terminals: SharedTerminals =
                Arc::new(SessionManager::new(db.clone(), TERMINATE_GRACE));
            if let Ok(s) = naurix_core::settings::get(&db) {
                terminals.set_max_agents(usize::from(s.agents.max_concurrent));
            }
            events::bridge_session_events(app.handle().clone(), &terminals);
            app.manage(terminals);
            Ok(())
        })
        .on_window_event(lifecycle::on_window_event)
        .on_page_load(|webview, payload| {
            // A (re)load discards the JS state that held the output consumers, but not the sessions:
            // they keep running and the new page reattaches (ADR 0026).
            if let tauri::webview::PageLoadEvent::Started = payload.event() {
                if let Some(t) = webview.try_state::<SharedTerminals>() {
                    t.detach_all();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            workspace::workspace_list,
            workspace::workspace_create,
            workspace::workspace_rename,
            workspace::workspace_open,
            workspace::workspace_remove,
            workspace::session_recent,
            agent::agent_list,
            agent::agent_set_path,
            agent::agent_custom_save,
            agent::agent_custom_delete,
            agent::agent_authorize,
            worktree::worktree_list,
            worktree::worktree_support,
            worktree::agents_in_workspace_folder,
            worktree::session_worktree,
            worktree::worktree_review_remove,
            worktree::worktree_remove,
            worktree::worktree_forget,
            agent::agent_launch,
            approval::approval_decide,
            approval::approval_list,
            approval::approval_revoke,
            approval::approval_revoke_all,
            mcp::mcp_list,
            mcp::mcp_catalog,
            mcp::mcp_catalog_input,
            mcp::mcp_save,
            mcp::mcp_delete,
            mcp::mcp_set_enabled,
            mcp::mcp_secret_set,
            mcp::mcp_secret_clear,
            mcp::mcp_approve,
            mcp::mcp_test,
            mcp::mcp_sync_agents,
            mcp::mcp_sync_plan,
            mcp::mcp_sync_apply,
            mcp::mcp_sync_runs,
            mcp::mcp_sync_rollback,
            git::git_status,
            git::git_diff,
            git::git_worktrees,
            git::git_trust,
            settings::settings_get,
            settings::settings_update,
            settings::settings_reset,
            settings::security_info,
            settings::security_untrust_repo,
            settings::security_revoke_mcp_approvals,
            settings::perf_snapshot,
            skills::skill_list,
            skills::skill_agents,
            skills::skill_inspect_git,
            skills::skill_inspect_local,
            skills::skill_install,
            skills::skill_discard_staging,
            skills::skill_set_link,
            skills::skill_check_update,
            skills::skill_apply_update,
            skills::skill_revert,
            skills::skill_uninstall,
            agent::agent_version,
            clipboard::clipboard_read,
            clipboard::clipboard_write,
            terminal::terminal_spawn,
            terminal::terminal_write,
            terminal::terminal_write_bytes,
            terminal::terminal_resize,
            terminal::terminal_ack,
            terminal::terminal_has_foreground_job,
            terminal::session_list,
            terminal::session_attach,
            terminal::session_detach,
            terminal::session_close,
            terminal::debug_smoke,
            terminal::debug_smoke_visual,
            terminal::bench_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Naurix");

    app.run(lifecycle::on_run_event);
}
