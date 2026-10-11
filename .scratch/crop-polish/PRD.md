Status: implemented

# Crop polish

## Problem Statement

Screenshot authors can drag a crop in the editor, but cannot adjust it through
handles or explicitly clear it. They also cannot see its pixel dimensions
while dragging. Correcting a crop requires redrawing it and guessing the
resulting image size, particularly when zoomed or panned.

## Solution

Complete the existing Crop tool with four corner handles, a visible Clear
crop control, and a live width-by-height label during crop creation and
adjustment. Cropping remains reversible and non-destructive: the original
screenshot and annotations remain available, and only saved or copied output
is cropped. All interactions use screenshot pixels regardless of the view.

## User Stories

1. As a screenshot author, I want to activate Crop through its toolbar control or C shortcut, so that I can use the existing workflow.
2. As a screenshot author, I want to drag a new crop in any direction, so that I can frame the desired region naturally.
3. As a screenshot author, I want an existing crop to show corner handles when Crop is active, so that I can discover how to adjust it.
4. As a screenshot author, I want to drag each corner independently, so that I can correct either pair of crop boundaries.
5. As a screenshot author, I want the opposite corner to remain fixed during adjustment, so that resizing is predictable.
6. As a screenshot author, I want the pointer to indicate a corner resize, so that I know which interaction will occur before dragging.
7. As a screenshot author, I want usable handle sizes at every zoom level, so that precise adjustment does not depend on magnification.
8. As a screenshot author, I want to redraw a crop away from its handles, so that I can replace a poorly chosen region quickly.
9. As a screenshot author, I want to see width and height in screenshot pixels while creating a crop, so that I know the output size before releasing.
10. As a screenshot author, I want dimensions to update while adjusting a handle, so that I can size the resulting screenshot accurately.
11. As a screenshot author, I want the dimension label to remain readable near canvas edges, so that cropping an edge does not hide the size.
12. As a screenshot author, I want the crop to stay within the original screenshot, so that dragging beyond its boundaries produces valid output.
13. As a screenshot author, I want an adjustment to retain at least one pixel in each dimension, so that resizing cannot create an empty or inverted crop.
14. As a screenshot author, I want a simple click or empty replacement drag to preserve my previous crop, so that a mistaken gesture does not erase it.
15. As a screenshot author, I want Clear crop to restore the full image, so that I can remove cropping without recreating the capture.
16. As a screenshot author, I want Clear crop to be unavailable when no crop exists, so that the control reflects the document state.
17. As a screenshot author, I want to undo or redo crop creation, replacement, adjustment, and clearing, so that I can compare framing choices.
18. As a screenshot author, I want each completed crop gesture to undo in one step, so that history does not contain every pointer movement.
19. As a screenshot author, I want unchanged or canceled crop actions to preserve redo, so that trying an interaction does not discard work.
20. As a screenshot author, I want Escape to cancel an active crop gesture and restore the previous crop, so that I can recover without closing the editor.
21. As a screenshot author, I want a drag released outside the canvas to finish cleanly, so that the editor does not remain stuck in an adjustment.
22. As a screenshot author, I want crop handles and dimensions to follow zoom, pan, and display scaling, so that they align with the pixels I am editing.
23. As a screenshot author, I want Space-drag to pan while Crop is active, so that I can reach another part of an enlarged image without changing tools.
24. As a screenshot author, I want switching tools to preserve my committed crop, so that I can annotate after framing the screenshot.
25. As a screenshot author, I want clearing or resizing a crop to preserve all annotations and original pixels, so that content outside the crop remains recoverable.
26. As a screenshot author, I want saved and copied images to match the crop's displayed pixel dimensions, so that the output has the size I chose.
27. As a screenshot author, I want handles, dimming, outlines, and dimension labels excluded from output, so that exported screenshots contain only my image and annotations.
28. As a screenshot author, I want crop controls and existing toolbar actions to remain reachable at the minimum editor size, so that the added controls do not obstruct editing or export.
29. As a Windows or macOS user, I want the same crop interactions with the platform's existing undo, redo, save, and copy shortcuts, so that cropping fits the rest of the editor.

## Implementation Decisions

- **Architecture.** Respect the accepted GPUI architecture ADR. Extend the
  editor's input, transient gesture state, toolbar, cursor feedback, and
  overlay painting. Retain the existing document crop, integer pixel
  rectangle, snapshot history, viewport, and core export contract. No new
  document schema, rendering backend, or public test interface is needed.
