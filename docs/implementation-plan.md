# Implementation plan: Herdr idle inhibitor

Status: owner-authorized implementation contract. The physical Windows/macOS gates were amended as recorded in [implementation-notes.md](implementation-notes.md); no commit, publication, plugin installation, or power-policy change is authorized. The owner additionally accepted the precise PowerDevil suppression/owner-death/reallow exception described below.

The product decisions in [the baseline](baseline-plan.md) and [scope record](scope-decisions.md) are binding. Technical defaults below were authorized with execution of the full plan; they do not add product features. Evidence is in [architecture-evidence.md](research/architecture-evidence.md); H1–H4, L1–L4, M1, W1–W2, and D1 refer to its sections. The [grill ledger](grill-session.md) records how the decisions were reached; [CONTEXT.md](../CONTEXT.md) defines terms.

## 1. Decision surface and go/no-go

### 1.1 Selected architecture

| Decision | Selection and reason |
| --- | --- |
| Runtime | One Rust executable, one background monitor per OS user on the native host. Multiple short-lived bootstrap/status commands and an optional popup process use that monitor. No second runtime binary, privileged helper, installed OS service, or watchdog. |
| State authority | Successful `agent.list` snapshots from local Herdr servers. Manifest event hooks request earlier snapshots; they do not supply authoritative work state. Two-second reconciliation covers lost/coalesced hints. This avoids maintaining per-pane socket subscriptions and their ordering traps (H3). |
| Aggregation | The shared monitor discovers/registers local servers, aggregates work, owns the native request, and serializes shared controls. Separate per-server daemons would complicate global Pause, JSON status, and ownership without a demonstrated benefit. |
| Controls | One persisted Pause setting, effective for every observed server. Pause lasts until Resume, including across monitor restarts. Herdr's plugin enable/disable remains the installation/lifecycle control; do not add a second equivalent Enabled toggle to our settings. |
| Native integration | Separate Hypridle/logind, GNOME, KDE, macOS, and Windows adapters. Select a verified environment profile; do not use one broad cross-platform library whose fallback may keep the display on or block manual sleep. |
| Public API | Only `herdr-idle-inhibitor status --json`, one response and exit. Internal local IPC is needed for the popup and bootstrap; it is not a public extension protocol. No `watch`, callbacks, HTTP, or network listener. |
| Persistent data | Small user configuration and bounded diagnostic logs. No database, activity journal, durable agent cache, inhibitor state files, or automatic migration framework. |
| Failure | Unknown observations never become invented `working` or authoritative zero. Retain an already owned request briefly after losing previously observed work; never acquire solely because observation is unknown. |
| Windows exception | The user accepted documentation of possible sleep on Modern Standby hardware running on battery. Use ordinary native requests. No special power-model detector/state machine, periodic renewal to evade Windows policy, or workaround (W1). |

### 1.2 What can be concluded now

- **Go for the proposed architecture at planning level:** load-bearing public APIs and Herdr integration points have been identified, and the documented Windows battery/model contradiction was explicitly resolved. Direct API documentation does not establish a lock-specific restriction; the separate Windows locking blocker was an overinterpretation of another product's help page and has been withdrawn (W2). This selects the design; it does not certify the unimplemented application.
- **Not a runtime support claim:** native profile experiments are the first implementation gate, particularly GNOME manual/lid behavior and Windows display-off/locked behavior. A failed gate stops that backend rather than changing the product silently.
- **Build permission granted:** the owner subsequently authorized full execution, with the documented verification amendment. Claims still depend on actual evidence, not plan completion.
- **No hidden high-availability promise:** an application crash releases its native resources except for the explicitly accepted PowerDevil suppressed-request defect. Herdr does not supervise a detached monitor. Recovery occurs at the next startup/event/window bootstrap; there is no finite restart guarantee while a long-running agent emits no further events. A stopped/frozen process is different from a terminated one: OS-owned inhibitors can remain held until that process resumes or is terminated. Error deadlines assume the controller is running; they are not kernel-enforced leases. A requirement for guaranteed crash/hang recovery would require a newly approved supervisor design, not another hidden process.

### 1.3 Explicit low-risk defaults for veto

Pause persists until Resume; release delay defaults to 5 seconds; observation-loss retention is fixed at 30 seconds; the popup uses keyboard-first Ratatui/Crossterm; runtime/config paths are per-user application paths independent of the invoking workspace; initial activation and update maintenance are explicit. These defaults do not add lid/display/manual-sleep modes.

## 2. Invariants and deployment boundary

1. Count only the literal Herdr status `working`. `blocked`, `idle`, and `done` do not independently justify inhibition. `unknown` is uncertainty, not active work.
2. Acquire from a current, valid positive observation, without an intentional acquisition debounce. A hook alone is insufficient evidence.
3. Release only this application's resource. Never call a system suspend/shutdown operation, `ForceUnInhibitAll`, synthetic-input API, screen-awake API, Away Mode, or a global power-setting writer.
4. Closing the popup or a status command cannot stop monitoring or release/acquire an inhibitor.
5. Pausing does not stop observations. An external consumer must still see working agents while inhibition is paused.
6. Acquisition intent, native ownership, native acknowledgment, and actual OS sleep behavior are separate facts. No UI/JSON boolean called `guaranteed_awake` exists.
7. Loss of one server cannot erase work still reported by another. A read failure cannot be deserialized as an empty list.
8. Old request/connection generations cannot overwrite current state. Returned resources from canceled acquisitions must be released.
9. The monitor holds no resource across its own termination. Graceful release and OS cleanup after an ungraceful kill are separately tested. **Owner-approved KDE exception:** PowerDevil 6.7.5 can retain a user-suppressed cookie after the monitor dies and reactivate it on later allowance; retain KDE support and document this exact upstream limitation, without hidden helpers or a blanket cleanup waiver. See [source evidence and acceptance](research/kde-suppression-cleanup.md).
10. The supported deployment is an unelevated native host process. No remote-host orchestration, WSL-to-Windows bridge, container-to-host power bridge, or privileged service is implied.

