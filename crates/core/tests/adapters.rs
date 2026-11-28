#![cfg(unix)] // Unix mock programs and symlinks; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! Adapter facts: completeness, the "only Verified is acted on" rule, and fixtures captured from
//! the real CLIs (tests/fixtures/agents).
use naurix_core::agents::*;
use naurix_core::persistence::Db;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const ALL_FEATURES: [Feature; 5] = [
    Feature::PtyLaunch,
    Feature::McpConfig,
    Feature::Skills,
    Feature::StatusSignals,
    Feature::SessionResume,
];

fn fixture(name: &str) -> serde_json::Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/agents")
        .join(name);
    serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn every_preset_has_an_adapter_and_custom_agents_do_not() {
    for (id, _, _) in PRESETS {
        assert_eq!(
            adapter(id)
                .unwrap_or_else(|| panic!("no adapter for {id}"))
                .agent_id,
            *id
        );
    }
    assert_eq!(all_adapters().len(), PRESETS.len());
    assert!(adapter("custom-123").is_none());
}

#[test]
fn every_adapter_states_every_feature_exactly_once_with_a_reason() {
    for a in all_adapters() {
        for f in ALL_FEATURES {
            assert_eq!(
                a.capabilities.iter().filter(|c| c.feature == f).count(),
                1,
                "{} / {:?}",
                a.agent_id,
                f
            );
        }
        for c in a.capabilities {
            match c.level {
                Level::Verified { evidence } => assert!(
                    evidence.len() > 20,
                    "{} {:?}: thin evidence",
                    a.agent_id,
                    c.feature
                ),
                Level::DocumentedOnly { source } => assert!(
                    source.starts_with("https://"),
                    "{} {:?}: source must be a URL",
                    a.agent_id,
                    c.feature
                ),
                Level::DocumentedAccepted { source, basis } => assert!(
                    source.starts_with("https://") && basis.contains("decision"),
                    "{} {:?}",
                    a.agent_id,
                    c.feature
                ),
                Level::Unsupported { reason } => assert!(!reason.is_empty()),
            }
        }
    }
}

#[test]
fn only_verified_capabilities_are_acted_on() {
    // Verified today: generic launch everywhere; MCP config for the two CLIs captured on this machine.
    for a in all_adapters() {
        assert!(a.acts_on(Feature::PtyLaunch), "{}", a.agent_id);
        let expect_mcp = matches!(a.agent_id, "claude-code" | "opencode");
        assert_eq!(
            a.acts_on(Feature::McpConfig),
            expect_mcp,
            "MCP for {}",
            a.agent_id
        );
        // Skills: only through the accepted reversible symlink (ADR 0016), and only where a folder is documented.
        let expect_skills = matches!(
            a.agent_id,
            "claude-code" | "codex" | "antigravity-cli" | "opencode"
        );
        assert_eq!(
            a.acts_on(Feature::Skills),
            expect_skills,
            "skills for {}",
            a.agent_id
        );
        assert!(
            !matches!(a.level(Feature::Skills), Level::Verified { .. }),
            "no skills fact is *verified* yet: {}",
            a.agent_id
        );
        // Status signals are verified for Claude Code only (ADR 0029); nothing else may act on them.
        assert_eq!(
            a.acts_on(Feature::StatusSignals),
            a.agent_id == "claude-code",
            "status signals for {}",
            a.agent_id
        );
        // Nothing has been verified for resume yet (ADR 0011 / 0014).
        assert!(
            !a.acts_on(Feature::SessionResume),
            "resume for {}",
            a.agent_id
        );
    }
}

#[test]
fn verified_versions_are_recorded_exactly_where_a_fixture_exists() {
    assert_eq!(
        adapter("claude-code").unwrap().verified_with,
        Some("2.1.322")
    );
    assert_eq!(adapter("opencode").unwrap().verified_with, Some("1.18.10"));
    for id in ["codex", "gemini-cli", "antigravity-cli"] {
        assert_eq!(
            adapter(id).unwrap().verified_with,
            None,
            "{id} has never been run here"
        );
    }
    // The evidence text must name the version of the fixture it points to.
    let ev = |id: &str| match adapter(id).unwrap().level(Feature::McpConfig) {
        Level::Verified { evidence } => evidence,
        other => panic!("{other:?}"),
    };
    assert!(
        ev("claude-code").contains("2.1.322")
            && ev("claude-code").contains("claude-2.1.322-project.mcp.json")
    );
    assert!(
        ev("opencode").contains("1.18.10")
            && ev("opencode").contains("opencode-1.18.10-global.opencode.jsonc")
    );
}

#[test]
fn claude_fixture_matches_the_claimed_shape() {
    let f = fixture("claude-2.1.322-project.mcp.json");
    let servers = f
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .expect("top-level mcpServers");
    let demo = &servers["demo"];
    assert_eq!(demo["command"], "echo");
    assert_eq!(demo["args"], serde_json::json!(["hello"]));
    assert_eq!(demo["env"]["DEMO_KEY"], "abc");
    let web = &servers["web"];
    assert_eq!(web["type"], "http");
    assert_eq!(web["url"], "https://example.com/mcp");
    assert_eq!(web["headers"]["X-Demo"], "1");
}

