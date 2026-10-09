# P0 platform and build matrix

## Verification authority

The owner authorized full implementation without physical Windows/macOS access, substituting native GitHub build/unit/integration jobs and honest Linux-only runtime claims. At the implementation commit boundary hosted jobs have not yet run. The owner has now requested the commit and native target-platform checks, authorizing CI submission to the existing repository; actual results will be recorded separately. Release publication remains unauthorized. Cross-checks and configured jobs are not native execution certificates. Linux physical sleep/lock/lid counterfactual trials were not performed or claimed; the workstation's settings and installed plugins were preserved.

## Fixed build choices

- Rust **1.98.0**, minimal profile plus rustfmt/clippy; exact dependency resolution in Cargo.lock.
- One Cargo package and executable; existing GPL-3.0 license retained.
- Reference desktop profiles: Hypridle **0.1.8**, GNOME session **51.0**, PowerDevil **6.7.5**. No stronger fallback for another profile.
- CI source: `.github/workflows/ci.yml`, immutable checkout/setup-python/upload-artifact/toolchain action revisions resolved against their official Git references.
- Hosted labels are fixed OS/architecture choices, not immutable VM patch images. Every job records ImageOS/ImageVersion, runner architecture, commit, compiler and OS/SDK evidence; actual image versions remain pending the first run.

| Native target | Runner / SDK baseline | Current evidence |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | Ubuntu 24.04, glibc 2.39; isolated dbus-daemon tests | Local Linux Rust checks/tests and real logind ownership passed; hosted Ubuntu build not run |
| aarch64-apple-darwin | macos-15 arm64; Xcode 16.4 build 16F6 / macOS SDK 15.5; deployment target 13.0 | All-target Linux-hosted official Rust cross-type-check passed; native compile/link/API probes pending CI |
| x86_64-apple-darwin | macos-15-intel; same SDK/deployment baseline | Native compile/link/API probes pending CI; no physical environment |
| x86_64-pc-windows-msvc | windows-2022, Windows SDK/MSVC from recorded runner image | GNU Windows all-target cross-type-check passed; MSVC linking/native security/API tests pending CI |

The macOS architecture labels and SDK selection were checked against the official [runner catalog](https://github.com/actions/runner-images/blob/main/README.md) and both macOS 15 image READMEs. `macos-15-intel` is an ordinary hosted label, not a paid `-large` selection. CI verifies the compiler host matches the native target and rejects accidental cross execution. Xcode/SDK availability/version mismatch fails explicitly rather than silently changing the baseline.

## Local compiler detail

The workstation compiler identifies as rustc 1.98.0 (88d9e12ae, 2026-08-18), Arch Linux package 1:1.98.0-1.1. Official Windows standard libraries were incompatible with this distributor's compiler metadata despite the matching version string. This was an environment failure, not thousands of application errors. An isolated official rustc 1.98.0 plus matching host/target libraries under `/tmp/herdr-cross-sysroot` then successfully checked all Windows GNU and macOS arm64 targets. No system toolchain was replaced. Cross checking exposed and corrected the Windows HPOWERNOTIFY unregister binding cast; it does not establish MSVC linking or native behavior.

## Fixtures and probes

- `tests/fixtures/herdr-0.9.3/`: minimized live ping/agent-list frames with capture provenance; prompts/titles/paths/identifiers discarded or normalized.
- `examples/native_probe.rs`: explicit native request/assertion lifecycle diagnostic, not an installation hook or sleep-effectiveness test.
- `examples/ownership_probe.rs`: explicit Linux logind owner/child inheritance probe. Local observed ownership counts **1 → 0** after owner SIGKILL while its child remained alive. See [end-to-end evidence](end-to-end.md).
- Native Windows/macOS library tests are opt-in/ignored in ordinary developer runs; CI explicitly runs them with `cargo test --locked --lib -- --ignored`. This tests request/assertion and notification ownership, not physical sleep effectiveness.

## Remaining boundaries

No hosted CI job, macOS quarantine/installation trial, Windows SmartScreen/clean-machine trial, cross-architecture installer trial, or physical Windows/macOS sleep matrix has passed by being configured. No fake desktop service certifies real GNOME/KDE power behavior. The accepted Windows battery limitation and precise accepted KDE suppression/owner-death/reallow cleanup exception remain visible in README and diagnostics.
