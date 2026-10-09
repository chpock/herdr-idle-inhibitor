# Architecture evidence for the full grill

Status: source/documentation evidence, not a runtime certification. Read alongside the [grill ledger](../grill-session.md). Planning does not authorize installing this plugin, changing power settings, disabling other inhibitors, or actually suspending this machine.

## Source identity and freshness

The load-bearing files were freshly read during the grill. Stable desktop checkouts were inspected after an initial upstream-main exploration; findings below use the stable commits, not an assumption that upstream main is installed locally.

| Source | Revision used |
| --- | --- |
| Herdr | `0.9.3`, `7b116c05bfda646af39d2524c54e70c751f57ee8` |
| gnome-session | `51.0`, `aed00cf18229a98231c23e14f5798ddb918c4956` |
| gnome-settings-daemon | `51.0`, `7f7913cc655b7c7b924d8f277177f95ac32bcd49` |
| KDE PowerDevil | `v6.7.5`, `5627bacf05394a737f1dd50295b187748150ffb1` |
| Installed Hypridle | `0.1.8`, source `e5c01af0842bd66617f7004568df9406111d6e80`; original config behavior is recorded in [the prior survey](plugin-survey.md). Must not substitute latest Hypridle semantics. |
| Apple PowerManagement | `d415e45501842834a280930c3eed9186544a67f0`; process-exit assertion cleanup was freshly inspected, not assumed from Rust destructors. |
| PowerToys Awake follow-up | `0825b72f96075e34c85c87da58820247698c30d0`; Manager, AwakeStateCalculator and SessionStateController checked to avoid transferring product-help claims to another API. This is a source revision, not an installed/released-version claim. |
| Installed Herdr | Read-only `herdr --version` returned `herdr 0.9.3`. `herdr session list --json` returned one running default server and its local API endpoint. No agent workload or power-policy experiment was performed. |

The research checkouts live under `/tmp/herdr-idle-inhibitor-research/`; that is disposable research storage, not a build dependency. The links below are the durable source identity. Microsoft/Apple/library documentation URLs are live documentation, not immutable source releases.

## H1. Herdr lifecycle is not daemon supervision

