# Dais Interactive Testing Checklist

Manual QA checklist for verifying all user-facing functionality.
Organized by category so tests can be executed efficiently in batches.

**Test files:** `tests/example.pdf`, `tests/fixtures/test.pdf`, `tests/fixtures/beamer-example.pdfpc`, `tests/fixtures/quarto-example.dais`

> **Notation:** ✅ = pass, ❌ = fail, ⏭ = skipped (note reason)
>
> Fill in the result column as you go. If a test fails, note what actually happened in the **Actual** column.

---

## A. Startup & CLI

Run each from a terminal. Verify before interacting with the window.

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| A1 | No-arg launch | Run `dais` with no arguments | Prints error: `Usage: dais <file.pdf>` and exits with non-zero code | | |
| A2 | PDF open | Run `dais tests/example.pdf` | Presenter window opens titled "Dais — Presenter Console". First slide is visible. No crash, no error in terminal | | |
| A3 | `--single` flag | Run `dais --single tests/example.pdf` | Opens in single mode: only one window appears (no audience window). Starts in fullscreen HUD immediately (presentation_mode toggled on at launch) | | |
| A4 | `--screen-share` flag | Run `dais --screen-share tests/example.pdf` | Two windows: presenter console + a resizable "Dais — Audience" window (not fullscreen). Audience window is draggable and resizable | | |
| A5 | `--config` flag | Create a `test.toml` with `[timer]\nmode = "countdown"\nduration_minutes = 5`. Run `dais --config test.toml tests/example.pdf` | Timer shows countdown from 05:00 instead of 00:00 count-up | | |
| A6 | `--edit` flag | Run `dais --edit tests/example.pdf` | Opens the Grouping Editor window titled "Dais — Grouping Editor". Shows a horizontal filmstrip of page thumbnails with alternating group backgrounds | | |
| A7 | `--test-input` no PDF | Run `dais --test-input` | Opens a "Dais — Test Input" diagnostic window. No PDF required. Shows key event log area | | |
| A8 | `--test-input` with config | Run `dais --test-input --config test.toml` | Same as A7, but uses keybindings from the config file. Pressing keys shows mapped actions in the log | | |
| A9 | Invalid PDF | Run `dais nonexistent.pdf` | Prints a clear error about file not found and exits non-zero | | |
| A10 | Version | Run `dais --version` | Prints `dais <version>` and exits | | |
| A11 | App icon | After A2 launch, inspect the taskbar/dock icon | Dais icon (not default framework icon) is visible in the taskbar and window title bar | | |

---

## B. Dual-Monitor Display (requires 2 monitors)

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| B1 | Auto dual detection | With 2 monitors connected, run `dais tests/example.pdf` | Presenter console on primary monitor, audience window fullscreen on secondary. Log shows "Dual mode: audience on '<name>'" | | |
| B2 | Audience fullscreen | Observe the audience window on secondary | Window is truly fullscreen — no title bar, no borders, fills entire monitor. Background is black with centered slide | | |
| B3 | Presenter placement | Observe the presenter console | Window is centered on primary monitor, clamped to fit within monitor bounds (not overflowing off screen) | | |
| B4 | Audience monitor by name | Set `[display]\naudience_monitor = "<exact monitor name>"` in config. Launch | Audience window placed on the named monitor. Log confirms match | | |
| B5 | Audience monitor by number | Set `audience_monitor = "2"` in config. Launch | Audience window placed on 2nd monitor (matches by ordinal position) | | |
| B6 | Wrong monitor name | Set `audience_monitor = "NONEXISTENT"` in config. Launch with 2 monitors | Warning toast appears ("Configured audience monitor 'NONEXISTENT' not found"). Falls back to secondary monitor for audience. Presenter still works | | |
| B7 | Runtime screen-share toggle | Press `Shift+S` during dual-mode presentation | Audience window changes from fullscreen to resizable windowed. Status bar shows "[S]creen-share" indicator | | |
| B8 | Screen-share toggle back | Press `Shift+S` again | Audience returns to fullscreen on secondary. Indicator disappears | | |

