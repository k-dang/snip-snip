Status: ready-for-agent

# Editor core model: shapes, hit testing, and undo/redo

## Problem Statement

The spike editor can only draw freehand strokes and single-line text, crop, and
export. Once something is drawn it can't be taken back: there is no undo, and
nothing can be selected, moved, or deleted. The only marks available are a
shaky freehand line and text. The most common screenshot annotations, an arrow
pointing at something and a box around it, can't be made cleanly. The document
model in the core crate has no way to say "which annotation is under the
pointer", so the selection and editing features planned next have nothing to
build on.

## Solution

Extend the framework-agnostic core so the document can hold arrows and
rectangles, answer "what is under this point" and "how big is this
annotation", move an annotation, and record an undo history of the document's
edits. Expose the new pieces in the editor with Arrow and Rectangle tools and
undo/redo shortcuts. A user can then mark up a screenshot with arrows and boxes
and step backwards and forwards through every edit, including crops and text.
The selection UI (select, move, delete, handles) is the next slice and builds on
the hit testing, bounds, and move operations added here.

## User Stories

1. As a user, I want an Arrow tool, so that I can point at a specific part of a screenshot.
2. As a user, I want to draw an arrow by dragging from where it starts to where it points, so that the arrowhead ends up where I released the mouse.
3. As a user, I want the arrowhead to scale with the line width, so that thick and thin arrows both look right.
4. As a user, I want the arrow's tip to land exactly where I released the mouse, so that I can point at small things precisely.
5. As a user, I want a click with the Arrow tool that doesn't drag to create nothing, so that a stray click doesn't leave a dot or a zero-length arrow.
6. As a user, I want a Rectangle tool, so that I can box an area of a screenshot.
7. As a user, I want to draw a rectangle by dragging between two opposite corners in any direction, so that I don't have to start from the top-left.
8. As a user, I want rectangles drawn as an outline, not filled, so that the boxed content stays visible.
9. As a user, I want a rectangle with no width or height to be discarded, so that accidental clicks don't add invisible annotations.
10. As a user, I want to see the arrow or rectangle while I'm dragging, so that I can place it precisely before letting go.
11. As a user, I want arrows and rectangles in the export to match the preview exactly, so that what I copy or save is what I saw.
12. As a user, I want arrows and rectangles drawn at the original image resolution, so that they stay sharp when the screenshot is viewed at 100%.
13. As a user, I want to switch to the Arrow tool with `A` and the Rectangle tool with `R`, so that I can change tools without the mouse.
14. As a user, I want Arrow and Rectangle buttons in the toolbar with their shortcut letters, so that I can find the tools.
15. As a user, I want to undo my last edit with Ctrl+Z (Cmd+Z on macOS), so that I can recover from a mistake.
16. As a user, I want to redo an undone edit with Ctrl+Shift+Z (Cmd+Shift+Z on macOS), so that I can change my mind about an undo.
17. As a Windows user, I want Ctrl+Y to redo as well, so that the shortcut I'm used to works.
18. As a user, I want undo to step back through every kind of edit (strokes, text, arrows, rectangles, crops), so that undo behaves the same everywhere.
19. As a user, I want a whole text label to undo in one step, not one character at a time, so that undo doesn't take dozens of presses.
20. As a user, I want re-editing an existing label to undo as one step back to its previous content, so that I can revert a bad edit to a label.
21. As a user, I want pressing undo while typing a label to finish the label first and then undo it, so that undo never leaves a half-edited label behind.
22. As a user, I want a crop to undo in one step back to the previous crop (or no crop), so that I can recover the area I cropped away.
23. As a user, I want a click that changes nothing (an empty label, a zero-size shape) not to create an undo step, so that undo never seems to do nothing.
24. As a user, I want making a new edit after undoing to discard the undone edits, so that redo behaves the way it does in every other editor.
25. As a user, I want undo and redo to do nothing once there is nothing left to undo or redo, so that pressing them too often is harmless.
26. As a user, I want copy and save to export the document as it is after any undos, so that undone edits never reappear in the output.
27. As a developer building the selection UI, I want to ask the core which annotation is topmost at a point, so that a click selects what the user sees on top.
28. As a developer, I want hit testing to take a tolerance in image pixels, so that thin lines can be clicked at any zoom level.
29. As a developer, I want a rectangle to hit only on its outline, so that clicking inside an empty box can select what's underneath.
30. As a developer, I want strokes and arrows to hit along their line, not their whole bounding box, so that a diagonal line doesn't block clicks on the area around it.
31. As a developer, I want text to hit anywhere inside its layout box, so that labels are easy to click.
32. As a developer, I want the bounding box of any annotation, including its line width, so that I can draw a selection outline around it.
33. As a developer, I want to move any annotation by an offset, so that dragging a selection is a single model operation.
34. As a developer, I want the undo history independent of GPUI, so that it is unit tested in the core and reusable by any front end.
35. As a developer, I want the undo history to ignore commits that don't change the document, so that callers can commit after every gesture without checking whether it did anything.

## Implementation Decisions

- **Two new annotation variants in the core document model.**
  - `Arrow`: a tail point, a head point, a color, and a line width, all in image
    pixels. The head is a filled triangle whose size is proportional to the
    width. The shaft stops at the base of the head, so a thick shaft doesn't
    stick out past the tip.
  - `Rectangle`: two opposite corner points (fractional image pixels, in any
    order), a color, and a line width. It is drawn as an outline with mitered
    corners and never filled. The name `Rectangle` keeps it separate from the
    existing integer `Rect`, which stays the crop type.
  - Both paint in document order alongside strokes and text, with the same
    renderer and anti-aliasing, so preview and export can't drift.
