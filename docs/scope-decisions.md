# Scope decisions and future composition

Status: product-scope record, not an implementation plan or a commitment to build another application.

## Idle inhibition, not general sleep management

After reviewing lid/manual-sleep feasibility, the user removed both features from this project. This supersedes the earlier proposal for optional lid protection and an explicitly configured combined lid/manual fallback.

The current product prevents idle-triggered host sleep while Herdr agents work. It allows display-off and screen locking, does not handle the lid, does not override manual sleep, and does not issue suspend commands when work ends.

### Reason for excluding lid handling

Lid behavior is not just another inhibitor flag. For example:

1. The user closes the lid while agents are working.
2. A lid controller keeps the machine running.
3. The agents finish while the lid is still closed.
4. The controller must decide whether the machine should now sleep, even though no new lid-close event occurred.

Correct handling can require lid-state observation, a separate sleep-decision policy, privileged or persistent power-setting changes, and restoration after failures. Those responsibilities would turn this project into a broader power-management application. They are not a hidden second phase of the idle inhibitor.

The [feasibility research](research/lid-sleep-feasibility.md) remains useful historical evidence. It is not authorization to implement any lid mechanism. The documented Windows Modern Standby restriction on idle power requests remains relevant to the active project and must not be discarded with the lid proposal.

## Read-only interface for a separate application

The user requested a small external API and selected a one-shot, read-only status command on the same Rust executable:

```sh
herdr-idle-inhibitor status --json
```

This is an approved planning direction, not existing functionality. External consumers poll when they need updates. The proposed output schema and failure semantics are specified in [status-api.md](status-api.md). No specific external consumer is included in this project.

Important boundary: work state, observation health, pause state, and actual idle-inhibitor ownership are different facts. An external controller must not interpret a paused inhibitor, failed acquisition, missing monitor, or failed query as proof that all agents finished. Status must describe current observation rather than a remembered transition.

The user selected query-only rather than the proposed query-plus-stream interface. There is no `watch` command and no outbound command execution or callback delivery contract. These features must not be introduced under the label of future extensibility.

Any future lid controller owns its own state, privileges, policy, and cleanup. This project does not install, launch, restart, or guarantee the correctness of that controller. No HTTP server, general plugin framework, persistent event queue, or privileged helper is justified merely by this possible future consumer.

## Additional confirmed boundaries

The user confirmed shared settings and Pause for all local Herdr servers of the current OS user. Hyprland/Hypridle, GNOME, and KDE are all required Linux environments, alongside macOS and Windows. Documented user-applied desktop configuration is acceptable; the plugin must not silently change it.

These decisions are incorporated in the [baseline plan](baseline-plan.md#confirmed-product-decisions). The user has authorized the full `grill-for-unknowns` review and implementation planning. Implementation and git commits remain unapproved.

## Accepted Windows limitation

During the grill, the user accepted a documentation-only limitation: on Windows computers using Modern Standby while powered by the battery, the application may not prevent idle sleep. Modern Standby is a hardware-supported Windows sleep model, not an application setting or the Windows Battery Saver toggle.

Microsoft documents that ordinary system/execution power requests in this combination terminate five minutes after the system sleep timeout expires. See [the architecture evidence](research/architecture-evidence.md#w1-windows-has-a-documented-batterypower-model-limit).

Use the normal native idle-sleep request. Do not add a bypass, synthetic activity, Away Mode, display-awake request, power-plan rewrite, special helper, or a runtime power-model state machine for this case. State the limitation clearly in compatibility documentation and Windows help. Do not pretend that owning a native request guarantees the OS will honor it indefinitely.

The exception is specific to Modern Standby on battery. It does not silently waive display-off, locking, manual/lid sleep, cleanup, or runtime verification for other Windows scenarios or platforms.

## Accepted KDE upstream cleanup limitation

During P4 the owner explicitly chose to retain KDE support and document PowerDevil 6.7.5's source-verified defect: suppressing a previously active request discards its owner association but retains the cookie; if the monitor then dies, later allowance can reactivate that orphan request. This is a narrow exception to guaranteed process-exit cleanup, not a change to idle-only behavior, a general cleanup waiver, or permission for a helper/watchdog. See [the pinned-source finding](research/kde-suppression-cleanup.md). Physical KDE reproduction remains unperformed.
