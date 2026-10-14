# snip-snip

A screenshot and annotation tool for Windows and macOS, written entirely in Rust and inspired by [Shottr](https://shottr.cc/). Press a shortcut, drag over the area you want to capture, add a few annotations, and copy or save the result.

The app lives in your system tray or menu bar, ready for the next snip. You can draw with a pencil, add arrows and rectangles, write text, and crop the image. Annotations stay editable, and undo/redo lets you change your mind. The editor uses the same renderer as the export, so what you see is what you copy or save.

## Getting started

Install [rustup](https://rustup.rs/), then run:

```sh
cargo run --release -p snip-gpui
```

The toolchain pinned in `rust-toolchain.toml` installs automatically on the first build.

Press `Alt+Shift+S` to take a screenshot, or choose **New snip** from the tray or menu-bar menu. The app freezes all your monitors while you select a region, then opens it in the editor. Press `Esc` to cancel the selection.

Closing the editor or Settings leaves the app running. Choose **Quit** from the tray or menu-bar menu to exit.

### macOS setup

The normal build needs full Xcode to compile GPUI's Metal shaders. If you only have the Command Line Tools, use runtime shader compilation instead:

```sh
cargo run --release -p snip-gpui --features gpui_platform/runtime_shaders
```

macOS also asks for Screen Recording permission when you take your first snip.

### Changing the capture shortcut

Open **Settings**, choose **Record shortcut**, press the key combination you want, then choose **Apply**. The change takes effect immediately and is saved for future sessions. Use Ctrl, Alt/Option, or Win/Command with a supported key; you can add Shift too.

Press `Esc` to cancel recording. You can also navigate Settings with Tab and activate the focused control with Enter or Space. If the shortcut is already in use or the settings can't be saved, Settings shows an error and keeps your previous shortcut. If there's a shortcut conflict at startup, Settings opens so you can fix it; **New snip** still works from the menu.

On macOS, capture shortcuts follow physical key positions across keyboard layouts. Settings labels keys by their U.S. positions, so the default shortcut stays on the physical U.S. S key even when you switch input sources. Recording a new shortcut binds the keys you actually press.

Shortcut settings live in your per-user configuration folder under `snip-snip/shortcut`. If that file is malformed, the app uses the default shortcut for the session. It only replaces the file when you explicitly choose **Apply**.

## Editing a snip

Choose a tool from the toolbar or use its keyboard shortcut:

| Key | Tool | How to use it |
| --- | --- | --- |
| `V` | Select | Click an annotation to select it, then drag to move it. |
| `P` | Pencil | Draw freehand. |
| `A` | Arrow | Drag toward the arrow's tip. |
| `R` | Rectangle | Draw a rectangle outline. |
| `T` | Text | Click to add or edit a label. Enter adds a new line; `Esc` or clicking away finishes editing. |
| `C` | Crop | Drag to choose the part of the image to keep. |

### Moving and changing annotations

Select an annotation to move it or change its style. Arrows have a handle at each endpoint, and rectangles have one at each corner. Hold Shift while drawing or resizing to snap arrows to 45° steps or make rectangles square. Click empty canvas to deselect, or press Delete or Backspace to remove the selected annotation.

To edit existing text, click it with the Text tool or double-click it with Select. Click inside the label to place the caret, or use the arrow keys and Home/End to move it.

The toolbar has five ink colors—red, yellow, green, blue, and black—and three stroke widths: Thin, Medium, and Thick. New editors start with red ink and Medium strokes. All annotation tools share the color; pencil, arrow, and rectangle also share the stroke width. Text keeps its existing font size and weight, so stroke width controls are disabled for it.

Choosing a color or width changes the selected annotation and sets the style for the next marks you draw. Selecting an annotation shows its style without changing those defaults. If you choose a color while typing, the app finishes the text edit and selects the label for recoloring.

Use `Ctrl/Cmd+Z` to undo and `Ctrl/Cmd+Shift+Z` to redo. On Windows, `Ctrl+Y` also redoes. Each completed move, resize, deletion, text edit, or style change is its own undo step. Undo and redo change the image without resetting your drawing defaults; editing text and recoloring it are separate steps.

### Cropping

Cropping keeps the original screenshot and annotations available, so you can adjust the crop later. With Crop selected, drag a corner handle to resize the crop, drag elsewhere on the image to replace it, or choose **Clear crop** to restore the full image.

When you resize, the opposite corner stays in place and each side stays at least one pixel long. The dimensions shown while dragging are screenshot pixels, regardless of zoom or display scaling. Press `Esc` to cancel a crop drag. Creating, adjusting, replacing, or clearing a crop can each be undone in one step.

### Zooming and panning

New editors start in Fit view, showing the whole screenshot and adjusting as you resize the window. Use the toolbar's **Fit** and **100%** controls or these shortcuts to change the view:

| Shortcut | Action |
| --- | --- |
| `Ctrl+scroll` on Windows, `Cmd+scroll` on macOS, or pinch | Zoom around the pointer or pinch center. |
| Hold `Space` and drag with the left mouse button | Pan; release Space or the mouse button to finish. |
| `Ctrl/Cmd+0` | Fit and center the screenshot. |
| `Ctrl/Cmd+1` | Center the screenshot at 100%. |

At 100%, one screenshot pixel takes up one physical display pixel, including on Retina and scaled Windows displays. You can zoom from the smaller of 10% and Fit up to 800%. Once you zoom manually, resizing the window preserves your zoom level and the image point at the canvas center.

Panning keeps the image edges reachable and centers any axis that already fits in the window. Press `Esc` during a pan to return to the view you started with. Zooming and panning preserve your tool, selection, and undo history, and exports always keep the screenshot's original resolution.

While editing text, Space types normally, and zooming keeps the text session and IME composition active. Navigation is ignored during drawing or other pointer gestures. Scrolling without a modifier doesn't move the view.

## Copying and saving

| Shortcut | Action |
| --- | --- |
| `Ctrl/Cmd+C` | Copy the image and keep editing. |
| `Ctrl/Cmd+Shift+C` | Copy the image and close the editor. |
| `Ctrl/Cmd+S` | Save the image as a PNG. |

Copy puts the image on the clipboard as PNG and DIB so it can be pasted into most apps. **Copy & Close** closes the editor only after the copy succeeds, and the copied image remains on the clipboard afterward. If another app has reserved its shortcut, use the toolbar button.

Save opens your native file picker in a `snip-snip` folder under Pictures, creating the folder on your first save. If Pictures isn't available, it uses a `snip-snip` folder under your home directory. The suggested filename includes the local export time, such as `snip-2026-10-11_23-59-59-007.png`, with a numeric suffix if that name already exists. You can change the filename or destination in the picker.

Saving keeps the editor open and shows the saved path. Canceling the picker takes you back to editing. If copying or saving fails, the editor shows an error and keeps your image available so you can try again.

All three actions finish any text edit and cancel unfinished drags before exporting. Selection outlines, annotation handles, and crop guides are left out of the result. The background app stays ready for your next snip.

In the editor, `Esc` handles whatever you're doing first: it finishes text editing, cancels a drag, clears the selection, or closes the editor, in that order.

## Project layout

- **`crates/snip-core`** contains the document model, renderer, geometry queries, and snapshot history. It uses tiny-skia and cosmic-text with a bundled Inter font, has no UI dependencies, and is covered by integration tests.
- **`crates/snip-gpui`** contains the app: the global shortcut, screen capture through xcap, the frozen-screen selection overlay, and the editor. It builds the `snip-snip` binary.

## Development

```sh
cargo test                 # Run the tests
cargo clippy --all-targets # Lint
cargo fmt                  # Format
```

To measure compositing time on a large screenshot (5120×2880 by default):

```sh
cargo run --release -p snip-core --example render_timing [width height]
```
