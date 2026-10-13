# Snip

The configured global shortcut (initially `Alt+Shift+S`) freezes every monitor.
Dragging selects the frozen region and opens it in an editor; Escape or a
right-click cancels.

## Sub-features

- Global shortcut and overlays on each monitor.
- Region selection includes the source pixel under both drag ends.
- A click or tiny jitter leaves the overlay open; a repeated shortcut is ignored.
- Cancellation closes overlays without an editor.

## How to get to it (user POV)

Press the configured shortcut, then drag a region. Escape or right-click cancels.

## Driving it with snipctl

Preconditions: launched, healthy app; no editor open.

- **Capture.** `target`, `snip 200 200 600 300`, then `shot snip-editor`.
  Confirm the blue/green fixture appears with the intended boundary.
  Copy once (`key mod+c`, `clipboard snip-copy`) and inspect its dimensions/content.
  The Windows baseline is 600×300. macOS selects source pixels from point coordinates,
  so dimensions depend on display scaling.
- **Cancel.** `cancel-snip` opens capture and sends Escape; it fails if an editor opens.
- **Precision, when capture/scaling changes.** Capture an asymmetric region with a
  known boundary and check its position and dimensions. On a scaled display,
  compare exported content with a native reference of the selected screen region.
  Include multiple monitors when monitor placement/selection changes.

## Gotchas

- Show the fixture before capture: the overlay freezes what is already visible.
  `snip` closes the owned fixture after opening the editor; use `target` for each snip.
- On a 2× macOS display the baseline drag selects 1199×599 source pixels:
  each drag end includes one source pixel, rather than a whole screen point.
- Native display profiles may change fixture RGB values. Use a same-display
  reference for fidelity checks.
- `cancel-snip` uses the default shortcut. With another configured chord, open
  capture with that chord and use `shell-key esc` to cancel.
- Right-click/tiny-jitter checks require explicit desktop input. Report them as
  untested if they are relevant to the change and were not exercised.
