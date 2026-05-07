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
sidecar_format = "dais"        # "dais" (default) or "pdfpc"

[display]
mode = "dual"                  # "dual", "single", or "screen-share"
single_monitor_view = "hud"    # "hud" or "split" when F5 enters single-monitor presentation mode
audience_monitor = "auto"      # Monitor name, monitor id, display number like "2", or "auto"
presenter_monitor = "auto"     # Monitor name, monitor id, display number like "1", or "auto"

[timer]
mode = "elapsed"               # "countdown" or "elapsed"
# duration_minutes = 20        # Optional. If omitted in elapsed mode, no limit is shown.
# warning_minutes = 5          # Optional. Used only when duration_minutes is set.
overrun_color = true           # Red when past duration

[laser]
color = "#FF0000"              # Default for all pointer styles
size = 12.0                    # Default pixels at 1x scale
style = "dot"                  # "dot", "crosshair", "arrow", "ring", "bullseye", or "highlight"

[spotlight]
radius = 80.0                  # Pixels at 1x scale
dim_opacity = 0.6              # 0.0 = invisible, 1.0 = fully black

[ink]
colors = ["#FF0000"]           # Pen color presets, RGB or RGBA hex strings
width = 3.0

[text_boxes]
color = "#000000"
background = "transparent"    # Hex color or "transparent"

[notes]
font_size = 16.0
font_size_step = 2.0           # Increment/decrement step

[keybindings]
# See docs/keybindings.md for the full reference
# Example overrides:
# next_slide = ["j", "Return"]
# toggle_laser = ["p"]
# cycle_laser_style = ["Ctrl+l"]
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
| `single` | Single-monitor presenter mode. Press `F5` to enter either the split workspace or HUD, depending on `single_monitor_view`. |
| `screen-share` | Audience window is a normal resizable window (not fullscreen). For Zoom/Teams screen sharing. Use `--screen-share` CLI flag or set in config. |

CLI flags (`--single`, `--screen-share`) override config. If no flag is given and config is `"dual"` (default), Dais auto-detects: 2+ monitors → dual, 1 monitor → single.

## Monitor Assignment

Set `audience_monitor` and `presenter_monitor` to a monitor name, monitor id, or a 1-based display number such as `"1"` or `"2"`. Use `"auto"` for automatic assignment.

Detected monitors are logged at startup with ids and names, so you can see which selector to use.

## Timer

- **Elapsed mode:** Starts at 0:00 and counts up. This is the default. If `duration_minutes` is omitted, no limit is shown.
- **Countdown mode:** Starts at `duration_minutes` and counts down. If you use countdown mode, you should set `duration_minutes`.

## Laser Pointer

`[laser]` controls the active pointer style and the default appearance used by every pointer style.
Add `[laser.dot]`, `[laser.crosshair]`, `[laser.arrow]`, `[laser.ring]`, `[laser.bullseye]`, or `[laser.highlight]` only when you want to override one style.

Example:

```toml
[laser]
color = "#FFFFFF"
size = 12.0
style = "dot"

[laser.crosshair]
color = "#00FF00"
size = 24.0

[laser.highlight]
color = "#FFFF0080"
size = 36.0
```

In this example, dot, arrow, ring, and bullseye stay white at size 12. Crosshair changes to green at size 24, and highlight changes to translucent yellow at size 36.

## Text Boxes

`[text_boxes]` controls the default style for newly created text boxes.

- `color` accepts `#RRGGBB` or `#RRGGBBAA`
- `background` accepts `#RRGGBB`, `#RRGGBBAA`, or `"transparent"`

Example:

```toml
[text_boxes]
color = "#111111"
background = "#FFF7CCDD"
```

## Sidecar Formats

Dais stores slide grouping, notes, and metadata in sidecar files next to your PDF.

| Format | Extension | Description |
|---|---|---|
| `dais` | `.dais` | Native EON-based format with versioning for Dais annotations and text boxes. Default save format. |
| `pdfpc` | `.pdfpc` | Compatible with pdfpc for notes and overlay grouping. |

Set the save format in config:

```toml
sidecar_format = "pdfpc"   # "dais" (default) or "pdfpc"
```

When loading, Dais checks in order: `.dais` sidecar → `.pdfpc` sidecar → embedded PDF metadata.
The grouping editor and `save_sidecar` action both use `sidecar_format` when choosing what to write.

## Single-Monitor Presentation Mode (F5)

In single-monitor mode, press **F5** to toggle between the presenter console and the active presentation surface selected by `display.single_monitor_view`.

`single_monitor_view = "split"` shows:

- The audience slide on the left
- A compact presenter workspace on the right with current slide, next preview, notes, and status

`single_monitor_view = "hud"` keeps the original fullscreen HUD, which shows:

- The audience slide
- A bottom bar that appears near the lower edge
- Notes when you hover near the bottom edge

Press **Escape** to exit HUD mode back to the console. In dual-monitor mode, F5 is available but the audience already has a dedicated screen.

## Monitor Recovery

If a configured audience monitor is missing at launch, Dais now:

- Falls back gracefully to another display or single-monitor mode
- Shows a startup dialog in the presenter window so you can reassign the audience output for the current session

The reassignment dialog does not rewrite `dais.toml`; if you want the new monitor choice to persist, update `display.audience_monitor` in config afterward.

## DPI and Scaling

Dais renders slides at the audience monitor's native resolution for maximum sharpness. The presenter console uses a fixed 1920×1080 canonical render size, scaled by the GPU.

On mixed-DPI setups (e.g., Retina laptop + 1080p projector):
- The audience window renders at the projector's native resolution
- The presenter window renders at the standard canonical size
- egui's built-in scaling handles UI element sizing per-window
