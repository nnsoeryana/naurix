//! Application lifecycle around sessions that outlive the view (ADR 0026): what closing the window,
//! clicking the Dock icon and quitting do. Sessions belong to Core; this only decides when the
//! process exits and whether to ask first.
//!
//! macOS only for now (the sole supported platform, ADR 0001): the close button hides the window so
//! the process and its sessions keep running, and Quit asks first when work is running. On Linux and
//! Windows closing the last window still quits, until each platform is validated (ADR 0021).

use crate::terminal::SharedTerminals;
use tauri::{AppHandle, Manager, RunEvent, Window, WindowEvent};
#[cfg(target_os = "macos")]
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

/// Menu id of the custom Quit item. The predefined Quit calls `NSApp terminate:` directly, which
/// Tauri cannot intercept, so it could not ask for confirmation.
#[cfg(target_os = "macos")]
pub const MENU_QUIT: &str = "naurix-quit";

#[cfg(target_os = "macos")]
fn show_main_window(app: &AppHandle) {
    // Startup failed (see `startup_failed` in lib.rs): there is nothing usable to show.
    if app.try_state::<SharedTerminals>().is_none() {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The close button hides the window instead of destroying it, so the webview, its terminals and
/// every running session stay alive. Closing the last window would otherwise exit the app.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    #[cfg(target_os = "macos")]
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let _ = window.hide();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, event);
}

pub fn on_run_event(app: &AppHandle, event: RunEvent) {
    match event {
        // Clicking the Dock icon while the window is hidden brings it back.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows,
            ..
        } => {
            if !has_visible_windows {
                show_main_window(app);
            }
        }
        // Explicit shutdown policy (TECHNICAL_DESIGN §6): terminate all child sessions on exit.
        RunEvent::Exit => {
            if let Some(t) = app.try_state::<SharedTerminals>() {
                t.shutdown();
            }
        }
        _ => {}
    }
}

/// Quit now, or ask first when something would be interrupted and the user left
/// "Ask before closing running work" on.
#[cfg(target_os = "macos")]
pub fn request_quit(app: &AppHandle) {
    let Some(terminals) = app.try_state::<SharedTerminals>() else {
        return app.exit(0);
    };
    let busy = terminals.busy_count();
    let ask = busy > 0
        && naurix_core::settings::get(terminals.db())
            .map(|s| s.general.confirm_close_running)
            .unwrap_or(true);
    if !ask {
        return app.exit(0);
    }
    // The window may be hidden; show it so the user sees what is running.
    show_main_window(app);
    let handle = app.clone();
    let what = if busy == 1 {
        "1 agent or command is still running.".to_string()
    } else {
        format!("{busy} agents or commands are still running.")
    };
    app.dialog()
        .message(format!("{what} Quitting stops them."))
        .title("Quit Naurix?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Quit".into(),
            "Keep running".into(),
        ))
        .show(move |quit| {
            if quit {
                handle.exit(0);
            }
        });
}