- **Handle visibility and priority.** Show four corner handles only when
  Crop is active and a nonempty crop exists. The dimmed exterior and outline
  remain visible with other tools as they are today. Handle hit testing takes
  priority over starting a replacement crop, including the portion of a
  handle extending outside the image. Use the existing annotation handles'
  visual sizing and hit tolerance in logical window pixels. When hit regions
  overlap on a tiny crop, choose the closest handle with a stable tie order.
- **Creation and replacement.** A left drag starting inside the screenshot
  away from a crop handle creates or replaces the crop, including when it
  starts inside the current crop. Preserve existing any-direction creation,
  outward whole-pixel rounding, and image-bound clamping. A click or empty
  completed drag restores the previous crop and does not add history.
- **Corner adjustment.** Dragging a handle fixes the opposite corner at its
  original pixel boundary. Only the two boundaries adjacent to the dragged
  corner change. Round those boundaries outward and clamp to the screenshot
  and to a minimum width and height of one pixel. Clamp at the opposite
  corner rather than crossing it or swapping corners. Keep the original
  gesture geometry as the reference throughout the drag to avoid drift.
- **Pointer feedback.** Use the appropriate diagonal resize cursor over a
  corner and during its drag; otherwise retain the Crop crosshair. Space
  navigation keeps its existing hand cursors and takes precedence before a
  crop gesture starts. Keep handle positions and pointer targets aligned
  through the current viewport, including after DPI changes.
- **Dimensions.** While creating a valid crop or adjusting one, display
  `width × height px` from the same integer rectangle that export will use.
  The label updates with the live crop, keeps a constant logical text size,
  and uses a contrasting background. Place it near the active corner with
  an offset that leaves the handle usable; reposition and clamp it inside
  the visible canvas near edges. Hide it when the gesture finishes or is
  canceled, and while no valid creation rectangle exists. It is a display
  overlay, not document content or an editable size field.
- **Clearing.** Add a compact, clearly labeled Clear crop toolbar control
  while Crop is active. Keep it visible but disabled without a crop.
  Clearing sets the document crop to none, removes its overlays, and commits
  one changed history step without changing the active tool, original image,
  annotations, or viewport. Do not repurpose Delete, Backspace, or Escape as
  a crop-clearing shortcut. Preserve their existing editor behavior.
- **Gesture lifecycle and history.** Capture the prior crop at gesture
  start, update the live crop while dragging, and commit once on release.
  Escape cancels and restores the prior crop without a history step. Follow
  existing cancellation when switching tools, using toolbar actions, or
  invoking undo/redo. Release outside the canvas completes with image-bound
  clamping. No-op adjustments and cancellation preserve redo. Existing text
  sessions finish through the normal tool/toolbar path before crop editing;
  text and crop changes remain separate undo steps.
- **Navigation and export.** Use the current image-to-window transform for
  painting and its inverse for pointer input. Preserve Space-pan between
  crop gestures and existing suppression of navigation during a gesture.
  Crop changes do not reset Fit, magnification, or pan. Save and Copy use the
  unchanged core export path, independent of viewport and overlay state.
- **Layout and documentation.** Keep controls nonoverlapping and reachable
  at the existing minimum window size, using the toolbar's wrapping layout.
  Update editor usage documentation with adjustment, clearing, dimensions,
  and cancellation. Update the roadmap after implementation and accurately
  record native checks that remain outstanding.

## Testing Decisions

- **Agreed primary seam.** Kevin confirmed the existing rendered-editor
  GPUI harness: drive normal mouse and keyboard input, inspect visible crop
  controls and dimensions, and compare saved PNGs. Supplement it with native
  Windows/macOS visual and clipboard checks. Extend that harness rather than
  introducing another framework or an API exposing private editor state.
- **Good tests.** Assert observable behavior: displayed geometry and size,
  control availability, usable layout, history results, and exported pixels.
  Do not call private gesture handlers, inspect transient fields, or repeat
  production geometry arithmetic to define expected results. Use known
  fixture coordinates and explicit expected crop rectangles.
- **Prior art and modules.** Extend the editor's existing selection-handle,
  crop/export, cancellation/history, minimum-window toolbar, and zoom/DPI
  scenarios. The harness already simulates input and the save picker and
  observes rendered bounds and saved PNGs. Existing core crop rendering and
  snapshot-history tests cover the retained contracts; add core tests only
  if implementation requires a real change to core behavior.
- **Creation and adjustment.** Cover replacement, all four handles,
  opposite-corner anchoring, whole-pixel rounding, image-edge clamping,
  minimum-size clamping, overlapping handles, and release outside the
  canvas. Check that a handle hit resizes rather than starts a new crop.
  Combine related assertions into focused scenarios.
