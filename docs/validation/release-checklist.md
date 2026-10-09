# Acceptance and release checklist

## Current exit

**IMPLEMENTATION BLOCKED — remaining verification/acceptance gates.** Application source is implemented, Linux software checks passed and five independent source-review seams closed without current defects. This is not the contract's `IMPLEMENTATION COMPLETE` exit: the required checks below have not all run. The review itself grants no execution authority. The owner has subsequently authorized the implementation commit and target-platform checks, including submission to the existing repository for CI. Installation, physical power trials and release publication remain separately gated.

## Evidence already obtained

| Check | Recorded result | Evidence |
| --- | --- | --- |
| Source model/policy, controller, private IPC, popup, public API and five native adapters | Implemented; independent review OK with notes | `../reviews/P1-core.md`, `P2-P3-runtime-herdr.md`, `P4-linux.md`, `P4-windows-macos.md`, `P5-P7-final.md` |
| Linux full Rust suite | 47 passed, no failures | `logs/rust-tests.txt` |
| Formatting, Clippy, Python syntax, workflow static validation | Parent-observed successful exit; empty logs alone are not independent execution proof | `logs/formatting.txt`, `clippy.txt`, `python-compile.txt`, `actionlint.txt` |
| Packaging unit suite | 4 passed, including real ZIP/tar fixtures and six-hook contract | `logs/packaging-tests.txt` |
| Captured Herdr work replay and query isolation | 2 observed working agents; status reads 11 → 112, no snapshot/discovery/native-operation increments from those reads | `logs/replay-counters.txt`, [provenance](../../tests/fixtures/herdr-0.9.3/provenance.json) |
| Actual logind process-death cleanup | Native request count 1 → 0 while inert child lived; no sleep/settings change | [end-to-end record](end-to-end.md#real-logind-ownership--inheritance) |
| Portable cross-type-checks | Windows GNU x86_64 and macOS arm64 all targets checked; no native execution or linking proof | `logs/cross-windows.txt`, `logs/cross-macos.txt`, [matrix](platform-matrix.md) |
| Linux archive and matching source | Checksum, build-free query, unavailable/null exit 3, zero HOME entries, offline vendored source check passed | `logs/package-verification.txt` |

## Remaining mandatory gates

| Gate | Status / dependency | Required closure evidence |
| --- | --- | --- |
| Four hosted native CI jobs | **Pending first run; owner has authorized commit and target-platform CI** | Linux GNU, macOS arm64/x86_64 and Windows MSVC job logs; actual runner/compiler/SDK versions; native unit/API/security results; bundle/offline-source checks. Cross checks do not close this gate. |
| Installed-Herdr acceptance | **Unrun; isolated installation/test-window authorization required** | Source/prebuilt installation, activation with already-working agents, two popup clients, shared persistent Pause, close/reopen/retry/crash, update/uninstall, no-tools bundle and security-warning behavior. Do not replace the user's installed plugins without permission. |
| Broader actual-workflow capture/replay | **Incomplete; controlled real Herdr workflows required** | Recorded working→blocked/done/idle, successive work, overlapping servers, detach, restart and disconnect trajectories with provenance and decision/resource ledgers. Current one-snapshot replay plus synthetic transitions does not substitute for this matrix. Coordinate with the isolated Herdr test window; do not manipulate the user's unrelated agents. |
| Linux profile physical power matrix | **Unrun; appropriate environments and explicit power-trial consent required** | Authorized Hypridle/GNOME/KDE idle-sleep A/B control, display-off/lock, manual/lid, resume, workload progress and native cleanup. Do not shorten timers, remove other inhibitors or initiate suspend automatically. |
| Physical Windows/macOS matrix | **Owner-waived as an initial delivery blocker, not passed** | README retains Linux-only runtime claims and expected portable compatibility. Native hosted jobs remain required. Future hardware evidence may strengthen claims without changing this waiver retrospectively. |
| Final acceptance / publication | **Not yet accepted or authorized for release** | The implementation commit and CI submission are now permitted. Remaining acceptance gates still require evidence; no tag, signing/notarization or marketplace submission occurs automatically. |

The only behavioral exceptions are the accepted Windows Modern Standby battery limitation and precise PowerDevil suppression → monitor death → reallow cleanup defect. They do not waive any unrelated row. Source-review closure is not physical certification.

## Exit discipline

Close individual gates only on actual retained output or a new explicit owner amendment. If native CI reproduces a defect, fix its responsible layer, add a regression and obtain fresh re-review. The owner has authorized the implementation commit and native-CI step; preserve unrelated installation/power/publication holds. Full acceptance can be proposed only after applicable mandatory gates pass; source implementation is already available for review.