Sources: [plugin startup/environment/popup documentation](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/docs/next/website/src/content/docs/plugins.mdx#L233-L323), [enable/disable implementation](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/app/api/plugins/mod.rs#L713-L744).

- Startup hooks run after the API is ready and on live handoff, not on plugin link/enable or merely attaching a UI client.
- The documented hook contract is initialize and exit; Herdr does not supervise a daemon started by a hook.
- Enable/disable updates the registry and response; it does not implement a daemon start/stop protocol.
- Unlink removes plugin records; existing plugin panes may keep running. A background child must not be assumed to die when a registry entry is removed.
- `plugin.list` refreshes the registry before returning enabled state. This is an available read-only lifecycle observation, not a plugin-disabled event invented by this project.

**Design consequence:** make bootstrap short-lived and idempotent, define initial activation explicitly, and monitor registry state. Document the recovery boundary rather than implying that an ordinary detached process is supervised. Do not bind inhibitor ownership to the popup.

## H2. Local servers, paths, and current-user discovery

Sources: [session discovery/path rules](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/session.rs#L161-L226), [configuration/state roots](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/config/io.rs#L30-L94), [plugin user directories](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/plugin_paths.rs#L15-L31).

- `herdr session list --json` enumerates the default server and named session directories under that invocation's Herdr configuration root. Fields include `name`, `default`, `running`, `socket_path`, and `session_dir`.
- `running` is derived from endpoint existence and an attempted connection, not proof of an authoritative agent snapshot or of a different server incarnation.
- `HERDR_SOCKET_PATH` can select a custom endpoint; standard session discovery is not an exhaustive filesystem/process scan.
- A named session does not imply a separate plugin configuration/state root.
- Runtime invocations provide `HERDR_BIN_PATH`, endpoint, plugin config/state paths, and plugin identity. User data belongs outside the managed source checkout.

**Design consequence:** combine discovery through the public CLI with registration of the invoking endpoint. Explicitly retain discovery coverage and failure state. Do not infer an empty machine from a failed discovery or assume arbitrary custom configuration roots can be discovered magically.

## H3. Wire protocol, identity, and subscription traps

Sources: [native local transport](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/ipc.rs#L35-L79), [client request behavior](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/client.rs#L50-L120), [response envelope](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/schema/response.rs#L24-L49), [agent schema](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/schema/agents.rs#L186-L229), [event schemas](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/schema/events.rs#L11-L85), [subscription initialization](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/subscriptions.rs#L206-L237), [stream acknowledgment and polling](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/server.rs#L909-L954), [vanished-pane setup regression test](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/server.rs#L1773-L1819).

- Herdr exchanges newline-delimited JSON, with a string request ID and tagged result/error envelopes. Ordinary calls use a separate connection, not an assumed multiplexed JSON-RPC connection.
- Unix uses `GenericFilePath`; native Windows uses `GenericNamespaced` with the endpoint path's lossy-string name. The Windows `.sock` path is also a marker file, not a Unix-domain socket. Reuse the documented Interprocess name mapping; do not invent a hash or connect to a TCP port.
- `AgentInfo` distinguishes `terminal_id`, `pane_id`, `workspace_id`, `agent_status`, `state_change_seq`, and `revision`. A server-scoped terminal is the useful count identity; an agent conversation is not the same thing.
- Status-change subscriptions require an individual `pane_id`. There is no schema-supported wildcard subscription to every agent's status.
- A subscription may fail altogether if one pane vanished during setup. A startup acknowledgment is a tagged success response; following messages are event envelopes.
- The public event envelope does not carry the server's internal event-hub sequence. `state_change_seq` in a snapshot is not a global event-stream cursor. Per-subscription iteration also means wire event order is not a global transaction order.
- Subscription initialization probes current pane state; with no status filter it is not an initial full snapshot.
- The [manifest hook allowlist](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/schema/events.rs#L286-L329) includes `pane.agent_status_changed`, `pane.agent_detected`, `pane.closed`, `pane.exited`, and `workspace.closed`. Unlike socket status subscriptions, these manifest hooks need no per-pane subscription installation. High-volume `pane.updated` is not a permitted manifest hook.
- The [reported binary protocol constant](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/protocol/wire.rs#L1-L20) is `22`; its source identifies it as the server/client binary wire contract. Do not invent a JSON-API compatibility gate by requiring that unrelated constant to stay unchanged.
- The [agent status enum](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/schema/common.rs#L158-L166) is exactly `idle`, `working`, `blocked`, `done`, `unknown`. The list result is `agent_list` with an `agents` array.

**Design consequence:** snapshots are the state authority. Use events only to request a coalesced refresh, not to overwrite a newer snapshot with an unversioned event. The selected simple design uses manifest event hooks plus periodic snapshots, so it does not need a dynamic per-pane `events.subscribe` stream. Preserve per-server request/generation ordering and reject malformed/truncated/oversized replies. If a stream is proposed later, its snapshot/subscribe/post-ack ordering is a new design obligation, not an already solved property.

## H4. The plugin must not invent agent activity

Sources: [agent enumeration](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/app/agents.rs#L23-L37), [agent projection](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/app/agents.rs#L366-L402).

The API enumerates agent terminals projected through this server's workspaces/panes. It is not a promise to enumerate every subprocess, detached task, or remote machine. Duplicate projections of one terminal must not inflate agent counts. The actual `AgentInfo` schema includes additional title/cwd/token fields, so an `agent.list` frame may already contain them. Parse only identity/status, discard unused raw data promptly, never persist it, and do not call prompt/terminal-output APIs merely to decide whether `agent_status == working`.

**Design consequence:** local means the queried server/endpoint, not an inference about where every command inside a terminal uses CPU. Do not follow remote-machine transports or reimplement agent-specific detection. Use real recorded agent workflows to verify the boundary Herdr actually reports.

## L1. logind is a mechanism, not a universal desktop contract

Source: [systemd inhibitor documentation](https://systemd.io/INHIBITOR_LOCKS/).

- `idle` suppresses inactivity policy; `sleep` is a separate high-level suspend/hibernate inhibitor.
- The returned descriptor owns the inhibitor; it remains until all descriptor duplicates close. Do not leak it into children.
- `block-weak` differs from `block`: it does not affect root or the user owning the lock. This does not retroactively change older desktop implementations.
- `PrepareForSleep(false)` can notify resume without taking a delay lock. A pre-sleep callback without a delay lock is not a reliable cleanup transaction; this project must not add a sleep-delay inhibitor just to observe resume.
- Permission denial is possible. Never retry through `sudo` or silently substitute a stronger inhibitor.

**Design consequence:** prefer the power manager responsible for the idle action. A resource's successful acquisition cannot alone prove display and manual-sleep behavior.

## L2. Hypridle requires explicit listener integration

Evidence: [installed Hypridle/source/config survey](plugin-survey.md).

For the inspected setup, logind idle inhibition suppresses ordinary Hypridle listeners. Display dim/off and lock listeners must explicitly ignore inhibition, while the idle-suspend listener must continue honoring it. Broad sleep inhibition is not an acceptable replacement.

**Design consequence:** document the exact user-applied listener changes for the verified version; distinguish configuration acknowledgment from verified runtime behavior. Do not parse a few text matches and claim to have evaluated arbitrary includes/variables. Missing required integration must produce an actionable setup state, not silently hold the display awake.

## L3. GNOME behavior is version-sensitive

Sources: [SessionManager Inhibit contract](https://gnome.pages.gitlab.gnome.org/gnome-session/re04.html#gdbus-method-org-gnome-SessionManager.Inhibit), [GNOME 51 action-to-inhibitor mapping](https://github.com/GNOME/gnome-settings-daemon/blob/7f7913cc655b7c7b924d8f277177f95ac32bcd49/plugins/power/gsd-power-manager.c#L1469-L1501), [GNOME 51 logind forwarding](https://github.com/GNOME/gnome-session/blob/aed00cf18229a98231c23e14f5798ddb918c4956/gnome-session/gsm-systemd.c#L538-L558), [explicit suspend implementation](https://github.com/GNOME/gnome-session/blob/aed00cf18229a98231c23e14f5798ddb918c4956/gnome-session/gsm-systemd.c#L388-L403).

- Flag `4` is the suspend inhibitor, while flag `8` inhibits session idle. Using `8` to mean only host idle sleep conflates screen idle with system sleep.
- In GNOME 51, the automatic suspend/hibernate path checks the suspend flag; screen blanking uses the idle flag.
- GNOME 51 forwards the suspend flag to logind as `sleep` / `block-weak`, not a strong `block` inhibitor.
- GNOME's explicit suspend path uses `SuspendWithFlags` with `SD_LOGIND_SKIP_INHIBITORS`.
- The documented cookie must be released; disconnecting from the session bus also removes the application's inhibition.

**Design consequence:** a version/capability-qualified GNOME backend can request only flag `4`, preserving idle display behavior in this implementation. Do not call it a universally idle-only GNOME API: older versions, distribution patches, other users' sessions, logind configuration, and manual/lid paths must be checked. A backend that introduces a strong manual-sleep block fails the contract; do not silently substitute it.

## L4. KDE has a real activation delay and user suppression

Sources: [FDO Inhibit mapping](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilfdoconnector.cpp#L84-98), [PolicyAgent acquisition](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilpolicyagent.cpp#L608-731), [lock-screen policy](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilpolicyagent.cpp#L529-604), [explicit-action bypass](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilaction.cpp#L55-70), [lid-to-explicit-action path](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/actions/bundled/handlebuttonevents.cpp#L142-197), [requested/active inhibition schema](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/dbus/org.kde.Solid.PowerManagement.PolicyAgent.xml).

- FDO PowerManagement inhibition maps to `InterruptSession`, not `ChangeScreenSettings`.
- The method returns a cookie before the implementation's five-second activation timer completes. A user can suppress the request by application/reason, including after activation.
- `RequestedInhibitions` and `ActiveInhibitions` expose different facts, with Active/Allowed flags. They are not cookie-indexed ownership lists; use a stable, plugin-specific application/reason pair under the singleton ownership rule and do not mistake another application's inhibitor for ours.
- Explicit actions bypass the automatic-policy check; lid handling enters that explicit path in this revision.
- Screen locking suppresses display-related inhibition, not the `InterruptSession` policy.

**Design consequence:** report pending versus active versus user-suppressed. Do not report full protection immediately after obtaining a cookie, add a stronger bridging sleep inhibitor, or defeat the user's suppression with random reason strings/repeated new requests. The backend delay is part of the acceptance contract, not an invented delay in our own acquisition logic.

## M1. macOS has the intended public assertion

Sources: [PreventUserIdleSystemSleep](https://developer.apple.com/documentation/iokit/kiopmassertiontypepreventuseridlesystemsleep), [Apple's machine-readable documentation](https://developer.apple.com/tutorials/data/documentation/iokit/kiopmassertiontypepreventuseridlesystemsleep.json), [QA1340 sleep/wake and assertions](https://developer.apple.com/library/archive/qa/qa1340/_index.html).

Apple states that `kIOPMAssertionTypePreventUserIdleSystemSleep` prevents automatic idle system sleep while the display may dim/sleep. Lid close, Apple-menu sleep, low battery, and other sleep reasons remain possible. Use an owned assertion with a non-sensitive fixed reason and balanced release, not a global power preference, screen assertion, user-activity assertion, or privileged helper.

[Apple's process-exit cleanup](https://github.com/apple-oss-distributions/PowerManagement/blob/d415e45501842834a280930c3eed9186544a67f0/pmconfigd/PMAssertions.c#L4650-L4704) explicitly releases assertions whose owning PID has exited. This matters because a killed process does not run Rust destructors.

**Design consequence:** direct IOKit/CoreFoundation bindings are appropriate. Process termination, suspend/resume, and assertion cleanup still require native verification; source semantics are not a substitute for those tests. If notifications are used, do not veto or delay the OS's forced sleep.

## W1. Windows has a documented battery/power-model limit

Sources: [PowerSetRequest parameters and remarks](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest), [Modern Standby preparation phases](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/prepare-software-for-modern-standby), [SetThreadExecutionState](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setthreadexecutionstate), [SYSTEM_POWER_CAPABILITIES](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-system_power_capabilities).

- `PowerRequestSystemRequired` addresses idle system sleep. `PowerRequestDisplayRequired` and Away Mode are not part of this product.
- `PowerRequestExecutionRequired` protects the requesting process from lifetime management; on S3 it implies SystemRequired. It is not a grant for unrelated agent processes to execute through Modern Standby.
- On Modern Standby DC power, system/execution requests terminate five minutes after the system sleep timeout expires. The No-CS phase documentation independently describes unlimited request blocking on AC versus a maximum five minutes on DC, followed by suspension of desktop applications.
- User-initiated sleep, lid close, and power-button sleep terminate ordinary requests. A still-open request handle must not be presented as proof that Windows continues honoring a request. Resume needs a new observation and explicit request-state handling.
- `SetThreadExecutionState` is another idle-sleep API, not documented evidence that the Modern Standby phase limit disappears. Resetting idle timers repeatedly, pretending to play audio, or changing the power plan is not a contract-preserving remedy.
- `SYSTEM_POWER_CAPABILITIES.AoAc` can identify the S0 low-power idle model. That is useful background for controlled tests, not a required runtime feature: the user chose documentation rather than special power-model handling. Do not assume that the absence of a runtime probe proves unrestricted support.

A normal [suspend/resume callback](https://learn.microsoft.com/en-us/windows/win32/api/powerbase/nf-powerbase-powerregistersuspendresumenotification) is available to an ordinary process through `DEVICE_NOTIFY_CALLBACK`; it does not require a hidden window, privileged service, or power-profile detector. Use it for ordinary request recovery, not for working around the battery limit.

**User decision:** accept this specific limitation and document that the application may not prevent sleep in this combination. Use the ordinary native mechanism without bypasses or complicated special handling. Document the condition in Windows help/compatibility notes; a static known-limitations entry may describe it, but must not pretend that the current hardware/power source was detected. See [the scope decision](../scope-decisions.md#accepted-windows-limitation).

## W2. Windows session locking and power requests

This section corrects the earlier classification of Windows locking as an unresolved architectural feasibility blocker. The follow-up used web search, then read the actual API documentation and pinned PowerToys implementation; generated search answers were not treated as evidence.

### Direct API documentation

- [PowerSetRequest parameters](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest#parameters) defines `PowerRequestSystemRequired`: "The system continues to run instead of entering sleep after a period of user inactivity." Display retention is a separate request that this application does not make.
- Its [remarks](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest#remarks) document request termination on user-initiated sleep and the Modern Standby battery time limit. They do **not** name workstation locking as a cancellation condition or require an unlocked interactive session.
- [LockWorkStation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-lockworkstation) "locks the workstation's display" to protect it from unauthorized use. Locking is not the same operation as entering system sleep.
- [PowerCreateRequest](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powercreaterequest) exposes an owned request object to desktop applications; `CloseHandle` frees the object. Neither that contract nor `PowerSetRequest` introduces a lock-screen-specific service/SYSTEM requirement.
- [Allow system required requests](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/sleep-settings-allow-system-required-requests) is a real independent condition: the power manager can accept or ignore application requests according to `SYSTEMREQUIRED` policy (`1` accepts, `0` ignores). A successful API call is not authority to override that policy. Do not modify it or add a runtime power-policy state machine.

**Scope of the conclusion:** the documented contract supports selecting ordinary `PowerRequestSystemRequired` for idle-sleep prevention, including the normal locked-workstation use case when policy honors requests. There is no explicit per-Win+L guarantee sentence covering every configuration; absence of a listed lock exception is not a hardware test or an unconditional guarantee. It is also not evidence for inventing a lock-specific restriction or privilege requirement.

### Why the PowerToys warning is not a blocker for this API

The [Awake help page](https://learn.microsoft.com/en-us/windows/powertoys/awake#lock-screen-behavior) still claims Awake cannot maintain requests at the lock screen because of a separate security context. That product-help statement must not be promoted into a Win32 `PowerSetRequest` contract.

The inspected PowerToys revision is `0825b72f96075e34c85c87da58820247698c30d0` (2026-09-07):

- [Manager.cs](https://github.com/microsoft/PowerToys/blob/0825b72f96075e34c85c87da58820247698c30d0/src/modules/awake/Awake/Core/Manager.cs) calls `SetThreadExecutionState`, not `PowerSetRequest`; session changes reapply the selected state.
- [AwakeStateCalculator.cs](https://github.com/microsoft/PowerToys/blob/0825b72f96075e34c85c87da58820247698c30d0/src/modules/awake/Awake/Core/AwakeStateCalculator.cs) retains `ES_SYSTEM_REQUIRED | ES_CONTINUOUS` when locked and omits only `ES_DISPLAY_REQUIRED`.
- [SessionStateController.cs](https://github.com/microsoft/PowerToys/blob/0825b72f96075e34c85c87da58820247698c30d0/src/modules/awake/Awake/Core/SessionStateController.cs) maps lock/unlock notifications to that state. The [merged change](https://github.com/microsoft/PowerToys/commit/0825b72f96075e34c85c87da58820247698c30d0) deliberately allows the display to turn off while locked.

This source check establishes what Awake requests, not measured effectiveness of our chosen API. It nevertheless makes the blanket help-page explanation insufficient to infer that all ordinary user processes lose all power requests on lock. Do not add a service, elevation, lock-state listener, or different API solely to work around that explanation.

Search also returned [PowerToys issue 48965](https://github.com/microsoft/PowerToys/issues/48965). It is a user proposal involving SYSTEM context, ExecutionRequired and AC-aware handling, not an authoritative test of our unelevated SystemRequired-only design. Its repetition of the same help-page warning is not independent corroboration.

**Revised design consequence:** withdraw the separate U01 planning blocker. Keep Win+L/display-off/workload-progress verification as ordinary mandatory native acceptance within P0 and the integrated matrix, just like release, cleanup and resume. No native test is reported as passed; a real failing supported profile still requires investigation or a blocker. The accepted Modern Standby battery exception remains unchanged.

## D1. Dependency choices should match these contracts

Sources: [Interprocess local-socket dispatch/stability](https://docs.rs/interprocess/latest/interprocess/local_socket/index.html), [zbus Tokio integration](https://docs.rs/zbus/latest/zbus/#special-tokio-support).

- Interprocess uses native Unix sockets/named pipes and promises a stable name-type mapping without inserting its own framing. That matches Herdr's actual transport dependency and avoids shelling out for every agent snapshot.
- Linux zbus should use the Tokio feature with default async-io features disabled when the application already uses Tokio; no second executor is needed.
- OS-specific dependencies must be target-gated. Exact dependency versions and the toolchain must be pinned by the implementation's lockfile/CI, not inferred from mutable `latest` documentation.
- The current research machine reports Rust `1.98.0`; that observation alone does not establish the product's MSRV or a reproducible release build.

## What this review has not proved

No real GNOME/KDE/macOS/Windows power experiment, locked-session Windows trial, mixed-inhibitor trial, native build, process-kill cleanup test, or recorded agent replay has run for this application: there is no application implementation yet. The installed Stay Awake plugin and other inhibitors can invalidate an uncontrolled experiment. Source-backed candidates, compatibility exceptions, implementation tests, and release evidence must remain separate labels.