---

## C. Single-Monitor Fallback

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| C1 | Auto fallback | With only 1 monitor, run `dais tests/example.pdf` (default dual config) | Falls back to Single mode. Warning toast: "Single monitor detected — expected dual. Using single mode." Only one window visible. Starts in HUD presentation mode | | |
| C2 | Exit HUD to console | Press `Escape` (or `q`) | Exits fullscreen HUD. Shows the normal presenter console layout (current slide, next preview, notes, status bar) in a single window | | |
| C3 | Re-enter HUD | Press `F5` | Returns to fullscreen HUD with the slide filling the screen and a semi-transparent bottom bar | | |
| C4 | No audience window in single | Verify after C2 | Only one application window exists. No second "Audience" window spawned | | |

---

## D. HUD Presentation Mode (Single Monitor)

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| D1 | F5 toggles HUD | From presenter console, press `F5` | View switches to fullscreen HUD: slide fills screen, black background. Semi-transparent bar appears when cursor near bottom | | |
| D2 | HUD bar hover | Move cursor to bottom ~64px of screen | Semi-transparent dark bar (48px tall) fades in at the bottom. Shows: slide position (e.g., "1 / 5"), timer, per-slide timer, and mode indicators (right side) | | |
| D3 | HUD bar content | Read the HUD bar | Shows: `<slide>/<total>`, overlay step if multi-overlay group (e.g., "step 1/3"), timer with ▶/⏸ icon, "Slide 00:XX" per-slide timer | | |
| D4 | HUD bar hide | Move cursor to center of screen | HUD bar fades out (no longer visible) | | |
| D5 | HUD notes hover | Move cursor to bottom ~80px of screen (on a slide that has notes) | Notes panel (200px tall) appears above the HUD bar. Shows notes text (scrollable if long). Only shows if current slide has notes | | |
| D6 | HUD notes absent | Navigate to a slide with no notes, hover at bottom | HUD bar appears but notes panel does NOT appear | | |
| D7 | HUD timer click | In HUD bar, click on the timer text | Timer toggles between running/paused. Icon changes from ▶ to ⏸ | | |
| D8 | HUD mode indicators | Activate laser (`l`), then hover HUD bar | "LASER" indicator appears in red on the right side of the HUD bar. Similarly "INK", "FROZEN", "SPOT", "ZOOM" appear for their respective modes | | |
| D9 | Escape exits HUD first | In HUD mode, press `Escape` | Exits HUD → shows presenter console. Does NOT quit the app. A second `Escape` (or `q`) would quit | | |
| D10 | Navigation in HUD | Press `Right` arrow while in HUD | Slide advances. Audience slide (the displayed one) updates | | |
| D11 | All overlays in HUD | Activate laser, ink, spotlight, zoom while in HUD | Each overlay renders on the fullscreen slide just as it would on the audience display | | |
| D12 | Blackout in HUD | Press `b` while in HUD | Screen goes black (audience blackout overlay covers the slide) | | |

---

## E. Navigation

