# P0 platform and build matrix

## Verification authority

The owner authorized full implementation without physical Windows/macOS access, substituting native GitHub build/unit/integration jobs and honest verification boundaries. On 2026-10-09 the owner additionally authorized commits and CI submission to the existing repository. All four native jobs subsequently passed against source commit `54cdf3686948dfa882274ecd601c79022a89eb3d`; see [actual native evidence](native-ci.md). Release publication remains unauthorized. Cross-checks/configuration alone are not native execution certificates. Physical sleep/lock/lid counterfactual trials were not performed or claimed; the workstation's settings and installed plugins were preserved.

## Fixed build choices

- Rust **1.98.0**, minimal profile plus rustfmt/clippy; exact dependency resolution in Cargo.lock.
- One Cargo package and executable; existing GPL-3.0 license retained.
- Reference desktop profiles: Hypridle **0.1.8**, GNOME session **51.0**, PowerDevil **6.7.5**. No stronger fallback for another profile.
- CI source: `.github/workflows/ci.yml`, immutable checkout/setup-python/upload-artifact/toolchain action revisions resolved against their official Git references.
- Hosted labels are fixed OS/architecture choices, not immutable VM patch images. Every job records ImageOS/ImageVersion, runner architecture, commit, compiler and OS/SDK evidence; actual tested image versions are retained in [the successful run record](native-ci/37940265352.json).

| Native target | Runner / SDK baseline | Current evidence |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | Ubuntu 24.04, glibc 2.39; isolated dbus-daemon tests | Local logind ownership/owner-death passed; native hosted build, 48 Rust tests and bundle/offline-source verification passed |
| aarch64-apple-darwin | macos-15 arm64; Xcode 16.4 build 16F6 / macOS SDK 15.5; deployment target 13.0 | Native compile/link, 42 ordinary tests plus real assertion lifecycle and bundle/offline-source verification passed on macOS 15.7.9 |
| x86_64-apple-darwin | macos-15-intel; same SDK/deployment baseline | Native compile/link, 42 ordinary tests plus real assertion lifecycle and bundle/offline-source verification passed on macOS 15.7.9; no physical environment |
| x86_64-pc-windows-msvc | windows-2022, Windows SDK/MSVC from recorded runner image | Native MSVC linking, 42 ordinary tests, power-request and notification lifecycle, IPC/detachment and bundle/offline-source verification passed |

The macOS architecture labels and SDK selection were checked against the official [runner catalog](https://github.com/actions/runner-images/blob/main/README.md) and both macOS 15 image READMEs. `macos-15-intel` is an ordinary hosted label, not a paid `-large` selection. CI verifies the compiler host matches the native target and rejects accidental cross execution. Xcode/SDK availability/version mismatch fails explicitly rather than silently changing the baseline.

## Local compiler detail

The workstation compiler identifies as rustc 1.98.0 (88d9e12ae, 2026-08-18), Arch Linux package 1:1.98.0-1.1. Official Windows standard libraries were incompatible with this distributor's compiler metadata despite the matching version string. This was an environment failure, not thousands of application errors. An isolated official rustc 1.98.0 plus matching host/target libraries under `/tmp/herdr-cross-sysroot` then successfully checked all Windows GNU and macOS arm64 targets. No system toolchain was replaced. Cross checking exposed and corrected the Windows HPOWERNOTIFY unregister binding cast; it does not establish MSVC linking or native behavior.

## Fixtures and probes

- `tests/fixtures/herdr-0.9.3/`: minimized live ping/agent-list frames with capture provenance; prompts/titles/paths/identifiers discarded or normalized.
- `examples/native_probe.rs`: explicit native request/assertion lifecycle diagnostic, not an installation hook or sleep-effectiveness test.
- `examples/ownership_probe.rs`: explicit Linux logind owner/child inheritance probe. Local observed ownership counts **1 → 0** after owner SIGKILL while its child remained alive. See [end-to-end evidence](end-to-end.md).
- Native Windows/macOS library tests are opt-in/ignored in ordinary developer runs; CI explicitly runs them with `cargo test --locked --lib -- --ignored`. This tests request/assertion and notification ownership, not physical sleep effectiveness.

## Remaining boundaries

Hosted CI passed by actual execution, not configuration; [the native record](native-ci.md) fixes its scope, compiler/image/SDK versions and source SHA. macOS quarantine/installation, Windows SmartScreen/clean-machine, cross-architecture installation and physical power trials remain unrun. No fake desktop service certifies real GNOME/KDE sleep behavior. The accepted Windows battery limitation and precise accepted KDE suppression/owner-death/reallow cleanup exception remain visible in README and diagnostics.
