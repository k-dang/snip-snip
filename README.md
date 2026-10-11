# snip-snip

A fast screenshot and annotation tool written entirely in Rust, inspired by [Shottr](https://shottr.cc/). Press a shortcut, drag a region, mark it up, then copy or save it.

## Features

- **Configurable global shortcut** (initially `Alt+Shift+S`): freezes every monitor and opens a selection overlay.
- **Tray/menu-bar controls** for New snip, Settings, and Quit.
- **Editor** with a pencil, arrows, rectangle outlines, editable text, cropping,
  selection, moving, shape handles, shared colors and stroke widths, and undo/redo.
- **Zoom and pan** with wheel or pinch zoom, Space+drag navigation, and Fit/100% controls.
- **Copy** to the clipboard as PNG and DIB, so it pastes into most apps.
- **Save** as a PNG at the original screenshot resolution.
- **What you see is what you export**: the editor preview is the same render that gets saved or copied.

## Getting started

You need [rustup](https://rustup.rs/). The pinned toolchain in `rust-toolchain.toml` installs automatically on first build.

```sh
cargo run --release -p snip-gpui
```

The app runs in the background with a Windows tray icon or macOS menu-bar
icon. Press `Alt+Shift+S` to snip, or choose **New snip** from its menu.

Open **Settings**, choose **Record shortcut**, press your preferred chord,
then choose **Apply**. Use Ctrl, Alt/Option, or Win/Command with a supported
key; Shift may be added. Escape cancels recording. Tab moves between the
controls, and Enter or Space activates the focused control. Successful
changes take effect immediately and survive restarts. If a chord is in use
or saving fails, the previous shortcut remains available and Settings shows
an error. A startup shortcut conflict opens Settings; New snip still works.

Closing Settings or an editor leaves the app running. Use **Quit** from the
tray/menu-bar menu to exit. Shortcut settings are stored in the per-user
configuration folder under `snip-snip/shortcut`; malformed settings use the
default for the current session and are repaired only through an explicit Apply.

On macOS, the build compiles GPUI's Metal shaders with full Xcode. With only the Command Line Tools, compile them at runtime instead:

```sh
cargo run --release -p snip-gpui --features gpui_platform/runtime_shaders
```

macOS also asks for Screen Recording permission on the first snip.
Capture shortcuts currently support the macOS ABC and U.S. keyboard layouts.
Other layouts show a Settings error and retain menu-based New snip, so the
app cannot silently bind a different physical key. Changing to a supported
layout restores shortcut registration when the chord is available.

## Usage

| Where | Key | Action |
| --- | --- | --- |
| Anywhere | Configured shortcut (initially `Alt+Shift+S`) | Start a snip |
| Overlay | Drag | Select a region and open it in the editor |
| Overlay | `Esc` | Cancel |
| Editor | `V` | Select (click an annotation; drag to move) |
| Editor | `P` | Pencil |
| Editor | `A` | Arrow (drag toward the tip) |
| Editor | `R` | Rectangle outline |
| Editor | `T` | Text (click to place or re-edit; `Enter` for a new line; `Esc` or click away to finish) |
| Editor | `C` | Crop |
| Editor | `Ctrl+scroll` (Windows) / `Cmd+scroll` (macOS), or pinch | Zoom around the pointer or pinch center |
| Editor | Hold `Space` + left-drag | Pan temporarily; release Space or the mouse button to finish |
| Editor | `Ctrl/Cmd+0` | Fit and center the screenshot |
| Editor | `Ctrl/Cmd+1` | Center at 100% |
| Editor | `Delete` / `Backspace` | Delete the selected annotation |
| Editor | `Shift` while dragging | Arrows in 45° steps; square rectangles (drawing or handles) |
| Editor | Double-click text in Select | Re-edit the label at the clicked point |
| Editor | Arrows, `Home`/`End` while typing | Move the caret; click inside the label to place it |
| Editor | `Ctrl/Cmd+Z` | Undo one completed edit |
| Editor | `Ctrl/Cmd+Shift+Z` | Redo (`Ctrl+Y` also works on Windows) |
| Editor | `Ctrl/Cmd+C` | Copy image |
| Editor | `Ctrl/Cmd+S` | Save as PNG |
| Editor | `Esc` | Finish text, cancel a drag, clear selection, or close, in that order |

New editors open in Fit, which shows the entire screenshot and follows window
resizing. Wheel or pinch zoom chooses a manual view; resizing preserves its
magnification and the image point at the canvas center. The toolbar shows the
current percentage and offers Fit and 100% actions with their shortcuts.
At 100%, one screenshot pixel occupies one physical display pixel, including
on Retina and scaled Windows displays.

Zoom ranges from the smaller of 10% and Fit up to 800%. Pan follows your drag,
keeps image edges reachable, and centers axes that already fit. Escape cancels
an active pan and restores its starting view. Navigation preserves your tool,
selection, and undo/redo history; it leaves saved and copied resolution intact.
Space types normally while editing text, and zoom keeps text editing and IME
composition active. Navigation input is ignored during drawing or another
pointer gesture. Unmodified scrolling does not navigate.

Select edits one annotation at a time. Arrows have handles at both endpoints;
rectangles have handles at their four corners. Click empty canvas to deselect.
Each completed move, resize, deletion, or text edit undoes in one step.
Selection outlines and handles never appear in copied or saved images.

Crop keeps the original screenshot and annotations available. With Crop active,
drag any of the four corner handles to adjust an existing crop; the opposite
corner stays fixed, and resizing stops at a minimum of one pixel per side.
Drag elsewhere on the image to replace the crop, or click **Clear crop** to
restore the full image. Width and height appear in screenshot pixels while
dragging, regardless of zoom or display scaling. Escape cancels a crop drag;
each completed creation, adjustment, replacement, or clear undoes in one step.
Crop handles and dimensions never appear in saved or copied images.

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