#[test]
fn opencode_fixture_is_the_flat_shape_not_the_v2_servers_shape() {
    let f = fixture("opencode-1.18.10-global.opencode.jsonc");
    let mcp = f
        .get("mcp")
        .and_then(|v| v.as_object())
        .expect("top-level mcp");
    assert!(
        !mcp.contains_key("servers"),
        "1.18.10 writes mcp.<name>, not mcp.servers.<name>"
    );
    let demo = &mcp["demo"];
    assert_eq!(demo["type"], "local");
    assert_eq!(demo["command"], serde_json::json!(["echo", "hello"]));
    assert_eq!(demo["environment"]["DEMO_KEY"], "abc");
}

#[test]
fn gemini_deprecation_notice_and_successor_are_recorded() {
    assert!(adapter("gemini-cli")
        .unwrap()
        .notice
        .unwrap()
        .contains("18 June 2026"));
    assert!(PRESETS
        .iter()
        .any(|p| p.0 == "antigravity-cli" && p.2 == "agy"));
    assert!(adapter("antigravity-cli")
        .unwrap()
        .notice
        .unwrap()
        .contains("agy"));
}

#[test]
fn presence_detection_reports_existence_only() {
    let base = std::env::temp_dir().join(format!("at-presence-{}", std::process::id()));
    let (home, ws) = (base.join("home"), base.join("ws"));
    std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join(".mcp.json"), "{ not even valid json").unwrap(); // contents must not matter
    let p = detect_presence(adapter("claude-code").unwrap(), &home, &ws);
    let get = |purpose: &str, scope: Scope| {
        p.iter()
            .find(|x| x.purpose == purpose && x.scope == scope)
            .unwrap()
    };
    assert!(get("mcp", Scope::Project).exists);
    assert!(get("skills", Scope::User).exists);
    assert!(!get("mcp", Scope::User).exists);
    assert_eq!(
        get("skills", Scope::User).path,
        home.join(".claude/skills").to_string_lossy()
    );
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn version_is_only_queried_for_presets_approved_for_every_workspace() {
    use naurix_core::approval::{ApprovalGate, Decision, Scope, Verdict};
    use std::sync::Arc;

    let dir = std::env::temp_dir().join(format!("at-verstore-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ran = dir.join("was-run");
    let exe = dir.join("mockagent");
    // Running it leaves a marker, so "must not execute an unreviewed binary" is checked, not assumed.
    std::fs::write(
        &exe,
        format!(
            "#!/bin/sh\ntouch {}\n[ \"$1\" = --version ] && echo 'mockagent 9.9.9'\n",
            ran.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

    let db = Arc::new(Db::open_in_memory().unwrap());
    let gate = ApprovalGate::new(db.clone());
    let st = AgentStore::new(&db);
    st.set_path_override("claude-code", Some(exe.to_str().unwrap()))
        .unwrap();
    let dirs = vec![dir.clone()];

    // Not approved: refuses, and nothing was executed.
    assert!(matches!(
        st.version_if_approved("claude-code", &dirs, &gate),
        Err(AgentError::ReviewRequired)
    ));
    assert!(!ran.exists(), "an unapproved executable must not be run");

    // A once or workspace approval is not enough to run it for a version query.
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap().id;
    let (def, plan) = st.prepare_launch("claude-code", &dirs).unwrap();
    let action = launch_action(&def, &plan.program, &ws);
    let Verdict::Required(r) = gate.preview(&action).unwrap() else {
        panic!("expected a request")
    };
    gate.decide(&r.id, Decision::Allow(Scope::Workspace))
        .unwrap();
    assert!(matches!(
        st.version_if_approved("claude-code", &dirs, &gate),
        Err(AgentError::ReviewRequired)
    ));
    assert!(!ran.exists());

    // Approved for every workspace: the version is read.
    let Verdict::Required(r) = gate
        .preview(&launch_action(&def, &plan.program, "other"))
        .unwrap()
    else {
        panic!("expected a request")
    };
    gate.decide(&r.id, Decision::Allow(Scope::App)).unwrap();
    assert_eq!(
        st.version_if_approved("claude-code", &dirs, &gate)
            .unwrap()
            .as_deref(),
        Some("mockagent 9.9.9")
    );
    assert!(ran.exists());

    // A custom agent can never hold an app-wide grant, so it never qualifies.
    let c = st
        .save_custom(CustomAgent {
            id: String::new(),
            name: "Mock".into(),
            executable: exe.to_string_lossy().into_owned(),
            args: vec![],
            env: vec![],
            ..Default::default()
        })
        .unwrap();
    assert!(matches!(
        st.version_if_approved(&c.id, &dirs, &gate),
        Err(AgentError::ReviewRequired)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}
