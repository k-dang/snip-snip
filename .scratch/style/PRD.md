Status: implemented

# Style

## Problem Statement

Every annotation currently uses the same red ink, and pencil marks, arrows,
and rectangle outlines use one fixed stroke width. Users cannot distinguish
different points in a screenshot by color, adjust emphasis with line thickness,
or change an existing annotation's appearance without recreating it.

## Solution

Add a compact, shared set of style controls to the editor toolbar: five color
swatches and three stroke widths. The chosen style carries across annotation
tools. With an annotation selected, changing a control also immediately
restyles that annotation. Each completed restyle can be undone and redone.
Live drawing, committed preview, saved PNGs, and clipboard images show the
same color and thickness.

## User Stories

1. As a screenshot author, I want five readily available colors, so that I can distinguish related annotations without opening a color picker.
2. As a screenshot author, I want three readily available stroke widths, so that I can vary emphasis without adjusting a numeric field.
3. As a screenshot author, I want to see the active color and width, so that I know how my next annotation will look.
4. As a screenshot author, I want new editors to start with the familiar red and medium width, so that the existing workflow remains predictable.
5. As a screenshot author, I want my chosen color to carry between pencil, arrow, rectangle, and text tools, so that I can annotate consistently.
6. As a screenshot author, I want my chosen width to carry between pencil, arrow, and rectangle tools, so that related marks have consistent thickness.
7. As a screenshot author, I want colored text, so that labels can match the shapes they describe.
8. As a screenshot author, I want stroke-width controls to clearly show when they do not apply to text, so that I do not expect them to change font size or weight.
9. As a screenshot author, I want to select an existing annotation and see its style, so that I can understand what I am changing.
10. As a screenshot author, I want a color change to immediately recolor only the selected annotation, so that I can correct its appearance without redrawing it.
11. As a screenshot author, I want a width change to immediately resize only the selected annotation's stroke, so that I can change emphasis without moving its geometry.
12. As a screenshot author, I want the selected annotation to remain selected after a restyle, so that I can make another adjustment immediately.
13. As a screenshot author, I want explicit style choices to also become the defaults for subsequent annotations, so that I can continue with the appearance I just chose.
14. As a screenshot author, I want selecting or deselecting annotations to leave my drawing defaults intact, so that inspecting existing marks does not silently change my next mark.
15. As a screenshot author, I want to recolor a label I am editing without losing its text, so that I can finish the label and adjust its appearance in one workflow.
16. As a screenshot author, I want to undo and redo a restyle in one step, so that I can recover a previous appearance without losing the annotation.
17. As a screenshot author, I want choosing an already-applied style to preserve redo, so that an accidental extra click does not discard work.
18. As a screenshot author, I want changes to drawing defaults to leave document undo history alone, so that undo continues to affect screenshot edits.
19. As a screenshot author, I want my chosen style to appear while I draw, so that releasing the pointer does not unexpectedly change the mark's appearance.
20. As a screenshot author, I want copied and saved images to preserve every annotation's style, so that recipients see the same emphasis I see in the editor.
21. As a screenshot author, I want controls to remain usable in small editor windows and on high-DPI displays, so that styling does not require enlarging the window.
22. As a screenshot author, I want toolbar interactions to leave drawing shortcuts available, so that I can resume annotating immediately.
23. As a screenshot author, I want style changes to leave the screenshot and crop unchanged, so that adjusting an annotation cannot change the captured region.

## Implementation Decisions

- Extend the existing GPUI editor toolbar and editor state. Keep style defaults
  local to each editor window; switching tools retains them, and a new editor
  starts fresh. Use the existing annotation fields, document model, renderer,
  and snapshot history. This slice needs no persistence, schema change, new
  rendering backend, or generic styling framework.
- Use five opaque colors: red `#E5484D`, yellow `#F5D90A`, green `#30A46C`,
  blue `#0090FF`, and black `#000000`. These are implementation defaults chosen
  for this spec; the roadmap specifies approximately five colors. Start red.
- Use Thin, Medium, and Thick presets of 1, 3, and 6 logical pixels. Start
  Medium, preserving the current 3-pixel default. Convert to image-pixel
  widths using the editor window's scale factor when creating or restyling an
  annotation, consistent with existing drawing behavior. Fitting an image to
  the canvas must not change stored widths.
- Color applies to pencil strokes, arrows, rectangle outlines, and text.
  Width applies to pencil strokes, arrows, and rectangle outlines. Text keeps
  its existing font size and weight. Screenshot pixels, crop, selection
  outlines, and handles are independent of annotation style.
- Keep one drawing color and one drawing width across tools. With no annotation
  selected or being edited, controls display these defaults. An explicit
  control choice updates the corresponding default only; it does not modify
  unrelated annotations or create a history entry.
- With an annotation selected, controls display its color and applicable
  width. Merely selecting an annotation does not overwrite drawing defaults.
  Deselecting or switching to another drawing tool reveals the defaults again.
  If an existing style does not match a preset, show no matching preset as
  active rather than silently changing the annotation. Compare displayed
  widths in logical pixels using the current window scale factor.
- An explicit color choice updates the shared drawing color and the selected
  annotation's color. An explicit width choice updates the shared drawing
  width and the selected stroke, arrow, or rectangle's width. Change only the
  chosen property; preserve geometry, content, annotation order, crop, and
  the other style property. Retain selection and the active tool.
- Disable width choices while text is selected or being edited, or while the
  Text tool is active without another selected annotation. Keep the shared
  drawing width for subsequent line tools. Color remains available. With Crop
  active and no selection, controls may update drawing defaults but never
  style the crop.
- Clicking a style control cancels an unfinished pointer gesture using the
  editor's existing cancellation behavior. Resolve cancellation before
  applying a style, so canceled geometry cannot become part of a restyle.
