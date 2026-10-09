# snip-snip

A fast screenshot and annotation tool written entirely in Rust, inspired by
[Shottr](https://shottr.cc/). Press a shortcut, drag a region, mark it up, then
copy or save it.

> **Status:** early feasibility spike built on [GPUI](https://gpui.rs/).
> Verified on Windows 11; macOS is coded but not yet run. See
> [PROJECT.md](PROJECT.md) for the plan, measurements, and open issues.

## Features

- **Global shortcut** (`Alt+Shift+S`): freezes every monitor and opens a
  selection overlay.
- **Editor** with a pencil, editable text, and cropping.
- **Copy** to the clipboard as PNG and DIB, so it pastes into most apps.
- **Save** as a PNG at the original screenshot resolution.
- **What you see is what you export**: the editor preview is the same render
  that gets saved or copied.

## Getting started

You need [rustup](https://rustup.rs/). The pinned toolchain in
`rust-toolchain.toml` installs automatically on first build.

```sh
cargo run --release -p snip-gpui
```

The app runs in the background with no window. Press `Alt+Shift+S` to snip.
Timings are logged to stderr.

## Usage

| Where | Key | Action |
| --- | --- | --- |
| Anywhere | `Alt+Shift+S` | Start a snip |
| Overlay | Drag | Select a region and open it in the editor |
| Overlay | `Esc` | Cancel |
| Editor | `P` | Pencil |
| Editor | `T` | Text (click to place; `Enter` or `Esc` to finish) |
| Editor | `C` | Crop |
| Editor | `Ctrl/Cmd+C` | Copy image |
| Editor | `Ctrl/Cmd+S` | Save as PNG |
| Editor | `Esc` | Close |

## Project layout

```
crates/
  snip-core/   Document model and the only renderer (tiny-skia + cosmic-text,
               bundled Inter font). No UI dependencies; unit tested.
  snip-gpui/   The app: global hotkey, screen capture (xcap), the frozen-frame
               overlay, and the editor. Builds the `snip-snip` binary.
```

A document is the original screenshot, an ordered list of annotations, and an
optional crop. All coordinates are in image pixels, so zoom never changes the
output.

## Development

```sh
cargo test                  # run the tests
cargo clippy --all-targets  # lint
cargo fmt                   # format

# Time compositing on a large screenshot (default 5120x2880)
cargo run --release -p snip-core --example render_timing [width height]
```

GPUI is pinned to a Zed commit in `Cargo.toml` because crates.io lags behind.
When bumping it, bump the toolchain in `rust-toolchain.toml` to match.

## License

MIT. The bundled Inter font is under the SIL Open Font License; see
[`crates/snip-core/fonts/Inter-LICENSE.txt`](crates/snip-core/fonts/Inter-LICENSE.txt).
