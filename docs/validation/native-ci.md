# Native target verification — 2026-10-09

## Passed source revision

- Implementation commit: `dda6a17c0f528ee3bc2e2569fe20336e35dc4201`.
- Reviewed test-fixture correction and **tested source commit**: `54cdf3686948dfa882274ecd601c79022a89eb3d`.
- [Successful GitHub run 37940265352](https://github.com/chpock/herdr-idle-inhibitor/actions/runs/37940265352), workflow **Native checks and bundles**. All four jobs completed with `success`.
- [Retained machine-readable result](native-ci/37940265352.json): exact job/step results, source SHA, runner images, compiler hosts, OS/SDK versions, actual test summaries, native probe names, bundle and artifact digests.
- Follow-up evidence/documentation commits do not change the tested application, fixture or workflow source. This result certifies the source SHA above, not arbitrary later code changes.

## Executed checks

| Native target | Ordinary Rust tests passed | Additional native lifecycle tests | Actual host/image |
| --- | ---: | ---: | --- |
| Linux GNU x86_64 | 48 | 0 hosted power probes; isolated D-Bus interface tests | Ubuntu 24.04 / image `20261004.327.1`, glibc 2.39 |
| macOS arm64 | 42 | 1 real IOKit assertion acquire/release | macOS 15.7.9 / image `20260907.0337.1` |
| macOS x86_64 | 42 | 1 real IOKit assertion acquire/release | macOS 15.7.9 / image `20260824.0482.1` |
| Windows MSVC x86_64 | 42 | 2: real power request acquire/release and notification registration/unregistration | Windows Server 2022, build `10.0.20348.5622` / image `20261004.326.1` |

Each job verified native compiler host identity, formatting, warnings-denied all-target Clippy, ordinary Rust unit/integration tests, all four packaging regressions, release linking, target-specific archive generation and matching-source vendoring. Windows/macOS jobs separately ran the ignored opt-in native lifecycle tests; ordinary ignored tests were not counted as passes.

All jobs used official Rust **1.98.0** (`88d9e12ae178fab0fb5cc050a94da85685d449ea`, LLVM 22.1.8). Both Apple jobs used **Xcode 16.4 / build 16F6**, macOS SDK **15.5**, and deployment target **13.0**; execution occurred on **15.7.9**, not macOS 13 hardware. Windows compiled and linked **MSVC**, not the earlier Linux-hosted GNU cross target.

The Windows named-pipe, SID/ACL and detachment paths were exercised by the ordinary native executable/IPC tests. These do not establish an exhaustive multi-user hostile-client/security matrix.

## Archive verification

All four native jobs emitted this exact verification result:

```json
{"checksum_matches":true,"build_free_manifest":true,"status_without_cargo_or_herdr_context":true,"unavailable_not_zero":true,"created_query_home_entries":0,"vendored_source_offline_check":true}
```

Thus the extracted native executable ran without Cargo or Herdr context, returned unavailable with unknown/null counts rather than inventing zero work, did not create HOME entries, and its matching vendored source passed `cargo check --offline --locked --all-targets` with an empty Cargo home. Windows verified ZIP; Unix targets verified tar.gz. All four downloaded archives were additionally checked against their SHA-256 files by the parent; recorded artifact digests and bundle digests are distinct values.

Artifacts are CI outputs, **not published releases**, and the retained GitHub metadata records their 14-day expiration. No signing/notarization or clean-machine installation is claimed.

## First run and defect closure

[Run 37935695739](https://github.com/chpock/herdr-idle-inhibitor/actions/runs/37935695739) passed Linux and Windows, but both macOS library suites failed fixture-identity assumptions and stalled in the suspend/resume test. The parent cancelled the incomplete jobs; [retained result](native-ci/37935695739.json) uses null counts where no complete passing suite/probe result exists.

A controlled aliased-TMPDIR Linux reproduction demonstrated the same assertion failure and indefinite wait. The correction supplies independently canonical, short Unix fixture parents and bounded failing waits, without changing production normalization or skipping platform tests. See [finding/reproduction](../reviews/native-ci-fixture-findings.md) and [fresh independent re-review](../reviews/native-ci-fixtures.md). The successful corrected run closes this specific validation defect on both actual macOS architectures.

## Verification boundary

The **four-hosted-native-CI gate is passed**. This is stronger than cross-type-checking, but it is not physical sleep-effectiveness or installed-Herdr acceptance:

- No idle-sleep A/B, display-off/locking, user sleep, lid or hardware resume trial was performed on these hosted VMs.
- No Linux GNOME/KDE/Hypridle physical profile matrix was run by CI. Earlier real local logind ownership/owner-death checks remain separate evidence.
- No plugin installation, real popup/update/uninstall lifecycle, Gatekeeper/SmartScreen clean-machine trial or complete recorded real-agent trajectory matrix was performed.
- The owner waived missing physical Windows/macOS access as an initial delivery blocker, not as a passed test. Modern Standby battery and precise PowerDevil suppression/owner-death/reallow exceptions remain unchanged.

The commit/CI request is satisfied. Overall release acceptance retains the unrelated [remaining gates](release-checklist.md).
