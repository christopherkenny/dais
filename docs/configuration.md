# Configuration Reference

Dais stores its configuration in a TOML file at the platform-appropriate location:

- **Windows:** `%APPDATA%\dais\config.toml`
- **macOS:** `~/Library/Application Support/dais/config.toml`
- **Linux:** `~/.config/dais/config.toml`

If the config file doesn't exist, Dais uses sensible defaults. All settings are optional.

## Full Default Configuration

```toml
[display]
mode = "dual"                  # "dual", "single", or "screen-share"
audience_monitor = "auto"      # Monitor name or "auto" (non-primary)
presenter_monitor = "auto"     # Monitor name or "auto" (primary)

[timer]
mode = "countdown"             # "countdown" or "elapsed"
duration_minutes = 20
warning_minutes = 5            # Yellow warning at this many minutes remaining
overrun_color = true           # Red when past duration

[pointer]
color = "#FF0000"
size = 12.0                    # Pixels at 1x scale
style = "dot"                  # "dot", "crosshair", or "arrow"

[spotlight]
radius = 150.0                 # Pixels at 1x scale
dim_opacity = 0.6              # 0.0 = invisible, 1.0 = fully black

[ink]
color = "#FF0000"
width = 3.0

[notes]
font_size = 16.0
font_size_step = 2.0           # Increment/decrement step

[keybindings]
# See docs/keybindings.md for the full reference
# Example overrides:
# next_slide = ["j", "Return"]
# toggle_laser = ["p"]
```

## Display Modes

| Mode | Description |
|---|---|
| `dual` | Audience fullscreen on secondary monitor, presenter console on primary. Default when 2+ monitors detected. |
| `single` | Presenter console only, no audience window. Use `--single` CLI flag or set in config. |
| `screen-share` | Audience window is a normal resizable window (not fullscreen). For Zoom/Teams screen sharing. Use `--screen-share` CLI flag or set in config. Auto-selected with one monitor. |

CLI flags (`--single`, `--screen-share`) override config. If no flag is given and config is `"dual"` (default), Dais auto-detects: 2+ monitors → dual, 1 monitor → screen-share.

## Monitor Assignment

Set `audience_monitor` and `presenter_monitor` to monitor names (as reported by your OS) for persistent assignment. Use `"auto"` for automatic assignment (primary = presenter, other = audience).

Monitor names are logged at startup — run Dais once to see available names.

## Timer

- **Countdown mode:** Starts at `duration_minutes` and counts down. Yellow at `warning_minutes` remaining, red when overrun.
- **Elapsed mode:** Starts at 0:00 and counts up. Yellow at `duration_minutes - warning_minutes`, red past `duration_minutes`.
