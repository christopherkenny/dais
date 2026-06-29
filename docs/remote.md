# Remote Control

Dais can expose a local remote-control API while a presentation is running. The
remote layer is designed for second-device control, local scripts, Stream Deck
profiles, classroom automation, and experimental adapters that should not live
inside the core presenter.

Remote control is an input adapter. Requests become normal Dais commands, and
the presentation engine remains the only owner of presentation state.

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

The browser remote is served directly by Dais at `/remote`. It is intended to be
usable from a phone, tablet, or local browser without installing a native app.

It currently shows:

- Current slide image
- Next slide image
- Current slide number and total
- Timer state and controls
- Current slide notes
- Previous and next controls
- Blackout, freeze, whiteboard, and laser controls
- Goto slide input
- Connection state and last-command feedback

Notes are currently view-only in the remote. Notes editing remains a presenter
console feature.

## Pairing A Second Device

For a phone or tablet on the same network, bind the server to a LAN-reachable
address:

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
| `token` | Authentication token. Empty means Dais generates a short pairing code per launch |
| `allow_unauthenticated_loopback` | Allows local same-machine requests without a token |

Loopback convenience only applies to loopback clients. Non-loopback clients
always need a pairing code.

CLI flags override config for the current session:

```powershell
dais --remote slides.pdf
dais --remote-lan slides.pdf
dais --remote --remote-port 4317 slides.pdf
dais --remote --remote-host 192.168.1.24 slides.pdf
```

Use `--remote-lan` for normal phone/tablet pairing. Use `--remote-host` only
when you need to bind to a specific interface.

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
```

Shared connection options:

```powershell
dais remote --host 127.0.0.1 --port 4317 state
dais remote --host 192.168.1.24 --port 4317 --token <token> action next_slide
```

The CLI is useful for smoke testing, automation, keyboard macro tools, and
external control programs that prefer shell commands over raw HTTP.

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
```

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

For simple presenter-control commands, the remote API uses an allowlist of the
same action names as the keybinding system. Useful remote actions include:

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
```

The remote API intentionally does not expose every keybinding action. Local-only
or editing-oriented actions such as quit, save sidecar, notes editing, notes font
changes, and text box mode are not dispatched through `remote action`.

Parameterized operations, such as `goto`, `pointer`, and timer subcommands, use
dedicated endpoints and CLI commands instead of pretending every operation is a
keybinding action.

## State Shape

`GET /api/v1/state` returns a stable remote state DTO, not the internal Rust
`PresentationState` type. This keeps the API free to evolve without exposing
engine internals.

The state includes:

- Current page and current logical slide
- Total pages and total logical slides
- Overlay step information
- Timer display, running state, and phase
- Current slide notes
- Notes visibility
- Blackout, freeze, whiteboard, and screen-share state
- Laser, ink, spotlight, and zoom state
- Pointer and zoom-position data where relevant
- URLs for current and next slide images

External tools should treat this DTO as the public contract and avoid depending
on undocumented fields.

## Server-Sent Events

`GET /api/v1/events` provides a browser-friendly event stream. The built-in web
remote uses it to update state without polling.

This is intentionally simpler than WebSocket support. REST plus server-sent
events is enough for the first second-device workflow while preserving a path to
WebSockets later if richer bidirectional UI needs appear.

## External Controllers

External controllers should call the stable action API rather than adding
device-specific code to Dais.

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

That keeps tools such as Stream Deck profiles, shell scripts, and experimental
adapters outside the core app while letting them drive the same presentation
commands.

## Relationship To Clickers

Traditional USB presenter clickers usually emulate keyboard keys such as
`PageDown` and `PageUp`. Those are handled by Dais through the keybinding and
clicker-profile system. See [clicker-setup.md](clicker-setup.md).

The remote API is for controls that are not naturally keyboard input: web
remotes, scripts, hardware macro pads, networked control surfaces, and
experimental adapters.

## Security Notes

The remote server is local-first:

- It binds to loopback by default.
- LAN binding requires an explicit host choice.
- Empty tokens generate a per-launch pairing code.
- Loopback requests can be unauthenticated for convenience.
- Non-loopback requests always require token authentication.
- Browser-originating requests receive basic Host and Origin checks.

Do not expose the remote API directly to the public internet. It is intended for
local-machine and local-network control during a presentation.

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

Click `Remote` in the presenter status bar, scan the QR code with a phone, and
verify that slide images, notes, timer state, and next/previous controls all
work from the phone.

For broader manual QA, add remote checks to the same real-room testing pass as
display modes, clickers, screen-share mode, and monitor recovery.