- **Dimensions and painting.** Observe the live label before mouse release,
  both for creation and adjustment, and its disappearance afterward and on
  cancellation. Compare its values with saved dimensions. Inspect visible
  handle and label placement, contrast, and canvas clipping; saved pixels
  alone cannot prove that an overlay is readable or correctly positioned.
- **Clearing and history.** Create, adjust, replace, and clear a crop, then
  undo and redo to verify one step per changed action and restoration of
  output. Cover empty drags, unchanged adjustment, Escape cancellation,
  disabled clearing, and preservation of redo. Use annotated or distinctively
  colored fixtures to prove clearing restores content outside the crop and
  resizing does not change annotation geometry.
- **Viewport and integration.** Exercise fitted and zoomed/panned views
  with 1x and 2x scale factors, including a scale change. Verify crop handles
  select the expected image pixels, dimensions remain image-space values,
  Space-pan still works between gestures, and other tools retain the crop.
  Check the added control at the minimum editor size alongside Copy and Save.
- **Native verification.** On Windows and macOS, drive capture-to-editor,
  create and adjust a crop, cancel, clear, undo/redo, save, and paste copied
  output into another app. Inspect cursor feedback, handle/label contrast,
  edge clipping, and alignment on scaled or Retina displays. Record missing
  platforms or hardware as outstanding rather than treating simulated input
  as native verification.
- **Required checks.** After implementation, run workspace tests, workspace
  lint checks with warnings denied, formatting, and diff checks. This PRD
  publication alone does not require building or testing the application.

## Out of Scope

- Edge-midpoint handles, dragging the entire crop to move it, aspect-ratio
  locks, Shift constraints, numeric size entry, presets, and snapping.
- Destructive cropping, image resampling or rotation, multiple crops, and
  persisting crop preferences between editor windows.
- Save-flow changes, default folders or filenames, copy-and-close, app-shell
  work, launch-at-login, and hotkey configuration.
- Linux support, GPUI upgrades, renderer/cache redesign, or a new test
  framework without a demonstrated blocker.
- Completing all earlier slices' outstanding native checks as a prerequisite
  to this feature.

## Further Notes

This is MVP roadmap slice 6, following implemented zoom and pan. Crop creation,
overlay dimming, history storage, and cropped export already exist; this slice
finishes their user interaction rather than replacing those contracts.

Four corner handles, non-crossing adjustment, a Crop-only Clear crop control,
and a drag-only dimension label are concrete defaults synthesized for this
specification. They were not separately requested by Kevin. The testing seam
was explicitly confirmed. Keep implementation within this scope unless an
observed usability or platform issue requires revisiting a decision.

## Verification

Implemented through the agreed rendered-editor GPUI seam. Seven focused crop
scenarios cover all four corners, outward rounding, fixed opposite corners,
grab offsets, image and minimum-size clamping, overlapping handles, live
dimensions and placement, clearing, cancellation and no-op history, and
zoom/pan at both DPI scales and across a scale change. Existing minimum-window
toolbar coverage now includes Clear crop.

Checks passed:

- Workspace tests: 68 total (22 core, 46 editor).
- All-target editor typecheck and workspace Clippy with warnings denied.
- Formatting and diff checks.
- Windows release build.

Native Windows verification used the repository's snipctl workflow against
the final release binary. It drove the global shortcut and frozen-screen
selection into the editor, created a crop, adjusted its bottom-right corner,
captured a readable live dimension label, cleared the crop, undid and redid
clearing, canceled an in-progress resize with Escape, and saved through the
native file picker. Saved and clipboard-readback PNGs were identical at
447 × 229 pixels; clearing restored the original 600 × 300 fixture with
90,000 blue and 90,000 green pixels. Undo restored the cropped PNG, redo
restored the full image, and Escape restored identical pre-gesture output.
Handles and dimensions were excluded from exported pixels. The verification
app and fixture processes were cleaned up.

Evidence is retained in the local build directory under
`target/verify/1791734577/`, including the live-resize screenshot, exports,
clipboard image, and action/app logs. The computer-use helper could not
connect to its native pipe (Windows error 2), but snipctl's OS input and window
capture worked. macOS, real mixed-DPI hardware checks, native cursor appearance,
and pasting into another application remain outstanding. Clipboard readback
proves the image payload but does not replace an application-paste check.

## Comments

Independent Standards and Spec reviews compared the working tree and new crop
modules against the preimplementation commit. Standards reported no findings.
Spec identified stale resize-cursor feedback when Delete or Backspace canceled
a crop with no annotation selected. Cursor refresh and notification now run
after cancellation independently of annotation deletion; the existing editor
scenario also covers those cancellation keys. The Spec reviewer rechecked the
fix and reported zero unresolved findings.