All local Herdr servers of the current OS user are the observation target, regardless of active workspace, pane focus, or attached client. Discovery covers the public session lists of registered Herdr roots plus explicitly registered local endpoints. A custom endpoint becomes known through its plugin bootstrap or the documented additional-endpoint setting. No available API proves that an arbitrary, unregistered server hidden under another custom root does not exist; report coverage honestly rather than scanning other users or the entire filesystem.

The reference desktop deployment has one active OS desktop session, with any number of Herdr servers. Concurrent multi-seat/mixed-desktop login buses are not assumed equivalent to multiple Herdr sessions. Detect conflicting registered desktop contexts and report the coverage/backend issue; do not silently switch a live request between unrelated buses. A verified multi-seat extension would need its own acceptance case, not speculative machinery now.

## 3. Processes, ownership, and local IPC

```text
Herdr servers A, B, ...
  startup/event hooks -> short bootstrap ----+
  action -> popup process ------------------+--> one per-user monitor
external status --json ---------------------+       |
                                                    +-- discovery / snapshots
                                                    +-- decision state and controls
                                                    +-- one selected native backend
```

The popup and public status command never talk directly to Herdr for agent state. This prevents competing snapshots, native ownership, and misleading cross-server counts.

### 3.1 Executable roles

| Role | Invocation | Behavior |
| --- | --- | --- |
| Public query | `status --json` | Connect to an existing monitor, request its current view, print one JSON object, exit. Never bootstrap. No second public management surface is required. |
| Bootstrap | `_ensure` | Internal manifest command. Read minimal invocation context, connect or start the monitor, register the endpoint/root, request a coalesced refresh, and exit. |
| Owner | `_serve` | Internal foreground process role launched detached by bootstrap. Own singleton claim, local listener, state, and native resources. Not a system service. |
| Herdr action | `_open` | Ensure activation, then open the declared Herdr popup entrypoint through `HERDR_BIN_PATH`. Surface `ui_busy`; do not replace someone else's popup. |
| Popup | `_ui` | Render status/settings and send bounded private control requests. Exit on close; do not terminate the owner. |

Underscore roles are implementation details, not another stable public management API. All command execution uses argument arrays, not constructed shell strings. Strip full plugin-context/event-payload environment variables when spawning the long-lived owner; ordinary child environment inheritance must not retain selected text or event metadata just because the IPC registration was minimal. Preserve only the necessary discovery/desktop environment context. Daemon stdin is null; stdout/stderr go to its own diagnostic sink, never to a Herdr hook's still-open pipes. On Unix create a new session; on Windows use appropriate detached/no-console creation semantics. Verify parent-exit and Herdr-lifetime behavior natively rather than assuming Unix detachment works on Windows.

### 3.2 Singleton and cleanup

Concurrent hooks from multiple Herdr servers are a real race, so one OS-backed exclusive file lock is justified. A PID file alone is not ownership.

- Unix rendezvous: a short, private `/tmp/herdr-idle-inhibitor-<uid>/` directory, mode `0700`, with `owner.lock` and `control.sock` (socket mode `0600`). Verify owner/type and reject symlink substitution. Resolve the OS's legitimate `/tmp` alias before checking the application's own directory.
- Windows rendezvous: a current-user LocalAppData runtime directory for `owner.lock`, and a named pipe whose stable name includes the current user's SID. Set a current-user/SYSTEM DACL at creation; do not rely on the default pipe ACL or a TCP localhost port.
- Acquire the exclusive lock before reclaiming a stale socket. Never remove the lock file on normal exit: unlinking a locked inode permits a second unrelated lock owner.
- A loser of the startup race connects to the winner. If the owner cannot answer within the bootstrap deadline, return an activation error; do not start a second authority or kill a PID from a file.
- Only the lock owner may remove its stale rendezvous socket. Unexpected file types or ownership are errors, not permission to delete.
- Native descriptors/handles and the singleton claim must not be inherited by discovery subprocesses. Test this explicitly.

Use Interprocess 2.4.x naming compatible with Herdr for Herdr connections (H3/D1); our own private endpoint has a separate namespace. Authenticate Unix peers by UID where available and Windows peers/ACLs by SID. The threat boundary is other OS users, not hostile code already running with the same privileges as the user. Run unelevated; no elevation flow is included.

### 3.3 Private messages

One newline-delimited JSON request/response per connection, with `protocol_version`, string `request_id`, and a tagged operation. Operations are `GetStatus`, `RegisterAndRefresh`, `SetPaused`, `ApplySettings`, and `ReloadSettings`. A popup may obtain an extended private diagnostic view, including paths and per-server rows; those are not required public API fields.

Bound frames to 1 MiB, requests to 2 seconds, and concurrent clients to 16. Reject invalid versions, IDs, types, and oversize input without changing state. Mutating acknowledgments describe actual application/persistence success; a retry of `SetPaused(true)` is idempotent. No arbitrary-method forwarding, command callbacks, shell evaluation, or durable request queue.

One controller task owns mutable application state. I/O tasks deliver typed results tagged with server/request generations. Native work is serialized; slow I/O must not freeze the status/control listener. This is a small actor-style event loop, not a distributed coordinator or multi-writer database.

## 4. Discovery and lifecycle

### 4.1 Registration and discovery

