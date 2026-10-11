# snip-snip

A fast screenshot and annotation tool written entirely in Rust, inspired by [Shottr](https://shottr.cc/). Press a shortcut, drag a region, mark it up, then copy or save it.

## Features

- **Global shortcut** (`Alt+Shift+S`): freezes every monitor and opens a selection overlay.
- **Editor** with a pencil, arrows, rectangle outlines, editable text, cropping,
  selection, moving, shape handles, shared colors and stroke widths, and undo/redo.
- **Copy** to the clipboard as PNG and DIB, so it pastes into most apps.
- **Save** as a PNG at the original screenshot resolution.
- **What you see is what you export**: the editor preview is the same render that gets saved or copied.

## Getting started

You need [rustup](https://rustup.rs/). The pinned toolchain in `rust-toolchain.toml` installs automatically on first build.

```sh
cargo run --release -p snip-gpui
```

The app runs in the background with no window. Press `Alt+Shift+S` to snip.

On macOS, the build compiles GPUI's Metal shaders with full Xcode. With only the Command Line Tools, compile them at runtime instead:

```sh
cargo run --release -p snip-gpui --features gpui_platform/runtime_shaders
```

macOS also asks for Screen Recording permission on the first snip.

## Usage

| Where | Key | Action |
| --- | --- | --- |
| Anywhere | `Alt+Shift+S` | Start a snip |
| Overlay | Drag | Select a region and open it in the editor |
| Overlay | `Esc` | Cancel |
| Editor | `V` | Select (click an annotation; drag to move) |
| Editor | `P` | Pencil |
| Editor | `A` | Arrow (drag toward the tip) |
| Editor | `R` | Rectangle outline |
| Editor | `T` | Text (click to place; `Enter` or `Esc` to finish) |
| Editor | `C` | Crop |
| Editor | `Delete` / `Backspace` | Delete the selected annotation |
| Editor | `Shift` while dragging | Arrows in 45° steps; square rectangles (drawing or handles) |
| Editor | Double-click text in Select | Re-edit the label |
| Editor | `Ctrl/Cmd+Z` | Undo one completed edit |
| Editor | `Ctrl/Cmd+Shift+Z` | Redo (`Ctrl+Y` also works on Windows) |
| Editor | `Ctrl/Cmd+C` | Copy image |
| Editor | `Ctrl/Cmd+S` | Save as PNG |
| Editor | `Esc` | Finish text, cancel a drag, clear selection, or close, in that order |

Select edits one annotation at a time. Arrows have handles at both endpoints;
rectangles have handles at their four corners. Click empty canvas to deselect.
Each completed move, resize, deletion, or text edit undoes in one step.
Selection outlines and handles never appear in copied or saved images.

The toolbar offers red, yellow, green, blue, and black ink, plus Thin, Medium,
and Thick strokes. Color carries between all annotation tools; stroke width
carries between pencil, arrow, and rectangle tools. New editors start with red
and Medium. Text uses the shared color and keeps its existing font size and
weight; width controls are disabled for text.

Select an annotation to see its style, then click a color or width to restyle
it. Each changed property undoes in one step. A style choice also becomes the
default for subsequent annotations; merely selecting a mark leaves those
defaults alone. Undo/redo changes the document without reverting your drawing
defaults. Clicking a color while typing finishes the text session and selects
the label for recoloring; text and color changes undo separately.

## Project layout

```
crates/
  snip-core/   Document model and the only renderer (tiny-skia + cosmic-text,
               bundled Inter font), geometry queries, and snapshot history.
               No UI dependencies; integration tested.
  snip-gpui/   The app: global hotkey, screen capture (xcap), the frozen-frame
               overlay, and the editor. Builds the `snip-snip` binary.
```

## Development

```sh
cargo test                  # run the tests
cargo clippy --all-targets  # lint
cargo fmt                   # format

# Time compositing on a large screenshot (default 5120x2880)
cargo run --release -p snip-core --example render_timing [width height]
```
