//! Application settings (M10): one typed JSON document in the `settings` table. Never secrets.
//!
//! Writes are validated strictly (an invalid value is an error the UI can show); reads are tolerant
//! (a hand-edited or newer-version value falls back to its default instead of breaking the app).

use crate::persistence::{Db, DbError};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;

const KEY: &str = "app.settings";
const LEGACY_FONT_FAMILY: &str = "terminal.fontFamily";
const LEGACY_FONT_SIZE: &str = "terminal.fontSize";

pub const SCROLLBACK_MIN: u32 = 500;
pub const SCROLLBACK_MAX: u32 = 100_000;
pub const FONT_MIN: u8 = 8;
pub const FONT_MAX: u8 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    #[default]
    Block,
    Bar,
    Underline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Renderer {
    #[default]
    Dom,
    Webgl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    /// A shell that exits cleanly (code 0) closes its pane. Agent panes always stay.
    pub close_on_clean_exit: bool,
    /// Ask before closing a pane or tab that still has something running.
    pub confirm_close_running: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            close_on_clean_exit: true,
            confirm_close_running: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Terminal {
    pub font_family: Option<String>,
    pub font_size: u8,
    pub scrollback: u32,
    pub cursor_blink: bool,
    pub cursor_style: CursorStyle,
    pub renderer: Renderer,
    /// Start the shell as a login shell (`-l`): loads your profile, but can be slow to start.
    pub login_shell: bool,
    /// Absolute path of the shell to start; `None` = your `$SHELL`.
    pub shell_path: Option<String>,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 13,
            scrollback: 5000,
            cursor_blink: true,
            cursor_style: CursorStyle::Block,
            renderer: Renderer::Dom,
            login_shell: true,
            shell_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    System,
    #[default]
    Dark,
    Light,
}

/// Theme preferences. Theme *ids* are opaque here (the frontend registry is authoritative and falls
/// back to the default for an unknown id), so imported custom themes can be added later without a
/// schema change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    pub mode: AppearanceMode,
    pub dark_theme: String,
    pub light_theme: String,
    /// Custom accent as `#rrggbb`, or `None` for the theme's own.
    pub accent: Option<String>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::Dark,
            dark_theme: "naurix-dark".into(),
            light_theme: "naurix-light".into(),
            accent: None,
        }
    }
}

fn valid_theme_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 48
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub general: General,
    pub appearance: Appearance,
    pub terminal: Terminal,
}

/// A CSS font-family list is assembled on the frontend from this string, so only characters that
/// cannot break out of a font list are accepted (no `;`, `{`, `}`, `(`, `)`, `\`, `/` or `:`).
pub fn validate_font_family(s: &str) -> Result<String, DbError> {
    let t = s.trim();
    let ok = !t.is_empty()
        && t.chars().count() <= 200
        && t.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | ',' | '\'' | '"' | '-' | '_' | '.'));
    if ok {
        Ok(t.to_string())
    } else {
        Err(DbError::Invalid("Font names may contain letters, digits, spaces and , ' \" - _ . only (max 200 characters).".into()))
    }
}

fn is_executable_file(p: &str) -> bool {
    p.starts_with('/')
        && !p.contains('\0')
        && std::fs::metadata(p)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

/// Strict validation for writes. Returns the normalized settings.
pub fn validate(s: &Settings) -> Result<Settings, DbError> {
    let mut s = s.clone();
    let bad = |m: &str| Err(DbError::Invalid(m.into()));
    s.terminal.font_family = match s.terminal.font_family.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(f) => Some(validate_font_family(f)?),
    };
    for id in [&s.appearance.dark_theme, &s.appearance.light_theme] {
        if !valid_theme_id(id) {
            return bad("Theme ids use lowercase letters, digits and '-' only.");
        }
    }
    s.appearance.accent = match s.appearance.accent.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(c) if valid_hex_color(c) => Some(c.to_ascii_lowercase()),
        Some(_) => return bad("The accent color must look like #8b7cff."),
    };
    if !(FONT_MIN..=FONT_MAX).contains(&s.terminal.font_size) {
        return bad(&format!(
            "Font size must be between {FONT_MIN} and {FONT_MAX}."
        ));
    }
    if !(SCROLLBACK_MIN..=SCROLLBACK_MAX).contains(&s.terminal.scrollback) {
        return bad(&format!(
            "Scrollback must be between {SCROLLBACK_MIN} and {SCROLLBACK_MAX} lines."
        ));
    }
    s.terminal.shell_path = match s.terminal.shell_path.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(p) if is_executable_file(p) => Some(p.to_string()),
        Some(_) => return bad("The shell must be an absolute path to an executable file."),
    };
    Ok(s)
}

