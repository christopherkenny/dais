# Remote Control

Dais can expose a local remote-control API while a presentation is running.
The remote layer is designed for second-device control, local scripts, Stream Deck profiles, classroom automation, and experimental adapters that should not live inside the core presenter.

Remote control is an input adapter.
Requests become normal Dais commands, and the presentation engine remains the only owner of presentation state.

## Quick Start

Start a presentation with the remote server enabled:

```powershell
dais --remote slides.pdf
```

For development from the repository:

```powershell
cargo run -p dais -- --remote tests/example.pdf
```

By default, Dais listens on:

```text
http://127.0.0.1:4317
```

Open the built-in browser remote:

```text
http://127.0.0.1:4317/remote
```

Or send commands from another terminal:

```powershell
dais remote state
dais remote action next_slide
dais remote action previous_slide
dais remote goto 12
dais remote timer toggle
```

## Browser Remote

The browser remote is served directly by Dais at `/remote`.
It is intended to be usable from a phone, tablet, or local browser without installing a native app.

It currently shows:

- Current slide image
- Next slide image
- Current slide number and total
- Timer state and controls
- Current slide notes, with inline Edit / Save / Cancel controls
- Previous and next controls
- Blackout, freeze, whiteboard, and laser controls
- Goto slide input
- Drawing and text-box controls on the Annotate tab
- Connection state and last-command feedback

### Notes editing

The Notes tab has an **Edit** button that opens the current slide's notes in a text field.
Click **Save** to write the change and persist it to the sidecar immediately.
**Cancel** discards the draft.
Slide changes received while editing do not overwrite the draft.

### Annotating

The Annotate tab shows the current slide with an overlay for drawing and text boxes.
Draw with a finger, mouse, or stylus (including Apple Pencil on iPad).
Each completed stroke is sent to the presenter screen and saved to the sidecar immediately.

Controls:

- **Tool selector** — **Pen**, **Hi** (highlighter), **Eraser**.
  Pen draws opaque strokes using the selected color and width.
  Highlighter draws semi-transparent wide strokes useful for marking key points.
  Eraser removes ink by proximity — drag over strokes to clip them away.
- **Text** — switches the Annotate tab to text-box editing.
- **Pen colors** — color swatches matching the presenter's configured ink palette.
- **Highlighter colors** — semi-transparent yellow, green, cyan, and pink (or configured presets).
- **Width** — Thin, Med, Thick (different pixel sizes for pen vs. highlighter).
- **Clear** — removes all ink on the current slide.

The active tool is kept in sync with the presenter console.
The local canvas provides immediate feedback while the stroke or erase is in transit, then refreshes from the presenter's authoritative ink state.
When the presenter navigates to a different slide, the canvas clears automatically to match the new slide.
When whiteboard mode is active, the Annotate tab switches to a blank white drawing surface and remote strokes are saved to the shared whiteboard instead of the current slide.

Drawing works alongside the presenter's own ink tools.
If the presenter already has ink mode active, remote strokes are added without toggling it; if ink mode is off, the remote enables it for the stroke and restores it afterward.

### Text boxes

The **Text** tool edits slide text boxes from the browser remote.
Drag on empty slide space to place a new text box.
Tap a text box to select it.
Drag the selected box to move it, drag its lower-right handle to resize it, edit its content in the text field, and use **Save** or **Delete** to persist the change.

The browser remote previews saved text boxes with SVGs rendered by Dais' Typst renderer.
The edit field remains plain text so Typst markup can be entered directly.

## Pairing A Second Device

For a phone or tablet on the same network, bind the server to a LAN-reachable address:

```powershell
dais --remote-lan slides.pdf
```

When remote mode is enabled, the presenter console shows a `Remote` item in the
bottom status bar. Click it to see copyable pairing URLs, the current pairing
code, and QR codes for phone/tablet URLs.

If Dais is bound to `0.0.0.0`, it does not advertise `0.0.0.0` as a pairing URL.
Instead, it shows loopback and likely LAN URLs such as:

```text
http://127.0.0.1:4317/remote
http://192.168.1.24:4317/remote?token=...
```

Non-loopback devices always need the pairing code. If a phone cannot connect, check:

