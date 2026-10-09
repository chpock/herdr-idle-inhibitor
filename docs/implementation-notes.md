# Implementation record

## Authorization and verification amendment

The owner authorized execution of the full plan and its proposed defaults, without commits or publication. Physical Windows/macOS environments are unavailable. The owner explicitly replaced their physical pre-implementation gates with native build/unit/integration jobs on GitHub runners and honest documentation: tested on Linux, expected to work on Windows/macOS; report failures as issues. Unperformed physical sleep/lock/lid/battery trials are not passes and do not block software implementation. Configuring CI is not evidence that hosted jobs ran; no push is authorized.

Linux desktop reference profiles remain Hypridle 0.1.8, GNOME 51 and PowerDevil 6.7.5. The available workstation is Hyprland. Non-destructive resource tests and isolated fake desktop services are allowed during implementation; changing the workstation's timers, installed plugins, or initiating suspend/lid trials is not authorized. These trials remain explicitly unperformed, not substituted with mocked sleep outcomes.

The existing GPLv3 LICENSE is preserved. Implementation will use one Cargo package, the exact status v1 contract and the idle-only backend arguments. No OS service, elevation, display request, manual-sleep/lid policy, watch API, or automatic updater is added.

## Subsequent commit and native-CI authorization

The owner subsequently requested a git commit and checks on the target platforms. The reviewed implementation was committed and submitted to the existing `origin/main` at `chpock/herdr-idle-inhibitor` to execute the four native GitHub jobs. This supersedes the earlier commit/CI-submission hold for this work only. Actual run results must be retained before closing any native gate. No release/tag/marketplace publication, installed-plugin change or physical power trial is authorized by that request.

## Phase evidence

Phase results, independent findings and deviations are recorded as work proceeds in docs/reviews/ and docs/validation/. The pre-commit working tree was independently reviewed; the owner has now authorized the implementation commit and target-platform CI checks.

## Owner-approved KDE exception

P4 independent review found and the parent freshly verified an upstream PowerDevil 6.7.5 owner-lifecycle defect. The owner selected **accept the KDE limitation**, retaining KDE support while documenting the exact suppress → owner death → reallow orphan scenario. See [evidence and approved disposition](research/kde-suppression-cleanup.md). No hidden cleanup helper is added; other cleanup requirements remain binding.

## Review infrastructure recovery

The original review workflow `1e415419-de57-438b-988f-8c259c18e711` failed because Linux reviewer `a90fdfad-5858-4dc4-a24a-b57901ceb41e` reached its 1,800,000 ms deadline while awaiting parent tests/decisions. Core/runtime reports were saved; Linux final report was not. No reviewer source changes occurred. Repository `/w/projects/herdr-idle-inhibitor`, branch `main`, base `4d70cb5487b512da2fa3c221760a351efe873eae`; tracked partial diff and untracked files were captured under `/tmp/herdr-idle-inhibitor-partial.diff` and `/tmp/herdr-idle-inhibitor-untracked.tar.gz`. Recovery must use the same native subagent protocol and cannot be reported as a passing review until a terminal artifact is obtained.

The recovery workflow `58c9086b-cdf4-4408-8d00-50dc8da1962f` completed the resumed Linux and portable reviews, but integrated reviewer `1694b24c-5ccf-4aa8-aadc-5e26415a7b70` reached its 3,600,000 ms deadline after readiness/fix waits. Six defects were independently parent-verified and corrected; the findings ledger is [P5–P7 findings](reviews/P5-P7-findings.md). Before the same-protocol integrated retry, the ready source was captured in safety checkpoint `final-review-ready-2026-10-09`, `/tmp/herdr-final-review-ready.patch` and `/tmp/herdr-final-review-ready-untracked.tar.gz`. No alternate CLI runner or reviewer source editing was used. Workflow `d0dd59d5-5b19-4a37-873d-783c34fdcf75` then resumed that same reviewer as `75f04548-96f5-461a-8534-9502daf7baf7` and produced [the final terminal report](reviews/P5-P7-final.md): no current source defects, OK with notes for software-source review. Full acceptance is not inferred from this verdict.

## Software verification and delivery record

- Source implementation now spans the P1–P7 seams: model/policy, strict status schema, per-user singleton/IPC, Herdr discovery/eligibility, native backends, popup controls, CLI, test replay, manifest and packaging/CI. No source claim substitutes for a native CI or physical acceptance run.
- Executed local verification and explicit gaps: [end-to-end record](validation/end-to-end.md); fixed toolchain/runner/SDK choices: [platform matrix](validation/platform-matrix.md).
- Independent P1, P2/P3, Linux and portable source reviews passed after parent fixes. The integrated final review has also completed and its terminal artifact was consumed; all six reported findings were freshly closed. Reviewer shell execution was unavailable; test executions in the ledger are parent evidence.
- `.github/workflows/ci.yml` declares four native jobs, native ownership/notification probes, checksums/build-free bundles and offline matching-source verification. Hosted jobs were unrun at the initial implementation boundary. They subsequently executed successfully on all four targets under the owner's commit/CI authorization; [actual native results](validation/native-ci.md) retain the source SHA and scope. Installation and release publication remain separately gated.
- **Current exit: IMPLEMENTATION BLOCKED at remaining acceptance/verification gates, not a reproduced source defect.** The source is implemented and independently reviewed, and its four native hosted jobs passed; installed-Herdr acceptance, broader real-workflow captures and separately authorized Linux power trials remain unperformed. See [acceptance/release checklist](validation/release-checklist.md). No further source or system change is authorized merely by this exit.
- Development archives live outside the repository under `/tmp/herdr-bundles`; these are test artifacts, not signed or published releases. Matching source preserves the GPL license and vendored dependency notices. The Linux executable smoke test runs without Cargo or Herdr environment, with unavailable state remaining unknown rather than zero.

## Executed native CI and fixture correction

Implementation commit `dda6a17c0f528ee3bc2e2569fe20336e35dc4201` was submitted to CI. The first run passed Linux GNU and Windows MSVC (including both real Windows lifecycle probes), but both macOS architectures exposed noncanonical temporary-path assumptions in controller fixtures and an unbounded command wait. The parent cancelled the stalled jobs, reproduced the mismatch on Linux with an aliased TMPDIR, and corrected only test fixtures/wait deadlines. [Fresh independent follow-up](reviews/native-ci-fixtures.md) found no issues; production normalization/native policy was preserved.

Correction commit `54cdf3686948dfa882274ecd601c79022a89eb3d` passed [all four native jobs](https://github.com/chpock/herdr-idle-inhibitor/actions/runs/37940265352): Linux 48 ordinary Rust tests, each macOS architecture 42 plus one real IOKit assertion lifecycle, Windows MSVC 42 plus two real power-request/notification lifecycle tests. All four native bundles passed build-free query/checksum and offline matching-source checks. Actual image/compiler/OS/SDK versions and bundle hashes are [retained](validation/native-ci/37940265352.json), not inferred from runner configuration.

The commit/target-platform-check request is complete. This does not close installed-Herdr, broader actual-trajectory or physical power acceptance and does not authorize a release. Evidence-only documentation commits leave the tested application/fixture/workflow source unchanged.
