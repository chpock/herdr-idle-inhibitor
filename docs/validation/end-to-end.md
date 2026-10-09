# Implementation verification evidence

This record distinguishes executed software/native-ownership checks from unperformed physical power trials. It is not a sleep-effectiveness certificate. The owner subsequently authorized commits and hosted native checks; implementation is committed, and source revision `54cdf3686948dfa882274ecd601c79022a89eb3d` passed all four native targets. No installed plugin or workstation power setting was changed. See [native-run evidence](native-ci.md) for the later target-platform execution; the initial local records below remain historical evidence.

## Executed Linux checks

- `cargo fmt --all`, `cargo clippy --all-targets --locked -- -D warnings`, and `cargo test --locked` passed. **47 tests** passed on Linux, including 14 library, 2 full controller/local-transport integration, executable singleton/query checks, schema/config/policy and fault/UI tests. No ignored physical/native portable tests were counted as passes.
- `cargo test --locked --test integration -- --nocapture` passed and emitted the counter ledger below. The source capture was a real live Herdr **0.9.3** response with six minimal agent rows, including **2 working**. Provenance: `tests/fixtures/herdr-0.9.3/provenance.json` (captured 2026-10-09T01:17:23Z). Only minimal identity/status fields survive; request/terminal IDs are normalized. Completion/fault transitions are explicitly synthetic, not invented recorded traffic.
- Captured work traversed the native local socket, strict framing/parser, root eligibility/discovery scheduler, controller, policy, owned fake native resource and validated public status. Real native ownership is verified separately below; fake acquisition is not proof of sleep prevention.
- Isolated dbus-daemon tests passed for exact logind `idle/block`, GNOME flag `4`/cookie, KDE pending→accepted/suppressed and disappearance; failed acquisition/release, original-owner replacement and sole-connection closure. The initial KDE pending interval uses **empty requested/active properties**, matching pinned PowerDevil, not an immediate fake row.
- Deterministic controller tests cover execution-gap invalidation before queued I/O, same-root custom endpoint handoff, normalized aliases, delayed acquisition followed by Pause, failed-release retained ownership, and short suspend/resume requiring fresh root and agent epochs. Policy tests cover nonrenewing retention and mixed-server/same-server completion regressions from independent review.

### Observable counters, not timing claims

From the final persisted integration run ([raw counter output](logs/replay-counters.txt); [full Rust suite](logs/rust-tests.txt)):

| Experiment | Before | After | Meaning |
| --- | --- | --- | --- |
| Public status reads | 11 | 112 | 100 repeated reads plus the final measurement; delta **101** |
| Snapshot valid / hook / initial / poll / resume during reads | 2 / 1 / 1 / 0 / 0 | 2 / 1 / 1 / 0 / 0 | No Herdr snapshots recomputed in this read-only experiment |
| Root discoveries / hints during reads | 1 / 1 | 1 / 1 | Reads did not register/schedule discovery |
| Native acquire / release during reads | 1 / 0 | 1 / 0 | Reads did not change native ownership |
| Captured working agents | — | 2 | Exactly the captured `working` rows, not all six rows |
| Captured replay valid snapshots / hook snapshots | 1 / 1 | 2 / 2 | One initial captured response plus one explicitly triggered synthetic completion |
| Captured replay native acquire / release | 1 / 0 | 1 / 1 | One owned fake request, released after confirmed completion |

The query projection can compute ages without I/O. Autonomous polling remains allowed; the table records the actual run, not a promise that timers stop during arbitrary external queries. A separate deterministic query test checks no generations, scheduled reads or native operations are mutated even after a long execution gap.

## Real logind ownership / inheritance

Executed `examples/ownership_probe.rs` against the workstation's actual logind, followed by a driver querying `ListInhibitors` for the created owner's PID:

```json
{"native_requests_before_owner_kill":1,"native_requests_after_owner_kill":0,"discovery_like_child_still_alive":true,"sleep_or_power_settings_changed":false}
```

The probe acquired only an `idle/block` descriptor and spawned another copy in inert `--child` mode with null standard streams. SIGKILL terminated only the created owner; the child remained alive when the request count reached zero. The driver then terminated its own test child. This establishes native owner-death cleanup and lack of resource inheritance into that child, **not** display/lock/lid/idle-suspend effectiveness. Other installed inhibitors were untouched and cannot substitute for a sleep counterfactual.

## Portable source/build checks

