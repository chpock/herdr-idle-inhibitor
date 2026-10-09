# Public status API, version 1

[Home](../README.md) · [Usage](usage.md) · [JSON Schema](../schemas/status-v1.json)

This is the implemented, read-only JSON interface for external consumers. Native request acknowledgment is not a physical sleep guarantee; see [Compatibility](compatibility.md). Herdr does not add the executable to PATH: use [your installation's executable path](installation.md#external-executable-path).

## Command and side effects

```text
herdr-idle-inhibitor status --json
```

A syntactically valid invocation writes one UTF-8 JSON object followed by one newline and exits. No banners, progress output, ANSI sequences, streaming, or additional stdout lines. A caller gets a snapshot, not a transaction that freezes Herdr or the OS.

The query only reads an already running per-user monitor. It does not create application directories/files, bootstrap/restart/register anything, query Herdr, validate a desktop through a new power request, or acquire/release an inhibitor. An in-memory diagnostic read counter may advance. The command has a total two-second deadline; no retry loop that outlives it.

There is no public `watch`, outbound command hook, HTTP endpoint, or remote address option. Internal local IPC can evolve independently as long as this public projection remains compatible.

## Result shape

All top-level fields listed below are required, including explicit `null` values when unavailable. Field order is not significant. Unknown additive fields must be ignored by consumers.

| Field | Type | Meaning |
| --- | --- | --- |
| `schema_version` | integer, exactly `1` | Public projection version, independent of app version and Herdr's binary wire version. |
| `available` | boolean | A live compatible monitor answered this query. It does not mean observation is complete or inhibition is effective. |
| `error` | issue or null | Query-level failure only. A live backend/observation problem belongs in diagnostics and does not make the query unavailable. |
| `monitor` | object or null | Instance identity and evaluation metadata. |
| `control` | object or null | Current shared Pause state and its persistence status. |
| `observation` | object | Work and confidence; remains present with unknown/null values when unavailable. |
| `inhibition` | object | Desired policy and separately reported native facts. |
| `diagnostics` | object or null | Non-sensitive issues, static known limitations, and per-instance counters. |

An **issue** is `{ "code": "stable_machine_code", "message": "Human-readable explanation" }`. Codes are extensible; consumers must not parse the message for policy. Public issues exclude arbitrary upstream payloads, paths, agent text and terminal escape sequences.

### Monitor and controls

`monitor` contains:

- `instance_id`: random identifier generated at owner startup; never reused deliberately and not a credential.
- `pid`: diagnostic integer, not an ownership proof or permission to kill that process.
- `app_version`: executable version string.
- `uptime_ms`: nonnegative elapsed time since this monitor started.
- `snapshot_seq`: local controller evaluation sequence. It resets with the instance; it is not a Herdr event cursor or persistent revision.
- `evaluated_at_unix_ms`: wall-clock correlation time of that evaluation. Clock adjustments can move it backward; it is not used for freshness decisions.

`control` contains `paused: boolean` and `pause_persisted: boolean`. `pause_persisted = false` reports a live Pause that could not be saved; a restart can restore the previous saved setting. A successful persisted Pause remains until an explicit Resume. Pause never means agents stopped working.

### Observation

| Field | Type | Meaning |
| --- | --- | --- |
| `scope` | string | `registered_local_herdr_roots_and_endpoints`. Never implies remote machines, other OS users, or exhaustive discovery of unregistered custom roots. |
| `work` | `working`, `none`, or `unknown` | Defined below; never infer it from inhibitor ownership. |
| `complete` | boolean | Known discovery roots are current/successful, all relevant running endpoints have current valid snapshots, and no unclassified/unknown agent status remains. Completeness is relative to the declared scope. |
| `working_agents_observed` | nonnegative integer or null | Deduplicated fresh `working` terminals. With incomplete observation it is a lower bound, not an exact machine-wide total. Null when the monitor is unavailable. |
| `servers_tracked` | nonnegative integer or null | Endpoints under active observation, including bounded disconnected/uncertain retention; confirmed stopped/retired endpoints are excluded. Not a count of workspaces, attached clients, or proven live processes. |
| `servers_current` | nonnegative integer or null | Endpoints with current successful snapshots. |
| `servers_unknown` | nonnegative integer or null | Endpoints without a trustworthy current observation; may include a disconnected endpoint before bounded retirement. |
| `unknown_agents_observed` | nonnegative integer or null | Unknown/unrecognized status values within otherwise valid current snapshots. It does not guess the number of agents on an inaccessible server. |
| `oldest_snapshot_age_ms` | nonnegative integer or null | Age of the oldest retained successful snapshot among relevant endpoints, including a stale one. Null if none exists. Uses suspend-aware elapsed time, not wall time. |

Work rules:

1. At least one fresh, deduplicated `working` observation -> `working`, even if another endpoint is unknown.
2. Zero working observations **and** complete observation -> `none`.
3. Otherwise -> `unknown`.

A complete empty discovered set can establish `none` while the monitor is waiting to exit. An uninitialized monitor with no completed scope-discovery round, a newly registered root awaiting verification, or failed/stale discovery cannot: they remain incomplete and, without fresh positive evidence, `unknown`. A retained native request during an error does not make work `working`. Unknown, unavailable, partial observation, Pause, and a failed power request must never collapse into the same `false` boolean.

### Inhibition

| Field | Type | Meaning |
| --- | --- | --- |
| `desired` | boolean or null | Last evaluated policy intent. It can be true while acquisition/setup fails, or false while native release is still completing. Null without a monitor. |
| `reason` | string | One of `working`, `release_grace`, `observation_grace`, `no_work`, `paused`, `ineligible`, `shutdown`, `unknown`, `unavailable`. |
| `backend` | string or null | `hypridle-logind`, `gnome-session`, `kde-powerdevil`, `macos-iokit`, `windows-power-request`; null before selection/unavailable. |
| `resource_owned` | boolean or null | The monitor knows it owns its local native resource. True is not a claim that the OS must continue honoring the request. Null when ownership cannot be determined or no monitor answered. |
| `request_state` | string | `none`, `pending`, `accepted`, `releasing`, `suppressed`, `failed`, or `unknown`. |
| `release_in_ms` | nonnegative integer or null | Remaining normal release grace, only while that deadline applies. |
| `retention_remaining_ms` | nonnegative integer or null | Remaining bounded observation-loss retention, only while it applies. |
| `last_error` | issue or null | Current native/setup/profile problem, not an old failure retained after successful recovery. |

`accepted` means the most recent relevant native acknowledgment supports the request. For KDE it additionally requires matching active-policy observation rather than merely obtaining a cookie. For Windows it does not establish that a hardware/power-policy exception will not terminate the request. No `effective = true` or `protected = true` guarantee is exposed.

Examples of valid distinctions:

- Fresh work + Pause: work `working`, desired false, resource false after cleanup.
- Fresh work + native denial: work `working`, desired true, resource false, request failed.
- KDE activation timer: desired true, resource true, request pending.
- Lost server after work: work unknown, resource may remain true for bounded observation grace.
- Confirmed idle during normal grace: work none, desired true until the release deadline.

### Diagnostics

`diagnostics` contains:

- `issues`: an array of issues, for example `discovery_failed`, `snapshot_stale`, `invalid_config`, `setup_required`, `unsupported_profile`, `backend_unavailable`, `desktop_suppressed`, `pause_not_persisted`.
- `known_limitations`: an array of static conditional identifiers. Windows includes `windows_modern_standby_battery`; the selected KDE adapter includes `kde_powerdevil_suppressed_owner_cleanup`. These describe [platform limitations](compatibility.md), **not** detection of the current power source, hardware model or an orphaned request. Other adapters currently use an empty list; consumers allow unknown additive identifiers.
- `counters`: nonnegative per-instance integers for `status_reads`, `discovery_attempts`, `discovery_failures`, `snapshot_initial`, `snapshot_poll`, `snapshot_hook`, `snapshot_resume`, `snapshot_valid`, `snapshot_failed`, `old_results_ignored`, `hints_received`, `hints_coalesced`, `native_acquires`, `native_releases`, `native_failures`, `backend_losses`. Additions are allowed; counters reset with `instance_id`.

Counter meanings are operational: `snapshot_*` origins count launched reads; `snapshot_valid` counts validated current-generation snapshots committed to observation; `snapshot_failed` counts read/validation failures; `old_results_ignored` counts discarded stale-generation results rather than committed snapshots. `hints_coalesced` counts hints absorbed without a separate scheduled read. `native_acquires` counts successful ownership acquisition (including a KDE pending cookie), `native_releases` counts relinquished owned resources, and `native_failures` counts failed adapter operations, not a policy refusal to acquire without work/setup. A terminated process cannot increment counters; its last counter values are not proof of native cleanup.

A read may age the returned observation view but cannot use that read as a trigger for Herdr I/O or a policy action. `snapshot_seq`/evaluation metadata and native facts refer to the monitor's last evaluation; diagnostic query counts can advance independently. Scheduled timers, not public readers, perform deadline transitions.

## Illustrative successful response

Example of a successful response with current work and a native request:

```json
{
  "schema_version": 1,
  "available": true,
  "error": null,
  "monitor": {
    "instance_id": "example-instance-1",
    "pid": 12345,
    "app_version": "0.1.0",
    "uptime_ms": 40000,
    "snapshot_seq": 17,
    "evaluated_at_unix_ms": 1800000000000
  },
  "control": {"paused": false, "pause_persisted": true},
  "observation": {
    "scope": "registered_local_herdr_roots_and_endpoints",
    "work": "working",
    "complete": true,
    "working_agents_observed": 3,
    "servers_tracked": 2,
    "servers_current": 2,
    "servers_unknown": 0,
    "unknown_agents_observed": 0,
    "oldest_snapshot_age_ms": 500
  },
  "inhibition": {
    "desired": true,
    "reason": "working",
    "backend": "macos-iokit",
    "resource_owned": true,
    "request_state": "accepted",
    "release_in_ms": null,
    "retention_remaining_ms": null,
    "last_error": null
  },
  "diagnostics": {
    "issues": [],
    "known_limitations": [],
    "counters": {
      "status_reads": 4,
      "discovery_attempts": 8,
      "discovery_failures": 0,
      "snapshot_initial": 2,
      "snapshot_poll": 18,
      "snapshot_hook": 1,
      "snapshot_resume": 0,
      "snapshot_valid": 21,
      "snapshot_failed": 0,
      "old_results_ignored": 0,
      "hints_received": 2,
      "hints_coalesced": 1,
      "native_acquires": 1,
      "native_releases": 0,
      "native_failures": 0,
      "backend_losses": 0
    }
  }
}
```

## Unavailable response

An unavailable response has `available = false`, a query-level issue, and `monitor/control/diagnostics = null`. Observation/inhibition objects remain present with:

- observation: the declared scope, `work = "unknown"`, `complete = false`, all numeric/count/age fields null;
- inhibition: `desired/resource_owned/backend = null`, reason `unavailable`, request state `unknown`, both deadlines and native `last_error` null.

Never synthesize a stopped monitor as zero working agents or a successfully released native request. The OS may still have another application's inhibitor, and the query has no authority over it.

## Exit codes and compatibility

| Exit | Meaning |
| --- | --- |
| `0` | A compatible monitor answered with a valid projection, including paused/partial/unknown/backend-error states. Check fields. |
| `1` | Output/internal command failure, such as an unwritable stdout; do not expect complete JSON if it could not be written. |
| `2` | CLI usage error. Standard help/usage may be stderr-only; malformed invocations are not successful machine queries. |
| `3` | No available monitor / connection closed without returning response payload. JSON error code `monitor_unavailable`. |
| `4` | Query deadline expired. JSON error code `query_timeout`. |
| `5` | Permission, framing, protocol-version, or other query I/O failure. JSON code identifies `permission_denied`, `protocol_error`, `incompatible_monitor`, or `query_io_error`. |

An empty reply is unavailable; a partial/malformed/oversized reply is a protocol failure. Neither is a successful empty observation. Stable field meaning, type, nullability and the three work values cannot change within schema v1. Additive fields/counters/issue codes are allowed; consumers ignore additions and conservatively reject unknown incompatible versions. A breaking change requires a new schema version, not merely a changed app version.

The [JSON Schema](../schemas/status-v1.json) describes the v1 output. Validate the envelope and inspect work, observation, controls and native facts independently; do not reduce every unavailable/unknown/paused/failed state to the same `false` result.
