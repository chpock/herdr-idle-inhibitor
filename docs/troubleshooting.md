# Troubleshooting

[Home](../README.md) · [Configuration](configuration.md) · [Compatibility](compatibility.md) · [Status API](status-api.md)

## Start with separate facts

Query the executable in the installed checkout/bundle:

```sh
/path/to/plugin/target/release/herdr-idle-inhibitor status --json
```

On Windows use the `.exe` with PowerShell's `&` invocation operator. Read these separately:

- `available`: whether the monitor answered, not whether sleep prevention works.
- `observation.work` / `complete`: reported work and coverage confidence.
- `control.paused` / `pause_persisted`: shared control and whether it was saved.
- `inhibition.desired`: policy intent.
- `inhibition.resource_owned` / `request_state` / `last_error`: native facts.

Exit 0 can include partial observation, Pause or a backend failure. Exit 3 means unavailable, not zero agents. Full fields and exit codes are in [Status API](status-api.md).

## Popup does not open

The `show` action activates the shared monitor, then asks Herdr to open the plugin's `status` pane. An attached Herdr UI and an available popup slot are required.

Inspect Herdr's own error output in the action log. The plugin reports a subprocess exit status or launch error; it does not assume that every rejection means another popup is open. For `ui_busy`, close the other popup and retry. If no UI is attached, attach one before invoking the action.

Monitor activation precedes the popup request. Use the read-only status command to check its state: a missing popup does not prove that monitoring stopped, and activation does not prove that a native request was accepted. Internal underscore roles are not a supported alternative management API.

## Monitor unavailable

A standalone status query never starts the monitor. Installation/enable in an already-running server may also leave it unactivated until an action, qualifying event or server startup occurs.

Check enabled state with `herdr plugin list`, then follow [activation](installation.md#first-activation). If startup failed, inspect plugin command logs and `monitor.log`. A live but unresponsive owner is an activation error, not permission to start another authority, delete a lock or kill a guessed PID.

A monitor can exit after all installations are disabled, after established absence of running servers, or after prolonged loss of observation. A crash is not automatically supervised; a later startup/event/action is needed.

## Configuration failures

| Diagnostic / symptom | Action |
| --- | --- |
| `invalid_config` | Correct TOML/schema/types/ranges, then Reload or stop/reactivate; new acquisition is disabled |
| `config_write_or_reload_failed` | Inspect permissions and external edits; the last valid runtime settings remain in use |
| `pause_not_persisted` / **PAUSE NOT SAVED** | Pause is live but restart can restore the old file; repair saving before relying on persistence |
| `config changed externally` | Reload your manual edits before saving from the popup |
| Resume fails | Fix saving first; failed persistence does not enable new acquisition |

There is no automatic file watcher. See [editing rules](configuration.md#editing-and-persistence); status queries do not Reload. If the popup is unavailable, load manual changes through the normal disable/stop/reactivate sequence rather than expecting a hidden hot reload.

## Linux setup or backend failure

- **`setup_required`:** complete [Hypridle setup](hypridle-setup.md), then acknowledge it. Merely setting the acknowledgment without listener changes can keep display/lock timers from behaving as intended.
- **`unsupported_profile`:** check [supported exact versions](compatibility.md#platforms), desktop context and selected backend. An explicit selection cannot bypass the version guard.
- **`backend_unavailable` / native failure:** check that the required logind/session-bus/desktop service is running and accessible to your normal user. Do not use sudo or substitute a broader sleep lock.
- **`desktop_context_conflict`:** registrations belong to different desktop/bus contexts. One active desktop session is supported; multiple Herdr servers within it are fine.
- **KDE `pending`:** the native activation delay is real; accepted status requires desktop confirmation.
- **KDE `suppressed`:** the user disabled that request through KDE. Allow it intentionally if wanted; the plugin does not override suppression.

On Hyprland, `systemd-inhibit --list` should show `herdr-idle-inhibitor`, reason `Herdr agents are working`, and **idle / block** while its request is held. The application does not invoke that diagnostic command itself. Do not confuse another application's inhibitor with this one.

## Missing or unknown work

`discovery_failed` means coverage is incomplete/failed/stale. `snapshot_stale` means at least one endpoint has no trustworthy current snapshot. Unknown status values do not count as working or nonworking evidence.

Check that Herdr 0.9.3 itself reports the relevant agent as `working`, the plugin is enabled under the relevant registered configuration root, and custom endpoints are registered or listed in [configuration](configuration.md#additional-local-endpoints). Do not expect monitoring of other users, arbitrary remote processes or tasks absent from Herdr's reports.

An incomplete non-null working count is a **lower bound**, not the exact machine-wide total. Repeated errors do not extend an existing request's bounded retention. Failure on one server does not erase fresh work on another.

## Computer still sleeps, or stays awake after work

Native acknowledgment is not an awake guarantee. Check the [Windows battery limitation](compatibility.md#windows-modern-standby-on-battery), power policy and selected Linux profile. Display-off and locking are intentionally allowed and are not themselves host sleep.

After work, normal release grace defaults to five seconds. Lost observation can temporarily retain an already justified request. Even after this plugin releases, another application or the desktop's existing idle policy may keep the system awake. The plugin does not force suspend or guarantee immediate sleep.

A frozen process can retain its resource. The [PowerDevil suppressed-request exception](compatibility.md#powerdevil-suppressed-request-cleanup) can leave an orphan after the exact suppression/death/reallow sequence. Do not forcibly remove other applications' requests or restart services without understanding their effect.

## Logs and privacy

Use Herdr's plugin command-log listing to diagnose startup/action failures:

```sh
herdr plugin log list --plugin herdr-idle-inhibitor
```

The monitor's own log is `monitor.log` under the [application log directory](configuration.md#file-locations), rotated at approximately 1 MiB with one previous file, `monitor.log.1`. It records bounded transitions/startup errors, not raw agent traffic. Review any log excerpt before sharing it; errors or private diagnostics can include local environment information.

Observation reads minimal identity/status from `agent.list`; unrelated wire fields are discarded. The plugin does not read prompts or terminal output APIs, retain raw agent responses, or store activity history. Public status omits session names, endpoints, terminal IDs and filesystem paths. Private popup Details can show local paths and server information; inspect screenshots before publishing them.

## Reporting an issue

Use the repository [issue tracker](https://github.com/chpock/herdr-idle-inhibitor/issues). Include:

- OS, architecture, desktop/version and power source; Modern Standby availability if relevant.
- Herdr and executable versions (`herdr --version`, executable `--version`).
- Installation type, expected behavior and exact visible error.
- Public `status --json`, its exit code and a short sanitized log excerpt.
- Whether another inhibitor, a user-suppressed KDE request or a paused setting could affect the result.

Do not upload prompts, tokens, full event/context payloads, raw terminal output or private endpoint paths. Do not change timers, trigger sleep, or remove unrelated inhibitors solely to prepare an issue unless you intentionally choose to do so on a safe machine.
