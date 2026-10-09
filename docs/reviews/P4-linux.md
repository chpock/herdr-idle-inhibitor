# P4 Linux review

## Review

**Merge verdict: OK with notes.**

No issues found.

This verdict covers the Linux implementation and shared native-worker boundary, under the explicitly amended contract. It does **not** certify physical sleep behavior.

### Hypridle / logind

- **Correct:** acquisition requests exactly `idle / block`; no sleep/display inhibitor is substituted (`src/backend/linux.rs:86–97`).
- **Correct:** the owned descriptor is close-on-exec and explicitly dropped during release. The isolated-bus test checks literal arguments, descriptor flags and peer-observed closure (`src/backend/linux.rs:357–411`).
- **Correct:** Hypridle 0.1.8 qualification and the required integration acknowledgment are separate gates (`src/backend/linux.rs:30–58`; `src/backend/mod.rs:82–89`).
- **Correct:** setup documentation preserves existing commands/timers, distinguishes display/lock listeners from suspend listeners, and warns that listener-level ignoring affects all inhibitors (`docs/hypridle-setup.md:7–13,48`).

### GNOME

- **Correct:** acquisition uses only suspend flag `4`, window ID `0`, and the fixed application/reason identifiers (`src/backend/linux.rs:99–101`). The version guard requires GNOME session 51.0 (`src/backend/linux.rs:33`).
- **Correct:** cookie operations target the original unique service owner. Replacement of the well-known owner invalidates the resource instead of sending its cookie to the replacement (`src/backend/linux.rs:82–84,140–144,163–170`).
- **Correct:** tests cover exact framing, normal cleanup, failed cookie release, lost acquisition reply and replacement-owner isolation (`src/backend/linux.rs:413–448,492–592`).

Version matching remains qualification evidence—not proof about distribution patches, settings-daemon/logind combinations, or physical manual/lid behavior.

### KDE / PowerDevil

- **Correct:** acquisition uses `InterruptSession` value `1`, not a screen policy (`src/backend/linux.rs:103–108`).
- **Fixed by parent — P1:** initial empty inhibition properties previously misclassified the real activation delay as loss. The requirement is to retain one pending cookie without reacquisition (`docs/implementation-validation.md:95–99`). PowerDevil inserts the requested entry only after its timer; the corrected state machine distinguishes initial absence from disappearance after confirmation (`src/backend/mod.rs:12–36`). Regression coverage exercises empty pending state, acceptance, suppression, disappearance and unchanged acquisition count (`src/backend/linux.rs:449–490`; `tests/backend.rs:3–28`).
- **Accepted upstream exception:** suppression removes owner tracking while retaining the cookie, permitting reactivation after monitor death. Fresh inspection confirmed this in pinned `powerdevilpolicyagent.cpp:516–526,653–669,733–758`. This is explicitly authorized in implementation-plan invariant 9 and `docs/scope-decisions.md:56–58`, documented in `README.md:29`, and exposed as a conditional limitation identifier (`src/controller.rs:396–401`; `docs/status-api.md:192–194`). No general cleanup waiver or workaround was introduced.

### Shared ownership and recovery

- **Correct:** acquisitions use dedicated connections; uncertain acquisition failures explicitly close them (`src/backend/linux.rs:69–78,125–130`).
- **Correct:** Linux cookie-release failure still closes its sole connection. The worker retains genuinely unresolved ownership until cleanup is confirmed (`src/backend/linux.rs:156–181`; `src/controller.rs:146–185`).
- **Correct:** stale acquisitions are scheduled for release; tests cover delayed acquisition followed by Pause and failed-release ownership retention (`src/controller.rs:793–796,910–920,1250–1400`).

## Validation and residual gaps

- Fresh AFT diagnostics: **zero errors and warnings** across the four scoped Rust files. Auxiliary analysis reported cache/lease limitations; this was not a complete codebase-health certification.
- Supervisor reports passing `cargo test --locked --lib --test backend --test integration --test transport --test ui`: **12+1+2+2+1 tests**. This reviewer could inspect tests but could not execute commands.
- Supervisor reports real-logind ownership counts **1 before owner kill / 0 afterward**, with a child still alive. This result was not independently reproduced; its persisted validation artifact was unavailable at review time.
- Physical idle-suspend, display-off, lock, manual-sleep, lid and resume trials remain unperformed. Isolated desktop services do not establish physical GNOME/KDE correctness.
- Automated coverage does not exhaust executable-version qualification failures, whole-bus reconnection, or real desktop restart/reacquisition behavior.
- Windows/macOS native validation is outside this review; README correctly states the hardware and hosted-CI evidence limits.