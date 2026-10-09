# Baseline plan: Herdr idle inhibitor

Status: baseline product decisions are agreed. The resulting [full implementation proposal](implementation-plan.md), [status contract](status-api.md), and [verification gates](implementation-validation.md) supersede the provisional architecture below; final planning validation is recorded in [the grill ledger](grill-session.md). Implementation and commits remain unapproved.
Date: 2026-10-08.
Evidence: [prior-art survey](research/plugin-survey.md) and [pinned sources](research/plugin-survey-sources.json). The [lid/manual-sleep feasibility study](research/lid-sleep-feasibility.md) is retained as historical research, not current feature scope; its Windows idle-inhibition limitation remains relevant. See [scope decisions](scope-decisions.md) for the reason lid handling was excluded.

## Confirmed product decisions

The user confirmed these decisions after the initial survey:

1. **Linux, macOS, and Windows are all mandatory platforms.** Windows is not deferred merely because native Windows Herdr support is currently advertised as preview.
2. **The project inhibits idle-triggered host sleep only.** The screen may turn off and lock. Lid-close and manual sleep remain under normal OS/user policy, not just as a default that another plugin mode can override.
3. **Only Herdr's `working` status counts as active work.** A blocked agent waiting for the user is not independently active work. `unknown` and failed observations require a separate error policy, not an invented active state.
4. **All local Herdr servers/sessions of the current OS user and their workspaces are covered.** Protection must not depend on the selected pane, focused workspace, or an attached UI client. Other OS users' sessions are not monitored.
5. **The implementation language is Rust.** Ship native executables rather than requiring a Node.js or Python runtime.
6. **The interface is an on-demand compact text window inside Herdr.** The plugin runs in the background without occupying a permanent pane. The window provides status, working-agent/session counts, the idle-inhibition state, pause/resume, and settings. Closing the window does not disable monitoring.
7. **Lid and manual-sleep control are excluded.** This supersedes the earlier approval of optional lid protection and a combined lid/manual fallback. The plugin never decides to suspend the system when work ends, even if a lid is already closed.
8. **The external API is a read-only, one-shot status command:** `herdr-idle-inhibitor status --json`. External consumers poll it if needed. No `watch` stream or outbound command callbacks are included. The proposed JSON v1 contract is now in [status-api.md](status-api.md); lid logic, a general extension framework, and another application remain outside scope.
9. **Settings and Pause are shared across all local Herdr servers of the current OS user.** The process topology is not implied by this shared control scope.
10. **Hyprland/Hypridle, GNOME, and KDE are all mandatory Linux environments.** macOS and Windows remain mandatory as well. This is the requested verification matrix, not a claim that it has already passed runtime tests.
11. **Documented user-applied desktop configuration is acceptable when required.** Diagnose and explain the necessary integration; do not silently modify system or desktop configuration.
12. **The Windows Modern Standby battery limitation is accepted as a documented limitation.** In this hardware/power-source combination the application may not prevent idle sleep. Use the ordinary native mechanism; do not add bypasses, power-plan changes, special power-profile machinery, or complicated runtime handling just for this case. This is not permission to weaken other platforms or unrelated Windows behavior.

At baseline planning completion, implementation and commits were not authorized. The owner subsequently authorized full execution with the verification amendments in [implementation-notes.md](implementation-notes.md). This baseline remains a product-decision record; no commit or publication is authorized.

## Behavior and controls

| Situation | Plugin behavior |
| --- | --- |
| Relevant agents are working and inhibition is enabled | Hold an idle-sleep inhibitor without keeping the screen lit or preventing locking. |
| All observed work has ended | Release the plugin's inhibitor according to the agreed release policy; do not issue a sleep command. |
| User pauses inhibition | Release the plugin's inhibitor; continue observation so status remains meaningful. |
| User requests sleep or closes the lid | Do not intercept or override the OS/user policy. |