1. Bootstrap sends its exact local `HERDR_SOCKET_PATH`, trusted absolute `HERDR_BIN_PATH`, plugin identity/root, and the minimal environment needed to list that Herdr configuration root. Do not copy selected text, the full event JSON, or the full plugin context into the monitor.
2. Validate that the endpoint belongs to the current user's local transport. There is no remote-machine selector or SSH connection in this adapter.
3. Run the registered Herdr executable's `session list --json` at activation and every 5 seconds per distinct root. Respect its root-selection environment, not the daemon's accidental current workspace. Bound subprocess lifetime/output and reap on timeout. Do not spawn Herdr for every agent poll.
4. Union discovered endpoints, bootstrap endpoints, and configured additional local endpoints. Deduplicate by normalized native endpoint identity, not session display name. Do not lowercase Unix paths or invent a Windows name transform.
5. A new endpoint gets `ping`, installation eligibility checks as applicable, and an initial `agent.list`. Record observed Herdr version; initially qualify `0.9.3`. Ignore additive fields, reject incompatible envelopes, and treat unfamiliar status values as uncertainty. Do not gate the JSON API on binary protocol constant `22` (H3).
6. The monitor owns the root/discovery context. Later popups inherit that shared view rather than creating a monitor/configuration for their workspace.

Multiple independent Herdr installation roots can register with the same monitor. Their settings still come from one application configuration. At least one freshly verified enabled installation is required for a new acquisition; an existing request may survive a temporarily failed eligibility check only within the bounded eligibility rule below. Disabling one of several independently installed copies does not erase the other copy; global Pause remains the explicit cross-root control. In the normal single-root case, Herdr's plugin disable/unlink stops the monitor.

### 4.2 Initial activation, disable, and shutdown

- Manifest startup hooks run `_ensure` after API readiness. Event hooks cover `pane.agent_status_changed`, `pane.agent_detected`, `pane.exited`, `pane.closed`, and `workspace.closed`.
- **Install/enable into an already running server:** opening the Idle Inhibitor action once is the required immediate activation step. Otherwise activation waits for a qualifying event or server startup. Do not claim the startup hook runs on enable (H1).
- **Registry checks:** read `plugin.list` every 5 seconds for installation roots. A valid disabled/missing installation result removes that root's activation eligibility immediately, without work-release grace. If no enabled roots remain, close resources and exit. Failure to read eligibility is uncertainty, not disabled/empty; retain permission to hold an existing resource for at most 30 seconds from the last verified enabled result, but do not authorize a new acquisition from that retained permission. Repeated failures do not renew it. If no valid/retained eligibility remains, release/stop rather than run indefinitely after removal.
- **No running servers:** after successful discovery and the bounded retirement of disconnected endpoints, release normally and exit after 30 seconds with no live servers. This permits a brief handoff without keeping a permanent background service.
- **All observation/discovery lost:** inhibition retention expires separately at 30 seconds. If no useful connection/discovery recovers for 120 seconds, exit. A subsequent bootstrap starts a fresh monitor.
- **App Pause:** release but stay alive and keep observing while Herdr is present. Pause is not Herdr plugin disable.
- **Crash/kill:** OS-owned resources disappear. A later hook or window-open starts a new monitor; no stale agent state is loaded from disk. Persisted Pause still applies.
- **Update/uninstall:** disable the plugin, close its popup, and verify the monitor/resource has stopped before replacing/removing binaries. This is required on Windows, where an executing file can be locked. Re-enable and open the action after update. No zero-gap live-update guarantee, runtime binary-copy manager, self-updater, or forced process termination is included.

## 5. Observation model and timing

### 5.1 Data model

| Type | Essential fields and meaning |
| --- | --- |
| `ServerKey` | Normalized local endpoint identity. Session name is display metadata, not identity. |
| `ConnectionGeneration` | Monotonic local generation incremented after disconnect/reconnect/handoff detection. Never a guessed global Herdr event cursor. |
| `AgentKey` | `(ServerKey, terminal_id)`; pane/workspace projections are not separate count identities. |
| `AgentObservation` | Literal status and timestamp of a successful snapshot. Do not store title, cwd, tokens, prompt text, or terminal output. |
| `ServerObservation` | Generation, last request ID, successful snapshot time, connectivity/parse health, and normalized agent map. |
| `Coverage` | Registered/discovered roots/endpoints, whether initial scope discovery is established, discovery age/errors, live/unknown/stopped counts. An uninitialized empty set is not complete coverage. |
| `Controls` | Persisted Pause and validated configuration. No per-server Pause. |
| `Decision` | Desired request, reason, release deadline, and bounded retention-only deadline. |
| `NativeState` | Backend, locally owned resource, latest request state, diagnostic error, backend generation. Not a guarantee of OS behavior. |

The minimal per-agent DTO requires string `terminal_id` and string `agent_status`; pane/workspace IDs and other projection metadata are not needed for this policy. Deserialize only those identity/status fields. `agent.list` can unavoidably carry additional title/cwd/token metadata on the wire; skip it while parsing and promptly discard the raw frame. Do not request `agent.read` or terminal-output APIs, or claim that unwanted metadata was never received.

Deduplicate repeated projections of one terminal. Conflicting statuses for the same terminal within one snapshot are an invalid observation, not a reason to choose a convenient value. Apply a validated snapshot atomically for that server. A missing agent in a successful complete snapshot is no longer reported by that server; a missing array or malformed response is not an empty snapshot.

### 5.2 Snapshots plus hints

