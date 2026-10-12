# Navigation

Ctrl/Cmd+wheel or pinch zooms around the pointer. Space+drag temporarily pans.
Fit and 100% work through toolbar controls and shortcuts. Navigation preserves
document edits, history, and original export resolution.

## Sub-features

- `navigation-zoom` modified wheel changes magnification; plain wheel does not.
- `navigation-pan` Space+drag moves the view and stops at reachable image edges.
- `navigation-presets` `mod+0` fits; `mod+1` centers at 100%.
- `navigation-edit` drawing, selection, and text editing follow the displayed view.
- `navigation-export` navigation and resizing preserve exported pixels.

## How to get to it (user POV)

- Hold Ctrl on Windows or Cmd on macOS while scrolling over the canvas.
- Hold Space and drag the canvas; release to resume editing.
- Click Fit/100% or use their shortcuts; resize the editor normally.

## Driving it with snipctl

Preconditions:

- Baseline fixture snip open, with annotations and a copied/saved reference PNG.
- Derive absolute canvas and toolbar points from a current screenshot and `layout`.

- **Presets.** `key mod+1`, `shot navigation-100-view`, then `key mod+0`
  and `shot navigation-fit-view`. Also click both toolbar controls. At 100%,
  one source pixel occupies one screen pixel on Windows.
- **Wheel.** `scroll-screen X Y -10 mod` zooms in ten notches. Record the
  percentage and pointer anchoring. `scroll-screen X Y 3` leaves placement
  unchanged. Compare copied pixels to the reference.
- **Pan.** `drag-screen X1 Y1 X2 Y2 space` moves the view. Photograph the
  changed placement and exercise both pan bounds. Copy/save still matches
  the reference at its original resolution.
- **Resize.** Windows `resize-editor 480 320` sets the client area; inspect
  wrapping and available canvas. Manual zoom retains its percentage; Fit
  responds to the new size. Restore the previous client dimensions.
- **Editing.** Use absolute `drag-screen`/`click-screen` positions at zoom.
  Export to verify image geometry, and compare undo/redo to prior PNGs. Start
  typing in Fit, wheel-zoom while editing, then continue typing and inspect
  the caret, final label, and native saved PNG.

## Gotchas

- Image-coordinate helpers assume Fit; after zoom/pan use absolute screen input.
- A wheel event proves wheel delivery. Hardware pinch needs a real touchpad;
  record it as skipped when that hardware is unavailable.
- Crop or active drawing gestures can ignore navigation input. Finish the
  current pointer gesture before testing a view change.
