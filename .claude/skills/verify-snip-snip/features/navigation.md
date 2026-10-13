# Navigation

Modified wheel or pinch zooms around the pointer. Space+drag pans; Fit and 100%
set the view. Navigation preserves edits, history, and original export resolution.

## Sub-features

- Modified wheel/pinch zoom; plain wheel leaves the view unchanged.
- Temporary pan with reachable image edges and Escape cancellation.
- Fit/100% presets, resize behavior, and display-scale changes.
- Drawing, selection, text, and crop follow the displayed view.

## How to get to it (user POV)

Ctrl+wheel (Windows) or Cmd+wheel (macOS), touchpad pinch, Space+drag,
Fit/100% toolbar controls, or `mod+0`/`mod+1`.

## Driving it with snipctl

Preconditions: fixture snip with an annotation; canvas points from `layout`.

- **Zoom/presets.** `scroll-screen X Y -10 mod` zooms around the pointer;
  `scroll-screen X Y 3` leaves placement unchanged. Check Fit/100% via shortcuts
  and, when toolbar controls changed, their visible buttons.
- **Pan.** `drag-screen X1 Y1 X2 Y2 space` moves the view; check both bounds.
- **Resize.** On Windows, `resize-editor 480 320` changes the client area.
  Manual zoom retains magnification; Fit responds to the new size.
- **Editing.** Image-coordinate `drag`/`click` follow the current painted transform.
  Draw or move an annotation at zoom and inspect the resulting geometry.
  For text changes, type, zoom while editing, continue typing, and inspect the caret/label.
- **Output invariant.** Copy a reference before navigation and one after zoom,
  pan, and resize; `compare reference after-navigation` must pass. Capture the
  final view. Additional intermediate exports are useful only to localize a failure.
- **Scaling, when affected.** Check 100% maps a source pixel to a physical display
  pixel, including a scaled display; check pointer mapping after a scale change.

## Gotchas

- Image points outside the visible canvas cannot be clicked. Reset Fit to reveal them.
- Wheel injection does not prove hardware pinch. Use a touchpad when pinch is in scope.
- Navigation is ignored during drawing or another active pointer gesture.
