# Clicker & Remote Setup

Dais reads clickers as keyboard input. Most USB presenter remotes send standard keys such as `PageDown`, `PageUp`, and `F5`, so they work through the normal keybinding system.

## How Clicker Profiles Work

A **clicker profile** is a named mapping from key names to Dais action names. Dais ships with a built-in `default` profile and also supports custom profiles in config.

The active profile is set with `clicker.profile` in your `config.toml` or `dais.toml`.

## Default Key Mappings

These are the keys mapped by the built-in `default` clicker profile:

| Key | Action | Description |
|---|---|---|
| PageDown | `next_slide` | Advance to next build step or slide |
| PageUp | `previous_slide` | Go back one build step or slide |
| F5 | `toggle_presentation_mode` | Toggle fullscreen presentation mode |
| B | `toggle_blackout` | Black out the audience display |
| . (period) | `toggle_blackout` | Black out the audience display |

These overlap with the default keybindings, so most clickers work without any configuration.

## Common Presenters

Most generic USB presenters send `PageDown` and `PageUp`. If yours sends different keys, use `--test-input` to discover the actual key names, then create a custom profile.

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
- Any active modifiers (`Shift`, `Ctrl`, `Alt`)
- Which action the key currently maps to

Press each button on your clicker and note the key names. Use these names when building a custom profile. Press Escape or click "Exit" to close.

You can also load a specific config to test against:

```bash
dais --test-input --config path/to/dais.toml
```

## Troubleshooting

**Clicker buttons do nothing:**
Run `--test-input` to verify that Dais receives the key events. Some clickers need their USB receiver connected before launch.

**Wrong action fires:**
Check the key name with `--test-input`, then remap it in a custom clicker profile or in `[keybindings]`.

**Clicker works in other apps but not Dais:**
Some clickers send mouse clicks or custom HID events instead of keyboard events. Dais only responds to keyboard events.

**Bluetooth clickers disconnect:**
This is usually an OS-level pairing issue. Some Bluetooth presenters work more reliably through their bundled USB receiver than through native Bluetooth.
