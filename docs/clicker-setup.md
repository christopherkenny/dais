# Clicker & Remote Setup

Dais works with USB presenter remotes (clickers) out of the box. Most clickers send standard keyboard events—PageDown, PageUp, F5, and similar—so they map directly to Dais actions through the keybinding system.

## How Clicker Profiles Work

A **clicker profile** is a named mapping from key names to Dais action names. Dais ships with a built-in `default` profile that covers the keys sent by most USB presenters. You can also define custom profiles in your config for specific hardware.

The active profile is set with `clicker.profile` in your `config.toml` or `dais.toml`.

## Default Key Mappings

These are the keys mapped by the built-in `default` clicker profile:

| Key | Action | Description |
|---|---|---|
| PageDown | `next_slide` | Advance to next slide |
| PageUp | `previous_slide` | Go back one slide |
| F5 | `toggle_presentation_mode` | Toggle fullscreen presentation mode |
| B | `toggle_blackout` | Black out the audience display |
| . (period) | `toggle_blackout` | Black out the audience display |

These overlap with the default keybindings, so most clickers work without any configuration.

## Common Presenters

### Logitech Spotlight / R500 / R400

These popular presenters send PageDown (forward), PageUp (back), and optionally F5 (start) or Escape (end). They work with the default profile. The Spotlight's "highlight" feature is an OS-level pointer and does not send key events to Dais.

### Kensington Expert / Wireless Presenter

Kensington remotes typically send PageDown, PageUp, and sometimes B for blank screen. All are covered by the default profile.

### Other USB Presenters

Most generic RF presenters send PageDown/PageUp. If yours sends different keys, use `--test-input` to discover the actual key names (see below), then create a custom profile.

## Creating a Custom Profile

Add a `[clicker]` section to your `config.toml` or project-local `dais.toml`:

```toml
[clicker]
profile = "my-remote"

[clicker.profiles.my-remote]
F5 = "toggle_presentation_mode"
Escape = "toggle_blackout"
PageDown = "next_slide"
PageUp = "previous_slide"
```

Each entry maps a key name (as shown by `--test-input`) to a Dais action name. See [keybindings.md](keybindings.md) for the full list of action names.

You can define multiple profiles and switch between them by changing `clicker.profile`:

```toml
[clicker]
profile = "logitech-spotlight"

[clicker.profiles.logitech-spotlight]
PageDown = "next_slide"
PageUp = "previous_slide"
F5 = "toggle_presentation_mode"

[clicker.profiles.kensington]
PageDown = "next_slide"
PageUp = "previous_slide"
b = "toggle_blackout"
```

## Using `--test-input` to Debug

Run the diagnostic mode to see exactly what key events your clicker sends:

```bash
dais --test-input
```

This opens a small window that displays:
- The key name Dais sees for each press
- Any active modifiers (Shift, Ctrl, Alt)
- Which action the key currently maps to

Press each button on your clicker and note the key names. Use these names when building a custom profile. Press Escape or click "Exit" to close.

You can also load a specific config to test against:

```bash
dais --test-input --config path/to/dais.toml
```

## Troubleshooting

**Clicker buttons do nothing:**
Run `--test-input` to verify Dais receives the key events. Some clickers need a USB receiver plugged in before launching Dais. On Linux, some RF receivers need the `uinput` module loaded.

**Wrong action fires:**
Check the key name with `--test-input`, then remap it in a custom clicker profile or in the `[keybindings]` section of your config.

**Clicker works in other apps but not Dais:**
Some clickers send mouse clicks or custom HID events instead of keyboard events. Dais only responds to keyboard events. Check your clicker's documentation for a "compatibility mode" that sends keyboard keys.

**Bluetooth clickers disconnect:**
This is an OS-level pairing issue, not a Dais issue. Re-pair the device. Some Bluetooth presenters work more reliably with their bundled USB RF receiver than over native Bluetooth.
