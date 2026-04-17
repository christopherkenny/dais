# Configuration Reference

Dais supports layered configuration. Settings are applied in this order:

1. Built-in defaults
2. Machine-wide config in the platform-appropriate location
3. Project-local `dais.toml` next to the PDF you open
4. An explicit `--config <path>` file, if provided

The machine-wide config lives at:

- **Windows:** `%APPDATA%\dais\config.toml`
- **macOS:** `~/Library/Application Support/dais/config.toml`
- **Linux:** `~/.config/dais/config.toml`

If a config layer doesn't exist, Dais skips it. All settings are optional.

## Full Default Configuration

```toml
[display]
mode = "dual"                  # "dual", "single", or "screen-share"
audience_monitor = "auto"      # Monitor name, monitor id, display number like "2", or "auto"
presenter_monitor = "auto"     # Monitor name, monitor id, display number like "1", or "auto"

[timer]
mode = "elapsed"               # "countdown" or "elapsed"
# duration_minutes = 20        # Optional. If omitted in elapsed mode, no limit is shown.
# warning_minutes = 5          # Optional. Used only when duration_minutes is set.
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

## Project-Local Config

To override machine-wide settings for a specific talk or course folder, create a `dais.toml`
file next to the PDF you open.

Example:

```toml
[display]
audience_monitor = "Projector"
mode = "dual"

[timer]
mode = "elapsed"
```

## Display Modes

| Mode | Description |
|---|---|
| `dual` | Audience fullscreen on secondary monitor, presenter console on primary. Default when 2+ monitors detected. |
| `single` | Presenter console only, no audience window. Use `--single` CLI flag or set in config. |
| `screen-share` | Audience window is a normal resizable window (not fullscreen). For Zoom/Teams screen sharing. Use `--screen-share` CLI flag or set in config. Auto-selected with one monitor. |

CLI flags (`--single`, `--screen-share`) override config. If no flag is given and config is `"dual"` (default), Dais auto-detects: 2+ monitors → dual, 1 monitor → single.

## Monitor Assignment

Set `audience_monitor` and `presenter_monitor` to a monitor name, monitor id, or a 1-based display number such as `"1"` or `"2"`. Use `"auto"` for automatic assignment.

Detected monitors are logged at startup with ids and names, so you can see which selector to use.

## Timer

- **Elapsed mode:** Starts at 0:00 and counts up. This is the default. If `duration_minutes` is omitted, no limit is shown.
- **Countdown mode:** Starts at `duration_minutes` and counts down. If you use countdown mode, you should set `duration_minutes`.

## Sidecar Formats

Dais stores slide grouping, notes, and metadata in sidecar files next to your PDF.

| Format | Extension | Description |
|---|---|---|
| `pdfpc` | `.pdfpc` | Compatible with pdfpc — the default for maximum interop. |
| `dais` | `.dais` | Native EON-based format with versioning for forward compatibility. |

Set the save format in config:

```toml
sidecar_format = "pdfpc"   # "pdfpc" (default) or "dais"
```

When loading, Dais checks in order: embedded PDF metadata → `.dais` sidecar → `.pdfpc` sidecar.
The grouping editor and `save_sidecar` action both use `sidecar_format` when choosing what to write.

## Presentation Mode (F5)

In single-monitor mode, press **F5** to toggle between the full presenter console and a HUD-focused presentation view. The HUD shows:

- The audience slide fullscreen
- A semi-transparent bottom bar with timer, slide count, and mode indicators
- Hover near the bottom edge to reveal notes

Press **Escape** to exit HUD mode back to the console. In dual-monitor mode, F5 is available but the audience already has a dedicated screen.

## DPI and Scaling

Dais renders slides at the audience monitor's native resolution for maximum sharpness. The presenter console uses a fixed 1920×1080 canonical render size, scaled by the GPU.

On mixed-DPI setups (e.g., Retina laptop + 1080p projector):
- The audience window renders at the projector's native resolution
- The presenter window renders at the standard canonical size
- egui's built-in scaling handles UI element sizing per-window