Official matching Rust 1.98.0 compiler/target libraries successfully ran `cargo check --locked --all-targets` for **Windows GNU x86_64** and **macOS arm64** on Linux. This caught a Windows notification-handle type error and includes the new registration-failure tests. It does not execute Windows tests, link MSVC, link Apple frameworks, or certify both macOS architectures. Subsequent actual native GitHub jobs passed Linux GNU, both macOS architectures and Windows MSVC, including separately executed opt-in assertion/power-request/notification lifecycle tests and all target archive/offline-source checks. Those [actual results](native-ci.md), not the earlier cross-checks, establish native linking/API execution. See [platform matrix](platform-matrix.md).

Independent review also found silent Windows notification-registration failure. It now fails startup before a native worker can acquire; a checked null/error registration boundary and millisecond suspend/resume regression are present. The rejected stale-round test requires **2 acquisitions / 1 release**, with neither an old root nor an old snapshot authorizing the second acquisition.

## Packaging and UI boundary

- Release Linux build and development-only native packager completed successfully. The generated archive has a build-free manifest, native target/header check, checksum, executable, documentation, and matching vendored source including third-party license files.
- All **4** pure packaging tests passed for all four declared target manifests, the complete six-hook set, tar/ZIP archive and checksum selection/extraction, and rejection of unexpected entrypoints. A regression parses Cargo's escaped Windows paths before producing the relative offline vendor configuration.
- The extracted Linux binary ran `status --json` with **no Cargo/PATH or HERDR context**, returned exit **3** / unavailable with unknown/null counts, and created **0** HOME entries. Archive checksum and build-free Linux-only manifest matched. Matching source passed `cargo check --offline --locked --all-targets` with an empty CARGO_HOME from its extracted root, proving complete relative vendor configuration without registry access. The first driver invocation used `--manifest-path` from the wrong working directory, where Cargo does not discover that root's configuration; the corrected root-cwd invocation passed. This is an executable/source smoke check, not installed Herdr certification.
- Popup render/key-routing tests passed at small/normal/large dimensions with control-character sanitization. At every supported 50×10-or-larger tested geometry, focused Status/Settings actions remain visible. Smaller windows accept only dismissal. The shared unsaved-Pause warning survives reopening, switching views and later success notices; Escape closes from every view. Real installed Herdr popup invocation, theme-dependent visual acceptance and close/restart/update/uninstall workflows remain unperformed; the plugin was not installed into the workstation.

## Final integrated review corrections

The parent independently checked the six reported defects and corrected them without widening scope. See [item-by-item findings and regression ledger](../reviews/P5-P7-findings.md). Exact final outputs are retained in `docs/validation/logs/`: format, Clippy, full Rust suite, replay counters, packaging tests, Python syntax and Windows/macOS cross-type-checks. Rejected reload/apply failures now survive in shared status without discarding valid settings, and clear only after a successful configuration operation. Herdr support is accurately limited to the source-qualified 0.9.3 profile; hook events now include `pane.agent_detected` and `pane.closed`. The public API document now describes the implemented command rather than a planning-only proposal.

## Independent review and accepted exceptions

Reports: `docs/reviews/P1-core.md`, `P2-P3-runtime-herdr.md`, `P4-linux.md`, `P4-windows-macos.md`, and [integrated final review](../reviews/P5-P7-final.md). All five source-review seams are closed with no current findings under the amended contract; the final verdict is OK with notes for source review only. Each real finding was parent-verified, fixed and freshly re-read. Reviewers could inspect tests but could not execute shell tests; executed commands above are parent evidence.

- Windows Modern Standby battery exception: ordinary native request only; no workaround or hardware power-profile logic.
- PowerDevil 6.7.5 suppression→owner death→reallow orphan: independently source-verified and explicitly owner-accepted. No general cleanup waiver. See [finding](../research/kde-suppression-cleanup.md).

## Unperformed checks / release gates

Hosted native compile/link, ordinary executable/IPC tests, real Windows/macOS API lifecycle tests and bundle checks have now passed. Installed-Herdr lifecycle and clean-machine install/security-warning tests remain unrun. Physical idle-sleep A/B, display-off, locking, manual sleep, lid and resume matrices have not been performed on Linux/GNOME/KDE or portable hardware. Missing portable physical access was explicitly accepted; unperformed checks are not passes. Linux ownership/API tests and hosted portable lifecycle checks do not certify physical sleep prevention. Commits and CI submission were authorized and completed; tags, release uploads, installation, physical power trials and marketplace publication remain separately gated. The broader recorded actual-workflow matrix is also incomplete: only one live work snapshot was captured and replayed; completion and fault changes are synthetic. The commit/target-platform-check request is complete; overall acceptance remains **IMPLEMENTATION BLOCKED at unrelated remaining verification gates**, detailed in the [release/acceptance checklist](release-checklist.md), not IMPLEMENTATION COMPLETE.
