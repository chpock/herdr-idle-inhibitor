# Herdr Idle Inhibitor

Keep long-running Herdr agent work from being interrupted by automatic idle sleep, without keeping your display on.

The plugin watches the agent states reported by your local Herdr sessions. While at least one agent reports `working`, it asks the operating system to hold off idle sleep. When the last agent stops working, it releases that request after five seconds by default. The computer then follows its normal sleep policy; the plugin never sends it to sleep itself.

- Your display can turn off and your desktop can lock normally.
- Closing the lid and choosing Sleep manually remain under OS/user control.
- `blocked`, `idle` and `done` do not count as working. Waiting for input is not active work.
- Monitoring runs in the background across registered local Herdr sessions for your OS user, regardless of which pane or workspace is focused.
- Pause and settings are shared across those sessions, not tied to a popup or workspace.

## Requirements

Use **Herdr 0.9.3** on your native desktop, as your normal user. No administrator helper or system service is needed.

| System | Supported environment |
| --- | --- |
| Linux x86_64 | Hyprland with Hypridle **0.1.8**, GNOME session **51.0**, or KDE with PowerDevil **6.7.5** |
| macOS arm64 / x86_64 | macOS **13.0+** |
| Windows x86_64 | Windows **10+**, native MSVC build |

Other Herdr or Linux desktop versions are rejected rather than falling back to a sleep blocker that could also affect locking or manual sleep. See [Compatibility](docs/compatibility.md) for platform details and limitations.

