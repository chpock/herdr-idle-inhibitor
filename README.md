# Herdr Idle Inhibitor

A native Rust plugin for **Herdr 0.9.3** that requests prevention of automatic idle sleep while local agents report `working`.

- The display can turn off and the session can lock.
- Lid actions and manual sleep stay under the operating system's control.
- `blocked`, `idle` and `done` do not count as working. Failed observations remain unknown.
- One background monitor and shared, persistent Pause cover the current user's registered local Herdr servers.
- Finishing work releases this plugin's request; it never tells the computer to sleep.
- The read-only external interface is `status --json`.

## Get started

Install from GitHub with Rust/Cargo **1.98.0** available:

```sh
herdr plugin install chpock/herdr-idle-inhibitor
```

See [Installation](docs/installation.md) for local checkouts, prebuilt bundles, activation, updates and removal. **Hyprland users must complete [Hypridle setup](docs/hypridle-setup.md) before enabling inhibition.**

The plugin action is **Idle Inhibitor: status and settings** (`show`). **Known issue:** its current popup-opening command is incompatible with Herdr 0.9.3's CLI arguments. See [Popup does not open](docs/troubleshooting.md#popup-does-not-open); a failed popup is not proof that the monitor stopped.

Query the executable in your installation; Herdr does not add it to PATH:

```sh
/path/to/plugin/target/release/herdr-idle-inhibitor status --json
```

A missing monitor returns unknown/null state and a nonzero exit code, not zero working agents. Queries never start monitoring or change power requests.

## Documentation

| Guide | Contents |
| --- | --- |
| [Installation](docs/installation.md) | Requirements, source/prebuilt installation, activation, updates and uninstall |
| [Usage](docs/usage.md) | Work detection, popup controls, Pause and background lifecycle |
| [Configuration](docs/configuration.md) | TOML settings, file locations, manual reload and path overrides |
| [Compatibility](docs/compatibility.md) | Supported versions/architectures, platform mechanisms and known limits |
| [Troubleshooting](docs/troubleshooting.md) | Diagnostics, common errors, logs and issue reporting |
| [Hypridle setup](docs/hypridle-setup.md) | Required listener configuration for Hyprland |
| [Status API](docs/status-api.md) | JSON v1 fields, examples, exit codes and consumer rules |

## Platform support

Linux x86_64 supports Hypridle **0.1.8**, GNOME session **51.0** and PowerDevil **6.7.5**. macOS supports arm64/x86_64; Windows supports x86_64. Other Herdr or Linux desktop versions are rejected rather than silently selecting a broader sleep inhibitor.

Local workstation checks were performed on Linux. Native hosted build, integration, API-lifecycle and bundle checks passed for all four targets; **physical sleep-effectiveness and installed-plugin lifecycle testing are not claimed**. macOS and Windows are expected to work; please [open an issue](https://github.com/chpock/herdr-idle-inhibitor/issues) if they do not.

Windows Modern Standby on battery can limit an ordinary request. PowerDevil has a specific suppressed-request cleanup limitation. Read [Compatibility](docs/compatibility.md) before relying on unattended work: a native request being accepted is not a guarantee the OS cannot sleep.

## License

[GPL-3.0-only](LICENSE).