Settings and Pause apply across the current OS user's local Herdr servers. The full proposal uses one persisted Pause until Resume, with Herdr plugin enable/disable remaining the lifecycle control rather than a duplicate application toggle. This is an explicitly labeled technical default for review. The UI and external status distinguish observed work from desired inhibition and native ownership.

No lid mode, manual-sleep mode, or display-awake mode is included in the current baseline. The previously suggested display-awake control was never approved. Operational settings such as release timing may be settled during the grill without reintroducing broader power policies.

Technical timing, error handling, supported profiles, and settings persistence are specified in the full proposal. The baseline remains the record of product choices, not a second competing technical specification.

## Intended outcome

While at least one relevant local Herdr agent is reported as working and inhibition is enabled, the plugin holds an owned idle-sleep inhibition resource. After all relevant work stops, it releases only its own resource and lets the existing OS/desktop power policy operate normally. Releasing an inhibitor is not a command to sleep, nor a promise of immediate sleep.

The default must not keep the display lit or disable screen locking as an accidental side effect. This requirement must be tested against the actual desktop, not inferred from an API flag's name.

## Recommended boundaries, subject to the grill

- Observe Herdr's public state rather than scraping terminals, recognizing process names, or implementing agent-specific integrations again.
- Do not initiate suspend/shutdown, simulate input, change persistent global power settings, or silently rewrite desktop configuration. Use owned inhibition resources rather than power-policy mutations. Explicit user-applied desktop integration is permitted with instructions and diagnostics; it is not permission for automatic configuration changes.
- Do not monitor or orchestrate remote hosts in the initial scope.
- Do not promise to detect detached tasks that Herdr itself does not report as working; verify this boundary against relevant real agent workflows.
- Do not handle lid state or inhibit manual sleep, emergency power actions, or shutdown. The earlier combined lid/manual fallback is no longer in scope.
- Keep operational state transient. A file saying "active" is not the authority for an OS inhibitor.

## Historical candidate architecture

This section records the pre-grill direction and alternatives. The [full proposal](implementation-plan.md) is the current technical plan; candidate alternatives here are not unresolved requirements or permission to implement multiple designs.

### Herdr adapter

Use the plugin manifest and supported startup/lifecycle mechanisms to ensure monitoring for every relevant local Herdr server. The confirmed access boundary is all servers of the current OS user, without inspecting other users' sessions. Do not introduce an administrator requirement merely for multi-server discovery. A single-server implementation must scope ownership by server identity; a shared monitor must explicitly register/discover and track all relevant servers. The full proposal selects one per-user monitor and makes discovery coverage explicit.