- The phone and computer are on the same network.
- The network allows devices to reach each other.
- The OS firewall allowed Dais to accept local network connections.
- The phone used the tokenized pairing URL or QR code from the Dais presenter UI.

## Configuration

Remote settings live under `[remote]` in `config.toml` or project-local
`dais.toml`:

```toml
[remote]
enabled = false
host = "127.0.0.1"
port = 4317
token = ""
allow_unauthenticated_loopback = true
```

Fields:

| Field | Description |
|---|---|
| `enabled` | Start the remote server when a presentation starts |
| `host` | Bind address. `127.0.0.1` is local-only; `0.0.0.0` accepts connections on all interfaces |
| `port` | TCP port. Use `0` to ask the OS for a free port |
| `token` | Authentication token. Empty means Dais generates a short pairing code per launch. Custom tokens may contain only ASCII letters and digits |
| `allow_unauthenticated_loopback` | Allows local same-machine requests without a token |

Loopback convenience only applies to loopback clients.
Non-loopback clients always need a pairing code.

CLI flags override config for the current session:

```powershell
dais --remote slides.pdf
dais --remote-lan slides.pdf
dais --remote --remote-port 4317 slides.pdf
dais --remote --remote-host 192.168.1.24 slides.pdf
```

Use `--remote-lan` for normal phone/tablet pairing.
Use `--remote-host` only when you need to bind to a specific interface.

## CLI Remote

The same `dais` binary can control an already-running presentation:

```powershell
dais remote state
dais remote action next_slide
dais remote action toggle_blackout
dais remote goto 12
dais remote pointer 0.5 0.5
dais remote timer start
dais remote timer pause
dais remote timer toggle
dais remote timer reset
dais remote notes "Mention the live demo here."
```

Shared connection options:

```powershell
dais remote --host 127.0.0.1 --port 4317 state
dais remote --host 192.168.1.24 --port 4317 --token <token> action next_slide
```

The CLI is useful for smoke testing, automation, keyboard macro tools, and external control programs that prefer shell commands over raw HTTP.

## REST API

All API endpoints are under `/api/v1`.

| Endpoint | Method | Purpose |
|---|---|---|
| `/api/v1/state` | `GET` | Return a stable presentation state snapshot |
| `/api/v1/events` | `GET` | Server-sent events for browser-friendly updates |
| `/api/v1/remote-status` | `GET` | Remote connection/status metadata |
| `/api/v1/actions/{action_name}` | `POST` | Dispatch a public Dais action |
| `/api/v1/commands/goto` | `POST` | Jump to a 1-based logical slide |
| `/api/v1/commands/pointer` | `POST` | Set normalized pointer position |
| `/api/v1/commands/timer` | `POST` | Start, pause, toggle, or reset the timer |
| `/api/v1/commands/notes` | `POST` | Set speaker notes for the current slide and save |
| `/api/v1/commands/ink/stroke` | `POST` | Add an ink stroke to the current slide and save |
| `/api/v1/commands/ink/erase` | `POST` | Erase ink near a path of points (segment-accurate) |
| `/api/v1/commands/ink/set_tool` | `POST` | Switch the active draw tool: `"pen"`, `"highlighter"`, or `"eraser"` |
| `/api/v1/commands/ink/clear` | `POST` | Clear all ink on the current slide and save |
| `/api/v1/commands/text-boxes/place` | `POST` | Place a text box on the current slide and save |
| `/api/v1/commands/text-boxes/select` | `POST` | Select a text box on the current slide |
| `/api/v1/commands/text-boxes/content` | `POST` | Update text box content and save |
| `/api/v1/commands/text-boxes/move` | `POST` | Move a text box and save |
| `/api/v1/commands/text-boxes/resize` | `POST` | Resize a text box and save |
| `/api/v1/commands/text-boxes/delete` | `POST` | Delete a text box and save |
| `/api/v1/text-boxes/<id>/svg?w=<px>&h=<px>&slide_w=<px>&slide_h=<px>` | `GET` | Render a current-slide text box as a Typst SVG |
| `/api/v1/slides/current.png` | `GET` | Render the current page as PNG |
| `/api/v1/slides/next.png` | `GET` | Render the next logical slide as PNG |
| `/api/v1/slides/<n>/thumbnail.png` | `GET` | Render logical slide `n` as PNG |

