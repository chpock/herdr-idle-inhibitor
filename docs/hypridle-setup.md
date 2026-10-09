# Hyprland / Hypridle setup

[Home](../README.md) · [Configuration](configuration.md) · [Compatibility](compatibility.md)

The supported version is **Hypridle 0.1.8**. The application uses logind `Inhibit("idle", ..., "block")`, not a strong `sleep` lock. Hypridle must honor that request for idle suspend while deliberately allowing its display/lock listeners to ignore inhibitors. The application never rewrites this file or restarts desktop services.

## Configuration steps

1. Run `hypridle --version` and inspect your existing Hypridle configuration.
2. In the existing `general` block, ensure `ignore_systemd_inhibit = false`. Do not change the existing lock, suspend, resume, or other commands.
3. Identify **every** listener that dims the screen, powers the display off, or locks the session. Add `ignore_inhibit = true` to these listeners only.
4. For idle-suspend/hibernate listeners, leave `ignore_inhibit` false (its default) or set it explicitly to false. Do not add a suspend command where none exists.
5. Keep each existing timeout, command and resume action unchanged. If one listener mixes display-off/locking with suspend, separate its policy intentionally before confirming; do not indiscriminately allow the mixed action to ignore inhibition.
6. Apply the configuration using your own normal Hypridle/service workflow. This plugin does not perform reloads or restarts for you.
7. Set **Hypridle integration confirmed** in Settings, or set `hypridle_integration_confirmed = true` in the configuration's `[linux]` section and load it at monitor startup as described in [Configuration](configuration.md#editing-and-persistence). Confirm only after the listener changes: this is an acknowledgment, not automatic detection or proof of correct configuration. Until confirmed the plugin refuses native acquisition with `setup_required`.

### Illustrative existing-listener edits

These blocks demonstrate the distinction; they are **not a replacement configuration**. Preserve your own timings, commands and service management. Do not copy new suspend/lock listeners over an existing setup.

```ini
general {
    # Preserve all other existing general settings.
    ignore_systemd_inhibit = false
}

# Your EXISTING lock listener, with its existing timeout/command:
listener {
    timeout = 300
    on-timeout = loginctl lock-session
    ignore_inhibit = true
}

# Your EXISTING display-off listener, with its existing timeout/resume command:
listener {
    timeout = 600
    on-timeout = hyprctl dispatch dpms off
    on-resume = hyprctl dispatch dpms on
    ignore_inhibit = true
}

# Your EXISTING idle-suspend listener (if configured):
listener {
    timeout = 900
    on-timeout = systemctl suspend
    ignore_inhibit = false
}
```

**Important:** per-listener `ignore_inhibit = true` ignores **all inhibitors for that listener**, not only this plugin. Consider the effect on media players and other applications. This is the documented integration trade-off needed to preserve display-off/locking with this logind idle mechanism.

## Checking your setup

While a locally observed Herdr agent is working:

- Inspect `status --json` (or the popup when available) for `working` evidence, desired inhibition, resource ownership, and native request state. `accepted` is not proof of every desktop configuration.
- `systemd-inhibit --list` should show application `herdr-idle-inhibitor`, a fixed non-sensitive reason, and **idle / block**, not a strong sleep lock.
- Display-off and session locking should still follow the configured listener behavior. Idle suspend should be held back.
- Pause should remove this application's request without erasing working-agent observations. Resume should restore it only with current positive evidence.
- When work ends, the application withdraws its request after the configured release delay; it does not issue `systemctl suspend` or any other suspend command. What happens next belongs to Hypridle's existing configuration.

Another application's inhibitor can also keep the machine awake. Resource listings alone do not establish that your display/lock/idle-sleep configuration behaves as intended. Observe it deliberately on a safe machine; this plugin does not change timeouts, disable other inhibitors or trigger sleep for you.

The [Hypridle 0.1.8 configuration source](https://github.com/hyprwm/hypridle/blob/e5c01af0842bd66617f7004568df9406111d6e80/src/config/ConfigManager.cpp) defines the version-specific inhibitor settings. Other desktop mechanisms and caveats are described in [Compatibility](compatibility.md).