- **Geometry queries on the renderer.** Text bounds need font layout, and the
  renderer owns the fonts, so the queries live there next to the existing text
  measurement:
  - `annotation_at(document, point, tolerance) -> Option<index>`: the topmost
    (last painted) annotation hit at `point`. `tolerance` is in image pixels.
    The caller converts its screen-space slop through the current zoom.
    - Stroke: the point is within `width / 2 + tolerance` of any segment (or of
      the dot, for a single-point stroke).
    - Arrow: within `width / 2 + tolerance` of the shaft, or inside the head.
    - Rectangle: within `width / 2 + tolerance` of one of the four edges. The
      interior does not hit.
    - Text: inside the layout box, grown by `tolerance`.
  - `bounds(annotation) -> Bounds`: the axis-aligned box covering everything
    the annotation paints, line width and arrowhead included.
  - A small fractional `Bounds` type (min and max points) is added to the
    public API for this.
- **Moving.** Every annotation can be translated by an image-pixel offset. All
  of its points move together, and size, width, and color stay unchanged.
  Moving does not clamp to the image. Off-image content is simply clipped on
  export, the same as strokes today.
- **Undo history as a separate core type, `History`.** It records snapshots of
  the editable state (the annotation list and the crop), never the screenshot
  pixels. That keeps each snapshot small and leaves the document plain data.
  - `History::new(&document)` captures the initial state.
  - `commit(&document)` records the document's current editable state as a new
    step if it differs from the current step, and drops any redo steps. A
    commit that changes nothing is ignored.
  - `undo(&mut document) -> bool` and `redo(&mut document) -> bool` restore the
    previous or next step into the document. They return `false` and leave the
    document untouched when there is nothing to undo or redo.
  - The history is unbounded. An editor lives for one screenshot and snapshots
    are small next to the image.
  - Coalescing is the caller's job: the editor commits once per finished user
    action, not once per mouse move or keystroke.
- **Editor wiring (GPUI front end).**
  - New tools Arrow (`A`) and Rectangle (`R`) with toolbar buttons beside
    Pencil, Text, and Crop.
  - While dragging, the in-progress arrow or rectangle is drawn with GPUI paint
    primitives, the same way the live pencil stroke is today. On mouse-up it is
    added to the document and the preview re-renders from the core.
  - Arrows shorter than a few image pixels and zero-area rectangles are
    discarded on mouse-up.
  - Commit points: mouse-up after a stroke, arrow, rectangle, or crop, and when
    a text session ends (Enter, Escape, clicking elsewhere, switching tools, or
    exporting).
  - Undo: Ctrl/Cmd+Z. Redo: Ctrl/Cmd+Shift+Z, plus Ctrl+Y on Windows. If a text
    session is active, undo and redo end it first. That commits the label, then
    applies the undo or redo, so undo during typing removes the label in one
    step.
  - After undo or redo the editor re-renders the preview and drops any text
    editing index, since the index may no longer be valid.
  - New annotations use the editor's existing ink color and width constants.
    The style picker is a later slice.

## Testing Decisions

- **One test seam: the core crate's public API** (document, renderer, history),
  exercised by integration tests in the core's test directory. The GPUI editor
  has no automated tests. Its wiring is checked by hand, end to end, on Windows
  (and macOS when available): hotkey, snip, draw each kind of annotation, undo
  and redo through them, crop, undo the crop, then copy and save.
- **Good tests check observable behavior**: pixels in the rendered or exported
  image, the index returned for a point, the bounds returned, and the document
  state after commits, undos, and redos. They don't assert on internal
  structures or how the drawing happens.
- **Prior art**: the existing render tests. They build a blank white document,
  add annotations at known image coordinates, render or export, and assert on
  specific pixels or on where ink appears (for example, "text paints only
  inside its reported bounds"). New tests follow the same style.
- **Planned coverage**:
  - Arrows and rectangles paint at the expected image pixels, survive a crop,
    and an empty rectangle's interior stays untouched.
  - Every pixel an annotation paints lies inside its reported bounds, for each
    annotation type. This is the same property test the text test already uses.
  - Hit testing: topmost wins when annotations overlap; a rectangle's outline
    hits and its interior misses; tolerance widens a thin line's hit area; a
    point beside a diagonal stroke but inside its bounding box misses.
  - Translating each annotation type moves its painted pixels by the offset.
  - History: undo and redo restore annotations and crop; a commit after undo
    drops the redo steps; a no-op commit adds no step; undo and redo at the
    ends return `false` and change nothing.
- The existing timing example should include arrows and rectangles in its
  workload, so render cost stays measured as new types are added.

## Out of Scope

- The selection UI: a Select tool, selection outline, dragging to move,
  deleting, and resize/endpoint handles. These are the next slice. This slice
  only adds the core operations they need.
- Shift-constrained drawing (45° arrows, square rectangles).
- A color and width picker. All new annotations use the existing ink style.
- Filled or rounded rectangles, ellipses, lines without heads, and other shapes.
- Multi-line text, a text caret, and OS text input (IME).
- Zoom and pan, crop handles, and save-location defaults.
- Persisting history across editor sessions.

## Further Notes

- The project plan stores annotation coordinates in image pixels so zoom never
  changes the output. Hit-test tolerance follows that rule: it is image pixels,
  and the editor scales its screen-space slop by the current zoom.
- Snapshot undo was chosen over command-based undo. The editable state is a
  small list plus a crop, so whole snapshots are simpler and can't get out of
  sync with the operations, and no inverse operations need to be written.
- At 5K, each commit re-renders the full composite (≈ 22 ms with 40
  annotations), so undo and redo cost about that much each. That's acceptable
  for now. Caching layers is the known fix if it becomes noticeable.
