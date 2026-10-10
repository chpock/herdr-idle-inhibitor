# How it works

[Home](../README.md) · [Configuration](configuration.md) · [Compatibility](compatibility.md) · [Status API](status-api.md)

The [README](../README.md#everyday-use) covers popup controls and daily use. This guide explains how work observations become an idle-sleep request, where the monitoring boundary lies, and what happens during failures, recovery and updates.

## Monitoring scope

One background monitor serves the current OS user's registered local Herdr configuration directories and endpoints. It discovers named/default servers under those directories and combines their work across panes and workspaces, including sessions with no attached UI. Multiple enabled installations share that monitor and its preferences; disabling one does not stop another enabled registration.

The plugin observes Herdr's reports, not CPU activity, terminal text or agent-specific subprocesses. A detached/background/remote task that Herdr does not report as working is not independent evidence. Custom servers under an unregistered configuration directory are not automatically found: activate their plugin registration or configure [additional local endpoints](configuration.md#additional-local-endpoints). The monitor does not control remote hosts or follow other users' agents.

## Monitor lifecycle

While the plugin is enabled, Herdr starts the monitor automatically at server startup and on agent detection or status changes. Pane exit/closure and workspace creation/closure also trigger registration and refresh. These hooks use the same idempotent startup operation: an existing monitor is reused rather than duplicated.

In Herdr 0.9.3, `enable` updates the plugin registry without running startup hooks immediately. After re-enabling, the next matching event or server startup starts monitoring automatically. The [manual start command](../README.md#optional-manual-start) is optional; use it if you want an immediate launch without waiting for an event. Starting the monitor leaves Pause unchanged.

Disabling all registered installations releases the sleep-prevention request and stops the monitor. Another enabled installation keeps the shared monitor alive. For temporary stops, use Pause: it releases the request while continuing to observe agents.

## What counts as work

Only the literal Herdr agent status `working` counts. `blocked`, `idle` and `done` do not justify a request. Unknown status values, failed reads and inaccessible servers remain uncertainty, not invented work or an empty agent list. Duplicate projections of the same terminal on one server count once.

The public work state is:

- **working:** at least one fresh working agent is observed, even if another server is unavailable;
- **none:** no working agents and complete, current observation of the registered scope;
- **unknown:** neither of those conclusions can be established.

A working count with incomplete observation is a lower bound, not an exact machine-wide total. The popup and JSON API keep work, observation coverage, Pause and native request state separate. A failed or paused request does not mean agents stopped.

## Request policy

| Situation | Behavior |
| --- | --- |
| Fresh working agent, freshly enabled installation, not paused | Request idle-sleep prevention through the selected native mechanism |
| One server stops working but another still works | Keep the shared request |
| Last reported work ends | Release after the configured delay; default five seconds |
| New work arrives during release delay | Cancel release and keep the existing request |
| Observation is lost after known work | Retain an existing request for at most 30 seconds from its last valid positive evidence; repeated errors do not renew it |
| Unknown observation without an existing justified request | Do not acquire |
| Pause or loss of eligibility from disabling all installations | Withdraw this plugin's request without normal completion grace |
| User requests sleep or closes the lid | Leave the action to OS/user policy |

Release is not a suspend command. The desktop decides what happens after the plugin relinquishes its request. Other applications' inhibitors are untouched. No stronger fallback is used when the selected desktop rejects the request or its setup is incomplete.

## Native state is not an awake guarantee

`accepted` means the selected OS/desktop mechanism acknowledged the request, not that the OS cannot override it. KDE can report `pending` or `suppressed` while a resource is still owned. The application distinguishes policy intent, resource ownership and native acknowledgment rather than claiming an unconditional protection flag.

Windows power policy and Modern Standby on battery can defeat an ordinary request. PowerDevil has a specific user-suppression/owner-death/reallow cleanup exception. These are documented in [Compatibility](compatibility.md).

## Timing and recovery

| Operation | Interval / limit |
| --- | --- |
| Agent snapshots | Every two seconds, plus coalesced event-triggered refreshes |
| Server discovery and plugin enabled-state checks | Every five seconds |
| Installed-executable update checks | Every five seconds while an eligible installation is tracked |
| Configuration metadata checks | Every two seconds; read/parse only when changed; recreate a missing file |
| Agent observation freshness | Six seconds; explicit read failure marks uncertainty immediately |
| Discovery/enabled-state freshness | Ten seconds |
| Normal release delay | Five seconds by default; configurable 0–60 seconds |
| Existing-request observation/enabled-state retention | At most 30 seconds from the relevant last successful evidence |
| Popup refresh | About once per second |

These are scheduling targets, not hard real-time guarantees. Request acquisition/release can still be in progress while policy changes. Polling cannot prevent sleep that starts before new work is observed. Update checks reuse unchanged file metadata; executable contents, not package version text alone, identify changed code.

After sleep or a large execution gap, stale observations are invalidated. Fresh discovery and work evidence are required before reacquiring. No cached work is loaded from disk.

The monitor normally exits after 30 seconds with no running servers once discovery/retirement establishes that state, or after 120 seconds without useful observation/discovery recovery. It also exits when all tracked installations lose eligibility. Pause keeps it alive while Herdr remains present, but failed enabled-state checks cannot keep an old request alive indefinitely.

Herdr does not supervise a crashed detached monitor. A later server startup or matching event starts it again; the optional `start` action requests an immediate launch. There is no finite automatic restart guarantee if no further event occurs. A frozen process differs from a terminated one: its resource can remain held until it resumes or terminates. See the [PowerDevil cleanup exception](compatibility.md#powerdevil-suppressed-request-cleanup).

## User interface and read-only consumers

The `show` action opens the Status and Settings window. The interface displays monitor state and provides controls for Pause, preferences and diagnostics. Its periodic refresh reads snapshots from the monitor. Explicit settings controls apply changes immediately; manual TOML edits load automatically in the monitor's configuration checks.

A standalone `status --json` query also never starts monitoring, registers a session, reloads settings, or acquires/releases a request. It reads the existing monitor and returns unavailable/unknown when no compatible monitor answers. Public consumers poll; there is no watch stream, management API, HTTP endpoint, remote selector or command callback. An external program owns its own actions and must not treat uncertainty as permission to sleep. See [Status API](status-api.md).

## Automatic updates

Use the [ordinary update command](../README.md#update), or [rebuild a linked checkout](development.md#change-and-rebuild-a-linked-checkout). The mechanics below explain the guarantees and limits; they are not extra steps for the user.

### Installation and process switch

Herdr builds a GitHub replacement in a separate directory before publishing it. The source manifest's preparation step stages the executable and coordinates with the monitor; it waits for installed files and the registered revision/directory to agree before applying a published replacement.

Long-lived monitors execute immutable, content-addressed copies under the application's [state directory](configuration.md#file-locations), rather than holding the installed executable or checkout working directory open. This lets Windows replace the installation. A popup still runs from the installation; update coordination can close it to release that file. In-place developer builds must close their Windows popup themselves.

The resident monitor also detects installed executable or registered-directory changes without waiting for a new Herdr event. Identical contents do not restart it. Separate unchanged installations do not make it oscillate between them.

During replacement, the old process releases its native resource before giving up shared-monitor ownership. The new process inherits registered/discovered sessions and effective Pause, including an unsaved live Pause, and loads settings from the same configuration file. It then obtains fresh observations rather than treating old work evidence as permission to acquire. Configuration files are not rewritten to transfer state; invalid startup settings still prevent acquisition.

This is not a zero-gap operation: this plugin's sleep request can briefly be absent and JSON status can temporarily be unavailable.

### Failure and compatibility

If publication fails, an already cache-resident monitor can remain on its previous code. If a compatible replacement fails to start after handover, the previous cached executable is restored with the captured state. This is runtime recovery, not rollback of Herdr's installed files. Failed executable contents are not retried indefinitely; a changed artifact or explicit installation retry can try again.

Automatic replacement rejects a candidate lacking the compatible update protocol before retiring the current owner, rather than silently losing state.

For an actual failure, use [update troubleshooting](troubleshooting.md#automatic-update), not manual PID killing or lock deletion. Native Windows/macOS update execution is subject to the [documented testing boundaries](compatibility.md#platforms).
