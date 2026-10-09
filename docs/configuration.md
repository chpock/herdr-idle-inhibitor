# Configuration

[Home](../README.md) · [Usage](usage.md) · [Hypridle setup](hypridle-setup.md)

Settings are shared by the current OS user's monitor, not by workspace, Herdr server or plugin checkout. The monitor is the application's only configuration writer. Configuration stores preferences, not agent history or proof that a native request exists.

## File locations

| OS | Configuration | Log directory |
| --- | --- | --- |
| Linux | `${XDG_CONFIG_HOME:-$HOME/.config}/herdr-idle-inhibitor/config.toml` | `${XDG_STATE_HOME:-$HOME/.local/state}/herdr-idle-inhibitor/` |
| macOS | `~/Library/Application Support/herdr-idle-inhibitor/config.toml` | `~/Library/Logs/herdr-idle-inhibitor/` |
| Windows | Known Folder LocalAppData, `herdr-idle-inhibitor/config.toml` | Same application directory, `logs/` |

On Windows the OS Known Folder location is authoritative; changing the text of an environment variable is not a way to relocate it. The private runtime lock lives under that application's `runtime/` directory. Unix rendezvous uses a private `herdr-idle-inhibitor-<uid>` directory beneath canonical `/tmp`.

A missing config is initialized with defaults when monitoring is activated, never by `status --json`. Invalid startup configuration prevents acquisition. A read-only status query does not create these directories or files.

## Default file

```toml
schema_version = 1
paused = false
release_delay_secs = 5
additional_endpoints = []

[linux]
backend = "auto"
hypridle_integration_confirmed = false
```

Missing settings receive defaults. Unknown keys, wrong types, unsupported schema versions, invalid backend names and out-of-range values are rejected rather than silently ignored or clamped.

| Setting | Accepted values | Meaning |
| --- | --- | --- |
| `schema_version` | `1` | Configuration format version; separate from the app and public JSON version |
| `paused` | Boolean; default `false` | Persistent shared Pause until Resume or an intentional config change |
| `release_delay_secs` | Integer 0–60; default `5` | Grace after confirmed completion of the last work |
| `additional_endpoints` | Array of up to 128 absolute local paths; default empty | Extra native Herdr endpoints within the current user's local scope |
| `linux.backend` | `auto`, `hypridle`, `gnome`, `kde` | Linux desktop mechanism selection |
| `linux.hypridle_integration_confirmed` | Boolean; default `false` | Your acknowledgment that the required Hypridle listener setup is complete |

The `[linux]` section is accepted on every platform, but backend selection and Hypridle acknowledgment only affect Linux. Timing other than release delay is fixed; see [Usage](usage.md#timing-and-recovery).

## Linux backend selection

`auto` uses the registered `XDG_CURRENT_DESKTOP` context to choose Hyprland/Hypridle, GNOME or KDE/Plasma. Missing or ambiguous desktop context requires an explicit supported selection in Settings/TOML.

An explicit backend does **not** bypass desktop-version checks, required services, permissions or Hypridle setup. It does not select a stronger fallback. Conflicting desktop/session-bus registrations are reported instead of switching one live request between unrelated desktops.

Only set `hypridle_integration_confirmed = true` after following [Hypridle setup](hypridle-setup.md). It is a user acknowledgment, not automatic configuration detection.

## Additional local endpoints

Normally registration plus `herdr session list --json` discovers the relevant servers. A server with an unusual endpoint or configuration root must register through its plugin activation or be listed explicitly.

Use the exact native endpoint path reported by Herdr, such as `socket_path` in its session listing or the `HERDR_SOCKET_PATH` provided to plugins. On Unix this is a socket path. On Windows Herdr uses that exact local path string to name its pipe; do not substitute a guessed pipe address or change its case.

Unix example:

```toml
additional_endpoints = ["/absolute/path/to/custom-herdr.sock"]
```

Windows TOML example, using single quotes to preserve backslashes:

```toml
additional_endpoints = ['C:\Users\example\AppData\Local\herdr\custom.sock']
```

These are illustrative paths, not defaults. URLs, NULs and Windows remote UNC paths are rejected. Current-user peer checks still apply. Listing an endpoint does not grant access to another user or authorize native acquisition without a freshly enabled plugin installation.

## Editing and persistence

- Popup settings take effect through acknowledged operations on the shared monitor.
- Manual edits are loaded at startup or by **Reload** in Settings; there is no file watcher.
- A rejected reload keeps the last valid runtime settings and reports the failure. The invalid file is not overwritten automatically.
- If the file changed externally, a popup save is rejected until you Reload; stale popup state cannot silently overwrite your edits.
- Pause applies in memory even if saving fails, with **PAUSE NOT SAVED** visible. Resume requires successful saving.

If the popup is unavailable, edit the file and follow the [disable/stop/reactivate sequence](installation.md#update) to load it at the next monitor startup. Do not assume that another event or a status query hot-reloads a running monitor.

The application replaces configuration through a temporary file in the same directory. Unix configuration directories/files are user-private. No database or native ownership flag is persisted.

## Path overrides

Advanced environments may set these before starting the first monitor:

| Environment variable | Value |
| --- | --- |
| `HERDR_IDLE_INHIBITOR_CONFIG` | Absolute path to the configuration file |
| `HERDR_IDLE_INHIBITOR_STATE` | Absolute path to the log/state directory |

Relative paths are rejected. Overrides apply to the monitor at startup. An already-running monitor continues using its own paths; another popup or Herdr root does not create a second configuration authority. To relocate them, stop the monitor first and provide consistent environment settings for reactivation. Do not copy runtime locks or infer power-request ownership from saved files.
