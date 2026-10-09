# Integrated findings and parent verification

This is the parent's item-by-item source verification/fix ledger, not an independent final approval. Reviewer run `1694b24c-5ccf-4aa8-aadc-5e26415a7b70` reported the six defects through supervisor dialogue, then timed out before the completed recheck. The same review was resumed as `75f04548-96f5-461a-8534-9502daf7baf7`; its [terminal independent report](P5-P7-final.md) freshly closes all six findings with an OK-with-notes software-source verdict. No native CI or physical acceptance is inferred here.

## 1. Windows bundle verification used the Linux archive format

- **Requirement:** P7 verifies each native bundle, matching checksum and build-free executable before retaining an artifact.
- **Root cause:** `scripts/package.py` produced `.zip` for MSVC, while `scripts/verify_bundle.py` constructed `.tar.gz` unconditionally and always used the tar reader. Windows CI would fail before reaching its executable/source smoke checks.
- **Resolution:** both scripts use the shared target-aware `archive_path` and `checksum_path`; verification dispatches to ZIP or tar extraction.
- **Regression:** `scripts/test_package.py` constructs an actual ZIP with the Windows suffix/checksum path and checks its extraction; Linux tar and all four native manifests remain covered. Four Python tests passed. This is not native Windows execution.

## 2. Unsaved Pause was only a transient local notice

- **Requirement:** plan 7.2 requires prominent warning when Pause applies but persistence fails; restart may restore the saved setting.
- **Root cause:** the control flag existed, but drawing did not inspect it. Reopening the popup or replacing a notice with an unrelated success hid the persistence failure.
- **Resolution:** `src/ui.rs` renders a fixed warning from shared `control.pause_persisted`, independently of the local notice and current view. It also warns in resize-only mode.
- **Regression:** `tests/ui.rs` renders all views with empty and success notices and requires `PAUSE NOT SAVED` at every supported tested geometry. The status fixture passes the strict public validation boundary.

## 3. Two required reconciliation hints were absent

- **Requirement:** plan 4.2 lists agent detection/status and pane close/exit; hooks are hints, never authoritative observations.
- **Root cause:** the manifest omitted `pane.agent_detected` and `pane.closed`; periodic reconciliation still worked, but the planned immediate hints were missing.
- **Resolution:** `herdr-plugin.toml` registers both using the same thin background `_ensure` entrypoint. No event payload is trusted as work evidence.
- **Regression:** packaging tests require the exact six-hook set in the source and each target-native manifest.

## 4. README promised an unqualified Herdr version range

- **Requirement:** plan 4.1 qualifies version behavior rather than treating every later version as compatible.
- **Root cause:** README advertised 0.9.3+, while `src/herdr/protocol.rs::parse_ping` accepts exactly 0.9.3.
- **Resolution:** README identifies the initial exact source-qualified profile and distinguishes the installer minimum from a compatibility promise.
- **Regression:** `tests/protocol.rs` accepts 0.9.3, rejects 0.9.2/0.9.4/1.0.0 and checks the README claim against this boundary.

## 5. Failed live configuration operations lost shared diagnostics

- **Requirement:** plan 7.2 keeps the last valid state and surfaces write/reload failure to every popup, not just the originating response.
- **Root cause:** `ConfigStore::reload/apply` returned errors without retaining them in `ConfigStore.error`; the controller's diagnostic projection therefore lost these failures immediately. Native success cannot be used as configuration success.
- **Resolution:** both methods capture the complete operation result, retain failures, preserve valid settings and clear the stored failure only after successful configuration application/reload. Raw error details remain private; public status uses the existing generic issue.
- **Regression:** `tests/config.rs` checks invalid TOML, external-edit rejection and invalid-value rejection/recovery. A controller test sends rejected operations, reads shared status repeatedly and checks successful reload clears the diagnostic. The newly added store test failed against the old implementation before the correction.

## 6. Small windows accepted invisible actions; Escape did not always close

- **Requirement:** plan 8.1 provides visible controls and popup closure without stopping the monitor.
- **Root cause:** wrapped text could push the selected row beyond a fixed panel, while keyboard dispatch remained active; Escape navigated back from secondary views despite the documented close behavior.
- **Resolution:** at supported sizes, interactive rows use predictable nonwrapping layout and a focus-aware viewport. Below 50 columns/10 rows, rendering asks for resize and all nondismissal inputs are rejected. Escape/q/Ctrl+C close before view-specific navigation; Enter returns from Details.
- **Regression:** `tests/ui.rs` checks each focus at 50×10, 52×12, 64×22 and 80×30 and rejects hidden actions below the minimum. Close-key routing is independent of view. Long diagnostics remain accessible in the wrapped read-only Details view.

## Minor documentation correction

`docs/status-api.md` now identifies the implemented v1 command/schema, without implying native runtime or physical certification.

## Executed evidence and recovery boundary

- `cargo fmt --check`, warning-free Clippy, **47 Rust tests**, **4 Python packaging tests**, Python syntax and all-target official Rust cross-type-checks for Windows GNU/macOS arm64 passed. Exact outputs: `docs/validation/logs/`.
- The final review deadline was 3,600,000 ms, including readiness/fix waits. The timeout was not approval. The resumed review subsequently completed with all finding closures verified; full acceptance remains blocked by the separately listed verification gates.
- Before retry, working tree `main` at base `4d70cb5487b512da2fa3c221760a351efe873eae` was captured in safety checkpoint `final-review-ready-2026-10-09`, `/tmp/herdr-final-review-ready.patch` and `/tmp/herdr-final-review-ready-untracked.tar.gz`. All source remains uncommitted.
- Hosted native jobs, installed Herdr/manual UI lifecycle, clean-machine acceptance and physical sleep matrices remain unperformed and separate from these fixes. The approved portable physical absence and precise KDE exception are not broader waivers.
