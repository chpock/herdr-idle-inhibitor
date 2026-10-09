# Architecture grill: Herdr idle inhibitor

Status: historical planning grill, completed before implementation. The former Windows lock-screen blocker was withdrawn through documentation/source verification, not a native test. The owner subsequently authorized execution with the physical Windows/macOS amendment and precise KDE exception recorded in [implementation-notes.md](implementation-notes.md). Current software/runtime evidence is under `docs/validation/`; unperformed physical trials remain unperformed. No commits/publication are authorized. Product artifacts remain English; discussion remains Russian.

## Request and map

Produce an evidence-grounded, complete implementation plan for the agreed [baseline](baseline-plan.md), without expanding it into general power management.

- Rust; Linux (Hyprland/Hypridle, GNOME, KDE), macOS, Windows.
- All local Herdr servers of the current OS user; only reported `working` counts.
- Prevent idle-triggered host sleep; preserve display-off, locking, lid policy, and manual sleep. Never request suspend after work.
- On-demand Herdr window; shared settings and Pause independent of window lifetime.
- External interface: one-shot read-only `status --json`; no watch, outbound commands, network server, or lid integration.
- Documented user-applied desktop integration is allowed; automatic power-configuration changes are not.

## Unknowns taxonomy (initial map)

| Category | Initial entries | Treatment |
| --- | --- | --- |
| Known knowns | The product boundaries above; Herdr 0.9.3 schemas/startup-hook constraints; the installed Hypridle display-inhibition issue. | Re-read source and preserve explicit user decisions. |
| Known unknowns | Native idle-only behavior in every target; monitor topology/lifetime; discovery; observation/error semantics; status/UI/config contracts; packaging and real acceptance. | Evidence first, then an explicit decision or bounded default. |
| Unknown knowns | What Pause means after a restart, how a degraded/limited state should look, acceptable low-level operational defaults. | Show concrete states/flows; label conservative defaults for veto rather than another broad questionnaire. |
| Suspected unknown unknowns | Desktop inhibitors affecting manual sleep/display; Windows Modern Standby limits; activation gaps after enabling a plugin; mixed local/remote agent lists; server handoff and event ordering; inherited inhibitor handles; disabled/uninstalled plugins leaving monitors alive. | Fresh source/docs blindspot pass with specific verification gates. |

## Material unknowns ledger

Initial count: 12 architecture topics, not 12 questions to the user. All now have source-backed designs, an explicit user decision, or a visible proposed default. Remaining blocking planning questions: **0**. U01 was rechecked against the selected Win32 API and pinned PowerToys source: no lock-only restriction was established, so the earlier special blocker was withdrawn. This completes the planning grill, not runtime support certification; P0 and later native acceptance still have to run.

| ID | Topic | Risk | Status / resolution |
| --- | --- | --- | --- |
| U01 | Windows idle-only contract, power models, resume/revocation | High | Resolved for API selection: ordinary SystemRequired, accepted battery/model caveat, explicit sleep/resume handling. Direct documentation names no lock-only cancellation; checked Awake code uses another API and retains its system requirement when locked. Former special blocker withdrawn; native lock testing remains mandatory acceptance, not already-passed evidence. W1–W2. |
| U02 | GNOME/KDE/Hypridle separation of idle sleep, screen idle, and explicit sleep | High | Source-qualified backend designs: logind idle plus Hypridle integration; GNOME suspend-only cookie on verified weak-forwarding versions; KDE InterruptSession with pending/suppressed states. Reference-version native experiments gate implementation progression. |
| U03 | macOS resource lifetime, capability, explicit sleep, and platform bindings | Medium | Resolved from public assertion docs and Apple's owning-process-exit cleanup. Use PreventUserIdleSystemSleep; verify cleanup/resume natively during implementation. |
| U04 | Herdr local-server discovery, custom endpoints, remote exclusion | High | Public session listing plus hook registration of local endpoints/roots; no process scan, remote transport, or assumption that an inaccessible custom server is empty. Coverage is an explicit status fact. |
| U05 | Activation, enable/disable/uninstall, crash recovery, and ownership scope | High | Default selected: one per-user monitor; short idempotent bootstrap on startup/hooks/window open, registry checks, owned-resource cleanup, explicit initial activation and update procedure. No OS service/watchdog; after a crash recovery requires the next bootstrap trigger. |
| U06 | Herdr wire types, subscription/snapshot ordering, reconnect/handoff | High | Fresh schemas/transport/tests inspected. Choose manifest hooks as refresh hints plus periodic authoritative snapshots, not dynamic per-pane streams. Serialize each server's reads and discard old connection/request generations. |
| U07 | Unknown/failed observations, aggregate confidence, release grace | High | Defaulted in plan sections 5.1–5.4: fresh positive evidence, three-valued work, five-second normal release, nonrenewing thirty-second retention of existing ownership only, no resurrection after a known nonworking state. |
| U08 | Shared controls, config paths/persistence, Pause and restart semantics | Medium | Defaulted in sections 3/7: one state/config writer, per-user application paths, Pause persists until Resume, no duplicate Enabled toggle, explicit reload and visible persistence failures. |
| U09 | Read-only status contract, freshness, errors, privacy/security | Medium | Fully proposed in status-api.md: exact fields/nullability/exit codes, unknown rather than false empty state, no bootstrap or fresh Herdr/native I/O, current-user IPC permissions, no sensitive raw agent data. |
| U10 | On-demand window behavior, keyboard flow, degraded-state presentation | Medium | Defaulted in section 8: Herdr popup, Ratatui/Crossterm, separate work/observation/request/control rows, shared commands, keyboard flows, unavailable/setup/pending/suppressed states and explicit Retry. |
| U11 | Rust dependencies, targets, manifest/install/update/release integration | Medium | Proposed in sections 9–10: one package/binary, target-gated native bindings, native plugin bundles and source builds, four initial CPU targets, explicit activation/update and P0 pinning of reproducible OS/toolchain/dependency versions. Publication/signing require owner authorization. |
| U12 | Real traffic, OS checks, race/fault tests, review and acceptance gates | High | Specified in implementation-validation.md: ordered P0–P7, real-wire replay plus native A/B experiments, operation counters, per-backend/final independent reviews and exact stop rules. Missing hardware is blocked, never passed. |

