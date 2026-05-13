# Dais — A Native PDF Presenter Console

## What It Is

Dais is a cross-platform PDF presentation console written in Rust, targeting researchers and academics who build slides in LaTeX/Beamer, Typst, PowerPoint, or Keynote and need a serious presenter tool that works natively on any platform without painful dependency chains or WSL workarounds. It ships as a single self-contained binary per platform — download, run, done.

Dais treats all platforms as first-class from the start, with the explicit goal of never creating a situation where switching platforms means losing your presentation tool. Compatibility with the `.pdfpc` sidecar format means users with existing notes and grouping metadata can bring them along.

**License: MIT**

---

## Design Principles

**No platform should be second-class.** The entire motivation for this project is that existing tools work well on one platform and poorly on others. Every architectural decision should be evaluated against whether it risks recreating that problem on a different OS.

**Distribution should be trivial.** One binary per platform, no runtime dependencies, no installers, no bundled DLLs. Download and run.

**Degrade gracefully.** A tool that only works perfectly under ideal conference room conditions is not a tool academics can rely on. Single-monitor mode, Zoom/screen-share mode, and unexpected hardware configurations all need defined behavior.

**Don't block the future.** Several features are explicitly deferred to post-v1, but the internal architecture should be designed so that adding them later requires extending the system rather than rewriting it. Constraints on day-one design are called out explicitly where they exist.

**Typst is the happy path, everything else is fully supported.** Typst with Polylux or touying produces the cleanest metadata. Beamer, PowerPoint, and Keynote PDF exports all work, with documented paths for getting the best experience from each.

---

## Stack

