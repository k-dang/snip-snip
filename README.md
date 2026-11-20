# snip-snip

A screenshot and annotation tool for Windows and macOS, written in Rust and inspired by [Shottr](https://shottr.cc/).

![The snip-snip editor with a rectangle, arrow, pencil mark, and text label on a screenshot](docs/images/editor.png)

## Download

Grab the Windows or macOS ZIP from [Releases](https://github.com/k-dang/snip-snip/releases/latest). The macOS app isn't notarized yet, so after unzipping, run `xattr -dr com.apple.quarantine snip-snip.app` before opening it.

## Run from source

Install [rustup](https://rustup.rs/), then:

```sh
cargo run --release -p snip-gpui
```

On macOS without full Xcode, add `--features gpui_platform/runtime_shaders`. macOS asks for Screen Recording permission on the first snip.

The app lives in the tray / menu bar. Press `Alt+Shift+S` (changeable in **Settings**) or choose **New snip**, drag over a region, and it opens in the editor.

## Shortcuts

| Key | Action |
| --- | --- |
| `V` `P` `A` `R` `T` `C` | Select, Pencil, Arrow, Rectangle, Text, Crop |
| `Shift` while drawing | Snap arrows to 45° / make rectangles square |
| `Delete` | Remove the selected annotation |
| `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` | Undo, redo |
| `Ctrl/Cmd+scroll`, `Space`+drag | Zoom, pan |
| `Ctrl/Cmd+0`, `Ctrl/Cmd+1` | Fit, 100% |
| `Ctrl/Cmd+C` | Copy and close |
| `Ctrl/Cmd+S` | Save as PNG |
| `Esc` | Back out of the current action, then close |

## Development

- `crates/snip-core` — document model and renderer (no UI dependencies)
- `crates/snip-gpui` — the app: shortcut, capture, selection overlay, editor

```sh
cargo test
cargo clippy --all-targets
cargo fmt
```
