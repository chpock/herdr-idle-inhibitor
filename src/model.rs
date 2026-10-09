use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SNAPSHOT_FRESH_MS: u64 = 6_000;
pub const CHECK_FRESH_MS: u64 = 10_000;
pub const RETAIN_MS: u64 = 30_000;
pub fn age(now: u64, then: u64) -> u64 {
    now.saturating_sub(then)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Work {
    Working,
    None,
    Unknown,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    pub terminal_id: String,
    pub agent_status: String,
}
#[derive(Debug, Clone, Default)]
pub struct ServerObservation {
    pub agents: BTreeMap<String, String>,
    pub snapshot_at: Option<u64>,
    pub failed: bool,
    pub positive: BTreeMap<String, u64>,
    pub ended_at: Option<u64>,
}
impl ServerObservation {
    pub fn commit(&mut self, agents: Vec<Agent>, now: u64) -> anyhow::Result<()> {
        let mut map = BTreeMap::new();
        for a in agents {
            anyhow::ensure!(!a.terminal_id.is_empty(), "empty terminal identity");
            if let Some(old) = map.insert(a.terminal_id, a.agent_status.clone()) {
                anyhow::ensure!(old == a.agent_status, "conflicting projections");
            }
        }
        let had_positive = self
            .positive
            .values()
            .any(|t| now < t.saturating_add(RETAIN_MS));
        self.positive.retain(|id, t| {
            now < t.saturating_add(RETAIN_MS)
                && map
                    .get(id)
                    .is_some_and(|s| s == "working" || !known_nonworking(s))
        });
        for (id, s) in &map {
            if s == "working" {
                self.positive.insert(id.clone(), now);
            }
        }
        if had_positive && self.positive.is_empty() {
            self.ended_at = Some(now);
        }
        self.agents = map;
        self.snapshot_at = Some(now);
        self.failed = false;
        Ok(())
    }
    pub fn fail(&mut self) {
        self.failed = true;
    }
    pub fn current(&self, now: u64) -> bool {
        !self.failed
            && self
                .snapshot_at
                .is_some_and(|t| age(now, t) <= SNAPSHOT_FRESH_MS)
    }
    pub fn retention_until(&self) -> Option<u64> {
        self.positive
            .values()
            .map(|t| t.saturating_add(RETAIN_MS))
            .max()
    }
}
fn known_nonworking(s: &str) -> bool {
    matches!(s, "idle" | "blocked" | "done")
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Observation {
    pub scope: String,
    pub work: Work,
    pub complete: bool,
    pub working_agents_observed: Option<u64>,
    pub servers_tracked: Option<u64>,
    pub servers_current: Option<u64>,
    pub servers_unknown: Option<u64>,
    pub unknown_agents_observed: Option<u64>,
    pub oldest_snapshot_age_ms: Option<u64>,
}
pub fn aggregate<'a>(
    servers: impl Iterator<Item = &'a ServerObservation>,
    coverage: bool,
    now: u64,
) -> Observation {
    let (mut working, mut unknown, mut total, mut current) = (0, 0, 0, 0);
    let mut oldest = None;
    for s in servers {
        total += 1;
        if let Some(t) = s.snapshot_at {
            oldest = Some(oldest.unwrap_or(0).max(age(now, t)));
        }
        if s.current(now) {
            current += 1;
            for v in s.agents.values() {
                if v == "working" {
                    working += 1;
                } else if !known_nonworking(v) {
                    unknown += 1;
                }
            }
        }
    }
    let complete = coverage && total == current && unknown == 0;
    Observation {
        scope: "registered_local_herdr_roots_and_endpoints".into(),
        work: if working > 0 {
            Work::Working
        } else if complete {
            Work::None
        } else {
            Work::Unknown
        },
        complete,
        working_agents_observed: Some(working),
        servers_tracked: Some(total),
        servers_current: Some(current),
        servers_unknown: Some(total - current),
        unknown_agents_observed: Some(unknown),
        oldest_snapshot_age_ms: oldest,
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Counters {
    pub status_reads: u64,
    pub discovery_attempts: u64,
    pub discovery_failures: u64,
    pub snapshot_initial: u64,
    pub snapshot_poll: u64,
    pub snapshot_hook: u64,
    pub snapshot_resume: u64,
    pub snapshot_valid: u64,
    pub snapshot_failed: u64,
    pub old_results_ignored: u64,
    pub hints_received: u64,
    pub hints_coalesced: u64,
    pub native_acquires: u64,
    pub native_releases: u64,
    pub native_failures: u64,
    pub backend_losses: u64,
}
