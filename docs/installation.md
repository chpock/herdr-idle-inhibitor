# Installation

[Home](../README.md) · [Compatibility](compatibility.md) · [Hypridle setup](hypridle-setup.md)

## Requirements

- Herdr **0.9.3**, running as your normal OS user on a native Linux, macOS or Windows host.
- A [supported desktop/OS profile](compatibility.md).
- For source installation: Rust/Cargo **1.98.0** and the platform's native linker/build tools. macOS source builds need Apple's command-line tools; Windows uses the MSVC toolchain.
- For a prebuilt bundle: no Rust, Cargo, Node.js, Python, jq or power-management command wrapper is required at runtime.

Run unelevated. There is no administrator helper or OS service to install. The plugin does not change desktop timers, power plans, or other plugins.

## Install from GitHub

```sh
herdr plugin install chpock/herdr-idle-inhibitor
```

Herdr runs the source manifest's build command:

```sh
cargo build --release --locked --target-dir target
```

To select a particular repository revision, use Herdr's `--ref` option. The executable is placed at `target/release/herdr-idle-inhibitor` (`herdr-idle-inhibitor.exe` on Windows) under the installed plugin directory.

## Link a local checkout

From outside or inside Herdr, link an absolute path to a checkout containing `herdr-plugin.toml`:

```sh
herdr plugin link /absolute/path/to/herdr-idle-inhibitor --enabled
```

Linking keeps that directory as the plugin source; do not delete it while the plugin is registered. The source manifest builds the executable with Cargo.

## Prebuilt bundles

Select the archive matching your host:

| Host | Target | Archive |
| --- | --- | --- |
| Linux x86_64 GNU | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.gz` |
| Windows x86_64 MSVC | `x86_64-pc-windows-msvc` | `.zip` |

Repository installation builds from source; it does not automatically download a binary. Native bundles may be available as artifacts of successful repository [Actions runs](https://github.com/chpock/herdr-idle-inhibitor/actions). CI artifacts expire and are not signed releases.

1. Download the archive and its `.sha256` companion. If downloaded as an Actions artifact, first extract the outer artifact ZIP.
2. Verify the archive's SHA-256 against the companion file. A checksum detects corruption; it is not a signature or independent proof of publisher identity.
3. Extract the archive into a persistent directory. Preserve the `target/release/` layout and the generated `herdr-plugin.toml`.
4. Link the extracted plugin directory with `herdr plugin link /absolute/path/to/extracted-plugin --enabled`.

The bundle's manifest has no build step. Its documentation, JSON Schema, license and matching vendored source are included. Do not substitute the source-checkout manifest: doing so reintroduces a Cargo build requirement. Windows bundle entrypoints explicitly name the `.exe`.

macOS bundles are unsigned/unnotarized; quarantine or Gatekeeper can prevent execution. Windows may show SmartScreen warnings. Use the OS's intentional per-application approval process only after inspecting the source and origin; do not globally disable security protections. See [Compatibility](compatibility.md#distribution-and-os-security).

## First activation

1. Confirm installation and enabled state with `herdr plugin list`. Enable it if necessary:

   ```sh
   herdr plugin enable herdr-idle-inhibitor
   ```

2. With a Herdr UI attached, invoke **Idle Inhibitor: status and settings** from Herdr's actions, or:

   ```sh
   herdr plugin action invoke show --plugin herdr-idle-inhibitor
   ```

   **Current limitation:** the action's popup-opening command fails argument parsing on Herdr 0.9.3. Its monitor-activation step precedes that command; inspect the [known popup issue](troubleshooting.md#popup-does-not-open) and query status rather than assuming activation failed or succeeded.

3. On Hyprland, apply [Hypridle setup](hypridle-setup.md). The plugin refuses to acquire until `linux.hypridle_integration_confirmed` is true. Settings can also be edited in the [configuration file](configuration.md) and loaded at monitor startup.
4. Check work, Pause and native request state separately. A running monitor is not itself proof of idle-sleep protection.

Herdr's startup hook does not necessarily run when a plugin is enabled in an already-running server. Other activation opportunities are server startup and supported agent/pane/workspace events. A standalone `status --json` query deliberately does not activate it.

## External executable path

Herdr does not place the executable on PATH. Use the absolute path in your checkout/bundle, or add a PATH entry yourself if desired.

Unix:

```sh
/path/to/plugin/target/release/herdr-idle-inhibitor status --json
```

Windows PowerShell:

```powershell
& 'C:\path\to\plugin\target\release\herdr-idle-inhibitor.exe' status --json
```

No Herdr invocation environment is required for this read-only command. See [Status API](status-api.md).

## Update

There is no automatic updater. Before replacing source or an executable:

1. Pause in the popup if available and immediate withdrawal is needed.
2. Close any plugin popup and disable this plugin in **every registered Herdr configuration root**:

   ```sh
   herdr plugin disable herdr-idle-inhibitor
   ```

3. Wait for `status --json` to return `monitor_unavailable` (exit 3). Disabling one installation does not stop a monitor still serving another enabled root. Successful registry checks normally run every five seconds; failed checks have bounded retention.
4. Update/rebuild the checkout or replace the extracted bundle. A running Windows executable can be locked; a failed replacement is not a successful update.
5. Re-enable and activate through Herdr. Saved Pause remains until explicit Resume or an intentional configuration change.

Do not remove a live singleton lock, kill a process solely because a PID appeared in status, or expect a zero-gap live update.

## Uninstall

Follow the disable/stop steps above first. Then:

- GitHub-installed plugin: `herdr plugin uninstall herdr-idle-inhibitor`.
- Linked checkout/bundle: `herdr plugin unlink herdr-idle-inhibitor`, then remove its directory if desired.

User configuration and logs are outside the plugin directory and remain intact. Remove the [application data paths](configuration.md#file-locations) manually only if you no longer need them. No OS service needs removal. The plugin does not uninstall or release another application's inhibitors.
