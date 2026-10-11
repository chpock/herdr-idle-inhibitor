# Development

[Home](../README.md) · [How it works](behavior.md) · [Advanced installation](advanced-installation.md)

Use this guide to build, modify or test your own version. Ordinary users do not need a checkout or these commands: Herdr builds the GitHub installation itself, as described in the [README](../README.md#get-started).

## Toolchain

- Rust/Cargo **1.98.0**. `rust-toolchain.toml` selects this version and includes rustfmt and Clippy when using rustup.
- Git to obtain the repository.
- Native linker/build tools: a C toolchain on Linux, Apple's command-line tools on macOS, or the Windows MSVC toolchain.
- Herdr **0.9.3** and a [supported desktop/OS](compatibility.md) to run the plugin through Herdr.
- Python **3.13** for packaging scripts and their tests; Python is not a runtime dependency of the plugin.
- Linux backend tests also need `dbus-daemon` for their isolated buses. Bubblewrap is useful for running process tests without access to an already-running monitor.

Build natively for your host. CI covers Linux x86_64 GNU, macOS arm64/x86_64 and Windows x86_64 MSVC. For the macOS 13 deployment target used in CI, set `MACOSX_DEPLOYMENT_TARGET=13.0` before building; a default local build need not match that target. See [Compatibility](compatibility.md) for binary baselines and testing boundaries.

## Get source and build

```sh
git clone https://github.com/chpock/herdr-idle-inhibitor.git
cd herdr-idle-inhibitor
cargo build --release --locked --target-dir target
```

The executable is `target/release/herdr-idle-inhibitor`, or `target/release/herdr-idle-inhibitor.exe` on Windows. A successful Cargo build alone does not register or activate it in Herdr.

Do not use `cargo install` as a plugin installation method: Herdr needs the repository's manifest and directory layout, not just an executable on PATH.

## Connect your checkout to Herdr

Use an absolute path to the repository root, the directory containing `herdr-plugin.toml`:

```sh
herdr plugin link /absolute/path/to/herdr-idle-inhibitor --enabled
```

On Windows, quote the absolute path, for example `"C:\Users\you\src\herdr-idle-inhibitor"`.

Herdr runs the source manifest's build steps when linking, so it may rebuild the executable you just produced. After Cargo succeeds, its next build step prepares automatic activation/update. Do not invoke internal underscore-prefixed commands yourself: the manifest supplies their installation context and order.

If the same plugin ID is currently GitHub-installed, [disable and uninstall that installation](../README.md#disable-re-enable-or-remove) first. Do not attempt to use both copies under one Herdr configuration directory. Settings and Pause are shared application data and survive this switch. If replacing another linked checkout, use the [linked installation instructions](advanced-installation.md#linked-installation-maintenance).

Keep the linked checkout at that path while registered. Monitoring starts automatically at Herdr server startup and on agent detection or status changes. Use the [optional manual start](../README.md#optional-manual-start) if you want it to begin immediately. Inspect the request using the [README checks](../README.md#2-open-status-and-settings); linking is not proof that sleep prevention is configured or working. If Hypridle also delays display-off/locking, see [Linux display troubleshooting](troubleshooting.md#linux-screen-does-not-turn-off); that tuning is optional.

## Change and rebuild a linked checkout

Edit source, then rebuild the artifact referenced by the manifest:

```sh
cargo build --release --locked --target-dir target
```

`cargo build` without `--release` updates the debug artifact, not the installed plugin executable. The monitor independently checks the linked release executable and detects changed contents, even if you keep the same package version. The process switches automatically, preserving registered sessions, settings and Pause, then obtains fresh observations. No disable/enable cycle is required. The popup can close during the switch.

On Windows, close the current plugin popup before rebuilding its installed `.exe` in place. The long-lived monitor runs from a private cached copy, but the popup still executes from the checkout. GitHub installation builds in a separate directory and handles its clients automatically; an in-place developer build is different.

An unchanged executable does not trigger a restart. A failing replacement is not endlessly retried automatically; correct the code and produce a new artifact, or retry the normal Herdr installation/link operation. Read [automatic-update behavior](behavior.md#automatic-updates) and [update troubleshooting](troubleshooting.md#automatic-update) for recovery semantics.

## Run checks safely

These are the normal source checks:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python -m unittest discover -s scripts -p "test_*.py" -v
```

**Process tests need an isolated environment with no existing monitor for that OS user.** Unix rendezvous is per-user under canonical `/tmp`; changing HOME, application paths or `TMPDIR` does not isolate it. `tests/cli.rs` deliberately refuses a live monitor. Use a disposable native CI environment, or on Linux with Bubblewrap run the Rust suite in private `/tmp` and PID namespaces:

```sh
bwrap --bind / / --tmpfs /tmp --dev-bind /dev /dev --proc /proc \
  --unshare-pid --die-with-parent cargo test --locked
```

Keep the checkout outside `/tmp`, since the private mount hides its original contents. This hides the user's control socket and contains fixture processes. It is not a sandbox for untrusted code: other host paths remain mounted. Do not stop your real monitor or delete its lock just to make a test pass.

Linux adapter tests use isolated D-Bus services. Controller/process tests use fake Herdr transport and minimized captured Herdr responses; passing them is not a physical idle-sleep test or an installed-Herdr lifecycle claim.

Windows/macOS opt-in native probes are excluded from the default suite. They include real power-request acquisition/release, notification lifecycle, and Windows executable replacement-lock checks. Run them intentionally on the corresponding native host, as CI does:

```sh
cargo test --locked --lib -- --ignored
```

Linux process tests cover automatic executable replacement, state handover and recovery against captured Herdr responses. Cross-target type checking is useful, but is not native execution or proof of filesystem replacement on another OS.

### Software SHA in process tests

`Cargo.toml` optimizes **only `sha2` in the test profile** (`opt-level = 3`). This is intentional: update process tests execute Cargo's test-built application and hash real executables while detecting, caching, verifying and starting a replacement. Removing debug symbols reduces file size but does not optimize SHA-256. Automatic detection can already consume part of the unchanged ten-second test deadline because its scan interval is five seconds; repeated unoptimized hashes add CPU work unrelated to the behavior under test.

This was added after [Linux CI run 38061716775](https://github.com/chpock/herdr-idle-inhibitor/actions/runs/38061716775), on source `9e905398aaa32cc7f2b90db6f8187741f0ed481f`, failed `resident_monitor_detects_same_version_file_replacement_without_herdr_events`. The saved report showed that the previous owner had exited successfully and the replacement was responding, but no discovery/snapshot attempt had started at its last reply. Its reported uptime includes executable hashing before the scheduler starts; uptime alone is not evidence of timer starvation.

Controlled Linux checks on four CPUs, without source changes or longer deadlines, established:

- The ordinary parallel run passed all 14 update process tests.
- Forcing the supported `sha2/force-soft` backend reproduced the same ten-second assertion failure **with just that test running**. Concurrent fixtures were not necessary. In this reproduction the previous monitor was still working and the updater had reached candidate verification; this was an earlier phase than the original CI failure.
- Optimizing only the test-profile `sha2` dependency made all 14 process tests pass with forced software SHA and the ordinary parallel runner.

These results establish a reproducible sensitivity to unoptimized hashing, not the original runner's CPU features or exact time distribution: those were not captured. Optimization also changes generated executable bytes/size, so the experiment does not isolate compression throughput alone. It does not establish a universal update-latency guarantee or a configuration-reload defect.

The override uses [Cargo's package-specific profiles](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides). Application code remains at the normal test optimization level; debug assertions, overflow checks, test deadlines, parallel execution, real SHA-256 checks and handover/Pause/fresh-observation assertions remain enabled and unchanged. Release settings are unchanged; release already optimizes this dependency. This is not a reason to skip hashes, serialize tests or increase deadlines when a future failure occurs.

CI retains the ordinary suite and additionally runs all Linux update process tests with `--features sha2/force-soft`. This exercises the software path even on runners with hardware SHA instructions. The feature applies only to that Cargo invocation, not to subsequent release builds or bundles. To reproduce safely on a workstation with an active monitor:

```sh
bwrap --bind / / --tmpfs /tmp --dev-bind /dev /dev --proc /proc \
  --unshare-pid --die-with-parent \
  cargo test --locked --test upgrade_process --features sha2/force-soft
```

Keep the same isolation precautions described above. A failure still needs its own [diagnostic evidence](#diagnose-a-failing-process-test); this build setting is not a blanket classification of intermittent failures.

## Diagnose a failing process test

The shared-monitor CLI test and Linux executable-update process tests preserve failure evidence **before cleaning up their fixture processes and directories**. Successful runs create no failure artifact.

By default, evidence goes to `dist/test-failures/<test-name>-<unique-suffix>/`. The failure output prints the path to `report.json`. Set `HERDR_TEST_ARTIFACTS` to choose a different destination outside the fixture directory. CI uploads these directories as `test-failures-<target>` even when the test step fails; missing artifacts do not turn a successful job into a failure.

The report contains:

- The failed wait or assertion context, endpoint/marker, last connection error, and last received status. Rust update waits also record their source call site and whether the status predicate matched.
- Each directly spawned process's command, PID, observed running/exit state and exit code, plus captured stdout/stderr.
- Executable size and SHA-256, original/next update digests, and bounded copies of fixture configuration, monitor logs, registry/publication markers and updater errors.
- On Linux, live `/proc` CPU counters, process state, open files and file positions. These details are explicitly unavailable on macOS/Windows; portable child state, logs and executable identity still apply there.

Rust reports include `context_sequence`, the recording order of each context entry. A saved connection error can predate the latest reply or the current wait; do not mistake an earlier error for the final outcome.

Use the evidence to locate the failing layer, not to guess that every failure needs a larger timeout:

| Observation | Next investigation |
| --- | --- |
| Child exited; stderr explains a startup/configuration error | Fix that startup failure before investigating polling |
| Connection unavailable; no responding owner | Compare endpoint, process state and fixture paths; distinguish a missing listener from a terminated process |
| Child running; query timed out | Inspect logs and Linux CPU/file positions for work still happening before IPC service; a large debug executable is one possible hashing cost, not proof of the cause |
| Reply received but `predicate_satisfied` is false | Compare the actual reply/counters with the condition expected by the failing test |
| Publication/ready marker missing | Compare the saved plan, expected commit/digests, registry state and worker stderr/error files |

The collector makes no extra Herdr/monitor queries, never signals a PID obtained from status, and leaves test deadlines and polling intervals unchanged. Direct child handles are still killed/reaped during cleanup. Artifact-writing errors are reported separately and do not replace the original failure.

Copies exclude binaries, symlink targets and full environment dumps. Text/log copies are capped at 128 files, 512 KiB per file and 8 MiB total; truncated tails and other omissions are listed in the report. Review paths/configuration before sharing an artifact. In-process transport tests print their last query error or unexpected status directly in the assertion output.

This is diagnostics, not an automatic root-cause verdict. A hard kill, process abort or job termination before cleanup runs cannot guarantee a report. Regression tests exercise early child exit, an actual IPC timeout, ordinary assertion failure, cleanup and an unwritable artifact destination.

## Build the source in a bundle

A binary bundle contains `matching-source.tar.gz`, including `vendor/`, `.cargo/config.toml`, the lockfile and matching project source. Extract it, enter its `herdr-idle-inhibitor-source` directory, then build:

```sh
cargo build --offline --release --locked --target-dir target
```

Run Cargo **from that source directory** so it finds the relative vendor configuration; a `--manifest-path` supplied from elsewhere is not equivalent. The Rust toolchain and native linker still have to be installed. Dependencies are vendored, not the compiler.

The extracted source manifest has build steps. If you want to link that source tree, follow [Connect your checkout](#connect-your-checkout-to-herdr); it is distinct from linking the original build-free binary bundle. Herdr's normal source build does not pass `--offline`, even though Cargo can use the included vendor configuration.

## Create and verify a native bundle

Build a native release first, then package it. Linux example:

```sh
cargo build --release --locked --target-dir target
python scripts/package.py --target x86_64-unknown-linux-gnu \
  --binary target/release/herdr-idle-inhibitor --output dist
python scripts/verify_bundle.py --target x86_64-unknown-linux-gnu --output dist
```

**The verifier also requires no running monitor in its namespace.** Its status check must return unavailable without creating application files. On Linux, if your own monitor is active, keep the output directory inside the checkout and run the verifier in the same Bubblewrap isolation shown above, replacing `cargo test --locked` with `python scripts/verify_bundle.py --target x86_64-unknown-linux-gnu --output dist`.

Choose your host's target from the [bundle table](advanced-installation.md#find-the-matching-archive); Windows uses `target/release/herdr-idle-inhibitor.exe`. The packager checks version and executable format/architecture, removes build steps from the bundle manifest, and includes docs, schema, license and matching vendored source. Vendoring may require network access if dependencies are not already cached. The verifier checks archive checksum/layout, runs the executable, and checks matching source offline.

These scripts neither install the plugin nor publish a release. They produce unsigned archives; do not describe a cross-built or type-checked executable as natively tested. A bundle built from a modified checkout must be paired with that checkout's source.

## Source map

| Location | Responsibility |
| --- | --- |
| `src/herdr/` | Herdr registration, discovery, protocol validation and local transport |
| `src/model.rs`, `src/policy.rs` | Observation and deterministic sleep-request policy |
| `src/controller.rs` | Shared monitoring, scheduling, configuration operations and native-request lifecycle |
| `src/backend/` | Idle-only Linux, macOS and Windows mechanisms |
| `src/runtime/` | Paths, singleton ownership, private IPC, configuration and automatic updates |
| `src/ui.rs` | Status/settings popup |
| `src/status.rs`, `schemas/status-v1.json` | Strict public JSON projection and schema |
| `tests/`, `scripts/` | Regression/integration checks and native bundle tooling |

Keep public status queries read-only, uncertain observations distinct from no work, and manual/lid/display behavior outside the inhibitor's scope. For the implemented contracts, read [How it works](behavior.md), [Configuration](configuration.md) and [Status API](status-api.md).