- Poll `agent.list` over the native local transport every 2 seconds per connected server, with at most one request in flight per server and eight across servers.
- Hooks trigger a refresh, coalesced over at most 100 ms. If a hint arrives during a read, mark a dirty bit and do exactly one follow-up read after completion. Repeated hints do not create an unbounded work queue.
- Never apply hook payload status directly. Snapshot completion wins over the timing of an old hook, and generation/request checks discard old I/O results.
- Periodic reads remain active even if hooks disappear. There are no `events.subscribe` streams, resubscription trees, replay cursors, or a guessed initial event snapshot.
- Read replies with a 1-second I/O deadline and an 8 MiB frame ceiling. Reject truncation, mismatched IDs/result types, and malformed required fields. Ignore irrelevant/additive fields. Apply finite backoff after repeated transport errors.
- A successful read's age is data, not an assertion that no transition occurred immediately afterward. No polling/event system can prevent sleep that already began before it observed work.

### 5.3 Timing defaults

| Item | Default | Contract |
| --- | --- | --- |
| Snapshot interval | 2 s | Monitored even while paused. |
| Hook coalescing | 100 ms maximum | No extra intentional acquisition delay after the resulting positive snapshot. |
| Discovery / eligibility check | 5 s | Per distinct root, bounded concurrency. |
| Observation freshness | 6 s | Older data is not current work evidence. Explicit read failure marks uncertainty immediately. |
| Discovery / eligibility freshness | 10 s | A failed check becomes uncertain immediately; otherwise this age limit prevents an unscheduled/missed check staying current forever. |
| Normal release grace | 5 s, configurable 0–60 s | Begins on confirmed end of the last work; unchanged idle snapshots do not restart it. |
| Lost-observation retention | 30 s from last valid positive evidence | Existing ownership only; unknown/error messages do not renew it. |
| API/private IPC timeout | 1 s / 2 s | Timeout is an error, never empty state. |
| Bootstrap readiness | 3 s | IPC readiness/registration, not a promise that snapshots or native activation have completed. |
| Lost eligibility retention | 30 s from last verified enabled result | Existing ownership only; failed checks do not renew permission or authorize reacquisition. |
| Native operation deadline | 2 s where the API supports cancellation | A timeout cannot be mislabeled a successful release/acquisition. Clean up the owning connection/handle and expose uncertainty until resolved. |
| Reconnect/acquisition retry | 1, 2, then 5 s maximum | Only retry acquisition while fresh work still requires it; no retry that defeats a user's KDE suppression. |
| All-servers-gone exit | 30 s | Requires successful discovery/retirement, not a single failed read. |
| Entire observation unavailable exit | 120 s | The inhibitor has already been released after its shorter error bound. |
| Popup refresh | 1 s | Reads the monitor cache, not Herdr. |

Intervals are scheduling targets, not hard real-time guarantees under overload or OS suspension. Missed freshness is reported as uncertainty; it does not silently extend an evidence deadline. A release deadline withdraws continuation intent and starts cleanup; it is not proof that an asynchronous native release already completed. Keep ownership/releasing/unknown facts truthful until cleanup is confirmed.

Use a testable elapsed-time clock that includes suspend for observation ages and retention deadlines (Linux `CLOCK_BOOTTIME`, macOS continuous time, Windows uptime clock). Wall-clock timestamps are for human correlation only. A large scheduling/resume gap invalidates stale observations before they can be used for reacquisition. Tokio wakeups schedule work; they are not the sole authority on the age of data after suspend. These clocks solve ordinary freshness, not Windows power-policy circumvention.

A wake/large execution gap is not proof that servers stopped or the plugin was disabled. Invalidate earlier in-flight read generations, release any expired/invalid retained ownership, and immediately schedule discovery, eligibility and snapshots. Allow one bounded initial revalidation round (at most 5 seconds) before applying shutdown rules based on pre-gap ages. This does not renew an inhibition grace or permit acquisition from old evidence. Fresh successful results restore normal operation; real failures then follow the normal bounded exit rules.

### 5.4 Decision table

Evaluate global controls/eligibility first, then current positive evidence, then bounded retention/release. Fresh enabled-installation evidence permits acquisition or continuation. Unexpired retained eligibility permits continuation of already owned resources only. With neither permission, desire release regardless of work. Eligibility and work retention are separate bounds: permission to continue cannot outlive either applicable deadline, release is requested when the first applicable bound expires, and native loss cannot turn retained eligibility into permission to reacquire.

| Condition | Decision |
| --- | --- |
| Paused, explicitly disabled/unlinked, or shutting down | Request release immediately; do not wait for normal/error grace. Continue observations only for Pause. |
| At least one fresh `working` agent, after the eligibility check above | Acquire or continue as allowed by that eligibility check, even if a different server is uncertain. Work is `working`, coverage may be partial; retained eligibility only permits an already owned resource to continue. |
| Last known work explicitly ends in successful snapshots, with no fresh work or still-retainable uncertain positive evidence elsewhere | Start the one normal release deadline if a resource is owned. Work is `none` if observation is complete, otherwise `unknown`. Never acquire just to provide grace. |
| Transport failure or `unknown` following verified work, with a resource already owned | Work is `unknown` unless another server has fresh work. Retain only until that evidence's nonrenewable 30-second deadline; show degraded observation. |
| Unknown state with no retainable positive evidence | Do not acquire. An already running normal release grace may finish but cannot be renewed; otherwise request release now. Work stays `unknown`, not `none`. |
| No fresh work, no valid grace/retention deadline | Desired inhibition is false; release any owned resource. This exhaustive default also covers complete empty/nonworking startup observations and expired error retention. |
| Native acquisition fails | Preserve observed work, report unprotected/error, retry with bounded backoff while fresh work remains. |
| New work during release grace | Cancel release and keep the existing resource; no release/reacquire churn. |

A successful explicit nonworking observation clears that agent's prior positive evidence. If A explicitly finishes while B is unknown but B never supplied positive evidence, B cannot keep A's request indefinitely: only A's already justified normal grace applies. A later error cannot resurrect ended work or extend a five-second end-of-work grace into thirty seconds. Retention is derived from per-agent/server evidence, not repeatedly resetting one global "last error" timer. A return from Pause cannot acquire on unknown state even if a pre-Pause request once existed.

