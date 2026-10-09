use crate::model::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub desired: bool,
    pub reason: String,
    pub release_in_ms: Option<u64>,
    pub retention_remaining_ms: Option<u64>,
}
impl Decision {
    pub fn off(reason: &str) -> Self {
        Self {
            desired: false,
            reason: reason.into(),
            release_in_ms: None,
            retention_remaining_ms: None,
        }
    }
}
#[derive(Debug, Default)]
pub struct Policy {
    normal_deadline: Option<u64>,
    witnessed_work: bool,
    ending_seen: Option<u64>,
}
impl Policy {
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate(
        &mut self,
        o: &Observation,
        servers: &[ServerObservation],
        now: u64,
        paused: bool,
        eligible_to_acquire: bool,
        owned: bool,
        release_secs: u64,
    ) -> Decision {
        if paused {
            self.normal_deadline = None;
            self.witnessed_work = false;
            return Decision::off("paused");
        }
        if !eligible_to_acquire && !owned {
            return Decision::off("ineligible");
        }
        if o.work == Work::Working {
            self.normal_deadline = None;
            self.witnessed_work = true;
            self.ending_seen = servers.iter().filter_map(|s| s.ended_at).max();
            return Decision {
                desired: true,
                reason: "working".into(),
                release_in_ms: None,
                retention_remaining_ms: None,
            };
        }
        if owned
            && let Some(end) = servers
                .iter()
                .filter_map(|s| s.retention_until())
                .filter(|t| *t > now)
                .max()
        {
            // A different server completing is not the end of the last retained work.
            // Consume that event now; retention expiry must not later manufacture grace.
            self.ending_seen = servers.iter().filter_map(|s| s.ended_at).max();
            self.normal_deadline = None;
            return Decision {
                desired: true,
                reason: "observation_grace".into(),
                release_in_ms: None,
                retention_remaining_ms: Some(end - now),
            };
        }
        let latest_end = servers.iter().filter_map(|s| s.ended_at).max();
        if owned && self.witnessed_work && latest_end.is_some() && latest_end != self.ending_seen {
            // Anchor to confirmed completion, never to a later error-retention expiry.
            self.normal_deadline = latest_end.map(|end| end.saturating_add(release_secs * 1000));
            self.ending_seen = latest_end;
            self.witnessed_work = false;
        }
        if owned
            && let Some(end) = self.normal_deadline
            && end > now
        {
            return Decision {
                desired: true,
                reason: "release_grace".into(),
                release_in_ms: Some(end - now),
                retention_remaining_ms: None,
            };
        }
        self.normal_deadline = None;
        if !owned {
            self.witnessed_work = false;
        }
        Decision::off(if o.work == Work::None {
            "no_work"
        } else {
            "unknown"
        })
    }
}