Herdr 0.9.3 startup hooks are one-shot initialization commands, not a daemon supervisor. They run after the API becomes ready and on live handoff, but not merely when a plugin is linked/enabled or a UI client attaches. Do not assume that placing a long-running monitor in a startup command supplies supervision or complete activation coverage. See the [pinned plugin documentation](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/docs/next/website/src/content/docs/plugins.mdx#L233-L249).

Read an initial authoritative agent snapshot, react promptly to relevant events, and periodically reconcile. Reconnect after interruption. Account for closed/exited panes and server shutdown. The full proposal uses manifest hook hints rather than per-pane socket subscriptions, avoiding the latter's setup-ordering obligations.

Herdr's local transport differs by platform; use the supported transport on each required platform rather than assuming Unix sockets everywhere. Pin and test the supported Herdr version range.

### Decision logic

Keep one small, testable decision model independent of transport and OS calls:

- Current observation health.
- Whether any observed agent is working.
- Enabled/paused state for the fixed idle-only policy.
- Whether a real OS inhibition resource is held.
- Any approved release grace and bounded observation-failure grace.

Acquire promptly when work is observed. The full proposal adds no separate acquisition grace; KDE's native five-second activation delay is a service behavior to expose and test, not a delay copied into the application's policy. The proposal specifies normal and bounded observation-loss release timing.

### OS adapters

Implement the idle-only contract using owned native resources:

- Linux: select and verify the logind/desktop mechanisms for the required Hyprland/Hypridle, GNOME, and KDE environments. In the inspected Hypridle setup, an `idle` inhibitor also suppresses display listeners unless their policy explicitly ignores inhibition; lock timers using the same listener mechanism need the same check. A broad `sleep` inhibitor is not an acceptable substitute because it may block manual sleep.
- macOS: the public `PreventUserIdleSystemSleep` assertion matches the intended distinction between idle sleep, display sleep, and explicit/lid sleep. Do not use global `SleepDisabled` or a lid helper.
- Windows: the full proposal selects System Required power requests, not a speculative Execution Required addition. The user accepted the documented Modern Standby battery limit without complex special handling. Do not change the power plan or enable Away Mode as a workaround.

Do not represent best effort or successful resource acquisition as a guarantee the OS cannot override. An unsupported environment must be reported explicitly; the three mandatory platforms cannot silently be reduced to two.

The original alternatives were separate server-scoped inhibitors versus one shared aggregate owner. The full proposal selects one per-user monitor to satisfy global status/Pause without cross-monitor coordination. No installed OS service, watchdog, or additional runtime binary is included.

### Approved interface and proposed configuration

Use an on-demand compact text window inside Herdr. The user selected this initial visual direction:

```text
+-- Idle Inhibitor -------------------+
| Status:       Active               |
| Working:      3 agents / 2 sessions |
| Sleep policy: Idle sleep only      |
| Display:      May turn off          |
|                                    |
| [Pause]  [Settings]  [Close]         |
+------------------------------------+
```

This is a wireframe, not a finalized rendering or keyboard-interaction contract. The UI is not the monitor: closing the window must leave protection running. The full proposal gives the window a read/control connection to the shared monitor. The pinned Herdr documentation supports a transient terminal `popup`; Ratatui/Crossterm and concrete state/keyboard flows are specified in the proposal.

Provide the agreed configuration controls in the settings view. Proposed settings, persistence, error handling and explicit reload semantics are defined in the full plan.

Expose status and diagnostics that distinguish:

1. Herdr work observed.
2. Whether idle inhibition is enabled or paused.
3. Native resource actually acquired.
4. Unsupported environment or degraded observation/backend state.

Do not introduce a permanently occupied pane or a separate desktop/tray application. Herdr actions open the selected status/settings window.

### Approved external command direction

The user chose the smallest read-only interface: a one-shot command on our Rust executable, not a new built-in Herdr API. Herdr already launches plugin command argument vectors; see the [pinned command runtime](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/app/api/plugins/runtime.rs#L16-L31).

Planned command, not yet implemented:

```sh
herdr-idle-inhibitor status --json
```

- `status` returns a current snapshot. An external consumer polls when it needs updates; there is no streaming/event-delivery contract.
- Report observed work and observation health separately from pause state and actual idle-inhibitor ownership. `inhibitor held = false` does not mean all agents have finished.
- Running the status command must not acquire/release inhibition or launch a second authority for monitoring. A missing/unreachable monitor is an unavailable observation, not an empty working-agent set.
- A future separate lid controller can consume this state and own its own lid/sleep policy. This project will not inspect the lid, launch that controller, or decide when it should suspend the machine.
- No `watch` command, outbound command callbacks, HTTP service, public network listener, dynamic extension loader, or durable event queue is included. The read-only choice supersedes the earlier recommendation for a state stream.

## Baseline handoff to the grill

The four remaining baseline choices have been answered and incorporated above: read-only status queries, shared current-user controls, all three named Linux environments, and documented user-applied desktop configuration.

Configuration format, exact timing, Rust libraries, transport details, and the number of monitor processes should be investigated in the grill rather than turned into an advance implementation questionnaire. The exact JSON schema, observation freshness, and error/exit-code semantics are also grill topics. The final OS/hardware matrix and access to real test environments remain a verification gate, not an assumed availability of macOS/Windows hardware.

The user has authorized the grill and final implementation planning. That authorization is not permission to implement or proof that every target environment has passed runtime verification.

## Verification-first implementation outline

Implementation begins only after the final plan is approved. The original outline below is expanded and ordered by the mandatory P0–P7 gates in [implementation-validation.md](implementation-validation.md); native feasibility now comes first.

1. **Contract and fixtures:** capture representative, sanitized Herdr API responses/events from the supported versions; specify state transitions and observable counters.
2. **Decision-model tests:** multiple agents and sessions, working/blocked/idle/done, unknown observations, acquisition/release, release cancellation by new work, and explicit disable.
3. **Herdr adapter tests:** initial synchronization, event bursts, pane closure, reconnects, server restart, duplicate startup attempts, and platform transport behavior.
4. **Native adapter tests:** ownership, acquisition failure, crash cleanup, pause/resume, and coexistence with other inhibitors. For the approved read-only status command, test snapshots, observation failures, unavailable monitoring, and the absence of inhibition side effects.
5. **Plugin packaging:** manifest, startup/control integration, reproducible release artifacts for all agreed targets, and clear installation/configuration instructions.
6. **Real end-to-end acceptance:** actual Herdr activity and actual OS resources on Hyprland/Hypridle, GNOME, KDE, macOS, and Windows. Verify screen-off/locking behavior, prevention of idle-triggered host sleep, and release after work and process termination. Manual and lid behavior are regression boundaries, not features to implement. Account for AC/DC and Modern Standby where applicable.
7. **Review gate:** review the final diff against the approved contract, resolve findings, and present uncommitted work for user acceptance.

Compilation, mocked OS calls, and a visible status indicator are not substitutes for the final cross-platform runtime gate. Tests must not be made to pass accidentally by the already installed Stay Awake plugin or other existing inhibitors. Any test requiring a real suspend, a configuration change, or disabling an installed plugin requires a separately approved procedure.

## Original questions carried into the grill

These questions are preserved as history. Their resolutions/defaults and remaining native verification gates are indexed by U01–U12 in [the grill ledger](grill-session.md), not left as a second open-question queue.

- Which concrete mechanisms and OS/hardware versions fulfill the idle-only policy, including the Windows Modern Standby battery restriction?
- How does each agreed Linux desktop honor idle inhibition while allowing display-off and locking? What diagnostics and any approved integration steps are required?
- How are monitor startup, enable/disable, shutdown, and per-server identity handled on each supported Herdr platform/version?
- How do the on-demand window and read-only status command obtain the agreed cross-server state without introducing competing authorities? How do shared controls reach all relevant monitoring?
- What are the precise persistent enable/disable and temporary pause semantics?
- What JSON fields, freshness indicators, and error/exit-code behavior make a one-shot status query reliable without creating a second observation or inhibition authority?
- What happens on `unknown`, malformed responses, a lost connection, or an OS resource failure?
- What release grace prevents gaps between turns without retaining protection indefinitely?
- What does Herdr report while real background/subagent work continues?
- What build targets and actual runtime environments can satisfy the mandatory three-platform acceptance gate?

Resolve retrievable technical facts from official documentation/source first. Ask the user only for decisions that materially change the product contract. Record low-risk defaults for veto rather than expanding this into an exhaustive questionnaire.

## Exit gates

- **Baseline complete:** basic platform, behavior, scope, implementation language, and interface decisions are discussed and recorded; the user confirms readiness for the grill.
- **Grill complete:** every material unknown is resolved, defaulted, or explicitly accepted; contradictions with source evidence are addressed.
- **Final plan complete:** exact behavior, supported environments, ordered implementation steps, verification gates, and deviation policy are documented.
- **Build permission:** the user explicitly approves implementation. A completed plan is not itself that permission.
