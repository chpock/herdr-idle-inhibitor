# Troubleshooting

[Home](../README.md) · [Configuration](configuration.md) · [Compatibility](compatibility.md) · [Status API](status-api.md)

## Start with separate facts

Start with **Status** and **Details** in the [popup](../README.md#2-open-status-and-settings). For a shareable machine-readable snapshot, run `status --json` using [your installation's executable path](status-api.md#finding-the-executable). Read these facts separately:

- `available`: whether the monitor answered, not whether sleep prevention works.
- `observation.work` / `complete`: reported work and coverage confidence.
- `control.paused` / `pause_persisted`: shared control and whether it was saved.
- `inhibition.desired`: policy intent.
- `inhibition.resource_owned` / `request_state` / `last_error`: native facts.

Exit 0 can include partial observation, Pause or a backend failure. Exit 3 means unavailable, not zero agents. Full fields and exit codes are in [Status API](status-api.md).

## Popup does not open

The `show` action asks Herdr to open the plugin's Status and Settings window. An attached Herdr UI and an available popup slot are required.

Inspect Herdr's own error output in the action log. The plugin reports a subprocess exit status or launch error; it does not assume that every rejection means another popup is open. For `ui_busy`, close the other popup and retry. If no UI is attached, attach one before invoking the action.

When the window is unavailable, use the [read-only status command](status-api.md#finding-the-executable) to inspect monitoring and the native sleep-prevention request.

## Monitor unavailable

Check that the plugin is enabled with `herdr plugin list`. Monitoring starts automatically at Herdr server startup and when agents are detected or their status changes. Re-enabling the plugin takes effect on the next such event; Herdr 0.9.3 does not run startup commands immediately on `enable`.

If you want to start monitoring now rather than wait for an event, use the [optional manual start](../README.md#optional-manual-start). The **Start monitoring** control is also available when Status reports **Monitor unavailable**. Starting leaves Pause unchanged. Use the documented action or control rather than internal underscore-prefixed commands.

If startup failed, inspect plugin command logs and `monitor.log`; Herdr's action acknowledgment confirms launch, not success. A live but unresponsive owner is an activation error, not permission to start another authority, delete a lock or kill a guessed PID.

A source installation also prepares automatic startup when running Herdr sessions are discoverable; a prebuilt bundle has no install-time preparation step. A monitor can exit after all installations are disabled, after established absence of running servers, or after prolonged loss of observation. A crash is not automatically supervised; the next matching event or server startup launches it again.

## Automatic update

Follow the [README update command](../README.md#update), without changing Pause or disabling the plugin. A closed Windows popup or briefly unavailable status during the process switch can be normal; reopen the popup after installation.

For `update_failed` or an install preparation error:

1. Inspect Herdr's plugin command log and the application's `monitor.log`.
2. Correct the reported filesystem, execution-policy or startup error, then retry installation. A recovered monitor does not mean Herdr's file installation succeeded.
3. For a first update from an older release, check whether Herdr enabled-state queries succeeded; preparation can abort rather than replacing files under a live old owner.
4. If the selected replacement predates automatic-update support, choose a compatible revision. That replacement is rejected before retiring the current monitor.

Do not delete locks or terminate a PID copied from status. Compatible-code recovery and the first-update migration have different semantics; [How it works](behavior.md#failure-and-compatibility) explains them. For rebuilding a linked checkout in place on Windows, close its popup first as described in [Development](development.md#change-and-rebuild-a-linked-checkout).

## Configuration failures

| Diagnostic / symptom | Action |
| --- | --- |
| `invalid_config` | Correct TOML/schema/types/ranges, then Reload or stop/reactivate; new acquisition is disabled |
| `config_write_or_reload_failed` | Inspect permissions and external edits; the last valid runtime settings remain in use |
| `pause_not_persisted` / **PAUSE NOT SAVED** | Pause is live but restart can restore the old file; repair saving before relying on persistence |
| `config changed externally` | Reload your manual edits before saving from the popup |
| Resume fails | Fix saving first; failed persistence does not enable new acquisition |

There is no automatic file watcher. See [editing rules](configuration.md#editing-and-persistence); status queries do not Reload. If Settings is unavailable, follow the [manual editing procedure](configuration.md#editing-and-persistence) to load changes on the monitor's next start.

## Linux setup or backend failure

- **`unsupported_profile`:** check [supported exact versions](compatibility.md#platforms), desktop context and selected backend. An explicit selection cannot bypass the version guard.
- **`backend_unavailable` / native failure:** check that the required logind/session-bus/desktop service is running and accessible to your normal user. Do not use sudo or substitute a broader sleep lock.
- **`desktop_context_conflict`:** registrations belong to different desktop/bus contexts. One active desktop session is supported; multiple Herdr servers within it are fine.
- **KDE `pending`:** the native activation delay is real; accepted status requires desktop confirmation.
- **KDE `suppressed`:** the user disabled that request through KDE. Allow it intentionally if wanted; the plugin does not override suppression.

On Hyprland, `systemd-inhibit --list` should show `herdr-idle-inhibitor`, reason `Herdr agents are working`, and **idle / block** while its request is held. The application does not invoke that diagnostic command itself. Do not confuse another application's inhibitor with this one.

## Linux screen does not turn off

On Hyprland with Hypridle, this plugin's logind idle inhibitor can also delay screen dimming, display-off or automatic locking while agents work. Hypridle normally applies inhibitors to those listeners as well as to idle-sleep listeners. This is not a plugin error, and display tuning is **not required** to use sleep prevention. There is no confirmation checkbox to enable protection.

If you want display-off and locking to continue during agent work:

1. Inspect your existing Hypridle configuration. For each listener that only dims, turns off the display or locks the session, add `ignore_inhibit = true` while preserving its timeout and commands.
2. Keep `ignore_inhibit = false` (the default) for listeners that suspend or hibernate. If a listener combines display/locking with sleep, split those actions before allowing either to ignore inhibition.
3. Keep `ignore_systemd_inhibit = false` in `general` so Hypridle still honors the sleep-prevention request. Apply changes using your usual Hypridle/service workflow; the plugin does not edit or reload that configuration.

Per-listener `ignore_inhibit = true` ignores **all** inhibitors for that listener, including those from media players and other applications. The [optional Hypridle guide](hypridle-setup.md) shows edits to existing listeners and checks. No plugin setting or acknowledgment is needed afterwards.

If you use GNOME/KDE, or the screen stays on even when this plugin is paused, inspect desktop display settings and other applications' inhibitors. Do not change a sleep listener to ignore inhibition just to fix display-off.

## Missing or unknown work

`discovery_failed` means coverage is incomplete/failed/stale. `snapshot_stale` means at least one endpoint has no trustworthy current snapshot. Unknown status values do not count as working or nonworking evidence.

Check that Herdr 0.9.3 itself reports the relevant agent as `working`, the plugin is enabled under the relevant registered configuration root, and custom endpoints are registered or listed in [configuration](configuration.md#additional-local-endpoints). Do not expect monitoring of other users, arbitrary remote processes or tasks absent from Herdr's reports.

An incomplete non-null working count is a **lower bound**, not the exact machine-wide total. Repeated errors do not extend an existing request's bounded retention. Failure on one server does not erase fresh work on another.

## Computer still sleeps, or stays awake after work

Native acknowledgment is not an awake guarantee. Check the [Windows battery limitation](compatibility.md#windows-modern-standby-on-battery), power policy and selected Linux profile. The plugin does not explicitly request display/lock inhibition; desktop idle handling can still couple those actions to sleep prevention as described [above](#linux-screen-does-not-turn-off). Display-off and locking are not themselves host sleep.

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
