use herdr_idle_inhibitor::herdr::protocol::{parse_agents, parse_plugins};
#[test]
fn strict_envelope_and_minimal_dto() {
    let good=br#"{"id":"x","result":{"type":"agent_list","agents":[{"terminal_id":"t","agent_status":"working","title":"discard","tokens":123}]}}"#;
    assert_eq!(parse_agents(good, "x").unwrap().len(), 1);
    for bad in [
        br#"{"id":"wrong","result":{"type":"agent_list","agents":[]}}"#.as_slice(),
        br#"{"id":"x","result":{"type":"agent_list"}}"#,
        br#"{"id":"x","result":{"type":"agent_list","agents":[{"terminal_id":"t"}]}}"#,
        br#"{"id":"x","error":{"code":"bad","message":"secret"}}"#,
    ] {
        assert!(parse_agents(bad, "x").is_err());
    }
    assert!(parse_plugins(br#"{"id":"p","result":{"type":"plugin_list","plugins":[{"plugin_id":"herdr-idle-inhibitor","enabled":true,"plugin_root":"/test"}]}}"#,"p").unwrap()[0].enabled);
}
#[tokio::test]
async fn bounded_framing_distinguishes_empty_and_truncated() {
    use herdr_idle_inhibitor::herdr::transport::read_frame;
    assert_eq!(read_frame(&mut &b""[..], 32).await.unwrap(), None);
    assert!(read_frame(&mut &b"{}"[..], 32).await.is_err());
    assert!(read_frame(&mut &b"123456\n"[..], 4).await.is_err());
    assert_eq!(
        read_frame(&mut &b"{}\n"[..], 32).await.unwrap(),
        Some(b"{}".to_vec())
    );
}

#[test]
fn root_discovery_removes_inherited_custom_xdg_for_default_root() {
    use std::{collections::BTreeMap, path::Path};
    let env = BTreeMap::from([("HOME".into(), "/default-home".into())]);
    let command = herdr_idle_inhibitor::herdr::discovery::root_command(
        Path::new("herdr"),
        &["session", "list", "--json"],
        &env,
    );
    let values: BTreeMap<_, _> = command
        .as_std()
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert_eq!(values.get("XDG_CONFIG_HOME"), Some(&None));
    assert_eq!(values.get("HOME"), Some(&Some("/default-home".into())));
    assert_eq!(values.get("HERDR_CONFIG_PATH"), Some(&None));
    // Any custom XDG_CONFIG_HOME inherited from the owner is explicitly unset.
}

#[test]
fn only_source_qualified_herdr_version_is_accepted_and_readme_matches() {
    use herdr_idle_inhibitor::herdr::protocol::parse_ping;
    for version in ["0.9.2", "0.9.4", "1.0.0"] {
        let payload = serde_json::to_vec(
            &serde_json::json!({"id":"ping","result":{"type":"pong","version":version}}),
        )
        .unwrap();
        assert!(parse_ping(&payload, "ping").is_err());
    }
    assert_eq!(
        parse_ping(
            br#"{"id":"ping","result":{"type":"pong","version":"0.9.3"}}"#,
            "ping"
        )
        .unwrap(),
        "0.9.3"
    );
    assert!(include_str!("../README.md").contains("**Herdr 0.9.3**"));
    assert!(!include_str!("../README.md").contains("Herdr 0.9.3+"));
}
