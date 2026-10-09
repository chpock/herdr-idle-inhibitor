# Herdr Idle Inhibitor

A Rust plugin initially qualified for **Herdr 0.9.3**. Other Herdr versions are rejected until their API is qualified; the manifest's minimum version is not a promise of compatibility with every later version. One background monitor observes local Herdr servers belonging to the current user and requests **idle-sleep prevention only while agents report `working`**.

- The display may turn off and the session may lock.
- No lid control, manual-sleep blocking, forced suspend, input simulation, administrator helper, or installed OS service.
- `blocked`, `idle` and `done` do not count as working. Missing/failed observations are unknown, not zero agents.
- Closing its Herdr popup does not stop monitoring. Pause is shared across servers and persists until Resume.
- The only stable external API is read-only `status --json`; external consumers poll.

## Compatibility and verification

**Runtime testing has only been performed on Linux. macOS and Windows are implemented and expected to work, but have not been physically tested. Please open an issue if they do not.** GNOME/KDE framing is tested against isolated services; this is not a claim of physical desktop sleep certification. GitHub native build/test jobs are configured, not claimed to have run before publication.

| Platform | Adapter / initial source-qualified profile | Verification boundary |
| --- | --- | --- |
| Linux x86_64 GNU | Hyprland + Hypridle **0.1.8**, logind `idle / block` | Local native acquire/release and abrupt-owner cleanup; requires the listener setup below |
| Linux x86_64 GNU | GNOME session **51.0**, suspend-only flag `4` | Source review and isolated D-Bus tests; no physical GNOME certification |
| Linux x86_64 GNU | PowerDevil **6.7.5**, `InterruptSession` only | Source review and isolated D-Bus tests; real pending delay and user suppression; accepted cleanup limitation below |
| macOS arm64 / x86_64 | IOKit `PreventUserIdleSystemSleep` | Deployment target macOS 13; native CI tests configured, physical tests unavailable |
| Windows x86_64 | `PowerRequestSystemRequired` | Windows 10+ native host; native CI tests configured, physical tests unavailable |

Unknown Linux profiles/versions produce an actionable error rather than a stronger fallback inhibitor. Initial Linux artifacts are built on Ubuntu 24.04 (glibc 2.39); older libc compatibility is not promised. WSL, containers controlling the host, remote machines and concurrent mixed-desktop login sessions are outside the supported deployment.

### Accepted limitations