## Territory inspected

- Fresh reads: `docs/baseline-plan.md`, `docs/scope-decisions.md`, `docs/research/plugin-survey.md`.
- Herdr `0.9.3`, commit `7b116c05bfda646af39d2524c54e70c751f57ee8`: lifecycle, session discovery, agent/event schemas, native transport, and subscription tests were freshly read.
- Skill instructions, grill-session template, and domain-modeling add-on were read before this review.
- `aft_inspect` found no application code; it cannot authoritatively type-check this documentation-only repository. Native/platform verification has not happened.
- Fresh stable GNOME 51 / PowerDevil 6.7.5 source, installed-version Hypridle code, Apple docs/process cleanup, Microsoft power-request documentation, and library docs are recorded in [architecture evidence](research/architecture-evidence.md).

## Domain ledger

Canonical definitions are maintained in the root [CONTEXT.md](../CONTEXT.md). In particular, observed work, desired inhibition, resource ownership, and actual OS sleep outcome are distinct facts. A Herdr server session is not an agent conversation or a workspace.

## Design branches and decisions

| Branch | Alternatives considered | Proposed resolution / evidence |
| --- | --- | --- |
| Ownership | Per-server workers versus shared monitor | Shared owner matches global Pause/status without a coordination network; bootstrap races require one real OS lock. H1–H2; plan section 3. |
| Observation | Dynamic per-pane socket streams versus manifest hints + snapshots | Hints + two-second snapshots avoid wildcard/ordering assumptions and retain event responsiveness. H3; sections 4–5. |
| Activity truth | Direct events/process heuristics versus Herdr snapshots | Literal snapshot `working`, deduplicated by server/terminal; no inference about unreported detached/remote work. H3–H4. |
| Error policy | Immediate fail-open, indefinite hold, or bounded retention | Retain an existing resource for at most thirty seconds of lost positive evidence; no unknown-only acquisition. Explicitly defaulted, not claimed to be an OS fact. |
| Controls | Separate persistent Enabled plus temporary Pause versus one shared Pause | Persist one Pause until Resume; installation enable/disable remains Herdr's responsibility. Simpler state and restart behavior. |
| Linux backend | Generic strong inhibitor versus desktop-specific contract | Hypridle listener integration, qualified GNOME weak-forwarding path, KDE InterruptSession with real activation/suppression states. L1–L4. |
| Windows limit | Workaround, strict block, or documented exception | User accepted the documented Modern Standby battery exception and rejected complexity. No runtime power-model state machine. W1/scope record. |
| Recovery | Installed supervisor/watchdog versus hook-triggered restart | No extra supervisor; crash release is guaranteed by resource ownership but automatic restart has no deadline without a new trigger. Explicit default for veto. |
| Storage/API | File-based activity flags/database/watch service versus live query | Only configuration/logs persist. A live local status query has no policy side effects; unavailable is unknown. |

## Blocking-question queue

One material product question was asked: Windows Modern Standby on battery. After asking what that term meant, the user explicitly chose to document the limitation and avoid complicated handling. No further product change currently requires a blocking question. Operational defaults will be visible for veto in the plan.

Budget: at most five blocking questions without agreeing a larger interview. Technical facts are not user questions. Low-risk defaults are presented together for veto.

## Resolved assumptions

Defaults for approval/veto are collected in the implementation plan, not presented as additional user decisions:

- One native-host monitor per current OS user; one active reference desktop login context, with all relevant local Herdr servers.
- Two-second snapshots, five-second discovery/eligibility checks, six-second snapshot freshness, ten-second discovery/eligibility freshness, five-second configurable normal release, thirty-second nonrenewing observation/eligibility retention.
- Pause persists until explicit Resume; no second equivalent application Enabled toggle; observations continue while paused.
- Per-user application configuration independent of workspaces/alternate Herdr roots, one writer, explicit manual reload, bounded diagnostics only.
- Manifest hints instead of dynamic per-pane streams; no extra watchdog/installed service and no zero-gap update guarantee.
- Initial native artifacts: Linux x86_64, macOS x86_64/aarch64, Windows x86_64. Desktop reference versions are source-qualified candidates until P0 confirms runtime behavior.
- Ratatui/Crossterm popup and the concrete JSON v1 projection. No extra public write/watch/callback interface.

