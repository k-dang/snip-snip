# 1. GPUI over Tauri

Date: October 9, 2026. Status: accepted.

## Context

snip-snip is a screenshot utility inspired by Shottr: a global shortcut opens
a frozen-frame overlay, you drag a rectangle, then annotate, crop, and save or
copy in an editor. The MVP targets Windows and macOS, with Linux later. The
core is Rust.

There were two UI candidates:

- **GPUI:** an entirely Rust application with a GPU-rendered interface.
  Promising for an annotation canvas, but its APIs change often and it is
  unpublished past crates.io 0.2.2. Its overlay behavior and export path were
  unproven.
- **Tauri:** a Rust core with a TypeScript/web UI. More mature, but it adds a
  second language and a web view between the core and the canvas.

The comparison so far rested on documentation, so we built a GPUI feasibility
spike that exercises the real workflow: hotkey, snip, pencil, text, crop, save
to PNG, and copy an image that pastes into other apps. GPUI would win if the
spike worked cleanly on both platforms. Tauri would win if GPUI needed
substantial framework patches or left export unreliable.

## Decision

Use GPUI, pinned to a Zed git commit.

The spike ran the full workflow end to end on Windows 11 (two 1920×1080
monitors at 100%) and macOS (a MacBook Air's built-in Retina display). Every
GPUI problem it found was worked around in the app, without patching the
framework:

- The overlay uses fullscreen windows. On Windows, a titlebar-less window's
  invisible frame overlaps neighbouring monitors. On macOS, titled windows sit
  below the menu bar, so the overlay uses simple fullscreen.
- OS-initiated closes are redirected to `remove_window`, because GPUI on
  Windows leaks a window the OS closes (about 34 MB per editor).
- Windows opened from the background app are activated explicitly. On macOS
  the app is also activated first, and hides itself when its last window
  closes so focus goes back to the previous app.
- The editor drops each replaced preview image from the GPU atlas.
- `arboard` replaces GPUI's PNG-only clipboard.
- `gpui_platform` needs its `font-kit` feature on macOS, and needs
  `runtime_shaders` when only the Xcode Command Line Tools are installed.

Each workaround is commented where it is applied.

Export does not depend on the UI framework. `snip-core` holds the document
model and is the only renderer (tiny-skia and cosmic-text with bundled Inter).
The editor preview is that renderer's output, so preview and export can't
drift.

## Measurements

From the spike, on Windows 11 with the pinned GPUI:

| Measure | Result |
| --- | --- |
| Hotkey → overlay first frame | 70–105 ms (capture of both monitors ≈ 35 ms) |
| Copied pixels vs. screen | Identical on both monitors (only live content, e.g. video, differed) |
| Edit commit (re-render + texture prep), 1700×900 | ≈ 3 ms |
| Composite, 5K with 40 annotations | ≈ 22 ms (`render_timing` example) |
| Idle CPU / memory | ≈ 0% / ≈ 45 MB working set |
| Memory over repeated snips | Flat |

macOS timings, a pixel-exact copy check on macOS, and HiDPI or mixed-DPI
setups were not measured.

## Consequences

- Upgrading GPUI means bumping the pinned commit, `gpui_platform`, and
  `rust-toolchain.toml` together, then checking each workaround still applies
  and is still needed.
- Every edit re-renders the full composite on the CPU. At 5K that costs about
  30 ms per keystroke while typing. Caching the layers beneath the edited
  annotation is the fix if it becomes noticeable.
- The UI is Rust only. There is no web stack to maintain, but also no web
  component ecosystem to draw on.