**PDF rendering — [hayro](https://github.com/typst/typst)**
A pure Rust, zero system dependency PDF processing library created by @LaurenzV, now embedded in the Typst compiler itself. Handles PDF rendering, rasterization, and SVG conversion with no external libraries. Because it is 100% Rust it compiles into the binary on all platforms identically — no DLL bundling, no static linking complexity, no per-platform binary matrix. Actively maintained by people deeply embedded in the academic PDF tooling ecosystem. Prototype needed to validate the renderer API surface is sufficient for a viewer use case; [mupdf-rs](https://github.com/messense/mupdf-rs) is the fallback if not.

**GUI — [egui](https://github.com/emilk/egui) + [eframe](https://github.com/emilk/egui/tree/master/crates/eframe)**
Immediate-mode, pure Rust, no system dependencies, genuinely cross-platform. The draw-every-frame model suits a presentation tool naturally. Built-in painter API handles pointer and ink drawing directly.

**Multi-monitor — abstracted `MonitorManager` trait with platform backends**
Defined as a trait from day one, with separate platform implementations compiled in via `cfg`. The monitor placement prototype covering all backends is the project's go/no-go gate before any other feature work begins.

- **Windows** — Win32 via [windows-rs](https://github.com/microsoft/windows-rs) (`EnumDisplayMonitors`, `SetWindowPos`)
- **macOS** — `NSScreen` via [objc2](https://github.com/madsmtm/objc2); known egui tab-instead-of-window issue needs an explicit fix
- **Linux/X11** — [x11rb](https://github.com/psychon/x11rb)
- **Linux/Wayland** — window placement is intentionally not exposed to applications by the Wayland protocol; fullscreen requests are compositor-dependent and best-effort. Documented limitation.

**Configuration — [directories](https://crates.io/crates/directories) + TOML**
Global preferences (monitor assignments, timer duration, pointer color, keybindings) stored in a TOML config file at the platform-appropriate location — `%APPDATA%` on Windows, `~/.config` on Linux, `~/Library/Application Support` on macOS. The `directories` crate handles path resolution cross-platform. Per-presentation state lives in `.pdfpc` sidecar files in v1. The config format and action names are public and documented from v1, since retrofitting a keybinding system later is painful.

**Notes rendering — [egui_commonmark](https://github.com/lampsitter/egui_commonmark)**
Markdown notes rendered natively inside egui. Reads and writes `.pdfpc` sidecar files.

**Distribution — GitHub Actions + GitHub Releases**
Native builds on `windows-latest`, `ubuntu-latest`, and `macos-latest` runners. Because hayro is pure Rust the release artifacts are single self-contained binaries — `dais-windows-x86_64.exe`, `dais-linux-x86_64`, `dais-macos-aarch64` — zipped for convenience. No DLL bundling, no per-platform matrix of bundled libraries. Post-v1: Winget manifest, Scoop bucket, Homebrew formula.

---

## Architectural Constraints for Future Extensibility

These are not v1 features but are constraints on how v1 is built internally. Getting them wrong means painful rewrites later; getting them right costs almost nothing up front.

**Command bus and state broadcast.** Every user action — next slide, previous slide, toggle laser, blackout, set pointer position, start timer, etc. — is a discrete typed message dispatched to the presentation engine through an internal command bus. The engine maintains a single authoritative `PresentationState` struct and broadcasts state changes to all subscribers. The UI renders `PresentationState` and nothing else; it holds no authoritative state of its own. This design means adding external control interfaces later — a REST API, a WebSocket connection for an iPad or Surface control surface, a CLI remote — is a matter of adding new input sources and state subscribers without touching the engine. Direct coupling between input handlers and renderer calls is explicitly forbidden.

**`DocumentSource` trait.** The rendering pipeline accepts a `DocumentSource` trait rather than a concrete PDF type. In v1 the only implementation is a hayro-backed PDF loader. This trait is what allows native Typst source file support to be added later — a live-compiled Typst document becomes another `DocumentSource` implementation — without changing anything downstream in the engine or renderer.

**Render loop runs on a timer.** The render loop ticks continuously rather than only on slide change events, even if in v1 every page renders to a static frame. This is the prerequisite for video playback support later, where a page's render output is an animated surface rather than a static bitmap. A render loop that only fires on navigation events cannot be retrofitted for animation without significant redesign.

**Per-window rasterization resolution.** Page rasterization resolution is a runtime parameter passed per render call, not a global constant baked in at document load time. Each window requests rasterization at its own appropriate resolution. In v1 this means the presenter and audience windows each get pages rendered at the right sharpness for their respective DPI. Post-v1 this is also the mechanism for any zoom or magnification features.

**Shared `.pdfpc` data structures.** The data structures for reading and writing `.pdfpc` sidecar files live in a shared internal crate accessible to both the presentation engine and any UI that edits them (the manual grouping editor, the notes editor). They are not embedded in either the engine or the UI layer. This keeps the sidecar format as a first-class internal abstraction rather than an implementation detail of one component. Critically, `.pdfpc` is a compatibility serialization format — it is not the internal data model. Dais's internal presentation metadata types are Dais's own; `.pdfpc` is one format they can be read from and written to.

**Sidecar format is an isolated module.** Following from the above: all sidecar read/write logic lives behind a format abstraction layer. In v1 the only format is `.pdfpc`. A future native `.dais` format slots in as an additional implementation without touching anything that consumes sidecar data. The engine and UI work with Dais's internal types throughout; format concerns are confined to the boundary.

---

## V1 Scope — All or Nothing

These feature areas ship together or the tool has no value over opening the PDF in a browser.

### 1. Multi-Monitor Presenter View

- Audience window — fullscreen on the designated output monitor, current slide only, no chrome
- Presenter window — on the presenter's monitor, containing everything below
- Monitor assignment — configurable, survives across sessions via TOML config
- Freeze — locks the audience screen while the presenter navigates freely
- Blackout — blanks the audience screen entirely for Q&A or breaks
- Aspect ratio handling — correct letterboxing/pillarboxing for 16:9 and 4:3 content on any screen, no stretching

### 2. Degraded Display Modes

Full dual-monitor is the primary mode but Dais must handle these without crashing or requiring reconfiguration:

- **Single-monitor mode** — windowed split view with audience left and presenter console right; or floating notes window alongside a fullscreen slide. Activated automatically when only one display is detected, or manually via config.
- **Screen-share/Zoom mode** — audience window presented as a normal shareable window rather than exclusive fullscreen. Essential for remote presenting. Togglable at runtime.
- **Unexpected hardware** — if the configured monitor is unavailable at launch, Dais falls back gracefully with a clear prompt to reassign rather than a crash or frozen display.

### 3. Presentation Aids

- **Laser pointer** — colored dot on the audience screen driven by mouse position on the presenter screen; color and size configurable
- **Freehand ink** — draw on the current slide for emphasis; single keypress to clear
- **Spotlight** — dims everything outside a moveable square to focus audience attention
- **Zoom** — keyboard zoom into a region of the current slide on the audience screen

### 4. Presenter Console & Notes

- Current slide + next slide preview thumbnail
- Per-slide notes panel with Markdown rendering, with configurable font size
- Timer — elapsed or countdown, configurable duration, color shift on warning and overrun
- Slide overview grid — all thumbnails, click to jump
- Logical slide count display where grouping metadata is available, raw page count otherwise
- Read and write `.pdfpc` sidecar files

### 5. Configuration

- TOML config file at platform-appropriate path via `directories` crate
- All keybindings remappable — actions are named and documented, config maps keys to actions
- Pointer color, size, and style preferences
- Default timer duration
- Preferred display mode (dual/single/screen-share)
- Monitor assignment persistence

---

## Slide Grouping and Overlay Support

Overlay grouping — treating multiple PDF pages as build steps of a single logical slide — affects slide counting, navigation, and pacing throughout the UI. Dais handles this through a priority chain rather than heuristics.

**1. Polylux/touying metadata**
[Polylux](https://github.com/andreasabel/polylux) and [touying](https://github.com/touying-typst/touying) embed pdfpc-compatible metadata directly into the compiled PDF. Dais reads this and gets correct grouping automatically with no sidecar needed. This is the best experience.

**2. `.pdfpc` sidecar file**
Explicit user-provided grouping that Dais reads and writes. Works for any toolchain.

**3. Beamer with the `\pdfpc` LaTeX package**
`\usepackage{pdfpc}` in the Beamer preamble embeds the same metadata into the PDF on compile. One-line change, documented prominently as the recommended Beamer path.

**4. Manual grouping UI**
For PowerPoint PDF exports and Beamer users without `\pdfpc` metadata, Dais provides a simple sidecar editor where page group boundaries can be set manually and saved. Heuristic detection is explicitly not attempted — it misfires too often and false groupings are actively disruptive during a talk.

**5. No grouping**
Falls back to raw PDF page count. Works fine for simple exports where each slide is one page.

---

## Quarto Compatibility

Quarto is a primary authoring environment for the academic audience Dais targets. Compatibility maps cleanly onto the existing source table with no new architecture required, but both workflows should be explicitly documented.

**Quarto + projector (Typst/Polylux)**
[Projector](https://github.com/christopherkenny/projector) converts a Quarto document to Polylux syntax and is designed as a near drop-in for Beamer users migrating to Typst. Because projector outputs Polylux, the compiled PDF contains full pdfpc-compatible metadata. Dais reads it automatically. This is the recommended Quarto workflow and requires no extra steps.

**Quarto + Beamer**
Quarto's `{.notes}` divs compile to Beamer `\note{}` macros but do not automatically produce a `.pdfpc` sidecar. Adding the following to the Quarto YAML front matter routes notes through the pdfpc LaTeX package and embeds them in the PDF metadata:

```yaml
format:
  beamer:
    include-in-header:
      text: |
        \usepackage[overridenote]{pdfpc}
```

This one addition gives Quarto Beamer users automatic notes and overlay support in Dais. It should be the first thing in the Dais documentation for Quarto users.

---

## Source Compatibility

| Source | Overlay grouping | Notes |
|---|---|---|
| Typst + Polylux/touying | ✅ Automatic | Best experience, recommended workflow |
| Quarto + projector | ✅ Automatic | Outputs Polylux, identical to above |
| Beamer + `\pdfpc` package | ✅ Automatic | One-line preamble addition |
| Quarto + Beamer + pdfpc header | ✅ Automatic | One-line YAML addition, documented |
| Beamer without `\pdfpc` | 🔧 Manual sidecar | Editor built in, documented path |
| PowerPoint PDF export | 🔧 Manual sidecar | Animations expand to separate pages |
| Keynote PDF export | ✅ Trivial | No animations in export by default |
| Other PDF sources | ✅ Trivial | Single page per slide, just works |

---

## Open Questions Before Planning Begins

**hayro renderer API validation.** hayro needs early prototyping to confirm the public API supports a viewer use case — specifically page-to-bitmap rendering at arbitrary resolution, correct handling of 16:9 and 4:3 aspect ratios, page count and dimension introspection, and PDF outline/bookmark access. If the API is too tightly coupled to Typst compiler internals, mupdf-rs is the fallback. This prototype runs in parallel with the monitor placement work.

**Multi-monitor prototype covers all platforms.** The deliverable before any other feature work is a minimal two-window egui app that correctly places each window fullscreen on the right monitor on Windows, macOS, and Linux/X11, handles DPI mismatch between screens gracefully, and documents Wayland behavior. This is the project's go/no-go gate.

**High DPI and mixed DPI rendering.** A MacBook Pro connected to a 1080p projector is the canonical academic presenting scenario and involves two screens at very different pixel densities. The per-window rasterization resolution constraint above addresses this architecturally, but the correct behavior needs explicit design and testing — specifically that the presenter screen renders crisply at retina resolution while the audience screen is not wastefully over-rendered.

**Clicker/remote input.** Most USB presentation clickers emulate PageUp/PageDown and work with egui out of the box. Bluetooth HID clickers vary. Test early against real hardware. RFID rings and other exotic input devices that emulate keypresses work automatically via the remappable keybinding system and need no special handling.

---

## Future Possibilities

These are explicitly not v1 features. They are documented here because each one has a corresponding architectural constraint in the v1 design that ensures it can be added later without a rewrite.

**External control surface (REST API / WebSocket)**
The command bus architecture means adding a local network control interface is an extension, not a rewrite. A REST or WebSocket server running alongside the presentation engine would allow an iPad, Surface, or phone to serve as a touch-friendly remote — showing notes, thumbnails, and timer while sending commands to the engine. Bidirectional state sync via WebSocket is the right transport since it avoids polling. The iPad browser could run a small web app with no native app required. This is also the interface that would support any future automation, scripting, or exotic input devices like RFID triggers that go beyond simple keypress emulation.

**Video triggers**
Rather than full embedded video playback — which requires platform-specific media pipelines and undermines the single-binary distribution story — Dais will support a linked video trigger convention. A slide note beginning with a structured marker (exact syntax TBD, lives in the `.dais` sidecar format when that exists) causes Dais to launch the system's default video player with a specified file when the presenter hits a designated key. No codec handling, no audio pipeline, no platform media APIs. Reliable on every platform because the OS handles playback. The render loop timer constraint ensures that if full embedded video is ever pursued later, the pipeline is ready for it without redesign.

**Native `.dais` sidecar format**
The `.pdfpc` format is an INI-like text format designed around pdfpc's specific feature set. It handles the basics but has no clean way to represent richer metadata — video triggers, per-slide pointer annotations, complex grouping hints, or anything Dais-specific. The internal data model is Dais's own types from day one; `.pdfpc` is just one serialization of them. When the need arises, a native `.dais` format will be introduced. The current candidate is [EON](https://github.com/emilk/eon), a human-friendly config format created by Emil Ernerfeldt — the same author as egui. The shared provenance is a coherent fit, and EON's native support for Rust enum/sum types maps cleanly to the kind of per-slide variant data (note vs. video trigger vs. grouping marker) that a richer sidecar needs. EON is young (0.2.0, released August 2025) and TOML remains the right choice for the v1 global config file where stability and editor support matter. The `.dais` format is a later milestone, not an afterthought — the sidecar abstraction layer in v1 is what makes it addable cleanly. When introduced, Dais will look for a `.dais` sidecar first and fall back to `.pdfpc` if not found, preserving full backwards compatibility.

**Native Typst source support**
The Typst compiler is available as a pure Rust library crate under the Apache-2.0 license. Combined with [tinymist](https://github.com/Myriad-Dreamin/tinymist)'s incremental compilation pipeline and the reflexo vector IR renderer, there is a credible path to Dais accepting `.typ` source files directly — compiling them internally and rendering with sub-page delta updates. This would give Typst users a live-recompile-on-change workflow during rehearsal, perfect overlay metadata without any sidecar, and slide counts derived directly from document structure. The `DocumentSource` trait constraint is what makes this addable without touching the engine.

**Full embedded video playback**
If video triggers prove insufficient and there is clear user demand, full embedded video remains a future possibility. The render loop timer constraint ensures the pipeline is ready. The most realistic cross-platform path is [ffmpeg-next](https://crates.io/crates/ffmpeg-next) rather than platform-specific media APIs, accepting that it requires shipping a pre-built ffmpeg alongside the binary and partially weakens the single-binary story. This is a significant undertaking and should be driven by actual user demand rather than anticipation.

---

## Explicit Non-Goals for V1

- External control surface / REST API (architecture supports it, implementation deferred)
- Native Typst source file support (architecture supports it, implementation deferred)
- Video triggers (architecture supports it via render loop timer, implementation deferred)
- Native `.dais` sidecar format (architecture supports it via format abstraction, implementation deferred)
- Heuristic overlay detection
- Full embedded video playback (possible future, significant undertaking)
- An installer on any platform
- Per-slide ink persistence across sessions

---

## Name Notes

CLI invocation is `dais <file.pdf>`. Verify `dais` is available on [crates.io](https://crates.io) before writing any code — `dais-rs` is the fallback. Repository at `github.com/<org>/dais`.
