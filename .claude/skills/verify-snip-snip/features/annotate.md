# Annotate

The editor draws red (#E5484D) pencil strokes, arrows, rectangle outlines, and text onto the snip. The preview is the exporter's own render, so what shows is what copies and saves.

## Sub-features

- `annotate-pencil` `P` then drag draws a freehand stroke.
- `annotate-arrow` `A` then drag draws an arrow, head at the release point; drags under 3 image px are ignored.
- `annotate-rect` `R` then drag draws a rectangle outline; an empty drag is ignored.
- `annotate-text` `T` then click places a caret; typing inserts at the caret; `Enter` starts a new line; arrows, `Home`/`End`, and clicks inside the label move the caret; `Esc` or clicking away finishes; a blank label is dropped; clicking existing text re-edits it at the click.
- `annotate-tool` the active tool is highlighted in the toolbar.
- `annotate-select` `V` selects one annotation; drag to move, Delete to remove, or drag an arrow endpoint/rectangle corner to resize. Shift constrains arrows and squares.
- `annotate-style` toolbar colors and Thin/Medium/Thick widths carry across tools and restyle a selected annotation. Text uses color and disables widths.

## How to get to it (user POV)

- Keys `P`, `A`, `R`, `T` in the editor, or the matching toolbar buttons.
- Drag or click on the image.
- Select with `V`; double-click text to re-edit. Choose colors and widths in the toolbar.

## Driving it with snipctl

Preconditions:

- Baseline snip open (600x300 two-color image); the default tool is Pencil.

- **Rectangle.** `snipctl key r`, `snipctl drag 100 80 500 220`, `snipctl shot annotate-rect`. A red outline spans both colors and `Rectangle` is highlighted.
- **Arrow.** `snipctl key a`, `snipctl drag 50 250 250 50`, `snipctl shot annotate-arrow`. The arrowhead is at the upper-right end.
- **Pencil.** `snipctl key p`, `snipctl drag 20 150 120 160`, `snipctl shot annotate-pencil`. A thin red line.
- **Text.** `snipctl key t`, `snipctl click 40 40`, `snipctl type Hello World`, `snipctl key esc`, `snipctl shot annotate-text`. `Hello World` in red at the click, case preserved.
- **Multi-line and caret.** `snipctl key t`, `snipctl click 40 40`, `snipctl type Hlo`, `snipctl key left left`, `snipctl type el`, `snipctl key end enter`, `snipctl type "café"`, `snipctl key esc`, `snipctl shot annotate-text-lines`. `Hello` above `café`.
- **Proof in pixels.** `snipctl key mod+c`, `snipctl clipboard annotate`, `snipctl inspect annotate`. `ink` is above zero and blue/green drop. With only the rectangle above, expect `blue=87840 green=87840 ink=2160`.
- **Selection and history.** Reset Fit, select the rectangle's left edge with `key v` and `click 100 150`, then `drag 100 150 140 170`. Export before/after: ink geometry moves 40×20 image pixels. Resize its new bottom-right with `drag 540 240 560 260`. Undo/redo must restore exact prior exports. Re-select the edge before Delete; deletion restores the two-color fixture.
- **Constraints.** `drag 100 60 220 100 shift` with Rectangle produces a square. With Select active, drag a selected rectangle corner or arrow endpoint with Shift and verify the constrained geometry in the copied PNG.
- **Text re-editing.** Create `Hello` at (40,40), finish, then `key v` and `double-click 70 50`. `key home right right`, `type X`, `key end enter`, `type World`, and `key esc` produce `HeXllo` above `World`. Undo/redo restores the original and edited exports exactly.
- **Styles.** Derive palette/width button screen coordinates from a fresh screenshot and `layout`, then use `click-screen`. Export each swatch and width, and compare restyle undo/redo with prior PNGs. With a selected text label, a disabled width click leaves the export unchanged. `drag-preview 360 80 520 80 style-live-view` records non-red Pencil paint while the mouse is held.

## Gotchas

- `inspect` counts exact colors; antialiased edges count as neither, so totals stay below 180000.
- `type` presses real keys for letters, digits, and spaces, and injects any other character as Unicode text, which reaches the editor through the OS text input path.
- While a label is being edited, tool letters type into it. Finish with `snipctl key esc` before switching tools.
- The bundled font has no CJK glyphs; CJK text arrives intact but renders as boxes.
- A click on the image with the Pencil tool draws a dot. `snipctl` focuses the editor through the OS, never by clicking the canvas.