/// Tolerant clean-up for reads: anything out of range or unusable becomes its default.
fn sanitize(mut s: Settings) -> Settings {
    let d = Terminal::default();
    let da = Appearance::default();
    if !valid_theme_id(&s.appearance.dark_theme) {
        s.appearance.dark_theme = da.dark_theme;
    }
    if !valid_theme_id(&s.appearance.light_theme) {
        s.appearance.light_theme = da.light_theme;
    }
    s.appearance.accent = s
        .appearance
        .accent
        .take()
        .filter(|c| valid_hex_color(c))
        .map(|c| c.to_ascii_lowercase());
    s.terminal.font_family = s
        .terminal
        .font_family
        .take()
        .and_then(|f| validate_font_family(&f).ok());
    if !(FONT_MIN..=FONT_MAX).contains(&s.terminal.font_size) {
        s.terminal.font_size = d.font_size;
    }
    if !(SCROLLBACK_MIN..=SCROLLBACK_MAX).contains(&s.terminal.scrollback) {
        s.terminal.scrollback = d.scrollback;
    }
    s.terminal.shell_path = s
        .terminal
        .shell_path
        .take()
        .filter(|p| is_executable_file(p));
    s
}

/// Current settings (defaults for anything unset). Reads the older font-only keys once if no
/// settings document exists yet.
pub fn get(db: &Db) -> Result<Settings, DbError> {
    if let Some(json) = db.get_setting(KEY)? {
        return Ok(sanitize(serde_json::from_str(&json).unwrap_or_default()));
    }
    let mut s = Settings::default();
    if let Some(f) = db
        .get_setting(LEGACY_FONT_FAMILY)?
        .and_then(|j| serde_json::from_str::<String>(&j).ok())
    {
        s.terminal.font_family = Some(f);
    }
    if let Some(n) = db
        .get_setting(LEGACY_FONT_SIZE)?
        .and_then(|j| serde_json::from_str::<u8>(&j).ok())
    {
        s.terminal.font_size = n;
    }
    Ok(sanitize(s))
}

pub fn update(db: &Db, s: &Settings) -> Result<Settings, DbError> {
    let v = validate(s)?;
    db.set_setting(KEY, &serde_json::to_string(&v).expect("serializable"))?;
    Ok(v)
}

pub fn reset(db: &Db) -> Result<Settings, DbError> {
    db.delete_setting(KEY)?;
    db.delete_setting(LEGACY_FONT_FAMILY)?;
    db.delete_setting(LEGACY_FONT_SIZE)?;
    Ok(Settings::default())
}

