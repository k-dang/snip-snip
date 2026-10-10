# snip-snip

A fast screenshot and annotation tool written entirely in Rust, inspired by [Shottr](https://shottr.cc/). Press a shortcut, drag a region, mark it up, then copy or save it.

## Features

- **Global shortcut** (`Alt+Shift+S`): freezes every monitor and opens a selection overlay.
- **Editor** with a pencil, arrows, rectangle outlines, editable text, cropping,
  and undo/redo.
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
| Editor | `P` | Pencil |
| Editor | `A` | Arrow (drag toward the tip) |
| Editor | `R` | Rectangle outline |
| Editor | `T` | Text (click to place; `Enter` or `Esc` to finish) |
| Editor | `C` | Crop |
| Editor | `Ctrl/Cmd+Z` | Undo one completed edit |
| Editor | `Ctrl/Cmd+Shift+Z` | Redo (`Ctrl+Y` also works on Windows) |
| Editor | `Ctrl/Cmd+C` | Copy image |
| Editor | `Ctrl/Cmd+S` | Save as PNG |
| Editor | `Esc` | Finish text or cancel a drag; otherwise close |

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
