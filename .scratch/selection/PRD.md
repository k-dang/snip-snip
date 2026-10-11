Status: implemented

# Selection

## Problem Statement

After annotating a screenshot, users cannot select an existing annotation to
move, reshape, or delete it. Fixing a misplaced arrow or rectangle requires
undoing work and drawing it again, potentially undoing unrelated edits along
the way. Text can already be reopened with the Text tool, but the editor lacks
a consistent way to manipulate all annotation types.

## Solution

Add a Select tool for editing one annotation at a time. Users click an
annotation to reveal its selection outline, drag it to move it, and press
Delete or Backspace to remove it. Selected arrows expose endpoint handles;
selected rectangles expose corner handles. Double-clicking a selected text
label opens the existing text editor.

Holding Shift while drawing or reshaping an arrow constrains it to 45° steps;
holding Shift while drawing or reshaping a rectangle makes it square. Every
completed change is individually undoable, and an unfinished drag can be
cancelled without altering the document.

## User Stories

1. As a screenshot annotator, I want a visible Select tool in the toolbar, so that I can discover how to edit annotations I have already drawn.
2. As a screenshot annotator, I want to activate Select with a keyboard shortcut, so that I can switch quickly between drawing and editing.
3. As a screenshot annotator, I want to select an arrow by clicking its shaft or head, so that I can correct its placement.
4. As a screenshot annotator, I want to select a rectangle by clicking its outline, so that I can adjust the highlighted area.
5. As a screenshot annotator, I want to select a pencil stroke or dot, so that I can move or remove a freehand mark.
6. As a screenshot annotator, I want to select a text label by clicking its layout area, so that I can reposition it without changing its content.
7. As a screenshot annotator, I want overlapping annotations to select in their visible stacking order, so that the annotation I see on top is the one I edit.
8. As a screenshot annotator, I want clicks inside an unfilled rectangle to reach annotations underneath, so that an outline does not block the rest of my screenshot.
9. As a screenshot annotator, I want a clear outline around the selected annotation, so that I know which annotation will change.
10. As a screenshot annotator, I want clicking another annotation to replace the selection, so that I can choose the next annotation to edit.
11. As a screenshot annotator, I want clicking empty canvas to clear the selection, so that I can inspect the screenshot without selection controls.
12. As a screenshot annotator, I want dragging an annotation to move it without jumping to the pointer, so that I can position it precisely.
13. As a screenshot annotator, I want movement to preserve an annotation's shape, style, and stacking order, so that repositioning does not change its appearance.
14. As a screenshot annotator, I want Delete and Backspace to remove the selected annotation, so that I can discard a mark without undoing unrelated work.
15. As a screenshot annotator, I want to drag either endpoint of a selected arrow while its other endpoint stays fixed, so that I can adjust where the arrow starts or points.
16. As a screenshot annotator, I want to drag any corner of a selected rectangle while its opposite corner stays fixed, so that I can resize the highlighted area.
17. As a screenshot annotator, I want rectangle resizing to work in every direction, including across the opposite corner, so that I can reshape it naturally.
18. As a screenshot annotator, I want Shift to constrain a new or edited arrow to 45° steps, so that horizontal, vertical, and diagonal arrows are easy to draw.
19. As a screenshot annotator, I want Shift to constrain a new or edited rectangle to a square, so that I can draw regular boxes precisely.
20. As a screenshot annotator, I want releasing Shift during a drag to restore freeform geometry, so that I can change my mind before committing.
21. As a screenshot annotator, I want to double-click text in Select to edit the existing label, so that I can correct its wording without creating another label.
22. As a screenshot annotator, I want text-editing keystrokes to edit text rather than delete the annotation or switch tools, so that typing remains predictable.
23. As a screenshot annotator, I want each move, resize, deletion, or completed text re-edit to undo and redo in one step, so that I can recover from mistakes easily.
24. As a screenshot annotator, I want selecting, deselecting, and clicking without changing an annotation to leave undo and redo untouched, so that navigating the editor does not create edits.
25. As a screenshot annotator, I want Esc to cancel a move or resize and restore the original annotation, so that I can abandon a change safely.
26. As a screenshot annotator, I want dragging to keep tracking after I leave the image area, so that releasing over the surrounding canvas does not leave an edit stuck.
27. As a screenshot annotator, I want selection controls and hit areas to remain usable on fitted and high-DPI screenshots, so that I can edit regardless of the screenshot's display scale.
28. As a screenshot annotator, I want copied and saved images to contain the edited annotations without outlines or handles, so that the exported screenshot looks finished.