Examples:

```powershell
curl http://127.0.0.1:4317/api/v1/state
curl -X POST http://127.0.0.1:4317/api/v1/actions/next_slide
curl -X POST http://127.0.0.1:4317/api/v1/actions/toggle_blackout
curl http://127.0.0.1:4317/api/v1/slides/current.png --output current.png
```

JSON command examples:

```powershell
curl -X POST http://127.0.0.1:4317/api/v1/commands/goto `
  -H "Content-Type: application/json" `
  -d '{ "slide": 12 }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/pointer `
  -H "Content-Type: application/json" `
  -d '{ "x": 0.5, "y": 0.5 }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/timer `
  -H "Content-Type: application/json" `
  -d '{ "action": "toggle" }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/notes `
  -H "Content-Type: application/json" `
  -d '{ "notes": "Mention the live demo here." }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/ink/stroke `
  -H "Content-Type: application/json" `
  -d '{ "points": [[0.1,0.2],[0.5,0.5],[0.9,0.8]], "tool": "highlighter", "color": [255,220,0,100], "width": 12.0 }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/ink/erase `
  -H "Content-Type: application/json" `
  -d '{ "points": [[0.3,0.4],[0.4,0.5]], "radius": 0.03 }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/ink/set_tool `
  -H "Content-Type: application/json" `
  -d '{ "tool": "highlighter" }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/ink/clear

curl -X POST http://127.0.0.1:4317/api/v1/commands/text-boxes/place `
  -H "Content-Type: application/json" `
  -d '{ "x": 0.15, "y": 0.2, "w": 0.3, "h": 0.12 }'

curl -X POST http://127.0.0.1:4317/api/v1/commands/text-boxes/content `
  -H "Content-Type: application/json" `
  -d '{ "id": 4, "content": "Remember the demo." }'
```

Ink stroke body fields:

| Field | Type | Required | Description |
|---|---|---|---|
| `points` | `[[f32, f32]]` | Yes | Two or more `[x, y]` pairs in normalized 0–1 coordinates |
| `tool` | `string` | No | `"pen"` or `"highlighter"` for this stroke. When present, Dais applies the tool before color, width, and points in one command batch |
| `color` | `[u8, u8, u8, u8]` | No | RGBA pen color; uses the active pen color if omitted. Pass alpha < 255 for highlighter strokes |
| `width` | `f32` | No | Stroke width in logical pixels; uses the active pen width if omitted |

Ink erase body fields:

| Field | Type | Required | Description |
|---|---|---|---|
| `points` | `[[f32, f32]]` | Yes | One or more `[x, y]` positions in normalized 0–1 coordinates. Each point erases within the given radius |
| `radius` | `f32` | No | Erase circle radius in normalized coordinates. Defaults to `0.03` (approximately 29 px on a 960 px slide). Clamped to 0.001–0.5 |

Set-tool body fields:

| Field | Type | Required | Description |
|---|---|---|---|
| `tool` | `string` | Yes | `"pen"`, `"highlighter"`, or `"eraser"` |

Text box body fields:

| Endpoint | Fields |
|---|---|
| `place` | `x`, `y`, `w`, and `h` as normalized `f32` slide coordinates |
| `select` | `id` as the text box id |
| `content` | `id` and `content` |
| `move` | `id`, `x`, and `y` as normalized coordinates |
| `resize` | `id`, `w`, and `h` as normalized sizes |
| `delete` | `id` |

Token-protected requests can authenticate with either header:

```powershell
curl http://192.168.1.24:4317/api/v1/state `
  -H "Authorization: Bearer <token>"

curl http://192.168.1.24:4317/api/v1/state `
  -H "X-Dais-Token: <token>"
