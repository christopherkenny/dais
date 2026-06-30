# Keybinding Reference

All keybindings are remappable via the `[keybindings]` section in `config.toml`.
The defaults are designed to feel familiar for presenter-console workflows.

## Default Keybindings

| Action | Default Key(s) | Description |
|---|---|---|
| `next_slide` | Right, Space, Down, PageDown | Advance to the next build step, or the next logical slide if the current slide is complete |
| `previous_slide` | Left, Up, PageUp | Go back to the previous build step, or the previous logical slide if already at the start |
| `next_overlay` | Shift+Right, Shift+Down | Next PDF page (overlay step) |
| `previous_overlay` | Shift+Left, Shift+Up | Previous PDF page (overlay step) |
| `first_slide` | Home | Jump to first slide |
| `last_slide` | End | Jump to last slide |
| `go_to_slide` | G then number then Enter | Jump to slide by number |
| `toggle_freeze` | F | Freeze/unfreeze audience display |
| `toggle_blackout` | B, . (period) | Black out audience display |
| `toggle_whiteboard` | W | Toggle whiteboard (white drawing canvas) |
| `toggle_laser` | L | Toggle laser pointer |
| `cycle_laser_style` | Ctrl+L | Cycle laser style: dot, minimal, crosshair, arrow, ring, bullseye, highlight |
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
| `swap_displays` | F6 | Swap the presenter and audience monitors for the current session |
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

When you define a binding for an action, it **replaces all defaults** for that action.
Unmentioned actions keep their defaults.

## Modifier Keys

- `Shift` — Shift key
- `Ctrl` / `Control` / `Cmd` / `Command` — Ctrl on Windows/Linux, Cmd on macOS
- `Alt` / `Option` — Alt on Windows/Linux, Option on macOS

Combine with `+`: `Shift+Right`, `Ctrl+s`, `Ctrl+Shift+s`

## Remote Actions

The remote API uses an allowlist of the same action names as this keybinding
reference for simple presenter-control commands. For example:

```powershell
dais remote action next_slide
dais remote action toggle_blackout
dais remote action toggle_laser
```

Parameterized controls use dedicated remote commands:

```powershell
dais remote goto 12
dais remote pointer 0.5 0.5
dais remote timer reset
```

This keeps external controllers generic: Stream Deck profiles, scripts, phone controls, or experimental devices can call the same stable action API without being built into Dais itself.

Not every keybinding action is remote-dispatchable.
Local-only or editing actions such as quit, save sidecar, notes editing, notes font changes, and text box mode stay inside the presenter console.
