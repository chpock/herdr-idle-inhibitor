use herdr_idle_inhibitor::status::Status;
#[test]
fn unavailable_has_required_nulls_and_strict_contract() {
    let v = serde_json::to_value(Status::unavailable(
        "monitor_unavailable",
        "Monitor unavailable",
    ))
    .unwrap();
    Status::from_wire(v.clone()).unwrap();
    for field in ["error", "monitor", "control", "diagnostics"] {
        let mut bad = v.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(Status::from_wire(bad).is_err(), "accepted missing {field}");
    }
    for field in [
        "working_agents_observed",
        "servers_tracked",
        "oldest_snapshot_age_ms",
    ] {
        let mut bad = v.clone();
        bad["observation"].as_object_mut().unwrap().remove(field);
        assert!(Status::from_wire(bad).is_err());
    }
    for (path, value) in [
        (vec!["schema_version"], serde_json::json!(2)),
        (vec!["observation", "scope"], serde_json::json!("x")),
        (vec!["inhibition", "reason"], serde_json::json!("bogus")),
        (
            vec!["inhibition", "request_state"],
            serde_json::json!("bogus"),
        ),
        (vec!["inhibition", "backend"], serde_json::json!("bogus")),
    ] {
        let mut bad = v.clone();
        let mut at = &mut bad;
        for k in path {
            at = &mut at[k];
        }
        *at = value;
        assert!(Status::from_wire(bad).is_err());
    }
    let mut additive = v;
    additive["future"] = serde_json::json!(true);
    Status::from_wire(additive).unwrap();
}
#[test]
fn generated_schema_matches_checked_in_schema() {
    let generated = herdr_idle_inhibitor::status::public_schema();
    let checked: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/status-v1.json")).unwrap();
    assert_eq!(generated, checked);
}

fn successful() -> serde_json::Value {
    let mut s = Status::unavailable("monitor_unavailable", "Monitor unavailable");
    s.available = true;
    s.error = None;
    s.monitor = Some(herdr_idle_inhibitor::status::Monitor {
        instance_id: "test".into(),
        pid: 1,
        app_version: "0.1.0".into(),
        uptime_ms: 0,
        snapshot_seq: 1,
        evaluated_at_unix_ms: 0,
    });
    s.control = Some(herdr_idle_inhibitor::status::Control {
        paused: false,
        pause_persisted: true,
    });
    s.diagnostics = Some(herdr_idle_inhibitor::status::Diagnostics {
        issues: vec![],
        known_limitations: vec![],
        counters: Default::default(),
    });
    s.observation = herdr_idle_inhibitor::model::aggregate([].iter(), true, 0);
    s.inhibition.desired = Some(false);
    s.inhibition.resource_owned = Some(false);
    s.inhibition.reason = "no_work".into();
    s.inhibition.request_state = "none".into();
    serde_json::to_value(s).unwrap()
}
#[test]
fn availability_shapes_are_validated_by_boundary_and_schema() {
    let schema = herdr_idle_inhibitor::status::public_schema();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let dead = serde_json::to_value(Status::unavailable(
        "monitor_unavailable",
        "Monitor unavailable",
    ))
    .unwrap();
    let live = successful();
    for good in [&dead, &live] {
        assert!(validator.is_valid(good));
        Status::from_wire(good.clone()).unwrap();
    }
    let mut fake_zero = dead;
    fake_zero["observation"]["work"] = serde_json::json!("none");
    fake_zero["observation"]["complete"] = serde_json::json!(true);
    fake_zero["observation"]["working_agents_observed"] = serde_json::json!(0);
    fake_zero["inhibition"]["desired"] = serde_json::json!(false);
    fake_zero["inhibition"]["resource_owned"] = serde_json::json!(false);
    fake_zero["inhibition"]["reason"] = serde_json::json!("no_work");
    assert!(!validator.is_valid(&fake_zero));
    assert!(Status::from_wire(fake_zero).is_err());
    let mut null_live = live;
    null_live["observation"]["working_agents_observed"] = serde_json::Value::Null;
    null_live["inhibition"]["desired"] = serde_json::Value::Null;
    assert!(!validator.is_valid(&null_live));
    assert!(Status::from_wire(null_live).is_err());
}
