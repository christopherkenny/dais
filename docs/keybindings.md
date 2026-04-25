# Keybinding Reference

All keybindings are remappable via the `[keybindings]` section in `config.toml`. The defaults are designed to feel familiar for presenter-console workflows.

## Default Keybindings

| Action | Default Key(s) | Description |
|---|---|---|
| `next_slide` | Right, Space, Down, PageDown | Advance to next logical slide |
| `previous_slide` | Left, Up, PageUp | Go back to previous logical slide |
| `next_overlay` | Shift+Right, Shift+Down | Next PDF page (overlay step) |
| `previous_overlay` | Shift+Left, Shift+Up | Previous PDF page (overlay step) |
| `first_slide` | Home | Jump to first slide |
| `last_slide` | End | Jump to last slide |
| `go_to_slide` | G then number then Enter | Jump to slide by number |
| `toggle_freeze` | F | Freeze/unfreeze audience display |
| `toggle_blackout` | B, . (period) | Black out audience display |
| `toggle_whiteboard` | W | Toggle whiteboard (white drawing canvas) |
| `toggle_laser` | L | Toggle laser pointer |
| `cycle_laser_style` | Ctrl+L | Cycle laser style: dot, crosshair, arrow |
| `toggle_ink` | D | Toggle freehand drawing |
| `clear_ink` | C | Clear all ink on current slide |
| `cycle_ink_color` | Ctrl+D | Cycle pen color |
| `cycle_ink_width` | Shift+D | Cycle pen width |
| `toggle_spotlight` | S | Toggle spotlight mode |
| `toggle_zoom` | Z | Toggle zoom mode |
| `toggle_overview` | O | Toggle slide overview grid |
| `toggle_notes` | N | Toggle notes panel visibility |
| `toggle_notes_edit` | Ctrl+N | Toggle inline notes editing |
| `start_pause_timer` | T | Start/pause timer |
| `reset_timer` | Shift+T | Reset timer |
| `increment_notes_font` | +, Shift+= | Increase notes font size |
| `decrement_notes_font` | - | Decrease notes font size |
| `toggle_screen_share` | Shift+S | Toggle screen-share mode |
| `toggle_presentation_mode` | F5 | Toggle the active single-monitor presentation surface |
| `toggle_text_box_mode` | X | Toggle text box placement mode |
| `quit` | Q, Escape | Quit presentation (exits HUD first) |
| `save_sidecar` | Ctrl+S | Save sidecar file |

## Custom Keybindings

Override any binding in `config.toml`:

```toml
[keybindings]
# Replace the defaults for an action entirely
next_slide = ["j", "Return"]
previous_slide = ["k"]

# Single key binding
toggle_laser = ["p"]

# Cycle the active laser style
cycle_laser_style = ["Ctrl+l"]

# Modifier combos
save_sidecar = ["Ctrl+Shift+s"]
```

When you define a binding for an action, it **replaces all defaults** for that action. Unmentioned actions keep their defaults.

## Modifier Keys

- `Shift` — Shift key
- `Ctrl` / `Control` / `Cmd` / `Command` — Ctrl on Windows/Linux, Cmd on macOS
- `Alt` / `Option` — Alt on Windows/Linux, Option on macOS

Combine with `+`: `Shift+Right`, `Ctrl+s`, `Ctrl+Shift+s`