Use `tests/fixtures/test.pdf` with the `beamer-example.pdfpc` sidecar for overlay group tests (copy .pdfpc next to test.pdf or use `tests/example.pdf`).

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| E1 | Next slide (Right) | Press `Right` | Advances to the next build step within the current logical slide before moving to the next logical slide. Slide/step counter, current slide panel, and next preview update | | |
| E2 | Next slide (Space) | Press `Space` | Same as E1 | | |
| E3 | Next slide (Down) | Press `Down` | Same as E1 | | |
| E4 | Next slide (PageDown) | Press `PageDown` | Same as E1 | | |
| E5 | Previous slide (Left) | Press `Left` | Rewinds to the previous build step within the current logical slide before moving to the previous logical slide. Slide/step counter updates | | |
| E6 | Previous slide (Up/PageUp) | Press `Up` or `PageUp` | Same as E5 | | |
| E7 | Next overlay (Shift+Right) | Press `Shift+Right` | Advances one raw PDF page within the current group. Status bar shows step counter (e.g., "step 2/3"). If at last overlay, advances to next slide | | |
| E8 | Previous overlay (Shift+Left) | Press `Shift+Left` | Goes back one raw page within the group. If at first overlay of a group, goes to last overlay of the previous group | | |
| E9 | First slide (Home) | Press `Home` | Jumps to slide 1 / page 0. Slide counter shows "1 / N" | | |
| E10 | Last slide (End) | Press `End` | Jumps to the last logical slide | | |
| E11 | Go to slide (G + digits + Enter) | Press `g`, then `3`, then `Enter` | Jumps to logical slide 3 (1-based). Status bar briefly shows "Go to: 3_" while digits accumulate | | |
| E12 | Go to slide cancel | Press `g`, then `5`, then `Escape` | Jump-to-slide mode cancelled. No navigation occurs | | |
| E13 | Go to slide timeout | Press `g`, then `1`, wait 3+ seconds | Jump-to-slide mode auto-cancels after 3 seconds of inactivity | | |
| E14 | Past last slide → blackout | Navigate to the last slide, then press `Right` | Audience display goes black (end-of-deck blackout). Presenter still shows the last slide | | |
| E15 | Blackout recovery | From E14 state, press `Left` | Blackout clears, returns to the last slide (not the second-to-last) | | |
| E16 | Ink cleared on navigate | Draw some ink (press `d`, draw, press `d`), then press `Right` | Ink strokes are cleared when navigating to a new slide | | |
| E17 | Audience follows navigation | During dual mode, navigate slides | Audience window updates in sync with presenter navigation (unless frozen) | | |

---

