# Native CI fixture failure and correction

## First native run

- Implementation commit: `dda6a17c0f528ee3bc2e2569fe20336e35dc4201`.
- [GitHub run 37935695739](https://github.com/chpock/herdr-idle-inhibitor/actions/runs/37935695739).
- Linux GNU and Windows MSVC completed successfully, including release/bundle checks. Windows also executed both opt-in native power-request and notification-registration lifecycle tests.
- Both macOS architectures reached the ordinary library tests, reported three failed controller cases, and stalled in `short_suspend_resume_releases_and_requires_fresh_root_and_snapshot`. The parent cancelled the two remaining jobs to retain their logs rather than waiting for the 45-minute job deadline. Cancellation is not a macOS pass; native assertion and packaging steps were not reached.

## Finding: noncanonical fixture identity and an unbounded test wait

**Classification:** validation defect blocking native acceptance, not an established production endpoint-normalization bug.

Production registration correctly normalizes Unix endpoint parents. Controller test fixtures instead constructed expectations and snapshot events from the original `tempfile` parent. A symlinked temporary directory therefore made the fixture's endpoint different from the controller's stored native identity. Explicit snapshots were ignored; the suspend/resume test waited without a deadline for an acquisition that could not be authorized. macOS temporary paths can also exceed its smaller Unix-domain-socket pathname limit.

**Controlled reproduction before correction:** on Linux, `TMPDIR` pointed to a symlink of a private temporary directory. The unchanged handoff case failed with exit 101 at `registered.contains(&b.endpoint)`. The unchanged suspend/resume case exceeded an external 15-second limit (exit 124). Neither experiment created a real power inhibitor or used another user's agents.

## Correction boundary

- `tests/support/tempdir.rs` constructs short, canonical Unix fixture parents under canonical `/tmp`. Windows retains its normal temporary directory behavior.
- Controller unit fixtures and CLI/transport/runtime/integration socket fixtures share that provider; expected identities do not call production normalization to hide mismatches.
- The existing explicit alias-deduplication regression remains intact. `tests/temp_paths.rs` independently checks canonical fixture identity and the macOS 104-byte socket buffer boundary.
- Suspend/resume command waits and the delayed-acquisition start wait have explicit five-second deadlines, so future missing events fail instead of hanging a hosted job.
- No production endpoint acceptance, normalization, polling, retention, native API arguments, power-policy or runtime path changed.

## Local verification

After correction, the entire Linux Rust suite completed under the same aliased `TMPDIR` (48 tests passed, none failed). Formatting, all-target Clippy with warnings denied and all four packaging tests also passed. Exact local reproduction/output files are retained under `/tmp/herdr-native-ci/` during this session; the final hosted-run record will retain portable evidence.

[Fresh independent re-review](native-ci-fixtures.md) closed the correction with no findings. The unchanged four-platform workflow subsequently passed on source commit `54cdf3686948dfa882274ecd601c79022a89eb3d`, including both real macOS assertion probes and all target bundle checks. See [retained native evidence](../validation/native-ci.md). This closes the fixture defect and hosted gate, not physical or installed-Herdr acceptance.