## Implementation Decisions

- Keep the existing Rust core and GPUI editor architecture, as established by
  the accepted GPUI ADR. The core document remains the editable screenshot;
  the core renderer remains the source of committed preview and export pixels.
- Add Select to the existing tool list and toolbar, with the unmodified `V`
  shortcut. Keep Pencil as the initial tool. Tool shortcuts do not intercept
  active text input or command-modified keys.
- Support one selected annotation. Selection is transient editor state, not
  part of the document, export, or undo history. Use the existing ordered
  annotation collection; this feature does not require persistent annotation
  IDs or a new selection framework.
- In Select, click or begin dragging on an annotation to select it. Resolve
  hits through the core renderer's existing geometry-aware hit testing and
  topmost paint order. Rectangle interiors and empty portions of a stroke's
  bounding box are not annotation hit areas. Clicking empty canvas or its
  surrounding padding clears selection. Switching to a drawing or crop tool
  clears selection after resolving any active edit.
- Use a four-logical-pixel hit tolerance, converted through the current
  viewport to image pixels. Draw a contrasting selection outline using the
  core renderer's bounds. Keep control dimensions independent of image scale:
  handles are eight logical pixels across, with a six-logical-pixel hit radius.
  These dimensions are defaults to verify visually during implementation.
- Show two handles at an arrow's actual tail and head and four handles at a
  rectangle's geometric corners. Pencil strokes and text have an outline but
  no resize handles. Selected handles take precedence over annotation hits;
  when handle hit areas overlap, choose the closest handle with a stable
  tie-break. The selection outline itself does not create another hit region.
- Start a move from the pointer's original grab position. Translate the
  original annotation by the pointer displacement, preserving its shape,
  stroke width, color, text size, content, and stacking order. Keep the
  annotation selected after a completed move or resize. Show a move cursor
  over movable ink or text and appropriate resize cursors over handles.
- Arrow endpoint drags leave the other endpoint fixed. Rectangle corner drags
  leave the opposite corner fixed, including when the dragged corner crosses
  it. Crossing must not make the active handle jump to another corner.
- Apply Shift constraints to both creation and handle editing. Snap arrows
  to the nearest multiple of 45° around the fixed endpoint, preserving the
  pointer's radial distance. For rectangles, use the greater absolute pointer
  displacement for both sides while preserving the drag quadrant. Shift does
  not constrain whole-annotation movement. Recompute from the current pointer
  and modifier state when Shift is pressed or released during a drag.
- Preserve the current creation validity rules: an arrow must be at least
  three image pixels long, and a rectangle must have nonzero width and height.
  If an endpoint or corner drag ends in invalid geometry, restore the original
  annotation without recording an edit. Do not leave invisible, unselectable
  annotations behind.
- Reuse the editor's viewport conversion and window-level drag tracking.
  Movement and reshaping may extend beyond image bounds; the renderer clips
  ink to the screenshot without changing the stored geometry. Releasing within
  the window but outside the canvas still completes the gesture. Preserve the
  existing crop while editing annotations, including those in the dimmed area
  outside the crop. Crop adjustment remains a separate tool.
- Show movement and resizing continuously, with no stationary duplicate of
  the original annotation and no changes to stacking order. Live feedback must
  agree with committed geometry. Draw selection controls as editor overlays;
  never insert them into the core document or exported pixels.
- Delete or Backspace removes the selected annotation and clears selection
  when text input is inactive. With no selection, these keys do nothing.
  Double-clicking a topmost text label in Select enters the existing text
  editing session without creating a label or committing a preliminary move.
  Place the caret at the end, consistent with the current text editor. Keep the
  existing Text tool behavior. Finish re-editing with Enter, Esc, a click away,
  tool change, or export; remove an empty label as the current editor does.
  Finishing a label in Select keeps it selected if it still exists.
- A completed changed move, resize, deletion, or text session records exactly
  one history snapshot. Selection changes and unchanged or cancelled gestures
  create no history entries and preserve redo. A new completed edit after undo
  discards redo through the existing history behavior.
