# Native CI Fixture Follow-up

## Review

**Scope:** only the uncommitted fixture correction against `dda6a17c0f528ee3bc2e2569fe20336e35dc4201`. Earlier implementation reports are unchanged.

**No issues found.**

- **Correct — independent fixture identity.** `tests/support/tempdir.rs:5-13` creates Unix fixtures beneath canonical `/tmp`, avoiding aliased/long macOS temporary parents without calling application normalization. Windows retains its existing temporary-directory behavior, consistent with production’s exact-string pipe identity (`src/herdr/discovery.rs:142-164`). `tests/temp_paths.rs:5-21` checks canonical Unix identity, normalization agreement, the longest fixture socket pathname against macOS’s 104-byte buffer, and Windows absolute paths.

- **Fixed — fixture mismatch, not production normalization.** Registration normalizes endpoints before indexing them (`src/controller.rs:407-409`). Previously, test events and assertions retained the unnormalized temporary path. The corrected fixtures supply matching identities without weakening normalization or rewriting expectations through production code. Explicit symlink alias-deduplication coverage remains intact (`src/controller.rs:1306-1328`).

- **Fixed — missing events now fail the affected waits.** The three suspend/resume command waits use five-second deadlines followed by failing `expect`/`unwrap` and command-variant assertions (`src/controller.rs:1197-1204,1218-1225,1257-1264`). Delayed-acquisition startup likewise fails on timeout (`src/controller.rs:1518-1521`). These deadlines do not synthesize events or turn absence into success. Existing ownership/generation assertions remain.

- **Correct — test-only boundary and source packaging.** The controller helper module is private and `#[cfg(test)]` (`src/controller.rs:1048-1053`). Other imports are confined to integration-test targets. The inspected diff changes no production policy, native arguments, endpoint normalization or public API. Matching-source packaging recursively includes `tests`, including the helper (`scripts/package.py:115-123`). No platform checks were disabled; all four jobs and native lifecycle probes remain configured (`.github/workflows/ci.yml:18-29,78-90`).

## Evidence and limitations

Freshly inspected parent/hosted logs show:

- The original aliased-TMPDIR handoff assertion failed: `/tmp/herdr-native-ci/alias-handoff-before.log:6-16`. The original resume log stops after starting one test; its external timeout/exit 124 is parent-recorded rather than present in that log.
- The corrected Linux suite reports **48 passing tests**, including all affected controller cases, explicit alias deduplication, replay integration and the new pathname regression: `/tmp/herdr-native-ci/alias-suite-after.log:5-127`. The aliased-TMPDIR setup is documented in `docs/reviews/native-ci-fixture-findings.md:16,28`.
- First hosted run `37935695739` reached successful Linux/Windows bundle verification and artifact upload. Both Windows native lifecycle probes passed (`37935695739-all.log:2212-2215,2973,3010,6164,6202`).
- Both macOS architectures reported the three controller failures and the long-running resume test (`37935695739-all.log:1048-1054,4078-4083`). Cancellation was **not** a macOS pass.
- Reviewer-run scoped diagnostics returned **0 errors and 0 warnings** across seven files. Auxiliary analysis was incomplete because of a cache lease timeout. No shell tests were executed by this reviewer; Clippy success remains parent-reported.

**Merge verdict: OK with notes for this fixture correction.** Corrected hosted execution remains pending, particularly both macOS architectures and their previously unreached native assertion/package checks. Run the unchanged four-platform workflow against the correction before closing native acceptance. This review authorizes no installation, physical power trials, tag or release.