- If a text session is active, finish it with the existing text-session
  semantics before applying a color choice. Keep the nonempty label selected
  as the restyle target and for subsequent style adjustments. Committing changed
  text and recoloring it are separate undoable edits. An empty label is removed
  as usual; the color choice then changes only the drawing default. Continue
  typing through the existing text re-edit interaction.
- Commit each changed selected-annotation style choice as one document history
  step and refresh the preview immediately. Undo/redo restores annotation
  styles through the existing snapshot history; drawing defaults are outside
  that history and retain the user's most recent explicit choices. Preserve
  the editor's existing selection-clearing behavior on undo/redo.
- Choosing an already-applied property is a document no-op and preserves redo.
  It may still set the corresponding drawing default. Changing a default
  without changing the document also preserves redo.
- Pass each annotation's color into live drawing. In particular, remove the
  pencil preview's fixed-red paint behavior. Live widths use stored
  image-pixel widths transformed by the viewport, like committed annotations.
  Keep the core renderer authoritative for committed previews and exports,
  following the accepted GPUI architecture decision.
- Show clear active, hover, and disabled states. Give swatches readable color
  names through labels or tooltips, and distinguish the active swatch with an
  outline or marker rather than hue alone. Show width choices with visibly
  different line samples and readable names. Black needs a visible boundary
  against the dark toolbar.
- Keep tool, style, Copy, and Save controls reachable at the existing minimum
  editor size. Wrap toolbar groups cleanly when needed; do not clip controls
  or let the status label crowd them out. Return focus to the editor after a
  style choice so its existing shortcuts continue to work.
- Update user-facing editor documentation to explain shared defaults,
  restyling a selection, and the distinction between text color and stroke
  width. Keep scope limited to this roadmap slice.

## Testing Decisions

- Kevin confirmed the existing rendered-editor test seam plus native
  Windows/macOS checks. Prefer this highest existing automated seam: drive
  actual GPUI toolbar and canvas events, save through the public editor flow,
  and observe the resulting PNG. Only the OS file picker is simulated.
  No additional test-only editor API is needed.
- Follow the existing selection tests' rendered-element selectors, pointer
  helpers, keyboard commands, fixture screenshots, and saved-image assertions.
  Verify external behavior rather than private fields or helper call order.
  Use expected compositions for orchestration and targeted pixel assertions
  to independently establish chosen color and stroke coverage.
- Cover creation across all four annotation tools with a shared color and
  across all three line tools with a shared width, including switching tools.
  Exercise every palette and width preset with a compact parameterized case
  rather than duplicating the entire workflow for every combination.
- Cover selected-annotation recoloring and line-width restyling, preserving
  geometry, the other style property, unrelated annotations, and crop.
  Observe retention of selection through a second consecutive style choice.
- Cover text recoloring both after selection and during a text session,
  including an empty session. Verify that width controls do not alter text
  size, weight, or content, and that text completion and restyling undo in the
  specified order.
- Cover one-step restyle undo/redo, same-value clicks preserving redo, and
  default-only changes preserving history. Verify defaults through subsequent
  drawing, including selecting/deselecting an older annotation and undoing a
  restyle.
- Include a focused high-DPI/fitted-canvas case proving that creation and
  restyling use the specified width conversion without modifying geometry.
  Include cancellation of an unfinished move or drawing gesture before a
  style choice if event propagation changes in that area.
- Native checks on Windows and macOS must use the normal shortcut → snip →
  editor workflow. Inspect the toolbar at minimum window size and typical
  high-DPI scaling; check active/disabled indicators, black-swatch contrast,
  line samples, wrapping, focus, and shortcuts. Observe non-red pencil marks
  and dots while the pointer is held, then after release. Compare saved and
  pasted output with the preview. Automated exported-image tests do not prove
  live GPUI paint or OS clipboard behavior.
- Run the existing workspace tests, formatting check, and lint checks when
  implementing. Add lower-level core tests only if core behavior actually
  changes and the editor seam cannot adequately prove it. Avoid duplicate
  tests that merely mirror style setters.

## Out of Scope

- Custom colors, a full color picker, opacity, gradients, fill colors, dashed
  lines, arrowhead options, and per-tool style presets.
- Text size or weight controls, multiline text, caret improvements, IME, and
  dead-key support; those belong to the Text roadmap slice.
- Style persistence across editor windows or app restarts, saved themes,
  preference screens, and new style keyboard shortcuts.
- Multi-selection, batch restyling, and applying a style to all annotations.
- Zoom/pan, crop polish, save-flow changes, and app-shell work from later
  roadmap slices.

## Further Notes

This is roadmap item #3, Style. The palette and width values above resolve
routine implementation choices so an agent can implement the slice without
an interview.

Selection is implemented and its automated checks pass, but native
Windows/macOS selection checks remain outstanding in the roadmap. Record
which platform checks actually ran when implementing Style; do not claim
cross-platform verification from automated tests alone.

## Comments

- October 10, 2026: Implemented shared styles and selected-annotation restyling.
  Rendered GPUI event tests verify saved PNGs, history, text sessions, DPI,
  crop preservation, cancellation, and minimum-window layout. Native Windows
  verification could not run: the Computer Use helper reported "failed to
  connect native pipe: The system cannot find the file specified. (os error 2)"
  on both inventory attempts. Native macOS verification is also outstanding.
- Validation: all 39 workspace tests passed (20 rendered-editor tests, 14
  editor-model tests, and 5 renderer tests). Workspace/all-target type checking,
  clippy with warnings denied, and formatting checks passed. Review against
  baseline `925921d` found no Standards issues and no product defects or scope
  creep on the Spec axis; the one remaining Spec gap is native verification.