Retire a disconnected endpoint only after successful discovery says it is not running, transport absence is consistent, and the bounded loss interval has elapsed. Permission errors, malformed replies, and hung APIs are not proof of server exit. Fresh work from a different endpoint remains sufficient throughout.

## 6. Native backend contracts

### 6.1 Adapter boundary

Provide one small `PowerBackend` abstraction for `probe`, `acquire`, explicit `release`, and native-status/loss notifications. A returned owned resource has a backend generation and a cleanup guard. Fake implementations exist for tests; this is not a loadable backend/plugin framework.

The controller decides **whether** to inhibit; adapters decide **how** to represent that request without expanding its meaning. All reasons are fixed, non-sensitive strings such as `Herdr agents are working`.

Cookie-based D-Bus ownership requires a dedicated owning bus connection. A timed-out acquisition may have succeeded on the service before its reply was lost: close that connection to remove uncertain cookies. A shared, permanently live bus connection would leak the uncertain request. On release, send `Uninhibit`/`ReleaseInhibition`; if that fails, close the owning connection. Do not keep hidden connection clones alive in unrelated tasks. Explicit async cleanup comes before guard fallback; a Rust destructor alone cannot await D-Bus cleanup. Kill tests verify the independent service-side cleanup path.

Native results carry an intent generation. If Pause, shutdown, a backend change, or newer observations make an in-flight acquisition obsolete, release its late result instead of publishing it as active. Never acquire twice because a retry overlapped an outstanding call. Track service name-owner changes and resource loss; invalidate a cookie from a previous service instance.

### 6.2 Hyprland / Hypridle

Reference: Hypridle `0.1.8`, systemd/logind; L1–L2.

- Call `org.freedesktop.login1.Manager.Inhibit` with `what="idle"`, a fixed application/reason, and `mode="block"`. Own the returned FD and set/verify close-on-exec.
- Never request `sleep`, `shutdown`, or `handle-lid-switch`; no `systemd-inhibit` subprocess is required.
- User integration must keep `general.ignore_systemd_inhibit = false`; display dim/off and lock listeners use `ignore_inhibit = true`, while the actual idle-suspend listener continues honoring inhibition. Explain that the per-listener override ignores inhibitors generally, not only this plugin.
- Preserve unrelated listener commands and timing. Instructions provide a reviewed example and a checklist, not an automatic config writer. `ignore_dbus_inhibit` is not a replacement for `ignore_systemd_inhibit`.
- Before explicit integration acknowledgment, report `setup_required` and do not acquire an inhibitor that could silently hold the display on. Acknowledgment is user confirmation, not proof that arbitrary includes/variables were statically evaluated. Runtime acceptance must demonstrate the actual behavior.
- Observe logind restart/resume signals without taking a `sleep` delay lock. Reconcile after resume; do not veto or delay manual/lid sleep.

### 6.3 GNOME

Reference: gnome-session and gnome-settings-daemon `51.0`; L3.

- Call `org.gnome.SessionManager.Inhibit(app_id, 0, reason, 4)`, retain its cookie and owning connection, and release with `Uninhibit`.
- Do not request flag `8` (session idle) or combine flags for logout/user switching. In the reference source, flag `4` blocks automatic suspend/hibernate without being the screen-blanking flag; forwarding to logind is `sleep` / `block-weak`, and GNOME's explicit suspend path skips inhibitors.
- Qualify an actual GNOME version/profile; do not silently use this path on older strong-forwarding implementations. `gnome-session --version` and available D-Bus interfaces provide initial diagnostics, not proof of arbitrary distribution patches. Record exact session/settings-daemon/systemd package versions in the native evidence.
- The release's compatibility table lists verified profiles. An unrecognized or incompatible profile produces `unsupported`/actionable diagnostics, not an automatic fallback to a strong logind `sleep` lock.
- Native acceptance must include idle suspend, screen blank/lock, the normal GNOME sleep action, a normal explicit sleep command, and lid policy under that profile. If only the GUI action works but an ordinary explicit sleep request is blocked by our lock, the profile fails.

### 6.4 KDE Plasma / PowerDevil

Reference: PowerDevil `6.7.5`; L4.

- Use `org.kde.Solid.PowerManagement.PolicyAgent` at `/org/kde/Solid/PowerManagement/PolicyAgent` on `org.kde.Solid.PowerManagement`.
- `AddInhibition(InterruptSession = 1, app_name, reason)` returns the owned cookie. Do not request `ChangeScreenSettings = 4`. Release only that cookie with `ReleaseInhibition`.
- Query/watch `RequestedInhibitions` and `ActiveInhibitions` for the stable plugin-specific application/reason pair. The singleton rule avoids multiple legitimate owners using that pair. Missing required properties mean an unqualified profile, not permission to infer activation from elapsed time.
- A cookie exists before the five-second activation timer completes. Report `pending` until the service reports the matching active request. The real gap remains visible; do not add a stronger temporary lock to bridge it.
- If the user suppresses the request, report `suppressed`. Keep the existing cookie state and respond to the user's later allowance; do not change the reason string or repeatedly reacquire to evade suppression.
- Explicit and lid actions bypass the automatic policy in the inspected implementation. Verify this on the target profile, including screen locking and a PowerDevil restart.

### 6.5 macOS

Reference: M1.

