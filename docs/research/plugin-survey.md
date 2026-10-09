# Herdr idle-inhibitor: prior-art and integration survey

Date: 2026-10-08. Status: research before product decisions, not an approved implementation plan.

## Goal and boundaries

The requested project prevents the machine from sleeping while AI agents in Herdr are working. The meaning of "working", supported operating environments, and the kinds of sleep to prevent still require confirmation.

Only research artifacts were added to this repository. No plugin was installed, updated, enabled, disabled, or executed for this survey. No desktop configuration was changed and no suspend test was attempted. Source inspection and read-only queries are not end-to-end verification of an inhibitor.

The agreed sequence is: research -> basic questions -> baseline plan -> adversarial plan review -> final plan. Implementation is outside this stage.

## Coverage and reproducibility

The complete official marketplace metadata snapshot was screened, rather than only the first page of search results:

- Catalog generated at `2026-10-08T21:30:29.799Z`.
- **1,626 plugin manifests across 1,572 repositories**.
- Collector query: `topic:herdr-plugin is:public`; the snapshot reports `truncated: false`.
- Screening covered repository names, descriptions, topics, and manifest metadata using sleep, wake, idle, inhibition, power, and related terminology. Broad matches included unrelated dashboards and notification plugins, which were excluded after metadata review.
- **60 candidate repositories** were examined at metadata level: eight direct inhibitors, one sleep scheduler, and 51 unrelated matches.
- Additional public search found **one more direct inhibitor**, `gw31415/herdr-amphetamine-macos`, absent from that snapshot.
- Manifests and relevant implementation paths were read for **all nine discovered direct inhibitors**, plus the adjacent sleep scheduler. This was not a line-by-line source audit of all 1,626 unrelated plugins.

The marketplace is automatically collected, not curated or security-reviewed. Its collector also reports excluded, invalid, missing, and unavailable repositories. Private, unlisted, or misleadingly described plugins cannot be exhaustively enumerated by this survey.

[plugin-survey-sources.json](plugin-survey-sources.json) records the snapshot hash, collector accounting, all 60 classified candidates, and immutable commits for the ten source-reviewed repositories. Reviewed indexed commits matched the catalog's `headCommit` values.

Primary discovery sources:

