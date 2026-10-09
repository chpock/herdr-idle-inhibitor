use crate::model::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    pub code: String,
    pub message: String,
}
impl Issue {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Monitor {
    pub instance_id: String,
    pub pid: u32,
    pub app_version: String,
    pub uptime_ms: u64,
    pub snapshot_seq: u64,
    pub evaluated_at_unix_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Control {
    pub paused: bool,
    pub pause_persisted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Inhibition {
    pub desired: Option<bool>,
    pub reason: String,
    pub backend: Option<String>,
    pub resource_owned: Option<bool>,
    pub request_state: String,
    pub release_in_ms: Option<u64>,
    pub retention_remaining_ms: Option<u64>,
    pub last_error: Option<Issue>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostics {
    pub issues: Vec<Issue>,
    pub known_limitations: Vec<String>,
    pub counters: Counters,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Status {
    pub schema_version: u32,
    pub available: bool,
    pub error: Option<Issue>,
    pub monitor: Option<Monitor>,
    pub control: Option<Control>,
    pub observation: Observation,
    pub inhibition: Inhibition,
    pub diagnostics: Option<Diagnostics>,
}
impl Status {
    pub fn unavailable(code: &str, message: &str) -> Self {
        let mut o = aggregate([].iter(), false, 0);
        o.working_agents_observed = None;
        o.servers_tracked = None;
        o.servers_current = None;
        o.servers_unknown = None;
        o.unknown_agents_observed = None;
        Self {
            schema_version: 1,
            available: false,
            error: Some(Issue::new(code, message)),
            monitor: None,
            control: None,
            observation: o,
            inhibition: Inhibition {
                desired: None,
                reason: "unavailable".into(),
                backend: None,
                resource_owned: None,
                request_state: "unknown".into(),
                release_in_ms: None,
                retention_remaining_ms: None,
                last_error: None,
            },
            diagnostics: None,
        }
    }
}
pub fn sanitized(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(256).collect()
}

pub const REASONS: &[&str] = &[
    "working",
    "release_grace",
    "observation_grace",
    "no_work",
    "paused",
    "ineligible",
    "shutdown",
    "unknown",
    "unavailable",
];
pub const STATES: &[&str] = &[
    "none",
    "pending",
    "accepted",
    "releasing",
    "suppressed",
    "failed",
    "unknown",
];
pub const BACKENDS: &[&str] = &[
    "hypridle-logind",
    "gnome-session",
    "kde-powerdevil",
    "macos-iokit",
    "windows-power-request",
];
pub fn public_schema() -> serde_json::Value {
    let mut schema =
        serde_json::to_value(schemars::schema_for!(Status)).expect("schema serializes");
    fn require_all(v: &mut serde_json::Value) {
        if let Some(props) = v.get("properties").and_then(|p| p.as_object()) {
            let names: Vec<_> = props.keys().cloned().collect();
            v["required"] = serde_json::json!(names);
        }
    }
    require_all(&mut schema);
    if let Some(defs) = schema["$defs"].as_object_mut() {
        for def in defs.values_mut() {
            require_all(def);
        }
    }
    schema["properties"]["schema_version"]["const"] = serde_json::json!(1);
    schema["$defs"]["Observation"]["properties"]["scope"]["const"] =
        serde_json::json!("registered_local_herdr_roots_and_endpoints");
    schema["$defs"]["Inhibition"]["properties"]["reason"]["enum"] = serde_json::json!(REASONS);
    schema["$defs"]["Inhibition"]["properties"]["request_state"]["enum"] =
        serde_json::json!(STATES);
    let mut backends: Vec<serde_json::Value> =
        BACKENDS.iter().map(|s| serde_json::json!(s)).collect();
    backends.push(serde_json::Value::Null);
    schema["$defs"]["Inhibition"]["properties"]["backend"]["enum"] = serde_json::json!(backends);
    let counts = [
        "working_agents_observed",
        "servers_tracked",
        "servers_current",
        "servers_unknown",
        "unknown_agents_observed",
    ];
    let mut live_counts = serde_json::Map::new();
    let mut dead_counts = serde_json::Map::new();
    for key in counts {
        live_counts.insert(key.into(), serde_json::json!({"type":"integer"}));
        dead_counts.insert(key.into(), serde_json::json!({"type":"null"}));
    }
    dead_counts.insert(
        "oldest_snapshot_age_ms".into(),
        serde_json::json!({"type":"null"}),
    );
    dead_counts.insert("work".into(), serde_json::json!({"const":"unknown"}));
    dead_counts.insert("complete".into(), serde_json::json!({"const":false}));
    let mut dead_native = serde_json::Map::new();
    for key in [
        "desired",
        "backend",
        "resource_owned",
        "release_in_ms",
        "retention_remaining_ms",
        "last_error",
    ] {
        dead_native.insert(key.into(), serde_json::json!({"type":"null"}));
    }
    dead_native.insert("reason".into(), serde_json::json!({"const":"unavailable"}));
    dead_native.insert(
        "request_state".into(),
        serde_json::json!({"const":"unknown"}),
    );
    schema["allOf"] = serde_json::json!([{"if":{"properties":{"available":{"const":true}}},"then":{"properties":{"monitor":{"type":"object"},"control":{"type":"object"},"diagnostics":{"type":"object"},"error":{"type":"null"},"observation":{"properties":live_counts},"inhibition":{"properties":{"desired":{"type":"boolean"}}}}},"else":{"properties":{"monitor":{"type":"null"},"control":{"type":"null"},"diagnostics":{"type":"null"},"error":{"type":"object"},"observation":{"properties":dead_counts},"inhibition":{"properties":dead_native}}}}]);
    schema
}
impl Status {
    /// Every external response passes this boundary before typed deserialization.
    /// Additive fields are allowed; missing required nullable fields are not.
    pub fn from_wire(value: serde_json::Value) -> anyhow::Result<Self> {
        fn fields(v: &serde_json::Value, names: &[&str]) -> anyhow::Result<()> {
            let o = v
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("status object required"))?;
            anyhow::ensure!(
                names.iter().all(|n| o.contains_key(*n)),
                "missing required status field"
            );
            Ok(())
        }
        fields(
            &value,
            &[
                "schema_version",
                "available",
                "error",
                "monitor",
                "control",
                "observation",
                "inhibition",
                "diagnostics",
            ],
        )?;
        fields(
            &value["observation"],
            &[
                "scope",
                "work",
                "complete",
                "working_agents_observed",
                "servers_tracked",
                "servers_current",
                "servers_unknown",
                "unknown_agents_observed",
                "oldest_snapshot_age_ms",
            ],
        )?;
        fields(
            &value["inhibition"],
            &[
                "desired",
                "reason",
                "backend",
                "resource_owned",
                "request_state",
                "release_in_ms",
                "retention_remaining_ms",
                "last_error",
            ],
        )?;
        if !value["monitor"].is_null() {
            fields(
                &value["monitor"],
                &[
                    "instance_id",
                    "pid",
                    "app_version",
                    "uptime_ms",
                    "snapshot_seq",
                    "evaluated_at_unix_ms",
                ],
            )?;
        }
        if !value["control"].is_null() {
            fields(&value["control"], &["paused", "pause_persisted"])?;
        }
        if !value["diagnostics"].is_null() {
            fields(
                &value["diagnostics"],
                &["issues", "known_limitations", "counters"],
            )?;
            let required = serde_json::to_value(Counters::default())?;
            let names: Vec<_> = required
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            fields(&value["diagnostics"]["counters"], &names)?;
        }
        let s: Self = serde_json::from_value(value)?;
        anyhow::ensure!(s.schema_version == 1, "incompatible public schema");
        anyhow::ensure!(
            s.observation.scope == "registered_local_herdr_roots_and_endpoints",
            "invalid scope"
        );
        anyhow::ensure!(
            REASONS.contains(&s.inhibition.reason.as_str())
                && STATES.contains(&s.inhibition.request_state.as_str()),
            "invalid inhibition enumeration"
        );
        anyhow::ensure!(
            s.inhibition
                .backend
                .as_deref()
                .is_none_or(|s| BACKENDS.contains(&s)),
            "invalid backend"
        );
        anyhow::ensure!(
            s.available == s.monitor.is_some()
                && s.available == s.control.is_some()
                && s.available == s.diagnostics.is_some(),
            "inconsistent availability"
        );
        anyhow::ensure!(s.available == s.error.is_none(), "inconsistent query error");
        let counts = [
            s.observation.working_agents_observed,
            s.observation.servers_tracked,
            s.observation.servers_current,
            s.observation.servers_unknown,
            s.observation.unknown_agents_observed,
        ];
        if s.available {
            anyhow::ensure!(
                counts.iter().all(Option::is_some) && s.inhibition.desired.is_some(),
                "live status needs counts and policy intent"
            );
        } else {
            anyhow::ensure!(
                counts.iter().all(Option::is_none)
                    && s.observation.oldest_snapshot_age_ms.is_none()
                    && s.observation.work == Work::Unknown
                    && !s.observation.complete,
                "unavailable is not empty work"
            );
            anyhow::ensure!(
                s.inhibition.desired.is_none()
                    && s.inhibition.resource_owned.is_none()
                    && s.inhibition.backend.is_none()
                    && s.inhibition.reason == "unavailable"
                    && s.inhibition.request_state == "unknown"
                    && s.inhibition.release_in_ms.is_none()
                    && s.inhibition.retention_remaining_ms.is_none()
                    && s.inhibition.last_error.is_none(),
                "unavailable is not a native ownership fact"
            );
        }
        Ok(s)
    }
}