/// The shell program and arguments for a new terminal, from settings and the environment.
pub fn shell_command(t: &Terminal, env_shell: Option<&str>) -> (String, Vec<String>) {
    let program = t
        .shell_path
        .clone()
        .filter(|p| is_executable_file(p))
        .or_else(|| {
            env_shell
                .filter(|p| is_executable_file(p))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "/bin/zsh".to_string());
    (
        program,
        if t.login_shell {
            vec!["-l".into()]
        } else {
            vec![]
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_documented_ones() {
        let s = Settings::default();
        assert_eq!(
            (
                s.terminal.font_size,
                s.terminal.scrollback,
                s.terminal.login_shell
            ),
            (13, 5000, true)
        );
        assert!(s.general.close_on_clean_exit && s.general.confirm_close_running);
        assert_eq!(
            (s.terminal.renderer, s.terminal.cursor_style),
            (Renderer::Dom, CursorStyle::Block)
        );
        assert_eq!(get(&Db::open_in_memory().unwrap()).unwrap(), s);
    }

    #[test]
    fn roundtrip_reset_and_camel_case_json() {
        let db = Db::open_in_memory().unwrap();
        let mut s = Settings::default();
        s.terminal.font_family = Some("  Dank Mono ".into());
        s.terminal.scrollback = 20_000;
        s.terminal.renderer = Renderer::Webgl;
        s.terminal.cursor_style = CursorStyle::Bar;
        s.general.close_on_clean_exit = false;
        let saved = update(&db, &s).unwrap();
        assert_eq!(saved.terminal.font_family.as_deref(), Some("Dank Mono"));
        assert_eq!(get(&db).unwrap(), saved);
        let json = serde_json::to_value(&saved).unwrap();
        assert_eq!(json["terminal"]["fontFamily"], "Dank Mono");
        assert_eq!(json["terminal"]["renderer"], "webgl");
        assert_eq!(json["general"]["closeOnCleanExit"], false);
        assert_eq!(reset(&db).unwrap(), Settings::default());
        assert_eq!(get(&db).unwrap(), Settings::default());
    }

    #[test]
    fn strict_validation_on_write() {
        let db = Db::open_in_memory().unwrap();
        let try_with = |f: &dyn Fn(&mut Settings)| {
            let mut s = Settings::default();
            f(&mut s);
            update(&db, &s)
        };
        assert!(try_with(&|s| s.terminal.font_size = 7).is_err());
        assert!(try_with(&|s| s.terminal.font_size = 33).is_err());
        assert!(try_with(&|s| s.terminal.scrollback = 499).is_err());
        assert!(try_with(&|s| s.terminal.scrollback = 100_001).is_err());
        assert!(try_with(&|s| s.terminal.font_family = Some("x; background:red".into())).is_err());
        assert!(try_with(&|s| s.terminal.shell_path = Some("relative/zsh".into())).is_err());
        assert!(try_with(&|s| s.terminal.shell_path = Some("/no/such/shell".into())).is_err());
        assert!(
            try_with(&|s| s.terminal.shell_path = Some("/etc/hosts".into())).is_err(),
            "not executable"
        );
        assert!(try_with(&|s| s.terminal.shell_path = Some("/bin/sh".into())).is_ok());
        assert!(
            try_with(&|s| s.terminal.font_family = Some("   ".into()))
                .unwrap()
                .terminal
                .font_family
                .is_none(),
            "blank means default"
        );
        // nothing invalid was stored by the failed attempts
        assert_eq!(get(&db).unwrap().terminal.font_size, 13);
    }

    #[test]
    fn reads_are_tolerant_of_bad_or_partial_documents() {
        let db = Db::open_in_memory().unwrap();
        db.set_setting(KEY, r#"{"terminal":{"fontSize":99,"scrollback":3,"fontFamily":"a;b","shellPath":"/no/such","renderer":"webgl"},"general":{"closeOnCleanExit":false},"futureField":1}"#).unwrap();
        let s = get(&db).unwrap();
        assert_eq!(
            (
                s.terminal.font_size,
                s.terminal.scrollback,
                s.terminal.font_family.clone(),
                s.terminal.shell_path.clone()
            ),
            (13, 5000, None, None)
        );
        assert_eq!(
            s.terminal.renderer,
            Renderer::Webgl,
            "valid values are kept"
        );
        assert!(
            !s.general.close_on_clean_exit && s.general.confirm_close_running,
            "missing fields use defaults"
        );
        db.set_setting(KEY, "not json at all").unwrap();
        assert_eq!(get(&db).unwrap(), Settings::default());
        db.set_setting(KEY, r#"{"terminal":{"renderer":"quantum"}}"#)
            .unwrap();
        assert_eq!(
            get(&db).unwrap(),
            Settings::default(),
            "an unknown enum value does not break startup"
        );
    }

    #[test]
    fn migrates_the_older_font_only_keys() {
        let db = Db::open_in_memory().unwrap();
        db.set_setting(LEGACY_FONT_FAMILY, "\"Dank Mono\"").unwrap();
        db.set_setting(LEGACY_FONT_SIZE, "15").unwrap();
        let s = get(&db).unwrap();
        assert_eq!(
            (s.terminal.font_family.as_deref(), s.terminal.font_size),
            (Some("Dank Mono"), 15)
        );
        update(&db, &s).unwrap();
        db.set_setting(LEGACY_FONT_SIZE, "20").unwrap();
        assert_eq!(
            get(&db).unwrap().terminal.font_size,
            15,
            "once a settings document exists, the old keys are ignored"
        );
    }

    #[test]
    fn appearance_defaults_validation_and_tolerant_reads() {
        let db = Db::open_in_memory().unwrap();
        let a = Appearance::default();
        assert_eq!(
            (
                a.mode,
                a.dark_theme.as_str(),
                a.light_theme.as_str(),
                a.accent.clone()
            ),
            (AppearanceMode::Dark, "naurix-dark", "naurix-light", None)
        );
        let mut s = Settings::default();
        s.appearance.mode = AppearanceMode::System;
        s.appearance.dark_theme = "tokyo-night".into();
        s.appearance.accent = Some(" #8B7CFF ".into());
        let saved = update(&db, &s).unwrap();
        assert_eq!(
            (saved.appearance.mode, saved.appearance.accent.as_deref()),
            (AppearanceMode::System, Some("#8b7cff"))
        );
        assert_eq!(
            serde_json::to_value(&saved).unwrap()["appearance"]["darkTheme"],
            "tokyo-night"
        );
        for bad in ["Tokyo Night", "", "x/y", "-lead", "UPPER", &"a".repeat(49)] {
            let mut t = Settings::default();
            t.appearance.dark_theme = bad.into();
            assert!(update(&db, &t).is_err(), "{bad:?}");
        }
        for bad in ["red", "#12345", "#gggggg", "8b7cff", "#8b7cffaa"] {
            let mut t = Settings::default();
            t.appearance.accent = Some(bad.into());
            assert!(update(&db, &t).is_err(), "{bad:?}");
        }
        // an unknown but well-formed id is kept (the frontend falls back); garbage on disk falls back here
        db.set_setting(KEY, r##"{"appearance":{"mode":"light","darkTheme":"future-theme","lightTheme":"NOT VALID","accent":"nope"}}"##).unwrap();
        let r = get(&db).unwrap();
        assert_eq!(
            (
                r.appearance.mode,
                r.appearance.dark_theme.as_str(),
                r.appearance.light_theme.as_str(),
                r.appearance.accent.clone()
            ),
            (AppearanceMode::Light, "future-theme", "naurix-light", None)
        );
        db.set_setting(KEY, r#"{"appearance":{"mode":"sepia"}}"#)
            .unwrap();
        assert_eq!(
            get(&db).unwrap(),
            Settings::default(),
            "an unknown mode never breaks startup"
        );
        // existing documents without an appearance group get the defaults
        db.set_setting(KEY, r#"{"terminal":{"fontSize":14}}"#)
            .unwrap();
        assert_eq!(get(&db).unwrap().appearance, Appearance::default());
    }

    #[test]
    fn shell_command_prefers_the_setting_then_env_then_zsh() {
        let mut t = Terminal::default();
        assert_eq!(
            shell_command(&t, Some("/bin/sh")),
            ("/bin/sh".into(), vec!["-l".into()])
        );
        assert_eq!(shell_command(&t, Some("/no/such")).0, "/bin/zsh");
        assert_eq!(
            shell_command(&t, Some("relative")).0,
            "/bin/zsh",
            "an invalid $SHELL is ignored"
        );
        t.shell_path = Some("/bin/bash".into());
        t.login_shell = false;
        assert_eq!(
            shell_command(&t, Some("/bin/sh")),
            ("/bin/bash".into(), vec![])
        );
        t.shell_path = Some("/gone".into());
        assert_eq!(
            shell_command(&t, Some("/bin/sh")).0,
            "/bin/sh",
            "a stale configured shell falls back"
        );
    }
}