- [Official marketplace](https://herdr.dev/plugins/)
- [Marketplace discovery rules](https://herdr.dev/docs/marketplace/)
- [Herdr plugin authoring and trust documentation](https://herdr.dev/docs/plugins/)

## Direct solutions

Versions below are those inspected, not compatibility guarantees or recommendations to install them.

### Linux-capable plugins

| Plugin | Declared platforms / version | Actual approach | Assessment for this project |
| --- | --- | --- | --- |
| [susomejias/herdr-awake](https://github.com/susomejias/herdr-awake/tree/71c90c3e7809732c1e44336632893a65306db0ef) | Linux, macOS / 0.2.2 | Shell daemon started by a startup hook; polls `herdr agent list` every 15 seconds by default. Treats `working`, `blocked`, and `unknown` as busy. Linux uses `systemd-inhibit` for `sleep:idle:shutdown`; macOS uses `caffeinate`. Tracks the launching server and child processes. | Small and understandable, but its policy is broader than "working agents prevent automatic sleep": blocked/unknown agents and shutdown are included. Polling and process/PID ownership are important design choices, not just implementation details. |
| [usrivastava92/herdr-wakeup](https://github.com/usrivastava92/herdr-wakeup/tree/43db0b9f88a4b1bc560593b0ce8f2a7d2a940f04) | Linux, macOS / 0.1.0 | Rust watcher subscribes to Herdr events and obtains authoritative `agent.list` snapshots; has periodic reconciliation, reconnects, controls, and diagnostics. Starts a bundled standalone `wakeup` utility. Defaults: working-only, 5-second acquisition grace, 30-second release grace. The bundled Linux backend uses `systemd-inhibit --what=idle`; display inhibition uses a ScreenSaver D-Bus connection. | Strong reference for event-driven observation and reconnection. Less minimal because there are two products/binaries and two process lifetimes. Linux `idle` semantics must be checked against the actual desktop: it does not inherently mean "keep the machine awake but let the display timer run". |
| [moosingin3space/herdr-sleep-inhibit](https://github.com/moosingin3space/herdr-sleep-inhibit/tree/82a0e59378ff3bce7d918dc55dbdd9643f5b8ad7) | Linux / 0.1.0 | Rust broker polls Herdr every two seconds, counts `working`, and owns an XDG portal Inhibit request with Suspend + Idle flags. Includes a TUI and enable/disable controls; inhibition is disabled by default. Startup and status-change hooks ensure the broker is running. | Closest Linux-only prior art for an explicitly owned inhibitor. Portal support depends on the selected desktop backend. Its singleton broker and shared plugin state directory need scrutiny for multiple Herdr servers: the first broker is associated with one socket. |
| [assawalhy/herdr-stay-awake](https://github.com/assawalhy/herdr-stay-awake/tree/b6a33e6ed09f188ba0d85377a9083d3d3a6adeaa) | Linux, macOS, Windows; WSL handling in code / 0.2.0 | Node.js commands run on startup/status events, persist working panes and inhibitor handles, and use a watchdog for reconciliation. Linux primarily uses `systemd-inhibit --what=sleep:idle`, with additional fallback mechanisms. Defaults include 5/30-second grace periods and a 12-hour maximum hold. | Broad controls and diagnostics, but a substantially larger state/process surface. The inspected upstream revision also has a default-enabled post-release path that explicitly requests suspend; that is more than passive inhibition. See the installed-version distinction below. |

Relevant source entry points: `awake.sh`; Wakeup's `crates/wakeup-herdr/src/{main,opts,state}.rs` and bundled utility backend; Sleep Inhibit's `src/main.rs`; Stay Awake's `src/{herdr,state,config,watchdog,util}.js` and `src/backends/linux.js`.

### macOS-only plugins

| Plugin | Version | Actual approach | Useful lesson / boundary |
| --- | --- | --- | --- |
| [nwarwick/herdr-caffeinate](https://github.com/nwarwick/herdr-caffeinate/tree/25bdfe36d3948123069c9dadbb02df4d1a895d07) | 0.1.0 | Python daemon, Herdr state observation, macOS `caffeinate`, with the assertion linked to the daemon's lifetime. | Good reference for process-lifetime ownership and keeping policy separate from the OS mechanism. Not a Linux implementation. |
| [jewei/herdr-caffeinated](https://github.com/jewei/herdr-caffeinated/tree/a4fd32c3594f59b7905eb3678f8e3c2b37fa70b2) | 0.4.0 | Shell/jq implementation with event hooks, state refresh/watchdog, delayed release, and `caffeinate -ims`. | Demonstrates event handling plus reconciliation; also illustrates how state files, helper processes, and timers accumulate even in shell. macOS flags are not portable Linux semantics. |
| [happyeric77/agent-keep-awake](https://github.com/happyeric77/agent-keep-awake/tree/eef5abfda527cd12ff64cfcd869305bbf375e031) | 0.1.0 | Node.js monitor polling pane state; starts `caffeinate`, with system and display sleep inhibition enabled by default and user controls. | Straightforward small monitor, but its default display policy is broader than preventing host sleep alone. |
| [Hanyang-Li/herdr-espresso](https://github.com/Hanyang-Li/herdr-espresso/tree/ee0b29cd82022f961b20149119dc4868a41f22fa) | 1.0.0 | Rust watcher driving the external Espresso CLI through renewable leases; counts working and blocked agents. Espresso offers an optional privileged helper that changes global `SleepDisabled` state for lid-related behavior (see the [lid feasibility check](lid-sleep-feasibility.md)). | Renewable ownership is interesting. Lid behavior, blocked agents, and a privileged helper are additional product scope, not prerequisites for a basic inhibitor. |
| [gw31415/herdr-amphetamine-macos](https://github.com/gw31415/herdr-amphetamine-macos/tree/e5ef0d75fbd47c8b9b4b600d4dbf720f2d1ac7ff) | 0.2.0 | Python session-scoped LaunchAgent polls Herdr and controls the Amphetamine application through AppleScript using expiring sessions. Interactive settings/status UI. | Another ownership model, but requires an external application and macOS-specific automation. Not present in the inspected marketplace snapshot. |

### Related, but not an inhibitor

[scheron/herdr-want-to-sleep 0.2.0](https://github.com/scheron/herdr-want-to-sleep/tree/120c210e838ec7333c14556c61e4f38eb462e3c3) explicitly schedules sleep or shutdown after agents settle, with a manual arm/disarm workflow, inactivity checks, and a warning interval. It is a power-action scheduler, not an interchangeable solution to "hold an inhibitor while work runs".

Wake-on-LAN, idle notifications, status overlays, and background-activity badges in the catalog likewise do not hold OS sleep inhibitors. They were not treated as equivalent implementations merely because their descriptions contain "wake" or "idle".

## What the existing local installation establishes

Read-only inspection found:

- Herdr **0.9.3**.
- Linux with **Hyprland / Hypridle 0.1.8**, systemd 261.2, XDG desktop portal 1.22.1, and Hyprland/GTK portal backends.
- **Stay Awake 0.2.0 is already installed and enabled.** Its checked-out commit is `c6ea775b7d246f7e497bf146a0cfbefda8cc8c0a`, with no local git modifications reported.
- The inspected upstream Stay Awake commit is **different**, `b6a33e6ed09f188ba0d85377a9083d3d3a6adeaa`, despite the same manifest version. Upstream's post-release suspend code must not be attributed to the installed revision without checking it there.
- The local Hypridle configuration has separate brightness, display-off, and suspend listeners. Suspend is requested by `systemctl suspend`; those listeners have no `ignore_inhibit` override.

This does **not** prove that the existing plugin is broken or that a new implementation is automatically preferable. The question is which exact behavior we want, and whether an existing implementation satisfies it without unnecessary policy or lifecycle machinery.

No installed-plugin settings were altered. Existing inhibitors must be accounted for during later acceptance tests, because another plugin can make a broken implementation appear to work.

## Herdr integration facts

Compatibility findings below use the **0.9.3 tag**, commit `7b116c05bfda646af39d2524c54e70c751f57ee8`, not assumptions based on a newer main branch.

1. **Use Herdr's public state, not terminal scraping or process-name guesses.** `agent.list` exposes `agent_status`, pane/workspace identities, and sequence/revision metadata. Statuses include `working`, `blocked`, `idle`, `done`, and `unknown`.
2. **An open agent is not necessarily a working agent.** The process can remain open while waiting for user input or after completing its work.
3. **Herdr can itself derive state from different sources.** Its documented integrations for Pi and several other agents report state directly while active; others rely on screen detection. A downstream inhibitor cannot claim to know about every detached task if Herdr does not report it as work.
4. **Startup hooks are supported.** A claim that Herdr has no startup hook is not valid for the installed version. Event hooks remain useful for reconciliation/recovery, but are not the only way to start a monitor.
5. **Event commands are not a serialized state machine.** The plugin runtime starts commands on separate threads/processes. Multiple hook invocations can overlap. Atomically renaming a JSON file does not serialize a read-modify-write sequence across those processes.
6. **Plugin configuration/state directories are shared for a plugin, not automatically partitioned by Herdr socket.** A global `broker.lock`, PID file, or working-pane file can therefore accidentally couple named sessions. Single-instance guarantees must state their scope.
7. **The wire contract must be taken from the installed schema.** `events.subscribe` uses typed `subscriptions`; a `pane.agent_status_changed` event carries `data.agent_status`. Initial snapshots, reconnects, pane closure, and missed-event reconciliation require an explicit design. Watching only status changes is not a complete lifecycle.

Primary source references:

- [AgentInfo schema](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/schema/agents.rs)
- [Event subscription and payload schema](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/api/schema/events.rs)
- [Plugin command/startup/event runtime](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/app/api/plugins/runtime.rs)
- [Plugin paths](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/plugin_paths.rs) and [base state directory](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/src/config/io.rs)
- [Integration behavior](https://github.com/herdrdev/herdr/blob/7b116c05bfda646af39d2524c54e70c751f57ee8/docs/next/website/src/content/docs/integrations.mdx)

## OS mechanisms are not interchangeable

### Linux / systemd-logind

An inhibitor is a resource with a lifetime, not a persistent "disable sleep" setting. The logind `Inhibit` method returns a file descriptor; the lock disappears after all copies of that descriptor are closed. A monitor that directly owns the descriptor can obtain crash cleanup without persisting an "active" claim as the authority.

- **`idle`** blocks automatic idle handling. It does not promise that explicit suspend requests are blocked.
- **`sleep`** blocks suspend/hibernation requests subject to the OS's override and permission rules; this can affect explicit user requests too.
- **`shutdown`** is separate scope and should not be added accidentally.
- Launching `systemd-inhibit` as a child is convenient, but then the child, not merely the monitor, owns the resource. Killing the monitor alone must not be assumed to kill every helper or close every inherited descriptor.

Sources: [Inhibitor locks](https://systemd.io/INHIBITOR_LOCKS/), [systemd-inhibit](https://www.freedesktop.org/software/systemd/man/latest/systemd-inhibit.html), [logind D-Bus API](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html).

### Hypridle: the most important local policy distinction

Hypridle 0.1.8 reads logind's block inhibitors and treats an `idle` block as a general inhibition lock. Its `onIdled` handler suppresses a listener unless that listener has `ignore_inhibit` enabled.

Consequently, **`--what=idle` is not "suspend-only" in the current configuration**: brightness and display-off listeners are suppressed as well. This conclusion is supported by the version-matched source and the local configuration, not by a live sleep experiment.

If the product must prevent automatic suspend while preserving display-off/locking and explicit user suspend, the desktop's listener policy must be part of the design. Per-listener configuration is one possible integration, but it must be explicit and approved; the plugin must not silently rewrite the user's desktop configuration. Simply changing `idle` to `sleep`, or adding every inhibitor flag, is not an equivalent solution.

Source: [Hypridle 0.1.8 `onIdled`, `onInhibit`, and `handleDbusBlockInhibits`](https://github.com/hyprwm/hypridle/blob/e5c01af0842bd66617f7004568df9406111d6e80/src/core/Hypridle.cpp), [Hypridle configuration documentation](https://wiki.hypr.land/Hypr-Ecosystem/hypridle/).

### XDG desktop portal

The portal distinguishes Logout, User Switch, Suspend, and Idle flags. A request is released through `Request.Close()`. Request/connection ownership matters; a successful method invocation alone is not sufficient evidence of sustained inhibition.

The local Hyprland portal descriptor does not advertise an Inhibit backend; the installed GTK descriptor does. Actual routing and desktop behavior require verification. Neither "Hyprland cannot use the portal" nor "the portal is guaranteed to work here" follows from those descriptors alone.

Source: [XDG portal Inhibit API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Inhibit.html).

### Native Wayland idle-inhibit

The protocol attaches inhibition to a visible `wl_surface`. It explicitly allows the inhibitor to stop being honored when the surface is hidden, occluded, unmapped, or otherwise not visually relevant. That is not a suitable default mechanism for an agent that must keep running while its terminal is hidden or the display is off.

Source: [Wayland idle-inhibit protocol](https://wayland.app/protocols/idle-inhibit-unstable-v1).

### macOS and Windows

macOS `caffeinate` / IOKit assertions and Windows execution-state/power-request APIs are legitimate native mechanisms, not reasons to emulate keyboard or mouse input. Their flags, lid behavior, explicit-sleep semantics, and ownership rules differ. Supporting all platforms would require separate contracts and real acceptance environments, not just a platform switch around command names.

## Conclusions and recommendations awaiting approval

1. **There is substantial prior art.** A new project should not be justified by claiming that no Herdr inhibitor exists.
2. **Useful references are complementary:** Wakeup for event-driven Herdr observation; Sleep Inhibit for owned Linux inhibition; the smaller macOS daemons for bounded ownership. Stay Awake demonstrates both useful controls and the cost of extra state, helpers, and power-action policy.
3. **A narrow new implementation is reasonable if its contract differs:** exact working-state semantics, explicit automatic-sleep behavior, reliable ownership, and no hidden suspend requests or global power-setting changes. This is a conditional recommendation, not a finding that all existing plugins are defective.
4. **Do not decide the Linux mechanism before the desired sleep/display behavior.** The current Hypridle configuration makes that a real product choice, not a naming detail.
5. **Prefer one owner of transient state per monitored Herdr server.** Events should prompt reconciliation against current Herdr state. Whether this is the approved structure depends on the selected session scope; no broker, database, distributed coordinator, or cross-platform abstraction is required merely for future possibilities.
6. **Acquire promptly; consider a short release grace.** A five-second acquisition delay copied from an existing plugin can leave new work unprotected near an idle deadline. Exact release timing and behavior on unknown/unreachable state belong in the subsequent plan review.
7. **Observe and explain failures.** A status command should distinguish "work observed", "inhibition requested", and "OS resource actually held"; a cached boolean is not proof of the last condition.

## Basic questions to settle next

All answers in the right column are proposals, not recorded user decisions.

| Question | Recommended answer | Why this changes the plan |
| --- | --- | --- |
| Which environments are mandatory? | Linux with systemd; Hyprland/Hypridle as the initially supported and tested desktop. | Determines the OS API and the honest compatibility boundary. macOS/Windows require separate behavior and verification. |
| What should remain possible while work is running? | Block automatic host sleep only; allow display-off, locking, and an explicit user sleep request. Never initiate sleep ourselves. | Determines inhibitor flags and whether documented Hypridle listener configuration is necessary. |
| What counts as work? | Herdr's `working` status. `blocked`, `idle`, and `done` do not independently require inhibition. | Waiting for a user is not ongoing autonomous work. Unknown state and observation failures need a separate bounded policy rather than being silently treated as completed work. |
| Which sessions/hosts are covered? | All local Herdr servers/sessions and their workspaces; protection belongs on the host executing the agents. No remote orchestration in the initial scope. | Determines process and state ownership. A UI client or selected workspace is not the complete set of local work. |

## Next gate

After answers, write the baseline plan with explicit scope and unresolved assumptions. Then pressure-test it against the real API and desktop behavior, including startup ordering, reconnects, stale states, concurrent agents/sessions, server/plugin shutdown, crash cleanup, and long-running or detached work.

The available skill is named `grill-for-unknowns`; no skill named exactly `grill-me` is listed in this environment. Confirm that the available docs-grounded grilling skill is the intended one before treating the final plan as approved. Loading its instructions during research does not mean the plan-review stage has already been performed.

No final architecture, implementation authorization, or successful runtime verification is claimed by this document.