The normal GitHub installation builds the plugin for you. Have [Rust/Cargo](https://rust-lang.org/tools/install/) **1.98.0** and native build tools available to Herdr: a C toolchain on Linux, Apple's command-line tools on macOS, or Microsoft C++ (MSVC) build tools on Windows. You do not need to clone the repository or build it manually. If you want an already-built executable instead, use [Advanced installation](docs/advanced-installation.md).

## Get started

### 1. Install

```sh
herdr plugin install chpock/herdr-idle-inhibitor
```

### 2. Open status and settings

Run this in a terminal pane inside a running Herdr window:

```sh
herdr plugin action invoke show --plugin herdr-idle-inhibitor
```

This activates monitoring and opens the **Idle Inhibitor** popup on **Status**. Press `s` for **Settings**. Closing the popup with `Esc` does **not** stop monitoring.

**On Hyprland:** complete [Hypridle setup](docs/hypridle-setup.md), then turn on **Hypridle integration confirmed** in Settings. Until you do, the plugin reports `setup_required` and does not prevent idle sleep. This setup keeps display-off and locking independent of sleep prevention.

### 3. Check it while an agent works

Start an agent task that Herdr reports as `working`. In the popup, check:

- **Work: Working** — Herdr reports at least one working agent.
- **Observation: Complete** — the registered sessions have current observations.
- **Request: accepted** — the selected OS/desktop mechanism acknowledged the request.

KDE can show `pending` for about five seconds before accepting a request. A setup error or `failed` request is not sleep protection, even when Work says Working. If the popup does not open or the request is not accepted, start with [Troubleshooting](docs/troubleshooting.md).

You can now close the popup and use Herdr normally. Server startup and agent events activate monitoring as needed; you do not need to keep this window open.

## Everyday use

Open the same popup command whenever you want to inspect work or change settings.

| Key | Action |
| --- | --- |
| `p` | Pause / Resume sleep prevention |
| `s` | Open Settings |
| `d` | Open Details and diagnostics; Enter returns to Status |
| Tab / Shift-Tab or Up / Down | Move focus in Status and Settings |
| Enter / Space | Activate the focused control |
| `Esc` / `q` / Ctrl+C | Close the popup from any view |

**Pause** releases this plugin's sleep-prevention request but keeps observing agents. It applies to all monitored sessions and is saved across restarts and updates. It has no timeout: press `p` again or choose **Resume** when you want protection back. Closing/reopening the popup does not Resume. If **PAUSE NOT SAVED** appears, the live Pause applies but could be lost on restart; see [configuration errors](docs/troubleshooting.md#configuration-failures).

In **Settings**, you can change the release delay, Pause, and Linux desktop selection. Focus **Release delay** and use Left / Right to choose 0–60 seconds. Changes take effect and are saved immediately when acknowledged; there is no separate Save button. Most users can keep the defaults, except for the required Hypridle confirmation. File locations, manual edits and extra endpoints are in [Configuration](docs/configuration.md).

The popup needs at least **50 columns × 10 rows**. In Details, use Up / Down or PageUp / PageDown to scroll.

## Update

Run the install command again:

```sh
herdr plugin install chpock/herdr-idle-inhibitor
```

The background monitor switches to the updated code automatically, preserving settings, sessions and Pause. No manual restart, Pause or disable/enable cycle is needed. An identical executable does not cause a restart; changing code is detected even if its version number stays the same.

Sleep protection can briefly be absent while the old process hands over to the new one. A Windows popup may close during replacement; reopen it with the usual popup command. If installation fails, inspect [update troubleshooting](docs/troubleshooting.md#automatic-update) and retry after fixing the reported error. The plugin does not undo Herdr's file installation.

## Disable, re-enable or remove

To stop using the plugin, close its popup and disable it:

```sh
herdr plugin disable herdr-idle-inhibitor
```

The monitor normally notices within about five seconds and stops after releasing its request. If you use multiple Herdr configuration directories, disable it in each one; another enabled installation keeps the shared monitor alive. For an explicit stop check, use the [status command](docs/status-api.md#finding-the-executable).

To enable it again and activate it immediately:

```sh
herdr plugin enable herdr-idle-inhibitor
herdr plugin action invoke show --plugin herdr-idle-inhibitor
```

Saved Pause remains unchanged; Resume if you intentionally want to end it.

To remove a GitHub installation, disable it as above and wait for the monitor to stop, then run:

```sh
herdr plugin uninstall herdr-idle-inhibitor
```

Settings and logs remain in the [application data directories](docs/configuration.md#file-locations), so reinstalling does not reset your preferences. For a linked bundle or checkout, see [linked installation maintenance](docs/advanced-installation.md#linked-installation-maintenance).

## Important limits

- Only Herdr's reported `working` state counts, not CPU activity, terminal output or arbitrary remote/background tasks. Other users and remote-host power control are outside the scope.
- Lost observations mean **unknown**, not that all work finished. An existing justified request can be retained briefly, but errors do not keep the computer awake indefinitely. See [How it works](docs/behavior.md).
- Native acceptance is not a guarantee against every OS policy. In particular, **Windows Modern Standby on battery can still sleep** despite a request.
- **PowerDevil 6.7.5:** a user-suppressed request can lose its owner association, survive monitor termination and reactivate when later allowed. See the [exact cleanup limitation](docs/compatibility.md#powerdevil-suppressed-request-cleanup).
- The plugin does not rewrite power plans, handle lid policy, block manual sleep, simulate input, or override other applications' inhibitors.

Linux is the only workstation-tested platform. Native hosted checks have also exercised macOS and Windows builds and power APIs, but physical sleep behavior and installed-Herdr lifecycle are not certified. The automatic-update mechanism has been exercised on Linux; its new Windows/macOS code is type-checked, with fresh native checks still pending. The detailed testing boundaries are in [Compatibility](docs/compatibility.md). Please [open an issue](https://github.com/chpock/herdr-idle-inhibitor/issues) if something does not work; [Troubleshooting](docs/troubleshooting.md#reporting-an-issue) explains what to include.

## Learn more

| Guide | Read it when you want to… |
| --- | --- |
| [Configuration](docs/configuration.md) | Find/edit settings, select a Linux desktop mechanism, or add custom local endpoints |
| [Hypridle setup](docs/hypridle-setup.md) | Configure Hyprland so idle sleep is held back but display-off and locking still work |
| [How it works](docs/behavior.md) | Understand observation, timing, shared monitoring, recovery and automatic updates |
| [Compatibility](docs/compatibility.md) | Check exact platform support, OS mechanisms, testing coverage and known limitations |
| [Troubleshooting](docs/troubleshooting.md) | Diagnose setup, work detection, updates and native request failures |
| [Status API](docs/status-api.md) | Query `status --json` from a script and interpret its result safely |
| [Advanced installation](docs/advanced-installation.md) | Install an available binary bundle, choose a repository revision, or maintain a linked installation |
| [Development](docs/development.md) | Obtain source, build and link your own version, run tests, or create a bundle |

## License

[GPL-3.0-only](LICENSE).
