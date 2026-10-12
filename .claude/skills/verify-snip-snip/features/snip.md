# Snip

Pressing `Alt+Shift+S` anywhere freezes every monitor under a dimmed overlay; dragging a region opens exactly those screen pixels in the editor. `Esc` or a right-click cancels.

## Sub-features

- `snip-hotkey` the global hotkey opens an overlay on every monitor.
- `snip-select` a drag selects a region, inclusive of the pixels under both ends, and opens the editor at that size.
- `snip-click` a click or sub-2px jitter selects nothing; the overlay stays.
- `snip-cancel` `Esc` (or right-click) closes all overlays without an editor.
- `snip-repeat` a second hotkey press during a snip is ignored.

## How to get to it (user POV)

- Press `Alt+Shift+S` while snip-snip runs in the background.
- On the overlay: drag with the left button; `Esc`, `Alt+F4`, or right-click to cancel.

## Driving it with snipctl

Preconditions:

- Baseline preconditions; no editor open.

- **Select.** Run `snipctl target` then `snipctl snip 200 200 600 300`. `app.log` gains `snip: captured N monitor(s)` and `overlay first frame`; the step log ends `editor open`.
- **Exact pixels.** Run `snipctl key mod+c`, `snipctl clipboard snip-select`, `snipctl inspect snip-select`. Expect `600x300 blue=90000 green=90000 ink=0`.
- **Edge precision.** Close the editor, then `snipctl target`, `snipctl snip 90 90 20 20`, copy and inspect. Expect `20x20 blue=100`: only the 10x10 quarter inside the fixture.
- **Cancel.** Run `snipctl cancel-snip`. It presses the hotkey, waits for the overlay, presses `Esc`, and fails if an editor appears.
- **Proof.** `snipctl shot snip-select-editor` right after the snip shows the editor with the two-color image.

## Gotchas

- The overlay is a frozen screenshot of whatever was on screen at the keypress. Show the fixture before the hotkey, not after.
- `snip` closes the fixture once the editor opens, because the topmost fixture would cover it. Run `target` again before each snip.
- `snip` records the requested size for `drag` mapping; it cannot read the app's real size. Copy and `inspect` to confirm. One early run produced 602x300 for a 600x300 request and never reproduced; treat a mismatch as a finding and retry once.
- On Retina, both drag ends include one screenshot pixel. The 600×300 point
  recipe therefore selects 1199×599 image pixels at 2× scale. The native
  fixture's captured colors may differ from the Windows RGB constants;
  compare clipboard pixels with a screenshot of the fixture on that display.
- Right-click cancel and `snip-click` have no snipctl command; report them as skipped.
- With several monitors an overlay opens on each, but only the first is activated for keys.
