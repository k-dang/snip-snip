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

Preconditions: fixture snip; image points visible (use `key mod+0` for Fit).

- **Tools.** Draw a rectangle (`key r`, `drag 100 80 500 220`), arrow (`key a`,
  `drag 50 250 250 50`), and pencil line (`key p`, `drag 20 150 120 160`).
  Check shape placement and arrow direction; capture the combined result.
- **Text.** `key t`, `click 40 40`, `type Hello World`, `key esc`. Check case
  and placement. For caret/multiline changes, type `Hlo`, move left twice,
  insert `el`, then `end enter` and type `café`; expect Hello above café.
- **Selection/handles.** Select with `key v`, `click 100 150`, move using
  `drag 100 150 140 170`, then resize the new corner using `drag 540 240 560 260`.
  Inspect geometry; undo/redo restores each completed edit. Delete removes it.
- **Constraints, when affected.** Draw a rectangle with `drag 100 60 220 100 shift`:
  it is square. Check selected rectangle corners and arrow endpoints with Shift.
- **Text re-editing, when affected.** Create Hello, finish, then Select and
  double-click the label. Insert text at the caret and add a line; undo restores
  the original label.
- **Styles, when affected.** Use screenshot-derived palette/width coordinates
  with `click-screen`. Check representative colors/widths, selected restyling,
  disabled text widths, and undo/redo. Use `drag-preview` for live-paint changes.
- **Output.** Inspect one final saved/copied image for the expected marks and
  geometry; reuse the shared Copy/Save comparison. Add before/after exports for
  a specific restoration bug rather than for every tool/property change.

## Gotchas

- Exact-color counts are diagnostic; antialiasing and style changes affect them.
- `type` presses real keys for letters, digits, and spaces, and injects any other character as Unicode text, which reaches the editor through the OS text input path.
- While a label is being edited, tool letters type into it. Finish with `snipctl key esc` before switching tools.
- The bundled font has no CJK glyphs; CJK text arrives intact but renders as boxes.
- A click on the image with the Pencil tool draws a dot. `snipctl` focuses the editor through the OS, never by clicking the canvas.
