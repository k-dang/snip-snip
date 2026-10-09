# Cross-platform screenshot utility

Planning note — October 8, 2026.

## Product direction

Build a screenshot utility inspired by Shottr, with a Rust core and a simple,
polished capture-to-edit workflow.

- MVP platforms: Windows and macOS. Linux follows later.
- A web UI with a Rust backend is acceptable; an entirely Rust application is
  also an option.
- Prefer simplicity, robustness, and long-term maintainability when choosing
  technology.

## MVP scope

The main workflow is: activate snipping, drag a rectangular selection, open it in
the editor, annotate or crop, then save or copy the resulting image.

The basic editor should support:

- Text annotations.
- A pencil/freehand drawing tool.
- Cropping.
- Saving an image and copying it to the clipboard.

A small set of shapes is a candidate addition. Undo/redo was proposed as part of
the editing experience; its scope still needs to be finalized.

## Technology decision

Evaluate GPUI before committing to Tauri. No framework has been selected yet.

- **GPUI:** an entirely Rust application with a custom GPU-rendered interface.
  Promising for an annotation canvas, but its changing APIs, overlay behavior,
  and image export approach need validation.
- **Tauri:** a Rust core with a TypeScript/web UI. An alternative if the GPUI
  evaluation exposes substantial framework patching or a fragile export path.

The discussion so far was based on documentation and source review, not a
working prototype or comparative benchmark.

## Agreed next step: a GPUI feasibility spike

Build the smallest prototype that exercises the actual product workflow:

1. Global shortcut → rectangular snip → editor.
2. One pencil tool and one editable text annotation.
3. Cropping.
4. Save to PNG and copy an image that can be pasted into another application.
5. Validate on Windows and macOS, including mixed-scale multiple monitors.

The spike must establish:

- Reliable capture overlay placement, focus, cancellation, and cleanup.
- Correct conversion between screen coordinates, UI coordinates, and image
  pixels.
- Exported text, strokes, and crop boundaries matching the preview at original
  image resolution. Capturing the editor window is not a suitable export path.
- Responsive drawing on a large screenshot, reasonable idle resource use, and
  acceptable shortcut-to-selection latency. Measure these rather than assuming
  a framework is faster.

If these work cleanly on both platforms, favor GPUI. If they require substantial
framework patches or leave export unreliable, favor Tauri. This is a feasibility
exercise, not the full editor implementation.

## Spike status (October 8, 2026)

Run with `cargo run --release -p snip-gpui`, then press **Alt+Shift+S**. In the
editor: `P` pencil, `T` text, `C` crop, `Ctrl/Cmd+C` copy, `Ctrl/Cmd+S` save,
`Esc` close. Timings are logged to stderr.

- `crates/snip-core`: the document model and the only renderer (tiny-skia
  strokes, cosmic-text with bundled Inter). Framework-agnostic and unit tested.
- `crates/snip-gpui`: GPUI app with the hotkey (`global-hotkey`),
  capture (`xcap`), frozen-frame overlay, and editor. The editor preview is the
  core renderer's output, so preview and export cannot drift. GPUI is a git
  dependency pinned to a Zed commit, since crates.io stops at the year-old
  0.2.2 and the split `gpui_platform` crates are unpublished.

Verified end-to-end on Windows 11 with two 1920×1080 monitors at 100% scale,
on the pinned GPUI:

| Measure | Result |
| --- | --- |
| Hotkey → overlay first frame | 70–105 ms (capture of both monitors ≈ 35 ms) |
| Copied pixels vs. screen | Identical on both monitors (only live content, e.g. video, differed) |
| Edit commit (re-render + texture prep), 1700×900 | ≈ 3 ms |
| Composite, 5K with 40 annotations | ≈ 22 ms (`render_timing` example) |
| Idle CPU / memory | ≈ 0% / ≈ 45 MB working set |
| Memory over repeated snips | Flat |

Clipboard holds PNG, `CF_DIBV5`, and `CF_DIB`. Save writes a PNG.

GPUI problems found on Windows. Three found on 0.2.2 are fixed in the pinned
commit: `PopUp` windows are topmost, `QuitMode::Explicit` keeps the app alive
with no windows, and closing a window frees its GPU atlas. Still present:

1. A titlebar-less window keeps a caption style whose invisible frame extends
   it ~8 px past its bounds, so adjacent overlays overlap and steal edge
   clicks. Overlays use GPUI fullscreen on Windows instead.
2. A window closed by the OS (title bar, Alt+F4) leaks entirely (about 34 MB
   per editor): its drag-drop registration is revoked after the HWND is
   destroyed, which fails. Overlay and editor redirect OS closes to
   `remove_window`.
3. The image clipboard writes only PNG, which many apps can't paste.
   `arboard` is used instead.
4. A window opened from a background app isn't activated, so it wouldn't
   receive keys. Overlay and editor call `activate_window`.
5. A painted image stays in the window's GPU atlas until the window closes,
   so the editor drops each replaced preview with `drop_image`.

Still open:

- macOS has not been run yet. Overlay level, focus, and Retina mapping are
  coded but unverified.
- Mixed-DPI monitors are untested. This machine has two 100% displays, and the
  display settings were not changed.
- A selection dragged onto another monitor gets stuck until Esc.
- At 5K, typing re-renders the full image per keystroke (≈ 30 ms). Caching the
  layers beneath the edited text would fix it if needed.

## Proposed editor design

Keep the original screenshot, an ordered list of editable annotations, and a
crop rectangle. Store annotation coordinates in image pixels so zooming does
not change the output. Save and copy should consume the same rendered result.

## References

- [Shottr](https://shottr.cc/)
- [GPUI](https://gpui.rs/)
- [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)
- [GPUI painting example](https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/painting.rs)
- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
- [XCap capture library](https://github.com/nashaofu/xcap)