- Esc first finishes active text input using the current behavior; otherwise,
  it cancels an active gesture and restores the original annotation while
  retaining selection. With an idle selection it clears selection; with no
  active edit or selection it closes the editor. Tool changes and export
  cancel unfinished pointer gestures. Undo and redo cancel unfinished
  gestures, finish active text as they currently do, and clear selection so a
  collection change cannot leave a stale selected annotation.
- Keep the existing copy, save, and crop export paths. Update the documented
  editor shortcuts and interactions as part of implementation.

## Testing Decisions

- Testing seams confirmed with Kevin: exercise the real GPUI editor, supported
  by focused tests through the existing public core document, renderer, and
  history APIs. Introduce no new testing abstraction for this slice.
- A good test proves observable behavior: which annotation is hit, how its
  geometry or rendered pixels change, what undo restores, and what the user
  sees or exports. Avoid tests of private gesture variants, incidental call
  counts, or snapshots of internal state that merely repeat the implementation.
- Use the existing core integration tests as prior art. They already cover
  topmost geometry-aware hits, transparent rectangle interiors, bounds,
  translation of every annotation type, cropped export, text-session history,
  and preservation of redo on unchanged commits. Reuse that coverage rather
  than duplicating it. Add core tests only for new externally visible core
  behavior required by the implementation.
- Validate event handling at the highest seam: start the native application,
  take a snip with the real shortcut, and operate its editor using pointer and
  keyboard input. Inspect selection controls and feedback visually. Use a
  controlled screenshot with overlapping arrows, rectangles, strokes, a dot,
  and text so hit targets and exported changes are easy to assess.
- Required selection checks: toolbar and `V` activate Select; all annotation
  types can be selected and moved; the topmost hit wins; empty rectangle
  interiors allow underlying hits; blank clicks clear selection; outlines and
  handles follow the selected annotation without altering export.
- Required geometry checks: both arrow endpoints and all rectangle corners
  resize about the correct fixed point; rectangle corners can cross their
  opposite corner; Shift works during creation and resize and toggles during
  the gesture; invalid final geometry restores the original annotation;
  movement preserves style and stacking order.
- Required edit lifecycle checks: Delete and Backspace remove only the selected
  annotation; double-click reopens a text label without duplication or movement;
  text keystrokes retain their editing meaning; one undo reverses each completed
  edit; redo restores it; selection-only, unchanged, invalid, and cancelled
  interactions preserve redo; Esc follows its stated precedence; undo and redo
  never leave controls attached to the wrong annotation.
- Required viewport and output checks: release a drag outside the image but
  inside the window; select and edit on a screenshot fitted smaller than its
  native size; verify controls and hit areas at high DPI; edit with an existing
  crop; copy and save while selection is visible; copy or save during a live
  move and confirm the unfinished move was cancelled. Inspect exported pixels
  and paste the copied image into an image-capable application.
- Check native behavior on Windows and macOS. Record the platform, scaling,
  scenarios exercised, and any platform verification still outstanding. Do not
  treat core tests as proof that native double-click, keyboard, or drag events
  work on an untested platform.
- Run the repository's Rust tests, lint, and formatting checks when implementing
  the feature. This PRD publication itself requires only document verification.

## Out of Scope

- Multiple selection, marquee selection, grouping, layer reordering, duplicate,
  copy/paste of annotations, and keyboard nudging.
- Rotation, general bounding-box transforms, resizing text or pencil strokes,
  rectangle edge handles, and editing individual pencil points.
- Style controls, colors, and adjustable stroke widths; these belong to the
  next roadmap slice.
- Multiline text, caret repositioning, text selection, and OS input-handler
  work for IME and dead keys; these belong to the Text slice.
- Zoom and pan controls, crop handles, save-flow changes, app-shell changes,
  and Linux support.
- A new UI framework, generic command system, persistent selection schema,
  or standalone automation framework.

## Further Notes

This is slice 2 of the MVP roadmap, following the completed editor core model.
It establishes the single-selection interaction needed by the later Style
slice, without implementing style changes now.

The detailed interaction defaults above synthesize the roadmap, the current
editor behavior, and the conversation. They make the slice implementable
without a separate design interview. Implementation should verify the control
sizes and cursor feedback in the native editor, especially on small fitted
screenshots and high-DPI displays.

Implementation and automated checks are complete. Native verification remains
outstanding; see [verification results](VERIFICATION.md) for the evidence and
remaining checks.