```

Browser flows can also pass the pairing code in the URL:

```text
?token=<token>
```

## Action Names

For simple presenter-control commands, the remote API uses an allowlist of the same action names as the keybinding system.
Useful remote actions include:

```text
next_slide
previous_slide
next_overlay
previous_overlay
first_slide
last_slide
toggle_blackout
toggle_freeze
toggle_whiteboard
toggle_laser
cycle_laser_style
toggle_ink
clear_ink
toggle_spotlight
toggle_zoom
toggle_overview
toggle_notes
start_pause_timer
reset_timer
toggle_screen_share
toggle_presentation_mode
swap_displays
```

The remote API intentionally does not expose every keybinding action.
Local-only or editing-oriented actions such as quit, save sidecar, notes editing, notes font changes, and text box mode are not dispatched through `remote action`.

Parameterized operations, such as `goto`, `pointer`, and timer subcommands, use dedicated endpoints and CLI commands instead of pretending every operation is a keybinding action.

## State Shape

`GET /api/v1/state` returns a stable remote state DTO, not the internal Rust `PresentationState` type.
This keeps the API free to evolve without exposing engine internals.

The state includes:

- Current page and current logical slide
- Total pages and total logical slides
- Overlay step information
- Timer display, running state, and phase
- Current slide notes
- Notes visibility
- Blackout, freeze, whiteboard, and screen-share state
- Laser, ink, spotlight, and zoom state
- Active draw tool (`draw_tool`: `"pen"`, `"highlighter"`, or `"eraser"`)
- Active pen color (`ink_pen_color`) and width (`ink_pen_width`)
- Pen color presets (`ink_color_presets`)
- Active highlighter color (`ink_highlighter_color`) and width (`ink_highlighter_width`)
- Highlighter color presets (`ink_highlighter_color_presets`)
- Active slide or whiteboard ink strokes (`ink_strokes`)
- Text box mode, selected text box, editing flag, and active slide text boxes (`text_boxes`)
- Pointer and zoom-position data where relevant
- URLs for current and next slide images

External tools should treat this DTO as the public contract and avoid depending
on undocumented fields.

## Server-Sent Events

`GET /api/v1/events` provides a browser-friendly event stream. The built-in web
remote uses it to update state without polling.

This is intentionally simpler than WebSocket support.
REST plus server-sent events is enough for the current second-device workflow while preserving a path to WebSockets later if richer bidirectional UI needs appear.

## External Controllers

External controllers call the stable action API rather than adding device-specific code to Dais.

Good fits include:

- Stream Deck profiles
- Shell scripts
- AutoHotkey or PowerShell helpers
- Classroom control panels
- Phone or tablet browser remotes
- Sensor or gesture experiments

For example, a hardware button can send:

```powershell
dais remote action next_slide
dais remote action toggle_blackout
```

Tools that can send HTTP requests can call the API directly:

```text
POST /api/v1/actions/next_slide
```

or:

```text
POST /api/v1/actions/toggle_blackout
```

That keeps tools such as Stream Deck profiles, shell scripts, and experimental adapters outside the core app while letting them drive the same presentation commands.

## Relationship To Clickers

Traditional USB presenter clickers usually emulate keyboard keys such as `PageDown` and `PageUp`.
Those are handled by Dais through the keybinding and clicker-profile system.
See [clicker-setup.md](clicker-setup.md).

The remote API is for controls that are not naturally keyboard input: web remotes, scripts, hardware macro pads, networked control surfaces, and experimental adapters.

## Security Notes

The remote server is local-first:

- It binds to loopback by default.
- LAN binding requires an explicit host choice.
- Empty tokens generate a per-launch pairing code.
- Loopback requests can be unauthenticated for convenience.
- Non-loopback requests always require token authentication.
- Browser-originating requests receive basic Host and Origin checks.

Do not expose the remote API directly to the public internet.
It is intended for local-machine and local-network control during a presentation.

## Testing

Basic local test:

```powershell
cargo run -p dais -- --remote --remote-port 4317 tests/example.pdf
```

Then, in another terminal:

```powershell
curl http://127.0.0.1:4317/api/v1/state
curl -X POST http://127.0.0.1:4317/api/v1/actions/next_slide
cargo run -p dais -- remote --port 4317 goto 2
```

Second-device test:

```powershell
cargo run -p dais -- --remote-lan --remote-port 4317 tests/example.pdf
```

Click `Remote` in the presenter status bar, scan the QR code with a phone, and verify that slide images, notes, timer state, and next/previous controls all work from the phone.

For broader manual QA, add remote checks to the same real-room testing pass as display modes, clickers, screen-share mode, and monitor recovery.