**Windows Modern Standby on battery:** Windows may stop honoring an ordinary request five minutes after the system sleep timeout expires. This is not five minutes after starting an agent. System policy can also ignore application requests. There is no workaround or power-plan modification; `accepted` does not mean guaranteed awake. See [API evidence](docs/research/architecture-evidence.md#w1-windows-has-a-documented-batterypower-model-limit).

**KDE PowerDevil 6.7.5:** if the user suppresses an active inhibitor through KDE, the monitor then dies, and the user later allows that application/reason again, PowerDevil can reactivate an orphaned request. Its suppression path drops owner tracking but retains the cookie. Graceful explicit release is different and is still attempted. The owner explicitly accepted this upstream exception rather than removing KDE support. No watchdog or policy-changing workaround is added. See [the source-verified finding](docs/research/kde-suppression-cleanup.md). An orphan can require desktop-side cleanup by the user; this application does not forcibly release other applications' requests or restart PowerDevil.

A frozen/stopped process is not a terminated one: it can retain its native resource until resumed or terminated. Retention deadlines assume the monitor is executing. Herdr does not supervise a crashed detached monitor; the next hook or explicit plugin action can reactivate it, but recovery has no finite guarantee if no further event occurs.

## Install and activate

### Source checkout

Requires Rust/Cargo **1.98.0**, the checked-in lockfile, and the selected desktop's normal services. No Node.js, Python, jq or power-management command wrapper is needed at runtime.

```sh
cargo build --release --locked --target-dir target
```

Install this repository through Herdr's normal plugin installation mechanism. Its source manifest runs that build. Enable it and invoke **Idle Inhibitor: status and settings** (`show`) for explicit first activation, including when agents were already working before installation. Installing a plugin does not necessarily run its startup hook on an already-running server. Installation in this working tree is not performed automatically by development/tests.

The source manifest uses one extensionless native executable path for all platforms, preserving unique action/pane IDs; Windows process creation resolves its `.exe`. Prebuilt Windows manifests use the explicit `.exe` path.

### Prebuilt bundles

The development packager produces a bundle containing the executable at the same `target/release/` entrypoint, a generated **build-free** manifest, README, license and setup/compatibility documentation. Rust/Cargo and the packaging Python script are not runtime prerequisites for a bundle.

No release, signing, notarization or marketplace submission is performed automatically. macOS archives are unsigned/unnotarized; quarantine/Gatekeeper can prevent execution. Windows can show SmartScreen warnings. Review the source/checksum and use the OS's intentional per-application approval process; do not globally disable security protections. Physical clean-machine installation has not been certified.

## Hyprland setup — required

Follow [the full Hypridle checklist and listener example](docs/hypridle-setup.md). In summary:

- `general.ignore_systemd_inhibit = false`;
- dim/display-off/lock listeners: `ignore_inhibit = true`;
- idle-suspend listeners: do **not** ignore inhibition;
- preserve your existing timeouts, commands and resume actions;
- only then enable **Hypridle integration confirmed** in Settings.

Per-listener ignoring applies to **all inhibitors**, not just this plugin. Confirmation is a user acknowledgment, not automatic configuration detection. The plugin never rewrites your Hypridle configuration or restarts services.

## Popup controls

Invoke the plugin's `show` action. The popup separately displays work evidence, policy intent, native ownership and request state; KDE `pending`/`suppressed` are not `accepted`.

- Tab / Shift-Tab / arrows: navigate; Enter / Space: select.
- `p`: Pause / Resume; `s`: Settings; `d`: Details.
- Escape / `q`: close; closing never withdraws a native request by itself.
- Details: Up/Down or PageUp/PageDown scroll; Enter returns to Status.
- Usable geometry is at least **50 columns × 10 rows**. Below it, only close keys are handled; hidden controls cannot be activated. Status/Settings keep the focused action visible. Long diagnostics are available in Details.

Settings expose Pause, release delay (0–60 seconds), Linux backend choice, Hypridle acknowledgment and explicit Reload. Additional local endpoints are an advanced TOML setting. Passive popup refresh never restarts a missing monitor; **Retry / activate** is explicit. Another open Herdr popup or no attached UI produces an error, not a permanent-panel fallback.

Pause keeps observing agents and withdraws this application's request. It is saved across restarts. If saving fails, every popup keeps a prominent **PAUSE NOT SAVED** warning while the shared state is unsaved, including after closing/reopening or another success notice; restart may restore the previous setting. Resume requires successful persistence. Closing/reopening a popup cannot silently Resume.

## Configuration

Defaults: `paused = false`, release delay **5 seconds**, snapshot reconciliation **2 seconds**, root discovery/eligibility **5 seconds**, observation-loss retention **30 seconds**. Intervals are scheduling targets, not hard real-time promises.

```toml
schema_version = 1
paused = false
release_delay_secs = 5
additional_endpoints = []

[linux]
backend = "auto" # auto | hypridle | gnome | kde
hypridle_integration_confirmed = false
```

| OS | Configuration | Logs |
| --- | --- | --- |
| Linux | `${XDG_CONFIG_HOME:-$HOME/.config}/herdr-idle-inhibitor/config.toml` | `${XDG_STATE_HOME:-$HOME/.local/state}/herdr-idle-inhibitor/` |
| macOS | `~/Library/Application Support/herdr-idle-inhibitor/config.toml` | `~/Library/Logs/herdr-idle-inhibitor/` |
| Windows | Known Folder LocalAppData, `herdr-idle-inhibitor/config.toml` | Same application directory, `logs/` |

Advanced owner-startup overrides are absolute paths in `HERDR_IDLE_INHIBITOR_CONFIG` and `HERDR_IDLE_INHIBITOR_STATE`. Popup/bootstrap clients use the existing owner's paths; these overrides do not create a second per-workspace authority.

Manual edits require **Reload**. Invalid startup config prevents acquisition. A rejected reload keeps the last valid runtime settings and reports the error. An externally changed file is not silently overwritten by a popup save.

`additional_endpoints` accepts only explicit local native Herdr endpoints. Current-user peer authentication still applies. Discovery covers registered roots and endpoints, not arbitrary unregistered custom roots or other users. Conflicting desktop/bus registrations are rejected.

When work ends, only this application's request is released after the delay; it never asks the OS to sleep. Observation loss can retain an **existing** request briefly, never justify a new one, and never renew the original deadline. Pause, disable and shutdown do not receive completion grace.

## External status API

Use the actual executable path from your plugin checkout/bundle; Herdr does not add it to PATH:

```sh
/path/to/plugin/target/release/herdr-idle-inhibitor status --json
```

Windows PowerShell:

```powershell
& 'C:\path\to\plugin\target\release\herdr-idle-inhibitor.exe' status --json
```

No `HERDR_*` invocation context is required. The command outputs one JSON object and newline within a two-second query deadline. It never starts the monitor, creates config/directories, refreshes Herdr or changes native requests. A live monitor with incomplete observation or backend failure returns a valid response; a missing monitor returns nonzero with unknown/null fields, **not zero agents**.

See [the v1 contract and exit codes](docs/status-api.md) and [JSON Schema](schemas/status-v1.json). Work, desired inhibition, owned resource and accepted request are different fields. `known_limitations` are conditional caveats, not hardware/state detectors. Internal underscore CLI roles and local control IPC are not a second stable public API.

## Updates, disable and uninstall

1. Pause through the popup if you want immediate withdrawal.
2. Disable this plugin in **all** registered Herdr roots and wait for the monitor to stop. Normal registry reconciliation detects disable; failed checks can take the bounded retention interval. `status --json` should report unavailable after shutdown.
3. Only then update/remove the checkout or executable. Do not replace a running Windows executable; a failed replacement is not a successful update.
4. Re-enable and invoke `show` after updating. Saved Pause remains until explicit Resume.

No OS service needs uninstalling. User config/logs are left intact; remove them manually only if desired. Do not delete a live singleton lock or use a PID file to kill an unrelated process. Running another idle inhibitor during validation can mask failures; manage those plugins yourself rather than having this one alter them.

## Development and evidence

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Native GitHub jobs cover Linux, both shipped macOS architectures and Windows MSVC, with opt-in request/assertion ownership probes on macOS/Windows. Those probes test API lifecycle, not hardware sleep effectiveness. Packaging tests ensure a build-free native bundle and matching entrypoints.

Minimized real Herdr 0.9.3 captures are under `tests/fixtures/herdr-0.9.3/`; request/terminal IDs are normalized, and prompts/titles/paths/tokens are discarded. Synthetic completions/faults are labeled separately. Integration replays pass captured work through native transport, parsing, controller, policy and public-status validation with a fake native resource; actual logind ownership is checked separately. Production logs contain bounded state transitions/errors, not raw agent traffic.

See [implementation record](docs/implementation-notes.md), [validation evidence](docs/validation/end-to-end.md), [approved architecture](docs/implementation-plan.md), [validation contract and amendments](docs/implementation-validation.md), and independent reports in `docs/reviews/`. Unperformed hardware/installation/hosted-CI checks are not called passes.

## License

GPL-3.0. See [LICENSE](LICENSE).
