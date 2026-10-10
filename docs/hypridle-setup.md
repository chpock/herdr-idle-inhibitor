# Optional Hyprland / Hypridle display tuning

[Home](../README.md) · [Configuration](configuration.md) · [Compatibility](compatibility.md)

The supported version is **Hypridle 0.1.8**. The application uses logind `Inhibit("idle", ..., "block")`, not a strong `sleep` lock. No display-listener setup or confirmation is required to enable the plugin. By default, Hypridle can honor the same idle request for both sleep and display/lock actions, so the screen may stay on while agents work.

This optional guide is for [Linux display-off troubleshooting](troubleshooting.md#linux-screen-does-not-turn-off): use it if you want display-off/locking to continue while idle sleep is inhibited. The application never rewrites this file or restarts desktop services.

## Optional configuration steps

1. Run `hypridle --version` and inspect your existing Hypridle configuration.
2. In the existing `general` block, ensure `ignore_systemd_inhibit = false`. Do not change the existing lock, suspend, resume, or other commands.
3. Identify **every** listener that dims the screen, powers the display off, or locks the session. Add `ignore_inhibit = true` to these listeners only.
4. For idle-suspend/hibernate listeners, leave `ignore_inhibit` false (its default) or set it explicitly to false. Do not add a suspend command where none exists.
5. Keep each existing timeout, command and resume action unchanged. If one listener mixes display-off/locking with suspend, separate its policy intentionally before applying the override; do not indiscriminately allow the mixed action to ignore inhibition.
6. Apply the configuration using your own normal Hypridle/service workflow. This plugin does not perform reloads or restarts for you.
No plugin setting, Reload or confirmation is needed afterwards. Existing `hypridle_integration_confirmed` values in old plugin configuration files are accepted but ignored; they no longer control acquisition.

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

**Important:** per-listener `ignore_inhibit = true` ignores **all inhibitors for that listener**, not only this plugin. Consider the effect on media players and other applications. This is an optional display-policy trade-off, not a prerequisite for preventing idle sleep.

## Checking the optional changes

While a locally observed Herdr agent is working:

- Inspect `status --json` (or the popup when available) for `working` evidence, desired inhibition, resource ownership, and native request state. `accepted` is not proof of every desktop configuration.
- `systemd-inhibit --list` should show application `herdr-idle-inhibitor`, a fixed non-sensitive reason, and **idle / block**, not a strong sleep lock.
- After the optional listener changes, display-off and session locking should follow those listeners even while the idle request is held. Idle suspend should still honor the request.
- Pause should remove this application's request without erasing working-agent observations. Resume should restore it only with current positive evidence.
- When work ends, the application withdraws its request after the configured release delay; it does not issue `systemctl suspend` or any other suspend command. What happens next belongs to Hypridle's existing configuration.

Another application's inhibitor can also keep the machine awake. Resource listings alone do not establish that your display/lock/idle-sleep configuration behaves as intended. Observe it deliberately on a safe machine; this plugin does not change timeouts, disable other inhibitors or trigger sleep for you.

The [Hypridle 0.1.8 configuration source](https://github.com/hyprwm/hypridle/blob/e5c01af0842bd66617f7004568df9406111d6e80/src/config/ConfigManager.cpp) defines the version-specific inhibitor settings. Other desktop mechanisms and caveats are described in [Compatibility](compatibility.md).
