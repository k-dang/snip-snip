# Annotate

The editor draws red (#E5484D) pencil strokes, arrows, rectangle outlines, and text onto the snip. The preview is the exporter's own render, so what shows is what copies and saves.

## Sub-features

- `annotate-pencil` `P` then drag draws a freehand stroke.
- `annotate-arrow` `A` then drag draws an arrow, head at the release point; drags under 3 image px are ignored.
- `annotate-rect` `R` then drag draws a rectangle outline; an empty drag is ignored.
- `annotate-text` `T` then click places a caret; typing adds text; `Enter` or `Esc` finishes; an empty label is dropped; clicking existing text re-edits it.
- `annotate-tool` the active tool is highlighted in the toolbar.

## How to get to it (user POV)

- Keys `P`, `A`, `R`, `T` in the editor, or the matching toolbar buttons.
- Drag or click on the image.

## Driving it with snipctl

Preconditions:

- Baseline snip open (600x300 two-color image); the default tool is Pencil.

- **Rectangle.** `snipctl key r`, `snipctl drag 100 80 500 220`, `snipctl shot annotate-rect`. A red outline spans both colors and `Rectangle` is highlighted.
- **Arrow.** `snipctl key a`, `snipctl drag 50 250 250 50`, `snipctl shot annotate-arrow`. The arrowhead is at the upper-right end.
- **Pencil.** `snipctl key p`, `snipctl drag 20 150 120 160`, `snipctl shot annotate-pencil`. A thin red line.
- **Text.** `snipctl key t`, `snipctl click 40 40`, `snipctl type Hello World`, `snipctl key enter`, `snipctl shot annotate-text`. `Hello World` in red at the click, case preserved.
- **Proof in pixels.** `snipctl key mod+c`, `snipctl clipboard annotate`, `snipctl inspect annotate`. `ink` is above zero and blue/green drop. With only the rectangle above, expect `blue=87840 green=87840 ink=2160`.

## Gotchas

- `inspect` counts exact colors; antialiased edges count as neither, so totals stay below 180000.
- `type` handles letters, digits, and spaces only.
- While a label is being edited, tool letters type into it. Finish with `snipctl key enter` before switching tools.
- A click on the image with the Pencil tool draws a dot. `snipctl` focuses the editor through the OS, never by clicking the canvas.