## F. Timer

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| F1 | Timer click to start | Click the timer text in the status bar | Timer starts counting. Display changes from `▶ 00:00` to `⏸ 00:01`, `00:02`, ... Icon shows ⏸ (pause-able) | | |
| F2 | Timer click to pause | Click again while running | Timer pauses. Time value freezes. Icon shows ▶ (resume-able) | | |
| F3 | Timer keyboard toggle | Press `t` | Timer toggles running/paused (same as click) | | |
| F4 | Timer reset | Press `Shift+T` | Timer resets to 00:00 and stops | | |
| F5 | Countdown mode | Set config `[timer]\nmode = "countdown"\nduration_minutes = 2`. Launch | Timer shows `▶ 02:00 / 02:00`. Clicking starts countdown. Display shows remaining time | | |
| F6 | Countdown warning | Set config `warning_minutes = 1` with 2-min countdown. Start timer, wait until under 1 min remaining | Timer text color changes to **yellow** (warning phase) | | |
| F7 | Countdown overrun | Let the countdown reach 00:00 and continue | Timer text changes to **red** (overrun phase). Time shows `00:00` (clamped, doesn't go negative) | | |
| F8 | Per-slide timer | Navigate between slides while timer is running | "Slide XX:XX" counter resets when changing slides. Each slide accumulates its own time. Going back to a previous slide resumes its accumulated time | | |
| F9 | Timer persists resume | Pause timer, navigate slides, resume timer | Elapsed time resumes from where it was paused (not reset) | | |

---

## G. Freeze & Blackout

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| G1 | Freeze toggle | Press `f` | Audience display freezes on current slide. Status bar shows **[F]rozen** in light blue. Presenter can navigate freely; audience stays on frozen page | | |
| G2 | Freeze verify audience | While frozen, press `Right` several times | Presenter advances slides (current slide panel updates). Audience window does NOT change — still shows the frozen page | | |
| G3 | Unfreeze | Press `f` again | Audience jumps to the presenter's current page. [F]rozen indicator disappears | | |
| G4 | Blackout toggle | Press `b` | Audience display goes completely black. Status bar shows **[B]lack** in yellow | | |
| G5 | Blackout also with period | Press `.` | Same as G4 | | |
| G6 | Unblackout | Press `b` again | Audience display returns to current slide. Indicator disappears | | |
| G7 | Navigate during blackout | While blacked out, press `Right` | Blackout clears, navigates forward (blackout is cleared on NextSlide) | | |
| G8 | Overlay during blackout | While blacked out, press `Shift+Right` | Blackout clears, advances one overlay step | | |

---

## H. Presentation Aids: Laser Pointer

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| H1 | Laser toggle | Press `l` | Laser mode activates. Status bar shows **[L]aser** in red | | |
| H2 | Laser dot on presenter | With laser on, move mouse over the current slide panel | Small red dot follows the cursor on the presenter's current slide view | | |
| H3 | Laser dot on audience | Same mouse movement | Red dot (with outer glow) appears at the corresponding position on the audience window | | |
| H4 | Laser off | Press `l` again | Laser deactivates. Dot disappears from both windows. Pointer position clears | | |
| H5 | Laser ↔ ink exclusive | With laser on, press `d` (ink toggle) | Laser deactivates, ink activates. They are mutually exclusive — only one at a time | | |
| H6 | Laser off-slide | Move cursor outside the slide image area | Laser position does not update (remains at last valid position or disappears) | | |

---

## I. Presentation Aids: Freehand Ink

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| I1 | Ink toggle | Press `d` | Ink mode activates. Status bar shows **[D]raw** in orange | | |
| I2 | Draw stroke | With ink on, click-drag on the current slide | A red stroke (default: #FF0000, width 3.0) appears following the drag path. Stroke renders on both presenter and audience | | |
| I3 | Multiple strokes | Release and draw again | Each drag creates a separate stroke. All persist on screen | | |
| I4 | Clear ink | Press `c` | All ink strokes on the current page are cleared from both displays | | |
| I5 | Ink off | Press `d` again | Ink mode deactivates. Indicator disappears. Existing strokes remain visible until cleared or slide change | | |
| I6 | Ink ↔ laser exclusive | With ink on, press `l` | Ink deactivates, laser activates. Mutually exclusive | | |
| I7 | Ink cleared on slide change | Draw ink, then press `Right` | Strokes cleared when navigating to a new slide (go_to_group clears ink) | | |
| I8 | Ink config color | Set config `[ink]\ncolors = ["#00FF00"]\nwidth = 5.0`. Draw | Strokes are green and thicker | | |

---

## J. Presentation Aids: Spotlight

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| J1 | Spotlight toggle | Press `s` | Spotlight mode activates. Status bar shows **Spotlight** in light yellow | | |
| J2 | Spotlight effect | Move mouse over the slide | Area around the cursor is bright; rest of the slide is dimmed with semi-transparent black overlay. A thin white square border marks the spotlight edge | | |
| J3 | Spotlight on audience | Move mouse on presenter | Audience window shows same spotlight effect at corresponding position | | |
| J4 | Spotlight off | Press `s` again | Dim overlay removed, full slide visible. Spotlight position clears | | |

---

## K. Presentation Aids: Zoom

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| K1 | Zoom toggle | Press `z` | Zoom mode activates. Status bar shows **[Z]oom** in green | | |
| K2 | Zoom indicator | With zoom on, observe audience window | A yellow rectangle on the audience display indicates the zoom region, with a label like "2.0x" | | |
| K3 | Zoom off | Press `z` again | Zoom indicator disappears. Zoom region clears | | |

---

## L. Slide Overview Grid

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| L1 | Open overview | Press `o` | Dark semi-transparent overlay appears with a grid of slide thumbnails. Each thumbnail shows the first page of its logical slide group. Current slide has a **light blue** border | | |
| L2 | Keyboard navigation | Press `Right`, `Left`, `Down`, `Up` in overview | Selection highlight (blue border) moves between thumbnails. Down/Up jump by one row (column count adapts to window width) | | |
| L3 | Select slide | Navigate to a slide and press `Enter` | Overview closes. Presentation jumps to the selected slide | | |
| L4 | Close overview (Escape) | Press `Escape` in overview | Overview closes. Presentation stays on current slide | | |
| L5 | Close overview (o) | Press `o` again while overview is open | Overview closes (toggle behavior) | | |
| L6 | Click thumbnail | Click a slide thumbnail with mouse | Overview closes and jumps to that slide | | |
| L7 | Slide numbers | Observe the labels below thumbnails | Each thumbnail has its 1-based logical slide number centered below it | | |
| L8 | Scrollable grid | Open overview with many slides | Grid is vertically scrollable if thumbnails don't fit in the viewport | | |

---

## M. Notes Panel

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| M1 | Notes visible by default | Open a PDF with a .pdfpc or .dais sidecar that has notes | Notes panel shows "Notes" header with the slide's markdown content rendered below. Panel is on the right side of the presenter console | | |
| M2 | Notes update on navigate | Navigate between slides | Notes content changes to match the current logical slide's notes. Slides without notes show "No notes for this slide" in gray | | |
| M3 | Toggle notes visibility | Press `n` | Notes panel hides. Press `n` again — panel reappears | | |
| M4 | Markdown rendering | Add notes with **bold**, *italic*, `code`, lists, headers | Notes render as formatted Markdown (using egui_commonmark), not raw text | | |
| M5 | Notes scrollable | Add very long notes to a slide | Notes panel has a vertical scroll area; content is scrollable without overflowing | | |
| M6 | Increase font | Press `+` or `Shift+=` | Notes text gets larger by 2pt (default step). Repeatable | | |
| M7 | Decrease font | Press `-` | Notes text gets smaller by 2pt. Minimum 8pt | | |
| M8 | Font size limits | Press `+` repeatedly (>30 times) | Font maxes out at 72pt and stops growing | | |

---

## N. Timer Display & Status Bar

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| N1 | Status bar layout | Observe the bottom status bar | Shows (left to right): slide position, separator, timer, separator, per-slide timer, separator, mode indicators, separator, jump-to-slide buffer (if active) | | |
| N2 | Slide position text | Navigate to slide 3 of 5 | Status bar shows "Slide 3/5" | | |
| N3 | Overlay step | Navigate to a multi-overlay slide, step 2 of 3 | Status bar shows "Slide 2/5 (step 2/3)" | | |
| N4 | Timer cursor | Hover over the timer text | Cursor changes to pointing hand (indicating clickable) | | |
| N5 | Mode indicators | Activate freeze, blackout, laser, ink, spotlight, zoom individually | Each shows its indicator: [F]rozen (light blue), [B]lack (yellow), [L]aser (red), [D]raw (orange), Spotlight (light yellow), [Z]oom (green) | | |
| N6 | Jump indicator | Press `g` | Status bar shows "Go to: _" in yellow. As digits are pressed, they appear (e.g., "Go to: 12_") | | |
| N7 | Multiple indicators | Activate freeze + spotlight simultaneously | Both indicators show side by side in the status bar | | |

---

## O. Toast Notifications

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| O1 | Monitor warning toast | Launch with 1 monitor (default dual config) | Yellow warning toast appears at top-right: "Single monitor detected — expected dual. Using single mode." | | |
| O2 | Toast auto-dismiss | Wait 4 seconds after O1 | Toast fades/disappears automatically | | |
| O3 | Toast dismiss button | While toast is visible, click the "×" button | Toast dismisses immediately | | |
| O4 | Monitor name mismatch toast | Set `audience_monitor = "NONEXISTENT"` with 2 monitors. Launch | Warning toast: "Configured audience monitor 'NONEXISTENT' not found. Available: <list>" | | |
| O5 | Toast stacking | If multiple warnings fire, observe | Toasts stack vertically from top-right, with spacing between them | | |

---

## P. Sidecar Loading & Saving

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| P1 | .pdfpc load | Place `beamer-example.pdfpc` next to a test PDF (rename appropriately). Launch | Notes from sidecar appear. Overlay groups are applied (check slide count vs page count). Log shows metadata source | | |
| P2 | .dais load | Place `quarto-example.dais` next to a test PDF. Launch | Notes and groups from .dais file are loaded. Check that notes and grouping match the fixture content | | |
| P3 | .dais > .pdfpc priority | Place both `.dais` and `.pdfpc` sidecars next to the same PDF. Launch | `.dais` sidecar takes priority. Log confirms which source was used | | |
| P4 | Save as dais (Ctrl+S) | With default config (`sidecar_format = "dais"`), press `Ctrl+S` | `.dais` sidecar file written next to the PDF. Log shows "Saved sidecar to <path>". File contains valid Dais metadata with current groups, notes, annotations, and text boxes | | |
| P5 | Save as pdfpc | Set `sidecar_format = "pdfpc"` in config. Press `Ctrl+S` | `.pdfpc` sidecar written. Contains pdfpc-compatible groups and notes | | |
| P6 | No sidecar | Launch a PDF with no sidecar file nearby | Works normally. Each page is its own slide (1:1 grouping). Notes panel shows "No notes for this slide" | | |

---

## Q. Grouping Editor (`--edit`)

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| Q1 | Editor layout | Run `dais --edit tests/example.pdf` | Window shows: top bar with "Grouping Editor" heading, page/slide count, "💾 Save" and "✕ Close" buttons. Below: horizontal filmstrip of page thumbnails | | |
| Q2 | Alternating group colors | Observe the filmstrip | Adjacent groups have alternating background colors (darker/lighter gray) to distinguish them visually | | |
| Q3 | Split a group | Click the "Split after" button beneath a thumbnail | A boundary is inserted. The group splits into two. Slide count increases by 1. | | |
| Q4 | Merge groups | Click the "Merge" button between two groups | Boundary removed. The two groups merge into one. Slide count decreases by 1 | | |
| Q5 | Page 0 immovable | Attempt to toggle boundary on page 0 | Nothing happens — page 0 is always the start of a boundary and cannot be toggled | | |
| Q6 | Save from editor | Click "💾 Save" | Status message briefly shows "Saved to <path>" in green. Sidecar file written in the configured format | | |
| Q7 | Close editor | Click "✕ Close" | Editor window closes | | |
| Q8 | Thumbnail labels | Observe thumbnails | Each group has a label like "Slide 1 (2 pages)" below its thumbnail row | | |
| Q9 | Scrollable filmstrip | Open a PDF with many pages | Filmstrip is horizontally scrollable | | |

---

## R. Audience Window Display Quality

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| R1 | Aspect ratio preserved | Open a 4:3 PDF on a 16:9 monitor | Slide is centered with black letterbox bars on left/right. No stretching or cropping | | |
| R2 | Aspect ratio 16:9 on 16:9 | Open a 16:9 PDF on a 16:9 monitor | Slide fills the entire audience window perfectly | | |
| R3 | Black background | Observe audience window at any aspect ratio | Non-slide area is solid black | | |
| R4 | Sharpness | Compare slide text on audience window to the original PDF | Text is crisp and readable. In dual mode, renders at the audience monitor's native resolution | | |
| R5 | Presenter vs audience resolution | In dual mode with different DPI monitors | Presenter renders at 1920×1080 (fallback). Audience renders at native monitor size (e.g., 3840×2160 for 4K). Both are sharp for their respective displays | | |

---

## S. Presenter Console Layout

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| S1 | Four-panel layout | Observe the presenter console | Four main areas: current slide (large, upper-left), next preview (upper-right), notes panel (lower-right), status bar (bottom strip) | | |
| S2 | Current slide panel | Observe | Shows the current PDF page rendered with aspect ratio preservation. Centered in its allocated area | | |
| S3 | Next preview panel | Navigate to a non-last slide | Shows a smaller preview of the next page. Helpful for anticipating transitions | | |
| S4 | Next preview on last slide | Navigate to the last slide | Next preview shows empty/blank (no next page to preview) | | |
| S5 | Resize window | Drag the presenter window to resize | All panels re-layout proportionally. No panels disappear or overlap. Minimum sizes respected | | |

---

## T. Keybinding Configuration

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| T1 | Default bindings | Press `Right`, `Left`, `Space`, `f`, `b`, etc. without any config override | All default keybindings work as documented in docs/keybindings.md | | |
| T2 | Override a binding | Add config `[keybindings]\nnext_slide = ["x"]`. Launch | `x` advances slide. `Right`, `Space`, `Down`, `PageDown` no longer advance (all defaults for that action replaced) | | |
| T3 | Multiple keys per action | Add config `[keybindings]\nnext_slide = ["x", "j"]` | Both `x` and `j` advance slides | | |
| T4 | Modifier combos | Add config `[keybindings]\ntoggle_freeze = ["Ctrl+f"]` | `Ctrl+F` freezes. Plain `f` no longer freezes (replaced) | | |
| T5 | Unknown action warning | Add config `[keybindings]\nfake_action = ["x"]`. Launch | Warning in terminal log: "Unknown action in keybinding config: fake_action". No crash | | |

---

## U. Clicker / Remote Profile

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| U1 | Default clicker profile | Without any clicker config, press `PageDown` / `PageUp` | PageDown advances slide, PageUp goes back (mapped via default clicker profile) | | |
| U2 | Custom profile | Add config: `[clicker]\nprofile = "custom"\n[clicker.profiles.custom]\nEscape = "toggle_blackout"` | `Escape` now toggles blackout instead of quitting (clicker profile overrides keybindings for that key) | | |
| U3 | Unknown profile fallback | Set `profile = "nonexistent"` | Warning in log: "Configured clicker profile 'nonexistent' not found; using default profile". Default profile applies | | |
| U4 | Test-input clicker verification | Run `--test-input` with custom clicker config | Pressing the clicker key shows the mapped action in the test-input log | | |

---

## V. Config Layering

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| V1 | Machine-wide config | Place a `config.toml` in the OS config dir (e.g., `%APPDATA%\dais\config.toml` on Windows). Set `[timer]\nmode = "countdown"` | Timer uses countdown mode on any PDF launched | | |
| V2 | Project-local config | Place `dais.toml` next to the PDF. Set `[timer]\nduration_minutes = 10` | Duration comes from project config, overriding machine-wide | | |
| V3 | `--config` highest precedence | Use `--config explicit.toml` with different timer settings | `--config` values win over project-local and machine-wide | | |
| V4 | Partial configs merge | Machine config sets timer mode. Project config sets display mode. Neither sets the other | Both timer and display settings apply. Fields not in a partial config keep their prior values | | |

---

## W. Test-Input Diagnostic Mode

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| W1 | Window opens | Run `dais --test-input` | Window titled "Dais — Test Input" at 600×500 | | |
| W2 | Key event logging | Press any key (e.g., `Right`) | Event appears in the log area showing: key name, modifiers (if any), and the mapped action (e.g., "Right → next_slide") | | |
| W3 | Unmapped key | Press a key not bound to any action (e.g., `F12` by default) | Shows the key name but "No action mapped" (or similar) | | |
| W4 | Modifier display | Press `Shift+T` | Shows "Shift+T → reset_timer" (or equivalent) | | |
| W5 | Continuous logging | Press several keys in sequence | All events appear in order. Log scrolls as entries accumulate | | |

---

## X. Edge Cases & Robustness

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| X1 | Single-page PDF | Open a PDF with exactly 1 page | Works without error. Navigation keys do nothing (already at first = last slide). Next preview is empty. End-of-deck blackout triggers on NextSlide | | |
| X2 | Large PDF | Open a PDF with 100+ pages | Opens without noticeable delay. Navigation is responsive (no multi-second lag). Slide overview grid renders thumbnails progressively | | |
| X3 | Rapid key presses | Press `Right` rapidly 20+ times | All key events processed. Slide counter advances smoothly without lag, skipping, or crashing | | |
| X4 | Rapid mode toggles | Press `f` rapidly 10 times | Freeze toggles cleanly each time. No stuck state | | |
| X5 | All aids simultaneously | Activate spotlight + zoom, navigate, toggle freeze | No crash. Overlays render in the correct layer order. State is coherent | | |
| X6 | Jump to out-of-range slide | Press `g`, type `999`, press `Enter` on a 5-slide deck | Jumps to the last valid slide (clamped by `go_to_group` bounds check). No crash | | |
| X7 | Jump to 0 | Press `g`, type `0`, press `Enter` | Jumps to slide 0 (first slide), since input is 1-based: `0.saturating_sub(1) = 0` | | |
| X8 | Quit with q | Press `q` | Application closes cleanly | | |
| X9 | Quit with Escape | Press `Escape` (when not in overview or HUD) | Application closes cleanly | | |
| X10 | Window close button | Click the OS window close button on the presenter | Application closes cleanly | | |

---

## Y. Performance Checks

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| Y1 | Startup time | Launch with a ~20-page PDF | Window appears within 2 seconds. First slide visible within 3 seconds | | |
| Y2 | Navigation latency | Press `Right` and observe slide change | Slide updates within ~100ms (perceptibly instant). No 1+ second delay | | |
| Y3 | Background prefetch | Navigate normally, then jump to a slide 3 ahead | Slide should be pre-cached and appear immediately (pipeline prefetches neighborhood) | | |
| Y4 | Timer updates | Start timer, observe updates | Timer increments smoothly every ~100ms while running (ctx.request_repaint_after(100ms)) | | |
| Y5 | Idle CPU | Leave app idle (timer stopped, no input) | CPU usage should be minimal. Repaint interval is 250ms when idle | | |

---

## Z. Logging & Diagnostics

| # | Test | Action | Expected Behavior | Result | Actual |
|---|------|--------|-------------------|--------|--------|
| Z1 | Default log level | Run `dais tests/example.pdf` normally | Terminal shows `info`-level logs: version, page count, display mode, metadata source | | |
| Z2 | Debug logging | Run `RUST_LOG=debug dais tests/example.pdf` | Verbose debug output: config file paths checked, monitor topology, render pipeline activity | | |
| Z3 | Monitor topology logged | Launch with 2 monitors | Log shows "Detected 2 monitor(s):" with name, size, position, scale, [primary] for each | | |
| Z4 | Config source logged | Launch with a project-local `dais.toml` | Log shows "Loaded config layer from <path>" | | |

---

## Test Setup Notes

### Creating test sidecars

To test sidecar loading, copy and rename fixtures:

```bash
# For pdfpc testing
cp tests/fixtures/beamer-example.pdfpc tests/example.pdfpc

# For dais testing (must rename to match PDF)
cp tests/fixtures/quarto-example.dais tests/example.dais
```

### Quick config file for testing

Save as `test-config.toml`:

```toml
[display]
mode = "dual"

[timer]
mode = "countdown"
duration_minutes = 5
warning_minutes = 1

[laser]
color = "#00FF00"
size = 16.0

[ink]
colors = ["#0000FF"]
width = 5.0

[spotlight]
radius = 200.0

[notes]
font_size = 18.0

sidecar_format = "dais"
```

Launch with: `dais --config test-config.toml tests/example.pdf`

### Multi-monitor testing

Tests B1–B8 require physically connecting a second monitor (or using a virtual display adapter). The remaining tests can be verified on a single monitor using `--single` or `--screen-share`.