Exact release compiler/dependency/SDK versions are execution artifacts to pin at P0, not a material product fork. Physical hardware and native tests remain unavailable as evidence in this planning-only session. A failed P0 blocks dependent work, rather than inheriting approval from these assumptions.

## ADR candidates

No ADR files were created for routine library/default choices. The surprising Windows trade-off already has a user-approved, reasoned record in scope-decisions.md; duplicating it is unnecessary. When the public JSON contract is approved for publication, consider a short ADR for its versioning/read-only compatibility commitment (hard to reverse for consumers). Until then, status-api.md is explicitly a proposal, not a published compatibility promise.

## Completion rules

- Grill complete only when every material topic is resolved, explicitly defaulted, or accepted; unresolved feasibility is a blocker, not hidden technical debt.
- A full implementation plan must specify behavior, interfaces, ownership, platform mechanisms, ordered test-first steps, observables, real acceptance, and deviation rules.
- Runtime evidence unavailable during planning is an explicit P0 feasibility gate before dependent implementation and a later integrated acceptance gate; it is not fabricated as a completed experiment.
- Starting implementation requires separate approval. This session must end with a precise completed-plan or blocker status.

## Blindspot pass and cheapest decisive checks

| Risk | Why it matters / evidence | Cheapest decisive next check | Decision owner |
| --- | --- | --- | --- |
| Medium: overgeneralizing product help into an API restriction | Awake help is not the PowerSetRequest contract; checked source retains its system requirement when locked. Direct docs support the normal request, subject to OS policy. W2. | Ordinary P0/integrated SystemRequired acceptance with a separate workload, display-off and Win+L. Do not infer success from a handle or claim this test already ran. | API/source review resolves the planning issue; native tests validate the implementation. |
| High: desktop names hide stronger semantics | GNOME flag 4 forwards differently across versions; Hypridle idle blocks ordinary display listeners. L1–L3. | Version-qualified native idle/display/manual/lid trial; explicit Hypridle integration. | Source plus native experiment; no fallback that expands scope. |
| High: hook lifecycle is not supervision | Enable does not run startup, registry disable does not kill detached processes, and crash does not schedule restart. H1. | Minimal native bootstrap/enable/disable/parent-exit test before daemon/UI work. | Proposed architecture/default, then P0/P2/P3. |
| High: stale state or a late acquisition outlives intent | Errors, handoff and delayed native replies can otherwise create permanent false work or a lock after Pause. H3/native ownership contracts. | Fake-clock reducer and generation tests; lost-reply D-Bus cleanup test; owner-kill with a living child. | Implementation tests and independent review. |
| Medium: KDE cookie is not activation | Five-second timer and user suppression make an immediate Active label false. L4. | Observe Requested/Active properties, suppress/allow in the desktop, confirm no reacquisition storm. | Native experiment. |
| Medium: update leaves an old Windows executable running | Replacing an executing binary can fail; unlink does not supervise the old child. H1/native Windows lifecycle. | Install/update/uninstall with actual running monitor/popup and documented stop-first maintenance. | Packaging test; no automatic updater added. |
| Medium: inactive/headless/custom Herdr scope differs from assumptions | Public enumeration is not an exhaustive process/remote-job inventory. H2–H4. | Real Herdr headless/inactive-workspace/multiple-root/custom-endpoint traces and explicit coverage assertions. | Source/live Herdr evidence; stop rather than add heuristics. |

No further blocking product question is justified by a merely theoretical case. The source-backed choices, visible defaults, and mandatory falsification experiments make the next work bounded without pretending the experiments have already happened.

## Planning exit record

- **PLAN DOCUMENTS PREPARED:** [architecture and ownership](implementation-plan.md), [public JSON contract](status-api.md), and [test-first P0–P7 sequence](implementation-validation.md), including concrete defaults, native call boundaries, operational counters, packaging and review/stop gates.
- **PLANNING GRILL COMPLETE:** U01's special lock-screen blocker was withdrawn after the requested documentation/source check (W2). This is a correction of an unsupported planning classification, not a report that native behavior was tested. The agreed battery/model exception is unchanged, and ordinary P0/native acceptance remains required.
- **NO ADDITIONAL PRODUCT QUESTION:** asking the user to guess an API's real behavior would not resolve it. Operational defaults are proposed together for approval/veto; no new platform exception was assumed.
- **DOCUMENT REVIEW:** [self-review findings and corrections](reviews/plan-self-review.md) distinguish specification fixes, mechanical checks and unperformed native/independent reviews.
- **NOT EXECUTED:** application implementation, native power trials, real replay, installation changes, commits and publication. No installed plugin or desktop power configuration was changed for this plan.