- Call `IOPMAssertionCreateWithName` with `kIOPMAssertionTypePreventUserIdleSystemSleep`, assertion level on, and the fixed reason. Balance it with `IOPMAssertionRelease`.
- Keep CoreFoundation strings and assertion ownership correct across FFI; distinguish creation failure from a valid assertion ID. The OS's process-exit cleanup, not just `Drop`, is the crash guarantee.
- Do not request display/user-active assertions, call `pmset`, change `SleepDisabled`, or install a helper.
- Idle display-off and lock are allowed; lid, explicit sleep, critical battery and OS safety policy may still suspend. Test ordinary wake/re-observation and assertion ownership after resume. Do not add a sleep-veto callback just to track resume; elapsed-time freshness prevents stale snapshots from remaining current.

### 6.6 Windows

Reference: W1–W2 and [the accepted exception](scope-decisions.md#accepted-windows-limitation).

- `PowerCreateRequest` creates the process-owned object; call `PowerSetRequest(PowerRequestSystemRequired)` once for a desired acquisition. Release with the matching `PowerClearRequest` and `CloseHandle`. Account for API request counters; do not increment once per poll.
- Use the fixed Unicode reason with correct `REASON_CONTEXT` lifetime. Do not request `PowerRequestDisplayRequired`, Away Mode, or `PowerRequestExecutionRequired` as a speculative benefit to unrelated Herdr agent processes.
- Register ordinary suspend/resume callbacks with `PowerRegisterSuspendResumeNotification` / `DEVICE_NOTIFY_CALLBACK`. They enqueue events and return promptly. They neither delay sleep nor change a power plan. Unregister safely before freeing callback context.
- User sleep can terminate requests while the object handle remains open. Mark request certainty lost on suspend, invalidate old observations, and on resume recreate/reconcile the ordinary request after a fresh positive snapshot. No recurring refresh intended to defeat power policy.
- Use the ordinary request across workstation locking; do not clear/recreate it solely because of Win+L or introduce a service/lock-state listener for that case. The API documentation names no lock-triggered cancellation. Display-off/locked-session behavior remains an ordinary mandatory native acceptance test, not a separate unresolved architecture blocker inferred from PowerToys help (W2).
- Windows policy can independently ignore application system-required requests (`SYSTEMREQUIRED`); respect it. A successful request does not prove effective enforcement, and this application does not rewrite that policy or add continuous policy monitoring.
- Always document the possible Modern Standby-on-battery limitation in Windows help and compatibility notes. A static JSON known-limitation identifier describes that conditional caveat; it must not imply detection of the current power source/model. Do not add AC/DC switching logic or model-specific workarounds for this exception.

### Accepted KDE cleanup exception

During P4 the owner explicitly chose to retain KDE and accept the source-verified PowerDevil 6.7.5 suppression/owner-death/reallow limitation. This supersedes the unconditional crash-cleanup guarantee only for that reachable upstream scenario. Normal explicit release, pending ownership, service-loss handling, user suppression, and other process-exit cleanup still require tests. Do not claim that disconnecting repairs the lost owner mapping. See [the finding](research/kde-suppression-cleanup.md).

### 6.7 Selection and environmental failure

On Linux, combine the registered desktop context, service availability, and qualified profile; prefer the responsible desktop power manager. `linux.backend` can resolve an ambiguous environment but cannot bypass safety/profile checks. No arbitrary fallback chain `GNOME -> sleep lock -> screen lock` exists. Unknown versions/permissions, missing bus services, required setup, and user suppression are distinct diagnostics.

Only one selected backend owns a request. A user-applied backend/configuration change releases the old resource and re-evaluates; report the gap rather than promise uninterrupted protection during maintenance. Backend loss does not erase observed agent work.

## 7. Configuration, persistence, and privacy

### 7.1 Paths and configuration ownership

Use native per-user application directories, outside the managed plugin checkout:

| Platform | Configuration | Logs/state |
| --- | --- | --- |
| Linux | `${XDG_CONFIG_HOME:-$HOME/.config}/herdr-idle-inhibitor/config.toml` | `${XDG_STATE_HOME:-$HOME/.local/state}/herdr-idle-inhibitor/` |
| macOS | `~/Library/Application Support/herdr-idle-inhibitor/config.toml` | `~/Library/Logs/herdr-idle-inhibitor/` |
| Windows | Known Folder LocalAppData, `herdr-idle-inhibitor/config.toml` | Same application directory, `logs/` and private `runtime/` |

This deliberately gives the shared application one configuration independent of workspace, named session, or alternate Herdr configuration roots. Herdr's injected plugin paths describe that installation; they are not additional independent settings authorities. Bootstrap/status/popup clients discover the active owner's selected path through IPC and never initialize competing files. Environment-based path overrides take effect at owner startup, not per popup.

The configuration contains preferences, not cached work or claimed native ownership. Proposed version 1:

```toml
schema_version = 1
paused = false
release_delay_secs = 5
additional_endpoints = []

[linux]
backend = "auto" # auto | hypridle | gnome | kde
hypridle_integration_confirmed = false
```

`additional_endpoints` is an explicit escape hatch for local custom endpoints the public session discovery cannot enumerate; it is not a remote address list. Native transport ownership checks still apply. Show it as advanced configuration, not the normal onboarding flow.

### 7.2 Validation and writes

- The monitor is the only application writer. Popup commands change typed fields; they do not write files directly or replace an entire stale settings object.
- Missing configuration gets validated defaults when the monitor is explicitly bootstrapped, never during `status`.
- Reject unsupported schema versions, unknown setting names, invalid types, invalid backend names, nonlocal endpoints, and release delays outside 0–60 seconds. Do not clamp a typo into a different policy.
- Invalid configuration at startup is visible and prevents acquisition until corrected. A failed explicit reload keeps the last valid runtime configuration and reports that the file was rejected. Do not overwrite the user's invalid file automatically.
- Save via a same-directory temporary file and replacement, with user-only permissions. The single-writer design needs no general transaction engine; safe replacement is only to avoid a truncated config after a crash. A failed write must not be reported as persisted.
- Pause applies immediately in memory and requests release even if persistence fails; prominently report `pause_not_persisted` and that restart may restore the last saved value. Resume requires a successful save before enabling new acquisition. Repeated commands are idempotent.
- Manual edits are loaded at startup or explicit Reload in Settings. No file watcher/hot-reload subsystem. Before an app save, detect an externally changed file and ask the user to reload instead of silently overwriting it; do not implement multi-writer merging.
- No migration machinery before an actual second schema exists. Future incompatible versions must get an explicit migration design and fixtures.

### 7.3 Diagnostics and privacy

Store only state transitions, bounded/rate-limited errors, version/profile information, and counters. Rotate `monitor.log` at 1 MiB, retaining one previous file. Keep no raw agent responses, event payloads, selected text, prompt/token data, or terminal contents in production logs. OS-visible inhibitor reasons are fixed strings.

Useful counters: discovery attempts/failures, snapshots by trigger (`initial`, `poll`, `hook`, `resume`), valid/failed snapshots, ignored old generations, hints received/coalesced, native acquire/release/failure counts, backend losses, and status reads. Counters are per monitor instance and reset on restart. They prove which operation ran; elapsed time alone is not sufficient evidence.

Public status omits session names, endpoints, pane/terminal IDs, workspace titles, and filesystem paths. The private popup may display sanitized session names and diagnostic paths for the local user. Escape terminal control characters in all externally supplied names/errors. Capture minimal real traffic for tests only with explicit authorization, redact it before adding fixtures, and distinguish captures from synthetic examples.

## 8. User interface and public status

The exact public fields/errors are specified in [status-api.md](status-api.md). Do not let the popup invent another definition of `working`, grace, or backend state.

### 8.1 Popup

Declare a Herdr popup entrypoint rather than a permanent panel. `_open` uses the supported plugin pane-open command, and `_ui` runs Ratatui/Crossterm in the popup terminal. Restore terminal modes on ordinary exit/error/panic, handle resize and small terminals, and never change Herdr's theme or layout globally.

```text
Idle Inhibitor
Work:          Working — 3 observed agents / 2 servers
Observation:   Complete
Request:       Accepted — macOS idle-sleep assertion
Control:       Running

[Pause]  [Settings]  [Details]  [Close]
```

These rows are separate. Required alternative states include `No work`, `Unknown`, `Paused`, `Releasing in 4s`, `Retained after observation loss`, `Setup required`, `Pending (KDE)`, `Suppressed by desktop`, `Native request failed`, and `Monitor unavailable`. Windows wording is `Native request accepted`, not an unconditional `Protected` claim.

- Keyboard: Tab/Shift-Tab/arrows navigate; Enter/Space activates; Escape/q closes the popup; p toggles Pause/Resume; s opens Settings; d opens Details. No hidden global power shortcut.
- Settings exposes persistent Pause, release delay, Linux backend choice, and Hypridle integration instructions/acknowledgment as applicable. Advanced endpoint editing may be through the documented TOML plus Reload rather than building a general text editor.
- Show `Pause applies to every local Herdr server and remains until Resume`. Paused observation continues. Failed persistence is not hidden behind a success toast.
- Details shows per-server health/age and the selected native profile/error. Windows Help includes the static Modern Standby battery warning. It does not need to poll power-profile changes.
- If the monitor dies, show unavailable; do not infer no work. An explicit Retry/activate action may bootstrap it. Passive refresh/status must not silently restart it.
- `ui_busy` or no attached Herdr UI produces an actionable error. Do not create a permanent fallback panel or choose a different attached user's client.

### 8.2 Status-only behavior

`status --json` has a two-second total query deadline. It prints exactly one versioned JSON object and newline, with diagnostics on stderr only. It never starts a monitor, registers a Herdr session, creates directories/configuration, refreshes Herdr, acquires/releases a native request, or writes a status cache. The owner may increment an in-memory query counter; that is not a policy action.

A disconnected/stopped monitor yields an unavailable result and nonzero exit, not `working_agents_observed = 0`. A live monitor with partial observation or a backend error still returns a valid status response; consumers must inspect its fields. External consumers poll and own their own behavior. This project provides no lid policy example that implicitly authorizes sleep on `unknown`.

## 9. Rust structure and dependencies

Use one Cargo package with a small library and one executable; OS-specific modules are compile-time gated. No workspace of tiny crates is justified initially.

```text
src/
  main.rs                  CLI role dispatch
  lib.rs
  model.rs                 typed observations, coverage, controls, native facts
  policy.rs                pure decision reducer and deadlines
  clock.rs                 test clock + suspend-aware elapsed time
  controller.rs            one state owner, scheduling and result generations
  herdr/
    transport.rs           native framing, request IDs, bounded reads
    protocol.rs            minimal needed DTOs, tolerant additive fields
    discovery.rs           public CLI listing and registration
    monitor.rs             per-server serialized snapshots/reconnect
  runtime/
    bootstrap.rs           detached start, registration, bounded readiness
    singleton.rs           OS lock, endpoint ownership/ACLs and cleanup
    ipc.rs                 private typed request/response protocol
    paths.rs               per-user roots, no cwd dependence
    config.rs              validation, explicit reload, single-writer saves
  backend/
    mod.rs                 small backend interface, owned resource/results
    linux_hypridle.rs
    linux_gnome.rs
    linux_kde.rs
    macos.rs
    windows.rs
  status.rs                public v1 projection and exit/error mapping
  ui.rs                    popup and settings, no independent power policy
  diagnostics.rs           bounded logs and observable counters
```

Start with only modules justified by these boundaries; splitting a long file later is not an architecture change. Do not mirror all Herdr types or depend on its entire application crate. Avoid copying third-party implementation code just because it was inspected as evidence.

Dependency intent:

- Tokio for one async runtime and process/I/O scheduling; Serde/serde_json for explicit DTOs; TOML for configuration; Clap for CLI roles.
- Interprocess `2.4.x` with appropriate async/native local-socket support, matching the inspected transport naming. Exact versions go in `Cargo.lock`.
- Linux-only zbus `5.x` with Tokio and default async-io disabled; small rustix/libc surface for credentials/FD flags/elapsed time as needed.
- macOS-only `core-foundation` for owned strings and a small audited FFI module for the exact public IOKit assertion creation/release functions. Pin SDK/deployment target and explain each unsafe boundary; no Objective-C UI framework is required.
- Windows-only `windows-sys` bindings for power requests, notifications, handles, SID/ACL handling and elapsed time. No helper executable or Windows service.
- Ratatui/Crossterm for the on-demand TUI; tracing with a bounded writer for diagnostics. Standard/OS file locking, not a distributed-lock package.
- Test-only fixtures, a controllable clock, fake native resources, temporary directories, and a local fake Herdr transport. Add property/state-machine tests where sequences are genuinely useful; no framework is needed for simple table tests.

Pin a released Rust toolchain, exact dependency resolution, and CI actions before implementation acceptance. The research host's `rustc 1.98.0` is an observation, not an already validated MSRV promise. Select/pin the toolchain at P0 after the native build smoke checks; do not leave release builds on a moving `stable` alias.

## 10. Packaging and supported profiles

Proposed initial artifact targets: Linux x86_64 GNU, macOS aarch64 and x86_64, Windows x86_64 MSVC. Linux/Windows ARM64 are not claimed merely because Rust can cross-compile them; adding them requires a native Herdr/backend acceptance entry. These architecture choices are defaults for review, not a user-requested platform removal.

- Plugin ID proposal: `herdr-idle-inhibitor`; executable of the same name; initial version `0.1.0`; `min_herdr_version = "0.9.3"`; manifest platforms `linux`, `macos`, `windows`.
- One source-build command: `cargo build --release --locked --target-dir target` in the plugin root. Manifest commands use the native `./target/release/herdr-idle-inhibitor` or Windows `.exe` path through platform-specific entries.
- Startup and each listed event invoke `_ensure`; action `show` invokes `_open`; the popup invokes `_ui`. Command values are argv arrays, not shell-dependent launch scripts.
- A prebuilt plugin bundle uses the same entrypoint layout and includes documentation/license/checksums. Its generated manifest omits source `[[build]]` steps; otherwise installation could unexpectedly require Cargo despite shipping a binary. Verify that all other entrypoints/metadata match the source manifest. A source install needs Rust/Cargo; a prebuilt installation/runtime does not need Cargo, Node.js, Python, `jq`, or a power-management CLI. Herdr itself and the named desktop's normal services remain prerequisites.
- Herdr installing a plugin does not automatically put its executable on the user's PATH. Document the executable's absolute path for external callers; an optional user-created PATH entry/symlink is a convenience, not an automatic shell-configuration edit. Test `status --json` without any `HERDR_*` environment variables.
- Build against an explicitly recorded OS/SDK/libc baseline. P0 fixes the exact runner/image/toolchain/deployment versions and native test machines; do not claim an unspecified `macOS 10+` or all Windows versions. The mandatory desktop reference profiles are Hypridle 0.1.8, GNOME 51, and PowerDevil 6.7.5 until other versions are qualified.
- Keep installation, first activation, Hypridle setup, Pause semantics, updates, recovery after crashes, uninstall, and the Windows limitation explicit in the user guide. Running an unsupported profile must be visible.
- No automatic updates or elevated install commands. On macOS, unsigned/quarantined distribution behavior must be documented/tested; signing/notarization is a distribution gate if the intended channel requires it, not something a Linux cross-build proves. Windows execution warnings and architecture mismatch need similar installation tests.
- Repository publishing identity, license choice, signing credentials, release permissions, and marketplace submission are publication prerequisites owned by the project owner. Do not silently choose a license, upload artifacts, or publish a marketplace listing as part of implementation.

The ordered test-first steps, concrete acceptance matrix, independent-review gates, and exact stop rules are in [implementation-validation.md](implementation-validation.md). That document is part of this plan, not an optional later testing wish list.

## 11. Deviation and handoff policy

At implementation kickoff, create `docs/implementation-notes.md` with the approved plan snapshot, decisions made, planned-versus-actual deviations, newly discovered unknowns, and verification results. It is a work record, not a replacement for the glossary or accepted scope.

- For a local, low-risk change that preserves public behavior and ownership (for example a compatible dependency patch or a module split), record the reason/evidence and continue with regression checks.
- Stop and ask before changing work classification, grace/retention semantics beyond the approved defaults, public JSON meaning, platform scope, privileges, persistent data, process/supervision topology, publication cost, or any user-visible sleep behavior. A failing native profile cannot be "fixed" with a stronger inhibitor.
- If fresh source/docs contradict this plan, prefer that evidence, write the exact contradiction and affected requirements, and reopen the relevant decision. Do not force code to follow a falsified assumption or quietly rewrite the historical user decision.
- Before any authorized delegation, provide a bounded launch packet: goal, agreed constraints, exact files/source revisions to read freshly, remaining risks, permitted edits, test/evidence artifacts, and stop conditions. Independent review is not permission to expand scope or accept a claim without checking it.
- Each phase ends with its gate result or a named blocker. Present uncommitted work for acceptance; commit/publish only when explicitly authorized.
