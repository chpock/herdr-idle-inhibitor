# Usage

[Home](../README.md) · [Configuration](configuration.md) · [Troubleshooting](troubleshooting.md)

## What is monitored

One background monitor serves the current OS user's registered local Herdr configuration roots and endpoints. It discovers named/default servers under those roots and combines their work regardless of focused pane, workspace, attached UI or an open plugin popup.

Only the literal Herdr status `working` counts. `blocked`, `idle` and `done` do not independently justify a request. Unknown status values, failed reads and inaccessible servers remain uncertainty, not invented work or an empty agent list. Duplicate projections of the same terminal on one server count once.

The plugin observes Herdr's reports, not process CPU usage, terminal text or agent-specific subprocesses. A detached/background/remote task that Herdr does not report as working is not independent evidence. Arbitrary custom servers under unregistered configuration roots are not automatically found; see [additional endpoints](configuration.md#additional-local-endpoints).

## Work and sleep behavior

| Situation | Behavior |
| --- | --- |
| Fresh working agent, enabled installation, not paused | Request idle-sleep prevention through the selected native backend |
| One server stops working but another still works | Keep the shared request |
| Last reported work ends | Release after the configured delay; default five seconds |
| New work arrives during release delay | Cancel release and keep the existing request |
| Observation is lost after known work | Retain an existing request for at most 30 seconds from its last valid positive evidence; repeated errors do not renew it |
| Unknown observation without an existing justified request | Do not acquire |
| Pause or plugin disable | Withdraw this plugin's request without normal completion grace |
| User requests sleep or closes the lid | Leave the action to OS/user policy |

Release is not a suspend command. The desktop decides what happens after the plugin relinquishes its request. Other applications' inhibitors are untouched.

## Popup and controls

Open **Idle Inhibitor: status and settings** (`show`) from Herdr's actions. If Herdr cannot open the popup, see [Troubleshooting](troubleshooting.md#popup-does-not-open).

The main view separates:

- **Work:** working, no work or unknown.
- **Observation:** complete or partial for the registered scope.
- **Request:** desired policy, resource ownership and native acknowledgment.
- **Control:** running or paused.

`accepted` means the backend acknowledged the request, not that the OS cannot override it. KDE can report `pending` or `suppressed` while a resource is still owned. Backend failure does not mean agents stopped.

| Key | Action |
| --- | --- |
| Tab / Shift-Tab / Up / Down | Move focus in Status/Settings |
| Enter / Space | Activate the focused action |
| `p` | Pause / Resume when the monitor is available |
| `s` | Open Settings |
| `d` | Open Details |
| Left / Right on release delay | Decrease / increase seconds |
| Up / Down / PageUp / PageDown in Details | Scroll diagnostics |
| Enter in Details | Return to Status |
| Escape / `q` / Ctrl+C | Close from any view |

Minimum usable size is **50 columns × 10 rows**. Below it, only close keys work; invisible settings cannot be activated. Details holds longer diagnostics. A different already-open Herdr popup or a missing attached Herdr UI can prevent opening this one.

Closing the popup never stops the monitor or withdraws its request. Its passive refresh reads monitor state; it does not query Herdr independently or restart a missing monitor. **Retry / activate** is an explicit action.

## Pause and Resume

Pause is shared across all monitored servers and saved across monitor restarts. It keeps observing agents but withdraws the plugin's request. Closing/reopening the popup does not Resume; Pause has no automatic timeout.

If saving Pause fails, the live Pause still applies and a prominent **PAUSE NOT SAVED** warning remains. Restart can restore the previously saved setting. Resume requires successful persistence before new requests are enabled. See [configuration writes](configuration.md#editing-and-persistence) for conflicts with manual edits.

Herdr enable/disable controls plugin lifetime. Pause is not an alternative installation switch.

## Timing and recovery

| Operation | Interval / limit |
| --- | --- |
| Agent snapshots | Every two seconds, plus coalesced event-triggered refreshes |
| Root discovery and plugin enabled-state checks | Every five seconds |
| Agent observation freshness | Six seconds; explicit read failure marks uncertainty immediately |
| Discovery/enabled-state freshness | Ten seconds |
| Normal release delay | Five seconds by default; configurable 0–60 seconds |
| Existing-request observation/enabled-state retention | At most 30 seconds from the relevant last successful evidence |
| Popup refresh | About once per second |

These are scheduling targets, not hard real-time guarantees. A request can be pending, releasing or uncertain while its native operation completes. Polling cannot prevent sleep that begins before new work is observed.

After sleep or a large execution gap, stale observations are invalidated and fresh discovery/work are required before reacquisition. No cached work is loaded from disk.

The monitor normally exits after 30 seconds with no running servers once discovery/retirement establishes that state, or after 120 seconds with no useful observation/discovery recovery. Pause keeps it alive while Herdr remains present. Failed enabled-state checks cannot keep an old request alive indefinitely.

Herdr does not supervise a crashed detached monitor. A later startup/event/action can reactivate it; there is no finite automatic-restart guarantee if no further event occurs. A frozen process differs from a terminated one: its resource can remain held until it resumes or is terminated. See the [PowerDevil cleanup exception](compatibility.md#powerdevil-suppressed-request-cleanup).

## External consumers

Use [the read-only status API](status-api.md) and poll when needed. Work, observation health, Pause and native request state are separate fields. A paused or failed request, an unavailable monitor, or a query error is never proof that all work finished.

There is no public watch stream, management API, HTTP endpoint, remote selector or command callback. An external program owns its own actions and must not treat `unknown` as permission to sleep.
